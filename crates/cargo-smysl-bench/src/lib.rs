//! Measuring extraction against what a person says a commit decided.
//!
//! This is the instrument, not a verdict: a label file is written by a person from the commit alone, an
//! adjudication pairs what the tool extracted with what they labelled, and a score counts. The judgement
//! that an extracted item means the same thing as a labelled one is the person's, recorded in a file.
//!
//! **Why it ships.** Extraction quality is a property of the model an operator chose, not of this code
//! (D19). The figures this project measured name their models and are in the documentation; the only
//! figure about someone else's model is the one they measure themselves, on their own commits, with
//! this.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub mod sheet;

// ----------------------------------------------------------------------------------------------
// Labels (written by a person)
// ----------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Decision,
    Prerequisite,
    Alternative,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Decision, Kind::Prerequisite, Kind::Alternative];
}

#[derive(Debug, Deserialize)]
pub struct LabelFile {
    pub schema: u32,
    pub repo: String,
    pub commit: String,
    pub status: String,
    #[serde(default, rename = "decision")]
    pub decisions: Vec<LabelItem>,
    #[serde(default, rename = "prerequisite")]
    pub prerequisites: Vec<LabelItem>,
    #[serde(default, rename = "alternative")]
    pub alternatives: Vec<LabelItem>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LabelItem {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub normative: Option<bool>,
    #[serde(default)]
    pub evidence: Option<String>,
}

impl LabelFile {
    pub fn is_done(&self) -> bool {
        self.status == "done"
    }

    pub fn items(&self, kind: Kind) -> &[LabelItem] {
        match kind {
            Kind::Decision => &self.decisions,
            Kind::Prerequisite => &self.prerequisites,
            Kind::Alternative => &self.alternatives,
        }
    }

    /// Ids must be unique per file, and prerequisites/alternatives must name a labelled decision.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for kind in Kind::ALL {
            for item in self.items(kind) {
                if !seen.insert(item.id.clone()) {
                    return Err(format!("duplicate id {}", item.id));
                }
                if item.text.trim().is_empty() {
                    return Err(format!("{} has no text", item.id));
                }
            }
        }
        let decisions: BTreeSet<_> = self.decisions.iter().map(|d| d.id.as_str()).collect();
        for item in self.prerequisites.iter().chain(&self.alternatives) {
            match item.decision.as_deref() {
                Some(d) if decisions.contains(d) => {}
                other => {
                    return Err(format!(
                        "{} names decision {:?}, which is not labelled",
                        item.id, other
                    ))
                }
            }
        }
        Ok(())
    }
}

// ----------------------------------------------------------------------------------------------
// Extractions (produced by a system under test, in the research JSON shape)
// ----------------------------------------------------------------------------------------------

/// An extracted item, normalised: ids `D1…`, `P1…`, `A1…` in the order the system produced them.
#[derive(Debug, Clone, PartialEq)]
pub struct Extracted {
    pub kind: Kind,
    pub id: String,
    pub text: String,
}

/// Read the research extraction shape: `decisions[].decision`, `prerequisites[].text`,
/// `alternatives[].alternative` (+ `reason`).
pub fn extracted_items(json: &serde_json::Value) -> Vec<Extracted> {
    let mut out = Vec::new();
    let arr = |k: &str| {
        json.get(k)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };
    for (i, d) in arr("decisions").iter().enumerate() {
        out.push(Extracted {
            kind: Kind::Decision,
            id: format!("D{}", i + 1),
            text: str_of(d, "decision"),
        });
    }
    for (i, p) in arr("prerequisites").iter().enumerate() {
        out.push(Extracted {
            kind: Kind::Prerequisite,
            id: format!("P{}", i + 1),
            text: str_of(p, "text"),
        });
    }
    for (i, a) in arr("alternatives").iter().enumerate() {
        let reason = str_of(a, "reason");
        let text = if reason.is_empty() {
            str_of(a, "alternative")
        } else {
            format!("{} — {}", str_of(a, "alternative"), reason)
        };
        out.push(Extracted {
            kind: Kind::Alternative,
            id: format!("A{}", i + 1),
            text,
        });
    }
    out
}

