//! The functions of a C++ or C file with a body, for choosing one: free functions, methods
//! defined in the class or outside it (`void Cart::add(…)`), constructors, destructors,
//! operators and templates, grouped by their namespaces and classes as written (`shop::Cart`).

use std::collections::BTreeSet;
use std::ops::Range;

use tree_sitter::Node;

use super::parameter_names;
use crate::text::{block_comment, doc_lines, node_text, one_line};
use crate::{MethodRef, operator_name};

const CLASSES: [&str; 3] = ["class_specifier", "struct_specifier", "union_specifier"];

/// A function with a body.
pub struct Member<'t> {
    pub reference: MethodRef,
    /// The diagram's title: `add(a, b)`, `~Cart()`, `operator +=(o)`.
    pub title: String,
    pub doc: Option<Vec<String>>,
    pub body: Node<'t>,
    /// A constructor's `: x_(x), Base(x)`.
    pub initializers: Option<Node<'t>>,
    range: Range<usize>,
}

impl Member<'_> {
    /// Where it is in the file.
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
}

pub fn methods(root: Node, src: &str) -> Vec<MethodRef> {
    members(root, src).into_iter().map(|m| m.reference).collect()
}

/// The member `method` names.
pub fn find<'t>(root: Node<'t>, src: &str, method: &MethodRef) -> Option<Member<'t>> {
    members(root, src)
        .into_iter()
        .find(|m| m.reference.start == method.start && m.reference.name == method.name)
}

/// The names of the functions the file declares or defines (outside function bodies), for
/// telling the program's own calls from the library's.
pub fn function_names(root: Node, src: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "compound_statement" {
            continue;
        }
        if node.kind() == "function_declarator"
            && let Some(name) = node.child_by_field_name("declarator")
        {
            let name = match name.kind() {
                "qualified_identifier" => last_name(name),
                _ => name,
            };
            names.insert(node_text(name, src).to_owned());
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    names
}

fn members<'t>(root: Node<'t>, src: &str) -> Vec<Member<'t>> {
    let mut out = Vec::new();
    declarations(root, src, &mut Vec::new(), &mut out);
    out.sort_by_key(|m| m.reference.start);
    out
}

/// Functions among `node`'s children, in namespaces, classes and templates too.
fn declarations<'t>(node: Node<'t>, src: &str, scope: &mut Vec<String>, out: &mut Vec<Member<'t>>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "namespace_definition" => {
                let name = child.child_by_field_name("name").map(|n| node_text(n, src));
                if let Some(name) = name {
                    scope.push(name.to_owned());
                }
                if let Some(body) = child.child_by_field_name("body") {
                    declarations(body, src, scope, out);
                }
                if name.is_some() {
                    scope.pop();
                }
            }
            "linkage_specification" | "template_declaration" | "declaration_list" => {
                match child.child_by_field_name("body") {
                    Some(body) => declarations(body, src, scope, out),
                    None => declarations(child, src, scope, out),
                }
            }
            kind if CLASSES.contains(&kind) => class(child, src, scope, out),
            "declaration" | "field_declaration" => {
                if let Some(t) = child.child_by_field_name("type")
                    && CLASSES.contains(&t.kind())
                {
                    class(t, src, scope, out);
                }
            }
            "function_definition" => {
                if let Some(m) = function(child, src, scope) {
                    out.push(m);
                }
            }
            _ => {}
        }
    }
}

fn class<'t>(node: Node<'t>, src: &str, scope: &mut Vec<String>, out: &mut Vec<Member<'t>>) {
    let Some(body) = node.child_by_field_name("body") else { return };
    let name = node.child_by_field_name("name").map(|n| scope_name(n, src));
    scope.push(name.unwrap_or_default());
    declarations(body, src, scope, out);
    scope.pop();
}

fn function<'t>(node: Node<'t>, src: &str, scope: &[String]) -> Option<Member<'t>> {
    let body = node.child_by_field_name("body")?;
    let declarator = function_declarator(node.child_by_field_name("declarator")?)?;
    let named = declarator.child_by_field_name("declarator")?;
    let parameters = declarator.child_by_field_name("parameters");
    // `shop::Cart::run`: the scopes as written join the enclosing ones.
    let mut path: Vec<String> = scope.to_vec();
    let mut at = named;
    while at.kind() == "qualified_identifier" {
        if let Some(s) = at.child_by_field_name("scope") {
            path.push(scope_name(s, src));
        }
        match at.child_by_field_name("name") {
            Some(n) => at = n,
            None => break,
        }
    }
    let written = one_line(node_text(at, src));
    let names = parameter_names(parameters, src).join(", ");
    let (name, title) = match at.kind() {
        "operator_name" => {
            let op = written.trim_start_matches("operator").trim();
            (operator_name(op, names.contains(',')), format!("operator {op}({names})"))
        }
        "operator_cast" => ("op_Implicit".to_owned(), format!("{written}({names})")),
        "destructor_name" => (written.replace(' ', ""), format!("{}()", written.replace(' ', ""))),
        _ => (written.clone(), format!("{written}({names})")),
    };
    let start = node.child_by_field_name("type").map_or(named.start_byte(), |t| t.start_byte());
    let end = parameters.map_or(declarator.end_byte(), |p| p.end_byte());
    let mut cursor = node.walk();
    let initializers =
        node.named_children(&mut cursor).find(|c| c.kind() == "field_initializer_list");
    Some(Member {
        reference: MethodRef {
            class: if path.is_empty() { Vec::new() } else { vec![path.join("::")] },
            name,
            signature: one_line(&src[start..end.max(start)]),
            line: node.start_position().row + 1,
            start: node.start_byte(),
        },
        title,
        doc: doc(node, src),
        body,
        initializers,
        range: node.byte_range(),
    })
}

