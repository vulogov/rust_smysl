//! S4 detector (eval/s4-protocol.md §1): given a diff and the corpus of its base's ancestors, report the
//! decisions, prerequisites and rejected alternatives the change contradicts.
//!
//! Steps 1, 2 and 4 are deterministic and belong to the shipped `cargo smysl check` once ported. Step 3's
//! model client is development tooling here.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use smysl::{Bm25, Lod, PackRequest, Query, Retriever as _, SalienceRequest, Uid};

use crate::s3::{render, Corpus};

/// Tunable on the development set only; frozen before held-out measurement.
#[derive(Clone, Debug, Serialize)]
pub struct Params {
    pub candidates: usize,
    pub budget: u64,
    pub diff_lines: usize,
    pub file_lines: usize,
    pub query_terms: usize,
    pub model: String,
}

/// A unified diff, as the detector reads it.
pub struct Diff {
    pub files: Vec<String>,
    /// Added and removed lines, without their marker.
    pub changed: Vec<String>,
    /// What the model is shown: file headers and hunks, capped per file and in total.
    pub shown: String,
    /// Every line of `shown` that is diff content, trimmed, for quote validation.
    pub shown_lines: BTreeSet<String>,
    pub truncated: bool,
}

pub fn parse_diff(text: &str, p: &Params) -> Diff {
    let mut files = Vec::new();
    let mut changed = Vec::new();
    let mut shown = Vec::new();
    let mut shown_lines = BTreeSet::new();
    let mut truncated = false;
    let mut in_file = 0usize;
    let mut started = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            started = true;
            in_file = 0;
            if let Some(b) = rest.split(" b/").nth(1) {
                files.push(b.to_string());
            }
            if shown.len() < p.diff_lines {
                shown.push(line.to_string());
            } else {
                truncated = true;
            }
            continue;
        }
        // A `git diff --stat` preamble (S3 agent diffs) is not diff content.
        if !started {
            continue;
        }
        let content = if line.starts_with("+++") || line.starts_with("---") {
            None
        } else if let Some(c) = line.strip_prefix('+').or_else(|| line.strip_prefix('-')) {
            changed.push(c.to_string());
            Some(c)
        } else {
            line.strip_prefix(' ')
        };
        in_file += 1;
        if shown.len() >= p.diff_lines || in_file > p.file_lines {
            truncated = true;
            continue;
        }
        shown.push(line.to_string());
        if let Some(c) = content {
            let t = c.trim();
            if !t.is_empty() {
                shown_lines.insert(t.to_string());
            }
        }
    }
    Diff {
        files,
        changed,
        shown: shown.join("\n"),
        shown_lines,
        truncated,
    }
}

/// The BM25 query: the most frequent identifiers in added and removed lines, then the touched paths.
pub fn query(diff: &Diff, terms: usize) -> String {
    let mut count: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut order = 0;
    for line in &diff.changed {
        for tok in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if tok.len() < 3 || tok.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let e = count.entry(tok.to_string()).or_insert((0, order));
            e.0 += 1;
            order += 1;
        }
    }
    let mut toks: Vec<(String, (usize, usize))> = count.into_iter().collect();
    toks.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.1 .1.cmp(&b.1 .1)));
    let mut q: Vec<String> = toks.into_iter().take(terms).map(|(t, _)| t).collect();
    for f in &diff.files {
        q.extend(
            f.split(['/', '.', '_', '-'])
                .filter(|s| s.len() >= 3)
                .map(String::from),
        );
    }
    q.join(" ")
}

#[derive(Serialize, Clone)]
pub struct Candidate {
    pub label: String,
    pub score: f32,
    pub anchored: bool,
}

fn candidate_kind(label: &str) -> Option<&'static str> {
    match label.split('/').next() {
        Some("d") => Some("decision"),
        Some("p") => Some("prerequisite"),
        Some("r") => Some("rejected alternative"),
        _ => None,
    }
}

