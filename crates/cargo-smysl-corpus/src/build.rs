//! Extraction → smysl units (plan D2–D5, D8).
//!
//! The model's extraction proposes content only. Here the tool assigns everything else: labels per
//! commit and run, sources pointing at the commit or a file at the commit, statuses from checking
//! each quote against the commit's own text, and the edges of the data model. Nothing here calls a
//! model or reads git: the caller supplies the commit's message and diff text.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use smysl::surface::hjson::{HObject, HValue, Spanned};
use smysl::surface::payload::object_to_payload;
use smysl::{
    canonical_uid, quote_support_in, KernelType, Label, QuoteSupport, RelKind, Relation,
    SchemaDecl, SchemaId, SourceKind, SourceRef, Span, Status, Uid, UnitCore, UnitCoreBuilder,
    QUOTE_KEY,
};

use crate::{Labels, CODE_SCHEMA_ID, REL_EXERCISES, REL_TOUCHES};

/// Payload key recording what kind of item a unit came from (e.g. `decline`, `existing-behaviour`).
pub const KIND_KEY: &str = "code:kind";

/// An extraction in the research shape (see `eval/extractions/`).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Extraction {
    #[serde(default)]
    pub decisions: Vec<ExDecision>,
    #[serde(default)]
    pub prerequisites: Vec<ExPrerequisite>,
    #[serde(default)]
    pub alternatives: Vec<ExAlternative>,
    #[serde(default)]
    pub consequences: Vec<ExConsequence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExDecision {
    pub decision: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExPrerequisite {
    pub decision: usize,
    pub text: String,
    #[serde(default)]
    pub kind: String,
    /// A rule the project states, rather than a fact about the code. D12 caps what a normative
    /// prerequisite may claim, so it is carried from extraction rather than guessed at later.
    #[serde(default)]
    pub normative: bool,
    #[serde(default)]
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExAlternative {
    pub decision: usize,
    pub alternative: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExConsequence {
    pub decision: usize,
    pub text: String,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub verification_quote: String,
}

/// The commit's own text, which every quote is checked against.
#[derive(Debug, Clone)]
pub struct CommitText<'a> {
    /// Full or abbreviated hex sha.
    pub sha: &'a str,
    pub message: &'a str,
    /// `(path, text)` per changed file: the diff's lines for that file, markers stripped.
    pub files: Vec<(&'a str, &'a str)>,
}

/// Units, edges and labels ready for `stage`, plus what the build had to do to stay honest.
#[derive(Debug, Default)]
pub struct Batch {
    pub units: Vec<UnitCore>,
    pub relations: Vec<Relation>,
    pub labels: BTreeMap<Label, Uid>,
    pub quotes: QuoteTally,
    /// Items that could not be placed (e.g. naming a decision that does not exist).
    pub dropped: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct QuoteTally {
    pub present: usize,
    pub loose: usize,
    pub absent: usize,
    pub in_diff: usize,
}

#[derive(Debug)]
pub enum BuildError {
    Label(smysl::IdError),
    Shape(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Label(e) => write!(f, "label: {e}"),
            BuildError::Shape(e) => write!(f, "unit shape: {e}"),
        }
    }
}

impl std::error::Error for BuildError {}

/// The `x.code/v1` declaration a batch is staged with.
pub fn code_schema() -> SchemaDecl {
    let mut decl = SchemaDecl::new(SchemaId::parse(CODE_SCHEMA_ID).expect("valid schema id"), 1);
    decl.relations = [REL_TOUCHES, REL_EXERCISES]
        .iter()
        .map(|r| RelKind::parse(r).expect("valid relation kind"))
        .collect();
    decl
}

/// Longest summary kept in a gist; longer text is shortened here and kept whole in the body.
const GIST_MAX_BYTES: usize = 110;
const GIST_MAX_WORDS: usize = 20;

fn gist_and_body(text: &str) -> (String, Option<String>) {
    let text = text.trim();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut gist = String::new();
    for w in words.iter().take(GIST_MAX_WORDS) {
        if gist.len() + w.len() + 1 > GIST_MAX_BYTES {
            break;
        }
        if !gist.is_empty() {
            gist.push(' ');
        }
        gist.push_str(w);
    }
    if gist.len() == text.len() {
        (gist, None)
    } else {
        (format!("{gist} …"), Some(text.to_string()))
    }
}

/// One unit to build.
struct Spec<'t> {
    kind: KernelType,
    label: Label,
    text: &'t str,
    body: Option<&'t str>,
    status: Status,
    source: Option<SourceRef>,
    grounds: Vec<Uid>,
    payload: Option<Vec<u8>>,
}

struct Builder<'a> {
    commit: &'a CommitText<'a>,
    labels: Labels,
    batch: Batch,
}

