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
    /// When a change does not fit, show the files the corpus has reasoning about rather than whichever
    /// sort first. Costs nothing: the same number of lines is shown either way.
    pub order_by_corpus: bool,
    /// Calls to have in flight at once. The calls are independent — a chunk of units against a part —
    /// so waiting for them one at a time is waiting for nothing. Measured: a call takes about a minute
    /// on a local 14B, and a check makes four of them. A local provider serves them one at a time unless
    /// it is configured otherwise, so more jobs help a hosted provider most.
    pub jobs: usize,
    /// Parts a change too large for one view may be read in. `1` shows what fits and names the rest;
    /// more reads the rest too, at one set of calls per part. Measured on held-out data: 59% of changes
    /// are truncated and 307 files across 55 cases are never examined, which is what more parts buys —
    /// and what they cost is a multiple of every call.
    pub max_parts: usize,
    /// Rotates the judged units before they are grouped. Two passes with different seeds see the same
    /// units in different company, and a finding only one pass makes is an artefact of that company.
    pub order_seed: usize,
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
            jobs: 1,
            order_by_corpus: true,
            // One part: the same work as before. Reading a change in parts is opt-in because it
            // multiplies what a check costs, and `check` is advisory whatever it reads.
            max_parts: 1,
            order_seed: 0,
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
#[derive(Clone)]
pub struct Change {
    pub files: Vec<String>,
    changed: Vec<String>,
    shown: String,
    lines: BTreeSet<String>,
    numbered: BTreeMap<u32, String>,
    pub truncated: bool,
    /// The change as it arrived, so it can be shown in a different order without being re-read.
    raw: String,
    /// Files of which the model saw at least one line. The rest were not examined, and a report that
    /// does not say so lets "nothing is contradicted" stand for "nothing in what I read".
    seen: BTreeSet<String>,
}

impl Change {
    /// The same change with its files in a different order, so that a cut keeps what matters.
    ///
    /// A diff is ordered by path, which has nothing to do with what the corpus knows. When only part of
    /// a change fits, showing the files the corpus has reasoning about beats showing whichever sort
    /// first — and it costs nothing, because the same number of lines is shown either way.
    pub fn ordered_by(&self, weight: &dyn Fn(&str) -> usize, s: &Settings) -> Change {
        let mut sections: Vec<(String, String)> = Vec::new();
        for part in self.raw.split("diff --git ") {
            if part.trim().is_empty() {
                continue;
            }
            let path = part
                .lines()
                .next()
                .and_then(|l| l.split(" b/").nth(1))
                .unwrap_or_default()
                .to_string();
            sections.push((path, format!("diff --git {part}")));
        }
        // Heaviest first, and the diff's own order among equals, so the result is stable.
        sections.sort_by_key(|(path, _)| std::cmp::Reverse(weight(path)));
        Change::from_diff(
            &sections
                .into_iter()
                .map(|(_, text)| text)
                .collect::<Vec<_>>()
                .join(""),
            s,
        )
    }

    /// The change as parts that each fit, whole files at a time, at most `max_parts` of them.
    ///
    /// A file too large for a part of its own is shown cut, as before — that is the one place this still
    /// truncates, and the part reports it like any other.
    pub fn parts(&self, s: &Settings) -> Vec<Change> {
        if !self.truncated || s.max_parts <= 1 {
            return vec![self.clone()];
        }
        let mut sections: Vec<String> = Vec::new();
        for part in self.raw.split("diff --git ") {
            if !part.trim().is_empty() {
                sections.push(format!("diff --git {part}"));
            }
        }
        let mut parts = Vec::new();
        let mut current = String::new();
        for section in sections {
            // A part is full when adding this file would truncate it, so each part is whole files.
            let would = format!("{current}{section}");
            if !current.is_empty() && Change::from_diff(&would, s).truncated {
                parts.push(Change::from_diff(&current, s));
                current = section;
                if parts.len() + 1 >= s.max_parts {
                    break;
                }
            } else {
                current = would;
            }
        }
        if !current.is_empty() {
            parts.push(Change::from_diff(&current, s));
        }
        parts.truncate(s.max_parts);
        parts
    }

    /// Files the model was shown nothing of.
    ///
    /// Measured on held-out data: 59% of changes were truncated, and 9 of the 16 that contain a
    /// contradiction. A reader told which files went unexamined can go and look; a reader not told
    /// cannot.
    pub fn unexamined(&self) -> Vec<String> {
        self.files
            .iter()
            .filter(|f| !self.seen.contains(*f))
            .cloned()
            .collect()
    }
}

impl Change {
    /// The diff as the model sees it, numbered and capped.
    pub fn shown(&self) -> &str {
        &self.shown
    }

