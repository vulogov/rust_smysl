//! Deterministic facts about Rust code (plan D9–D10, Phase 2).
//!
//! `item::facts` is the extractor: per-function events with their control context, macro arguments,
//! doc comments, const values with array elements, struct fields and `cfg`. `functions` is the small
//! view of the same parse, for a caller that only wants the items.

pub mod cache;
pub mod item;
pub mod scope;

pub use cache::{Cache, CacheError};
pub use item::{facts, Constant, Event, EventKind, Fact, Structure, TypeAlias, EXTRACTOR_VERSION};
pub use scope::{render, select, Around, Reason, Selected};

use quote::ToTokens;
use syn::visit::{self, Visit};

/// A function or method, as `syn` sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// `impl` self type or inline module path, if any (e.g. `Session`, `tests`).
    pub owner: Option<String>,
    pub name: String,
    /// `private`, or the visibility tokens (`pub`, `pub (crate)`).
    pub visibility: String,
    /// Carries `#[test]`.
    pub is_test: bool,
    pub first_line: usize,
    pub last_line: usize,
}

impl Function {
    /// `Owner::name`, or `name`.
    pub fn label(&self) -> String {
        match &self.owner {
            Some(o) => format!("{o}::{}", self.name),
            None => self.name.clone(),
        }
    }
}

/// Every function and method in a source file. A file that does not parse is an error, not an
/// empty list: absence of facts must never read as a fact.
pub fn functions(source: &str) -> Result<Vec<Function>, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut v = Collector::default();
    v.visit_file(&file);
    Ok(v.out)
}

#[derive(Default)]
struct Collector {
    owners: Vec<String>,
    out: Vec<Function>,
}

fn visibility(v: &syn::Visibility) -> String {
    match v {
        syn::Visibility::Inherited => "private".into(),
        other => other.to_token_stream().to_string(),
    }
}

fn is_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("test"))
}

impl Collector {
    fn push(
        &mut self,
        sig: &syn::Signature,
        vis: String,
        attrs: &[syn::Attribute],
        block: &syn::Block,
    ) {
        self.out.push(Function {
            owner: (!self.owners.is_empty()).then(|| self.owners.join("::")),
            name: sig.ident.to_string(),
            visibility: vis,
            is_test: is_test(attrs),
            first_line: sig.fn_token.span.start().line,
            last_line: block.brace_token.span.close().end().line,
        });
    }
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.push(&i.sig, visibility(&i.vis), &i.attrs, &i.block);
        visit::visit_item_fn(self, i);
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        self.owners.push(i.self_ty.to_token_stream().to_string());
        visit::visit_item_impl(self, i);
        self.owners.pop();
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        self.push(&i.sig, visibility(&i.vis), &i.attrs, &i.block);
        visit::visit_impl_item_fn(self, i);
    }

    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        self.owners.push(i.ident.to_string());
        visit::visit_item_mod(self, i);
        self.owners.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "\
pub struct Session;
impl Session {
    pub fn reading(&self) -> u64 {
        0
    }
}
fn helper() {}
#[cfg(test)]
mod tests {
    #[test]
    fn a_reading_is_zero() {
        assert_eq!(super::Session.reading(), 0);
    }
}
";

    #[test]
    fn functions_carry_owner_visibility_test_marker_and_span() {
        let fns = functions(SRC).unwrap();
        let labels: Vec<_> = fns.iter().map(Function::label).collect();
        assert_eq!(
            labels,
            ["Session::reading", "helper", "tests::a_reading_is_zero"]
        );
        assert_eq!(fns[0].visibility, "pub");
        assert_eq!((fns[0].first_line, fns[0].last_line), (3, 5));
        assert!(!fns[1].is_test && fns[2].is_test);
    }

    #[test]
    fn a_file_that_does_not_parse_is_an_error() {
        assert!(functions("fn broken( {").is_err());
    }
}
