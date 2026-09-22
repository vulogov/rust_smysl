use std::collections::BTreeSet;
use std::path::Path;

use crate::cli;
use cargo_smysl_corpus::query::dependents_of;
use cargo_smysl_corpus::store::Corpus;
use cargo_smysl_extract::{Cache as ExtractionCache, Recipe};
use cargo_smysl_facts::{builds, coverage, render, select, Around, Cache, Coverage};
use cargo_smysl_verdict::matching::{match_claim, retrieve, Retrieval};
use cargo_smysl_verdict::policy::decide;
use cargo_smysl_verdict::review::{close, confirm, describe, person, queue, reject};
use cargo_smysl_verdict::{Change, Provider, ProviderJudge, Settings};

use crate::cli::{Command, SmyslArgs};
use crate::exit;

pub fn run(args: SmyslArgs) -> u8 {
    match &args.command {
        Command::Doctor => doctor(&args),
        Command::Facts {
            rev,
            file,
            json,
            scope,
            names,
            hops,
        } => facts(
            &args,
            rev.as_deref(),
            file,
            *json,
            scope.then(|| Around {
                files: file.clone(),
                names: names.clone(),
                hops: *hops,
            }),
        ),
        Command::Hooks { what } => hooks(&args, what),
        Command::MergeDriver { base, ours, theirs } => merge_driver(base, ours, theirs),
        // Hidden, and the only place this tool panics on purpose: it is how the handler is tested.
        Command::SelfTestPanic => panic!("a deliberate panic, to test the report"),
        Command::Extract {
            rev,
            queued,
            since,
            max_commits,
            estimate,
            force,
            dry_run,
            recipe,
            provider,
            model,
            endpoint,
            key_var,
            window,
        } => extract_command(
            &args,
            Recording {
                rev: rev.as_deref(),
                queued: *queued,
                since: since.as_deref(),
                max_commits: *max_commits,
                estimate: *estimate,
                force: *force,
                dry_run: *dry_run,
            },
            ExtractHow {
                recipe,
                provider,
                model,
                endpoint,
                key_var,
                window: *window,
            },
        ),
        Command::Why {
            item,
            commit,
            markdown,
        } => match commit {
            Some(sha) => why_commit(&args, sha, *markdown),
            None => why(&args, item.as_deref().unwrap_or_default()),
        },
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
            passes,
            dry_run,
            no_corpus_order,
            parts,
            jobs,
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
                passes: *passes,
                dry_run: *dry_run,
                no_corpus_order: *no_corpus_order,
                parts: *parts,
                jobs: *jobs,
            },
        ),
        Command::Evidence {
            label,
            run,
            tests,
            link,
            mutate,
            provider,
            model,
            endpoint,
            key_var,
            window,
            chars_per_token,
        } => evidence(
            &args,
            label,
            EvidenceHow {
                run,
                shortlist_tests: *tests,
                link_tests: *link,
                mutate: *mutate,
                chars_per_token: *chars_per_token,
            },
            ExtractHow {
                recipe: "",
                provider,
                model,
                endpoint,
                key_var,
                window: *window,
            },
        ),
        Command::Bench { step } => bench(&args, step),
        Command::Stale { since, json } => stale(&args, since.as_deref(), *json),
        Command::Review {
            as_person,
            item,
            confirm,
            reject,
            close,
            all,
        } => review(
            &args,
            as_person.as_deref(),
            *item,
            *confirm,
            reject.as_deref(),
            close.as_deref(),
            *all,
        ),
    }
}

/// `why <label>`: what rests on a recorded unit — the decisions it conditions and what those cause.
///
/// The corpus is read from `.smysl` beside the workspace root. A label that names no unit, or more than
/// one, is an error rather than an empty answer: silence would read as "nothing depends on this".
/// `why --commit`: everything one commit recorded, in the shape a reviewer reads.
///
/// No model and no search: the grouping is the labels the tool assigned when it recorded the commit.
fn why_commit(args: &SmyslArgs, sha: &str, markdown: bool) -> u8 {
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
    // A revision the corpus does not hold is usually a revision the person has not recorded yet, so say
    // that rather than "nothing found".
    let resolved = cargo_smysl_git::read_commit(&root, sha)
        .map(|c| c.sha)
        .unwrap_or_else(|_| sha.to_string());
    let record =
        cargo_smysl_corpus::report::commit_record(&store, &corpus.labels(&store), &resolved);
    if record.is_empty() {
        println!(
            "{}: nothing recorded for this commit — cargo smysl extract {}",
            short(&resolved),
            short(&resolved)
        );
        return exit::OK;
    }
    print!(
        "{}",
        if markdown {
            cargo_smysl_corpus::report::as_markdown(&record)
        } else {
            cargo_smysl_corpus::report::as_text(&record)
        }
    );
    exit::OK
}

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

/// `facts <rev>`: what the code says, deterministically (D9, D10).
///
/// The cache is keyed by each file's bytes and the extractor version, so a second run reparses nothing
/// and a changed file cannot hit a stale entry.
fn facts(
    args: &SmyslArgs,
    rev: Option<&str>,
    only: &[String],
    json: bool,
    scope: Option<Around>,
) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl facts: {e}");
            return exit::FAILURE;
        }
    };
    let sources = match sources_at(&root, rev, only) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cargo smysl facts: {e}");
            return exit::FAILURE;
        }
    };
    if sources.is_empty() {
        eprintln!("cargo smysl facts: no Rust file in that change");
        return exit::FAILURE;
    }
    let cache = Cache::at(&root);
    let (mut all, mut failed) = (Vec::new(), 0);
    for (path, source) in &sources {
        match cache.facts_of(path, source) {
            Ok(facts) => all.extend(facts),
            Err(e) => {
                // A file that does not parse is reported, never counted as having no facts (D9).
                eprintln!("cargo smysl facts: {e}");
                failed += 1;
            }
        }
    }
    if let Some(mut around) = scope {
        // A commit's files are the change; the working tree's are not. Without a revision and without
        // `--file`, the scope is what the names reach — otherwise every file counts as touched and the
        // selection says nothing.
        if around.files.is_empty() && rev.is_some() {
            around.files = sources.iter().map(|(p, _)| p.clone()).collect();
        }
        if around.files.is_empty() && around.names.is_empty() {
            eprintln!("cargo smysl facts: --scope needs a revision, --file, or --name");
            return exit::FAILURE;
        }
        let picked = select(&all, &around);
        // What CI builds, so a fact behind a `cfg` no job compiles is marked rather than read as true.
        let ci = builds(root.join(".github").join("workflows"));
        // Which features are on by default is the manifest's answer, not the workflow's.
        let defaults = default_features(args);
        println!(
            "{} of {} fact(s) bear on this change ({} file(s), {} name(s), {} hop(s))\n",
            picked.len(),
            all.len(),
            around.files.len(),
            around.names.len(),
            around.hops
        );
        if ci.is_empty() {
            println!("(no CI workflow found: coverage of a `cfg` is unknown)\n");
        }
        for s in &picked {
            let note = match s.fact {
                cargo_smysl_facts::Fact::Function(f)
                    if !f.cfg.is_empty() || !f.file_cfg.is_empty() =>
                {
                    let cfg: Vec<String> = f.cfg.iter().chain(f.file_cfg.iter()).cloned().collect();
                    match coverage(&cfg, &ci, &defaults) {
                        Coverage::Always => String::new(),
                        Coverage::Some { builds, of } => {
                            format!(" [CI builds it in {builds} of {of}]")
                        }
                        Coverage::Never => " [CI never builds it]".into(),
                        Coverage::Unknown => String::new(),
                    }
                }
                _ => String::new(),
            };
            println!("[{:?}]{note} {}\n", s.reason, render(s.fact));
        }
        return exit::OK;
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&all).unwrap_or_default());
    } else {
        let functions = all
            .iter()
            .filter(|f| matches!(f, cargo_smysl_facts::Fact::Function(_)))
            .count();
        let events: usize = all
            .iter()
            .map(|f| match f {
                cargo_smysl_facts::Fact::Function(x) => x.events.len(),
                _ => 0,
            })
            .sum();
        println!(
            "{} file(s), {functions} function(s), {events} event(s), {} other item(s)",
            sources.len(),
            all.len() - functions
        );
        println!("cache: {}", cache.dir().display());
    }
    if failed > 0 {
        exit::FAILURE
    } else {
        exit::OK
    }
}

/// The Rust files to read: those a commit touched, or the working tree's.
fn sources_at(
    root: &Path,
    rev: Option<&str>,
    only: &[String],
) -> Result<Vec<(String, String)>, String> {
    let wanted =
        |path: &str| path.ends_with(".rs") && (only.is_empty() || only.iter().any(|f| f == path));
    match rev {
        Some(rev) => {
            let commit = cargo_smysl_git::read_commit(root, rev).map_err(|e| e.to_string())?;
            Ok(commit
                .files
                .into_iter()
                .filter(|f| wanted(&f.path))
                .filter_map(|f| f.after.map(|text| (f.path, text)))
                .collect())
        }
        None if !only.is_empty() => only
            .iter()
            .map(|rel| {
                std::fs::read_to_string(root.join(rel))
                    .map(|text| (rel.clone(), text))
                    .map_err(|e| format!("{rel}: {e}"))
            })
            .collect(),
        None => {
            let mut out = Vec::new();
            walk(root, root, &mut out);
            Ok(out.into_iter().filter(|(p, _)| wanted(p)).collect())
        }
    }
}

