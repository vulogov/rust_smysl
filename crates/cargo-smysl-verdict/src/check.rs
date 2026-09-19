//! `cargo smysl check`: what a change contradicts in the recorded corpus.
//!
//! Five steps, four of them deterministic and the fourth guarding the one that is not (S4,
//! `eval/s4-protocol.md`):
//!
//! 1. **Candidates** — decisions, prerequisites and rejected alternatives anchored to the files the
//!    change touches, plus BM25 hits on the change's own identifiers.
//! 2. **Context** — those candidates packed with what they rest on, inside what the model will take.
//! 3. **Judgement** — one model call per group of units.
//! 4. **Validation** — a verdict stands only if its label is one that was judged and its quote is a line
//!    of the diff it was shown.
//! 5. **Report** — findings, and an exit code.
//!
//! Measured on held-out data with a local 14B: recall 0.38, 13% of ordinary commits drew a flag, and 10
//! of 96 flags were correct (precision 0.10). So `check` is advisory by default (`Outcome::exit_code`)
//! and says what it is: a prompt to look, not a claim that something is wrong. A stronger model reaches
//! 0.75 recall and ~0.89 precision on the same pipeline.

use std::collections::{BTreeMap, BTreeSet};

use cargo_smysl_corpus::KIND_KEY;
use serde::Serialize;
use smysl::{
    label_index, pack, salience, Bm25, EdgeSet, Lod, PackRequest, Query, Retriever as _,
    SalienceRequest, Store, Tokenizer, Uid,
};

use cargo_smysl_extract::{Judge, JudgeError, RawVerdict};

/// The kinds a change can contradict, as the corpus writes them (`code:kind`).
const JUDGED_KINDS: [&str; 8] = [
    "act",
    "decline",
    "existing-behaviour",
    "invariant",
    "tool-setting",
    "prior-change",
    "assumption",
    "rejected-alternative",
];

/// Everything tunable, with the defaults S4 froze.
#[derive(Debug, Clone)]
pub struct Settings {
    pub candidates: usize,
    /// Model tokens of recorded units to carry.
    pub budget: u64,
    pub diff_lines: usize,
    pub file_lines: usize,
    pub query_terms: usize,
    /// Units asked about per call: a small model loses the thread over a long list.
    pub chunk: usize,
    /// Most units judged for one change; 0 judges every one in the pack.
    pub judge_limit: usize,
    /// Characters per token for the chosen model. A setting, not a constant: tokenizers differ, and a
    /// wrong value is what makes a provider truncate silently.
    pub chars_per_token: f32,
    pub window: u32,
    pub answer_tokens: u32,
    /// Replaces the built-in judgement prompt.
    pub system: Option<String>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            candidates: 12,
            budget: 3000,
            diff_lines: 400,
            file_lines: 150,
            query_terms: 80,
            chunk: 6,
            judge_limit: 24,
            chars_per_token: 2.0,
            window: 32768,
            answer_tokens: 2048,
            system: None,
        }
    }
}

impl Settings {
    pub fn tokens(&self, text: &str) -> u32 {
        (text.len() as f32 / self.chars_per_token).ceil() as u32
    }

    /// smysl counts bytes/4; a budget in model tokens is this many of those.
    fn smysl_budget(&self, model_tokens: u64) -> u64 {
        (model_tokens as f64 * f64::from(self.chars_per_token) / 4.0).ceil() as u64
    }

    pub fn system_prompt(&self) -> &str {
        self.system.as_deref().unwrap_or(DEFAULT_SYSTEM)
    }
}

pub const DEFAULT_SYSTEM: &str = "You check a code change against the reasons recorded for this repository.\n\
Recorded units are decisions, prerequisites (what must stay true for a decision to hold) and rejected \
alternatives (options the project decided against).\n\
Report ONLY the units this change contradicts: after the change a prerequisite no longer holds, a \
decision is reversed or undone, or a rejected alternative is what the change does. A unit the change \
does not bear on, or bears on and keeps true, is left out. If the change contradicts none of them, \
return {\"verdicts\": []} — that is the common answer.\n\
Judge what the change does, not what its comments or changelog say about it. Two kinds of unit describe \
one past commit rather than a lasting rule, and later work moving on from them is never a contradiction: \
a statement of the repository's state at that time (a version number, a changelog section, a count), and \
a decision about that commit's own scope (leaving other sites unchanged, deferring something).\n\
Every diff line is printed with a number. For each unit you report, give its label exactly as written, \
the number of the line that shows the contradiction, that line copied, and one sentence of reason.\n\
Return one JSON object: {\"verdicts\": [{\"label\": string, \"verdict\": \"contradicts\", \"line\": number, \
\"diff_line\": string, \"reason\": string}]}";