/// The function declarator inside a definition's declarator (`Cart& operator+=(…)` wraps it
/// in a reference declarator).
fn function_declarator(declarator: Node) -> Option<Node> {
    if declarator.kind() == "function_declarator" {
        return Some(declarator);
    }
    let inner = declarator.child_by_field_name("declarator").or_else(|| {
        let mut cursor = declarator.walk();
        declarator.named_children(&mut cursor).find(|c| !c.kind().ends_with("qualifier"))
    })?;
    function_declarator(inner)
}

/// A class or namespace name for grouping, without template arguments: `get_idx<Key, …>` →
/// `get_idx`, `SkipList<K, C>::Node` → `SkipList::Node`.
fn scope_name(node: Node, src: &str) -> String {
    match node.kind() {
        "template_type" => match node.child_by_field_name("name") {
            Some(name) => scope_name(name, src),
            None => one_line(node_text(node, src)),
        },
        "qualified_identifier" => {
            let parts = [node.child_by_field_name("scope"), node.child_by_field_name("name")];
            let parts: Vec<String> =
                parts.into_iter().flatten().map(|p| scope_name(p, src)).collect();
            parts.join("::")
        }
        "type_identifier"
        | "identifier"
        | "namespace_identifier"
        | "nested_namespace_specifier" => node_text(node, src).to_owned(),
        // Code the parser could not read: its first name only.
        _ => {
            let text = node_text(node, src).trim_start();
            let end = text
                .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
                .unwrap_or(text.len());
            text[..end].to_owned()
        }
    }
}

/// The last name of `a::b::c`.
fn last_name(mut node: Node) -> Node {
    while node.kind() == "qualified_identifier" {
        match node.child_by_field_name("name") {
            Some(n) => node = n,
            None => break,
        }
    }
    node
}

/// The documentation right before a definition (or its `template <…>`): a run of `///`
/// comments or a `/** … */` comment.
fn doc(node: Node, src: &str) -> Option<Vec<String>> {
    let anchor = match node.parent() {
        Some(p) if p.kind() == "template_declaration" => p,
        _ => node,
    };
    let mut lines = Vec::new();
    let mut prev = anchor.prev_sibling();
    let mut row = anchor.start_position().row;
    while let Some(p) = prev {
        let text = node_text(p, src);
        if p.kind() != "comment" || p.end_position().row + 1 < row {
            break;
        }
        if text.starts_with("/**") && lines.is_empty() {
            return Some(block_comment(text));
        }
        if !text.starts_with("///") {
            break;
        }
        lines.push(text);
        row = p.start_position().row;
        prev = p.prev_sibling();
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(doc_lines(lines))
}

#[cfg(test)]
mod tests {
    use crate::{Language, Source};

    #[test]
    fn functions_in_namespaces_classes_and_outside() {
        let src = r#"
#include <vector>
int helper(int a, int b);
namespace shop {
/// Adds.
int add(int a, int b) { return a + b; }
class Cart : public Base {
public:
    Cart(int n) : count_(n) { }
    ~Cart() { }
    Cart& operator+=(const Cart& o) { return *this; }
    template <typename T> T first(const std::vector<T>& xs) { return xs[0]; }
    void declared();
    struct Item { int price() const { return 1; } };
};
}
void shop::Cart::run(int n, ...) const { }
static int main(void) { return 0; }
"#;
        let s = Source::parse(Language::Cpp, src);
        let found: Vec<(String, String, String)> =
            s.methods().into_iter().map(|m| (m.class_name(), m.name, m.signature)).collect();
        let expected = [
            ("shop", "add", "int add(int a, int b)"),
            ("shop::Cart", "Cart", "Cart(int n)"),
            ("shop::Cart", "~Cart", "~Cart()"),
            ("shop::Cart", "op_AdditionAssignment", "Cart& operator+=(const Cart& o)"),
            ("shop::Cart", "first", "T first(const std::vector<T>& xs)"),
            ("shop::Cart::Item", "price", "int price()"),
            ("shop::Cart", "run", "void shop::Cart::run(int n, ...)"),
            ("", "main", "int main(void)"),
        ];
        let expected: Vec<(String, String, String)> = expected
            .iter()
            .map(|(c, n, s)| ((*c).to_owned(), (*n).to_owned(), (*s).to_owned()))
            .collect();
        assert_eq!(found, expected);
        let made = crate::Made { author: "t", date: "2026-10-30", origin: "test" };
        let titles: Vec<Vec<String>> =
            s.methods().iter().map(|m| s.diagram(m, &made).unwrap().0.text()).collect();
        assert_eq!(titles[0], ["add(a, b)"]);
        assert_eq!(titles[3], ["operator +=(o)"]);
        assert_eq!(titles[6], ["run(n, ...)"]);
        assert_eq!(titles[7], ["main()"]);
        let (add, _) = s.diagram(&s.methods()[0], &made).unwrap();
        assert_eq!(add.comment(), ["Adds."]);
        assert_eq!(s.methods()[6].file_name(), "shop.Cart.run.nsd");
    }
}
