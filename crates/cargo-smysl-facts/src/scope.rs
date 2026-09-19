//! Which facts bear on a change, and how they read (D9).
//!
//! A workspace has tens of thousands of facts and a question concerns a handful. The scope is stated
//! rather than guessed: the items a change touches, the items a claim names, and **one hop outward** —
//! what those items call, and what calls them. One hop, because the research measured that two pulls in
//! most of the crate and the model stops distinguishing.
//!
//! Selection is deterministic: same facts, same change, same claim, same list in the same order. It has
//! to be, or a verdict is not reproducible.

use std::collections::{BTreeMap, BTreeSet};

use crate::item::{Constant, Fact, Function, Structure};

/// What to select around.
#[derive(Debug, Default, Clone)]
pub struct Around {
    /// Files the change touches, repository-relative.
    pub files: Vec<String>,
    /// Identifiers a message or a claim names (`Session::now`, `root_beside`, `LEDGER`).
    pub names: Vec<String>,
    /// Hops outward along calls. 1 is the measured default; 0 is the touched items alone.
    pub hops: usize,
}

impl Around {
    pub fn files(files: impl IntoIterator<Item = String>) -> Around {
        Around {
            files: files.into_iter().collect(),
            names: Vec::new(),
            hops: 1,
        }
    }

    pub fn naming(mut self, names: impl IntoIterator<Item = String>) -> Around {
        self.names = names.into_iter().collect();
        self
    }
}

/// Why a fact is in the scope, which is worth saying: a reader can tell what was asked for from what was
/// dragged in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// In a file the change touches.
    Touched,
    /// Its name appears in the message or the claim.
    Named,
    /// Called by something already selected.
    Calls,
    /// Calls something already selected.
    CalledBy,
}

#[derive(Debug, Clone)]
pub struct Selected<'a> {
    pub fact: &'a Fact,
    pub reason: Reason,
}

/// The facts that bear on `around`, in file and line order.
pub fn select<'a>(facts: &'a [Fact], around: &Around) -> Vec<Selected<'a>> {
    let mut why: BTreeMap<usize, Reason> = BTreeMap::new();
    let named: BTreeSet<&str> = around.names.iter().map(String::as_str).collect();

    for (i, fact) in facts.iter().enumerate() {
        if around.files.iter().any(|f| f == file_of(fact)) {
            why.insert(i, Reason::Touched);
        } else if named.contains(name_of(fact)) || named.contains(label_of(fact).as_str()) {
            why.insert(i, Reason::Named);
        }
    }

    // One hop: what the selected functions call, and what calls them. Names are matched on the callee's
    // last path segment, which is what `syn` gives without a resolver — good within a crate, and honest
    // about being a name rather than a binding (plan §9, "name-based resolution").
    for _ in 0..around.hops {
        let mut added: BTreeMap<usize, Reason> = BTreeMap::new();
        let selected_names: BTreeSet<String> = why
            .keys()
            .map(|i| name_of(&facts[*i]).to_string())
            .collect();
        let calls_of_selected: BTreeSet<String> = why
            .keys()
            .filter_map(|i| function(&facts[*i]))
            .flat_map(callees)
            .collect();
        for (i, fact) in facts.iter().enumerate() {
            if why.contains_key(&i) {
                continue;
            }
            if calls_of_selected.contains(name_of(fact)) {
                added.insert(i, Reason::Calls);
                continue;
            }
            if let Some(f) = function(fact) {
                if callees(f).iter().any(|c| selected_names.contains(c)) {
                    added.insert(i, Reason::CalledBy);
                }
            }
        }
        if added.is_empty() {
            break;
        }
        why.extend(added);
    }

    let mut out: Vec<Selected<'a>> = why
        .into_iter()
        .map(|(i, reason)| Selected {
            fact: &facts[i],
            reason,
        })
        .collect();
    out.sort_by(|a, b| {
        (file_of(a.fact), line_of(a.fact), name_of(a.fact)).cmp(&(
            file_of(b.fact),
            line_of(b.fact),
            name_of(b.fact),
        ))
    });
    out
}

