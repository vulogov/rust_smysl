# cargo-smysl-facts

Deterministic facts about Rust code. Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why** Rust code changed and keeps the record honest.

What a function is, what it calls, what calls it, its control context and `cfg`, and a hash of its body — parsed with `syn`, cached, regenerable. No model.

Measured figures, design rules and the full documentation are in the [repository](https://github.com/vulogov/rust_smysl).

Licence: Unlicense.
