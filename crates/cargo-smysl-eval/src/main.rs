//! `smysl-eval`: templates, adjudication and scoring for the S0 evaluation set (see eval/README.md).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_smysl_eval::{
    adjudication, extracted_items, score, Adjudication, Commit, Commits, Kind, LabelFile, Tally,
};
use clap::{Parser, Subcommand};

mod s3;
mod s4;

#[derive(Parser)]
#[command(
    name = "smysl-eval",
    version,
    about = "S0 evaluation set: templates, adjudication, scoring"
)]
struct Cli {
    /// The eval directory.
    #[arg(long, default_value_os_t = default_eval_dir())]
    eval_dir: PathBuf,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write a label template for every commit that has no label file yet.
    Templates,
    /// S0 labelling order, and a reading sheet (message and diff) beside each unfinished label file.
    Worklist {
        /// Only the commits the research already extracted, which can be scored the moment they are
        /// labelled.
        #[arg(long)]
        studied: bool,
        /// Lines of one file's diff on the sheet before it is cut.
        #[arg(long, default_value_t = 400)]
        file_lines: usize,
    },
    /// Write or refresh adjudication files for a system's extractions of labelled commits.
    Adjudicate { system: String },
    /// Score a system: precision and recall per kind, per repository, and overall.
    Score { system: String },
    /// Build and stage every extraction of a system against its commit's real text (read with gix),
    /// and report units, quote support and staging errors. Exits non-zero if any batch has errors.
    Stage { system: String },
    /// S3 corpus arm: build each repository's corpus from the registered system's extractions (commits
    /// that are ancestors of the task base) and write a packed context per task and per question to
    /// eval/s3/context/.
    S3Context,
    /// Extract the labelled commits with the shipped extraction code, as a system S0 can score.
    ///
    /// The same `cargo_smysl_extract::extract`, the same prompt and the same recipe the command uses;
    /// what is left out is only the workspace discovery, which S0 does not measure. The clones here are
    /// bare, so this reads them directly.
    ExtractShipped {
        /// Where the extractions go: eval/extractions/SYSTEM.
        #[arg(long, default_value = "shipped-local14")]
        system: String,
        /// Only these commits (short sha).
        #[arg(long)]
        sha: Vec<String>,
        /// Re-extract a commit that already has a file.
        #[arg(long)]
        force: bool,
        #[arg(long, env = "SMYSL_CHECK_PROVIDER", default_value = "ollama")]
        provider: String,
        #[arg(long, env = "SMYSL_CHECK_MODEL", default_value = "qwen2.5-coder:14b")]
        model: String,
        #[arg(
            long,
            env = "SMYSL_CHECK_ENDPOINT",
            default_value = "http://localhost:11434/api/chat"
        )]
        endpoint: String,
        #[arg(long, env = "SMYSL_CHECK_KEY_VAR", default_value = "")]
        key_var: String,
        #[arg(long, env = "SMYSL_CHECK_WINDOW", default_value_t = 32768)]
        window: u32,
    },
    /// What the commits say about the quotes in an extraction: the quote checks, on results already
    /// on disk. No model call — the checks are the tool's, not a judgement.
    CheckQuotes {
        /// Which extractions to check, by system name under eval/extractions.
        #[arg(long)]
        system: String,
    },
    /// Export S4 cases as trees the shipped `cargo smysl check` can read: a `.smysl/` corpus and the
    /// change, one directory per case. The reproduction of the held-out figures runs against these.
    S4Export {
        /// dev or heldout.
        #[arg(long)]
        set: String,
        /// Where the case directories go.
        #[arg(long)]
        to: PathBuf,
        /// Only these case ids.
        #[arg(long)]
        id: Vec<String>,
    },
    /// S4 detector (eval/s4-protocol.md): check each diff of a set against the corpus of its base's
    /// ancestors, writing eval/s4/results/RUN/ID.json. Existing results are kept (resume).
    S4Check(Box<S4Args>),
}

#[derive(clap::Args)]
struct S4Args {
    /// dev or heldout.
    #[arg(long)]
    set: String,
    /// Results go to eval/s4/results/RUN.
    #[arg(long)]
    run: String,
    /// Only these case ids.
    #[arg(long)]
    id: Vec<String>,
    #[arg(long, default_value_t = 6)]
    jobs: usize,
    /// Steps 1, 2 and the fingerprint only: no model call (the determinism check).
    #[arg(long)]
    no_model: bool,
    #[arg(long, default_value_t = 12)]
    candidates: usize,
    #[arg(long, default_value_t = 3000)]
    budget: u64,
    #[arg(long, default_value_t = 400)]
    diff_lines: usize,
    #[arg(long, default_value_t = 150)]
    file_lines: usize,
    #[arg(long, default_value_t = 80)]
    query_terms: usize,
    /// Model name, or $SMYSL_CHECK_MODEL.
    #[arg(
        long,
        env = "SMYSL_CHECK_MODEL",
        default_value = "Qwen2.5-Coder:7B-Instruct"
    )]
    model: String,
    /// `ollama` or `openai` (any OpenAI-compatible endpoint), or $SMYSL_CHECK_PROVIDER.
    #[arg(long, env = "SMYSL_CHECK_PROVIDER", default_value = "ollama")]
    provider: String,
    /// Chat endpoint, or $SMYSL_CHECK_ENDPOINT.
    #[arg(
        long,
        env = "SMYSL_CHECK_ENDPOINT",
        default_value = "http://localhost:11434/api/chat"
    )]
    endpoint: String,
    /// Environment variable holding the provider's key, or $SMYSL_CHECK_KEY_VAR.
    #[arg(long, env = "SMYSL_CHECK_KEY_VAR", default_value = "DEEPSEEK_API_KEY")]
    key_var: String,
    /// Context window asked of the provider, where it takes one.
    #[arg(long, env = "SMYSL_CHECK_NUM_CTX", default_value_t = 16384)]
    num_ctx: u32,
    /// Units judged per call, an upper bound; 0 lets the context limit decide.
    #[arg(long, env = "SMYSL_CHECK_CHUNK", default_value_t = 6)]
    chunk: usize,
    /// Tokens the model takes in one request; defaults to --num-ctx for a local model.
    #[arg(long, env = "SMYSL_CHECK_CONTEXT_LIMIT")]
    context_limit: Option<u32>,
    /// Tokens left for the answer when fitting a call to the context limit.
    #[arg(long, env = "SMYSL_CHECK_RESERVE_OUTPUT", default_value_t = 2048)]
    reserve_output: u32,
    /// Most units judged for one diff; 0 judges every one in the pack.
    #[arg(long, env = "SMYSL_CHECK_JUDGE_LIMIT", default_value_t = 0)]
    judge_limit: usize,
    /// Characters per token for the chosen model's tokenizer (measured: Qwen2.5-Coder charges ~2.05 on diffs; prose ≈ 4).
    #[arg(long, env = "SMYSL_CHECK_CHARS_PER_TOKEN", default_value_t = 2.0)]
    chars_per_token: f32,
    /// Smallest term contribution for a retrieved unit to count (smysl 1.6 R22); 0 keeps every hit.
    #[arg(long, env = "SMYSL_CHECK_MIN_TERM_WEIGHT", default_value_t = 0.0)]
    min_term_weight: f32,
    /// Rotate the order units are grouped in, so a second run groups them differently.
    #[arg(long, default_value_t = 0)]
    order_seed: usize,
    /// A file holding the system prompt, or $SMYSL_CHECK_PROMPT_FILE; the built-in default otherwise.
    #[arg(long, env = "SMYSL_CHECK_PROMPT_FILE")]
    prompt_file: Option<PathBuf>,
}

