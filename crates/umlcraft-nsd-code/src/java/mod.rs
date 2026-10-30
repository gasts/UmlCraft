//! Java: methods of a `.java` file and their bodies as structograms.

mod build;
mod methods;

use tree_sitter::Node;
use umlcraft_nsd::Element;

use crate::core::{Builder, Grammar};
use crate::pseudo::assign;
use crate::text::{lines_at, node_text};

pub use build::diagram;
pub use methods::{find, methods};

pub struct Java;

impl Grammar for Java {
    const COMMENTS: &'static [&'static str] = &["line_comment", "block_comment"];
    const BLOCKS: &'static [&'static str] =
        &["block", "constructor_body", "switch_block_statement_group"];
    const IGNORED: &'static [&'static str] = &["switch_label"];
    const ENDS_CASE: &'static [&'static str] = &[
        "break_statement",
        "return_statement",
        "throw_statement",
        "continue_statement",
        "yield_statement",
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

/// The diagram's title, `name(a, b)`: a method's or constructor's name and its parameters'
/// names.
pub fn title(method: Node, src: &str) -> String {
    let name = method.child_by_field_name("name").map(|n| node_text(n, src)).unwrap_or_default();
    let Some(parameters) = method.child_by_field_name("parameters") else {
        // A record's compact constructor has none.
        return name.to_owned();
    };
    let mut cursor = parameters.walk();
    let names: Vec<&str> = parameters
        .named_children(&mut cursor)
        .filter_map(|p| match p.kind() {
            "formal_parameter" => p.child_by_field_name("name"),
            "spread_parameter" => {
                let mut cursor = p.walk();
                let declarator =
                    p.named_children(&mut cursor).find(|c| c.kind() == "variable_declarator");
                declarator.and_then(|d| d.child_by_field_name("name"))
            }
            // `Outer this` only names the receiver's type.
            _ => None,
        })
        .map(|n| node_text(n, src))
        .collect();
    format!("{name}({})", names.join(", "))
}

/// The assignments of a local variable declaration, one per variable with a value:
/// `int a = 1, b;` → `a := 1`. A variable without a value has none.
pub fn declaration(node: Node, src: &str) -> Vec<Vec<String>> {
    let column = node.start_position().column;
    let mut cursor = node.walk();
    node.children_by_field_name("declarator", &mut cursor)
        .filter_map(|d| {
            let name = node_text(d.child_by_field_name("name")?, src);
            Some(assign(name, lines_at(d.child_by_field_name("value")?, src, column)))
        })
        .collect()
}
