# cargo-smysl-extract

Extraction in passes, and the model client. Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why** Rust code changed and keeps the record honest.

Decisions first, then what surrounds each one. Fits the commit to the model's window, reads a large commit in parts, and reports anything cut or salvaged. The client speaks plain HTTP to a local provider; the `hosted` feature adds TLS.

Measured figures, design rules and the full documentation are in the [repository](https://github.com/vulogov/rust_smysl).

Licence: Unlicense.