/// The workspace's own Rust files.
///
/// Symlinks are not followed: a link can leave the workspace entirely, and one did — a scratch `HOME`
/// under `eval/` pointed at the real home directory, so the walk left the repository and then failed on
/// a directory it had no business reading. A directory that cannot be read is skipped with a note, since
/// one unreadable corner is not a reason to report no facts.
fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("cargo smysl facts: {}: {e} (skipped)", dir.display());
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" {
            continue;
        }
        let Ok(meta) = entry.file_type() else {
            continue;
        };
        if meta.is_symlink() {
            continue;
        }
        if meta.is_dir() {
            walk(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            match std::fs::read_to_string(&path) {
                Ok(text) => out.push((rel, text)),
                Err(e) => eprintln!("cargo smysl facts: {rel}: {e} (skipped)"),
            }
        }
    }
}

/// `evidence <label>`: check one recorded claim against the code (D11, D12).
/// What the caller asked `evidence` for, beside the claim itself.
struct EvidenceHow<'a> {
    /// Names this run, so two runs can be told apart when agreement is counted (D12).
    run: &'a str,
    shortlist_tests: bool,
    link_tests: bool,
    /// Changes the mutation gate may make per test; 0 leaves it off (S1).
    mutate: usize,
    chars_per_token: f32,
}

fn evidence(args: &SmyslArgs, label: &str, e: EvidenceHow<'_>, how: ExtractHow<'_>) -> u8 {
    let EvidenceHow {
        run,
        shortlist_tests,
        link_tests,
        mutate,
        chars_per_token,
    } = e;
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    let store = match Corpus::at(&root).load() {
        Ok(s) if s.units().count() > 0 => s,
        Ok(_) => {
            eprintln!("cargo smysl evidence: no corpus recorded yet");
            return exit::FAILURE;
        }
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    // The claim is the recorded unit's own words, not the caller's paraphrase.
    let claim = match claim_text(&store, label) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    println!("{label}: {claim}\n");

    let sources = match sources_at(&root, None, &[]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    let cache = Cache::at(&root);
    let mut all = Vec::new();
    for (path, source) in &sources {
        match cache.facts_of(path, source) {
            Ok(facts) => all.extend(facts),
            Err(e) => eprintln!("cargo smysl evidence: {e}"),
        }
    }
    let judge = ProviderJudge {
        provider: Provider {
            kind: how.provider.to_string(),
            endpoint: how.endpoint.to_string(),
            model: how.model.to_string(),
            key_var: how.key_var.to_string(),
            window: how.window,
            ..Provider::local(how.model)
        },
    };
    if shortlist_tests {
        let picked = cargo_smysl_evidence::candidates(&all, &claim, &[], 5);
        println!("tests that might bear on it:");
        for c in &picked {
            println!(
                "  {} ({}:{})  shared: {}",
                c.label,
                c.file,
                c.line,
                c.shared.join(", ")
            );
            // S1's cheap check: an edge resting on a test that cannot fail is worthless.
            if let Some(f) = all.iter().find_map(|f| match f {
                cargo_smysl_facts::Fact::Function(x) if x.label() == c.label => Some(x),
                _ => None,
            }) {
                for reason in cargo_smysl_evidence::vacuous(f) {
                    println!("      warning: {reason}");
                }
            }
        }
        if picked.is_empty() {
            println!("  (none)");
        }
        println!();
        if link_tests {
            return link_evidence(
                &root,
                Linking {
                    store: &store,
                    label,
                    claim: &claim,
                    shortlist: &picked,
                    facts: &all,
                    judge: &judge,
                    mutate,
                },
            );
        }
    }

    let retrieval = Retrieval {
        run: run.to_string(),
        ..Retrieval::fitted(how.window, chars_per_token)
    };
    let shown = retrieve(&claim, &all, &retrieval);
    println!(
        "{} structural fact(s) and {} prose fact(s) retrieved",
        shown.structural.len(),
        shown.prose.len()
    );
    // D17: what did not fit is said out loud, never dropped quietly.
    let (kept, asked) = (
        shown.shown_count(),
        shown.structural.len() + shown.prose.len(),
    );
    if kept < asked {
        eprintln!(
            "cargo smysl evidence: warning: {} of {asked} retrieved fact(s) did not fit a {}-token \
             window and were left out",
            asked - kept,
            how.window
        );
    }
    let matched = match match_claim(&claim, &shown, &judge, &retrieval) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    for invented in &matched.invented {
        eprintln!("cargo smysl evidence: dropped a citation of {invented}, which was not shown");
    }
    // One run, so the policy will not raise a status — and says so rather than implying more.
    let normative = claim_is_normative(&store, label);
    let decision = decide(std::slice::from_ref(&matched.judgement), normative, false);
    println!("\ncovered:");
    for part in &matched.judgement.covered {
        println!("  {part}");
    }
    if matched.judgement.covered.is_empty() {
        println!("  (nothing)");
    }
    if !matched.judgement.uncovered.is_empty() {
        println!("uncovered:");
        for part in &matched.judgement.uncovered {
            println!("  {part}");
        }
    }
    println!("\nverdict: {:?} — {}", decision.verdict, decision.because);
    exit::OK
}

/// Run the shortlisted tests, record what they did, and propose the edges (D13).
///
/// Nothing here concludes anything: the readings are measurements, and every edge waits for a person.
struct Linking<'a> {
    store: &'a smysl::Store,
    label: &'a str,
    claim: &'a str,
    shortlist: &'a [cargo_smysl_evidence::Candidate],
    facts: &'a [cargo_smysl_facts::Fact],
    judge: &'a ProviderJudge,
    /// Changes the mutation gate may make per test; 0 leaves it off (S1).
    mutate: usize,
}

fn link_evidence(root: &Path, l: Linking<'_>) -> u8 {
    let Linking {
        store,
        label,
        claim,
        shortlist,
        facts,
        judge,
        mutate,
    } = l;
    if shortlist.is_empty() {
        println!("no test to run");
        return exit::OK;
    }
    let (classified, invented) =
        match cargo_smysl_evidence::classify(claim, shortlist, facts, judge) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("cargo smysl evidence: {e}");
                return exit::FAILURE;
            }
        };
    for name in &invented {
        eprintln!("cargo smysl evidence: dropped {name}, which was not on the shortlist");
    }
    let wanted: Vec<&cargo_smysl_evidence::Classified> = classified
        .iter()
        .filter(|c| c.kind != cargo_smysl_evidence::link::Kind::Unrelated)
        .collect();
    if wanted.is_empty() {
        println!("no test bears on this claim");
        return exit::OK;
    }
    for c in &wanted {
        println!("  {:?}: {} — {}", c.kind, c.test, c.because);
    }

    // The opt-in gate (S1): does a test that claims to verify this claim notice the code changing?
    // It only ever refuses a test that noticed nothing, and it says what it did either way.
    let mut vacuous: BTreeSet<String> = BTreeSet::new();
    if mutate > 0 {
        for note in cargo_smysl_evidence::recover(root) {
            eprintln!("cargo smysl evidence: {note}");
        }
        let Some(target) = anchor_function(claim, facts) else {
            eprintln!(
                "cargo smysl evidence: no function of this repository answers to the claim, so the \
                 mutation gate has nothing to change"
            );
            return exit::FAILURE;
        };
        println!(
            "\nmutating {}:{} ({}), at most {mutate} change(s) per test",
            target.file,
            target.line,
            target.label()
        );
        for c in wanted
            .iter()
            .filter(|c| c.kind == cargo_smysl_evidence::link::Kind::Verifies)
        {
            let one = cargo_smysl_evidence::Plan {
                tests: vec![c.test.clone()],
                ..cargo_smysl_evidence::Plan::default()
            };
            match cargo_smysl_evidence::gate(root, target, &one, "", mutate) {
                Ok(score) => {
                    println!("  {}: {}", c.test, score.because());
                    for note in &score.notes {
                        println!("      {note}");
                    }
                    if !score.backs() {
                        vacuous.insert(c.test.clone());
                    }
                }
                Err(e) => {
                    eprintln!("cargo smysl evidence: the mutation gate could not run: {e}");
                    return exit::FAILURE;
                }
            }
        }
    }

    let plan = cargo_smysl_evidence::Plan {
        tests: wanted.iter().map(|c| c.test.clone()).collect(),
        ..cargo_smysl_evidence::Plan::default()
    };
    println!("\nrunning: cargo {}", plan.args().join(" "));
    let head = cargo_smysl_git::read_commit(root, "HEAD")
        .map(|c| c.sha)
        .unwrap_or_else(|_| "unknown".into());
    let readings = match cargo_smysl_evidence::run(root, &plan, &head) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    if readings.is_empty() {
        eprintln!("cargo smysl evidence: the tests produced no reading; nothing is recorded");
        return exit::FAILURE;
    }
    for r in &readings {
        println!("  {} {} ({}s)", r.outcome, r.test, r.run_seconds);
    }

    let imported = match cargo_smysl_evidence::import(&readings, "cargo test") {
        Ok(i) => i,
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    let Ok(claim_label) = smysl::Label::new(label.to_string()) else {
        eprintln!("cargo smysl evidence: {label} is not a label");
        return exit::FAILURE;
    };
    let Ok(claim_uid) = smysl::resolve_label(store, &claim_label) else {
        eprintln!("cargo smysl evidence: {label} names no unit");
        return exit::FAILURE;
    };
    // A reading's unit, by the test it is about, so an edge names the measurement and not the test.
    let by_test: Vec<(String, smysl::Uid)> = imported
        .units
        .iter()
        .zip(&readings)
        .map(|(unit, reading)| (reading.test.clone(), smysl::canonical_uid(unit)))
        .collect();
    let links: Vec<cargo_smysl_evidence::Link> = wanted
        .iter()
        .filter_map(|c| {
            by_test
                .iter()
                .find(|(test, _)| test.ends_with(&c.test) || c.test.ends_with(test))
                .map(|(_, uid)| cargo_smysl_evidence::Link {
                    reading: *uid,
                    claim: claim_uid,
                    // A test the gate found blind to this code runs it; it does not verify it.
                    kind: if vacuous.contains(&c.test) {
                        cargo_smysl_evidence::link::Kind::Exercises
                    } else {
                        c.kind
                    },
                })
        })
        .collect();
    let outcome = |uid: smysl::Uid| -> Option<cargo_smysl_evidence::Reading> {
        by_test
            .iter()
            .position(|(_, u)| *u == uid)
            .map(|i| readings[i].clone())
    };
    let proposer = match smysl::AgentId::new(format!("model:{}", agent_name(&judge.provider.model)))
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    let edges = match cargo_smysl_evidence::edges(&links, &outcome, &proposer) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            return exit::FAILURE;
        }
    };
    let mut records: Vec<smysl::Record> = store.iter().cloned().collect();
    records.extend(imported.records());
    records.extend(edges.iter().cloned());
    let corpus = Corpus::at(root);
    match corpus.save_records(&records) {
        Ok(()) => {
            let proposed = edges
                .iter()
                .filter(|r| matches!(r, smysl::Record::Relation(_)))
                .count();
            println!(
                "\n{} reading(s) recorded, {proposed} edge(s) proposed — each waits for a person: \
                 cargo smysl review",
                readings.len()
            );
            exit::OK
        }
        Err(e) => {
            eprintln!("cargo smysl evidence: {e}");
            exit::FAILURE
        }
    }
}

