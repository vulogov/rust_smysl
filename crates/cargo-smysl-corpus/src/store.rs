//! Where a repository's corpus lives, and how a commit is added to it.
//!
//! Two forms, because they answer different questions (plan §8, item 6):
//!
//! - **`.smysl/commits/<sha12>.smy`**, one surface document per commit. Text, so a reviewer can read
//!   what was recorded for a change and a diff of the corpus is a diff of prose.
//! - **`.smysl/store.cbor`**, every commit merged. The wire form, which is what queries and packing
//!   read, and what smysl's own merge rules apply to.
//!
//! The surface documents are the record; the store is derived from them and can be rebuilt
//! (`rebuild`). That order matters: surface text survives a format reader older than the one that
//! wrote it, and a store file does not.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use smysl::{
    from_cbor_seq, merge, parse_surface, to_cbor_seq, write_surface, Label, MergeOptions, Record,
    Staged, Store, Uid, WriteContext,
};

use crate::CORPUS_DIR;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("{0}: {1}")]
    Parse(PathBuf, String),
    #[error("{0}")]
    Smysl(String),
}

/// The corpus of one repository: `<root>/.smysl`.
pub struct Corpus {
    dir: PathBuf,
}

impl Corpus {
    /// The corpus under `root`, creating nothing until something is written.
    pub fn at(root: impl AsRef<Path>) -> Corpus {
        Corpus {
            dir: root.as_ref().join(CORPUS_DIR),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn store_path(&self) -> PathBuf {
        self.dir.join("store.cbor")
    }

    pub fn commit_path(&self, sha: &str) -> PathBuf {
        let short: String = sha.chars().take(12).collect();
        self.dir.join("commits").join(format!("{short}.smy"))
    }

    /// Every commit document, oldest name first.
    pub fn commit_documents(&self) -> Result<Vec<PathBuf>, StoreError> {
        let dir = self.dir.join("commits");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| StoreError::Io(dir.clone(), e))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "smy"))
            .collect();
        out.sort();
        Ok(out)
    }

    /// Write one commit's staged batch as surface text, and merge it into the store.
    ///
    /// Idempotent in the sense that matters: writing the same batch twice leaves the same documents and
    /// the same store, because smysl's merge recognises records it already holds (rule U, fixed for
    /// every record type in 1.4.0).
    pub fn record(&self, sha: &str, staged: &Staged) -> Result<Recorded, StoreError> {
        let doc = self.commit_path(sha);
        let parent = doc.parent().expect("commit path has a directory");
        std::fs::create_dir_all(parent).map_err(|e| StoreError::Io(parent.to_path_buf(), e))?;
        let text = surface(staged);
        std::fs::write(&doc, &text).map_err(|e| StoreError::Io(doc.clone(), e))?;

        let mut store = self.load()?;
        let before = store.iter().count();
        let batch = Store::from_records(staged.records());
        let report = merge(&mut store, &batch, MergeOptions::default())
            .map_err(|e| StoreError::Smysl(e.to_string()))?;
        self.save(&store)?;
        Ok(Recorded {
            document: doc,
            added: report.added,
            records: store.iter().count(),
            grew: store.iter().count() - before,
            contentions: report.new_contentions.len(),
        })
    }

    /// The merged store, empty when nothing has been recorded yet.
    pub fn load(&self) -> Result<Store, StoreError> {
        let path = self.store_path();
        if !path.exists() {
            return Ok(Store::from_records(Vec::new()));
        }
        let bytes = std::fs::read(&path).map_err(|e| StoreError::Io(path.clone(), e))?;
        let (records, _) =
            from_cbor_seq(&bytes).map_err(|e| StoreError::Parse(path.clone(), e.to_string()))?;
        Ok(Store::from_records(records))
    }

    /// Write records straight to the store, for a caller that appended to what it loaded — review, which
    /// adds attestations, withdrawals and resolutions rather than staging a batch.
    pub fn save_records(&self, records: &[Record]) -> Result<(), StoreError> {
        let path = self.store_path();
        std::fs::create_dir_all(&self.dir).map_err(|e| StoreError::Io(self.dir.clone(), e))?;
        std::fs::write(&path, to_cbor_seq(records)).map_err(|e| StoreError::Io(path, e))
    }

    fn save(&self, store: &Store) -> Result<(), StoreError> {
        let path = self.store_path();
        std::fs::create_dir_all(&self.dir).map_err(|e| StoreError::Io(self.dir.clone(), e))?;
        let records: Vec<Record> = store.iter().cloned().collect();
        std::fs::write(&path, to_cbor_seq(&records)).map_err(|e| StoreError::Io(path, e))
    }

    /// Rebuild the store from the commit documents, which are the record.
    ///
    /// For a store lost, corrupted, or written by a version whose merge appended what it already held
    /// (smysl 1.3, R10).
    pub fn rebuild(&self) -> Result<Store, StoreError> {
        let mut store = Store::from_records(Vec::new());
        for doc in self.commit_documents()? {
            let text = std::fs::read_to_string(&doc).map_err(|e| StoreError::Io(doc.clone(), e))?;
            let parsed =
                parse_surface(&text).map_err(|e| StoreError::Parse(doc.clone(), e.to_string()))?;
            let batch = Store::from_records(parsed.records);
            merge(&mut store, &batch, MergeOptions::default())
                .map_err(|e| StoreError::Smysl(e.to_string()))?;
        }
        self.save(&store)?;
        Ok(store)
    }

    /// Every label the store binds, uid to label (smysl 1.5, R17).
    pub fn labels(&self, store: &Store) -> BTreeMap<Uid, Vec<Label>> {
        smysl::label_index(store)
    }
}

/// What recording one commit did.
#[derive(Debug)]
pub struct Recorded {
    pub document: PathBuf,
    /// Records the merge appended.
    pub added: usize,
    /// Records in the store afterwards.
    pub records: usize,
    /// How much the store grew, which is 0 when the commit was already recorded.
    pub grew: usize,
    /// Labels this commit bound to a unit another commit binds differently.
    pub contentions: usize,
}

/// The staged batch as surface text, with its labels, so the document reads as it was authored.
pub fn surface(staged: &Staged) -> String {
    let ctx = WriteContext::from_labels(&staged.labels);
    write_surface(None, &staged.records(), &ctx)
}
