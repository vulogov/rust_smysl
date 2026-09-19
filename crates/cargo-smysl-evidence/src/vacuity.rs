//! Tests that cannot fail (spike S1).
//!
//! S1 measured mutation gating and found it too blunt to keep most valid edges, so it is opt-in. What it
//! did catch cheaply, and what ships instead, is the static check: a test whose assertion compares a
//! thing with itself, or asserts a literal truth, cannot fail, so an edge resting on it is worthless
//! however confidently it was proposed.
//!
//! This is deliberately narrow. It reports what it can prove from the tokens, not what it suspects: a
//! test it says nothing about may still be weak, and the report says so rather than implying a clean
//! bill of health.

use cargo_smysl_facts::item::Function;
use cargo_smysl_facts::EventKind;

/// Why a test cannot fail, in the words a report uses. Empty means nothing was proved either way.
pub fn vacuous(test: &Function) -> Vec<String> {
    let mut out = Vec::new();
    let mut assertions = 0;
    for event in test.events.iter().filter(|e| e.kind == EventKind::Macro) {
        let name = event
            .detail
            .iter()
            .find(|(k, _)| k == "name")
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        let tokens = event
            .detail
            .iter()
            .find(|(k, _)| k == "tokens")
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        match name {
            "assert_eq" | "assert_ne" => {
                assertions += 1;
                if let Some((left, right)) = split_two(tokens) {
                    if normalise(&left) == normalise(&right) {
                        out.push(format!(
                            "{name}! at :{} compares a thing with itself: `{left}`",
                            event.line
                        ));
                    }
                }
            }
            "assert" => {
                assertions += 1;
                let arg = first_arg(tokens);
                if matches!(normalise(&arg).as_str(), "true" | "1 == 1" | "! false") {
                    out.push(format!(
                        "assert! at :{} asserts a literal truth",
                        event.line
                    ));
                }
            }
            _ => {}
        }
    }
    if assertions == 0 && !test.events.is_empty() {
        out.push(
            "no assertion: it can only fail by panicking, which is a weaker thing than it looks"
                .into(),
        );
    }
    out
}

/// `a, b` at the top level, which is what a two-argument assertion looks like before formatting args.
fn split_two(tokens: &str) -> Option<(String, String)> {
    let (mut depth, mut parts, mut current) = (0i32, Vec::new(), String::new());
    for c in tokens.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    match parts.len() {
        0 | 1 => None,
        _ => Some((parts[0].clone(), parts[1].clone())),
    }
}

fn first_arg(tokens: &str) -> String {
    split_two(tokens)
        .map(|(a, _)| a)
        .unwrap_or_else(|| tokens.to_string())
}

/// Token text as `syn` prints it, with spacing collapsed: `a . b` and `a.b` are the same expression.
fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