impl<'a> Builder<'a> {
    fn payload(quote: &str, kind: &str) -> Option<Vec<u8>> {
        let span = Span::new(0, 0);
        let mut o = HObject::default();
        if !quote.trim().is_empty() {
            o.insert(
                Spanned::new(QUOTE_KEY.to_string(), span),
                Spanned::new(HValue::Str(quote.to_string()), span),
            );
        }
        if !kind.trim().is_empty() {
            o.insert(
                Spanned::new(KIND_KEY.to_string(), span),
                Spanned::new(HValue::Str(kind.to_string()), span),
            );
        }
        object_to_payload(&o)
    }

    /// Where a quote occurs in the commit's text, without counting it.
    fn locate(&self, quote: &str) -> (QuoteSupport, Option<String>) {
        if quote.trim().is_empty() {
            return (QuoteSupport::Absent, None);
        }
        let mut sources: Vec<(&str, &str)> = vec![("message", self.commit.message)];
        sources.extend(self.commit.files.iter().copied());
        let (support, place) = quote_support_in(quote, &sources);
        (support, place.map(str::to_string))
    }

    /// `cited` with a source when the quote is in the commit's text; `speculative` with none when not.
    fn sourced(&mut self, quote: &str) -> (Status, Option<SourceRef>) {
        let (support, place) = self.locate(quote);
        let short: String = self.commit.sha.chars().take(12).collect();
        match (support, place) {
            (QuoteSupport::Present | QuoteSupport::Loose, Some(place)) => {
                if support == QuoteSupport::Present {
                    self.batch.quotes.present += 1;
                } else {
                    self.batch.quotes.loose += 1;
                }
                let source = if place == "message" {
                    SourceRef::new(SourceKind::Doc, format!("git:{}", self.commit.sha))
                } else {
                    self.batch.quotes.in_diff += 1;
                    SourceRef::new(SourceKind::File, format!("{place}@{short}"))
                };
                (Status::Cited, Some(source))
            }
            _ => {
                self.batch.quotes.absent += 1;
                (Status::Speculative, None)
            }
        }
    }

    fn unit(&mut self, spec: Spec<'_>) -> Result<Uid, BuildError> {
        let (gist, overflow) = gist_and_body(spec.text);
        let mut b = UnitCoreBuilder::new(spec.kind, gist, spec.status);
        match (spec.body.filter(|b| !b.trim().is_empty()), overflow) {
            (Some(body), _) => b = b.body(body.trim()),
            (None, Some(full)) => b = b.body(full),
            (None, None) => {}
        }
        if let Some(s) = spec.source {
            b = b.source(s);
        }
        if !spec.grounds.is_empty() {
            b = b.grounds(spec.grounds);
        }
        if let Some(p) = spec.payload {
            b = b.payload(p);
        }
        let core = b.build().map_err(|e| BuildError::Shape(e.to_string()))?;
        let uid = canonical_uid(&core);
        self.batch.units.push(core);
        self.batch.labels.insert(spec.label, uid);
        Ok(uid)
    }

    fn edge(&mut self, kind: &str, from: Uid, to: Uid) {
        self.batch.relations.push(Relation::new(
            RelKind::parse(kind).expect("valid relation kind"),
            from,
            to,
        ));
    }
}

