//! A commit's own text, read in-process with gix: its message, and the before/after text of every file
//! it changes. This is what extracted quotes are checked against (plan D8), so it must not depend on a
//! `git` binary being installed (plan §1, self-contained).
//!
//! Changed files are found by comparing the blob ids of the commit's tree with its first parent's.
//! A root commit, or a shallow clone whose parent object is absent, compares against an empty tree.

use std::collections::BTreeMap;
use std::path::Path;

use gix::bstr::ByteSlice;

/// Largest file whose text is kept; larger files are listed but carry no text.
pub const MAX_FILE_BYTES: usize = 1 << 20;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("cannot open repository {path}: {source}")]
    Open {
        path: String,
        #[source]
        source: Box<gix::open::Error>,
    },
    #[error("cannot resolve revision {rev}: {message}")]
    Revision { rev: String, message: String },
    #[error("cannot read object: {0}")]
    Object(String),
}

/// One file the commit changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    /// Text before the commit; `None` when added, binary, or larger than [`MAX_FILE_BYTES`].
    pub before: Option<String>,
    /// Text after the commit; `None` when deleted, binary, or larger than [`MAX_FILE_BYTES`].
    pub after: Option<String>,
}

impl ChangedFile {
    /// Before and after text together: the text a quote from this file may come from.
    pub fn text(&self) -> String {
        match (&self.before, &self.after) {
            (Some(b), Some(a)) => format!("{b}\n{a}"),
            (Some(t), None) | (None, Some(t)) => t.clone(),
            (None, None) => String::new(),
        }
    }
}

/// A commit's message and the files it changes, in path order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitData {
    /// Full hex id.
    pub sha: String,
    pub message: String,
    pub files: Vec<ChangedFile>,
    /// True when the first parent exists but its object is not in the repository (a shallow clone).
    pub parent_missing: bool,
}

/// Read a commit by any revision gix understands (`HEAD`, a sha prefix, a branch).
pub fn read_commit(repo: impl AsRef<Path>, rev: &str) -> Result<CommitData, GitError> {
    let path = repo.as_ref();
    let repo = gix::open(path).map_err(|e| GitError::Open {
        path: path.display().to_string(),
        source: Box::new(e),
    })?;
    let id = repo.rev_parse_single(rev).map_err(|e| GitError::Revision {
        rev: rev.to_string(),
        message: e.to_string(),
    })?;
    let commit = id
        .object()
        .map_err(|e| GitError::Object(e.to_string()))?
        .try_into_commit()
        .map_err(|e| GitError::Object(e.to_string()))?;
    let message = commit
        .message_raw()
        .map_err(|e| GitError::Object(e.to_string()))?
        .to_str_lossy()
        .into_owned();
    let tree = commit
        .tree_id()
        .map_err(|e| GitError::Object(e.to_string()))?
        .detach();

    let parent_id = commit.parent_ids().next().map(|p| p.detach());
    let (parent_tree, parent_missing) = match parent_id {
        None => (None, false),
        Some(pid) => match repo
            .try_find_object(pid)
            .map_err(|e| GitError::Object(e.to_string()))?
        {
            None => (None, true),
            Some(obj) => {
                let c = obj
                    .try_into_commit()
                    .map_err(|e| GitError::Object(e.to_string()))?;
                (
                    Some(
                        c.tree_id()
                            .map_err(|e| GitError::Object(e.to_string()))?
                            .detach(),
                    ),
                    false,
                )
            }
        },
    };

    let after = blobs(&repo, Some(tree))?;
    let before = blobs(&repo, parent_tree)?;
    let mut paths: Vec<&String> = after.keys().chain(before.keys()).collect();
    paths.sort();
    paths.dedup();

    let mut files = Vec::new();
    for p in paths {
        let (b, a) = (before.get(p), after.get(p));
        if b == a {
            continue;
        }
        files.push(ChangedFile {
            path: p.clone(),
            before: text(&repo, b)?,
            after: text(&repo, a)?,
        });
    }
    Ok(CommitData {
        sha: id.detach().to_string(),
        message,
        files,
        parent_missing,
    })
}

/// Every blob in a tree, by path.
fn blobs(
    repo: &gix::Repository,
    tree: Option<gix::ObjectId>,
) -> Result<BTreeMap<String, gix::ObjectId>, GitError> {
    let mut out = BTreeMap::new();
    let Some(tree) = tree else { return Ok(out) };
    let mut stack = vec![(String::new(), tree)];
    while let Some((prefix, id)) = stack.pop() {
        let obj = repo
            .find_object(id)
            .map_err(|e| GitError::Object(e.to_string()))?;
        let tree = obj
            .try_into_tree()
            .map_err(|e| GitError::Object(e.to_string()))?;
        let decoded = tree.decode().map_err(|e| GitError::Object(e.to_string()))?;
        for entry in decoded.entries {
            let name = entry.filename.to_str_lossy();
            let path = if prefix.is_empty() {
                name.into_owned()
            } else {
                format!("{prefix}/{name}")
            };
            let oid = entry.oid.to_owned();
            if entry.mode.is_tree() {
                stack.push((path, oid));
            } else if entry.mode.is_blob() {
                out.insert(path, oid);
            }
        }
    }
    Ok(out)
}

/// A blob's text, or `None` when absent, binary (a NUL in the first 8 KiB) or too large.
fn text(repo: &gix::Repository, id: Option<&gix::ObjectId>) -> Result<Option<String>, GitError> {
    let Some(id) = id else { return Ok(None) };
    let blob = repo
        .find_object(*id)
        .map_err(|e| GitError::Object(e.to_string()))?;
    let data = &blob.data;
    if data.len() > MAX_FILE_BYTES || data[..data.len().min(8192)].contains(&0) {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(data).into_owned()))
}
