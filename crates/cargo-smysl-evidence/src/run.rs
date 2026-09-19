//! Running the tests and recording what they did (D13).
//!
//! The reading is the tool's, not a model's: `cargo test` is run with `--locked` and `--exact`, and what
//! comes back — passed, failed, ignored, and how long it took — becomes a `measured` unit through
//! smysl's `from_csv`. A test that was never run has no reading; an ignored test has one saying so,
//! because "ignored" is a fact about the evidence and silence is not.
//!
//! Self-contained: the only process started is the `cargo` that invoked this tool (`$CARGO`).

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("running cargo: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("{0}")]
    Smysl(String),
}

/// What one test did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    /// The test's path as libtest prints it, e.g. `scope::a_touched_file_brings_its_items`.
    pub test: String,
    /// `passed`, `failed` or `ignored`.
    pub outcome: String,
    /// Seconds the whole run took, not this test: libtest reports per-suite time.
    pub run_seconds: f64,
    /// The commit the tests were run at.
    pub commit: String,
    /// The toolchain, because a reading is only about the machine that made it.
    pub toolchain: String,
}

impl Reading {
    pub fn passed(&self) -> bool {
        self.outcome == "passed"
    }
}

/// How to run: which package and target, and which tests by exact name.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub package: Option<String>,
    /// `--test <name>` for an integration target, or none for the library's own tests.
    pub target: Option<String>,
    /// Exact test paths. Empty runs the whole target, which is what a first pass wants.
    pub tests: Vec<String>,
    /// Features the tests need, from the `cfg` the facts recorded.
    pub features: Vec<String>,
}

impl Plan {
    /// The arguments, in the order a person could paste into a terminal.
    pub fn args(&self) -> Vec<String> {
        let mut out = vec![
            "test".to_string(),
            "--locked".into(),
            "--no-fail-fast".into(),
        ];
        if let Some(p) = &self.package {
            out.push("-p".into());
            out.push(p.clone());
        }
        if let Some(t) = &self.target {
            out.push("--test".into());
            out.push(t.clone());
        }
        if !self.features.is_empty() {
            out.push("--features".into());
            out.push(self.features.join(","));
        }
        if !self.tests.is_empty() {
            out.push("--".into());
            out.push("--exact".into());
            out.extend(self.tests.clone());
        }
        out
    }
}

/// Run the plan and read what libtest said.
pub fn run(root: &Path, plan: &Plan, commit: &str) -> Result<Vec<Reading>, RunError> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(&cargo)
        .args(plan.args())
        .current_dir(root)
        .output()
        .map_err(RunError::Spawn)?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(readings(&text, commit, &toolchain(&cargo)))
}

fn toolchain(cargo: &str) -> String {
    Command::new(cargo)
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Parse libtest's output. A line reads `test path::name ... ok`, `FAILED`, or `ignored`.
pub fn readings(output: &str, commit: &str, toolchain: &str) -> Vec<Reading> {
    let mut seconds = 0.0;
    let mut out: Vec<Reading> = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("test result:") {
            // `test result: ok. 6 passed; … finished in 0.02s`
            if let Some(t) = rest.split("finished in ").nth(1) {
                seconds = t.trim_end_matches('s').trim().parse().unwrap_or(0.0);
            }
            // The suite's time belongs to the tests it just reported.
            for r in out.iter_mut().filter(|r| r.run_seconds == 0.0) {
                r.run_seconds = seconds;
            }
            continue;
        }
        let Some(rest) = line.strip_prefix("test ") else {
            continue;
        };
        let Some((name, verdict)) = rest.split_once(" ... ") else {
            continue;
        };
        let outcome = match verdict.trim() {
            "ok" => "passed",
            v if v.starts_with("FAILED") => "failed",
            v if v.starts_with("ignored") => "ignored",
            _ => continue,
        };
        // A name with a space is libtest's summary line, not a test.
        if name.contains(' ') {
            continue;
        }
        out.push(Reading {
            test: name.trim().to_string(),
            outcome: outcome.to_string(),
            run_seconds: 0.0,
            commit: commit.to_string(),
            toolchain: toolchain.to_string(),
        });
    }
    out
}

/// The readings as smysl's import sees them: one row per test, the test's name as the key column.
pub fn to_csv(readings: &[Reading]) -> String {
    let mut out = String::from("test,commit,outcome,run_seconds,toolchain\n");
    for r in readings {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            escape(&r.test),
            escape(&r.commit),
            r.outcome,
            r.run_seconds,
            escape(&r.toolchain)
        ));
    }
    out
}

fn escape(field: &str) -> String {
    field.replace(',', ";")
}

/// Import the readings as `measured` units, attested to the tool that ran them.
///
/// Every reading traces to `tool:smysl-import` through smysl's own import attestation: the licence to
/// call a unit `measured` is the attestation, not the caller's say-so.
pub fn import(readings: &[Reading], source: &str) -> Result<smysl::Imported, RunError> {
    let agent =
        smysl::AgentId::new("tool:smysl-import").map_err(|e| RunError::Smysl(e.to_string()))?;
    let mut opts = smysl::ImportOptions::new(source, agent.clone(), smysl::Hlc::zero(agent));
    opts.key = vec!["test".to_string(), "commit".to_string()];
    Ok(smysl::from_csv(&to_csv(readings), &opts))
}
