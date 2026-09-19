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
fn an_unimplemented_command_says_where_it_is_planned() {
    // `why` is implemented (Phase 1); `stale` is not, and says where it is planned.
    let out = cargo(&["smysl", "stale"]);
    assert_eq!(out.status.code(), Some(3), "{}", text(&out));
    assert!(text(&out).contains("docs/implementation-plan.md"));
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
