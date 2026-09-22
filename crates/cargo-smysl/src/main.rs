//! `cargo smysl`: the cargo subcommand.
//!
//! Cargo runs an external subcommand as `cargo-smysl smysl <args…>`, with `$CARGO` pointing at the
//! cargo that invoked it. Running the binary directly (`cargo-smysl <args…>`) is accepted too, so
//! the same argument list works both ways.

mod cli;
mod commands;

use std::process::ExitCode;

use clap::Parser;
use human_panic::{setup_panic, Metadata};

/// Exit codes. `2` is clap's usage error; the rest are ours.
pub mod exit {
    pub const OK: u8 = 0;
    pub const FAILURE: u8 = 1;
    pub const NOT_IMPLEMENTED: u8 = 3;
}

fn main() -> ExitCode {
    // A panic here is a bug in this tool, not something the person running it did. They get a sentence
    // saying so, where to report it and where the details were written, instead of a backtrace — and
    // the report file means the details are not lost either.
    setup_panic!(Metadata::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
        .homepage("https://github.com/vulogov/rust_smysl")
        .support("Open an issue at https://github.com/vulogov/rust_smysl/issues with the report below."));
    let cli::Cargo::Smysl(args) = cli::Cargo::parse_from(normalise_args(std::env::args_os()));
    ExitCode::from(commands::run(args))
}

/// Insert the subcommand name when the binary was run directly rather than through cargo.
fn normalise_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    let mut args: Vec<_> = args.into_iter().collect();
    if args.get(1).map(|a| a != "smysl").unwrap_or(true) {
        args.insert(1.min(args.len()), "smysl".into());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::normalise_args;

    fn argv(xs: &[&str]) -> Vec<String> {
        normalise_args(xs.iter().map(Into::into))
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect()
    }

    #[test]
    fn cargo_style_invocation_is_left_alone() {
        assert_eq!(
            argv(&["cargo-smysl", "smysl", "doctor"]),
            ["cargo-smysl", "smysl", "doctor"]
        );
    }

    #[test]
    fn direct_invocation_gains_the_subcommand_name() {
        assert_eq!(
            argv(&["cargo-smysl", "doctor"]),
            ["cargo-smysl", "smysl", "doctor"]
        );
        assert_eq!(argv(&["cargo-smysl"]), ["cargo-smysl", "smysl"]);
    }
}