fn str_of(v: &serde_json::Value, k: &str) -> String {
    v.get(k)
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string()
}

// ----------------------------------------------------------------------------------------------
// Suggestions
// ----------------------------------------------------------------------------------------------

const STOP: &[&str] = &[
    "a", "an", "the", "and", "or", "of", "to", "in", "on", "for", "is", "are", "be", "it", "its",
    "this", "that", "with", "as", "by", "at", "from", "not", "no", "into", "than", "so", "can",
    "must", "when", "which",
];

fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(str::to_lowercase)
        .filter(|w| w.len() > 1 && !STOP.contains(&w.as_str()))
        .collect()
}

/// Label ids of the same kind, most word overlap first (Jaccard), up to `limit`, overlap > 0 only.
pub fn suggest(item: &Extracted, labels: &LabelFile, limit: usize) -> Vec<String> {
    let a = words(&item.text);
    let mut scored: Vec<(f64, &str)> = labels
        .items(item.kind)
        .iter()
        .filter_map(|l| {
            let b = words(&l.text);
            let inter = a.intersection(&b).count();
            let union = a.union(&b).count();
            (inter > 0).then(|| (inter as f64 / union as f64, l.id.as_str()))
        })
        .collect();
    scored.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(y.1)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, id)| id.to_string())
        .collect()
}

// ----------------------------------------------------------------------------------------------
// Adjudications (written by a person)
// ----------------------------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Adjudication {
    pub system: String,
    pub repo: String,
    pub commit: String,
    #[serde(default, rename = "item")]
    pub items: Vec<AdjudicatedItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdjudicatedItem {
    pub kind: Kind,
    pub id: String,
    pub text: String,
    pub suggest: Vec<String>,
    /// A label id of the same kind, `"none"`, or empty while not yet adjudicated.
    #[serde(rename = "match")]
    pub matched: String,
}

/// A fresh adjudication for an extraction, keeping any `match` a person already recorded for an
/// item with the same kind, id and text.
pub fn adjudication(
    system: &str,
    labels: &LabelFile,
    extracted: &[Extracted],
    previous: Option<&Adjudication>,
) -> Adjudication {
    let kept: BTreeMap<(Kind, &str, &str), &str> = previous
        .map(|p| {
            p.items
                .iter()
                .map(|i| ((i.kind, i.id.as_str(), i.text.as_str()), i.matched.as_str()))
                .collect()
        })
        .unwrap_or_default();
    Adjudication {
        system: system.to_string(),
        repo: labels.repo.clone(),
        commit: labels.commit.clone(),
        items: extracted
            .iter()
            .map(|e| AdjudicatedItem {
                kind: e.kind,
                id: e.id.clone(),
                text: e.text.clone(),
                suggest: suggest(e, labels, 3),
                matched: kept
                    .get(&(e.kind, e.id.as_str(), e.text.as_str()))
                    .copied()
                    .unwrap_or_default()
                    .to_string(),
            })
            .collect(),
    }
}

// ----------------------------------------------------------------------------------------------
// Scores
// ----------------------------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    pub extracted: usize,
    pub matched: usize,
    pub labels: usize,
    pub covered: usize,
    pub pending: usize,
}

impl Tally {
    pub fn add(&mut self, o: Tally) {
        self.extracted += o.extracted;
        self.matched += o.matched;
        self.labels += o.labels;
        self.covered += o.covered;
        self.pending += o.pending;
    }
    pub fn precision(&self) -> Option<f64> {
        (self.extracted > self.pending)
            .then(|| self.matched as f64 / (self.extracted - self.pending) as f64)
    }
    pub fn recall(&self) -> Option<f64> {
        (self.labels > 0 && self.pending == 0).then(|| self.covered as f64 / self.labels as f64)
    }
}

