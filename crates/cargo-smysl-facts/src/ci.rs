//! What CI builds, so a `cfg` can be known to be exercised or not (D9).
//!
//! A fact behind `#[cfg(feature = "hosted")]` is only as true as the builds that compile it: a claim
//! checked against code no job ever builds rests on nothing. This reads the workflows for the cargo
//! commands they run and the feature flags they pass, then answers, for one `cfg`, whether every build
//! compiles it, some do, or none.
//!
//! **It reads words, not YAML semantics.** There is no YAML parser here — the shipped tool takes no
//! dependency it does not need — so this finds `cargo …` lines and the feature flags near them, and
//! matrix entries that look like flag lists. That is enough for the shapes CI files actually use, and
//! wrong for a workflow that assembles its flags elsewhere, so the answer says how many builds it found:
//! a caller that sees none knows not to trust "never built".

use std::collections::BTreeSet;
use std::path::Path;

/// One way CI builds the code.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Build {
    /// The cargo subcommand: `test`, `build`, `clippy`, `doc`.
    pub command: String,
    /// `--all-features`.
    pub all_features: bool,
    /// Default features are on unless `--no-default-features`.
    pub default_features: bool,
    /// Features named with `--features`.
    pub features: BTreeSet<String>,
}

impl Build {
    /// Whether this build compiles code gated on `feature`.
    ///
    /// `defaults` are the features the manifest turns on by default, which the workflow cannot say: the
    /// first version of this treated "default features on" as enabling anything, and then reported the
    /// `hosted` branch of this very tool as always built and its non-`hosted` branch as never built —
    /// backwards, since no job passes `--features hosted`.
    pub fn enables(&self, feature: &str, defaults: &BTreeSet<String>) -> bool {
        self.all_features
            || self.features.contains(feature)
            || (self.default_features && defaults.contains(feature))
    }

    pub fn is_test(&self) -> bool {
        self.command == "test"
    }
}

/// How well CI covers one `cfg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// Every build compiles it.
    Always,
    /// Some do: this many of `of`.
    Some { builds: usize, of: usize },
    /// No build does. A claim about this code rests on nothing CI checks.
    Never,
    /// No build was found to judge by.
    Unknown,
}

/// Every build the workflows in `dir` run.
pub fn builds(dir: impl AsRef<Path>) -> Vec<Build> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir.as_ref()) else {
        return out;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yml" || e == "yaml"))
        .collect();
    files.sort();
    for path in files {
        if let Ok(text) = std::fs::read_to_string(&path) {
            out.extend(builds_in(&text));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The builds one workflow file runs.
pub fn builds_in(text: &str) -> Vec<Build> {
    // Flag lists a matrix offers, e.g. `- "--no-default-features --features cli"`. A cargo line that
    // interpolates a matrix value stands for each of them.
    let matrix: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("- "))
        .map(|l| l.trim_matches(['"', '\''].as_ref()).to_string())
        // Any flag-shaped entry: `--all-features` does not contain the substring `--features`, which is
        // how the first version of this quietly lost a third of one matrix.
        .filter(|l| l.starts_with("--"))
        .collect();

    let mut out = Vec::new();
    for line in text.lines() {
        let Some(at) = line.find("cargo ") else {
            continue;
        };
        let rest = &line[at + "cargo ".len()..];
        let Some(command) = rest.split_whitespace().next() else {
            continue;
        };
        let command = command.trim_start_matches('+'); // a toolchain, not a command
        if !["test", "build", "clippy", "doc", "run", "install"].contains(&command) {
            continue;
        }
        let interpolates = rest.contains("${{");
        let variants: Vec<String> = if interpolates && !matrix.is_empty() {
            matrix.iter().map(|m| format!("{rest} {m}")).collect()
        } else {
            vec![rest.to_string()]
        };
        for text in variants {
            out.push(Build {
                command: command.to_string(),
                all_features: text.contains("--all-features"),
                default_features: !text.contains("--no-default-features"),
                features: named_features(&text),
            });
        }
    }
    out
}

fn named_features(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(at) = rest.find("--features") {
        rest = &rest[at + "--features".len()..];
        let list = rest
            .trim_start_matches([' ', '='])
            .split_whitespace()
            .next()
            .unwrap_or("");
        for f in list.split(',') {
            let f = f.trim_matches(['"', '\'', ','].as_ref());
            if !f.is_empty() && !f.starts_with("--") {
                out.insert(f.to_string());
            }
        }
    }
    out
}

/// How CI covers a `cfg` expression, as `syn` renders it (`cfg(feature = "x")`, `cfg(test)`,
/// `cfg(not(feature = "y"))`, `test`).
pub fn coverage(cfg: &[String], builds: &[Build], defaults: &BTreeSet<String>) -> Coverage {
    if cfg.is_empty() {
        return Coverage::Always;
    }
    if builds.is_empty() {
        return Coverage::Unknown;
    }
    let compiling = builds
        .iter()
        .filter(|b| cfg.iter().all(|c| holds(c, b, defaults)))
        .count();
    match compiling {
        0 => Coverage::Never,
        n if n == builds.len() => Coverage::Always,
        n => Coverage::Some {
            builds: n,
            of: builds.len(),
        },
    }
}

/// Whether one `cfg` predicate holds in a build. Unknown predicates are treated as holding: a fact this
/// cannot judge must not be reported as unbuilt.
fn holds(cfg: &str, build: &Build, defaults: &BTreeSet<String>) -> bool {
    let inner = cfg
        .trim()
        .strip_prefix("cfg")
        .map(|r| r.trim())
        .and_then(|r| r.strip_prefix('('))
        .and_then(|r| r.strip_suffix(')'))
        .unwrap_or(cfg.trim());
    predicate(inner.trim(), build, defaults)
}

fn predicate(p: &str, build: &Build, defaults: &BTreeSet<String>) -> bool {
    let p = p.trim();
    if let Some(rest) = p.strip_prefix("not") {
        let inner = rest.trim().trim_start_matches('(').trim_end_matches(')');
        return !predicate(inner, build, defaults);
    }
    for (word, all) in [("any", false), ("all", true)] {
        if let Some(rest) = p.strip_prefix(word) {
            let inner = rest.trim().trim_start_matches('(').trim_end_matches(')');
            let parts = split_top(inner);
            return if all {
                parts.iter().all(|x| predicate(x, build, defaults))
            } else {
                parts.iter().any(|x| predicate(x, build, defaults))
            };
        }
    }
    if p == "test" {
        return build.is_test();
    }
    if let Some(rest) = p.strip_prefix("feature") {
        let name = rest
            .trim()
            .trim_start_matches('=')
            .trim()
            .trim_matches(['"', '\''].as_ref());
        return build.enables(name, defaults);
    }
    true
}

/// Split `a, b(c, d)` at top-level commas.
fn split_top(text: &str) -> Vec<String> {
    let (mut out, mut depth, mut current) = (Vec::new(), 0usize, String::new());
    for c in text.chars() {
        match c {
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}