fn default_eval_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval")
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Cmd::Templates => templates(&cli.eval_dir),
        Cmd::Worklist {
            studied,
            file_lines,
        } => worklist(&cli.eval_dir, *studied, *file_lines),
        Cmd::Adjudicate { system } => adjudicate(&cli.eval_dir, system),
        Cmd::Score { system } => score_system(&cli.eval_dir, system),
        Cmd::Stage { system } => stage_system(&cli.eval_dir, system),
        Cmd::S3Context => s3_context(&cli.eval_dir),
        Cmd::S4Check(args) => s4_check(&cli.eval_dir, args),
        Cmd::S4Export { set, to, id } => s4_export(&cli.eval_dir, set, to, id),
        Cmd::CheckQuotes { system } => check_quotes(&cli.eval_dir, system),
        Cmd::ExtractShipped {
            system,
            sha,
            force,
            provider,
            model,
            endpoint,
            key_var,
            window,
        } => extract_shipped(
            &cli.eval_dir,
            system,
            sha,
            *force,
            cargo_smysl_extract::Provider {
                kind: provider.clone(),
                endpoint: endpoint.clone(),
                model: model.clone(),
                key_var: key_var.clone(),
                window: *window,
                ..cargo_smysl_extract::Provider::local(model)
            },
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("smysl-eval: {e}");
            ExitCode::FAILURE
        }
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn commits(eval: &Path) -> Result<Commits, String> {
    toml::from_str(&read(&eval.join("commits.toml"))?).map_err(|e| format!("commits.toml: {e}"))
}

fn label_path(eval: &Path, c: &Commit) -> PathBuf {
    eval.join("labels")
        .join(&c.repo)
        .join(format!("{}.toml", c.sha))
}

