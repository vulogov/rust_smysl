# What this tool trusts, and what it does not

A scan of the code against its own threat model, 2026-09-22, with what was found and what was changed.
`cargo-smysl` reads repositories that other people wrote, takes answers from a model, writes into a
working tree, runs `cargo test`, and can send code to a provider. Each of those is a boundary.

## What it trusts

| Input | Trusted? | What guards it |
|---|---|---|
| A commit's message and diff | **no** | read with `gix`, never executed; quotes are checked against it rather than believed |
| A model's answer | **no** | labels, sources and statuses are the tool's; a quote absent from the commit caps its unit at `speculative`; a cited fact or unit that was not shown is dropped; a test name that was not on the shortlist is dropped |
| `.smysl/` in a cloned repository | **no** | it travels with the repository, so it is someone else's data — see *Restoring a backup* below |
| The provider's reply | **no** | parsed as JSON into fixed shapes; an unreadable answer is a call that found nothing, and a truncated one is read up to its last whole item |
| Settings (provider, model, endpoint, prompt) | **yes** | they are the operator's own |

## What changed in this scan

**An interrupted mutation run could be made to write anywhere.** `.smysl/mutation-backup/` holds a file
per mutated source file, named by path with `/` written as `%`, and the next run restores each one by
decoding that name. Nothing checked where the decoded name pointed, and `.smysl/` is tracked in git — so
a repository could carry `..%..%.zshrc.orig` and have it written outside the workspace by anyone who ran
`cargo smysl evidence --mutate` after cloning. Names are now refused unless they are relative, free of
`..`, and do not land on a symbolic link; a refused backup is reported rather than obeyed.

**Plain HTTP could carry a commit off the machine.** The default build has no TLS stack and speaks
HTTP/1.1 to a local provider. `--endpoint http://somewhere-else/` would have sent the commit — its
message and the text of its files — in the clear. A non-loopback host over plain HTTP is now refused with
what to do instead, rather than warned about: by the time a warning is read, the code has gone. A host
that merely looks local (`localhost.example.com`) is not local; the check is exact.

**Sending code to a hosted provider said nothing about it.** `extract` and `check` now say what they are
sending and where, once, before the first call, whenever the endpoint is not on this machine.

## What was checked and left alone

- **Secrets.** `--key-var` names the environment variable holding the key rather than taking the key, so
  it never appears in a command line, a shell history or a run's record. The key is read at the moment of
  the request and sent as a bearer token; errors carry the variable's *name*, never its value.
- **TLS.** The `hosted` feature uses `ureq` with certificate verification on. Nothing in this tree
  disables it, and the default build cannot make an HTTPS request at all.
- **Process execution.** The only program started is the `cargo` that invoked the tool (`$CARGO`), with
  arguments passed as a list — never through a shell. Test names reaching `--exact` come from the
  deterministic shortlist; a name a model invents is dropped before it is a candidate, let alone an
  argument.
- **Git.** Commits are read with `gix` as a library. The tool never runs `git`, so a repository cannot
  reach it through a hook, an alias, or `core.fsmonitor`.
- **The mutation gate writes to your source on purpose.** That is what it is for, and it is opt-in
  (`--mutate`), restores from a backup taken first, and restores again at the start of the next run.

## What is worth knowing rather than fixing

- **Extraction sends the commit, not a summary.** The message and the text of every changed file go to
  the model, fitted to its window. On a local provider that is a local matter. On a hosted one, your
  code is in someone else's logs under their retention policy, and no setting here changes that.
- **A corpus is a claim, not a proof.** Units carry the status the tool assigned when it recorded them,
  and a cloned repository's corpus says what its author's model said. Read `speculative` as what it is.
- **`cargo smysl hooks install` writes into `.git/hooks`**, and refuses to replace a hook this tool did
  not write unless `--force` is given.

## Reporting something

Open an issue at <https://github.com/vulogov/rust_smysl>. If it is a way to make this tool write, read or
send something it should not, say so in the issue and it will be treated as the priority.