/// The recorded claim's own text.
fn claim_text(store: &smysl::Store, label: &str) -> Result<String, String> {
    let label =
        smysl::Label::new(label.to_string()).map_err(|_| format!("{label} is not a label"))?;
    let uid = smysl::resolve_label(store, &label).map_err(|e| e.to_string())?;
    let unit = store.get(&uid).ok_or("the label names no unit")?;
    Ok(match &unit.core.body {
        Some(body) => format!("{} {}", unit.core.gist, body),
        None => unit.core.gist.clone(),
    })
}

/// A prerequisite the extraction marked normative reaches `ImplementedBy` at most (D12).
/// The function the claim is about: the best-scoring structural fact that is not itself a test.
///
/// The gate changes this code and asks whether the linked test notices, so choosing it wrongly makes
/// every answer meaningless. It is the same retrieval the matcher uses, which is why it is the same
/// function a reader would name.
fn anchor_function<'a>(
    claim: &str,
    facts: &'a [cargo_smysl_facts::Fact],
) -> Option<&'a cargo_smysl_facts::item::Function> {
    let shown = retrieve(claim, facts, &Retrieval::default());
    shown.structural.iter().find_map(|f| match f {
        cargo_smysl_facts::Fact::Function(x) if !x.is_test() => Some(x),
        _ => None,
    })
}

/// A model's name as an agent id may carry: `qwen2.5-coder:14b` names a tag with a colon, and an agent
/// id keeps one colon for its kind, so the rest become dashes. The name still reads as the model.
fn agent_name(model: &str) -> String {
    model
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "-._".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn claim_is_normative(store: &smysl::Store, label: &str) -> bool {
    let Ok(label) = smysl::Label::new(label.to_string()) else {
        return false;
    };
    let Ok(uid) = smysl::resolve_label(store, &label) else {
        return false;
    };
    store
        .get(&uid)
        .and_then(|u| u.core.payload.as_ref())
        .map(|p| String::from_utf8_lossy(p).contains("normative"))
        .unwrap_or(false)
}

/// `review`: what waits for a person, and what their answer records (D15).
fn review(
    args: &SmyslArgs,
    as_person: Option<&str>,
    item: Option<usize>,
    do_confirm: bool,
    do_reject: Option<&str>,
    do_close: Option<&str>,
    all: bool,
) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl review: {e}");
            return exit::FAILURE;
        }
    };
    let corpus = Corpus::at(&root);
    let store = match corpus.load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cargo smysl review: {e}");
            return exit::FAILURE;
        }
    };
    let items = queue(&store);
    let waiting: Vec<_> = items.iter().filter(|i| !i.resolved).collect();

    let Some(position) = item else {
        if waiting.is_empty() && !all {
            println!("nothing is waiting for a person");
            return exit::OK;
        }
        println!("{} item(s) waiting:", waiting.len());
        for (n, i) in waiting.iter().enumerate() {
            println!("  {n}: {}", describe(i));
        }
        if all {
            for i in items.iter().filter(|i| i.resolved) {
                println!("  (dealt with) {}", describe(i));
            }
        }
        println!(
            "\nanswer one with: cargo smysl review --as-person NAME --item N \\\n               --confirm | --reject \"why\" | --close \"note\""
        );
        return exit::OK;
    };
    let Some(subject) = waiting.get(position) else {
        eprintln!(
            "cargo smysl review: no item {position}; {} are waiting",
            waiting.len()
        );
        return exit::FAILURE;
    };
    let who = match as_person.map(person) {
        Some(Ok(who)) => who,
        Some(Err(e)) => {
            eprintln!("cargo smysl review: {e}");
            return exit::FAILURE;
        }
        None => {
            eprintln!("cargo smysl review: --as-person is required to answer an item");
            return exit::FAILURE;
        }
    };
    let records = match (do_confirm, do_reject, do_close) {
        (true, _, _) => confirm(subject, &who),
        (_, Some(why), _) => reject(subject, &who, why),
        (_, _, Some(note)) => close(subject, &who, note),
        _ => {
            eprintln!("cargo smysl review: say what to do: --confirm, --reject or --close");
            return exit::FAILURE;
        }
    };
    let records = match records {
        Ok(r) => r,
        Err(e) => {
            eprintln!("cargo smysl review: {e}");
            return exit::FAILURE;
        }
    };
    // Append, never rewrite: the store is a log.
    let mut all_records: Vec<smysl::Record> = store.iter().cloned().collect();
    all_records.extend(records.iter().cloned());
    match corpus.save_records(&all_records) {
        Ok(()) => {
            println!("{}: {} record(s) written", describe(subject), records.len());
            exit::OK
        }
        Err(e) => {
            eprintln!("cargo smysl review: {e}");
            exit::FAILURE
        }
    }
}

#[derive(Clone, Copy)]
struct ExtractHow<'a> {
    recipe: &'a str,
    provider: &'a str,
    model: &'a str,
    endpoint: &'a str,
    key_var: &'a str,
    window: u32,
}

/// `extract <rev>`: read the commit, ask the model, record what it said.
///
/// The tool reads git itself and assigns labels, sources and statuses (D5); the model proposes content
/// only, and every quote it gives is checked against the commit (D8). A commit is extracted once per
/// recipe (D7) — `--force` is for a deliberate redo, not for a retry loop.
/// What a recording run covers: one commit, or everything since a revision.
struct Recording<'a> {
    rev: Option<&'a str>,
    queued: bool,
    since: Option<&'a str>,
    max_commits: usize,
    estimate: bool,
    force: bool,
    dry_run: bool,
}

/// `extract`, over one commit or a range.
///
/// A range is recorded newest first and is resumable: a commit already recorded for this recipe costs
/// nothing to skip. Before anything is spent, the run says what it expects to cost — extraction is the
/// slowest thing this tool does, and a person deserves to know that before it starts rather than after.
fn extract_command(args: &SmyslArgs, r: Recording<'_>, how: ExtractHow<'_>) -> u8 {
    if r.queued {
        return extract_queued(args, &r, how);
    }
    let Some(since) = r.since else {
        return extract(args, r.rev, r.force, r.dry_run, how);
    };
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl extract: {e}");
            return exit::FAILURE;
        }
    };
    let shas = match cargo_smysl_git::commits_between(&root, Some(since), "HEAD", r.max_commits) {
        Ok(shas) => shas,
        Err(e) => {
            eprintln!("cargo smysl extract: {e}");
            return exit::FAILURE;
        }
    };
    let corpus = Corpus::at(&root);
    let recorded: BTreeSet<String> = corpus
        .commit_documents()
        .unwrap_or_default()
        .iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    let mut corpus_only = 0;
    let todo: Vec<String> = shas
        .iter()
        .filter(|sha| r.force || !recorded.contains(&short(sha)))
        .filter(|sha| {
            let skip = cargo_smysl_git::read_commit(&root, sha)
                .map(|c| records_only_the_corpus(&c))
                .unwrap_or(false);
            corpus_only += usize::from(skip);
            !skip
        })
        .cloned()
        .collect();

    println!(
        "{} commit(s) since {since}, {} already recorded, {} that only write the corpus, {} to do",
        shas.len(),
        shas.len() - todo.len() - corpus_only,
        corpus_only,
        todo.len()
    );
    let mut minutes = 0.0;
    for sha in &todo {
        let size = cargo_smysl_git::read_commit(&root, sha)
            .map(|c| c.files.iter().map(|f| f.text().len()).sum::<usize>())
            .unwrap_or(0);
        minutes += estimate_minutes(size);
        if r.estimate {
            println!(
                "  {}  {:>7} KB  about {:.0} min",
                short(sha),
                size / 1024,
                estimate_minutes(size)
            );
        }
    }
    println!(
        "estimated {:.0} minute(s) on {}; measured on this project, and a guess about your model",
        minutes, how.model
    );
    if r.estimate {
        println!("nothing was run: --estimate only says what it would cost");
        return exit::OK;
    }

    let mut done = 0;
    for (n, sha) in todo.iter().enumerate() {
        println!("\n[{}/{}] {}", n + 1, todo.len(), short(sha));
        let code = extract(args, Some(sha), r.force, r.dry_run, how);
        if code != exit::OK {
            // One commit failing is not the run failing: the rest are still worth recording.
            eprintln!("cargo smysl extract: {} failed; carrying on", short(sha));
            continue;
        }
        done += 1;
    }
    println!("\n{done} of {} commit(s) recorded", todo.len());
    exit::OK
}

