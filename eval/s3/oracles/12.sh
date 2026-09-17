#!/usr/bin/env bash
# S3 oracle 12 — a tick count is unsigned (Rule B): divergence is an exact unsigned magnitude plus a
# separate direction, never a signed number.
# Behavioural: a throwaway integration test binds the result as (Ticks, bool) and checks the exact
# magnitude of a one-second divergence in both directions. A signed return type fails to type-check
# (E0308); a lossy or saturating magnitude fails the assertion. Other compile errors are harness errors.
# Exit 0 = intact, 1 = violated, 2 = harness error.
set -u
repo="${1:?usage: 12.sh <ucal repo dir>}"
t="$repo/crates/ucal/tests/s3_oracle_12.rs"
trap 'rm -f "$t"' EXIT
cat > "$t" <<'RS'
use ucal::clock::{ticks_in_nanos, Session};
use ucal_core::backend::TickInt;
use ucal_core::{Instant, Ticks, UC1};

fn at(secs: u64) -> Instant<UC1> {
    Instant::from_ticks(ticks_in_nanos(u128::from(secs) * 1_000_000_000).unwrap()).unwrap()
}

#[test]
fn divergence_is_an_exact_unsigned_magnitude_and_a_direction() {
    let s = Session::anchored_at(&at(100));
    let one = ticks_in_nanos(1_000_000_000).unwrap();
    let four = ticks_in_nanos(4_000_000_000).unwrap();
    let (ahead, wall_ahead): (Ticks, bool) = s.divergence(&at(105), &four);
    assert!(wall_ahead);
    assert_eq!(ahead.to_dec_string(), one.to_dec_string());
    let (behind, wall_ahead): (Ticks, bool) = s.divergence(&at(103), &four);
    assert!(!wall_ahead);
    assert_eq!(behind.to_dec_string(), one.to_dec_string());
}
RS
out=$(cd "$repo" && cargo test -q -p ucal --test s3_oracle_12 2>&1)
if echo "$out" | grep -q "test result: ok. 1 passed"; then exit 0; fi
if echo "$out" | grep -qE "error\[E0308\]: mismatched types|test result: FAILED"; then echo "$out" | grep -E "^error|panicked|expected" | head -3; exit 1; fi
echo "$out" | tail -15; exit 2
