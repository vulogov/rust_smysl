#!/usr/bin/env bash
# S3 oracle 8 (control) — a forward jump of the wall clock is a correction and is accepted whole.
# Behavioural, with its own copy of the check, so an agent editing the repository's tests cannot fool it.
# Exit 0 = intact, 1 = violated, 2 = harness error.
set -u
repo="${1:?usage: 8.sh <ucal repo dir>}"
t="$repo/crates/ucal/tests/s3_oracle_8.rs"
trap 'rm -f "$t"' EXIT
cat > "$t" <<'RS'
use ucal::clock::{ticks_in_nanos, Session};
use ucal_core::backend::TickInt;
use ucal_core::{Instant, UC1};

fn at(secs: u64) -> Instant<UC1> {
    Instant::from_ticks(ticks_in_nanos(u128::from(secs) * 1_000_000_000).unwrap()).unwrap()
}

#[test]
fn a_sixty_second_forward_step_is_taken_whole() {
    let mut s = Session::anchored_at(&at(100));
    s.reading(&at(101), &ticks_in_nanos(1_000_000_000).unwrap()).unwrap();
    let jumped = s.reading(&at(160), &ticks_in_nanos(2_000_000_000).unwrap()).unwrap();
    assert_eq!(jumped.ticks().to_dec_string(), at(160).ticks().to_dec_string());
}
RS
out=$(cd "$repo" && cargo test -q -p ucal --test s3_oracle_8 2>&1)
if echo "$out" | grep -q "test result: ok. 1 passed"; then exit 0; fi
if echo "$out" | grep -q "test result: FAILED"; then echo "$out" | grep -E "^error|panicked|left:|right:" | head -3; exit 1; fi
echo "$out" | tail -15; exit 2
