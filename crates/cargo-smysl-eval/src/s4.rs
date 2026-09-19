//! S4 detector (eval/s4-protocol.md §1): given a diff and the corpus of its base's ancestors, report the
//! decisions, prerequisites and rejected alternatives the change contradicts.
//!
//! Steps 1, 2 and 4 are deterministic and belong to the shipped `cargo smysl check` once ported. Step 3's
//! model client is development tooling here.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use cargo_smysl_corpus::KIND_KEY;
use serde::{Deserialize, Serialize};
use smysl::{
    Bm25, EdgeSet, Lod, PackRequest, Query, Retriever as _, SalienceRequest, Tokenizer, Uid,
};

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
    /// `ollama` (its own chat API) or `openai` (any OpenAI-compatible endpoint). Never a fixed
    /// vendor: the shipped command takes the same three settings from flags, the environment or
    /// configuration, so an operator chooses provider, endpoint and model.
    pub provider: String,
    pub endpoint: String,
    /// Environment variable holding the key, for a provider that needs one.
    pub key_var: String,
    /// Context window, where the provider must be asked for one (Ollama).
    pub num_ctx: u32,
    /// Units judged per call, as an upper bound. A small local model loses the thread over a long list;
    /// 0 asks for one call and lets the context limit decide. Fitting the context can split further.
    pub chunk: usize,
    /// Tokens the provider will take in one request, prompt and answer together. The judgement is fitted
    /// to it, and a split is reported (D17).
    pub context_limit: u32,
    /// Tokens left for the answer when fitting a call to `context_limit`.
    pub reserve_output: u32,
    /// Smallest contribution a matched term must make for a retrieved unit to count as retrieved on
    /// merit (smysl 1.6, R22). A unit whose only matches are near-zero-IDF words — `with`, `two`, `the`
    /// — is about the same prose, not the same code. 0 keeps every hit.
    pub min_term_weight: f32,
    /// Most units judged for one diff, ranked candidates first; 0 judges every decision, prerequisite
    /// and rejected alternative in the pack. A slower model needs a smaller number to stay usable.
    pub judge_limit: usize,
    /// Characters per token for this provider's tokenizer. A run reports the error against what the
    /// provider charged, so an operator can correct it for their model.
    pub chars_per_token: f32,
    /// Rotates the order units are grouped in. Two runs at different seeds group differently, so a
    /// verdict that survives both is not an artefact of one grouping.
    pub order_seed: usize,
    /// The system prompt. `DEFAULT_SYSTEM` unless the operator supplies one: models differ in what they
    /// need told, so the shipped command reads it from a flag, the environment or a file, and says which
    /// it used.
    pub system: String,
    /// Where `system` came from, for the record.
    pub system_source: String,
}

impl Params {
    pub fn is_ollama(&self) -> bool {
        self.provider.eq_ignore_ascii_case("ollama")
    }

    /// Tokens this provider's model will charge for `text`.
    ///
    /// Tokenizers differ — code and diffs run about 2.5 characters per token on Qwen, prose about 4 —
    /// so this is a setting, not a constant: the operator names their model's rate and every run records
    /// what the provider actually charged beside what was predicted (`Usage::predicted_tokens`), which is
    /// how a wrong rate shows up. smysl's own `bytes / 4` estimate is right for prose and wrong for a diff:
    /// measured against Qwen2.5-Coder, a prompt it called 9k was 31k, and Ollama silently kept the last
    /// `num_ctx` tokens, dropping the system prompt and the units to judge.
    pub fn model_tokens(&self, text: &str) -> u32 {
        (text.len() as f32 / self.chars_per_token).ceil() as u32
    }
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
    /// The same lines by the number the model is shown, so an answer can name one instead of
    /// copying it: a small model paraphrases a quote and loses the check otherwise.
    pub numbered: BTreeMap<u32, String>,
    pub truncated: bool,
}

