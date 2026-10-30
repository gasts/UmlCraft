//! C++ (and C): functions, methods, constructors, destructors and operators of a source or
//! header file and their bodies as structograms.

mod build;
mod methods;

use tree_sitter::Node;
use umlcraft_nsd::Element;

use crate::core::{Builder, Grammar};
use crate::pseudo::assign;
use crate::text::{declared_name, lines_at, node_text, one_line};

pub use build::diagram;
pub use methods::{find, methods};

pub struct Cpp;

impl Grammar for Cpp {
    const COMMENTS: &'static [&'static str] = &["comment"];
    const BLOCKS: &'static [&'static str] = &["compound_statement"];
    const IGNORED: &'static [&'static str] =
        &["preproc_include", "preproc_def", "preproc_function_def", "preproc_call"];
    const ENDS_CASE: &'static [&'static str] = &[
        "break_statement",
        "return_statement",
        "throw_statement",
        "continue_statement",
        "goto_statement",
        "co_return_statement",
    ];
    const RUNS_ON: bool = true;
    const BREAK_ENDS_CASE: bool = true;
    const TRUE: &'static str = "true";

    fn statement(b: &mut Builder<'_, Self>, node: Node, out: &mut Vec<Element>) {
        build::statement(b, node, out);
    }

    fn simple(b: &Builder<'_, Self>, node: Node) -> Vec<Element> {
        build::simple(b, node)
    }
}

/// The names of a parameter list's parameters: `(int a, const T& b = 1, ...)` → `a, b, ...`;
/// unnamed parameters have none.
fn parameter_names(parameters: Option<Node>, src: &str) -> Vec<String> {
    let Some(parameters) = parameters else { return Vec::new() };
    let mut cursor = parameters.walk();
    parameters
        .children(&mut cursor)
        .filter_map(|p| match p.kind() {
            "..." => Some("...".to_owned()),
            "parameter_declaration"
            | "optional_parameter_declaration"
            | "variadic_parameter_declaration" => p
                .child_by_field_name("declarator")
                .and_then(declared_name)
                .map(|n| node_text(n, src).to_owned()),
            _ => None,
        })
        .collect()
}

/// The assignments of a declaration, one per variable with a value: `int a = 1, b(2), c{3};`
/// → `a := 1`, `b := 2`, `c := 3`; `Foo f(x, y);` → `f := Foo(x, y)`; `int d;` → none.
pub fn declaration(node: Node, src: &str) -> Vec<Vec<String>> {
    let column = node.start_position().column;
    let type_text = node.child_by_field_name("type").map(|t| one_line(node_text(t, src)));
    let type_text = type_text.unwrap_or_default();
    let mut cursor = node.walk();
    let declarators: Vec<Node> = node.children_by_field_name("declarator", &mut cursor).collect();
    declarators
        .into_iter()
        .filter_map(|d| match d.kind() {
            "init_declarator" => {
                let name = node_text(declared_name(d.child_by_field_name("declarator")?)?, src);
                let value = d.child_by_field_name("value")?;
                let mut c = d.walk();
                let copies = d.children(&mut c).any(|t| t.kind() == "=");
                let value = match value.kind() {
                    // `int x(1)`, `int x{1}`: one value is the value, several make an object.
                    "argument_list" | "initializer_list" if !copies => {
                        let mut c = value.walk();
                        let items: Vec<Node> = value
                            .named_children(&mut c)
                            .filter(|i| i.kind() != "comment")
                            .collect();
                        match items.as_slice() {
                            [one] => lines_at(*one, src, column),
                            _ => vec![format!("{type_text}{}", one_line(node_text(value, src)))],
                        }
                    }
                    _ => lines_at(value, src, column),
                };
                Some(assign(name, value))
            }
            // `Foo f(x, y);` reads as a function; in a body it makes an object.
            "function_declarator" => {
                let name = node_text(declared_name(d.child_by_field_name("declarator")?)?, src);
                let parameters = d.child_by_field_name("parameters")?;
                let mut c = parameters.walk();
                let objects = parameters
                    .named_children(&mut c)
                    .all(|p| p.child_by_field_name("declarator").is_none());
                objects.then(|| {
                    assign(
                        name,
                        vec![format!("{type_text}{}", one_line(node_text(parameters, src)))],
                    )
                })
            }
            _ => None,
        })
        .collect()
}
