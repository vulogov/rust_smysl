//! One commit's record, read back out for a person.
//!
//! The store is a graph and reads like one. A person reviewing a change wants the other shape: this
//! decision, what it rests on, what was turned down, what follows. That grouping is already in the
//! labels the tool assigned — `d/g<sha>-2` is the second decision of that commit, `p/g<sha>-2-1` the
//! first prerequisite of that decision — so reading it back needs no search and no model.
//!
//! **Status is carried through, never smoothed over.** A unit whose quote was not in the commit is
//! `speculative`, and it says so wherever it is shown; a reader who is not told cannot weigh it.

use std::collections::BTreeMap;

use smysl::{Label, Store, Uid};

/// One recorded unit, as a reader meets it.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub label: String,
    pub gist: String,
    /// The rationale or note recorded with it, if any.
    pub body: String,
    /// `cited`, `speculative`, and so on, as the tool assigned it (D8).
    pub status: String,
    /// Where it came from: the commit, or a file in it.
    pub source: String,
}

/// A decision and everything the commit recorded around it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Decision {
    pub item: Option<Item>,
    pub prerequisites: Vec<Item>,
    pub alternatives: Vec<Item>,
    pub consequences: Vec<Item>,
    /// Code anchors this decision touches, by gist (`path::item`).
    pub anchors: Vec<String>,
}

/// Everything one commit put in the corpus.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CommitRecord {
    pub commit: String,
    pub decisions: Vec<Decision>,
    /// Units of this commit that name no decision: rare, and worth showing rather than dropping.
    pub loose: Vec<Item>,
}

impl CommitRecord {
    pub fn is_empty(&self) -> bool {
        self.decisions.is_empty() && self.loose.is_empty()
    }

    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let sum = |f: fn(&Decision) -> usize| self.decisions.iter().map(f).sum();
        (
            self.decisions.len(),
            sum(|d| d.prerequisites.len()),
            sum(|d| d.alternatives.len()),
            sum(|d| d.consequences.len()),
        )
    }
}

/// Read one commit's record out of the store.
///
/// `sha` may be abbreviated; labels carry the first twelve characters.
pub fn commit_record(store: &Store, labels: &BTreeMap<Uid, Vec<Label>>, sha: &str) -> CommitRecord {
    let short: String = sha.chars().take(12).collect();
    let mut record = CommitRecord {
        commit: short.clone(),
        ..CommitRecord::default()
    };
    // Label -> item, for everything this commit recorded.
    let mut items: BTreeMap<String, Item> = BTreeMap::new();
    let mut anchors: BTreeMap<String, Uid> = BTreeMap::new();
    for (uid, unit) in store.units() {
        let Some(label) = labels.get(uid).and_then(|l| l.first()) else {
            continue;
        };
        let label = label.as_str().to_string();
        if !label.contains(&format!("g{short}")) {
            continue;
        }
        let item = Item {
            label: label.clone(),
            gist: unit.core.gist.clone(),
            body: unit.core.body.clone().unwrap_or_default(),
            status: unit.core.status.to_string(),
            source: unit
                .core
                .source
                .as_ref()
                .map(|s| s.reference.clone())
                .unwrap_or_default(),
        };
        if label.starts_with("a/") {
            anchors.insert(item.gist.clone(), *uid);
        }
        items.insert(label, item);
    }

    // The label scheme is the grouping: `d/…-2` owns `p/…-2-1`, `r/…-2-1`, `q/…-2-1`.
    let decision_numbers: Vec<String> = items
        .keys()
        .filter_map(|l| {
            l.strip_prefix("d/")?
                .rsplit_once('-')
                .map(|(_, n)| n.to_string())
        })
        .collect();
    for n in decision_numbers {
        let owned = |prefix: &str| -> Vec<Item> {
            items
                .iter()
                .filter(|(l, _)| {
                    l.starts_with(prefix)
                        && l.rsplit_once('-')
                            .and_then(|(head, _)| head.rsplit_once('-'))
                            .map(|(_, d)| d == n)
                            .unwrap_or(false)
                })
                .map(|(_, item)| item.clone())
                .collect()
        };
        let label = format!("d/g{short}-{n}");
        let item = items.get(&label).cloned();
        let touching: Vec<String> = match item.as_ref() {
            Some(_) => anchors
                .iter()
                .filter(|(_, anchor)| {
                    store.relations().any(|r| {
                        r.kind.to_string() == crate::REL_TOUCHES
                            && r.to == **anchor
                            && labels
                                .get(&r.from)
                                .and_then(|l| l.first())
                                .map(|l| l.as_str() == label)
                                .unwrap_or(false)
                    })
                })
                .map(|(gist, _)| gist.clone())
                .collect(),
            None => Vec::new(),
        };
        record.decisions.push(Decision {
            item,
            prerequisites: owned("p/"),
            alternatives: owned("r/"),
            consequences: owned("q/"),
            anchors: touching,
        });
    }
    record.decisions.sort_by(|a, b| {
        let key = |d: &Decision| {
            d.item
                .as_ref()
                .and_then(|i| {
                    i.label
                        .rsplit_once('-')
                        .and_then(|(_, n)| n.parse::<u32>().ok())
                })
                .unwrap_or(u32::MAX)
        };
        key(a).cmp(&key(b))
    });
    record
}

