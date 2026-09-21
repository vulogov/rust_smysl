//! Reads commits from this repository's own history. CI checks out full history for these tests; in a
//! shallow clone without the commits they report and return rather than fail.

use std::path::{Path, PathBuf};

use cargo_smysl_git::{read_commit, GitError};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn commit(rev: &str) -> Option<cargo_smysl_git::CommitData> {
    match read_commit(repo(), rev) {
        Ok(c) => Some(c),
        Err(GitError::Revision { .. }) => {
            eprintln!("{rev} is not in this clone (shallow?); skipping");
            None
        }
        Err(e) => panic!("{rev}: {e}"),
    }
}

#[test]
fn the_scaffold_commit_adds_its_files() {
    let Some(c) = commit("d69fced") else { return };
    assert!(c.sha.starts_with("d69fced"));
    assert!(
        c.message.starts_with("Scaffold cargo-smysl"),
        "{}",
        c.message
    );
    assert!(!c.parent_missing);
    assert_eq!(
        c.files.len(),
        21,
        "{:?}",
        c.files.iter().map(|f| &f.path).collect::<Vec<_>>()
    );
    let main = c
        .files
        .iter()
        .find(|f| f.path == "crates/cargo-smysl/src/main.rs")
        .expect("main.rs changed");
    assert!(main.before.is_none(), "added in this commit");
    assert!(main.after.as_deref().unwrap().contains("fn normalise_args"));
}

#[test]
fn the_msrv_commit_modifies_two_files_with_both_sides() {
    let Some(c) = commit("28064aa") else { return };
    let paths: Vec<&str> = c.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["Cargo.lock", "Cargo.toml"]);
    let toml = &c.files[1];
    assert!(toml.before.as_deref().unwrap().contains("resolver = \"2\""));
    assert!(toml.after.as_deref().unwrap().contains("resolver = \"3\""));
    assert!(toml.text().contains("resolver = \"2\"") && toml.text().contains("resolver = \"3\""));
}

#[test]
fn head_reads_in_any_clone() {
    let c = read_commit(repo(), "HEAD").expect("HEAD is always present");
    assert_eq!(c.sha.len(), 40);
    assert!(!c.message.trim().is_empty());
}

#[test]
fn an_unknown_revision_is_a_revision_error() {
    assert!(matches!(
        read_commit(repo(), "no-such-branch-xyz"),
        Err(GitError::Revision { .. })
    ));
}

/// A range names what the newer revision reaches and the older one does not, newest first.
#[test]
fn a_range_is_what_the_newer_revision_reaches_and_the_older_does_not() {
    use cargo_smysl_git::commits_between;
    let repo = repo();
    let Ok(all) = commits_between(&repo, None, "HEAD", 5) else {
        eprintln!("history not in this clone (shallow?); skipping");
        return;
    };
    if all.len() < 5 {
        eprintln!("shallow clone; skipping");
        return;
    }

    let from_third = commits_between(&repo, Some("HEAD~3"), "HEAD", 0).unwrap();
    assert_eq!(from_third.len(), 3, "HEAD~3..HEAD is three commits");
    assert_eq!(
        from_third,
        all[..3].to_vec(),
        "and they are the newest three, newest first"
    );

    let none = commits_between(&repo, Some("HEAD"), "HEAD", 0).unwrap();
    assert!(none.is_empty(), "a range that reaches nothing new is empty");
    assert!(
        commits_between(&repo, Some("no-such-rev"), "HEAD", 0).is_err(),
        "an unknown revision is an error, not silence"
    );
}