/// A unified diff, as `check` reads it.
pub struct Change {
    pub files: Vec<String>,
    changed: Vec<String>,
    shown: String,
    lines: BTreeSet<String>,
    numbered: BTreeMap<u32, String>,
    pub truncated: bool,
}

impl Change {
    /// Read a unified diff, numbering the lines the model may quote and capping what it is shown.
    pub fn from_diff(text: &str, s: &Settings) -> Change {
        let (mut files, mut changed, mut shown) = (Vec::new(), Vec::new(), Vec::new());
        let (mut lines, mut numbered) = (BTreeSet::new(), BTreeMap::new());
        let (mut in_file, mut tokens, mut started, mut truncated) = (0usize, 0u32, false, false);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("diff --git ") {
                started = true;
                in_file = 0;
                if let Some(b) = rest.split(" b/").nth(1) {
                    files.push(b.to_string());
                }
                shown.push(format!("{:>4}| {line}", shown.len() as u32 + 1));
                continue;
            }
            if !started {
                continue; // a `--stat` preamble is not diff content
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
            // At most half the window goes to the change: the units and the answer need the rest, and a
            // provider that truncates drops the units first.
            if shown.len() >= s.diff_lines
                || in_file > s.file_lines
                || tokens + s.tokens(line) > s.window / 2
            {
                truncated = true;
                continue;
            }
            tokens += s.tokens(line) + 1;
            let n = shown.len() as u32 + 1;
            shown.push(format!("{n:>4}| {line}"));
            if let Some(c) = content {
                let t = c.trim();
                if !t.is_empty() {
                    lines.insert(t.to_string());
                    numbered.insert(n, t.to_string());
                }
            }
        }
        Change {
            files,
            changed,
            shown: shown.join("\n"),
            lines,
            numbered,
            truncated,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    /// The identifiers the change touches, most frequent first, then its paths.
    fn query(&self, terms: usize) -> String {
        let mut count: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut order = 0;
        for line in &self.changed {
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
        for f in &self.files {
            q.extend(
                f.split(['/', '.', '_', '-'])
                    .filter(|s| s.len() >= 3)
                    .map(String::from),
            );
        }
        q.join(" ")
    }
}

/// One recorded unit the change contradicts.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub label: String,
    pub kind: String,
    pub text: String,
    pub source: String,
    pub line: Option<u32>,
    pub diff_line: String,
    pub reason: String,
}

/// What `check` did, whatever it found.
#[derive(Debug, Default, Serialize)]
pub struct Outcome {
    pub findings: Vec<Finding>,
    /// Verdicts validation refused, and why.
    pub dropped: Vec<String>,
    /// What the caller should know about how the answer was produced (D17).
    pub warnings: Vec<String>,
    pub units_judged: usize,
    pub calls: usize,
    pub predicted_tokens: u64,
    pub charged_tokens: u64,
    pub judge: String,
}

impl Outcome {
    /// Advisory by default: findings are reported, not enforced. `--strict` is the gate, and 5 is
    /// smysl's code for "items await review".
    pub fn exit_code(&self, strict: bool) -> u8 {
        if self.findings.is_empty() || !strict {
            0
        } else {
            5
        }
    }
}

fn kind_of(label: &str) -> Option<&'static str> {
    match label.split('/').next() {
        Some("d") => Some("decision"),
        Some("p") => Some("prerequisite"),
        Some("r") => Some("rejected alternative"),
        _ => None,
    }
}

/// Step 1 and 2: the units worth asking about, packed with what they rest on.
fn context(store: &Store, change: &Change, s: &Settings) -> Option<(String, Vec<String>)> {
    let names = label_index(store);
    let name = |uid: &Uid| -> String {
        names
            .get(uid)
            .and_then(|l| l.first())
            .map(|l| l.as_str().to_string())
            .unwrap_or_else(|| uid.short())
    };
    let anchored: BTreeSet<Uid> = change
        .files
        .iter()
        .flat_map(|f| store.units_with_source_prefix(&format!("{f}@")))
        .collect();
    let query = change.query(s.query_terms);
    let hits: BTreeMap<Uid, f32> = if query.is_empty() {
        BTreeMap::new()
    } else {
        Bm25::index_with(store, Tokenizer::folding())
            .search(
                &Query::new(query, s.candidates)
                    .with_payload(KIND_KEY, JUDGED_KINDS.iter().copied()),
            )
            .into_iter()
            .map(|h| (h.uid, h.score))
            .collect()
    };
    let best = hits.values().copied().fold(1.0f32, f32::max);
    let mut ranked: Vec<(Uid, String, f32)> = store
        .units()
        .filter_map(|(uid, _)| {
            let label = name(uid);
            kind_of(&label)?;
            let score = hits.get(uid).copied().unwrap_or(0.0)
                + if anchored.contains(uid) {
                    best / 2.0
                } else {
                    0.0
                };
            (score > 0.0).then_some((*uid, label, score))
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    ranked.truncate(s.candidates);

    let reserve =
        u64::from(s.tokens(s.system_prompt()) + s.tokens(&change.shown) + s.answer_tokens);
    let sal = salience(store, &SalienceRequest::default());
    while !ranked.is_empty() {
        let ceiling = u64::from(s.window).max(reserve + 512);
        let req = PackRequest::budget(s.smysl_budget((reserve + s.budget).min(ceiling)))
            .reserving(s.smysl_budget(reserve))
            .focusing(ranked.iter().map(|(u, _, _)| *u))
            .resting_on(EdgeSet::premises());
        if let Ok(packed) = pack(store, &sal, &req) {
            let mut judged: Vec<String> = ranked.iter().map(|(_, l, _)| l.clone()).collect();
            let mut blocks: Vec<String> = Vec::new();
            for (uid, level) in &packed.selection {
                let Some(unit) = store.get(uid) else { continue };
                let label = name(uid);
                if kind_of(&label).is_some() && !judged.contains(&label) {
                    judged.push(label.clone());
                }
                let source = unit
                    .core
                    .source
                    .as_ref()
                    .map(|x| format!(", source {}", x.reference))
                    .unwrap_or_default();
                let mut block = format!(
                    "[{label}] {} ({}{})\n  {}",
                    unit.core.schema, unit.core.status, source, unit.core.gist
                );
                if *level >= Lod::L1 {
                    if let Some(b) = &unit.core.body {
                        block.push('\n');
                        block.push_str(&b.lines().map(|l| format!("  {l}\n")).collect::<String>());
                    }
                }
                blocks.push(block);
            }
            if s.judge_limit > 0 {
                judged.truncate(s.judge_limit);
            }
            return Some((blocks.join("\n\n"), judged));
        }
        ranked.pop();
    }
    None
}

/// Run `check` over one change against one corpus.
pub fn check(store: &Store, change: &Change, judge: &dyn Judge, s: &Settings) -> Outcome {
    let mut out = Outcome {
        judge: judge.describe(),
        ..Outcome::default()
    };
    if change.is_empty() {
        out.warnings.push("the change is empty".into());
        return out;
    }
    if change.truncated {
        out.warnings
            .push("the change is larger than this model's window and was truncated".into());
    }
    let Some((text, judged)) = context(store, change, s) else {
        out.warnings
            .push("nothing recorded bears on the files this change touches".into());
        return out;
    };
    out.units_judged = judged.len();
    let units: BTreeMap<String, String> = text
        .split("\n\n")
        .filter_map(|b| {
            let label = b.strip_prefix('[')?.split(']').next()?.to_string();
            Some((label, b.to_string()))
        })
        .collect();
    let size = if s.chunk == 0 {
        judged.len().max(1)
    } else {
        s.chunk
    };
    let groups: Vec<&[String]> = judged.chunks(size).collect();
    if groups.len() > 1 {
        out.warnings.push(format!(
            "{} recorded unit(s) judged in {} calls: this model takes about {} tokens, and each call \
             sees only its own units",
            judged.len(),
            groups.len(),
            s.window
        ));
    }
    let mut verdicts: Vec<RawVerdict> = Vec::new();
    for group in groups {
        let block: String = group
            .iter()
            .filter_map(|l| units.get(l).cloned())
            .collect::<Vec<_>>()
            .join("\n\n");
        let user = format!(
            "RECORDED UNITS:\n\n{block}\n\nJUDGE: {}\n\nCHANGE{}:\n{}\n",
            group.join(", "),
            if change.truncated { " (truncated)" } else { "" },
            change.shown
        );
        out.calls += 1;
        out.predicted_tokens += u64::from(s.tokens(&user) + s.tokens(s.system_prompt()));
        match judge.ask(s.system_prompt(), &user) {
            Ok((mut v, charged)) => {
                out.charged_tokens += charged.prompt_tokens;
                verdicts.append(&mut v);
            }
            // An unreadable answer is a call that found nothing, said out loud: dropping the change
            // would be a silent "no findings".
            Err(JudgeError::Shape(e)) => out.warnings.push(format!(
                "a call answered in an unreadable shape ({e}); it found nothing"
            )),
            Err(e) => {
                out.warnings.push(format!("a call failed: {e}"));
            }
        }
    }
    validate(store, change, &judged, &verdicts, &mut out);
    out
}

/// Step 4: a verdict stands only if its label was judged and its quote is a line of the change.
fn validate(
    store: &Store,
    change: &Change,
    judged: &[String],
    verdicts: &[RawVerdict],
    out: &mut Outcome,
) {
    let names = label_index(store);
    let by_label: BTreeMap<String, Uid> = names
        .iter()
        .flat_map(|(uid, labels)| labels.iter().map(move |l| (l.as_str().to_string(), *uid)))
        .collect();
    for v in verdicts {
        if !v.verdict.trim().eq_ignore_ascii_case("contradicts") {
            continue;
        }
        let label = v.label.trim().trim_matches(['[', ']']).to_string();
        if !judged.contains(&label) || kind_of(&label).is_none() {
            out.dropped
                .push(format!("{label}: not a unit this run judged"));
            continue;
        }
        let text = v.diff_line.trim();
        let text = text
            .strip_prefix('+')
            .or_else(|| text.strip_prefix('-'))
            .unwrap_or(text)
            .trim();
        let quote = match v.line.and_then(|n| change.numbered.get(&n)) {
            Some(l) => l.clone(),
            None if !text.is_empty() && change.lines.contains(text) => text.to_string(),
            None => {
                out.dropped.push(format!(
                    "{label}: neither the line number nor the quote is in the change"
                ));
                continue;
            }
        };
        if out.findings.iter().any(|f| f.label == label) {
            continue;
        }
        let Some(unit) = by_label.get(&label).and_then(|u| store.get(u)) else {
            continue;
        };
        out.findings.push(Finding {
            kind: kind_of(&label).unwrap_or("other").to_string(),
            label,
            text: match &unit.core.body {
                Some(b) => format!("{}\n{}", unit.core.gist, b),
                None => unit.core.gist.clone(),
            },
            source: unit
                .core
                .source
                .as_ref()
                .map(|s| s.reference.clone())
                .unwrap_or_default(),
            line: v.line,
            diff_line: quote,
            reason: v.reason.clone(),
        });
    }
}

/// A unified diff of one file, for a caller holding the two texts (`cargo-smysl-git`).
pub fn unified(path: &str, before: &str, after: &str, context_lines: usize) -> String {
    use imara_diff::{Algorithm, Diff, InternedInput};
    let input = InternedInput::new(before, after);
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let mut config = imara_diff::UnifiedDiffConfig::default();
    config.context_len(context_lines as u32);
    let body = diff
        .unified_diff(
            &imara_diff::BasicLineDiffPrinter(&input.interner),
            config,
            &input,
        )
        .to_string();
    if body.trim().is_empty() {
        return String::new();
    }
    format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n{body}")
}

/// The label of every judged unit, for a caller that wants to report what was considered.
pub fn judged_labels(store: &Store, change: &Change, s: &Settings) -> Vec<String> {
    context(store, change, s)
        .map(|(_, j)| j)
        .unwrap_or_default()
}