/// Record what the post-commit hook queued, oldest first.
///
/// A commit is forgotten only when it is recorded, so an interrupted run resumes where it stopped and a
/// commit that fails stays queued to be tried again.
fn extract_queued(args: &SmyslArgs, r: &Recording<'_>, how: ExtractHow<'_>) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl extract: {e}");
            return exit::FAILURE;
        }
    };
    let path = queue_path(&root);
    let (queued, refused) = read_queue(&path);
    for line in &refused {
        eprintln!(
            "cargo smysl extract: {} is not a revision; dropped from the queue",
            line.chars().take(50).collect::<String>()
        );
    }
    // The same commit twice in the queue is one commit to record.
    let mut seen = BTreeSet::new();
    let queued: Vec<String> = queued
        .into_iter()
        .filter(|s| seen.insert(s.clone()))
        .collect();
    if queued.is_empty() {
        println!("nothing queued (the post-commit hook queues; cargo smysl hooks install)");
        return exit::OK;
    }
    // The tool's own commits are not work. Dropping them here is what makes the queue reach empty.
    let (queued, corpus_only): (Vec<String>, Vec<String>) = queued.into_iter().partition(|sha| {
        !cargo_smysl_git::read_commit(&root, sha)
            .map(|c| records_only_the_corpus(&c))
            .unwrap_or(false)
    });
    if !corpus_only.is_empty() {
        println!(
            "{} queued commit(s) only write the corpus and were dropped from the queue",
            corpus_only.len()
        );
        write_queue(&path, &queued);
    }
    if queued.is_empty() {
        println!("nothing left to record");
        return exit::OK;
    }
    let take = if r.max_commits == 0 {
        queued.len()
    } else {
        r.max_commits.min(queued.len())
    };
    println!("{} queued, recording {take}", queued.len());
    if r.estimate {
        println!("nothing was run: --estimate only says what it would cost");
        return exit::OK;
    }

    let mut left: Vec<String> = queued.clone();
    for sha in queued.iter().take(take) {
        println!("\n{}", short(sha));
        if extract(args, Some(sha), r.force, r.dry_run, how) == exit::OK {
            left.retain(|s| s != sha);
            // Written after each one, so an interrupted run does not redo what it finished.
            write_queue(&path, &left);
        } else {
            eprintln!("cargo smysl extract: {} stays queued", short(sha));
        }
    }
    println!("\n{} left in the queue", left.len());
    exit::OK
}

/// Minutes a commit of this size is likely to take, from what this project measured on a local 14B:
/// 7-11 minutes for about 100 KB, about 29 for 700 KB read in six parts. A hosted model is much faster
/// and this will overstate it — which is the safe direction for a number someone decides on.
fn estimate_minutes(bytes: usize) -> f64 {
    let kb = bytes as f64 / 1024.0;
    (4.0 + kb / 25.0).min(35.0)
}

fn extract(
    args: &SmyslArgs,
    rev: Option<&str>,
    force: bool,
    dry_run: bool,
    how: ExtractHow<'_>,
) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl extract: {e}");
            return exit::FAILURE;
        }
    };
    let commit = match cargo_smysl_git::read_commit(&root, rev.unwrap_or("HEAD")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cargo smysl extract: {e}");
            return exit::FAILURE;
        }
    };
    let recipe = Recipe {
        name: how.recipe.to_string(),
        ..Recipe::default()
    };
    let cache = ExtractionCache::at(&root);

    let kept = if force {
        None
    } else {
        cache.read(&commit.sha, &recipe)
    };
    let extraction = match kept {
        Some(extraction) => {
            println!(
                "{}: already extracted ({})",
                short(&commit.sha),
                recipe.name
            );
            extraction
        }
        None => {
            // What the model sees: the commit as the tool read it, message then diff.
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
            let judge = ProviderJudge {
                provider: Provider {
                    kind: how.provider.to_string(),
                    endpoint: how.endpoint.to_string(),
                    model: how.model.to_string(),
                    key_var: how.key_var.to_string(),
                    window: how.window,
                    ..Provider::local(how.model)
                },
            };
            say_what_is_sent(&judge.provider, "extract", args.quiet);
            match cargo_smysl_extract::extract(&source, &judge, &recipe) {
                Ok((extraction, report)) => {
                    for w in &report.warnings {
                        eprintln!("cargo smysl extract: {w}");
                    }
                    match cache.write(&commit.sha, &recipe, &extraction) {
                        Ok(path) => {
                            println!("{} call(s); kept in {}", report.calls, path.display())
                        }
                        Err(e) => {
                            eprintln!("cargo smysl extract: {e}");
                            return exit::FAILURE;
                        }
                    }
                    extraction
                }
                Err(e) => {
                    eprintln!("cargo smysl extract: {e}");
                    return exit::FAILURE;
                }
            }
        }
    };
    println!(
        "{} decision(s), {} prerequisite(s), {} alternative(s), {} consequence(s)",
        extraction.decisions.len(),
        extraction.prerequisites.len(),
        extraction.alternatives.len(),
        extraction.consequences.len()
    );
    if dry_run {
        return exit::OK;
    }

    // Build, stage and record: the corpus assigns labels and sources, checks every quote, and refuses a
    // batch that breaks a smysl rule.
    let texts: Vec<(String, String)> = commit
        .files
        .iter()
        .map(|f| (f.path.clone(), f.text()))
        .collect();
    let text = cargo_smysl_corpus::CommitText {
        sha: &commit.sha,
        message: &commit.message,
        files: texts
            .iter()
            .map(|(p, t)| (p.as_str(), t.as_str()))
            .collect(),
        touched: touched_items(&commit),
    };
    let batch = match cargo_smysl_corpus::build(&extraction, &text, 0) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cargo smysl extract: {e}");
            return exit::FAILURE;
        }
    };
    println!(
        "quotes: {} present, {} loose, {} absent (capped at speculative){}",
        batch.quotes.present,
        batch.quotes.loose,
        batch.quotes.absent,
        if batch.dropped.is_empty() {
            String::new()
        } else {
            format!("; {} item(s) dropped", batch.dropped.len())
        }
    );
    let corpus = Corpus::at(&root);
    let store = match corpus.load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cargo smysl extract: {e}");
            return exit::FAILURE;
        }
    };
    let batch_reflowed = batch.reflowed;
    let staged = cargo_smysl_corpus::stage(&store, batch, 0);
    let errors: Vec<String> = staged
        .report
        .iter()
        .filter(|d| d.severity == smysl::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    if !errors.is_empty() {
        // A diagnostic names a uid, which tells a person nothing. Show the unit it is about.
        let units: std::collections::BTreeMap<String, (String, String)> = staged
            .records()
            .iter()
            .filter_map(|r| match r {
                smysl::Record::Unit(u) => Some((
                    smysl::canonical_uid(u).to_string(),
                    (u.gist.clone(), u.body.clone().unwrap_or_default()),
                )),
                _ => None,
            })
            .collect();
        for e in errors.iter().take(5) {
            eprintln!("cargo smysl extract: {e}");
            if let Some((uid, (gist, body))) = units
                .iter()
                .find(|(uid, _)| e.contains(uid.as_str()))
                .map(|(uid, v)| (uid.clone(), v.clone()))
            {
                let _ = uid;
                eprintln!("      gist: {}", gist.lines().next().unwrap_or(&gist));
                for line in body.lines().take(4) {
                    eprintln!("      body: {line}");
                }
            }
        }
        if errors.len() > 5 {
            eprintln!("cargo smysl extract: and {} more", errors.len() - 5);
        }
        // The model's answers are already cached (D7), so this failure costs minutes of reading, not
        // minutes of a model. Saying so is the difference between "try again" and "that was wasted".
        eprintln!(
            "cargo smysl extract: the model's answers are kept in {}; \n\
             this failed while recording them, so `cargo smysl extract {}` after a fix \
             costs no model calls.",
            cache.path(&commit.sha, &recipe).display(),
            short(&commit.sha)
        );
        return exit::FAILURE;
    }
    if batch_reflowed > 0 {
        println!(
            "{batch_reflowed} body/bodies were joined into one paragraph, which is what this \
             granularity admits"
        );
    }
    match corpus.record(&commit.sha, &staged) {
        Ok(r) => {
            println!(
                "recorded {} ({} record(s) added, {} in the corpus)",
                r.document.display(),
                r.added,
                r.records
            );
            exit::OK
        }
        Err(e) => {
            eprintln!("cargo smysl extract: {e}");
            exit::FAILURE
        }
    }
}

