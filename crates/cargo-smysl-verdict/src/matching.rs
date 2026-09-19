//! Checking one claim against the facts (D11).
//!
//! **One claim at a time.** The research measured this: asked about several claims at once a model
//! spreads one claim's evidence over its neighbours, and the round that scored best asked about a single
//! prerequisite with its own retrieved facts.
//!
//! **Structural facts and prose are shown apart** (D10). A doc comment says what someone believed; it is
//! shown because it helps a model find the right code, and it is labelled so that a claim covered only by
//! prose cannot come back `Supported` (the policy refuses it).
//!
//! Every citation is validated the way `check` validates a quote: a fact the model names must be one it
//! was shown. A verdict resting on an invented fact is worse than none.

use std::collections::BTreeSet;

use cargo_smysl_facts::{render, Fact};
use serde::Deserialize;

use crate::policy::Judgement;
use cargo_smysl_extract::{Judge, JudgeError};

/// How much to show, and what to call the run.
#[derive(Debug, Clone)]
pub struct Retrieval {
    /// Structural facts: what the code does.
    pub structural: usize,
    /// Author prose: what the code says about itself.
    pub prose: usize,
    /// Names the run, so two runs can be told apart when the policy counts agreement (D12).
    pub run: String,
    /// Lines of one fact the model is shown. A function's rendering lists every call it makes, and
    /// thirty of those is a prompt no local model survives: measured, a 14B runner died on it.
    pub lines_per_fact: usize,
    /// Characters the facts may occupy in total, whatever the counts allow.
    pub max_chars: usize,
}

impl Default for Retrieval {
    fn default() -> Retrieval {
        // 30 and 10 are the research's numbers: the round that scored 16 correct with 0 false
        // contradictions retrieved about this much per prerequisite.
        Retrieval {
            structural: 30,
            prose: 10,
            run: "run-1".into(),
            lines_per_fact: 12,
            max_chars: 24_000,
        }
    }
}

impl Retrieval {
    /// Sized to the model in front of us (D17): the facts may fill half the window, the rest belongs to
    /// the system prompt, the claim and the answer. A small window also shortens each fact, because
    /// twenty facts of one line each are worth more to a matcher than two facts in full.
    pub fn fitted(window_tokens: u32, chars_per_token: f32) -> Retrieval {
        let chars = (f64::from(window_tokens) * f64::from(chars_per_token) / 2.0) as usize;
        let mut r = Retrieval {
            max_chars: chars.max(2_000),
            ..Retrieval::default()
        };
        if chars < 12_000 {
            r.lines_per_fact = 6;
        }
        r
    }
}

/// What was shown to the model for one claim.
pub struct Shown<'a> {
    pub structural: Vec<&'a Fact>,
    pub prose: Vec<&'a Fact>,
    /// How each fact is cut down for the prompt.
    pub limits: (usize, usize),
}

impl Shown<'_> {
    /// How many of the retrieved facts actually reached the prompt.
    pub fn shown_count(&self) -> usize {
        self.names().len()
    }

    pub fn is_empty(&self) -> bool {
        self.structural.is_empty() && self.prose.is_empty()
    }

    /// The text the model reads: structural facts first, prose last and marked, each cut to
    /// `lines_per_fact` and the whole to `max_chars`. A fact that does not fit is left out rather than
    /// half shown, and its name is never offered, so a citation of it cannot validate.
    pub fn text(&self) -> String {
        let (lines, max_chars) = self.limits;
        let mut out = String::new();
        for (n, fact) in self.structural.iter().enumerate() {
            let block = format!("F{}: {}\n\n", n + 1, brief(fact, lines));
            if out.len() + block.len() > max_chars {
                break;
            }
            out.push_str(&block);
        }
        for (n, fact) in self.prose.iter().enumerate() {
            let block = format!("P{}: {}\n\n", n + 1, brief(fact, lines));
            if out.len() + block.len() > max_chars {
                break;
            }
            out.push_str(&block);
        }
        out
    }

    /// The names that appear in `text`, which are the only citations that may validate.
    fn names(&self) -> BTreeSet<String> {
        let text = self.text();
        let structural = (1..=self.structural.len())
            .map(|n| format!("F{n}"))
            .filter(|name| text.contains(&format!("{name}: ")));
        let prose = (1..=self.prose.len())
            .map(|n| format!("P{n}"))
            .filter(|name| text.contains(&format!("{name}: ")));
        structural.chain(prose).collect()
    }
}

/// The facts worth showing for one claim: what the claim names, what those items call and are called by,
/// then whatever else shares its words.
pub fn retrieve<'a>(claim: &str, facts: &'a [Fact], r: &Retrieval) -> Shown<'a> {
    let claim_terms = terms(claim);
    let mut scored: Vec<(f32, &Fact)> = facts
        .iter()
        .map(|fact| (overlap(&claim_terms, &render(fact)), fact))
        .filter(|(score, _)| *score > 0.0)
        .collect();
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| render(a.1).cmp(&render(b.1)))
    });
    let (mut structural, mut prose) = (Vec::new(), Vec::new());
    for (_, fact) in scored {
        if is_prose(fact) {
            if prose.len() < r.prose {
                prose.push(fact);
            }
        } else if structural.len() < r.structural {
            structural.push(fact);
        }
        if structural.len() >= r.structural && prose.len() >= r.prose {
            break;
        }
    }
    Shown {
        structural,
        prose,
        limits: (r.lines_per_fact, r.max_chars),
    }
}

