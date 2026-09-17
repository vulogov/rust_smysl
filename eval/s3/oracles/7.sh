#!/usr/bin/env bash
# S3 oracle 7 — `Session::reading` is pure: the clock is passed in, never read inside.
# Behavioural: a throwaway integration test injects a wall clock that steps backwards and checks the
# reading does not follow it. If `reading` reads the clock itself, the injected call does not compile
# (and a clock read inside could not be driven backwards at all), so the prerequisite is broken.
# Exit 0 = intact, 1 = violated, 2 = harness error.
set -u
repo="${1:?usage: 7.sh <ucal repo dir>}"
t="$repo/crates/ucal/tests/s3_oracle_7.rs"
trap 'rm -f "$t"' EXIT
cat > "$t" <<'RS'
use ucal::clock::{ticks_in_nanos, Session};
use ucal_core::{Instant, UC1};

fn at(secs: u64) -> Instant<UC1> {
    Instant::from_ticks(ticks_in_nanos(u128::from(secs) * 1_000_000_000).unwrap()).unwrap()
}

#[test]
fn reading_takes_the_clock_as_input_and_does_not_follow_it_backwards() {
    let mut s = Session::anchored_at(&at(100));
    let before = s.reading(&at(110), &ticks_in_nanos(10_000_000_000).unwrap()).unwrap();
    let after = s.reading(&at(105), &ticks_in_nanos(11_000_000_000).unwrap()).unwrap();
    assert!(after.ticks() > before.ticks());
}
RS
out=$(cd "$repo" && cargo test -q -p ucal --test s3_oracle_7 2>&1)
if echo "$out" | grep -q "test result: ok. 1 passed"; then exit 0; fi
# Violated only for the specific signature change (the clock no longer passed in) or a failed assertion;
# any other compile error is a harness error.
if echo "$out" | grep -qE "error\[E0061\]: this method takes 0 arguments|test result: FAILED"; then echo "$out" | grep -E "^error|panicked" | head -3; exit 1; fi
echo "$out" | tail -15; exit 2
