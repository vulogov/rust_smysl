use std::path::Path;

use cargo_smysl_corpus::query::dependents_of;
use cargo_smysl_corpus::store::Corpus;
use cargo_smysl_verdict::{Change, Provider, ProviderJudge, Settings};

use crate::cli::{Command, SmyslArgs};
use crate::exit;

pub fn run(args: SmyslArgs) -> u8 {
    match &args.command {
        Command::Doctor => doctor(&args),
        Command::Facts { .. } => not_yet("facts", "Phase 2 — facts and extraction"),
        Command::Extract { .. } => not_yet("extract", "Phase 2 — facts and extraction"),
        Command::Why { item } => why(&args, item),
        Command::Check {
            rev,
            patch,
            strict,
            json,
            provider,
            model,
            endpoint,
            key_var,
            window,
            chars_per_token,
            prompt_file,
        } => check(
            &args,
            CheckArgs {
                rev: rev.as_deref(),
                patch: patch.as_deref(),
                strict: *strict,
                json: *json,
                provider,
                model,
                endpoint,
                key_var,
                window: *window,
                chars_per_token: *chars_per_token,
                prompt_file: prompt_file.as_deref(),
            },
        ),
        Command::Evidence { .. } => not_yet("evidence", "Phase 3 — verdicts and test evidence"),
        Command::Stale { .. } => not_yet("stale", "a later phase (item 5, staleness)"),
        Command::Review => not_yet("review", "Phase 3 — verdicts and test evidence"),
    }
}

/// `why <label>`: what rests on a recorded unit — the decisions it conditions and what those cause.
///
/// The corpus is read from `.smysl` beside the workspace root. A label that names no unit, or more than
/// one, is an error rather than an empty answer: silence would read as "nothing depends on this".
fn why(args: &SmyslArgs, item: &str) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl why: {e}");
            return exit::FAILURE;
        }
    };
    let corpus = Corpus::at(&root);
    let store = match corpus.load() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("cargo smysl why: {e}");
            return exit::FAILURE;
        }
    };
    if store.units().count() == 0 {
        eprintln!(
            "cargo smysl why: no corpus at {} — nothing has been recorded yet",
            corpus.dir().display()
        );
        return exit::FAILURE;
    }
    match dependents_of(&store, item) {
        Err(e) => {
            eprintln!("cargo smysl why: {e}");
            exit::FAILURE
        }
        Ok(dependents) if dependents.is_empty() => {
            println!("{item}: nothing recorded rests on it");
            exit::OK
        }
        Ok(dependents) => {
            println!("{item}: {} unit(s) rest on it", dependents.len());
            for d in dependents {
                let name = d
                    .labels
                    .first()
                    .map(|l| l.as_str().to_string())
                    .unwrap_or_else(|| d.uid.short());
                println!("  {name}  {} ({})\n      {}", d.schema, d.status, d.gist);
            }
            exit::OK
        }
    }
}

struct CheckArgs<'a> {
    rev: Option<&'a str>,
    patch: Option<&'a str>,
    strict: bool,
    json: bool,
    provider: &'a str,
    model: &'a str,
    endpoint: &'a str,
    key_var: &'a str,
    window: u32,
    chars_per_token: f32,
    prompt_file: Option<&'a Path>,
}

/// `check`: what this change contradicts in the corpus.
///
/// Advisory (D18): findings are printed and the exit code stays 0 unless `--strict`, because the
/// measured precision does not support blocking by default (S4).
fn check(args: &SmyslArgs, c: CheckArgs<'_>) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl check: {e}");
            return exit::FAILURE;
        }
    };
    let store = match Corpus::at(&root).load() {
        Ok(s) if s.units().count() > 0 => s,
        Ok(_) => {
            eprintln!("cargo smysl check: no corpus recorded yet; nothing to check against");
            return exit::FAILURE;
        }
        Err(e) => {
            eprintln!("cargo smysl check: {e}");
            return exit::FAILURE;
        }
    };
    let diff = match read_change(&root, c.rev, c.patch) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("cargo smysl check: {e}");
            return exit::FAILURE;
        }
    };
    let mut settings = Settings {
        window: c.window,
        chars_per_token: c.chars_per_token,
        ..Settings::default()
    };
    if let Some(path) = c.prompt_file {
        match std::fs::read_to_string(path) {
            Ok(text) => settings.system = Some(text),
            Err(e) => {
                eprintln!("cargo smysl check: {}: {e}", path.display());
                return exit::FAILURE;
            }
        }
    }
    let judge = ProviderJudge {
        provider: Provider {
            kind: c.provider.to_string(),
            endpoint: c.endpoint.to_string(),
            model: c.model.to_string(),
            key_var: c.key_var.to_string(),
            window: c.window,
            answer_tokens: settings.answer_tokens,
            ..Provider::local(c.model)
        },
    };
    let change = Change::from_diff(&diff, &settings);
    let outcome = cargo_smysl_verdict::check(&store, &change, &judge, &settings);

    if c.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&outcome).unwrap_or_default()
        );
    } else {
        for w in &outcome.warnings {
            eprintln!("cargo smysl check: {w}");
        }
        if outcome.findings.is_empty() {
            println!(
                "nothing recorded is contradicted ({} unit(s) judged, {})",
                outcome.units_judged, outcome.judge
            );
        } else {
            println!(
                "{} finding(s) against {} unit(s) judged, {}:\n",
                outcome.findings.len(),
                outcome.units_judged,
                outcome.judge
            );
            for f in &outcome.findings {
                println!("{} {}  ({})", f.label, f.kind, f.source);
                for line in f.text.lines() {
                    println!("    {line}");
                }
                println!("  at: {}", f.diff_line);
                println!("  because: {}\n", f.reason);
            }
            if !c.strict {
                println!(
                    "advisory: `check` reports, it does not block. `--strict` exits 5 on findings."
                );
            }
        }
    }
    outcome.exit_code(c.strict)
}

