//! What a Rust file says, as facts a claim can be checked against (plan D9, D10).
//!
//! Ported from the research extractor, which was JSON and is now typed. Nothing here knows about any
//! particular prerequisite: for every function it records what the function *does* — calls, method
//! calls, field reads, string literals, macros, bindings — and the control context each sits in, so
//! "returns an error when the file is missing" can be checked against a branch rather than a whole body.
//!
//! **Prose never verifies (D10).** Doc comments, string literals and string-valued constants are
//! author text: they say what someone believed, not what the code does. They are extracted, because a
//! model needs them to find candidates, and every one of them is tagged so no verdict can rest on one.

use proc_macro2::Span;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use syn::visit::{self, Visit};

/// The extractor's own version. A fact cache is keyed by it, so a change here regenerates everything
/// rather than mixing shapes.
pub const EXTRACTOR_VERSION: u32 = 1;

/// One thing a function does, with where it sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    pub line: usize,
    /// The control context, outermost first: `if …`, `arm …`, `for … in …`, `let-else …`, `closure`.
    pub ctx: Vec<String>,
    /// The event's text, shaped by its kind (callee and arguments, method and receiver, …).
    pub detail: Vec<(String, String)>,
    /// Author prose (D10): a string literal or doc text, which never verifies a claim.
    pub prose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    Call,
    Method,
    Field,
    Let,
    Macro,
    Str,
}

/// A fact about one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "item", rename_all = "kebab-case")]
pub enum Fact {
    Function(Function),
    Const(Constant),
    Struct(Structure),
    TypeAlias(TypeAlias),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    pub file: String,
    /// `impl` self type or module path (`Session`, `tests`).
    pub owner: Option<String>,
    pub name: String,
    pub visibility: String,
    /// `#[cfg(…)]` on the item, and `#[test]`.
    pub cfg: Vec<String>,
    /// The file's own `#![cfg(…)]`.
    pub file_cfg: Vec<String>,
    /// Doc comment: prose (D10).
    pub doc: String,
    pub line: usize,
    pub end: usize,
    pub params: String,
    pub returns: String,
    /// The body's tokens hashed, so a moved-but-unchanged function is recognised.
    pub body_hash: String,
    pub events: Vec<Event>,
}

impl Function {
    pub fn label(&self) -> String {
        match &self.owner {
            Some(o) => format!("{o}::{}", self.name),
            None => self.name.clone(),
        }
    }