/// The code items a commit changed: what an anchor is for (D4).
///
/// Deterministic, and the tool's own reading of the code — a model proposes no part of this (D5). An
/// item counts as touched when its body hashes differently before and after, or when it is new; a file
/// that only moved around it says nothing.
fn touched_items(commit: &cargo_smysl_git::CommitData) -> Vec<cargo_smysl_corpus::TouchedItem> {
    let items = |path: &str, text: &str| -> std::collections::BTreeMap<String, String> {
        cargo_smysl_facts::facts(path, text)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|f| match f {
                cargo_smysl_facts::Fact::Function(x) => Some((x.label(), x.body_hash.clone())),
                _ => None,
            })
            .collect()
    };
    let mut out = Vec::new();
    for file in &commit.files {
        if !file.path.ends_with(".rs") {
            continue;
        }
        let Some(after) = &file.after else { continue };
        let was = file
            .before
            .as_deref()
            .map(|text| items(&file.path, text))
            .unwrap_or_default();
        for (item, hash) in items(&file.path, after) {
            if was.get(&item) != Some(&hash) {
                out.push(cargo_smysl_corpus::TouchedItem {
                    path: file.path.clone(),
                    item,
                    body_hash: hash,
                });
            }
        }
    }
    out
}

/// A judge that says what it is doing, because a minute of silence reads as a hang.
///
/// It wraps the real one and prints to stderr: a line when a call goes out, a line when it comes back
/// with how long it took. Nothing is printed when the output is machine-readable or quiet was asked for
/// — a progress line in a pipe is noise.
struct Narrating<'a> {
    inner: &'a (dyn cargo_smysl_extract::Judge + Sync),
    state: std::sync::Mutex<(usize, usize)>,
    quiet: bool,
}

impl<'a> Narrating<'a> {
    fn around(inner: &'a (dyn cargo_smysl_extract::Judge + Sync), quiet: bool) -> Narrating<'a> {
        Narrating {
            inner,
            state: std::sync::Mutex::new((0, 0)),
            quiet,
        }
    }
}

impl cargo_smysl_extract::Judge for Narrating<'_> {
    fn describe(&self) -> String {
        self.inner.describe()
    }

    fn input_chars(&self) -> Option<usize> {
        self.inner.input_chars()
    }

    fn ask_text(
        &self,
        system: &str,
        user: &str,
    ) -> Result<(String, cargo_smysl_extract::Charged), cargo_smysl_extract::JudgeError> {
        let started = std::time::Instant::now();
        let n = {
            let mut state = self.state.lock().unwrap();
            state.0 += 1;
            state.1 += 1;
            if !self.quiet {
                eprintln!(
                    "  asking {} — call {} ({} in flight, {} KB)",
                    self.inner.describe(),
                    state.0,
                    state.1,
                    user.len() / 1024
                );
            }
            state.0
        };
        let answer = self.inner.ask_text(system, user);
        let mut state = self.state.lock().unwrap();
        state.1 -= 1;
        if !self.quiet {
            eprintln!(
                "  call {n} {} after {:.0}s",
                if answer.is_ok() { "answered" } else { "failed" },
                started.elapsed().as_secs_f32()
            );
        }
        answer
    }
}

/// Say what is about to leave the machine, when it is leaving it.
///
/// Extraction and checking send the commit — its message and the text of its files — to whichever model
/// the operator named. On this machine that is a local matter; to a hosted provider it is the code
/// leaving, and a person is entitled to know before it does rather than from a bill afterwards.
fn say_what_is_sent(provider: &Provider, kind: &str, quiet: bool) {
    if quiet {
        return;
    }
    let local = provider.endpoint.contains("://localhost")
        || provider.endpoint.contains("://127.")
        || provider.endpoint.contains("://[::1]");
    if local {
        return;
    }
    eprintln!(
        "cargo smysl {kind}: sending this repository's code to {} ({}). \
         A local provider keeps it on this machine.",
        provider.endpoint, provider.model
    );
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
    passes: usize,
    dry_run: bool,
    no_corpus_order: bool,
    parts: usize,
    jobs: usize,
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
        order_by_corpus: !c.no_corpus_order,
        max_parts: c.parts.max(1),
        jobs: c.jobs.max(1),
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
    if c.dry_run {
        // The preview reorders exactly as a real run would, so a person sees what will be examined.
        let change = if change.truncated && settings.order_by_corpus {
            let weight = |path: &str| store.units_with_source_prefix(&format!("{path}@")).len();
            change.ordered_by(&weight, &settings)
        } else {
            change
        };
        // A preview of a run that would read the change in parts previews every part, or it is a
        // preview of something else.
        let parts = change.parts(&settings);
        let mut judged: Vec<String> = Vec::new();
        let mut text = String::new();
        let mut unexamined: BTreeSet<String> = BTreeSet::new();
        let mut examined: BTreeSet<String> = BTreeSet::new();
        for part in &parts {
            if let Some((t, j)) = cargo_smysl_verdict::check::preview(&store, part, &settings) {
                text.push_str(&t);
                judged.extend(j);
            }
            let unseen = part.unexamined();
            for file in &part.files {
                if unseen.contains(file) {
                    unexamined.insert(file.clone());
                } else {
                    examined.insert(file.clone());
                }
            }
        }
        judged.sort();
        judged.dedup();
        let shown = serde_json::json!({
            "parts": parts.len(),
            "unexamined": unexamined.difference(&examined).cloned().collect::<Vec<_>>(),
            "judged": judged,
            "units_judged": judged.len(),
            "pack_text": text,
            "diff_lines_shown": parts.iter().map(|p| p.shown().lines().count()).sum::<usize>(),
            "diff_truncated": change.truncated,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&shown).unwrap_or_default()
        );
        return exit::OK;
    }
    // Two passes by default, which is the configuration the quoted figures were measured at.
    let seeds: Vec<usize> = cargo_smysl_verdict::check::AGREEMENT_SEEDS
        .iter()
        .copied()
        .cycle()
        .take(c.passes.max(1))
        .collect();
    say_what_is_sent(&judge.provider, "check", c.json || args.quiet);
    // Say what is happening while it happens: these calls take about a minute each.
    let narrating = Narrating::around(&judge, c.json || args.quiet);
    if !(c.json || args.quiet) {
        eprintln!(
            "checking {} file(s) against {} recorded commit(s), {} pass(es), {} call(s) at a time",
            change.files.len(),
            store.units().count().min(9999),
            seeds.len().max(1),
            settings.jobs.max(1)
        );
    }
    let outcome = if seeds.len() > 1 {
        cargo_smysl_verdict::check::check_agreed(&store, &change, &narrating, &settings, &seeds)
    } else {
        cargo_smysl_verdict::check(&store, &change, &narrating, &settings)
    };

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
                "nothing recorded is contradicted{} ({} unit(s) judged, {})",
                if outcome.unexamined.is_empty() {
                    String::new()
                } else {
                    format!(
                        " in what was examined; {} file(s) were not",
                        outcome.unexamined.len()
                    )
                },
                outcome.units_judged,
                outcome.judge
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
                    "advisory: `check` reports, it does not block. `--strict` exits 5 on findings.\n\
                     measured on a local 14B: about 1 flag in 10 was correct, and it finds about a\n\
                     third of real contradictions. Read a finding as a place to look."
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

/// Every feature the workspace's packages turn on by default, with what those features enable.
///
/// A workflow says `--no-default-features --features cli`; it cannot say what `default` means. That is
/// here, and without it "CI builds this" is a guess.
fn default_features(args: &SmyslArgs) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let mut cmd = cargo_metadata::MetadataCommand::new();
    if let Some(path) = &args.manifest_path {
        cmd.manifest_path(path);
    }
    let Ok(metadata) = cmd.no_deps().exec() else {
        return out;
    };
    for package in metadata.workspace_packages() {
        let mut queue: Vec<String> = package.features.get("default").cloned().unwrap_or_default();
        while let Some(feature) = queue.pop() {
            let name = feature.split('/').next().unwrap_or(&feature).to_string();
            if out.insert(name.clone()) {
                if let Some(more) = package.features.get(&name) {
                    queue.extend(more.clone());
                }
            }
        }
    }
    out
}

/// The workspace root, the way cargo sees it.
/// Whether a commit only writes the corpus: the tool's own output, and not a change to reason about.
///
/// Recording a commit produces a commit — the documents and the extraction cache — and extracting *that*
/// produces another, which is a march with no end. A commit that touches nothing but `.smysl/` has no
/// decisions in it that were not already recorded, so it is skipped and the backlog reaches zero.
///
/// A commit that touches `.smysl/` *and* source is a real change, and is not skipped.
fn records_only_the_corpus(commit: &cargo_smysl_git::CommitData) -> bool {
    !commit.files.is_empty()
        && commit.files.iter().all(|f| {
            f.path
                .starts_with(&format!("{}/", cargo_smysl_corpus::CORPUS_DIR))
                || f.path == cargo_smysl_corpus::CORPUS_DIR
        })
}

/// The first twelve characters of a revision, however short or strange it is.
///
/// Slicing a string by byte count panics on a short one or on a character boundary, and shas reach this
/// tool from a queue file as well as from git.
fn short(sha: &str) -> String {
    sha.chars().take(12).collect()
}

