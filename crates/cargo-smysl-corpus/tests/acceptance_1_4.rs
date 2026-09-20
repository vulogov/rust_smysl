//! smysl 1.4.0 acceptance for requests R10 and R12 (docs/smysl-requests-1.4.md), on real staged batches and
//! test-result rows. Accepted against dev/1.4.0 before publish (docs/smysl-1.4.0-acceptance.md); held against the pin since.

use std::path::Path;

use cargo_smysl_corpus::{build, stage, CommitText, Extraction};
use smysl::{merge, to_cbor_seq, Label, LabelBinding, MergeOptions, Record, Store};

fn staged_store(sha: &str) -> Store {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../eval/extractions/research-pro-v2/smysl/{sha}.json"
    ));
    let ex: Extraction = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let batch = build(
        &ex,
        &CommitText {
            touched: Vec::new(),
            sha,
            message: "",
            files: vec![],
        },
        0,
    )
    .unwrap();
    let s = stage(&Store::from_records(Vec::new()), batch, 0);
    assert!(!s.has_errors());
    Store::from_records(s.records())
}

#[test]
fn r10_repeated_self_merge_appends_nothing() {
    let a = staged_store("90ec2f7");
    let n = a.iter().count();
    let kinds = |s: &Store| {
        let mut k = std::collections::BTreeMap::<&'static str, usize>::new();
        for r in s.iter() {
            *k.entry(match r {
                Record::Unit(_) => "unit",
                Record::Relation(_) => "relation",
                Record::Attestation(_) => "attestation",
                Record::LabelBinding(_) => "label",
                Record::SchemaDecl(_) => "schema",
                _ => "other",
            })
            .or_default() += 1;
        }
        k
    };
    eprintln!("staged batch: {n} records {:?}", kinds(&a));
    let mut aa = a.clone();
    for _ in 0..4 {
        let r = merge(&mut aa, &a, MergeOptions::default()).unwrap();
        assert_eq!(r.added, 0);
    }
    assert_eq!(aa.iter().count(), n);
}

#[test]
fn r10_two_commits_merged_both_ways_then_again_are_stable() {
    let a = staged_store("90ec2f7");
    let b = staged_store("532e4d2");
    let mut ab = a.clone();
    merge(&mut ab, &b, MergeOptions::default()).unwrap();
    let mut ba = b.clone();
    merge(&mut ba, &a, MergeOptions::default()).unwrap();
    assert_eq!(ab.iter().count(), ba.iter().count());
    let n = ab.iter().count();
    assert_eq!(
        merge(&mut ab, &ba, MergeOptions::default()).unwrap().added,
        0
    );
    assert_eq!(ab.iter().count(), n);
}

#[test]
fn r10_a_label_bound_to_a_different_uid_is_still_appended() {
    let a = staged_store("90ec2f7");
    let (label, other) = {
        let mut units = a.units().map(|(u, _)| *u);
        let first = units.next().unwrap();
        let second = units.next().unwrap();
        let label = a
            .iter()
            .find_map(|r| match r {
                Record::LabelBinding(b) if b.uid == first => Some(b.label.clone()),
                _ => None,
            })
            .unwrap_or_else(|| Label::new("d/acceptance").unwrap());
        (label, second)
    };
    let rival = Store::from_records(vec![Record::LabelBinding(LabelBinding::new(
        label.clone(),
        other,
    ))]);
    let mut merged = a.clone();
    let n = merged.iter().count();
    let r = merge(&mut merged, &rival, MergeOptions::default()).unwrap();
    assert_eq!(r.added, 1, "a rival binding is a distinct record");
    assert_eq!(merged.iter().count(), n + 1);
    assert!(
        !r.new_contentions.is_empty(),
        "the collision is detected from the store's own bindings"
    );
}

#[test]
fn r10_a_store_opened_from_a_file_merged_with_its_own_contents_appends_nothing() {
    let a = staged_store("532e4d2");
    let records: Vec<Record> = a.iter().cloned().collect();
    let dir = std::env::temp_dir().join(format!("acc14-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("store.cbor");
    std::fs::write(&path, to_cbor_seq(&records)).unwrap();
    let mut opened = Store::open(&path).unwrap();
    let n = opened.iter().count();
    assert_eq!(n, records.len());
    let r = merge(&mut opened, &a, MergeOptions::default()).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(r.added, 0);
    assert_eq!(opened.iter().count(), n);
}

/// R12: test evidence imported with `from_csv` must check clean, with every cell kept.
#[test]
fn r12_an_imported_reading_with_a_long_key_checks_clean_and_keeps_every_cell() {
    use smysl::{check, from_csv, AgentId, CheckOptions, Hlc, ImportOptions, Severity};
    let long_test = format!("tests::a_very_long_test_name_{}", "x".repeat(72));
    assert!(long_test.len() >= 100);
    let wide_cell = "y".repeat(300);
    let mut header: Vec<String> = [
        "test",
        "commit",
        "outcome",
        "run_seconds",
        "toolchain",
        "os",
        "profile",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut row: Vec<String> = [
        long_test.as_str(),
        "90ec2f7",
        "passed",
        "0.1",
        "1.94.1",
        "macos",
        "debug",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    // Wider than 23 columns, with one cell over 255 bytes.
    for i in 0..20 {
        header.push(format!("extra{i}"));
        row.push(if i == 0 {
            wide_cell.clone()
        } else {
            format!("v{i}")
        });
    }
    let csv = format!(
        "{}\n{}\nshort,90ec2f7,failed,0.2,1.94.1,macos,debug{}\n",
        header.join(","),
        row.join(","),
        (0..20).map(|i| format!(",w{i}")).collect::<String>()
    );
    let agent = AgentId::new("tool:cargo-smysl").unwrap();
    for key in [vec!["test".to_string()], vec![]] {
        let mut opts = ImportOptions::new("results.csv", agent.clone(), Hlc::zero(agent.clone()));
        opts.key = key.clone();
        let imported = from_csv(&csv, &opts);
        assert_eq!(
            imported.units.len(),
            2,
            "key {key:?}: {:?}",
            imported.diagnostics
        );
        let store = Store::from_records(imported.records());
        let report = check(&store, CheckOptions::default());
        let errors: Vec<String> = report
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.to_string())
            .collect();
        assert!(errors.is_empty(), "key {key:?}: {errors:?}");
        let payload = imported.units[0].payload.as_ref().expect("a payload");
        for cell in &row {
            assert!(
                payload.windows(cell.len()).any(|w| w == cell.as_bytes()),
                "key {key:?}: cell {:.40}… missing from the payload",
                cell
            );
        }
        for name in &header {
            assert!(
                payload.windows(name.len()).any(|w| w == name.as_bytes()),
                "column {name} missing"
            );
        }
    }
}