pub fn parse_diff(text: &str, p: &Params) -> Diff {
    let mut files = Vec::new();
    let mut changed = Vec::new();
    let mut shown = Vec::new();
    let mut shown_lines = BTreeSet::new();
    let mut numbered = BTreeMap::new();
    let mut shown_tokens = 0u32;
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
                shown.push(format!("{:>4}| {line}", shown.len() as u32 + 1));
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
        // Half the window, at most, goes to the diff: the units to judge and the answer need the rest,
        // and a model that truncates drops the units first.
        if shown_tokens + p.model_tokens(line) > p.context_limit / 2 {
            truncated = true;
            continue;
        }
        shown_tokens += p.model_tokens(line) + 1;
        let n = shown.len() as u32 + 1;
        shown.push(format!("{n:>4}| {line}"));
        if let Some(c) = content {
            let t = c.trim();
            if !t.is_empty() {
                shown_lines.insert(t.to_string());
                numbered.insert(n, t.to_string());
            }
        }
    }
    Diff {
        files,
        changed,
        shown: shown.join("\n"),
        shown_lines,
        numbered,
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
    /// The query terms this unit matched, strongest first (smysl 1.6, R22).
    pub terms: Vec<String>,
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
    // R20 (smysl 1.5): the store's index answers which units are anchored to a file this diff touches.
    let anchored: BTreeSet<Uid> = diff
        .files
        .iter()
        .flat_map(|f| corpus.store.units_with_source_prefix(&format!("{f}@")))
        .collect();
    let hits: BTreeMap<Uid, (f32, Vec<(String, f32)>)> = if q.is_empty() {
        BTreeMap::new()
    } else {
        // R19 (1.5): folding, so a diff saying `required` reaches a unit saying `require`. Measured:
        // task 5's prerequisite was unreachable at any budget without it.
        Bm25::index_with(&corpus.store, Tokenizer::folding())
            // R23 (1.6): the schema's own kind, indexed once with the store, rather than the
            // eligible set rebuilt per diff. `kinds` cannot express it: a rejected alternative and a
            // consequence are both `Claim`.
            .search(
                &Query::new(q, p.candidates)
                    // The values the corpus actually writes (cargo-smysl-corpus build.rs): a decision's
                    // own kind, a prerequisite's kind, and a rejected alternative.
                    .with_payload(
                        KIND_KEY,
                        [
                            "act",
                            "decline",
                            "existing-behaviour",
                            "invariant",
                            "tool-setting",
                            "prior-change",
                            "assumption",
                            "rejected-alternative",
                        ],
                    ),
            )
            .into_iter()
            .map(|h| (h.uid, (h.score, h.terms.clone())))
            .collect()
    };
    let best = hits.values().map(|(s, _)| *s).fold(1.0f32, f32::max);
    let mut ranked: Vec<(Uid, Candidate)> = corpus
        .store
        .units()
        .filter_map(|(uid, _)| {
            let label = corpus.name(uid);
            candidate_kind(&label)?;
            let is_anchored = anchored.contains(uid);
            let hit = hits.get(uid).filter(|(_, terms)| {
                p.min_term_weight <= 0.0 || terms.iter().any(|(_, c)| *c >= p.min_term_weight)
            });
            let score =
                hit.map(|(s, _)| *s).unwrap_or(0.0) + if is_anchored { best / 2.0 } else { 0.0 };
            // R22 (1.6): why this unit was retrieved, so a miss is visible without reading packs.
            let terms: Vec<String> = hit
                .map(|(_, t)| {
                    t.iter()
                        .take(5)
                        .map(|(w, c)| format!("{w}:{c:.2}"))
                        .collect()
                })
                .unwrap_or_default();
            (score > 0.0).then_some((
                *uid,
                Candidate {
                    label,
                    score,
                    anchored: is_anchored,
                    terms,
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
pub fn context(
    corpus: &Corpus,
    mut ranked: Vec<(Uid, Candidate)>,
    p: &Params,
    reserve: u64,
) -> Option<Context> {
    let sal = smysl::salience(&corpus.store, &SalienceRequest::default());
    while !ranked.is_empty() {
        // C8 (1.5): a packed decision carries the prerequisites it rests on, which D3's `conditions`
        // edges keep outside its uid. Measured: decisions arriving with their prerequisites, 21% to 54%.
        // R21 (1.6): the budget is the model's, less what the rest of the prompt occupies, so packing
        // and fitting are one decision instead of the caller dropping units the solver chose whole.
        // The budget is what the model takes; `reserving` keeps room for the prompt, diff and answer,
        // and the pack itself is still capped at `budget` so runs stay comparable.
        // Both numbers are model tokens; the solver counts in smysl's, so convert (R24).
        let ceiling = u64::from(p.context_limit).max(reserve + 512);
        let req = PackRequest::budget(smysl_budget(
            (reserve + p.budget).min(ceiling),
            p.chars_per_token,
        ))
        .reserving(smysl_budget(reserve, p.chars_per_token))
        .focusing(ranked.iter().map(|(u, _)| *u))
        .resting_on(EdgeSet::premises());
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

/// The default, tuned against the models this project measured. An operator may replace it.
pub const DEFAULT_SYSTEM: &str = "You check a code change against the reasons recorded for this repository.\n\
Recorded units are decisions, prerequisites (what must stay true for a decision to hold) and rejected \
alternatives (options the project decided against).\n\
Report ONLY the units this change contradicts: after the change a prerequisite no longer holds, a \
decision is reversed or undone, or a rejected alternative is what the change does. A unit the change \
does not bear on, or bears on and keeps true, is left out. If the change contradicts none of them, \
return {\"verdicts\": []} — that is the common answer.\n\
Judge what the change does, not what its comments or changelog say about it. Two kinds of unit describe \
one past commit rather than a lasting rule, and later work moving on from them is never a contradiction: \
a statement of the repository\'s state at that time (a version number, a changelog section, a count), and \
a decision about that commit\'s own scope (leaving other sites unchanged, deferring something).\n\
Every diff line is printed with a number. For each unit you report, give its label exactly as written, \
the number of the line that shows the contradiction, that line copied, and one sentence of reason.\n\
Return one JSON object: {\"verdicts\": [{\"label\": string, \"verdict\": \"contradicts\", \"line\": number, \
\"diff_line\": string, \"reason\": string}]}";

/// The labels the model judges: every decision, prerequisite and rejected alternative in the pack,
/// the ranked candidates first.
pub fn judged(ctx: &Context, order_seed: usize, limit: usize) -> Vec<String> {
    let mut out: Vec<String> = ctx.candidates.iter().map(|c| c.label.clone()).collect();
    for l in &ctx.labels {
        if candidate_kind(l).is_some() && !out.contains(l) {
            out.push(l.clone());
        }
    }
    if limit > 0 && out.len() > limit {
        out.truncate(limit);
    }
    if order_seed > 0 && !out.is_empty() {
        let by = order_seed % out.len();
        out.rotate_left(by);
    }
    out
}

/// smysl counts `bytes / 4`; a model charges `bytes / chars_per_token`. A budget in model tokens is this
/// many smysl tokens (1.6's `counting_with` would do this properly, but its facade does not export
/// `ExternalCost`; docs/smysl-requests-1.7.md, R24).
pub fn smysl_budget(model_tokens: u64, chars_per_token: f32) -> u64 {
    (model_tokens as f64 * f64::from(chars_per_token) / 4.0).ceil() as u64
}

/// What had to be given up to fit the model's limits, reported with the result and on stderr (D17).
#[derive(Default, Serialize, Clone)]
pub struct Fitting {
    pub calls: usize,
    pub warnings: Vec<String>,
}

/// Group the units to judge so each call fits `context_limit` with `reserve_output` left for the answer.
/// A split is not free — a model judging six units sees less than one judging sixty — so it is reported.
pub fn fit<'a>(
    labels: &'a [String],
    units: &BTreeMap<String, String>,
    diff: &Diff,
    p: &Params,
) -> (Vec<Vec<&'a String>>, Fitting) {
    let fixed = p.model_tokens(&p.system) + p.model_tokens(&diff.shown) + 64;
    let room = p.context_limit.saturating_sub(p.reserve_output);
    let mut warn = Fitting::default();
    if fixed >= room {
        warn.warnings.push(format!(
            "the diff and the system prompt need about {fixed} tokens of the {room} this model leaves for input; \
             the diff is already truncated, so the answer may be poor"
        ));
    }
    let cap = if p.chunk == 0 {
        labels.len().max(1)
    } else {
        p.chunk
    };
    let mut groups: Vec<Vec<&String>> = Vec::new();
    let mut group: Vec<&String> = Vec::new();
    let mut used = fixed;
    for label in labels {
        let cost = units.get(label).map(|u| p.model_tokens(u)).unwrap_or(0) + 8;
        let full = group.len() >= cap || (!group.is_empty() && used + cost > room);
        if full {
            groups.push(std::mem::take(&mut group));
            used = fixed;
        }
        group.push(label);
        used += cost;
    }
    if !group.is_empty() {
        groups.push(group);
    }
    warn.calls = groups.len();
    if groups.len() > 1 {
        warn.warnings.push(format!(
            "{} recorded unit(s) judged in {} calls: this model takes about {} tokens, and each call sees only \
             its own units",
            labels.len(),
            groups.len(),
            p.context_limit
        ));
    }
    (groups, warn)
}

/// The units of `ctx.text`, keyed by label, so a chunk can carry only the ones it asks about.
fn blocks(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for block in text.split("\n\n") {
        if let Some(label) = block.strip_prefix('[').and_then(|b| b.split(']').next()) {
            out.insert(label.to_string(), block.to_string());
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
    /// The number printed with the diff line, which a model reproduces more reliably than the text.
    #[serde(default)]
    pub line: Option<u32>,
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
    /// What this run predicted the prompt would cost, at `chars_per_token`.
    pub predicted_tokens: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub seconds: f64,
}

/// Step 3: the model's judgement, fitted to what the provider takes (D17).
pub fn judge(
    ctx: &Context,
    diff: &Diff,
    p: &Params,
) -> Result<(Vec<Verdict>, Usage, Fitting), String> {
    let labels = judged(ctx, p.order_seed, p.judge_limit);
    let units = blocks(&ctx.text);
    let (groups, mut fitting) = fit(&labels, &units, diff, p);
    for w in &fitting.warnings {
        eprintln!("check: {w}");
    }
    let mut verdicts = Vec::new();
    let mut usage = Usage::default();
    let mut unusable = 0usize;
    for group in groups {
        let text: String = group
            .iter()
            .filter_map(|l| units.get(*l).cloned())
            .collect::<Vec<_>>()
            .join("\n\n");
        let user = format!(
            "RECORDED UNITS:\n\n{text}\n\nJUDGE: {}\n\nDIFF{}:\n{}\n",
            group
                .iter()
                .map(|l| l.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            if diff.truncated { " (truncated)" } else { "" },
            diff.shown
        );
        // An answer in a shape we cannot read is a call that found nothing, recorded as such: dropping
        // the case instead would quietly shrink the denominator of every rate computed from this run.
        let (mut v, u) = match call(&user, p) {
            Ok(x) => x,
            Err(e) if e.contains("not the JSON asked for") => {
                unusable += 1;
                eprintln!("check: unusable answer, counted as no finding: {e}");
                (Vec::new(), Usage::default())
            }
            Err(e) => return Err(e),
        };
        usage.predicted_tokens += u64::from(p.model_tokens(&user) + p.model_tokens(&p.system));
        verdicts.append(&mut v);
        usage.prompt_tokens += u.prompt_tokens;
        usage.completion_tokens += u.completion_tokens;
        usage.seconds += u.seconds;
    }
    if unusable > 0 {
        fitting
            .warnings
            .push(format!("{unusable} call(s) answered in an unreadable shape and found nothing"));
    }
    Ok((verdicts, usage, fitting))
}

fn call(user: &str, p: &Params) -> Result<(Vec<Verdict>, Usage), String> {
    let local = p.is_ollama();
    let body = if local {
        serde_json::json!({
            "model": p.model,
            "messages": [{"role": "system", "content": &p.system}, {"role": "user", "content": user}],
            "format": "json",
            "stream": false,
            // A local model in JSON mode can generate until something gives up: measured, four
            // requests in a row ending at the client's 30-minute timeout with nothing to show. The
            // answer this asks for is short, so cap it and fail fast instead.
            "options": {"temperature": 0, "num_ctx": p.num_ctx, "num_predict": p.reserve_output},
        })
    } else {
        serde_json::json!({
            "model": p.model,
            "temperature": 0,
            "response_format": {"type": "json_object"},
            "messages": [{"role": "system", "content": &p.system}, {"role": "user", "content": user}],
        })
    };
    let started = Instant::now();
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(if local { 420 } else { 1800 }))
        .build();
    let mut last = String::new();
    let attempts = if local { 3 } else { 6 };
    for attempt in 0..attempts {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(if local { 5 } else { 20 }));
        }
        let mut req = agent.post(&p.endpoint);
        if !local {
            let key = std::env::var(&p.key_var).map_err(|_| format!("{} is not set", p.key_var))?;
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        let v: serde_json::Value = match req.send_json(body.clone()) {
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
        let (content, usage) = if local {
            (
                v["message"]["content"].as_str().unwrap_or("").to_string(),
                Usage {
                    predicted_tokens: 0,
                    prompt_tokens: v["prompt_eval_count"].as_u64().unwrap_or(0),
                    completion_tokens: v["eval_count"].as_u64().unwrap_or(0),
                    seconds: started.elapsed().as_secs_f64(),
                },
            )
        } else {
            (
                v["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                Usage {
                    predicted_tokens: 0,
                    prompt_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                    completion_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
                    seconds: started.elapsed().as_secs_f64(),
                },
            )
        };
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default)]
            verdicts: Vec<Verdict>,
        }
        // A small model sometimes answers with the array alone, or wraps it in another key.
        let parsed: Result<Answer, _> = serde_json::from_str(&content);
        let verdicts = match parsed {
            Ok(a) if !a.verdicts.is_empty() => a.verdicts,
            _ => match serde_json::from_str::<Vec<Verdict>>(&content) {
                Ok(v) => v,
                Err(_) => match parsed {
                    Ok(a) => a.verdicts,
                    Err(e) => {
                        last = format!("answer is not the JSON asked for: {e}");
                        continue;
                    }
                },
            },
        };
        if local && usage.prompt_tokens >= u64::from(p.num_ctx) {
            // The provider read exactly its window: it truncated, and what it dropped is the head —
            // the system prompt and the units. A verdict from that is not a verdict on this input.
            return Err(format!(
                "the model truncated the prompt ({} tokens at a {} window): the units to judge were cut",
                usage.prompt_tokens, p.num_ctx
            ));
        }
        return Ok((verdicts, usage));
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
        let text = v.diff_line.trim();
        let text = text
            .strip_prefix('+')
            .or_else(|| text.strip_prefix('-'))
            .unwrap_or(text)
            .trim();
        // Either the line's number or its text identifies it; the number is what a small model gets right.
        let quote = match v.line.and_then(|n| diff.numbered.get(&n)) {
            Some(l) => l.clone(),
            None if !text.is_empty() && diff.shown_lines.contains(text) => text.to_string(),
            None => {
                dropped.push(Dropped {
                    verdict: v.clone(),
                    why: "neither the line number nor the quote is a line of the diff".into(),
                });
                continue;
            }
        };
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
            diff_line: quote,
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
