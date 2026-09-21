//! The tool must work as an external cargo subcommand, so these tests go through real `cargo`,
//! with the built binary first on PATH, as `cargo install` would leave it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_cargo-smysl");

fn workspace_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml")
}

/// `cargo <args>` with the directory of the built `cargo-smysl` first on PATH.
fn cargo(args: &[&str]) -> Output {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let bin_dir = Path::new(BIN).parent().expect("binary has a directory");
    let path = std::env::join_paths(std::iter::once(bin_dir.to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("PATH joins");
    Command::new(cargo)
        .args(args)
        .env("PATH", path)
        .output()
        .expect("cargo runs")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn cargo_finds_and_runs_the_subcommand() {
    let manifest = workspace_manifest();
    let out = cargo(&[
        "smysl",
        "doctor",
        "--manifest-path",
        manifest.to_str().unwrap(),
    ]);
    let all = text(&out);
    assert!(out.status.success(), "{all}");
    assert!(all.contains("invoked by cargo: "), "{all}");
    assert!(
        !all.contains("$CARGO unset"),
        "cargo must set $CARGO for the subcommand:\n{all}"
    );
    // The linked smysl, whatever version the pin resolves to, not a hardcoded one.
    assert!(
        all.contains(&format!("smysl library: {}", smysl::VERSION)),
        "{all}"
    );
    assert!(all.contains(" 0 parse failure(s)"), "{all}");
}

#[test]
fn cargo_help_reaches_the_subcommand() {
    let out = cargo(&["smysl", "--help"]);
    let all = text(&out);
    assert!(out.status.success(), "{all}");
    for command in [
        "doctor", "facts", "extract", "why", "check", "evidence", "stale", "review",
    ] {
        assert!(
            all.contains(command),
            "help does not list `{command}`:\n{all}"
        );
    }
}

#[test]
fn the_binary_also_runs_directly() {
    let direct = Command::new(BIN).arg("--version").output().unwrap();
    let as_cargo = Command::new(BIN)
        .args(["smysl", "--version"])
        .output()
        .unwrap();
    assert!(direct.status.success() && as_cargo.status.success());
    assert_eq!(direct.stdout, as_cargo.stdout);
    assert!(String::from_utf8_lossy(&direct.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn every_command_the_help_lists_is_implemented() {
    // Nothing answers "not implemented yet" any more: `stale` was the last, and a command that exits
    // 3 has been added to the help without being written.
    let help = cargo(&["smysl", "--help"]);
    let listed: Vec<String> = text(&help)
        .lines()
        .skip_while(|l| !l.starts_with("Commands:"))
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .filter(|w| w.chars().all(|c| c.is_ascii_lowercase()) && w != "help")
        .collect();
    assert!(listed.len() >= 7, "the help lists the commands: {listed:?}");
    for command in listed {
        let out = cargo(&["smysl", &command, "--help"]);
        assert_ne!(
            out.status.code(),
            Some(3),
            "{command} is listed but not implemented"
        );
    }
}

/// `why` answers from the corpus beside the workspace, and says so when there is none.
/// The corpus is built here the way the tool builds it, so the test exercises the shipped path.
#[test]
fn why_reports_what_rests_on_a_prerequisite() {
    use cargo_smysl_corpus::store::Corpus;
    use cargo_smysl_corpus::{build, stage, CommitText, Extraction};

    let root = std::env::temp_dir().join(format!("cargo-smysl-why-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"why-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "").unwrap();

    let manifest = root.join("Cargo.toml");
    let out = cargo(&[
        "smysl",
        "why",
        "p/gdeadbeef0000-1-1",
        "--manifest-path",
        manifest.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "no corpus is a failure, not silence:\n{}",
        text(&out)
    );
    assert!(
        text(&out).contains("nothing has been recorded yet"),
        "{}",
        text(&out)
    );

    let ex: Extraction = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../eval/extractions/research-pro-v2/smysl/90ec2f7.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let sha = "90ec2f781421002876548124e9fe02073503372c";
    let batch = build(
        &ex,
        &CommitText {
            touched: Vec::new(),
            sha,
            message: "",
            files: vec![],
        },
        0,
    )
    .unwrap();
    let staged = stage(&smysl::Store::from_records(Vec::new()), batch, 0);
    Corpus::at(&root).record(sha, &staged).unwrap();

    let out = cargo(&[
        "smysl",
        "why",
        "p/g90ec2f781421-8-1",
        "--manifest-path",
        manifest.to_str().unwrap(),
    ]);
    let all = text(&out);
    assert!(out.status.success(), "{all}");
    assert!(all.contains("rest on it"), "{all}");
    assert!(
        all.contains("d/g90ec2f781421-8"),
        "the decision it conditions:\n{all}"
    );

    let out = cargo(&[
        "smysl",
        "why",
        "p/nosuch-1-1",
        "--manifest-path",
        manifest.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "an unbound label is an error:\n{}",
        text(&out)
    );
    std::fs::remove_dir_all(&root).ok();
}

/// The merge driver git calls for `*.smy`: two branches that recorded different commits merge without
/// anyone choosing, because records are append-only and smysl's merge is order-independent.
#[test]
fn two_corpus_documents_merge_without_a_conflict() {
    use cargo_smysl_corpus::store::surface;
    use cargo_smysl_corpus::{build, stage, CommitText, Extraction};
    use smysl::Store;

    let document = |sha: &str, decision: &str| -> String {
        let ex: Extraction = serde_json::from_value(serde_json::json!({
            "decisions": [{"decision": decision, "kind": "act", "rationale": "", "quote": ""}],
            "prerequisites": [], "alternatives": [], "consequences": []
        }))
        .unwrap();
        let commit = CommitText {
            touched: Vec::new(),
            sha,
            message: decision,
            files: vec![],
        };
        let batch = build(&ex, &commit, 0).unwrap();
        surface(&stage(&Store::from_records(Vec::new()), batch, 0))
    };

    let dir = std::env::temp_dir().join(format!("smysl-merge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = dir.join("base.smy");
    let ours = dir.join("ours.smy");
    let theirs = dir.join("theirs.smy");
    std::fs::write(&base, "").unwrap();
    std::fs::write(&ours, document("aaaaaaaaaaaa", "Pin the dependency")).unwrap();
    std::fs::write(&theirs, document("bbbbbbbbbbbb", "Split the crate")).unwrap();

    let out = cargo(&[
        "smysl",
        "merge-driver",
        base.to_str().unwrap(),
        ours.to_str().unwrap(),
        theirs.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));

    // Both branches' reasoning is in the file git will keep.
    let merged = std::fs::read_to_string(&ours).unwrap();
    assert!(
        merged.contains("Pin the dependency"),
        "ours survives: {merged}"
    );
    assert!(
        merged.contains("Split the crate"),
        "theirs arrives: {merged}"
    );
    assert!(
        smysl::parse_surface(&merged).is_ok(),
        "and the result is a document, not a conflict marker"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A document that does not parse is left to a person rather than half-merged.
#[test]
fn a_document_that_does_not_parse_is_left_alone() {
    let dir = std::env::temp_dir().join(format!("smysl-merge-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (base, ours, theirs) = (dir.join("b"), dir.join("o"), dir.join("t"));
    std::fs::write(&base, "").unwrap();
    std::fs::write(&ours, "this is not a smysl document").unwrap();
    std::fs::write(&theirs, "nor is this").unwrap();

    let out = cargo(&[
        "smysl",
        "merge-driver",
        base.to_str().unwrap(),
        ours.to_str().unwrap(),
        theirs.to_str().unwrap(),
    ]);
    assert_ne!(out.status.code(), Some(0), "git is told the merge failed");
    assert_eq!(
        std::fs::read_to_string(&ours).unwrap(),
        "this is not a smysl document",
        "and our file is untouched"
    );
    std::fs::remove_dir_all(&dir).ok();
}