/// The change to check: a commit, a patch file, or stdin.
fn read_change(root: &Path, rev: Option<&str>, patch: Option<&str>) -> Result<String, String> {
    if let Some(p) = patch {
        if p == "-" {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                .map_err(|e| format!("stdin: {e}"))?;
            return Ok(text);
        }
        return std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    }
    let commit = cargo_smysl_git::read_commit(root, rev.unwrap_or("HEAD"))
        .map_err(|e| format!("reading {}: {e}", rev.unwrap_or("HEAD")))?;
    let mut out = String::new();
    for file in &commit.files {
        out.push_str(&cargo_smysl_verdict::check::unified(
            &file.path,
            file.before.as_deref().unwrap_or(""),
            file.after.as_deref().unwrap_or(""),
            3,
        ));
    }
    Ok(out)
}

/// The workspace root, the way cargo sees it.
fn workspace_root(args: &SmyslArgs) -> Result<std::path::PathBuf, String> {
    let mut cmd = cargo_metadata::MetadataCommand::new();
    if let Some(path) = &args.manifest_path {
        cmd.manifest_path(path);
    }
    let metadata = cmd.no_deps().exec().map_err(|e| e.to_string())?;
    Ok(metadata.workspace_root.into_std_path_buf())
}

fn not_yet(name: &str, phase: &str) -> u8 {
    eprintln!(
        "cargo smysl {name}: not implemented yet; planned in {phase} (docs/implementation-plan.md)"
    );
    exit::NOT_IMPLEMENTED
}

/// What the tool sees, with no model and no network: proves the subcommand runs under cargo, finds
/// the workspace the way cargo does, links smysl, and parses the workspace's Rust with syn.
fn doctor(args: &SmyslArgs) -> u8 {
    println!("cargo-smysl: {}", env!("CARGO_PKG_VERSION"));
    println!(
        "smysl library: {} (formats {})",
        smysl::VERSION,
        smysl::FORMAT_VERSIONS_SUPPORTED.join(", ")
    );
    println!("code schema: {}", cargo_smysl_corpus::CODE_SCHEMA_ID);
    match std::env::var_os("CARGO") {
        Some(cargo) => println!("invoked by cargo: {}", Path::new(&cargo).display()),
        None => println!("invoked by cargo: no ($CARGO unset; run as `cargo smysl`)"),
    }

    let mut cmd = cargo_metadata::MetadataCommand::new();
    cmd.no_deps();
    if let Some(path) = &args.manifest_path {
        cmd.manifest_path(path);
    }
    let metadata = match cmd.exec() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("cargo smysl doctor: cannot read the workspace: {e}");
            return exit::FAILURE;
        }
    };
    let root = metadata.workspace_root.as_std_path();
    println!("workspace: {}", root.display());
    println!("members: {}", metadata.workspace_members.len());

    let corpus = root.join(cargo_smysl_corpus::CORPUS_DIR);
    println!(
        "corpus: {} ({})",
        corpus.display(),
        if corpus.is_dir() { "present" } else { "absent" }
    );

    let mut files = 0usize;
    let mut fns = 0usize;
    let mut failed = Vec::new();
    for package in metadata.workspace_packages() {
        let dir = package
            .manifest_path
            .parent()
            .map(|p| p.as_std_path().to_path_buf());
        for file in dir.into_iter().flat_map(|d| rust_files(&d)) {
            let Ok(src) = std::fs::read_to_string(&file) else {
                continue;
            };
            files += 1;
            match cargo_smysl_facts::functions(&src) {
                Ok(found) => fns += found.len(),
                Err(e) => failed.push(format!("{}: {e}", file.display())),
            }
        }
    }
    println!(
        "facts: {files} Rust file(s), {fns} function(s), {} parse failure(s)",
        failed.len()
    );
    for f in failed.iter().take(5) {
        println!("  {f}");
    }
    if failed.is_empty() {
        exit::OK
    } else {
        exit::FAILURE
    }
}

/// Rust files under a package directory, skipping `target` and hidden directories.
fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if path.is_dir() {
                if name != "target" && !name.starts_with('.') {
                    stack.push(path);
                }
            } else if name.ends_with(".rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
