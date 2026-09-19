//! The fact cache: derived, regenerable, and never part of the corpus (D9).
//!
//! Facts are a function of one file's bytes and the extractor's version, so the cache is keyed by both
//! and a stale entry cannot be read as a fresh one. It lives in `.smysl/facts/`, which a repository may
//! ignore: deleting it costs a reparse, never a fact.

use std::path::{Path, PathBuf};

use crate::item::{facts, Fact, EXTRACTOR_VERSION};

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("{0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("{0}: {1}")]
    Parse(String, #[source] syn::Error),
    #[error("{0}: {1}")]
    Codec(PathBuf, String),
}

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    /// `<root>/.smysl/facts`.
    pub fn at(root: impl AsRef<Path>) -> Cache {
        Cache {
            dir: root.as_ref().join(".smysl").join("facts"),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The facts of one file, parsed once and kept.
    ///
    /// `source` is the file's exact bytes; the key is their hash with the extractor version, so a file
    /// edited back to an earlier state hits the earlier entry, which is correct — same bytes, same facts.
    pub fn facts_of(&self, path: &str, source: &str) -> Result<Vec<Fact>, CacheError> {
        let entry = self.entry(source);
        if let Ok(text) = std::fs::read_to_string(&entry) {
            match serde_json::from_str(&text) {
                Ok(facts) => return Ok(facts),
                // A corrupted or older-shaped entry is regenerated rather than trusted.
                Err(_) => {
                    let _ = std::fs::remove_file(&entry);
                }
            }
        }
        let facts = facts(path, source).map_err(|e| CacheError::Parse(path.to_string(), e))?;
        let text = serde_json::to_string(&facts)
            .map_err(|e| CacheError::Codec(entry.clone(), e.to_string()))?;
        if let Some(parent) = entry.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CacheError::Io(parent.to_path_buf(), e))?;
        }
        std::fs::write(&entry, text).map_err(|e| CacheError::Io(entry, e))?;
        Ok(facts)
    }

    /// Where one file's facts live: version, then the hash of its bytes.
    pub fn entry(&self, source: &str) -> PathBuf {
        let hash = smysl::hash_bytes(source.as_bytes());
        let hex: String = hash.iter().take(12).map(|b| format!("{b:02x}")).collect();
        self.dir
            .join(format!("v{EXTRACTOR_VERSION}"))
            .join(format!("{hex}.json"))
    }

    /// Remove everything, for a caller that wants the reparse.
    pub fn clear(&self) -> Result<(), CacheError> {
        if self.dir.exists() {
            std::fs::remove_dir_all(&self.dir).map_err(|e| CacheError::Io(self.dir.clone(), e))?;
        }
        Ok(())
    }
}
