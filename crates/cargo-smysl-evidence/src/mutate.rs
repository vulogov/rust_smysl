//! The opt-in mutation gate (S1).
//!
//! A `backs` edge says the test would fail if the claim were false. This tries to make that false in a
//! small way — change the code the claim is about — and asks whether the test notices.
//!
//! **In-process, and deliberately small.** S1 ruled out depending on an external mutation binary (the
//! tool is self-contained), and it measured what a mutation gate is worth: it kept only 2 of 5 valid
//! links, so it cannot be allowed to reject an edge on its own. What it did do reliably is catch the
//! vacuous test — one that noticed *nothing*. So the gate refuses a `backs` edge only when the test
//! missed every viable mutant, and otherwise reports the score and leaves the edge to a person (D15).
//!
//! **The mutations are the type-preserving ones**, applied inside the function's own lines: a
//! comparison flipped, a boolean literal flipped, `&&` for `||`, `+` for `-`. A mutation that does not
//! compile is *unviable* — it says nothing about the test and is not counted. A mutation in a comment or
//! a string is not a mutation of the code, so lines are read for that first.
//!
//! **The file is restored.** The original bytes are written to `.smysl/mutation-backup/` before the file
//! is touched and restored when the run ends, including when the test run panics; a backup found at the
//! start of a later run is restored first, so an interrupted gate cannot leave mutated source behind.

use std::path::{Path, PathBuf};

use cargo_smysl_facts::item::Function;

use crate::run::{run_capturing, Plan, Reading, RunError};

/// One change to the code under test.
#[derive(Debug, Clone, PartialEq)]
pub struct Mutant {
    pub file: String,
    /// 1-based line in the file.
    pub line: usize,
    /// What the line was, and what it became: a report a person can check by eye.
    pub before: String,
    pub after: String,
    pub operator: String,
}

/// What the gate found for one test and one claim.
#[derive(Debug, Clone, Default)]
pub struct Score {
    /// Mutants the test failed on: it noticed.
    pub caught: Vec<Mutant>,
    /// Mutants it passed on: it did not.
    pub missed: Vec<Mutant>,
    /// Mutants that did not compile, which say nothing either way.
    pub unviable: Vec<Mutant>,
    /// Why nothing ran, for each unviable mutant: a mutant the compiler refused and a workspace that
    /// would not build at all look the same from here, and only the first says something about the code.
    pub notes: Vec<String>,
}

impl Score {
    pub fn viable(&self) -> usize {
        self.caught.len() + self.missed.len()
    }

    /// Whether a `backs` edge may be proposed: the test noticed at least one change it was given.
    ///
    /// A test that noticed nothing is the case S1 measured this gate on. A test that noticed some but
    /// not all is insensitive, not vacuous, and S1 measured that rejecting those costs more valid edges
    /// than it saves — so it passes, with its score reported.
    pub fn backs(&self) -> bool {
        self.viable() == 0 || !self.caught.is_empty()
    }

    /// The sentence a report and a review note use.
    pub fn because(&self) -> String {
        if self.viable() == 0 {
            return format!(
                "no viable mutation of this code compiled ({} tried), so the gate says nothing",
                self.unviable.len()
            );
        }
        if self.caught.is_empty() {
            return format!(
                "the test passed under all {} mutation(s) of the code it is said to verify: it does \
                 not notice this code changing",
                self.missed.len()
            );
        }
        format!(
            "the test failed under {} of {} mutation(s) of this code",
            self.caught.len(),
            self.viable()
        )
    }
}