/// One fact, cut to its first `lines` lines: the signature and what it does first, the long tail of calls
/// dropped with a note so the reader knows something was left out.
fn brief(fact: &Fact, lines: usize) -> String {
    let full = render(fact);
    let mut kept: Vec<&str> = full.lines().take(lines).collect();
    let dropped = full.lines().count().saturating_sub(kept.len());
    let note;
    if dropped > 0 {
        note = format!("  … {dropped} more line(s) not shown");
        kept.push(&note);
    }
    kept.join("\n")
}

/// A fact that is only what someone wrote about the code (D10).
fn is_prose(fact: &Fact) -> bool {
    match fact {
        Fact::Const(c) => c.prose,
        Fact::Function(f) => f.events.iter().all(|e| e.prose) && !f.doc.is_empty(),
        _ => false,
    }
}

pub const SYSTEM: &str = "You check one claim about a Rust codebase against facts taken from the code.\n\
The facts are `F1`, `F2`, … (what the code does) and `P1`, `P2`, … (what the code says about itself: doc \
comments, string literals, string constants).\n\
Break the claim into its parts — each thing it asserts — and for each part say whether the facts show it. \
Cite the facts you used by name.\n\
Rules you must follow:\n\
- Cite only facts you were shown. Do not recall anything about this project from memory.\n\
- A part no fact shows is uncovered. Saying so is the useful answer; guessing is not.\n\
- If a fact shows the claim is false, set `contradicted` and cite it.\n\
- `P` facts are what someone wrote, not what the code does: they may point at the right place, and they \
never establish a part on their own.\n\
Return one JSON object: {\"covered\": [{\"part\": string, \"facts\": [string]}], \"uncovered\": [string], \
\"contradicted\": boolean, \"contradicted_by\": [string]}";

#[derive(Debug, Default, Deserialize)]
struct Answer {
    #[serde(default)]
    covered: Vec<Covered>,
    #[serde(default)]
    uncovered: Vec<String>,
    #[serde(default)]
    contradicted: bool,
    #[serde(default)]
    contradicted_by: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Covered {
    #[serde(default)]
    part: String,
    #[serde(default)]
    facts: Vec<String>,
}

/// What the model made of one claim, with its citations validated.
#[derive(Debug, Clone)]
pub struct Matched {
    pub judgement: Judgement,
    /// Citations dropped because the model named a fact it was not shown.
    pub invented: Vec<String>,
}

/// Ask about one claim.
pub fn match_claim(
    claim: &str,
    shown: &Shown<'_>,
    judge: &dyn Judge,
    r: &Retrieval,
) -> Result<Matched, JudgeError> {
    let mut judgement = Judgement {
        covered: BTreeSet::new(),
        uncovered: BTreeSet::new(),
        prose_used: false,
        contradicted: false,
        run: r.run.clone(),
    };
    if shown.is_empty() {
        judgement.uncovered.insert(claim.to_string());
        return Ok(Matched {
            judgement,
            invented: Vec::new(),
        });
    }
    let user = format!("CLAIM: {claim}\n\nFACTS:\n\n{}", shown.text());
    let (text, _charged) = judge.ask_text(SYSTEM, &user)?;
    let answer: Answer =
        serde_json::from_str(&text).map_err(|e| JudgeError::Shape(e.to_string()))?;

    let known = shown.names();
    let mut invented = Vec::new();
    for part in answer.covered {
        if part.part.trim().is_empty() {
            continue;
        }
        let cited: Vec<String> = part
            .facts
            .iter()
            .map(|f| f.trim().to_string())
            .filter(|f| {
                let ok = known.contains(f);
                if !ok {
                    invented.push(f.clone());
                }
                ok
            })
            .collect();
        // A part covered only by facts that do not exist is not covered.
        if cited.is_empty() {
            judgement.uncovered.insert(part.part);
            continue;
        }
        if cited.iter().all(|f| f.starts_with('P')) {
            judgement.prose_used = true;
        }
        judgement.covered.insert(part.part);
    }
    for part in answer.uncovered {
        if !part.trim().is_empty() && !judgement.covered.contains(&part) {
            judgement.uncovered.insert(part);
        }
    }
    // A contradiction counts only when it names a fact that was shown.
    judgement.contradicted = answer.contradicted
        && answer
            .contradicted_by
            .iter()
            .any(|f| known.contains(f.trim()));
    Ok(Matched {
        judgement,
        invented,
    })
}

fn terms(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| t.len() >= 3)
        .map(str::to_lowercase)
        .collect()
}

fn overlap(claim: &BTreeSet<String>, text: &str) -> f32 {
    let other = terms(text);
    claim.iter().filter(|t| other.contains(*t)).count() as f32
}