    pub fn is_test(&self) -> bool {
        self.cfg.iter().any(|c| c == "test")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constant {
    pub file: String,
    pub owner: Option<String>,
    pub name: String,
    /// `const` or `static`.
    pub keyword: String,
    pub ty: String,
    pub cfg: Vec<String>,
    pub file_cfg: Vec<String>,
    pub doc: String,
    pub line: usize,
    pub value: String,
    /// Elements, when the value is an array — the common shape of a table a claim counts.
    pub elements: Option<Vec<String>>,
    /// A string-valued constant is author prose (D10).
    pub prose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Structure {
    pub file: String,
    pub name: String,
    pub visibility: String,
    pub cfg: Vec<String>,
    pub file_cfg: Vec<String>,
    pub doc: String,
    pub line: usize,
    pub fields: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeAlias {
    pub file: String,
    pub name: String,
    pub line: usize,
    pub ty: String,
}

/// Every fact in one file. A file that does not parse is an error, not an empty list: absence of facts
/// must never read as a fact.
pub fn facts(file: &str, source: &str) -> Result<Vec<Fact>, syn::Error> {
    let parsed = syn::parse_file(source)?;
    let file_cfg = cfgs(&parsed.attrs);
    let mut out = Vec::new();
    items(file, &parsed.items, None, &file_cfg, &mut out);
    Ok(out)
}

fn tokens(t: &impl ToTokens, max: usize) -> String {
    let s = t.to_token_stream().to_string();
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

fn line_of(span: Span) -> usize {
    span.start().line
}

fn cfgs(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("test"))
        .map(|a| tokens(&a.meta, usize::MAX))
        .collect()
}

fn docs(attrs: &[syn::Attribute]) -> String {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => Some(s.value()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn visibility(v: &syn::Visibility) -> String {
    match v {
        syn::Visibility::Inherited => "private".into(),
        other => tokens(other, 40),
    }
}

#[derive(Default)]
struct Body {
    events: Vec<Event>,
    ctx: Vec<String>,
}

impl Body {
    fn push(&mut self, kind: EventKind, line: usize, detail: Vec<(String, String)>, prose: bool) {
        self.events.push(Event {
            kind,
            line,
            ctx: self.ctx.clone(),
            detail,
            prose,
        });
    }
}

fn detail(pairs: [(&str, String); 2]) -> Vec<(String, String)> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

impl<'ast> Visit<'ast> for Body {
    fn visit_expr_if(&mut self, i: &'ast syn::ExprIf) {
        self.visit_expr(&i.cond);
        self.ctx.push(format!("if {}", tokens(&*i.cond, 80)));
        self.visit_block(&i.then_branch);
        self.ctx.pop();
        if let Some((_, e)) = &i.else_branch {
            self.ctx.push(format!("else-of {}", tokens(&*i.cond, 80)));
            self.visit_expr(e);
            self.ctx.pop();
        }
    }

    fn visit_arm(&mut self, i: &'ast syn::Arm) {
        self.ctx.push(format!("arm {}", tokens(&i.pat, 80)));
        visit::visit_arm(self, i);
        self.ctx.pop();
    }

    fn visit_expr_for_loop(&mut self, i: &'ast syn::ExprForLoop) {
        self.visit_expr(&i.expr);
        self.ctx.push(format!(
            "for {} in {}",
            tokens(&*i.pat, 40),
            tokens(&*i.expr, 80)
        ));
        self.visit_block(&i.body);
        self.ctx.pop();
    }

    fn visit_expr_while(&mut self, i: &'ast syn::ExprWhile) {
        self.ctx.push(format!("while {}", tokens(&*i.cond, 80)));
        visit::visit_expr_while(self, i);
        self.ctx.pop();
    }

    fn visit_expr_closure(&mut self, i: &'ast syn::ExprClosure) {
        self.ctx.push("closure".into());
        visit::visit_expr_closure(self, i);
        self.ctx.pop();
    }

    fn visit_local(&mut self, i: &'ast syn::Local) {
        if let Some(init) = &i.init {
            self.visit_expr(&init.expr);
            if let Some((_, diverge)) = &init.diverge {
                self.ctx.push(format!("let-else {}", tokens(&i.pat, 60)));
                self.visit_expr(diverge);
                self.ctx.pop();
            }
        }
        let init = i
            .init
            .as_ref()
            .map(|x| tokens(&*x.expr, 600))
            .unwrap_or_default();
        self.push(
            EventKind::Let,
            line_of(i.let_token.span),
            detail([("pat", tokens(&i.pat, 80)), ("init", init)]),
            false,
        );
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        self.push(
            EventKind::Call,
            line_of(i.paren_token.span.open()),
            detail([
                ("callee", tokens(&*i.func, 120)),
                ("args", tokens(&i.args, 400)),
            ]),
            false,
        );
        visit::visit_expr_call(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        self.push(
            EventKind::Method,
            line_of(i.method.span()),
            vec![
                ("method".into(), i.method.to_string()),
                ("receiver".into(), tokens(&*i.receiver, 200)),
                ("args".into(), tokens(&i.args, 400)),
            ],
            false,
        );
        visit::visit_expr_method_call(self, i);
    }

    fn visit_expr_field(&mut self, i: &'ast syn::ExprField) {
        self.push(
            EventKind::Field,
            line_of(i.dot_token.span),
            detail([
                ("base", tokens(&*i.base, 80)),
                ("member", tokens(&i.member, 40)),
            ]),
            false,
        );
        visit::visit_expr_field(self, i);
    }

    fn visit_lit_str(&mut self, i: &'ast syn::LitStr) {
        // Prose: what the code says about itself, not what it does (D10).
        self.push(
            EventKind::Str,
            line_of(i.span()),
            vec![("text".into(), i.value())],
            true,
        );
    }

    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        self.push(
            EventKind::Macro,
            line_of(i.path.segments[0].ident.span()),
            detail([
                ("name", tokens(&i.path, 40)),
                ("tokens", tokens(&i.tokens, 200)),
            ]),
            false,
        );
        // A macro body is a token stream, not syntax. Most macros in ordinary code take
        // comma-separated expressions (`format!`, `assert!`, `vec!`, `write!`), so parse those and the
        // calls inside them stay visible. Anything else stays opaque rather than guessed at.
        let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        if let Ok(args) = syn::parse::Parser::parse2(parser, i.tokens.clone()) {
            for a in &args {
                self.visit_expr(a);
            }
        }
    }
}

/// Where an item sits: the file, its enclosing `cfg`, and the impl or module that owns it.
#[derive(Clone)]
struct Where<'a> {
    file: &'a str,
    owner: Option<String>,
    file_cfg: &'a [String],
}

fn function(
    at: &Where<'_>,
    sig: &syn::Signature,
    vis: String,
    attrs: &[syn::Attribute],
    block: &syn::Block,
) -> Fact {
    let (file, owner, file_cfg) = (at.file, at.owner.clone(), at.file_cfg);
    let mut body = Body::default();
    body.visit_block(block);
    let text = block.to_token_stream().to_string();
    let hash = smysl::hash_bytes(text.as_bytes());
    Fact::Function(Function {
        file: file.to_string(),
        owner,
        name: sig.ident.to_string(),
        visibility: vis,
        cfg: cfgs(attrs),
        file_cfg: file_cfg.to_vec(),
        doc: docs(attrs),
        line: line_of(sig.fn_token.span),
        end: block.brace_token.span.close().end().line,
        params: tokens(&sig.inputs, 200),
        returns: tokens(&sig.output, 80),
        body_hash: hash.iter().take(8).map(|b| format!("{b:02x}")).collect(),
        events: body.events,
    })
}

fn constant(
    at: &Where<'_>,
    keyword: &str,
    ident: &syn::Ident,
    ty: &syn::Type,
    expr: &syn::Expr,
    attrs: &[syn::Attribute],
) -> Fact {
    let (file, owner, file_cfg) = (at.file, at.owner.clone(), at.file_cfg);
    // An array, possibly behind `&`, is the common table shape: keep its elements so a claim that
    // counts them can be checked.
    let mut e = expr;
    if let syn::Expr::Reference(r) = e {
        e = &r.expr;
    }
    let elements = match e {
        syn::Expr::Array(a) => Some(a.elems.iter().map(|x| tokens(x, 300)).collect()),
        _ => None,
    };
    let prose = matches!(
        e,
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(_),
            ..
        })
    );
    Fact::Const(Constant {
        file: file.to_string(),
        owner,
        name: ident.to_string(),
        keyword: keyword.to_string(),
        ty: tokens(ty, 80),
        cfg: cfgs(attrs),
        file_cfg: file_cfg.to_vec(),
        doc: docs(attrs),
        line: line_of(ident.span()),
        value: tokens(expr, 300),
        elements,
        prose,
    })
}

fn here<'a>(file: &'a str, owner: Option<String>, file_cfg: &'a [String]) -> Where<'a> {
    Where {
        file,
        owner,
        file_cfg,
    }
}

fn items(
    file: &str,
    list: &[syn::Item],
    owner: Option<String>,
    file_cfg: &[String],
    out: &mut Vec<Fact>,
) {
    for it in list {
        match it {
            syn::Item::Fn(f) => out.push(function(
                &here(file, owner.clone(), file_cfg),
                &f.sig,
                visibility(&f.vis),
                &f.attrs,
                &f.block,
            )),
            syn::Item::Impl(i) => {
                let who = tokens(&*i.self_ty, 60);
                for ii in &i.items {
                    match ii {
                        syn::ImplItem::Fn(m) => out.push(function(
                            &here(file, Some(who.clone()), file_cfg),
                            &m.sig,
                            visibility(&m.vis),
                            &m.attrs,
                            &m.block,
                        )),
                        syn::ImplItem::Const(k) => out.push(constant(
                            &here(file, Some(who.clone()), file_cfg),
                            "const",
                            &k.ident,
                            &k.ty,
                            &k.expr,
                            &k.attrs,
                        )),
                        _ => {}
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    let name = format!(
                        "{}{}",
                        owner.clone().map(|o| o + "::").unwrap_or_default(),
                        m.ident
                    );
                    // A `#[cfg(test)]` module marks everything in it, which is how a test is told from
                    // the code it exercises.
                    let mut inner_cfg = file_cfg.to_vec();
                    inner_cfg.extend(cfgs(&m.attrs));
                    items(file, inner, Some(name), &inner_cfg, out);
                }
            }
            syn::Item::Const(c) => out.push(constant(
                &here(file, None, file_cfg),
                "const",
                &c.ident,
                &c.ty,
                &c.expr,
                &c.attrs,
            )),
            syn::Item::Static(s) => out.push(constant(
                &here(file, None, file_cfg),
                "static",
                &s.ident,
                &s.ty,
                &s.expr,
                &s.attrs,
            )),
            syn::Item::Struct(s) => out.push(Fact::Struct(Structure {
                file: file.to_string(),
                name: s.ident.to_string(),
                visibility: visibility(&s.vis),
                cfg: cfgs(&s.attrs),
                file_cfg: file_cfg.to_vec(),
                doc: docs(&s.attrs),
                line: line_of(s.ident.span()),
                fields: s
                    .fields
                    .iter()
                    .map(|f| {
                        (
                            f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default(),
                            tokens(&f.ty, 120),
                        )
                    })
                    .collect(),
            })),
            syn::Item::Type(t) => out.push(Fact::TypeAlias(TypeAlias {
                file: file.to_string(),
                name: t.ident.to_string(),
                line: line_of(t.ident.span()),
                ty: tokens(&*t.ty, 160),
            })),
            _ => {}
        }
    }
}