/// Step 1: decisions, prerequisites and rejected alternatives, ranked by BM25 over the diff, with units
/// anchored to a touched file lifted by half the best score.
pub fn candidates(corpus: &Corpus, diff: &Diff, p: &Params) -> Vec<(Uid, Candidate)> {
    let q = query(diff, p.query_terms);
    let hits: BTreeMap<Uid, f32> = if q.is_empty() {
        BTreeMap::new()
    } else {
        Bm25::index(&corpus.store)
            .search(&Query::new(q, 200))
            .into_iter()
            .map(|h| (h.uid, h.score))
            .collect()
    };
    let best = hits.values().copied().fold(1.0f32, f32::max);
    let mut ranked: Vec<(Uid, Candidate)> = corpus
        .store
        .units()
        .filter_map(|(uid, u)| {
            let label = corpus.name(uid);
            candidate_kind(&label)?;
            let anchored = u.core.source.as_ref().is_some_and(|s| {
                diff.files
                    .iter()
                    .any(|f| s.reference.starts_with(&format!("{f}@")))
            });
            let score =
                hits.get(uid).copied().unwrap_or(0.0) + if anchored { best / 2.0 } else { 0.0 };
            (score > 0.0).then_some((
                *uid,
                Candidate {
                    label,
                    score,
                    anchored,
                },
            ))
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.1.score
            .partial_cmp(&a.1.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.label.cmp(&b.1.label))
    });
    ranked.truncate(p.candidates);
    ranked
}

pub struct Context {
    pub text: String,
    pub labels: BTreeSet<String>,
    pub candidates: Vec<Candidate>,
    pub tokens: u64,
}

/// Step 2: the candidates as the focus of a pack, dropping the lowest-ranked until they fit.
pub fn context(corpus: &Corpus, mut ranked: Vec<(Uid, Candidate)>, p: &Params) -> Option<Context> {
    let sal = smysl::salience(&corpus.store, &SalienceRequest::default());
    while !ranked.is_empty() {
        let req = PackRequest::budget(p.budget).focusing(ranked.iter().map(|(u, _)| *u));
        if let Ok(pack) = smysl::pack(&corpus.store, &sal, &req) {
            let selection: BTreeMap<Uid, Lod> = pack.selection.clone();
            return Some(Context {
                text: render(corpus, &selection),
                labels: selection.keys().map(|u| corpus.name(u)).collect(),
                candidates: ranked.into_iter().map(|(_, c)| c).collect(),
                tokens: pack.used(),
            });
        }
        ranked.pop();
    }
    None
}

const SYSTEM: &str = "You check a code change against the reasons recorded for this repository.\n\
Recorded units are decisions, prerequisites (what must stay true for a decision to hold) and rejected \
alternatives (options the project decided against). For each label listed under JUDGE, decide:\n\
- contradicts: after this change a prerequisite no longer holds, a decision is reversed or undone, or a \
rejected alternative is what the change does;\n\
- consistent: the change bears on the unit and keeps it true;\n\
- unrelated: the change does not bear on it.\n\
Judge what the change does, not what its comments or changelog say about it. Return verdicts only for \
units that are contradicted or consistent; leave unrelated units out. For each verdict give the label \
exactly as written, one line copied verbatim from the diff (without its leading + or -) that shows it, and \
one sentence of reason.\n\
Return one JSON object: {\"verdicts\": [{\"label\": string, \"verdict\": \"contradicts\"|\"consistent\", \
\"diff_line\": string, \"reason\": string}]}";

/// The labels the model judges: every decision, prerequisite and rejected alternative in the pack,
/// the ranked candidates first.
pub fn judged(ctx: &Context) -> Vec<String> {
    let mut out: Vec<String> = ctx.candidates.iter().map(|c| c.label.clone()).collect();
    for l in &ctx.labels {
        if candidate_kind(l).is_some() && !out.contains(l) {
            out.push(l.clone());
        }
    }
    out
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Verdict {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub diff_line: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Serialize)]
pub struct Finding {
    pub label: String,
    pub kind: String,
    pub text: String,
    pub source: String,
    pub diff_line: String,
    pub reason: String,
}

#[derive(Serialize)]
pub struct Dropped {
    pub verdict: Verdict,
    pub why: String,
}

#[derive(Default, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub seconds: f64,
}