fn git(repo_dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A full clone under eval/.repos, created on first use. Full, not blobless: commit text is read with
/// gix (cargo-smysl-git), which reads only objects that are present. A blobless clone left by an older
/// version of this tool is replaced.
fn repo_dir(eval: &Path, name: &str, url: &str) -> Result<PathBuf, String> {
    let dir = eval.join(".repos").join(name);
    let exists = dir.join(".git").exists() || dir.join("HEAD").exists();
    if exists {
        let partial = git(
            &dir,
            &["config", "--get", "remote.origin.partialclonefilter"],
        )
        .is_ok();
        if !partial {
            return Ok(dir);
        }
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(dir.parent().unwrap()).map_err(|e| e.to_string())?;
    let status = Command::new("git")
        .args(["clone", "--quiet", "--no-checkout", url])
        .arg(&dir)
        .status()
        .map_err(|e| format!("git clone: {e}"))?;
    if !status.success() {
        return Err(format!("git clone {url} failed"));
    }
    Ok(dir)
}

fn templates(eval: &Path) -> Result<(), String> {
    let set = commits(eval)?;
    let (mut written, mut kept) = (0, 0);
    for c in &set.commits {
        let path = label_path(eval, c);
        if path.exists() {
            kept += 1;
            continue;
        }
        let repo = set
            .repos
            .get(&c.repo)
            .ok_or_else(|| format!("unknown repo {}", c.repo))?;
        let dir = repo_dir(eval, &c.repo, &repo.url)?;
        let full = git(&dir, &["rev-parse", &format!("{}^{{commit}}", c.sha)])?
            .trim()
            .to_string();
        let message = git(&dir, &["show", "-s", "--format=%B", &full])?;
        let stat = git(&dir, &["show", "--stat=100", "--format=", &full])?;
        let comment = |text: &str| {
            text.trim_end()
                .lines()
                .map(|l| format!("#   {l}\n"))
                .collect::<String>()
        };
        let body = format!(
            "# S0 label — {repo} {short}\n\
             # Why this commit is in the set: {why}\n\
             # Label from the message AND the diff: git -C eval/.repos/{repo} show {full}\n\
             # Definitions and boundary cases: eval/README.md. Label blind: do not open extractions first.\n\
             #\n\
             # Message:\n{message}#\n# Files:\n{stat}\n\
             schema = 1\n\
             repo = \"{repo}\"\n\
             commit = \"{full}\"\n\
             labeller = \"\"\n\
             status = \"todo\"   # set to \"done\" when finished\n\n\
             # Copy a block per item. Ids must be unique; prerequisites and alternatives name a decision.\n\
             #\n\
             # [[decision]]\n# id = \"D1\"\n# text = \"\"\n# kind = \"act\"          # act | decline\n# evidence = \"\"       # optional verbatim span\n#\n\
             # [[prerequisite]]\n# id = \"P1\"\n# decision = \"D1\"\n# text = \"\"\n\
             # kind = \"existing-behaviour\"   # existing-behaviour | invariant | tool-setting | prior-change | assumption\n\
             # normative = false\n# evidence = \"\"\n#\n\
             # [[alternative]]\n# id = \"A1\"\n# decision = \"D1\"\n# text = \"\"\n# evidence = \"\"\n",
            repo = c.repo,
            short = c.sha,
            why = c.why,
            message = comment(&message),
            stat = comment(&stat),
        );
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, body).map_err(|e| e.to_string())?;
        written += 1;
    }
    println!("templates: {written} written, {kept} already present");
    Ok(())
}

/// The labelling order, and the text to label from.
///
/// Labelling is the owner's time, and the cost of it is switching windows: the message in one place, the
/// diff in another, the definitions in a third. So each unfinished commit gets a `<sha>.diff` sheet
/// beside its label file holding the message and the diff as one document, and the order puts the
/// studied commits first — those already have extractions, so each one labelled turns into a score.
///
/// The sheets are reading material, not labels: they are regenerated freely, and they say nothing about
/// any extraction, because a label written after reading a model's answer measures agreement, not
/// quality.
fn worklist(eval: &Path, studied_only: bool, file_lines: usize) -> Result<(), String> {
    let set = commits(eval)?;
    let systems = extraction_systems(eval);
    let mut rows: Vec<(bool, &Commit, bool, usize)> = Vec::new();
    for c in &set.commits {
        let is_studied = c.stratum == "studied";
        if studied_only && !is_studied {
            continue;
        }
        let done = label_is_done(eval, c);
        let extracted = systems
            .iter()
            .filter(|sys| {
                eval.join("extractions")
                    .join(sys)
                    .join(&c.repo)
                    .join(format!("{}.json", c.sha))
                    .exists()
            })
            .count();
        rows.push((is_studied, c, done, extracted));
    }
    // Studied first, then the ones with the most extractions waiting on them.
    rows.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(b.3.cmp(&a.3))
            .then(a.1.sha.cmp(&b.1.sha))
    });

    let (mut written, mut done_count) = (0, 0);
    for (is_studied, c, done, extracted) in &rows {
        if *done {
            done_count += 1;
            println!("done     {:>6} {:<6}", c.sha, c.repo);
            continue;
        }
        let repo = set
            .repos
            .get(&c.repo)
            .ok_or_else(|| format!("unknown repo {}", c.repo))?;
        let dir = repo_dir(eval, &c.repo, &repo.url)?;
        let full = git(&dir, &["rev-parse", &format!("{}^{{commit}}", c.sha)])?
            .trim()
            .to_string();
        let sheet = sheet_text(&dir, &full, c, file_lines)?;
        let path = label_path(eval, c).with_extension("diff");
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, sheet).map_err(|e| e.to_string())?;
        written += 1;
        println!(
            "todo     {:>6} {:<6} {:<8} {} extraction(s) waiting  ->  {}",
            c.sha,
            c.repo,
            if *is_studied { "studied" } else { "new" },
            extracted,
            path.display()
        );
    }
    println!(
        "\n{done_count} labelled, {written} sheet(s) written. Label the file beside each sheet \
         (`<sha>.toml`), set status = \"done\", then: cargo run -p cargo-smysl-eval -- score <system>"
    );
    Ok(())
}

/// One commit as a single document: why it is in the set, its message, then its diff, each file cut to
/// `file_lines` with a note saying what was cut. A cut file is named, so nothing disappears silently.
fn sheet_text(dir: &Path, full: &str, c: &Commit, file_lines: usize) -> Result<String, String> {
    let message = git(dir, &["show", "-s", "--format=%B", full])?;
    let stat = git(dir, &["show", "--stat=100", "--format=", full])?;
    let diff = git(dir, &["show", "--format=", "--unified=3", full])?;
    let mut body = String::new();
    for chunk in diff.split("\ndiff --git ") {
        let chunk = if body.is_empty() {
            chunk.to_string()
        } else {
            format!("diff --git {chunk}")
        };
        let lines: Vec<&str> = chunk.lines().collect();
        if lines.len() > file_lines {
            let head = lines[..file_lines].join("\n");
            body.push_str(&format!(
                "{head}\n… {} more line(s) of this file not shown; read them with \
                 `git -C eval/.repos/{} show {full}`\n\n",
                lines.len() - file_lines,
                c.repo
            ));
        } else {
            body.push_str(&chunk);
            body.push('\n');
        }
    }
    Ok(format!(
        "S0 reading sheet — {repo} {sha}\n\
         Why this commit is in the set: {why}\n\
         Label in {sha}.toml. Definitions and boundary cases: eval/README.md.\n\
         Label blind: do not open eval/extractions/ for this commit first.\n\
         \n=== message ===\n\n{message}\n=== files ===\n\n{stat}\n=== diff ===\n\n{body}",
        repo = c.repo,
        sha = c.sha,
        why = c.why,
    ))
}

