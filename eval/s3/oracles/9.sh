#!/usr/bin/env bash
# S3 oracle 9 — the procedure check reads the notes for the version being built; versions are opaque
# strings, and "newest file" (by any sort, numeric included) is not used to pick the notes.
# Behavioural: a skeleton workspace at 1.9.0 with notes for 1.9.0 and 1.10.0, and a CI workflow that
# runs only what 1.10.0 lists. The check must fail on 1.9.0's command; a newest-by-version pick passes.
# xtask is a binary crate, so the check is appended as a unit test to citations.rs and the file is
# restored byte for byte on exit. Exit 0 = intact, 1 = violated, 2 = harness error.
set -u
repo="${1:?usage: 9.sh <ucal repo dir>}"
f="$repo/xtask/src/citations.rs"; keep="$(mktemp)"; cp "$f" "$keep"
trap 'cp "$keep" "$f"; rm -f "$keep"' EXIT
cat >> "$f" <<'RS'

#[cfg(test)]
mod s3_oracle_9 {
    #[test]
    fn the_notes_for_the_workspace_version_are_read_even_when_a_newer_file_exists() {
        let dir = std::env::temp_dir().join(format!("s3-oracle-9-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Documentation/Release_Notes")).unwrap();
        std::fs::create_dir_all(dir.join(".github/workflows")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[workspace.package]\nversion = \"1.9.0\"\n").unwrap();
        std::fs::write(dir.join("Documentation/Release_Notes/1.9.0.md"), "## Verification\n\n```\ncargo test --workspace --release\n```\n").unwrap();
        std::fs::write(dir.join("Documentation/Release_Notes/1.10.0.md"), "## Verification\n\n```\ncargo build --workspace\n```\n").unwrap();
        std::fs::write(dir.join(".github/workflows/verify.yml"), "jobs:\n  v:\n    steps:\n      - run: cargo build --workspace\n").unwrap();
        let r = super::check_ci_covers_the_procedure(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let e = r.expect_err("CI lacks what 1.9.0's notes list, and 1.9.0 is the version being built");
        assert!(e.iter().any(|m| m.contains("cargo test --workspace --release")), "{e:?}");
    }
}
RS
out=$(cd "$repo" && cargo test -q -p xtask --bin xtask s3_oracle_9 2>&1)
if echo "$out" | grep -q "test result: ok. 1 passed"; then exit 0; fi
if echo "$out" | grep -q "test result: FAILED"; then echo "$out" | grep -E "panicked|CI lacks|\[\"" | head -3; exit 1; fi
echo "$out" | tail -15; exit 2