/// One fact as a line a model can read, with its prose marked (D10).
///
/// Short on purpose: a claim is checked against a handful of these, and a template that repeats the
/// source teaches a model to quote code instead of judging it.
pub fn render(fact: &Fact) -> String {
    match fact {
        Fact::Function(f) => {
            let mut s = format!(
                "fn {}({}) {} at {}:{}",
                f.label(),
                f.params,
                f.returns,
                f.file,
                f.line
            );
            if f.is_test() {
                s.push_str(" [test]");
            }
            if !f.cfg.is_empty() || !f.file_cfg.is_empty() {
                let cfg: Vec<&str> = f
                    .cfg
                    .iter()
                    .chain(f.file_cfg.iter())
                    .map(String::as_str)
                    .collect();
                s.push_str(&format!(" [{}]", cfg.join(" ")));
            }
            if !f.doc.is_empty() {
                s.push_str(&format!("\n  doc (prose): {}", first_line(&f.doc)));
            }
            for e in f.events.iter().filter(|e| !e.prose) {
                let detail: Vec<String> = e
                    .detail
                    .iter()
                    .filter(|(_, v)| !v.is_empty())
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                let ctx = if e.ctx.is_empty() {
                    String::new()
                } else {
                    format!(" under {}", e.ctx.join(" / "))
                };
                s.push_str(&format!(
                    "\n  {:?} {} at :{}{}",
                    e.kind,
                    detail.join(" "),
                    e.line,
                    ctx
                ));
            }
            for e in f.events.iter().filter(|e| e.prose) {
                if let Some((_, text)) = e.detail.first() {
                    s.push_str(&format!("\n  string (prose) at :{}: {text}", e.line));
                }
            }
            s
        }
        Fact::Const(Constant {
            file,
            name,
            keyword,
            ty,
            line,
            value,
            elements,
            prose,
            ..
        }) => {
            let mut s = format!("{keyword} {name}: {ty} = {value} at {file}:{line}");
            if let Some(e) = elements {
                s.push_str(&format!("\n  {} element(s)", e.len()));
            }
            if *prose {
                s.push_str("\n  (prose: a string constant never verifies a claim)");
            }
            s
        }
        Fact::Struct(Structure {
            file,
            name,
            line,
            fields,
            doc,
            ..
        }) => {
            let mut s = format!("struct {name} at {file}:{line}");
            for (n, t) in fields {
                s.push_str(&format!("\n  {n}: {t}"));
            }
            if !doc.is_empty() {
                s.push_str(&format!("\n  doc (prose): {}", first_line(doc)));
            }
            s
        }
        Fact::TypeAlias(t) => format!("type {} = {} at {}:{}", t.name, t.ty, t.file, t.line),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("").trim()
}

fn function(fact: &Fact) -> Option<&Function> {
    match fact {
        Fact::Function(f) => Some(f),
        _ => None,
    }
}

/// The names a function calls: the last path segment of each callee, and each method name.
fn callees(f: &Function) -> BTreeSet<String> {
    f.events
        .iter()
        .filter_map(|e| {
            let (key, value) = e.detail.first()?;
            match key.as_str() {
                "callee" => value.rsplit("::").next().map(|s| s.trim().to_string()),
                "method" => Some(value.clone()),
                _ => None,
            }
        })
        .collect()
}

fn file_of(fact: &Fact) -> &str {
    match fact {
        Fact::Function(f) => &f.file,
        Fact::Const(c) => &c.file,
        Fact::Struct(s) => &s.file,
        Fact::TypeAlias(t) => &t.file,
    }
}

fn line_of(fact: &Fact) -> usize {
    match fact {
        Fact::Function(f) => f.line,
        Fact::Const(c) => c.line,
        Fact::Struct(s) => s.line,
        Fact::TypeAlias(t) => t.line,
    }
}

fn name_of(fact: &Fact) -> &str {
    match fact {
        Fact::Function(f) => &f.name,
        Fact::Const(c) => &c.name,
        Fact::Struct(s) => &s.name,
        Fact::TypeAlias(t) => &t.name,
    }
}

fn label_of(fact: &Fact) -> String {
    match fact {
        Fact::Function(f) => f.label(),
        other => name_of(other).to_string(),
    }
}