/// Which systems have extractions at all, so the worklist can say what a label would score.
fn extraction_systems(eval: &Path) -> Vec<String> {
    let Ok(dir) = std::fs::read_dir(eval.join("extractions")) else {
        return Vec::new();
    };
    let mut out: Vec<String> = dir
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

fn label_is_done(eval: &Path, c: &Commit) -> bool {
    std::fs::read_to_string(label_path(eval, c))
        .ok()
        .and_then(|t| toml::from_str::<toml::Value>(&t).ok())
        .and_then(|v| v.get("status").and_then(|s| s.as_str().map(str::to_string)))
        .is_some_and(|s| s == "done")
}

fn done_labels(eval: &Path) -> Result<Vec<(Commit, Option<LabelFile>)>, String> {
    let set = commits(eval)?;
    let mut out = Vec::new();
    for c in set.commits {
        let path = label_path(eval, &c);
        let label = if path.exists() {
            let l: LabelFile =
                toml::from_str(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
            if l.is_done() {
                l.validate()
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                Some(l)
            } else {
                None
            }
        } else {
            None
        };
        out.push((c, label));
    }
    Ok(out)
}

fn extraction_path(eval: &Path, system: &str, c: &Commit) -> PathBuf {
    eval.join("extractions")
        .join(system)
        .join(&c.repo)
        .join(format!("{}.json", c.sha))
}

fn adjudication_path(eval: &Path, system: &str, c: &Commit) -> PathBuf {
    eval.join("adjudications")
        .join(system)
        .join(&c.repo)
        .join(format!("{}.toml", c.sha))
}

fn adjudicate(eval: &Path, system: &str) -> Result<(), String> {
    let (mut written, mut unlabelled, mut unextracted) = (0, 0, 0);
    for (c, label) in done_labels(eval)? {
        let Some(label) = label else {
            unlabelled += 1;
            continue;
        };
        let ex = extraction_path(eval, system, &c);
        if !ex.exists() {
            unextracted += 1;
            continue;
        }
        let json: serde_json::Value =
            serde_json::from_str(&read(&ex)?).map_err(|e| format!("{}: {e}", ex.display()))?;
        let path = adjudication_path(eval, system, &c);
        let previous: Option<Adjudication> = if path.exists() {
            Some(toml::from_str(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?)
        } else {
            None
        };
        let adj = adjudication(system, &label, &extracted_items(&json), previous.as_ref());
        let text = toml::to_string_pretty(&adj).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(
            &path,
            format!("# Set `match` for each item: a label id of the same kind, or \"none\". `suggest` is word overlap only.\n\n{text}"),
        )
        .map_err(|e| e.to_string())?;
        written += 1;
    }
    println!("adjudications: {written} written; {unlabelled} commit(s) not labelled yet; {unextracted} labelled without an extraction");
    Ok(())
}

fn pct(x: Option<f64>) -> String {
    x.map(|v| format!("{:5.1}%", v * 100.0))
        .unwrap_or_else(|| "    —".into())
}

fn score_system(eval: &Path, system: &str) -> Result<(), String> {
    let mut by_repo: BTreeMap<String, BTreeMap<Kind, Tally>> = BTreeMap::new();
    let mut overall: BTreeMap<Kind, Tally> = BTreeMap::new();
    let (mut scored, mut skipped) = (0, 0);
    for (c, label) in done_labels(eval)? {
        let path = adjudication_path(eval, system, &c);
        let (Some(label), true) = (label, path.exists()) else {
            skipped += 1;
            continue;
        };
        let adj: Adjudication =
            toml::from_str(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        for (kind, t) in score(&label, &adj)? {
            by_repo
                .entry(c.repo.clone())
                .or_default()
                .entry(kind)
                .or_default()
                .add(t);
            overall.entry(kind).or_default().add(t);
        }
        scored += 1;
    }
    println!(
        "system {system}: {scored} commit(s) scored, {skipped} without labels or adjudication\n"
    );
    println!(
        "{:<8} {:<13} {:>9} {:>7} {:>9} {:>7} {:>8} {:>10}",
        "repo", "kind", "extracted", "labels", "precision", "recall", "pending", ""
    );
    let mut rows: Vec<(String, &BTreeMap<Kind, Tally>)> =
        by_repo.iter().map(|(r, t)| (r.clone(), t)).collect();
    rows.push(("ALL".into(), &overall));
    for (repo, tallies) in rows {
        for (kind, t) in tallies {
            println!(
                "{:<8} {:<13} {:>9} {:>7} {:>9} {:>7} {:>8}",
                repo,
                format!("{kind:?}").to_lowercase(),
                t.extracted,
                t.labels,
                pct(t.precision()),
                pct(t.recall()),
                t.pending
            );
        }
    }
    Ok(())
}

fn stage_system(eval: &Path, system: &str) -> Result<(), String> {
    let set = commits(eval)?;
    let mut failures = 0;
    let mut totals = (0usize, 0usize, 0usize, 0usize, 0usize);
    println!(
        "{:<6} {:<9} {:>5} {:>5} {:>7} {:>5} {:>6} {:>7} {:>6}",
        "repo", "commit", "units", "edges", "present", "loose", "absent", "dropped", "errors"
    );
    for c in &set.commits {
        let ex_path = extraction_path(eval, system, c);
        if !ex_path.exists() {
            continue;
        }
        let repo = set
            .repos
            .get(&c.repo)
            .ok_or_else(|| format!("unknown repo {}", c.repo))?;
        let dir = repo_dir(eval, &c.repo, &repo.url)?;
        let data = cargo_smysl_git::read_commit(&dir, &c.sha)
            .map_err(|e| format!("{} {}: {e}", c.repo, c.sha))?;
        let texts: Vec<(String, String)> = data
            .files
            .iter()
            .map(|f| (f.path.clone(), f.text()))
            .collect();
        let commit = cargo_smysl_corpus::CommitText {
            sha: &data.sha,
            message: &data.message,
            files: texts
                .iter()
                .map(|(p, t)| (p.as_str(), t.as_str()))
                .collect(),
        };
        let ex: cargo_smysl_corpus::Extraction = serde_json::from_str(&read(&ex_path)?)
            .map_err(|e| format!("{}: {e}", ex_path.display()))?;
        let batch = cargo_smysl_corpus::build(&ex, &commit, 0)
            .map_err(|e| format!("{} {}: {e}", c.repo, c.sha))?;
        let (units, edges, quotes, dropped) = (
            batch.units.len(),
            batch.relations.len(),
            batch.quotes,
            batch.dropped.len(),
        );
        let staged = cargo_smysl_corpus::stage(&smysl::Store::from_records(Vec::new()), batch, 0);
        let errors: Vec<String> = staged
            .report
            .iter()
            .filter(|d| d.severity == smysl::Severity::Error)
            .map(|d| d.to_string())
            .collect();
        println!(
            "{:<6} {:<9} {:>5} {:>5} {:>7} {:>5} {:>6} {:>7} {:>6}",
            c.repo,
            c.sha,
            units,
            edges,
            quotes.present,
            quotes.loose,
            quotes.absent,
            dropped,
            errors.len()
        );
        for e in errors.iter().take(3) {
            println!("    {e}");
        }
        failures += usize::from(!errors.is_empty());
        totals.0 += units;
        totals.1 += quotes.present;
        totals.2 += quotes.loose;
        totals.3 += quotes.absent;
        totals.4 += errors.len();
    }
    println!(
        "\nsystem {system}: {} units; quotes present {}, loose {}, absent {}; {} staging error(s)",
        totals.0, totals.1, totals.2, totals.3, totals.4
    );
    if failures > 0 {
        return Err(format!("{failures} commit(s) staged with errors"));
    }
    Ok(())
}

/// A commit's extraction built against its real text.
fn commit_batch(
    dir: &Path,
    sha: &str,
    ex: &cargo_smysl_corpus::Extraction,
) -> Result<cargo_smysl_corpus::Batch, String> {
    let data = cargo_smysl_git::read_commit(dir, sha).map_err(|e| format!("{sha}: {e}"))?;
    let texts: Vec<(String, String)> = data
        .files
        .iter()
        .map(|f| (f.path.clone(), f.text()))
        .collect();
    let commit = cargo_smysl_corpus::CommitText {
        sha: &data.sha,
        message: &data.message,
        files: texts
            .iter()
            .map(|(p, t)| (p.as_str(), t.as_str()))
            .collect(),
    };
    cargo_smysl_corpus::build(ex, &commit, 0).map_err(|e| format!("{sha}: {e}"))
}

/// An extracted commit before the task base: sha, clone, extraction, and the paths its diff touches.
type Extracted = (String, PathBuf, cargo_smysl_corpus::Extraction, Vec<String>);

fn s3_context(eval: &Path) -> Result<(), String> {
    let reg = s3::read_registry(&eval.join("s3/tasks.toml"))?;
    let set = commits(eval)?;
    // Per repository: each extracted commit before the base, with the paths its diff touches.
    let mut history: BTreeMap<String, Vec<Extracted>> = BTreeMap::new();
    for (name, run) in &reg.repos {
        let repo = set
            .repos
            .get(name)
            .ok_or_else(|| format!("unknown repo {name}"))?;
        let dir = repo_dir(eval, name, &repo.url)?;
        let entry = history.entry(name.clone()).or_default();
        for c in set.commits.iter().filter(|c| &c.repo == name) {
            let ex_path = extraction_path(eval, &reg.system, c);
            if !ex_path.exists() {
                continue;
            }
            // Only history the agent's working tree already has: a commit after the base did not happen.
            if git(&dir, &["merge-base", "--is-ancestor", &c.sha, &run.base]).is_err() {
                println!("{name} {}: not an ancestor of the base, skipped", c.sha);
                continue;
            }
            let ex: cargo_smysl_corpus::Extraction = serde_json::from_str(&read(&ex_path)?)
                .map_err(|e| format!("{}: {e}", ex_path.display()))?;
            let paths = cargo_smysl_git::read_commit(&dir, &c.sha)
                .map_err(|e| format!("{}: {e}", c.sha))?
                .files
                .into_iter()
                .map(|f| f.path)
                .collect();
            entry.push((c.sha.clone(), dir.clone(), ex, paths));
        }
    }
    // A corpus of the commits in `scope`, or all of the repository's when `scope` is None.
    let corpus_of = |repo: &str, scope: Option<&BTreeSet<String>>| -> Result<s3::Corpus, String> {
        let mut corpus = s3::Corpus::new();
        for (sha, dir, ex, _) in history.get(repo).into_iter().flatten() {
            if scope.is_none_or(|s| s.contains(sha)) {
                corpus.add(sha, commit_batch(dir, sha, ex)?)?;
            }
        }
        Ok(corpus)
    };
    let whole: BTreeMap<&String, s3::Corpus> = reg
        .repos
        .keys()
        .map(|r| corpus_of(r, None).map(|c| (r, c)))
        .collect::<Result<_, _>>()?;

    let out = eval.join("s3/context");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let jobs = reg
        .tasks
        .iter()
        .map(|t| {
            (
                format!("task-{}", t.n),
                &t.repo,
                &t.statement,
                t.files.clone(),
            )
        })
        .chain(
            reg.questions
                .iter()
                .map(|q| (format!("question-{}", q.n), &q.repo, &q.text, Vec::new())),
        );
    println!(
        "{:<12} {:>7} {:>5} {:>5} {:>5} {:>6}  commits",
        "context", "corpus", "focus", "units", "L1+", "tokens"
    );
    for (name, repo, query, files) in jobs {
        // A task's scope: the commits that touched its files, plus the commits whose units the statement
        // retrieves across the whole repository (rationale for code is often recorded where its tests
        // changed). Questions name no files and search the whole repository.
        let all = whole
            .get(repo)
            .ok_or_else(|| format!("{name}: no corpus for {repo}"))?;
        let touched = history
            .get(repo)
            .into_iter()
            .flatten()
            .any(|(_, _, _, paths)| paths.iter().any(|p| files.contains(p)));
        // No extracted commit touched the files: the whole repository is the scope.
        let corpus = if !touched {
            None
        } else {
            let mut scope = all.commits_retrieved(query, 6);
            scope.extend(
                history
                    .get(repo)
                    .into_iter()
                    .flatten()
                    .filter(|(_, _, _, paths)| paths.iter().any(|p| files.contains(p)))
                    .map(|(sha, ..)| sha.clone()),
            );
            Some(corpus_of(repo, Some(&scope)).map_err(|e| format!("{name}: {e}"))?)
        };
        let corpus = corpus.as_ref().unwrap_or(all);
        let p = s3::pack(corpus, query, &files, reg.budget).map_err(|e| format!("{name}: {e}"))?;
        std::fs::write(out.join(format!("{name}.txt")), &p.text).map_err(|e| e.to_string())?;
        println!(
            "{:<12} {:>7} {:>5} {:>5} {:>5} {:>6}  {}",
            name,
            corpus.store.units().count(),
            p.focus,
            p.units,
            p.detailed,
            p.used,
            corpus.commits.join(" ")
        );
    }
    Ok(())
}

#[derive(serde::Deserialize, Clone)]
struct S4Case {
    set: String,
    id: String,
    repo: String,
    kind: String,
    base: String,
    source: String,
    task: Option<u32>,
    contradicting: Option<bool>,
}

/// Apply the shipped quote checks to extractions already made, and count what they would change.
///
/// Deterministic and free: the checks read the commit, which the tool already has. Re-extracting to
/// learn this would spend an hour of a local model to produce the same answer about the same answers.
fn check_quotes(eval: &Path, system: &str) -> Result<(), String> {
    use cargo_smysl_extract::{check_quote, Source, SourceFile, Support};
    let set = commits(eval)?;
    let framing = [
        cargo_smysl_extract::pass_decisions_system(),
        "Report at most",
    ];
    println!(
        "{:<16}{:>7}{:>7}{:>9}{:>7}{:>8}{:>8}{:>10}",
        "commit", "kept", "added", "incommit", "loose", "absent", "prompt", "prose-only"
    );
    let (mut total, mut t_added, mut t_in, mut t_loose, mut t_absent, mut t_prompt, mut t_prose) =
        (0, 0, 0, 0, 0, 0, 0);
    for c in &set.commits {
        let path = eval
            .join("extractions")
            .join(system)
            .join(&c.repo)
            .join(format!("{}.json", c.sha));
        if !path.exists() {
            continue;
        }
        let extraction: cargo_smysl_corpus::Extraction =
            serde_json::from_str(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        let repo = set
            .repos
            .get(&c.repo)
            .ok_or_else(|| format!("unknown repo {}", c.repo))?;
        let dir = repo_dir(eval, &c.repo, &repo.url)?;
        let commit = cargo_smysl_git::read_commit(&dir, &c.sha).map_err(|e| e.to_string())?;
        let source = Source {
            message: commit.message.clone(),
            files: commit
                .files
                .iter()
                .map(|f| SourceFile {
                    path: f.path.clone(),
                    before: f.before.clone().unwrap_or_default(),
                    after: f.after.clone().unwrap_or_default(),
                })
                .collect(),
        };
        let (mut added, mut incommit, mut loose, mut absent, mut prompt, mut prose) =
            (0, 0, 0, 0, 0, 0);
        for d in &extraction.decisions {
            let checked = check_quote(&d.quote, &source, &framing);
            match checked.support {
                Support::Added => added += 1,
                Support::InCommit => incommit += 1,
                Support::Loose => loose += 1,
                Support::Absent => absent += 1,
                Support::Prompt => prompt += 1,
            }
            if checked.prose_only {
                prose += 1;
            }
        }
        println!(
            "{:<16}{:>7}{:>7}{:>9}{:>7}{:>8}{:>8}{:>10}",
            format!("{}/{}", c.repo, c.sha),
            extraction.decisions.len(),
            added,
            incommit,
            loose,
            absent,
            prompt,
            prose
        );
        total += extraction.decisions.len();
        t_added += added;
        t_in += incommit;
        t_loose += loose;
        t_absent += absent;
        t_prompt += prompt;
        t_prose += prose;
    }
    println!(
        "{:<16}{total:>7}{t_added:>7}{t_in:>9}{t_loose:>7}{t_absent:>8}{t_prompt:>8}{t_prose:>10}",
        "ALL"
    );
    println!(
        "\nrank_by_quote keeps {} of {total} ahead of the {} the commit does not bear out.",
        t_added + t_in + t_loose,
        t_absent + t_prompt
    );
    println!(
        "drop_prompt_quotes would drop {t_prompt}; drop_prose_only_quotes would drop {t_prose}."
    );
    Ok(())
}

/// Run the shipped extraction over every labelled commit, writing what S0 scores.
fn extract_shipped(
    eval: &Path,
    system: &str,
    shas: &[String],
    force: bool,
    provider: cargo_smysl_extract::Provider,
) -> Result<(), String> {
    let set = commits(eval)?;
    use cargo_smysl_extract::Judge as _;
    let judge = cargo_smysl_extract::ProviderJudge { provider };
    let recipe = cargo_smysl_extract::Recipe::default();
    let (mut done, mut kept) = (0, 0);
    let mut failed: Vec<String> = Vec::new();
    for (c, label) in done_labels(eval)? {
        if label.is_none() || (!shas.is_empty() && !shas.contains(&c.sha)) {
            continue;
        }
        let out = eval
            .join("extractions")
            .join(system)
            .join(&c.repo)
            .join(format!("{}.json", c.sha));
        if out.exists() && !force {
            kept += 1;
            continue;
        }
        let repo = set
            .repos
            .get(&c.repo)
            .ok_or_else(|| format!("unknown repo {}", c.repo))?;
        let dir = repo_dir(eval, &c.repo, &repo.url)?;
        let commit = cargo_smysl_git::read_commit(&dir, &c.sha).map_err(|e| e.to_string())?;
        let source = cargo_smysl_extract::Source {
            message: commit.message.clone(),
            files: commit
                .files
                .iter()
                .map(|f| cargo_smysl_extract::SourceFile {
                    path: f.path.clone(),
                    before: f.before.clone().unwrap_or_default(),
                    after: f.after.clone().unwrap_or_default(),
                })
                .collect(),
        };
        let began = std::time::Instant::now();
        // One commit failing is not the run failing: the rest are still worth having, and the failure
        // is named so it can be redone on its own.
        let (extraction, report) = match cargo_smysl_extract::extract(&source, &judge, &recipe) {
            Ok(x) => x,
            Err(e) => {
                println!("{:<6} {:<8} FAILED: {e}", c.repo, c.sha);
                failed.push(format!("{} {}", c.repo, c.sha));
                continue;
            }
        };
        for w in &report.warnings {
            println!("  {} {}: {w}", c.repo, c.sha);
        }
        std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(
            &out,
            serde_json::to_string_pretty(&extraction).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        // Beside every extraction, what produced it. Two runs of different builds once landed in one
        // directory and the result had to be thrown away for want of this; a result whose provenance is
        // recorded can always be sorted out later, and is never wasted.
        let made_by = out.with_extension("run.json");
        std::fs::write(
            &made_by,
            serde_json::to_string_pretty(&serde_json::json!({
                "tool_commit": git(&eval.join(".."), &["rev-parse", "HEAD"]).unwrap_or_default().trim(),
                "judge": judge.describe(),
                "recipe": recipe.name,
                "max_input": recipe.max_input,
                "max_decisions": recipe.max_decisions,
                "calls": report.calls,
                "warnings": report.warnings,
                "seconds": began.elapsed().as_secs(),
                "at": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            }))
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!(
            "{:<6} {:<8} {} decision(s), {} prerequisite(s), {} alternative(s); {} call(s), {}s",
            c.repo,
            c.sha,
            extraction.decisions.len(),
            extraction.prerequisites.len(),
            extraction.alternatives.len(),
            report.calls,
            began.elapsed().as_secs()
        );
        done += 1;
    }
    println!(
        "{system}: {done} extracted, {kept} already present, {} failed",
        failed.len()
    );
    if !failed.is_empty() {
        println!("  redo with: smysl-eval extract-shipped --system {system} --sha <sha>");
        for f in &failed {
            println!("  {f}");
        }
    }
    Ok(())
}

/// The corpora and diffs a set of S4 cases is checked against: one corpus per distinct set of ancestor
/// commits, and the change each case carries (a commit's diff, or a written patch).
///
/// Shared by the harness and the export, so a run of the shipped binary is measured against exactly the
/// corpus the harness used, and a difference in the figures is a difference in `check`.
struct S4Corpora {
    corpora: BTreeMap<(String, Vec<String>), s3::Corpus>,
    key_of: BTreeMap<String, (String, Vec<String>)>,
    texts: BTreeMap<String, String>,
}

fn s4_corpora(eval: &Path, cases: &[S4Case]) -> Result<S4Corpora, String> {
    let reg = s3::read_registry(&eval.join("s3/tasks.toml"))?;
    let set = commits(eval)?;
    // Extracted S0 commits per repository, and a corpus per distinct set of ancestors.
    let mut dirs: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut extracted: BTreeMap<String, Vec<(String, cargo_smysl_corpus::Extraction)>> =
        BTreeMap::new();
    for c in &set.commits {
        let ex_path = extraction_path(eval, &reg.system, c);
        if !ex_path.exists() {
            continue;
        }
        let repo = set
            .repos
            .get(&c.repo)
            .ok_or_else(|| format!("unknown repo {}", c.repo))?;
        let dir = repo_dir(eval, &c.repo, &repo.url)?;
        dirs.insert(c.repo.clone(), dir);
        let ex = serde_json::from_str(&read(&ex_path)?)
            .map_err(|e| format!("{}: {e}", ex_path.display()))?;
        extracted
            .entry(c.repo.clone())
            .or_default()
            .push((c.sha.clone(), ex));
    }
    let mut corpora: BTreeMap<(String, Vec<String>), s3::Corpus> = BTreeMap::new();
    let mut key_of: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    for c in cases {
        let dir = dirs
            .get(&c.repo)
            .ok_or_else(|| format!("no clone for {}", c.repo))?;
        let text = if c.kind == "commit" {
            git(
                dir,
                &[
                    "show",
                    "--format=",
                    "--no-color",
                    "--no-ext-diff",
                    "-U3",
                    &c.source,
                ],
            )?
        } else {
            let p = eval.join(&c.source);
            if !p.exists() {
                println!("{}: {} does not exist yet, skipped", c.id, c.source);
                continue;
            }
            read(&p)?
        };
        let shas: Vec<String> = extracted
            .get(&c.repo)
            .into_iter()
            .flatten()
            .filter(|(sha, _)| git(dir, &["merge-base", "--is-ancestor", sha, &c.base]).is_ok())
            .map(|(sha, _)| sha.clone())
            .collect();
        let key = (c.repo.clone(), shas);
        if !corpora.contains_key(&key) {
            let mut corpus = s3::Corpus::new();
            for (sha, ex) in extracted.get(&c.repo).into_iter().flatten() {
                if key.1.contains(sha) {
                    corpus.add(sha, commit_batch(dir, sha, ex)?)?;
                }
            }
            corpora.insert(key.clone(), corpus);
        }
        key_of.insert(c.id.clone(), key);
        texts.insert(c.id.clone(), text);
    }
    Ok(S4Corpora {
        corpora,
        key_of,
        texts,
    })
}

/// Write each case as a small tree: a stub package so `cargo metadata` finds a workspace root the way
/// the shipped tool does, a `.smysl/` store holding exactly the corpus the harness used, and the change
/// as a patch file. What the shipped binary then does is its own; that is the point of the exercise.
fn s4_export(eval: &Path, set: &str, to: &Path, ids: &[String]) -> Result<(), String> {
    let cases = s4_cases(eval, set, ids)?;
    let built = s4_corpora(eval, &cases)?;
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    let mut written = 0;
    for c in &cases {
        let Some(text) = built.texts.get(&c.id) else {
            continue;
        };
        let key = &built.key_of[&c.id];
        let corpus = &built.corpora[key];
        let dir = to.join(&c.id);
        std::fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "# Written by `smysl-eval s4-export`: a stub package, so the shipped tool finds a\n\
                 # workspace root. The corpus under .smysl/ is what is being checked.\n\
                 # `[workspace]` makes it a root of its own: the export usually lands inside another\n\
                 # workspace, and cargo refuses a package that believes it is in one it is not in.\n\
                 [workspace]\n\n[package]\nname = \"s4-{}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
                c.id.to_lowercase()
            ),
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(dir.join("src/lib.rs"), "").map_err(|e| e.to_string())?;
        let records: Vec<smysl::Record> = corpus.store.iter().cloned().collect();
        cargo_smysl_corpus::store::Corpus::at(&dir)
            .save_records(&records)
            .map_err(|e| e.to_string())?;
        std::fs::write(dir.join("change.diff"), text).map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join("case.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "id": c.id, "set": c.set, "repo": c.repo, "kind": c.kind,
                "task": c.task, "contradicting": c.contradicting, "corpus_commits": key.1,
            }))
            .unwrap(),
        )
        .map_err(|e| e.to_string())?;
        written += 1;
    }
    println!(
        "s4-export {set}: {written} case(s) under {}\n\
         each is checked with: cargo smysl check --patch change.diff --json",
        to.display()
    );
    Ok(())
}

/// The cases of one set, optionally narrowed to some ids.
fn s4_cases(eval: &Path, set: &str, ids: &[String]) -> Result<Vec<S4Case>, String> {
    #[derive(serde::Deserialize)]
    struct Sets {
        case: Vec<S4Case>,
    }
    let sets: Sets = toml::from_str(&read(&eval.join("s4/sets.toml"))?)
        .map_err(|e| format!("sets.toml: {e}"))?;
    Ok(sets
        .case
        .into_iter()
        .filter(|c| c.set == set && (ids.is_empty() || ids.contains(&c.id)))
        .collect())
}

fn s4_check(eval: &Path, a: &S4Args) -> Result<(), String> {
    let cases = s4_cases(eval, &a.set, &a.id)?;
    let params = s4::Params {
        candidates: a.candidates,
        budget: a.budget,
        diff_lines: a.diff_lines,
        file_lines: a.file_lines,
        query_terms: a.query_terms,
        model: a.model.clone(),
        provider: a.provider.clone(),
        endpoint: a.endpoint.clone(),
        key_var: a.key_var.clone(),
        num_ctx: a.num_ctx,
        chunk: a.chunk,
        context_limit: a.context_limit.unwrap_or(a.num_ctx),
        reserve_output: a.reserve_output,
        chars_per_token: a.chars_per_token,
        min_term_weight: a.min_term_weight,
        judge_limit: a.judge_limit,
        order_seed: a.order_seed,
        system: match &a.prompt_file {
            Some(f) => read(f)?,
            None => s4::DEFAULT_SYSTEM.to_string(),
        },
        system_source: match &a.prompt_file {
            Some(f) => f.display().to_string(),
            None => "built-in default".into(),
        },
    };
    let out = eval.join("s4/results").join(&a.run);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let head = git(&eval.join(".."), &["rev-parse", "HEAD"]).unwrap_or_default();
    std::fs::write(
        out.join("params.json"),
        serde_json::to_string_pretty(&serde_json::json!({"params": params, "tool_commit": head.trim(), "set": a.set, "no_model": a.no_model}))
            .unwrap(),
    )
    .map_err(|e| e.to_string())?;

    let S4Corpora {
        corpora,
        key_of,
        texts,
    } = s4_corpora(eval, &cases)?;
    println!(
        "s4-check {}: {} case(s), {} corpus variant(s), results in {}",
        a.set,
        texts.len(),
        corpora.len(),
        out.display()
    );

    let queue = std::sync::Mutex::new(
        cases
            .iter()
            .filter(|c| texts.contains_key(&c.id))
            .collect::<Vec<_>>(),
    );
    let failures = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..a.jobs.max(1) {
            scope.spawn(|| loop {
                let Some(c) = queue.lock().unwrap().pop() else { break };
                let path = out.join(format!("{}.json", c.id));
                if path.exists() && !a.no_model {
                    continue;
                }
                let key = &key_of[&c.id];
                let corpus = &corpora[key];
                let diff = s4::parse_diff(&texts[&c.id], &params);
                let ranked = s4::candidates(corpus, &diff, &params);
                // What the pack must leave room for: the system prompt, the diff, and the answer.
                let reserve = u64::from(
                    params.model_tokens(&params.system) + params.model_tokens(&diff.shown),
                ) + u64::from(params.reserve_output);
                let ctx = s4::context(corpus, ranked, &params, reserve);
                let mut result = serde_json::json!({
                    "id": c.id, "set": c.set, "repo": c.repo, "kind": c.kind, "task": c.task,
                    "contradicting": c.contradicting, "corpus_commits": key.1,
                    "files": diff.files, "diff_lines_shown": diff.shown.lines().count(), "diff_truncated": diff.truncated,
                    "candidates": ctx.as_ref().map(|x| &x.candidates),
                    "pack_units": ctx.as_ref().map(|x| x.labels.len()), "pack_tokens": ctx.as_ref().map(|x| x.tokens),
                    "pack_text": ctx.as_ref().map(|x| &x.text),
                    "fingerprint": s4::fingerprint(ctx.as_ref(), &diff),
                });
                if !a.no_model {
                    match &ctx {
                        None => {
                            result["findings"] = serde_json::json!([]);
                            result["note"] = "no candidate: no model call".into();
                        }
                        Some(x) => match s4::judge(x, &diff, &params) {
                            Ok((verdicts, usage, fitting)) => {
                                let (findings, dropped) = s4::validate(corpus, x, &diff, &verdicts);
                                result["verdicts"] = serde_json::to_value(&verdicts).unwrap();
                                result["findings"] = serde_json::to_value(&findings).unwrap();
                                result["dropped"] = serde_json::to_value(&dropped).unwrap();
                                result["usage"] = serde_json::to_value(&usage).unwrap();
                                result["fitting"] = serde_json::to_value(&fitting).unwrap();
                            }
                            Err(e) => {
                                eprintln!("{}: {e}", c.id);
                                failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                continue;
                            }
                        },
                    }
                }
                let findings = result["findings"].as_array().map(|f| f.len());
                std::fs::write(&path, serde_json::to_string_pretty(&result).unwrap() + "\n").unwrap();
                println!(
                    "{:<44} candidates {:>2}  findings {}",
                    c.id,
                    ctx.as_ref().map(|x| x.candidates.len()).unwrap_or(0),
                    findings.map(|n| n.to_string()).unwrap_or_else(|| "-".into())
                );
            });
        }
    });
    let failed = failures.into_inner();
    if failed > 0 {
        return Err(format!("{failed} case(s) failed; rerun to resume"));
    }
    Ok(())
}