fn workspace_root(args: &SmyslArgs) -> Result<std::path::PathBuf, String> {
    let mut cmd = cargo_metadata::MetadataCommand::new();
    if let Some(path) = &args.manifest_path {
        cmd.manifest_path(path);
    }
    let metadata = cmd.no_deps().exec().map_err(|e| e.to_string())?;
    Ok(metadata.workspace_root.into_std_path_buf())
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
    // What is not recorded yet, so the backlog is something you see rather than something you remember.
    let store = Corpus::at(root);
    let recorded: BTreeSet<String> = store
        .commit_documents()
        .unwrap_or_default()
        .iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    if !recorded.is_empty() {
        match cargo_smysl_git::commits_between(root, None, "HEAD", 200) {
            Ok(history) => {
                // A commit that only writes the corpus is not a backlog item: recording it would
                // produce another like it, and the count would never reach zero.
                let behind = history
                    .iter()
                    .take_while(|sha| !recorded.contains(&short(sha)))
                    .filter(|sha| {
                        !cargo_smysl_git::read_commit(root, sha)
                            .map(|c| records_only_the_corpus(&c))
                            .unwrap_or(false)
                    })
                    .count();
                println!(
                    "recorded: {} commit(s); {behind} newer commit(s) not recorded{}",
                    recorded.len(),
                    if behind > 0 {
                        " (cargo smysl extract --since <rev> --estimate)"
                    } else {
                        ""
                    }
                );
            }
            Err(e) => println!(
                "recorded: {} commit(s); history unreadable ({e})",
                recorded.len()
            ),
        }
    }

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

// -------------------------------------------------------------------------------------------------
// bench: measuring extraction against a person's labels (D19)
// -------------------------------------------------------------------------------------------------

/// Where a benchmark keeps its files: beside the corpus, because it is about this repository.
fn bench_dir(root: &Path) -> std::path::PathBuf {
    root.join(".smysl").join("bench")
}

fn bench(args: &SmyslArgs, step: &cli::BenchStep) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl bench: {e}");
            return exit::FAILURE;
        }
    };
    match step {
        cli::BenchStep::Init {
            revs,
            last,
            file_lines,
            force,
        } => bench_init(&root, revs, *last, *file_lines, *force),
        cli::BenchStep::Status => bench_status(&root),
        cli::BenchStep::Adjudicate { recipe } => bench_adjudicate(&root, recipe),
        cli::BenchStep::Score { recipe } => bench_score(&root, recipe),
    }
}

/// Write a sheet and a template per commit. An existing template is kept unless `--force`: it may hold
/// an afternoon's work.
fn bench_init(root: &Path, revs: &[String], last: usize, file_lines: usize, force: bool) -> u8 {
    let dir = bench_dir(root);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("cargo smysl bench: {}: {e}", dir.display());
        return exit::FAILURE;
    }
    let chosen: Vec<String> = if revs.is_empty() {
        (0..last).map(|n| format!("HEAD~{n}")).collect()
    } else {
        revs.to_vec()
    };
    let repo = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repository".into());
    let (mut written, mut kept) = (0, 0);
    for rev in &chosen {
        let (sha, template, reading) =
            match cargo_smysl_bench::sheet::of_commit(root, &repo, rev, file_lines) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("cargo smysl bench: {rev}: {e}");
                    continue;
                }
            };
        let label = dir.join(format!("{sha}.toml"));
        if label.exists() && !force {
            kept += 1;
        } else if let Err(e) = std::fs::write(&label, template) {
            eprintln!("cargo smysl bench: {}: {e}", label.display());
            return exit::FAILURE;
        } else {
            written += 1;
        }
        let sheet = dir.join(format!("{sha}.diff"));
        if let Err(e) = std::fs::write(&sheet, reading) {
            eprintln!("cargo smysl bench: {}: {e}", sheet.display());
            return exit::FAILURE;
        }
    }
    println!(
        "{written} template(s) written, {kept} kept, in {}\n\
         read <sha>.diff, fill in <sha>.toml, set status = \"done\", then: cargo smysl bench status",
        dir.display()
    );
    exit::OK
}

/// The labels, and what each one is waiting for.
fn bench_status(root: &Path) -> u8 {
    let labels = match bench_labels(root) {
        Ok(labels) => labels,
        Err(e) => {
            eprintln!("cargo smysl bench: {e}");
            return exit::FAILURE;
        }
    };
    if labels.is_empty() {
        println!("no labels yet: cargo smysl bench init");
        return exit::OK;
    }
    let (mut done, mut todo) = (0, 0);
    for (sha, label) in &labels {
        if label.is_done() {
            done += 1;
            let counts = cargo_smysl_bench::Kind::ALL
                .iter()
                .map(|k| format!("{} {:?}", label.items(*k).len(), k))
                .collect::<Vec<_>>()
                .join(", ");
            println!("done  {sha}  {counts}");
        } else {
            todo += 1;
            println!("todo  {sha}  .smysl/bench/{sha}.diff");
        }
        if let Err(e) = label.validate() {
            println!("      warning: {e}");
        }
    }
    println!("\n{done} labelled, {todo} to go");
    if done > 0 {
        println!("next: cargo smysl extract <sha> for each, then cargo smysl bench adjudicate");
    }
    exit::OK
}

fn bench_labels(root: &Path) -> Result<Vec<(String, cargo_smysl_bench::LabelFile)>, String> {
    let dir = bench_dir(root);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e != "toml").unwrap_or(true) {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let label: cargo_smysl_bench::LabelFile =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let sha = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push((sha, label));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Pair what the tool extracted with what the person labelled, keeping any pairing already made.
fn bench_adjudicate(root: &Path, recipe: &str) -> u8 {
    let labels = match bench_labels(root) {
        Ok(labels) => labels,
        Err(e) => {
            eprintln!("cargo smysl bench: {e}");
            return exit::FAILURE;
        }
    };
    let cache = ExtractionCache::at(root);
    let recipe = Recipe {
        name: recipe.to_string(),
        ..Recipe::default()
    };
    let dir = bench_dir(root);
    let (mut written, mut unlabelled, mut unextracted) = (0, 0, 0);
    for (sha, label) in &labels {
        if !label.is_done() {
            unlabelled += 1;
            continue;
        }
        let Some(extraction) = cache.read(&label.commit, &recipe) else {
            unextracted += 1;
            println!("{sha}: nothing extracted yet: cargo smysl extract {sha}");
            continue;
        };
        let json = match serde_json::to_value(&extraction) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("cargo smysl bench: {sha}: {e}");
                return exit::FAILURE;
            }
        };
        let items = cargo_smysl_bench::extracted_items(&json);
        let path = dir.join(format!("{sha}.{}.adjudication.toml", recipe.name));
        // A pairing already made is kept: this file is the person's work, and rerunning must not undo it.
        let previous: Option<cargo_smysl_bench::Adjudication> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| toml::from_str(&t).ok());
        let adj = cargo_smysl_bench::adjudication(&recipe.name, label, &items, previous.as_ref());
        let text = match toml::to_string_pretty(&adj) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("cargo smysl bench: {e}");
                return exit::FAILURE;
            }
        };
        let header = "# Set `match` for each item: a label id of the same kind, or \"none\".\n\
                      # `suggest` is word overlap, nothing more — it proposes, you decide.\n\n";
        if let Err(e) = std::fs::write(&path, format!("{header}{text}")) {
            eprintln!("cargo smysl bench: {}: {e}", path.display());
            return exit::FAILURE;
        }
        written += 1;
        let pending = adj.items.iter().filter(|i| i.matched.is_empty()).count();
        println!(
            "{sha}: {} item(s), {pending} to pair  ->  {}",
            adj.items.len(),
            path.display()
        );
    }
    println!(
        "\n{written} file(s); {unlabelled} label(s) unfinished, {unextracted} commit(s) not extracted"
    );
    if written > 0 {
        println!("set `match` in each, then: cargo smysl bench score");
    }
    exit::OK
}

