//! Extraction in passes (D6), once per commit and recipe (D7).
//!
//! **Decisions first, then the rest.** The research measured this: asked for everything at once a model
//! returns decisions with thin prerequisites, and asked per decision it returns prerequisites that name
//! the decision they condition. So pass one reads the commit and answers "what was decided"; pass two
//! asks, for each decision, what had to be true, what was turned down, and what follows.
//!
//! **The model proposes content only (D5).** Labels, sources and statuses are the tool's, assigned by
//! `cargo-smysl-corpus` from the commit it read. Quotes are checked there too (D8): a quote that is not
//! in the commit caps its unit at speculative, which is why every pass is asked for verbatim quotes.

use cargo_smysl_corpus::build::{
    ExAlternative, ExConsequence, ExDecision, ExPrerequisite, Extraction,
};
use serde::Deserialize;

use crate::judge::{Judge, JudgeError};

/// How the passes are run, so a recipe can be named in a cache key (D7).
#[derive(Debug, Clone)]
pub struct Recipe {
    /// What to call this way of extracting, in the cache and in the record.
    pub name: String,
    /// Decisions asked for at most; 0 is no limit.
    pub max_decisions: usize,
    /// Characters of the commit (message and diff) the model is shown.
    pub max_input: usize,
}

impl Default for Recipe {
    fn default() -> Recipe {
        Recipe {
            name: "v1".into(),
            max_decisions: 12,
            max_input: 40_000,
        }
    }
}

pub const DECISIONS_SYSTEM: &str = "You read one commit of a Rust project and report the decisions it \
makes.\n\
A decision is a choice the author made in this commit: `act` for something done, `decline` for something \
deliberately not done. The subject of the commit, a restatement of the diff, or a description of what the \
code now looks like is not a decision — what was chosen, and could have been chosen otherwise, is.\n\
For each decision give: `decision`, one sentence in the author's terms; `kind`, `act` or `decline`; \
`rationale`, why it was made, from the commit itself and not invented; and `quote`, a span copied \
verbatim from the message or the diff that shows it. A decision you cannot quote is one you should not \
report.\n\
Return one JSON object: {\"decisions\": [{\"decision\": string, \"kind\": \"act\"|\"decline\", \
\"rationale\": string, \"quote\": string}]}";

pub const ITEMS_SYSTEM: &str = "You read one commit of a Rust project and one decision it makes, and \
report what surrounds that decision.\n\
- `prerequisites`: what had to be true for the decision to make sense — existing behaviour, an invariant, \
a tool's setting, an earlier change, an assumption. Not the motivation, and not a restatement of the \
decision. Each carries `kind`, one of existing-behaviour, invariant, tool-setting, prior-change, \
assumption; and `normative` true when it is a rule the project states rather than a fact about the code.\n\
- `alternatives`: options the commit turned down, in the form the author considered them.\n\
- `consequences`: what follows from the decision. `verified` is true only when the commit itself shows it \
happening — a test that passes, a count that changed — and then `verification_quote` copies that span.\n\
Every item carries a `quote` copied verbatim from the message or the diff. An item you cannot quote is \
one you should not report: a quote that is not in the commit caps the item at speculative.\n\
Return one JSON object: {\"prerequisites\": [{\"text\": string, \"kind\": string, \"normative\": bool, \
\"quote\": string}], \"alternatives\": [{\"alternative\": string, \"quote\": string}], \"consequences\": \
[{\"consequence\": string, \"verified\": bool, \"verification_quote\": string, \"quote\": string}]}";

#[derive(Debug, Default, Deserialize)]
struct DecisionsAnswer {
    #[serde(default)]
    decisions: Vec<DecisionAnswer>,
}

#[derive(Debug, Deserialize)]
struct DecisionAnswer {
    #[serde(default)]
    decision: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    rationale: String,
    #[serde(default)]
    quote: String,
}

#[derive(Debug, Default, Deserialize)]
struct ItemsAnswer {
    #[serde(default)]
    prerequisites: Vec<PrerequisiteAnswer>,
    #[serde(default)]
    alternatives: Vec<AlternativeAnswer>,
    #[serde(default)]
    consequences: Vec<ConsequenceAnswer>,
}

#[derive(Debug, Deserialize)]
struct PrerequisiteAnswer {
    #[serde(default)]
    text: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    normative: bool,
    #[serde(default)]
    quote: String,
}

#[derive(Debug, Deserialize)]
struct AlternativeAnswer {
    #[serde(default)]
    alternative: String,
    #[serde(default)]
    quote: String,
}

#[derive(Debug, Deserialize)]
struct ConsequenceAnswer {
    #[serde(default)]
    consequence: String,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    verification_quote: String,
    #[serde(default)]
    quote: String,
}

/// What one extraction cost and left behind.
#[derive(Debug, Default)]
pub struct Report {
    pub calls: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Items the model returned that carried no text, and were dropped.
    pub empty: usize,
    pub warnings: Vec<String>,
}

