//! Who answers "does this change contradict that unit", and how the tool reaches them.
//!
//! The provider is the operator's choice (D16), and the default build carries no TLS stack (D18): a
//! local model over plain HTTP needs none, and the self-contained objective means one binary with no
//! external tools. A hosted provider is the `hosted` feature, which adds the TLS client.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum JudgeError {
    #[error("{0}")]
    Transport(String),
    #[error("{provider} answered {status}: {body}")]
    Status {
        provider: String,
        status: u16,
        body: String,
    },
    #[error("the answer is not the JSON asked for: {0}")]
    Shape(String),
    #[error("the model truncated the prompt ({charged} tokens at a {window} window): what it dropped is the head, where the units to judge are")]
    Truncated { charged: u64, window: u32 },
    #[error("{0} is not set")]
    NoKey(String),
    #[error("{0} needs the `hosted` feature: this build carries no TLS stack")]
    NotCompiled(String),
}

/// What the provider charged, so a run can compare it with what it predicted.
#[derive(Debug, Default, Clone, Copy)]
pub struct Charged {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// One verdict from the model, before validation.
#[derive(Debug, Clone, Deserialize)]
pub struct RawVerdict {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub diff_line: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    verdicts: Vec<RawVerdict>,
}

/// How to reach a model. Everything here is a setting (D16).
#[derive(Debug, Clone)]
pub struct Provider {
    /// `ollama`, or `openai` for any OpenAI-compatible endpoint.
    pub kind: String,
    pub endpoint: String,
    pub model: String,
    /// The environment variable holding the key, for a provider that needs one.
    pub key_var: String,
    /// Context window the model will take, prompt and answer together.
    pub window: u32,
    /// Tokens reserved for the answer, and the generation cap.
    pub answer_tokens: u32,
    /// Characters per token for this model's tokenizer (D16). Code runs about two on Qwen, prose about
    /// four; a wrong value here is what makes a provider truncate silently.
    pub chars_per_token: f32,
    pub timeout: Duration,
}

impl Provider {
    /// The zero-cost default: a model on this machine.
    pub fn local(model: impl Into<String>) -> Provider {
        Provider {
            kind: "ollama".into(),
            endpoint: "http://localhost:11434/api/chat".into(),
            model: model.into(),
            key_var: String::new(),
            window: 32768,
            answer_tokens: 2048,
            chars_per_token: 2.0,
            timeout: Duration::from_secs(420),
        }
    }

    pub fn is_local(&self) -> bool {
        self.kind.eq_ignore_ascii_case("ollama")
    }
}

/// Anything that can answer a question. A trait so a pipeline is testable without a model, and so a
/// caller may supply its own client.
///
/// The primitive is the model's text, because the two callers want different shapes from it: `extract`
/// reads its own JSON, `check` reads verdicts. Parsing verdicts is provided here so both agree on what a
/// malformed answer is.
pub trait Judge {
    fn ask_text(&self, system: &str, user: &str) -> Result<(String, Charged), JudgeError>;

    /// What the answers are attributed to, for the record.
    fn describe(&self) -> String;

    /// Characters of input this model can be shown, if it is known. A caller that has more than this
    /// must cut it or split it, and say so (D17). `None` means the judge will not say, and the caller
    /// keeps its own default.
    fn input_chars(&self) -> Option<usize> {
        None
    }

    fn ask(&self, system: &str, user: &str) -> Result<(Vec<RawVerdict>, Charged), JudgeError> {
        let (text, charged) = self.ask_text(system, user)?;
        Ok((parse_answer(&text)?, charged))
    }
}

/// What the instructions and the answer's shape take, beside the text a caller wants judged. Measured
/// from the prompts here: the longest is under three thousand characters.
const FRAMING_CHARS: usize = 4_000;

/// The shipped client.
pub struct ProviderJudge {
    pub provider: Provider,
}

impl Judge for ProviderJudge {
    fn describe(&self) -> String {
        format!("{}:{}", self.provider.kind, self.provider.model)
    }

    /// The window less the answer's room, as characters, less what the instructions occupy.
    ///
    /// A provider whose tokenizer is kinder than `chars_per_token` says simply leaves room unused, which
    /// is the safe direction: the costly mistake is being shown more than fits, which a provider answers
    /// by silently dropping the beginning.
    fn input_chars(&self) -> Option<usize> {
        let p = &self.provider;
        let room =
            f64::from(p.window.saturating_sub(p.answer_tokens)) * f64::from(p.chars_per_token);
        Some((room as usize).saturating_sub(FRAMING_CHARS).max(2_000))
    }