    /// Read a unified diff, numbering the lines the model may quote and capping what it is shown.
    pub fn from_diff(text: &str, s: &Settings) -> Change {
        let (mut files, mut changed, mut shown) = (Vec::new(), Vec::new(), Vec::new());
        let (mut lines, mut numbered) = (BTreeSet::new(), BTreeMap::new());
        let (mut in_file, mut tokens, mut started, mut truncated) = (0usize, 0u32, false, false);
        let (mut seen, mut current) = (BTreeSet::new(), String::new());
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("diff --git ") {
                started = true;
                in_file = 0;
                if let Some(b) = rest.split(" b/").nth(1) {
                    files.push(b.to_string());
                    current = b.to_string();
                }
                // A file's header counts against the cap like any other line: past it, the change is
                // truncated, and saying "no more files" by silence would be a lie of omission.
                if shown.len() < s.diff_lines {
                    shown.push(format!("{:>4}| {line}", shown.len() as u32 + 1));
                } else {
                    truncated = true;
                }
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
            if !current.is_empty() {
                seen.insert(current.clone());
            }
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
            seen,
            raw: text.to_string(),
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
    /// Files of the change the model was shown nothing of (B, 0.2.0).
    pub unexamined: Vec<String>,
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
/// What `check` would show a model, without asking one: the packed units and the labels to be judged.
///
/// Everything up to the model call is deterministic, and a reader — or a second implementation being
/// compared against this one — can see all of it for nothing.
pub fn preview(store: &Store, change: &Change, s: &Settings) -> Option<(String, Vec<String>)> {
    context(store, change, s)
}

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
            // The edges between units that were both packed: a decision arriving with the prerequisite
            // it rests on says more than either alone, and this is where the reader sees the link.
            let selected: BTreeSet<Uid> = packed.selection.keys().copied().collect();
            let mut edges: BTreeMap<Uid, Vec<String>> = BTreeMap::new();
            for r in store.relations() {
                if selected.contains(&r.from) && selected.contains(&r.to) {
                    edges
                        .entry(r.from)
                        .or_default()
                        .push(format!("{} {}", r.kind, name(&r.to)));
                }
            }
            // By label, so a commit's decision stands beside its own prerequisites. Uid order, which is
            // what the pack comes back in, scatters them.
            let mut order: Vec<(&Uid, &Lod)> = packed.selection.iter().collect();
            order.sort_by_key(|(uid, _)| name(uid));
            for (uid, level) in order {
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
                for e in edges.get(uid).into_iter().flatten() {
                    block.push_str(&format!("\n  -> {e}"));
                }
                blocks.push(block);
            }
            if s.judge_limit > 0 {
                judged.truncate(s.judge_limit);
            }
            if s.order_seed > 0 && !judged.is_empty() {
                let by = s.order_seed % judged.len();
                judged.rotate_left(by);
            }
            return Some((blocks.join("\n\n"), judged));
        }
        ranked.pop();
    }
    None
}

/// Ask every question, up to `jobs` at a time, and return the answers in the order they were asked.
///
/// Order is kept because a report that changes between identical runs is a report nobody can compare.
fn ask_all(
    judge: &(dyn Judge + Sync),
    s: &Settings,
    asks: &[String],
) -> Vec<Result<(Vec<RawVerdict>, crate::Charged), JudgeError>> {
    let jobs = s.jobs.max(1).min(asks.len().max(1));
    if jobs == 1 || asks.len() == 1 {
        return asks
            .iter()
            .map(|a| judge.ask(s.system_prompt(), a))
            .collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let answers: Vec<std::sync::Mutex<Option<_>>> = (0..asks.len())
        .map(|_| std::sync::Mutex::new(None))
        .collect();
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(ask) = asks.get(i) else { break };
                let answer = judge.ask(s.system_prompt(), ask);
                *answers[i].lock().unwrap() = Some(answer);
            });
        }
    });
    answers
        .into_iter()
        .map(|slot| {
            slot.into_inner()
                .unwrap()
                .unwrap_or_else(|| Err(JudgeError::Shape("no answer".into())))
        })
        .collect()
}

/// A list a person can read: the first few names, then how many more.
fn shortened(files: &[String]) -> String {
    const SHOWN: usize = 6;
    if files.len() <= SHOWN {
        return files.join(", ");
    }
    format!(
        "{}, and {} more",
        files[..SHOWN].join(", "),
        files.len() - SHOWN
    )
}

/// The seeds two passes use, from the configuration S4 froze.
pub const AGREEMENT_SEEDS: [usize; 2] = [0, 5];

