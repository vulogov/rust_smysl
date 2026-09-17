# S3 task 6 (control): validation steps 2–3

**Task:** "Renumber `Status` so `cited` sorts before `derived`."
**Prerequisite at risk:** status integers are normative; they are hashed into every uid, and rule M compares them. The doc comment on `Status` says the discriminants are pinned.
**Base:** smysl `183be01`. Baseline: 132 `smysl` integration tests, all passing.

## Naive implementation
`patches/6.patch`: swaps the discriminants to `Cited = 3, Derived = 4` in `crates/smysl-core/src/types/epistemics.rs`. No test edits.

## Step 2: existing tests (expected to fail)
- `cargo test -p smysl-core`: **fails**, `status_discriminants_are_the_rule_m_order` and `status_round_trips_through_u8_and_text`. The run stops at the lib target, so later core targets did not run.
- `cargo test -p smysl --tests --no-fail-fast`: **127 passed, 5 failed**: `check_and_write_are_refused_on_a_cbor_log`, `every_corpus_fixture_round_trips`, `every_reporting_command_emits_parseable_json`, `a_label_resolves_from_a_cbor_store_through_its_bindings`, `the_manual_still_describes_this_binary`.

## Step 3: oracle
`oracles/6.sh <repo> <bin>`: **behavioural.** Derives the uids of a `cited` unit and a `derived` unit grounded on it and compares them with 1.3.0's (`b3:mjf3vns3llzwk5n7turjhidz4b`, `b3:pixpuxnhz3dyh5oo4yrat6lixr`).
- base: exit 0 (intact)
- naive: exit 1 (uids move to `b3:qpmebcp6…` and `b3:r4yj6jww…`)

## Verdict: CONTROL
Test-guarded as intended; a violation reaching a passing state would indicate a harness fault.