    fn ask_text(&self, system: &str, user: &str) -> Result<(String, Charged), JudgeError> {
        let p = &self.provider;
        let body = if p.is_local() {
            serde_json::json!({
                "model": p.model,
                "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
                "format": "json",
                "stream": false,
                "options": {"temperature": 0, "num_ctx": p.window, "num_predict": p.answer_tokens},
            })
        } else {
            serde_json::json!({
                "model": p.model,
                "temperature": 0,
                "response_format": {"type": "json_object"},
                "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
            })
        };
        let value = self.post(&body)?;
        let (content, charged) = if p.is_local() {
            (
                value["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                Charged {
                    prompt_tokens: value["prompt_eval_count"].as_u64().unwrap_or(0),
                    completion_tokens: value["eval_count"].as_u64().unwrap_or(0),
                },
            )
        } else {
            (
                value["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                Charged {
                    prompt_tokens: value["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                    completion_tokens: value["usage"]["completion_tokens"].as_u64().unwrap_or(0),
                },
            )
        };
        // A provider that read exactly its window truncated, and what it drops is the head — the system
        // prompt and the units. Measured: it says so only in its own log, so the tool says it here.
        if charged.prompt_tokens >= u64::from(p.window) {
            return Err(JudgeError::Truncated {
                charged: charged.prompt_tokens,
                window: p.window,
            });
        }
        Ok((content, charged))
    }
}

/// A small model answers in the shape it feels like: an object, a bare array, or something unusable.
fn parse_answer(content: &str) -> Result<Vec<RawVerdict>, JudgeError> {
    if let Ok(a) = serde_json::from_str::<Answer>(content) {
        if !a.verdicts.is_empty() {
            return Ok(a.verdicts);
        }
    }
    if let Ok(v) = serde_json::from_str::<Vec<RawVerdict>>(content) {
        return Ok(v);
    }
    match serde_json::from_str::<Answer>(content) {
        Ok(a) => Ok(a.verdicts),
        Err(e) => Err(JudgeError::Shape(e.to_string())),
    }
}

impl ProviderJudge {
    fn post(&self, body: &serde_json::Value) -> Result<serde_json::Value, JudgeError> {
        if self.provider.is_local() || self.provider.endpoint.starts_with("http://") {
            self.post_plain(body)
        } else {
            self.post_tls(body)
        }
    }

    /// Plain HTTP/1.1, written out rather than pulled in: a local provider needs no TLS, and a default
    /// build with no network stack is what the self-contained objective asks for.
    fn post_plain(&self, body: &serde_json::Value) -> Result<serde_json::Value, JudgeError> {
        let url = &self.provider.endpoint;
        let rest = url.strip_prefix("http://").ok_or_else(|| {
            JudgeError::Transport(format!("{url}: only http:// is supported without TLS"))
        })?;
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let host = authority.to_string();
        let payload = serde_json::to_vec(body).map_err(|e| JudgeError::Transport(e.to_string()))?;
        let mut stream = TcpStream::connect(&host)
            .map_err(|e| JudgeError::Transport(format!("connecting to {host}: {e}")))?;
        stream
            .set_read_timeout(Some(self.provider.timeout))
            .and_then(|()| stream.set_write_timeout(Some(self.provider.timeout)))
            .map_err(|e| JudgeError::Transport(e.to_string()))?;
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.write_all(&payload))
            .and_then(|()| stream.flush())
            .map_err(|e| JudgeError::Transport(e.to_string()))?;

        let mut reader = BufReader::new(stream);
        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .map_err(|e| JudgeError::Transport(e.to_string()))?;
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| JudgeError::Transport(format!("no status in {status_line:?}")))?;
        let mut chunked = false;
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|e| JudgeError::Transport(e.to_string()))?;
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if line.to_ascii_lowercase().starts_with("transfer-encoding:")
                && line.to_ascii_lowercase().contains("chunked")
            {
                chunked = true;
            }
        }
        let mut raw = Vec::new();
        reader
            .read_to_end(&mut raw)
            .map_err(|e| JudgeError::Transport(e.to_string()))?;
        let body_bytes = if chunked { dechunk(&raw)? } else { raw };
        let text = String::from_utf8_lossy(&body_bytes).into_owned();
        if !(200..300).contains(&status) {
            return Err(JudgeError::Status {
                provider: self.provider.kind.clone(),
                status,
                body: text.chars().take(400).collect(),
            });
        }
        serde_json::from_str(&text).map_err(|e| JudgeError::Transport(e.to_string()))
    }

    #[cfg(feature = "hosted")]
    fn post_tls(&self, body: &serde_json::Value) -> Result<serde_json::Value, JudgeError> {
        let key = std::env::var(&self.provider.key_var)
            .map_err(|_| JudgeError::NoKey(self.provider.key_var.clone()))?;
        let agent = ureq::AgentBuilder::new()
            .timeout(self.provider.timeout)
            .build();
        match agent
            .post(&self.provider.endpoint)
            .set("Authorization", &format!("Bearer {key}"))
            .send_json(body.clone())
        {
            Ok(r) => r
                .into_json()
                .map_err(|e| JudgeError::Transport(e.to_string())),
            Err(ureq::Error::Status(status, r)) => Err(JudgeError::Status {
                provider: self.provider.kind.clone(),
                status,
                body: r
                    .into_string()
                    .unwrap_or_default()
                    .chars()
                    .take(400)
                    .collect(),
            }),
            Err(e) => Err(JudgeError::Transport(e.to_string())),
        }
    }

    #[cfg(not(feature = "hosted"))]
    fn post_tls(&self, _body: &serde_json::Value) -> Result<serde_json::Value, JudgeError> {
        Err(JudgeError::NotCompiled(self.provider.endpoint.clone()))
    }
}

fn dechunk(raw: &[u8]) -> Result<Vec<u8>, JudgeError> {
    let mut out = Vec::new();
    let mut rest = raw;
    loop {
        let nl = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| JudgeError::Transport("chunked body ends mid-header".into()))?;
        let size = usize::from_str_radix(std::str::from_utf8(&rest[..nl]).unwrap_or("").trim(), 16)
            .map_err(|e| JudgeError::Transport(format!("chunk size: {e}")))?;
        rest = &rest[nl + 2..];
        if size == 0 {
            return Ok(out);
        }
        if rest.len() < size {
            return Err(JudgeError::Transport("chunked body is short".into()));
        }
        out.extend_from_slice(&rest[..size]);
        rest = &rest[size.min(rest.len())..];
        rest = rest.strip_prefix(b"\r\n".as_slice()).unwrap_or(rest);
    }
}