/// Build one extraction of one commit into units and edges.
pub fn build(ex: &Extraction, commit: &CommitText<'_>, run: u32) -> Result<Batch, BuildError> {
    let mut b = Builder {
        commit,
        labels: Labels::new(commit.sha, run).map_err(BuildError::Label)?,
        batch: Batch::default(),
    };

    let mut decisions: Vec<(Uid, Status)> = Vec::new();
    for (i, d) in ex.decisions.iter().enumerate() {
        let n = i as u32 + 1;
        let (status, source) = b.sourced(&d.quote);
        let label = b.labels.decision(n);
        let payload = Builder::payload(&d.quote, &d.kind);
        let uid = b.unit(Spec {
            kind: KernelType::Decision,
            label,
            text: &d.decision,
            body: Some(&d.rationale),
            status,
            source,
            grounds: vec![],
            payload,
        })?;
        decisions.push((uid, status));
    }
    let decision = |b: &mut Builder, idx: usize, what: &str| -> Option<(u32, Uid, Status)> {
        match decisions.get(idx.wrapping_sub(1)) {
            Some(&(uid, status)) => Some((idx as u32, uid, status)),
            None => {
                b.batch
                    .dropped
                    .push(format!("{what} names decision {idx}, which does not exist"));
                None
            }
        }
    };

    // Numbering within a decision, in the order items appear (matches the research labels).
    let mut seq: BTreeMap<(char, u32), u32> = BTreeMap::new();
    let mut next = |k: char, d: u32| {
        let e = seq.entry((k, d)).or_insert(0);
        *e += 1;
        *e
    };

    for p in &ex.prerequisites {
        let Some((d, d_uid, _)) = decision(&mut b, p.decision, "prerequisite") else {
            continue;
        };
        let (status, source) = b.sourced(&p.quote);
        let label = b.labels.prerequisite(d, next('p', d));
        let payload = Builder::payload(&p.quote, &p.kind);
        let uid = b.unit(Spec {
            kind: KernelType::Constraint,
            label,
            text: &p.text,
            body: None,
            status,
            source,
            grounds: vec![],
            payload,
        })?;
        // D3: `conditions`, not `grounds`, so rewording a prerequisite never moves the decision.
        b.edge("conditions", uid, d_uid);
    }

    for a in &ex.alternatives {
        let Some((d, d_uid, _)) = decision(&mut b, a.decision, "alternative") else {
            continue;
        };
        let (status, source) = b.sourced(&a.quote);
        let label = b.labels.alternative(d, next('r', d));
        let text = format!("Rejected: {}", a.alternative);
        let payload = Builder::payload(&a.quote, "rejected-alternative");
        let uid = b.unit(Spec {
            kind: KernelType::Claim,
            label,
            text: &text,
            body: Some(&a.reason),
            status,
            source,
            grounds: vec![],
            payload,
        })?;
        b.edge("contrasts", uid, d_uid);
    }

    for c in &ex.consequences {
        let Some((d, d_uid, d_status)) = decision(&mut b, c.decision, "consequence") else {
            continue;
        };
        let n = next('q', d);
        let label = b.labels.consequence(d, n);
        let checked = c.verified
            && matches!(
                b.locate(&c.verification_quote).0,
                QuoteSupport::Present | QuoteSupport::Loose
            );
        if checked {
            let (e_status, e_source) = b.sourced(&c.verification_quote);
            let e_label = b.labels.evidence(d, n);
            let text = format!("Check: {}", c.verification_quote);
            let payload = Builder::payload(&c.verification_quote, "");
            let e_uid = b.unit(Spec {
                kind: KernelType::Evidence,
                label: e_label,
                text: &text,
                body: None,
                status: e_status,
                source: e_source,
                grounds: vec![],
                payload,
            })?;
            let (status, source) = b.sourced(&c.quote);
            let payload = Builder::payload(&c.quote, "");
            let uid = b.unit(Spec {
                kind: KernelType::Finding,
                label,
                text: &c.text,
                body: None,
                status,
                source,
                grounds: vec![],
                payload,
            })?;
            b.edge("backs", e_uid, uid);
            b.edge("causes", d_uid, uid);
        } else {
            // An anticipated effect is inferred from the decision, never stronger than it (rule M).
            let status = if d_status == Status::Speculative {
                Status::Speculative
            } else {
                Status::Inferred
            };
            let payload = Builder::payload(&c.quote, "anticipated");
            let uid = b.unit(Spec {
                kind: KernelType::Claim,
                label,
                text: &c.text,
                body: None,
                status,
                source: None,
                grounds: vec![d_uid],
                payload,
            })?;
            b.edge("causes", d_uid, uid);
        }
    }

    Ok(b.batch)
}