/// Score one commit. Errors if the adjudication names a label id that does not exist.
pub fn score(labels: &LabelFile, adj: &Adjudication) -> Result<BTreeMap<Kind, Tally>, String> {
    let mut out = BTreeMap::new();
    for kind in Kind::ALL {
        let ids: BTreeSet<&str> = labels.items(kind).iter().map(|l| l.id.as_str()).collect();
        let mut t = Tally {
            labels: ids.len(),
            ..Tally::default()
        };
        let mut covered = BTreeSet::new();
        for item in adj.items.iter().filter(|i| i.kind == kind) {
            t.extracted += 1;
            match item.matched.as_str() {
                "" => t.pending += 1,
                "none" => {}
                id if ids.contains(id) => {
                    t.matched += 1;
                    covered.insert(id);
                }
                id => {
                    return Err(format!(
                        "{} {}: match {id:?} is not a {kind:?} label id",
                        adj.commit, item.id
                    ))
                }
            }
        }
        t.covered = covered.len();
        out.insert(kind, t);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> LabelFile {
        toml::from_str(
            r#"
schema = 1
repo = "smysl"
commit = "90ec2f7"
status = "done"

[[decision]]
id = "D1"
text = "Add a test that runs every command"
kind = "act"

[[prerequisite]]
id = "P1"
decision = "D1"
text = "cli() registers all subcommands from the COMMANDS table unconditionally"
kind = "existing-behaviour"

[[prerequisite]]
id = "P2"
decision = "D1"
text = "clap rejects a missing required argument before routing"
kind = "existing-behaviour"
"#,
        )
        .unwrap()
    }

    fn extraction() -> serde_json::Value {
        serde_json::json!({
            "decisions": [{"decision": "Add tests/dispatch.rs to run every command"}],
            "prerequisites": [
                {"decision": 1, "text": "The cli() function registers all subcommands unconditionally."},
                {"decision": 1, "text": "Seven commands require an argument."},
                {"decision": 1, "text": "COMMANDS table registers subcommands in cli()"}
            ],
            "alternatives": []
        })
    }

    #[test]
    fn labels_validate_and_catch_dangling_decisions() {
        assert!(labels().validate().is_ok());
        let mut bad = labels();
        bad.prerequisites[0].decision = Some("D9".into());
        assert!(bad.validate().unwrap_err().contains("D9"));
    }

    #[test]
    fn suggestions_rank_by_overlap_within_a_kind() {
        let items = extracted_items(&extraction());
        assert_eq!(
            items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            ["D1", "P1", "P2", "P3"]
        );
        assert_eq!(suggest(&items[1], &labels(), 3)[0], "P1");
        assert_eq!(
            suggest(&items[0], &labels(), 3),
            ["D1"],
            "a decision is only suggested decision labels"
        );
    }

    #[test]
    fn scoring_counts_duplicates_once_for_recall_and_reports_pending() {
        let mut adj = adjudication("research", &labels(), &extracted_items(&extraction()), None);
        let set = |adj: &mut Adjudication, id: &str, m: &str| {
            adj.items.iter_mut().find(|i| i.id == id).unwrap().matched = m.into()
        };
        set(&mut adj, "D1", "D1");
        set(&mut adj, "P1", "P1");
        set(&mut adj, "P2", "none");
        let pending = score(&labels(), &adj).unwrap();
        assert_eq!(pending[&Kind::Prerequisite].pending, 1);
        assert_eq!(
            pending[&Kind::Prerequisite].recall(),
            None,
            "recall is not reported while items are pending"
        );

        set(&mut adj, "P3", "P1");
        let s = score(&labels(), &adj).unwrap();
        let p = s[&Kind::Prerequisite];
        assert_eq!((p.extracted, p.matched, p.labels, p.covered), (3, 2, 2, 1));
        assert_eq!(p.precision(), Some(2.0 / 3.0));
        assert_eq!(p.recall(), Some(0.5));
        assert_eq!(s[&Kind::Decision].recall(), Some(1.0));
    }

    #[test]
    fn an_unknown_match_id_is_an_error_not_a_miss() {
        let mut adj = adjudication("research", &labels(), &extracted_items(&extraction()), None);
        adj.items[1].matched = "P7".into();
        assert!(score(&labels(), &adj).is_err());
    }

    #[test]
    fn regenerating_an_adjudication_keeps_recorded_matches() {
        let mut first = adjudication("research", &labels(), &extracted_items(&extraction()), None);
        first.items[1].matched = "P1".into();
        let again = adjudication(
            "research",
            &labels(),
            &extracted_items(&extraction()),
            Some(&first),
        );
        assert_eq!(again.items[1].matched, "P1");
    }
}