/// Run the passes over one commit's text.
///
/// `input` is the commit as the tool read it — message first, then the diff — already the tool's own
/// reading of git, never the model's.
pub fn extract(
    input: &str,
    judge: &dyn Judge,
    recipe: &Recipe,
) -> Result<(Extraction, Report), JudgeError> {
    let mut report = Report::default();
    let shown = truncate(input, recipe.max_input);
    if shown.len() < input.len() {
        report.warnings.push(format!(
            "the commit is {} characters; the model was shown the first {}",
            input.len(),
            shown.len()
        ));
    }

    report.calls += 1;
    let answer: DecisionsAnswer = ask(judge, DECISIONS_SYSTEM, &format!("COMMIT:\n{shown}\n"))?;
    let mut extraction = Extraction::default();
    for (i, d) in answer.decisions.into_iter().enumerate() {
        if recipe.max_decisions > 0 && i >= recipe.max_decisions {
            report.warnings.push(format!(
                "the model reported more than {} decisions; the rest were not asked about",
                recipe.max_decisions
            ));
            break;
        }
        if d.decision.trim().is_empty() {
            report.empty += 1;
            continue;
        }
        let number = extraction.decisions.len() + 1;
        extraction.decisions.push(ExDecision {
            decision: d.decision.clone(),
            kind: if d.kind.trim().is_empty() {
                "act".into()
            } else {
                d.kind.clone()
            },
            rationale: d.rationale,
            quote: d.quote,
        });

        // Pass two, per decision: prerequisites, alternatives and consequences in the decision's own
        // terms. Asked one decision at a time because the research measured thin prerequisites when
        // everything was asked at once.
        report.calls += 1;
        let user = format!(
            "COMMIT:\n{shown}\n\nDECISION {number}: {}\nRATIONALE: {}\n",
            d.decision,
            extraction.decisions[number - 1].rationale
        );
        let items: ItemsAnswer = match ask(judge, ITEMS_SYSTEM, &user) {
            Ok(items) => items,
            // One decision's items failing is not the whole extraction failing; the decision stands.
            Err(e) => {
                report
                    .warnings
                    .push(format!("decision {number}: no items ({e})"));
                ItemsAnswer::default()
            }
        };
        for p in items.prerequisites {
            if p.text.trim().is_empty() {
                report.empty += 1;
                continue;
            }
            extraction.prerequisites.push(ExPrerequisite {
                decision: number,
                text: p.text,
                kind: p.kind,
                normative: p.normative,
                quote: p.quote,
            });
        }
        for a in items.alternatives {
            if a.alternative.trim().is_empty() {
                report.empty += 1;
                continue;
            }
            extraction.alternatives.push(ExAlternative {
                decision: number,
                alternative: a.alternative,
                reason: String::new(),
                quote: a.quote,
            });
        }
        for c in items.consequences {
            if c.consequence.trim().is_empty() {
                report.empty += 1;
                continue;
            }
            extraction.consequences.push(ExConsequence {
                decision: number,
                text: c.consequence,
                quote: c.quote,
                verified: c.verified,
                verification_quote: c.verification_quote,
            });
        }
    }
    Ok((extraction, report))
}

fn ask<T: serde::de::DeserializeOwned + Default>(
    judge: &dyn Judge,
    system: &str,
    user: &str,
) -> Result<T, JudgeError> {
    let (text, _charged) = judge.ask_text(system, user)?;
    serde_json::from_str(&text).map_err(|e| JudgeError::Shape(e.to_string()))
}

/// Cut on a character boundary, keeping the head: a commit's message and the start of its diff say more
/// than its tail.
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Where an extraction is kept, so a commit is extracted once per recipe (D7).
///
/// An improvement arrives as a new recipe, never as a re-extraction merged into the old one: extraction
/// is unstable across runs, and merging two runs of the same recipe would put two wordings of one
/// prerequisite in the store as two units.
pub struct Cache {
    dir: std::path::PathBuf,
}

impl Cache {
    pub fn at(root: impl AsRef<std::path::Path>) -> Cache {
        Cache {
            dir: root.as_ref().join(".smysl").join("extractions"),
        }
    }

    pub fn path(&self, sha: &str, recipe: &Recipe) -> std::path::PathBuf {
        let short: String = sha.chars().take(12).collect();
        self.dir.join(&recipe.name).join(format!("{short}.json"))
    }

    pub fn read(&self, sha: &str, recipe: &Recipe) -> Option<Extraction> {
        let text = std::fs::read_to_string(self.path(sha, recipe)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn write(
        &self,
        sha: &str,
        recipe: &Recipe,
        extraction: &Extraction,
    ) -> Result<std::path::PathBuf, std::io::Error> {
        let path = self.path(sha, recipe);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(extraction)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(&path, text + "\n")?;
        Ok(path)
    }
}