/// Run `check` more than once and keep only what every pass reported (D12's reason, applied to one
/// change): measured on held-out data, two passes turned 170 and 192 flags into 96 and removed every
/// false flag on a real commit in development. It costs recall — 7 of 13 became 5 of 13 — and that trade
/// is why the figures this tool quotes are the agreed ones.
///
/// One pass is `check` itself; this is what the shipped default runs.
pub fn check_agreed(
    store: &Store,
    change: &Change,
    judge: &(dyn Judge + Sync),
    s: &Settings,
    seeds: &[usize],
) -> Outcome {
    let mut passes: Vec<Outcome> = seeds
        .iter()
        .map(|seed| {
            check(
                store,
                change,
                judge,
                &Settings {
                    order_seed: *seed,
                    ..s.clone()
                },
            )
        })
        .collect();
    let Some(mut out) = passes.pop() else {
        return check(store, change, judge, s);
    };
    for other in &passes {
        let kept: BTreeSet<&str> = other.findings.iter().map(|f| f.label.as_str()).collect();
        let before = out.findings.len();
        out.findings.retain(|f| kept.contains(f.label.as_str()));
        let dropped = before - out.findings.len();
        if dropped > 0 {
            out.dropped.push(format!(
                "{dropped} finding(s) only one pass reported, so they were not kept"
            ));
        }
        out.calls += other.calls;
        out.predicted_tokens += other.predicted_tokens;
        out.charged_tokens += other.charged_tokens;
        for w in &other.warnings {
            if !out.warnings.contains(w) {
                out.warnings.push(w.clone());
            }
        }
    }
    out
}

/// Run `check` over one change against one corpus.
/// Check a change against the corpus, reading it in parts when it does not fit and the settings allow.
///
/// Each part is judged against the units retrieved for *it*, so a file that never reached the model
/// before is now judged against the reasoning recorded about that file. Findings are unioned and a unit
/// reported by two parts is reported once.
pub fn check(store: &Store, change: &Change, judge: &(dyn Judge + Sync), s: &Settings) -> Outcome {
    let parts = change.parts(s);
    if parts.len() <= 1 {
        return check_one(store, change, judge, s);
    }
    let mut out = Outcome {
        judge: judge.describe(),
        ..Outcome::default()
    };
    out.warnings.push(format!(
        "the change did not fit and was read in {} parts, one set of calls each",
        parts.len()
    ));
    let mut seen: BTreeSet<(String, Option<u32>)> = BTreeSet::new();
    let mut unexamined: Vec<String> = Vec::new();
    for part in &parts {
        let mut result = check_one(store, part, judge, s);
        out.calls += result.calls;
        out.predicted_tokens += result.predicted_tokens;
        out.charged_tokens += result.charged_tokens;
        out.units_judged += result.units_judged;
        out.dropped.append(&mut result.dropped);
        for w in result.warnings {
            // The per-part truncation notice is the whole change's story, told once.
            if !out.warnings.contains(&w) {
                out.warnings.push(w);
            }
        }
        for finding in result.findings {
            if seen.insert((finding.label.clone(), finding.line)) {
                out.findings.push(finding);
            }
        }
        unexamined.extend(result.unexamined);
    }
    // A file is unexamined only if no part examined it.
    let examined: BTreeSet<String> = parts
        .iter()
        .flat_map(|p| p.files.iter().cloned())
        .filter(|f| !parts.iter().all(|p| p.unexamined().contains(f)))
        .collect();
    out.unexamined = unexamined
        .into_iter()
        .filter(|f| !examined.contains(f))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    out
}

fn check_one(store: &Store, change: &Change, judge: &(dyn Judge + Sync), s: &Settings) -> Outcome {
    let mut out = Outcome {
        judge: judge.describe(),
        ..Outcome::default()
    };
    if change.is_empty() {
        out.warnings.push("the change is empty".into());
        return out;
    }
    // A change that did not fit is shown in the order the corpus cares about (A, 0.2.0). Measured: 59%
    // of held-out changes were truncated, and a diff's own order has nothing to do with what is recorded.
    let reordered;
    let change = if change.truncated && s.order_by_corpus {
        let weight = |path: &str| store.units_with_source_prefix(&format!("{path}@")).len();
        reordered = change.ordered_by(&weight, s);
        &reordered
    } else {
        change
    };
    if change.truncated {
        out.unexamined = change.unexamined();
        let mut warning =
            "the change is larger than this model's window and was truncated".to_string();
        if !out.unexamined.is_empty() {
            // Naming them is the point: "nothing is contradicted" must not stand for "nothing in the
            // part I read".
            warning.push_str(&format!(
                "; {} file(s) were not examined at all: {}",
                out.unexamined.len(),
                shortened(&out.unexamined)
            ));
        }
        out.warnings.push(warning);
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
    // What to ask, before anything is asked: the work is decided here and then carried out, which is
    // what lets it be carried out in any order.
    let asks: Vec<String> = groups
        .iter()
        .map(|group| {
            let block: String = group
                .iter()
                .filter_map(|l| units.get(l).cloned())
                .collect::<Vec<_>>()
                .join("\n\n");
            format!(
                "RECORDED UNITS:\n\n{block}\n\nJUDGE: {}\n\nCHANGE{}:\n{}\n",
                group.join(", "),
                if change.truncated { " (truncated)" } else { "" },
                change.shown
            )
        })
        .collect();
    out.calls += asks.len();
    for ask in &asks {
        out.predicted_tokens += u64::from(s.tokens(ask) + s.tokens(s.system_prompt()));
    }

    let answers = ask_all(judge, s, &asks);
    let mut verdicts: Vec<RawVerdict> = Vec::new();
    for answer in answers {
        match answer {
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
