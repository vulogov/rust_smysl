use std::collections::BTreeSet;
use std::path::Path;

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
        Command::Extract {
            rev,
            force,
            dry_run,
            recipe,
            provider,
            model,
            endpoint,
            key_var,
            window,
        } => extract(
            &args,
            rev.as_deref(),
            *force,
            *dry_run,
            ExtractHow {
                recipe,
                provider,
                model,
                endpoint,
                key_var,
                window: *window,
            },
        ),
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
            passes,
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
            run,
            *tests,
            *link,
            *mutate,
            *chars_per_token,
            ExtractHow {
                recipe: "",
                provider,
                model,
                endpoint,
                key_var,
                window: *window,
            },
        ),
        Command::Stale { .. } => not_yet("stale", "a later phase (item 5, staleness)"),
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
fn evidence(
    args: &SmyslArgs,
    label: &str,
    run: &str,
    shortlist_tests: bool,
    link_tests: bool,
    mutate: usize,
    chars_per_token: f32,
    how: ExtractHow<'_>,
) -> u8 {
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
            return link_evidence(&root, &store, label, &claim, &picked, &all, &judge, mutate);
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
fn link_evidence(
    root: &Path,
    store: &smysl::Store,
    label: &str,
    claim: &str,
    shortlist: &[cargo_smysl_evidence::Candidate],
    facts: &[cargo_smysl_facts::Fact],
    judge: &ProviderJudge,
    mutate: usize,
) -> u8 {
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
            println!("{}: already extracted ({})", &commit.sha[..12], recipe.name);
            extraction
        }
        None => {
            // What the model sees: the commit as the tool read it, message then diff.
            let mut input = commit.message.clone();
            for file in &commit.files {
                input.push_str(&format!("\n\n--- {} ---\n{}", file.path, file.text()));
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
            match cargo_smysl_extract::extract(&input, &judge, &recipe) {
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
    let staged = cargo_smysl_corpus::stage(&store, batch, 0);
    let errors: Vec<String> = staged
        .report
        .iter()
        .filter(|d| d.severity == smysl::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    if !errors.is_empty() {
        for e in errors.iter().take(5) {
            eprintln!("cargo smysl extract: {e}");
        }
        return exit::FAILURE;
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
    // Two passes by default, which is the configuration the quoted figures were measured at.
    let seeds: Vec<usize> = cargo_smysl_verdict::check::AGREEMENT_SEEDS
        .iter()
        .copied()
        .cycle()
        .take(c.passes.max(1))
        .collect();
    let outcome = if seeds.len() > 1 {
        cargo_smysl_verdict::check::check_agreed(&store, &change, &judge, &settings, &seeds)
    } else {
        cargo_smysl_verdict::check(&store, &change, &judge, &settings)
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
