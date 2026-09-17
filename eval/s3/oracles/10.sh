#!/usr/bin/env bash
# S3 oracle 10 (control) — a version bumped without its notes is refused, not answered with another
# version's notes. Behavioural: a skeleton workspace at 9.9.9 with notes only for 1.0.0, whose command CI
# does run, so a fallback would pass. The check must refuse with the missing-notes diagnostic.
# Appended as a unit test to citations.rs, restored on exit. Exit 0 = intact, 1 = violated, 2 = harness.
set -u
repo="${1:?usage: 10.sh <ucal repo dir>}"
f="$repo/xtask/src/citations.rs"; keep="$(mktemp)"; cp "$f" "$keep"
trap 'cp "$keep" "$f"; rm -f "$keep"' EXIT
cat >> "$f" <<'RS'

#[cfg(test)]
mod s3_oracle_10 {
    #[test]
    fn a_version_without_notes_is_refused_even_when_other_notes_would_pass() {
        let dir = std::env::temp_dir().join(format!("s3-oracle-10-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Documentation/Release_Notes")).unwrap();
        std::fs::create_dir_all(dir.join(".github/workflows")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[workspace.package]\nversion = \"9.9.9\"\n").unwrap();
        std::fs::write(dir.join("Documentation/Release_Notes/1.0.0.md"), "## Verification\n\n```\ncargo build --workspace\n```\n").unwrap();
        std::fs::write(dir.join(".github/workflows/verify.yml"), "jobs:\n  v:\n    steps:\n      - run: cargo build --workspace\n").unwrap();
        let r = super::check_ci_covers_the_procedure(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let e = r.expect_err("9.9.9 has no notes; another version's must not stand in");
        assert!(e.iter().any(|m| m.contains("9.9.9") && m.contains("does not exist")), "{e:?}");
    }
}
RS
out=$(cd "$repo" && cargo test -q -p xtask --bin xtask s3_oracle_10 2>&1)
if echo "$out" | grep -q "test result: ok. 1 passed"; then exit 0; fi
if echo "$out" | grep -q "test result: FAILED"; then echo "$out" | grep -E "panicked|must not stand|\[\"" | head -3; exit 1; fi
echo "$out" | tail -15; exit 2