/// The record as prose, for a terminal.
pub fn as_text(record: &CommitRecord) -> String {
    let (d, p, r, q) = record.counts();
    let mut out = format!(
        "{}: {d} decision(s), {p} prerequisite(s), {r} alternative(s), {q} consequence(s)\n",
        record.commit
    );
    for decision in &record.decisions {
        let Some(item) = &decision.item else { continue };
        out.push_str(&format!(
            "\n{} [{}]\n  {}\n",
            item.label, item.status, item.gist
        ));
        if !item.body.is_empty() {
            out.push_str(&format!("  because: {}\n", item.body));
        }
        for anchor in &decision.anchors {
            out.push_str(&format!("  touches: {anchor}\n"));
        }
        let section = |name: &str, items: &[Item], out: &mut String| {
            for i in items {
                out.push_str(&format!("  {name}: {} [{}]\n", i.gist, i.status));
            }
        };
        section("needs", &decision.prerequisites, &mut out);
        section("not", &decision.alternatives, &mut out);
        section("so", &decision.consequences, &mut out);
    }
    out
}

/// The record as Markdown, for a pull request.
///
/// Speculative units are marked where they are read, not in a footnote: the status is the reason a
/// reader should treat two lines differently.
pub fn as_markdown(record: &CommitRecord) -> String {
    let (d, p, r, q) = record.counts();
    let mut out = format!(
        "### Why this change\n\n\
         Recorded from `{}` — {d} decision(s), {p} prerequisite(s), {r} alternative(s), \
         {q} consequence(s).\n",
        record.commit
    );
    for decision in &record.decisions {
        let Some(item) = &decision.item else { continue };
        out.push_str(&format!("\n**{}**", item.gist));
        if item.status != "cited" {
            out.push_str(&format!(" _({})_", item.status));
        }
        out.push('\n');
        if !item.body.is_empty() {
            out.push_str(&format!("\n{}\n", item.body));
        }
        let list = |name: &str, items: &[Item], out: &mut String| {
            if items.is_empty() {
                return;
            }
            out.push_str(&format!("\n{name}\n"));
            for i in items {
                let mark = if i.status == "cited" {
                    String::new()
                } else {
                    format!(" _({})_", i.status)
                };
                out.push_str(&format!("- {}{mark}\n", i.gist));
            }
        };
        list("Rests on:", &decision.prerequisites, &mut out);
        list("Turned down:", &decision.alternatives, &mut out);
        list("So:", &decision.consequences, &mut out);
        if !decision.anchors.is_empty() {
            out.push_str(&format!("\nTouches: `{}`\n", decision.anchors.join("`, `")));
        }
    }
    out.push_str(
        "\n<sub>Recorded by `cargo smysl`. A unit marked speculative quoted something that is not in \
         the commit.</sub>\n",
    );
    out
}