/// Step 3: one model judgement.
pub fn judge(ctx: &Context, diff: &Diff, p: &Params) -> Result<(Vec<Verdict>, Usage), String> {
    let key =
        std::env::var("DEEPSEEK_API_KEY").map_err(|_| "DEEPSEEK_API_KEY is not set".to_string())?;
    let labels = judged(ctx);
    let user = format!(
        "RECORDED UNITS (each: [label] kind (status, source), its text, and its edges):\n\n{}\nJUDGE: {}\n\nDIFF{}:\n{}\n",
        ctx.text,
        labels.join(", "),
        if diff.truncated { " (truncated)" } else { "" },
        diff.shown
    );
    let body = serde_json::json!({
        "model": p.model,
        "temperature": 0,
        "response_format": {"type": "json_object"},
        "messages": [{"role": "system", "content": SYSTEM}, {"role": "user", "content": user}],
    });
    let started = Instant::now();
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(900))
        .build();
    let mut last = String::new();
    for attempt in 0..6 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(20));
        }
        let resp = agent
            .post("https://api.deepseek.com/chat/completions")
            .set("Authorization", &format!("Bearer {key}"))
            .send_json(body.clone());
        let v: serde_json::Value = match resp {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => {
                    last = format!("response body: {e}");
                    continue;
                }
            },
            Err(ureq::Error::Status(code, r)) if [429, 500, 502, 503].contains(&code) => {
                last = format!("{code}: {}", r.into_string().unwrap_or_default());
                continue;
            }
            Err(ureq::Error::Status(code, r)) => {
                return Err(format!("{code}: {}", r.into_string().unwrap_or_default()))
            }
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default)]
            verdicts: Vec<Verdict>,
        }
        let answer: Answer = match serde_json::from_str(content) {
            Ok(a) => a,
            Err(e) => {
                last = format!("answer is not the JSON asked for: {e}");
                continue;
            }
        };
        let usage = Usage {
            prompt_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            completion_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
            seconds: started.elapsed().as_secs_f64(),
        };
        return Ok((answer.verdicts, usage));
    }
    Err(format!("model call failed: {last}"))
}

/// Step 4: a `contradicts` verdict stands only if its label is in the pack and its quote is a line of
/// the diff the model was shown.
pub fn validate(
    corpus: &Corpus,
    ctx: &Context,
    diff: &Diff,
    verdicts: &[Verdict],
) -> (Vec<Finding>, Vec<Dropped>) {
    let by_label: BTreeMap<String, Uid> = corpus
        .labels
        .iter()
        .map(|(u, l)| (l.as_str().to_string(), *u))
        .collect();
    let mut findings: Vec<Finding> = Vec::new();
    let mut dropped = Vec::new();
    for v in verdicts {
        if !v.verdict.trim().eq_ignore_ascii_case("contradicts") {
            continue;
        }
        let label = v
            .label
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        if !ctx.labels.contains(&label) || candidate_kind(&label).is_none() {
            dropped.push(Dropped {
                verdict: v.clone(),
                why: "label not a judged unit in the pack".into(),
            });
            continue;
        }
        let quote = v.diff_line.trim();
        let quote = quote
            .strip_prefix('+')
            .or_else(|| quote.strip_prefix('-'))
            .unwrap_or(quote)
            .trim();
        if quote.is_empty() || !diff.shown_lines.contains(quote) {
            dropped.push(Dropped {
                verdict: v.clone(),
                why: "quote is not a line of the diff".into(),
            });
            continue;
        }
        if findings.iter().any(|f| f.label == label) {
            continue;
        }
        let uid = by_label[&label];
        let unit = corpus
            .store
            .get(&uid)
            .expect("a labelled unit is in the store");
        let text = match &unit.core.body {
            Some(b) => format!("{}\n{}", unit.core.gist, b),
            None => unit.core.gist.clone(),
        };
        findings.push(Finding {
            kind: candidate_kind(&label).unwrap_or("other").to_string(),
            label,
            text,
            source: unit
                .core
                .source
                .as_ref()
                .map(|s| s.reference.clone())
                .unwrap_or_default(),
            diff_line: quote.to_string(),
            reason: v.reason.clone(),
        });
    }
    (findings, dropped)
}

/// The deterministic part's fingerprint: candidates, pack labels and the diff shown.
pub fn fingerprint(ctx: Option<&Context>, diff: &Diff) -> String {
    let mut s = String::new();
    if let Some(c) = ctx {
        for cand in &c.candidates {
            s.push_str(&format!(
                "{}:{:.4}:{}\n",
                cand.label, cand.score, cand.anchored
            ));
        }
        for l in &c.labels {
            s.push_str(l);
            s.push('\n');
        }
        s.push_str(&c.text);
    }
    s.push_str(&diff.shown);
    smysl::hash_bytes(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