/// The mutations worth trying inside one function, best first, at most `limit`.
///
/// "Best" is the plainest: a flipped comparison changes what the code decides, which is what a test of
/// that code should see. Lines that are comments, attributes or macro strings are left alone.
pub fn mutants(target: &Function, source: &str, limit: usize) -> Vec<Mutant> {
    const OPERATORS: [(&str, &str); 7] = [
        (" == ", " != "),
        (" != ", " == "),
        (" < ", " >= "),
        (" > ", " <= "),
        (" && ", " || "),
        ("true", "false"),
        (" + ", " - "),
    ];
    let mut out = Vec::new();
    for (n, line) in source.lines().enumerate() {
        let number = n + 1;
        if number < target.line || number > target.end {
            continue;
        }
        if skip(line) {
            continue;
        }
        for (from, to) in OPERATORS {
            if !line.contains(from) {
                continue;
            }
            out.push(Mutant {
                file: target.file.clone(),
                line: number,
                before: line.to_string(),
                after: line.replacen(from, to, 1),
                operator: format!("{} to {}", from.trim(), to.trim()),
            });
            break;
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// A line no mutation should touch: what it says is not what the code does.
fn skip(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//")
        || t.starts_with("#[")
        || t.starts_with("#!")
        || t.starts_with('*')
        || t.contains("\"")
}

/// Run the gate: mutate, run the tests, restore, once per mutant.
///
/// `plan` is the test that claims to verify the code. A mutant is *caught* when that test stops passing
/// — including when the build fails for a reason the compiler reports as an error in the test itself,
/// which is why an unviable mutant is told apart by nothing having run at all.
pub fn gate(
    root: &Path,
    target: &Function,
    plan: &Plan,
    commit: &str,
    limit: usize,
) -> Result<Score, RunError> {
    let path = root.join(&target.file);
    let source = std::fs::read_to_string(&path).map_err(RunError::Spawn)?;
    let mut score = Score::default();
    for mutant in mutants(target, &source, limit) {
        let guard = Guard::hold(root, &target.file, &path, &source)?;
        let mutated = replace_line(&source, mutant.line, &mutant.after);
        std::fs::write(&path, &mutated).map_err(RunError::Spawn)?;
        let ran = run_capturing(root, plan, commit);
        drop(guard);
        match ran {
            // Nothing ran. Usually the mutant did not compile, which says nothing about the test — but
            // it can also be the workspace refusing to build for its own reasons, so the first error
            // cargo printed is kept and shown.
            Ok((r, output)) if r.is_empty() => {
                score.notes.push(format!(
                    "{}:{} — nothing ran: {}",
                    mutant.file,
                    mutant.line,
                    first_error(&output)
                ));
                score.unviable.push(mutant);
            }
            Ok((r, _)) if r.iter().all(Reading::passed) => score.missed.push(mutant),
            Ok(_) => score.caught.push(mutant),
            Err(e) => return Err(e),
        }
    }
    Ok(score)
}

/// The first line cargo called an error, which is what a person needs to see.
fn first_error(output: &str) -> String {
    output
        .lines()
        .find(|l| l.trim_start().starts_with("error"))
        .unwrap_or("cargo printed no error")
        .trim()
        .chars()
        .take(160)
        .collect()
}

fn replace_line(source: &str, line: usize, with: &str) -> String {
    let mut out: Vec<&str> = source.lines().collect();
    if let Some(slot) = out.get_mut(line.saturating_sub(1)) {
        *slot = with;
    }
    let mut text = out.join("\n");
    if source.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// Holds the original file while a mutant is in place, and puts it back whatever happens.
struct Guard {
    path: PathBuf,
    backup: PathBuf,
}

impl Guard {
    fn hold(root: &Path, relative: &str, path: &Path, source: &str) -> Result<Guard, RunError> {
        let dir = root.join(".smysl").join("mutation-backup");
        std::fs::create_dir_all(&dir).map_err(RunError::Spawn)?;
        // The whole path is the backup's name, so a recovery knows where the file belongs.
        let backup = dir.join(format!("{}.orig", relative.replace('/', "%")));
        std::fs::write(&backup, source).map_err(RunError::Spawn)?;
        Ok(Guard {
            path: path.to_path_buf(),
            backup,
        })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(original) = std::fs::read(&self.backup) {
            let _ = std::fs::write(&self.path, original);
            let _ = std::fs::remove_file(&self.backup);
        }
    }
}

/// The path under `root` this name asks for, or nothing when it asks for somewhere else.
///
/// Refused by inspection rather than by canonicalising, because the target may not exist yet and a
/// symbolic link inside the workspace can still point out of it: only plain, relative, climbing-free
/// paths are restored.
fn within(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() {
        return None;
    }
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        return None;
    }
    for part in candidate.components() {
        match part {
            std::path::Component::Normal(_) => {}
            // `..`, a prefix such as `C:`, or a root: none of these belong in a backup's name.
            _ => return None,
        }
    }
    let target = root.join(candidate);
    // And the file it would overwrite must not be a link out of the workspace.
    if std::fs::symlink_metadata(&target)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return None;
    }
    Some(target)
}

/// Put back anything an interrupted run left mutated. Called before the gate starts.
///
/// A backup still here means a previous run did not finish, and the file it names holds a mutant.
/// Restoring is always right: the backup is the source as the tool found it, and the tool is the only
/// thing that wrote the mutant.
pub fn recover(root: &Path) -> Vec<String> {
    let dir = root.join(".smysl").join("mutation-backup");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let backup = entry.path();
        let Some(name) = backup.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let relative = name.trim_end_matches(".orig").replace('%', "/");
        // A backup's name decides where its bytes are written, and `.smysl/` travels with a repository:
        // a file called `..%..%.zshrc.orig`, committed by someone else, would otherwise be restored
        // outside this workspace. A path that is absolute, or that climbs, is refused by name.
        let Some(target) = within(root, &relative) else {
            out.push(format!(
                "{} names a path outside this workspace and was not restored; delete it if it is not \
                 yours",
                backup.display()
            ));
            continue;
        };
        match std::fs::read(&backup).and_then(|bytes| std::fs::write(&target, bytes)) {
            Ok(()) => {
                let _ = std::fs::remove_file(&backup);
                out.push(format!(
                    "an interrupted mutation run had left {relative} mutated; it is restored"
                ));
            }
            Err(e) => out.push(format!(
                "{relative} was left mutated and could not be restored ({e}): the original is in {}",
                backup.display()
            )),
        }
    }
    out
}