/// Count. Precision is of what was judged; recall waits until nothing is pending, because a recall
/// computed over half an adjudication flatters whatever was easy to pair.
fn bench_score(root: &Path, recipe: &str) -> u8 {
    let labels = match bench_labels(root) {
        Ok(labels) => labels,
        Err(e) => {
            eprintln!("cargo smysl bench: {e}");
            return exit::FAILURE;
        }
    };
    let dir = bench_dir(root);
    let mut totals: std::collections::BTreeMap<cargo_smysl_bench::Kind, cargo_smysl_bench::Tally> =
        std::collections::BTreeMap::new();
    let mut scored = 0;
    for (sha, label) in &labels {
        if !label.is_done() {
            continue;
        }
        let path = dir.join(format!("{sha}.{recipe}.adjudication.toml"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let adj: cargo_smysl_bench::Adjudication = match toml::from_str(&text) {
            Ok(adj) => adj,
            Err(e) => {
                eprintln!("cargo smysl bench: {}: {e}", path.display());
                return exit::FAILURE;
            }
        };
        match cargo_smysl_bench::score(label, &adj) {
            Ok(tallies) => {
                for (kind, tally) in tallies {
                    totals.entry(kind).or_default().add(tally);
                }
                scored += 1;
            }
            Err(e) => {
                eprintln!("cargo smysl bench: {e}");
                return exit::FAILURE;
            }
        }
    }
    if scored == 0 {
        println!("nothing to score yet: cargo smysl bench adjudicate");
        return exit::OK;
    }
    let show = |v: Option<f64>| match v {
        Some(x) => format!("{:.0}%", x * 100.0),
        None => "—".to_string(),
    };
    println!("{scored} commit(s), recipe {recipe}\n");
    println!(
        "{:<14}{:>10}{:>8}{:>11}{:>8}{:>9}",
        "kind", "extracted", "labels", "precision", "recall", "pending"
    );
    for (kind, t) in &totals {
        println!(
            "{:<14}{:>10}{:>8}{:>11}{:>8}{:>9}",
            format!("{kind:?}").to_lowercase(),
            t.extracted,
            t.labels,
            show(t.precision()),
            show(t.recall()),
            t.pending
        );
    }
    println!(
        "\nThese are figures about the model you used, on these commits — not about the tool.\n\
         Measured elsewhere for comparison (docs/implementation-plan.md §6): a hosted pro model reached\n\
         96% precision on decisions and 61% on prerequisites; a local 14B over-produces both."
    );
    exit::OK
}

// -------------------------------------------------------------------------------------------------
// stale: reasoning whose code has moved
// -------------------------------------------------------------------------------------------------

/// `stale`: which recorded commits rest on code that has since changed.
///
/// The comparison is by item and by body hash, so a function that moved down a file without changing is
/// not stale. It reports; it never withdraws a unit or lowers a status (D15).
fn stale(args: &SmyslArgs, since: Option<&str>, json: bool) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl stale: {e}");
            return exit::FAILURE;
        }
    };
    let corpus = Corpus::at(&root);
    let store = match corpus.load() {
        Ok(store) if store.units().count() > 0 => store,
        Ok(_) => {
            eprintln!("cargo smysl stale: no corpus recorded yet");
            return exit::FAILURE;
        }
        Err(e) => {
            eprintln!("cargo smysl stale: {e}");
            return exit::FAILURE;
        }
    };
    let documents = match corpus.commit_documents() {
        Ok(docs) => docs,
        Err(e) => {
            eprintln!("cargo smysl stale: {e}");
            return exit::FAILURE;
        }
    };
    // Which commits the corpus holds, oldest first by their own order on disk.
    let mut shas: Vec<String> = documents
        .iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    if let Some(since) = since {
        match cargo_smysl_git::read_commit(&root, since) {
            Ok(base) => shas.retain(|sha| {
                base.sha.starts_with(sha.as_str()) || is_after(&root, sha, &base.sha)
            }),
            Err(e) => {
                eprintln!("cargo smysl stale: {since}: {e}");
                return exit::FAILURE;
            }
        }
    }
    let units_of = |sha: &str| -> usize {
        store
            .units()
            .filter(|(_, u)| {
                u.core
                    .source
                    .as_ref()
                    .map(|s| s.reference.contains(sha))
                    .unwrap_or(false)
            })
            .count()
    };

    // The working tree is the same for every commit, so parse each file once rather than once per
    // commit that touched it: with a hundred commits over the same files that is the difference between
    // a hundred parses and one.
    let mut now_cache: std::collections::BTreeMap<String, Vec<cargo_smysl_facts::Fact>> =
        std::collections::BTreeMap::new();
    let mut reports = Vec::new();
    let mut behind: Vec<Vec<(String, Vec<String>)>> = Vec::new();
    for sha in &shas {
        let commit = match cargo_smysl_git::read_commit(&root, sha) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("cargo smysl stale: {sha}: {e}");
                continue;
            }
        };
        let mut recorded = Vec::new();
        let mut now = Vec::new();
        let mut unreadable = Vec::new();
        for file in &commit.files {
            if !file.path.ends_with(".rs") {
                continue;
            }
            // The file as the commit left it: the code the reasoning was recorded about.
            if let Some(after) = &file.after {
                match cargo_smysl_facts::facts(&file.path, after) {
                    Ok(facts) => recorded.extend(facts),
                    Err(e) => unreadable.push(format!("{} as recorded: {e}", file.path)),
                }
            }
            // The file as it is now. A file that is gone leaves its items gone, which is the answer.
            if let Some(cached) = now_cache.get(&file.path) {
                now.extend(cached.iter().cloned());
            } else if let Ok(text) = std::fs::read_to_string(root.join(&file.path)) {
                match cargo_smysl_facts::facts(&file.path, &text) {
                    Ok(facts) => {
                        now_cache.insert(file.path.clone(), facts.clone());
                        now.extend(facts);
                    }
                    Err(e) => unreadable.push(format!("{}: {e}", file.path)),
                }
            }
        }
        let changes = cargo_smysl_verdict::stale::compare(&recorded, &now);
        // Which decisions rest on each moved item, when the corpus knows: a decision quoted from a file
        // carries an `x.code/touches` edge to that file's items (D4). A decision quoted from the message
        // has the commit as its scope and is named at the commit level, as before.
        // Collected, not printed: `--json` must put nothing on stdout but JSON, and this used to
        // print here, which made the output unparseable for anything downstream.
        let anchored = anchored_decisions(&store, &corpus.labels(&store), &changes);
        reports.push(cargo_smysl_verdict::stale::Report {
            commit: sha.clone(),
            units: units_of(sha),
            changes,
            unreadable,
        });
        behind.push(anchored);
    }

    if json {
        let value: Vec<serde_json::Value> = reports
            .iter()
            .zip(&behind)
            .map(|(r, anchored)| {
                serde_json::json!({
                    "commit": r.commit,
                    "units": r.units,
                    "changed": r.counts().0,
                    "gone": r.counts().1,
                    "items": r.changes.iter().map(|c| serde_json::json!({
                        "item": c.label, "file": c.file, "because": c.because(),
                    })).collect::<Vec<_>>(),
                    // Which recorded decisions each moved item is behind, when the corpus knows.
                    "behind": anchored.iter().map(|(item, labels)| serde_json::json!({
                        "item": item, "decisions": labels,
                    })).collect::<Vec<_>>(),
                    "unreadable": r.unreadable,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        );
        return exit::OK;
    }

    for anchored in &behind {
        for (item, labels) in anchored {
            println!("  {item} is behind: {}", labels.join(", "));
        }
    }
    let stale: Vec<_> = reports.iter().filter(|r| r.is_stale()).collect();
    for r in &stale {
        let (changed, gone) = r.counts();
        println!(
            "{}: {} unit(s) rest on code that has moved — {changed} item(s) changed, {gone} gone",
            r.commit, r.units
        );
        for c in r.changes.iter().take(12) {
            println!("  {}", c.because());
        }
        if r.changes.len() > 12 {
            println!("  … {} more", r.changes.len() - 12);
        }
        for u in &r.unreadable {
            println!("  warning: {u}");
        }
        println!();
    }
    println!(
        "{} of {} recorded commit(s) rest on code that has moved.\n\
         Nothing is withdrawn: whether the reasoning still holds is a person's call, and the answer \
         belongs in cargo smysl review.",
        stale.len(),
        reports.len()
    );
    exit::OK
}

/// The decisions anchored to each moved item, by label.
///
/// An anchor's gist is `path::item`, so a moved item is matched by name; the edges are the ones the
/// corpus recorded, never inferred here.
fn anchored_decisions(
    store: &smysl::Store,
    labels: &std::collections::BTreeMap<smysl::Uid, Vec<smysl::Label>>,
    changes: &[cargo_smysl_verdict::stale::Change],
) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for change in changes {
        let wanted = format!("{}::{}", change.file, change.label);
        let anchors: Vec<smysl::Uid> = store
            .units()
            .filter(|(_, u)| u.core.gist == wanted)
            .map(|(uid, _)| *uid)
            .collect();
        if anchors.is_empty() {
            continue;
        }
        let named: Vec<String> = store
            .relations()
            .filter(|r| r.kind.to_string() == cargo_smysl_corpus::REL_TOUCHES)
            .filter(|r| anchors.contains(&r.to))
            .filter_map(|r| labels.get(&r.from)?.first().map(|l| l.to_string()))
            .collect();
        if !named.is_empty() {
            out.push((change.label.clone(), named));
        }
    }
    out
}

/// Whether `sha` is a descendant of `base`: the corpus holds commits, and `--since` asks for the recent
/// ones. A commit the repository cannot resolve is left in rather than silently dropped.
fn is_after(root: &Path, sha: &str, base: &str) -> bool {
    let Ok(commit) = cargo_smysl_git::read_commit(root, sha) else {
        return true;
    };
    commit.sha != base
}

// -------------------------------------------------------------------------------------------------
// hooks: git integration that costs nothing until you ask for it
// -------------------------------------------------------------------------------------------------

/// What the post-commit hook writes: a sha per line, in `.smysl/queue`.
///
/// It runs on every commit, so it must be instant and must never fail a commit. Appending a line is
/// both. Extraction — which is 7 to 30 minutes of a model — stays something a person starts.
const POST_COMMIT: &str = "#!/bin/sh\n\
# Written by `cargo smysl hooks install`.\n\
# Queues this commit for recording. It calls no model and cannot fail your commit.\n\
# Record what is queued when you choose to: cargo smysl extract --queued\n\
root=$(git rev-parse --show-toplevel) || exit 0\n\
mkdir -p \"$root/.smysl\" || exit 0\n\
git rev-parse HEAD >> \"$root/.smysl/queue\" 2>/dev/null || true\n\
exit 0\n";

fn queue_path(root: &Path) -> std::path::PathBuf {
    root.join(cargo_smysl_corpus::CORPUS_DIR).join("queue")
}

/// Write the queue back, always ending with a newline.
///
/// The hook appends with `>>`. A file that does not end in a newline therefore has the next sha welded
/// onto its last line, and the result is a revision no repository has ever heard of. That happened.
fn write_queue(path: &Path, shas: &[String]) {
    let mut text = shas.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    let _ = std::fs::write(path, text);
}

/// The shas in the queue, repairing lines that were welded together.
///
/// A line of several shas run end to end is exactly what a missing newline produces, and the shas in it
/// are still the shas someone wanted recorded — so they are split out rather than thrown away. Anything
/// that is not a plausible revision is reported and dropped, because guessing is worse.
fn read_queue(path: &Path) -> (Vec<String>, Vec<String>) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let (mut shas, mut refused) = (Vec::new(), Vec::new());
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let hex = line.chars().all(|c| c.is_ascii_hexdigit());
        match (hex, line.len()) {
            (true, n) if (7..=40).contains(&n) => shas.push(line.to_string()),
            // Several full-length revisions with the newline missing between them.
            (true, n) if n % 40 == 0 => {
                for chunk in line.as_bytes().chunks(40) {
                    shas.push(String::from_utf8_lossy(chunk).into_owned());
                }
            }
            _ => refused.push(line.to_string()),
        }
    }
    (shas, refused)
}

