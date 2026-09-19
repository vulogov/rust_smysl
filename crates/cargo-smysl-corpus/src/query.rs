//! What rests on a recorded unit, which is the question the corpus exists to answer.
//!
//! "What breaks if this prerequisite stops holding?" walks the edges the other way: a prerequisite
//! `conditions` a decision (D3 — by an edge, so rewording the prerequisite does not move the decision's
//! uid), a decision `causes` its consequences, and evidence `backs` a finding. `EdgeSet::premises()` is
//! exactly that set, so the walk is smysl's, not ours.

use smysl::{
    dependents_via, label_index, resolve_label, EdgeSet, Label, LabelError, Status, Store, Uid,
};

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("{0}: {1}")]
    Label(Label, #[source] LabelError),
    #[error("{0} is not a label")]
    Malformed(String),
}

/// One unit that rests on the subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependent {
    pub uid: Uid,
    /// Every label bound to it, canonical order; empty for a unit the store names only by uid.
    pub labels: Vec<Label>,
    pub schema: String,
    pub status: Status,
    pub gist: String,
}

/// What rests on `label`: the decisions it conditions, and what those decisions cause, transitively.
///
/// Ordered as the walk found them, which is deterministic for a given store.
pub fn dependents_of(store: &Store, label: &str) -> Result<Vec<Dependent>, QueryError> {
    let label = Label::new(label.to_string()).map_err(|_| QueryError::Malformed(label.into()))?;
    let uid = resolve_label(store, &label).map_err(|e| QueryError::Label(label.clone(), e))?;
    Ok(rests_on_uid(store, uid))
}

/// The same walk from a uid, for a caller that already resolved one.
pub fn rests_on_uid(store: &Store, uid: Uid) -> Vec<Dependent> {
    let names = label_index(store);
    dependents_via(store, uid, &EdgeSet::premises())
        .into_iter()
        .filter(|u| *u != uid)
        .filter_map(|u| {
            let unit = store.get(&u)?;
            Some(Dependent {
                uid: u,
                labels: names.get(&u).cloned().unwrap_or_default(),
                schema: unit.core.schema.to_string(),
                status: unit.core.status,
                gist: unit.core.gist.clone(),
            })
        })
        .collect()
}
