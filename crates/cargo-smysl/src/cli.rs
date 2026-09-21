use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// The shape cargo hands an external subcommand: `cargo-smysl smysl <args…>`.
#[derive(Parser, Debug)]
#[command(name = "cargo", bin_name = "cargo")]
pub enum Cargo {
    /// Record why Rust code changed, and keep that reasoning honest.
    #[command(version, author)]
    Smysl(SmyslArgs),
}

#[derive(Args, Debug)]
pub struct SmyslArgs {
    /// Path to Cargo.toml of the workspace to work on.
    #[arg(long, global = true, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,

    /// Print less.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Report what cargo-smysl sees: versions, the workspace, the corpus, and whether facts parse.
    Doctor,
    /// Extract deterministic facts about a commit's code into the fact cache.
    Facts {
        /// Commit or revision; defaults to the working tree.
        rev: Option<String>,
        /// Only these files (repository-relative). Every Rust file of the change otherwise.
        #[arg(long)]
        file: Vec<String>,
        /// Print the facts rather than a summary.
        #[arg(long)]
        json: bool,
        /// Select what bears on the change (D9) and print it as a model would read it: the touched
        /// items, anything these names mention, and one hop along calls.
        #[arg(long)]
        scope: bool,
        /// Identifiers to select around, with `--scope`.
        #[arg(long = "name")]
        names: Vec<String>,
        /// Hops outward along calls, with `--scope`.
        #[arg(long, default_value_t = 1)]
        hops: usize,
    },
    /// Extract decisions, prerequisites, alternatives and consequences for a commit, and record them.
    ///
    /// Once per commit and recipe (D7): a second run reads the kept extraction rather than asking again.
    Extract {
        /// Commit or revision; defaults to `HEAD`.
        rev: Option<String>,
        /// Record every commit after this revision instead of one, newest first. Resumable: a commit
        /// already recorded for this recipe is skipped.
        #[arg(long, conflicts_with = "rev")]
        since: Option<String>,
        /// Most commits to record in one run, with `--since`.
        #[arg(long, default_value_t = 10)]
        max_commits: usize,
        /// Say what a run would cost and stop. No model call.
        #[arg(long)]
        estimate: bool,
        /// Extract again even if this commit and recipe are already recorded.
        #[arg(long)]
        force: bool,
        /// Extract only: do not record into the corpus.
        #[arg(long)]
        dry_run: bool,
        /// Names the way of extracting, in the cache and the record.
        #[arg(long, env = "SMYSL_EXTRACT_RECIPE", default_value = "v1")]
        recipe: String,
        /// `ollama`, or `openai` for any OpenAI-compatible endpoint.
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
    /// Explain why an item is the way it is, from the corpus.
    Why {
        /// Item path or label, e.g. `crate::cli::cli` or `d/g90ec2f7-1`.
        #[arg(required_unless_present = "commit")]
        item: Option<String>,
        /// Read back everything recorded for a commit, rather than what rests on one item.
        #[arg(long, conflicts_with = "item")]
        commit: Option<String>,
        /// Markdown, for a pull request or a change log.
        #[arg(long)]
        markdown: bool,
    },
    /// Report what a change contradicts in the recorded corpus.
    ///
    /// Advisory: findings are printed and the exit code stays 0 unless `--strict` is given. Measured on
    /// held-out data with a local 14B: recall 0.38, precision 0.10 (10 of 96 flags correct), and 13% of
    /// ordinary commits drew a flag
    /// (docs/implementation-plan.md §4, S4).
    Check {
        /// Commit to check; defaults to `HEAD`.
        rev: Option<String>,
        /// A unified diff to check instead of a commit; `-` reads stdin.
        #[arg(long, conflicts_with = "rev")]
        patch: Option<String>,
        /// Exit 5 when there are findings, for a hook or a CI step.
        #[arg(long)]
        strict: bool,
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
        /// `ollama`, or `openai` for any OpenAI-compatible endpoint.
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
        /// Environment variable holding the provider's key, for one that needs it.
        #[arg(long, env = "SMYSL_CHECK_KEY_VAR", default_value = "")]
        key_var: String,
        /// Context window this model takes, prompt and answer together.
        #[arg(long, env = "SMYSL_CHECK_WINDOW", default_value_t = 32768)]
        window: u32,
        /// Characters per token for this model's tokenizer.
        #[arg(long, env = "SMYSL_CHECK_CHARS_PER_TOKEN", default_value_t = 2.0)]
        chars_per_token: f32,
        /// A file holding the judgement prompt, replacing the built-in one.
        #[arg(long, env = "SMYSL_CHECK_PROMPT_FILE")]
        prompt_file: Option<PathBuf>,
        /// Show what would be sent to the model — the units packed and the labels to be judged — and
        /// stop there. No model call, so it costs nothing and is the same every time.
        #[arg(long)]
        dry_run: bool,
        /// Passes over the change; a finding must be reported by every pass to be kept. Two is what the
        /// measured figures were taken at: it roughly halves the flags and costs some recall. One is
        /// faster and noisier.
        #[arg(long, env = "SMYSL_CHECK_PASSES", default_value_t = 2)]
        passes: usize,
    },
    /// Check a recorded claim against the code, and say what may be concluded (D11, D12).
    ///
    /// One claim at a time, with its own retrieved facts. A single run never raises a status: the verdict
    /// says what is covered, what is not, and what a second run or a person would settle.
    Evidence {
        /// The claim's label, e.g. `p/g90ec2f781421-8-1`.
        label: String,
        /// Names this run, so two runs can be told apart when agreement is counted.
        #[arg(long, default_value = "run-1")]
        run: String,
        /// Also shortlist the tests that might bear on it (D14).
        #[arg(long)]
        tests: bool,
        /// Run those tests, record what they did, and propose the edges (D13). Every edge waits for a
        /// person: `cargo smysl review`.
        #[arg(long, requires = "tests")]
        link: bool,
        /// Opt-in mutation gate (S1): change the code this claim is about, this many times, and ask
        /// whether each linking test notices. A test that notices nothing is recorded as exercising the
        /// code rather than verifying the claim. Slow: every change reruns the test.
        #[arg(long, requires = "link", default_value_t = 0, value_name = "CHANGES")]
        mutate: usize,
        /// `ollama`, or `openai` for any OpenAI-compatible endpoint.
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
        /// Characters per token for this model, used to keep the prompt inside the window.
        #[arg(long, env = "SMYSL_CHECK_CHARS_PER_TOKEN", default_value_t = 2.0)]
        chars_per_token: f32,
    },
    /// Measure extraction against your own labels, on your own commits, with your own model (D19).
    ///
    /// How good extraction is depends on the model you chose, so the only figure about your model is one
    /// you measure. Four steps: `init` writes a reading sheet and a label template per commit, you label
    /// them, `adjudicate` pairs what the tool extracted with what you labelled, `score` counts.
    Bench {
        #[command(subcommand)]
        step: BenchStep,
    },
    /// Report reasoning whose code has moved since it was recorded.
    ///
    /// A decision was made about code as it stood; when that code changes, the decision is not wrong,
    /// it is unexamined. This says which recorded commits rest on items that have since changed or
    /// gone. It withdraws nothing and lowers no status: a moved item is a question for a person.
    Stale {
        /// Only commits recorded at or after this revision, e.g. `v1.2.0`.
        #[arg(long)]
        since: Option<String>,
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Work through what is waiting for a person, and record the answer (D15).
    ///
    /// Nothing is deleted or rewritten: confirming writes an attestation, rejecting a withdrawal with a
    /// unit saying why, closing a resolution with a note.
    Review {
        /// Who is reviewing; their answer is attributed to them, not to the tool.
        #[arg(long, env = "SMYSL_REVIEW_AS")]
        as_person: Option<String>,
        /// Act on the item at this position in the queue.
        #[arg(long, requires = "as_person")]
        item: Option<usize>,
        /// Confirm it.
        #[arg(long, requires = "item", conflicts_with_all = ["reject", "close"])]
        confirm: bool,
        /// Reject it, with the reason.
        #[arg(long, requires = "item", conflicts_with = "close")]
        reject: Option<String>,
        /// Close a disagreement, with a note.
        #[arg(long, requires = "item")]
        close: Option<String>,
        /// Include what has already been dealt with.
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BenchStep {
    /// Write a reading sheet and a label template for each commit, under `.smysl/bench/`.
    Init {
        /// Commits to label; defaults to the last `--last` commits.
        revs: Vec<String>,
        /// How many recent commits to take when none are named.
        #[arg(long, default_value_t = 5)]
        last: usize,
        /// Lines of one file's diff on the sheet before it is cut.
        #[arg(long, default_value_t = 400)]
        file_lines: usize,
        /// Rewrite a template that already exists, losing what is in it.
        #[arg(long)]
        force: bool,
    },
    /// What is labelled, what is not, and what each unfinished label would let you score.
    Status,
    /// Pair what the tool extracted with what you labelled, for you to confirm.
    ///
    /// Reads the extraction the tool cached for each commit (`cargo smysl extract <rev>`), suggests a
    /// label for each extracted item by word overlap, and leaves `match` for you to set.
    Adjudicate {
        /// Which extraction to judge, by the recipe that produced it.
        #[arg(long, default_value = "v1")]
        recipe: String,
    },
    /// Precision and recall per kind, for your model on your commits.
    Score {
        #[arg(long, default_value = "v1")]
        recipe: String,
    },
}