fn hooks(args: &SmyslArgs, what: &cli::HooksStep) -> u8 {
    let root = match workspace_root(args) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cargo smysl hooks: {e}");
            return exit::FAILURE;
        }
    };
    let hook = root.join(".git").join("hooks").join("post-commit");
    let attributes = root.join(".gitattributes");
    let ours = |text: &str| text.contains("cargo smysl hooks install");

    match what {
        cli::HooksStep::Status => {
            let installed = std::fs::read_to_string(&hook)
                .map(|t| ours(&t))
                .unwrap_or(false);
            println!(
                "post-commit hook: {}",
                if installed {
                    "installed (queues commits, calls no model)"
                } else if hook.exists() {
                    "a hook is there, but this tool did not write it"
                } else {
                    "not installed"
                }
            );
            let driver = std::fs::read_to_string(&attributes)
                .map(|t| t.contains("merge=smysl"))
                .unwrap_or(false);
            println!(
                "merge driver: {}",
                if driver {
                    "registered in .gitattributes (run `git config merge.smysl.driver …` per clone)"
                } else {
                    "not registered"
                }
            );
            let queued = std::fs::read_to_string(queue_path(&root))
                .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
                .unwrap_or(0);
            println!("queued: {queued} commit(s)");
            exit::OK
        }
        cli::HooksStep::Install { force } => {
            if hook.exists() {
                let existing = std::fs::read_to_string(&hook).unwrap_or_default();
                if !ours(&existing) && !force {
                    eprintln!(
                        "cargo smysl hooks: {} was written by something else; --force to replace it",
                        hook.display()
                    );
                    return exit::FAILURE;
                }
            }
            if let Some(dir) = hook.parent() {
                if let Err(e) = std::fs::create_dir_all(dir) {
                    eprintln!("cargo smysl hooks: {}: {e}", dir.display());
                    return exit::FAILURE;
                }
            }
            if let Err(e) = std::fs::write(&hook, POST_COMMIT) {
                eprintln!("cargo smysl hooks: {}: {e}", hook.display());
                return exit::FAILURE;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755));
            }
            println!("post-commit hook written to {}", hook.display());

            // The corpus documents are append-only records, so two branches recording different commits
            // is not a conflict — it is a merge smysl already knows how to do.
            let line = "*.smy merge=smysl\n";
            let existing = std::fs::read_to_string(&attributes).unwrap_or_default();
            if !existing.contains("merge=smysl") {
                if let Err(e) = std::fs::write(&attributes, format!("{existing}{line}")) {
                    eprintln!("cargo smysl hooks: {}: {e}", attributes.display());
                    return exit::FAILURE;
                }
                println!("registered `*.smy merge=smysl` in {}", attributes.display());
            }
            println!(
                "\nand once per clone, because git keeps drivers out of the repository:\n  \
                 git config merge.smysl.name 'smysl corpus'\n  \
                 git config merge.smysl.driver 'cargo smysl merge-driver %O %A %B'"
            );
            exit::OK
        }
        cli::HooksStep::Uninstall => {
            if std::fs::read_to_string(&hook)
                .map(|t| ours(&t))
                .unwrap_or(false)
            {
                let _ = std::fs::remove_file(&hook);
                println!("removed {}", hook.display());
            } else {
                println!("no hook of ours to remove");
            }
            println!("leave or remove `*.smy merge=smysl` in .gitattributes as you prefer");
            exit::OK
        }
    }
}

/// The merge driver git calls for `*.smy`: merge two versions of a corpus document.
///
/// Records are append-only and smysl's merge is idempotent and order-independent, so two branches that
/// recorded different commits merge without anyone choosing. A document one side could not parse is
/// left to a person rather than half-merged.
fn merge_driver(base: &Path, ours: &Path, theirs: &Path) -> u8 {
    let read = |p: &Path| std::fs::read_to_string(p).unwrap_or_default();
    let parse = |text: &str, what: &str| match smysl::parse_surface(text) {
        // Text we understood nothing of is not an empty document. Rewriting it would replace a person's
        // file with nothing, so it is refused and git is told the merge failed.
        Ok(parsed) if parsed.records.is_empty() && !text.trim().is_empty() => {
            eprintln!(
                "cargo smysl merge-driver: {what} has content but no records; leaving it to you"
            );
            None
        }
        Ok(parsed) => Some(parsed),
        Err(e) => {
            eprintln!("cargo smysl merge-driver: {what} does not parse ({e}); leaving it to you");
            None
        }
    };
    let (Some(mine), Some(other)) = (
        parse(&read(ours), "our version"),
        parse(&read(theirs), "their version"),
    ) else {
        return exit::FAILURE;
    };
    let _ = base;

    let mut store = smysl::Store::from_records(mine.records.clone());
    let incoming = smysl::Store::from_records(other.records.clone());
    if smysl::merge(&mut store, &incoming, smysl::MergeOptions::default()).is_err() {
        eprintln!("cargo smysl merge-driver: the two versions do not merge; leaving it to you");
        return exit::FAILURE;
    }
    let mut labels = mine.labels.clone();
    labels.extend(other.labels.clone());
    let records: Vec<smysl::Record> = store.iter().cloned().collect();
    let ctx = smysl::WriteContext::from_labels(&labels);
    let merged = smysl::write_surface(None, &records, &ctx);
    match std::fs::write(ours, merged) {
        Ok(()) => exit::OK,
        Err(e) => {
            eprintln!("cargo smysl merge-driver: {}: {e}", ours.display());
            exit::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::records_only_the_corpus;
    use cargo_smysl_git::{ChangedFile, CommitData};

    fn commit(paths: &[&str]) -> CommitData {
        CommitData {
            sha: "0".repeat(40),
            message: String::new(),
            files: paths
                .iter()
                .map(|p| ChangedFile {
                    path: (*p).to_string(),
                    before: None,
                    after: Some(String::new()),
                })
                .collect(),
            parent_missing: false,
        }
    }

    /// The queue is appended to by a shell hook and rewritten by this tool. Both must agree about the
    /// newline, and they did not: a rewrite that left the file without one had the next sha welded onto
    /// its last line, and `cargo smysl extract --queued` then asked git for a 120-character revision.
    #[test]
    fn a_queue_written_without_a_newline_does_not_weld_the_next_sha_on() {
        use super::{read_queue, write_queue};
        let path = std::env::temp_dir().join(format!("smysl-queue-{}.txt", std::process::id()));
        let a = "856a434cf607ac9d8cc73d7ba9f77ca7aef27e3d";
        let b = "0d79d4a840b4a08d9bd4e60085346a05d82a5daf";

        write_queue(&path, &[a.to_string()]);
        // What the hook does: append a line.
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&format!("{b}\n"));
        std::fs::write(&path, &text).unwrap();
        assert_eq!(read_queue(&path).0, vec![a.to_string(), b.to_string()]);

        // And the damage already done is repaired rather than thrown away.
        std::fs::write(&path, format!("{a}{b}\n")).unwrap();
        let (shas, refused) = read_queue(&path);
        assert_eq!(
            shas,
            vec![a.to_string(), b.to_string()],
            "welded shas are split"
        );
        assert!(refused.is_empty());

        // Something that is not a revision is reported, not guessed at.
        std::fs::write(&path, "not-a-sha\n").unwrap();
        let (shas, refused) = read_queue(&path);
        assert!(shas.is_empty());
        assert_eq!(refused, vec!["not-a-sha".to_string()]);

        std::fs::remove_file(&path).ok();
    }

    /// Recording a commit produces a commit. If that one is recorded too, the backlog never empties —
    /// which is what happened: every extraction left exactly one more commit to extract.
    #[test]
    fn a_commit_that_only_writes_the_corpus_is_not_work() {
        assert!(records_only_the_corpus(&commit(&[
            ".smysl/commits/abc.smy",
            ".smysl/extractions/v1/abc.json",
        ])));

        // A change that touches source is a change, whatever else it carries.
        assert!(!records_only_the_corpus(&commit(&[
            ".smysl/commits/abc.smy",
            "crates/cargo-smysl/src/main.rs",
        ])));
        assert!(!records_only_the_corpus(&commit(&["src/lib.rs"])));

        // A commit that changes nothing is not skipped on these grounds: it has its own oddity, and
        // pretending to know why it is empty is not this function's business.
        assert!(!records_only_the_corpus(&commit(&[])));

        // A path that merely starts with the same letters is not inside the corpus.
        assert!(!records_only_the_corpus(&commit(&[".smysl-notes/x.md"])));
    }
}
