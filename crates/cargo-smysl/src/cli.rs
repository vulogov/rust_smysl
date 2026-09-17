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
    },
    /// Extract decisions, prerequisites, alternatives and consequences for a commit.
    Extract {
        /// Commit or revision.
        rev: Option<String>,
    },
    /// Explain why an item is the way it is, from the corpus.
    Why {
        /// Item path or label, e.g. `crate::cli::cli` or `d/g90ec2f7-1`.
        item: String,
    },
    /// Check the corpus: smysl rules, label ambiguity, declared extensions.
    Check,
    /// Link prerequisites to tests, run them at the commit, and record measured evidence.
    Evidence {
        /// Commit or revision.
        rev: Option<String>,
    },
    /// Report reasoning whose code has moved since it was recorded.
    Stale {
        /// Revision range, e.g. `v1.2.0..HEAD`.
        range: Option<String>,
    },
    /// Work through verdicts waiting for a person.
    Review,
}
