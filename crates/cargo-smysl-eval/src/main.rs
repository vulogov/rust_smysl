//! `smysl-eval`: templates, adjudication and scoring for the S0 evaluation set (see eval/README.md).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_smysl_eval::{
    adjudication, extracted_items, score, Adjudication, Commit, Commits, Kind, LabelFile, Tally,
};
use clap::{Parser, Subcommand};

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
    /// Write or refresh adjudication files for a system's extractions of labelled commits.
    Adjudicate { system: String },
    /// Score a system: precision and recall per kind, per repository, and overall.
    Score { system: String },
}

fn default_eval_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval")
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Cmd::Templates => templates(&cli.eval_dir),
        Cmd::Adjudicate { system } => adjudicate(&cli.eval_dir, system),
        Cmd::Score { system } => score_system(&cli.eval_dir, system),
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

/// A blobless clone under eval/.repos, created on first use.
fn repo_dir(eval: &Path, name: &str, url: &str) -> Result<PathBuf, String> {
    let dir = eval.join(".repos").join(name);
    if !dir.join(".git").exists() && !dir.join("HEAD").exists() {
        std::fs::create_dir_all(dir.parent().unwrap()).map_err(|e| e.to_string())?;
        let status = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--filter=blob:none",
                "--no-checkout",
                url,
            ])
            .arg(&dir)
            .status()
            .map_err(|e| format!("git clone: {e}"))?;
        if !status.success() {
            return Err(format!("git clone {url} failed"));
        }
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
