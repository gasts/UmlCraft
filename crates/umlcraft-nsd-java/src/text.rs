//! Source text of nodes as element texts.

use tree_sitter::Node;

pub fn node_text<'s>(node: Node, src: &'s str) -> &'s str {
    &src[node.byte_range()]
}

/// Whitespace runs (line breaks included) as single spaces.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A node's text as lines: the first as it starts, the following ones without the
/// indentation the node's first line has in the file, trailing blanks removed.
pub fn lines(node: Node, src: &str) -> Vec<String> {
    lines_of(node_text(node, src), node.start_position().column)
}

/// A node's text as lines, the following ones without the indentation of `column` (where the
/// statement the node is part of starts).
pub fn lines_at(node: Node, src: &str, column: usize) -> Vec<String> {
    lines_of(node_text(node, src), column)
}

pub fn lines_of(text: &str, column: usize) -> Vec<String> {
    text.split('\n')
        .enumerate()
        .map(|(i, line)| {
            let line = if i == 0 {
                line
            } else {
                let blank = line.len() - line.trim_start_matches([' ', '\t']).len();
                &line[blank.min(column)..]
            };
            line.trim_end().to_owned()
        })
        .collect()
}

/// A condition without its outer parentheses: `(a > b)` → `a > b`.
pub fn condition(node: Node, src: &str) -> Vec<String> {
    if node.kind() == "parenthesized_expression"
        && let Some(inner) = node.named_child(0)
    {
        return lines(inner, src);
    }
    lines(node, src)
}

/// The text of a comment without its delimiters.
pub fn comment(node: Node, src: &str) -> Vec<String> {
    let text = node_text(node, src);
    if let Some(line) = text.strip_prefix("//") {
        return vec![line.trim().to_owned()];
    }
    block_comment(text)
}

/// A `/* … */` or `/** … */` comment's lines without the delimiters, the stars at line starts
/// and blank lines at either end.
pub fn block_comment(text: &str) -> Vec<String> {
    let inner = text.strip_suffix("*/").unwrap_or(text);
    let inner = inner.strip_prefix("/*").unwrap_or(inner);
    let inner = inner.strip_prefix('*').unwrap_or(inner);
    let lines: Vec<String> = inner
        .split('\n')
        .map(|l| l.trim_start().trim_start_matches('*').trim_start_matches([' ', '\t']).trim_end())
        .map(str::to_owned)
        .collect();
    let first = lines.iter().position(|l| !l.is_empty());
    let last = lines.iter().rposition(|l| !l.is_empty());
    match (first, last) {
        (Some(a), Some(b)) => lines[a..=b].to_vec(),
        _ => Vec::new(),
    }
}

/// Lines without a final `;`.
pub fn without_semicolon(mut lines: Vec<String>) -> Vec<String> {
    if let Some(last) = lines.last_mut()
        && let Some(stripped) = last.strip_suffix(';')
    {
        *last = stripped.trim_end().to_owned();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_lines_lose_the_first_lines_indentation() {
        let text = "builder\n            .add(1)\n            .build();";
        assert_eq!(lines_of(text, 8), ["builder", "    .add(1)", "    .build();"]);
        assert_eq!(lines_of("a\n  b", 8), ["a", "b"]);
    }

    #[test]
    fn comments_lose_their_delimiters() {
        assert_eq!(
            block_comment("/**\n     * Creates.\n     *\n     * @param a x\n     */"),
            ["Creates.", "", "@param a x"]
        );
        assert_eq!(block_comment("/* one */"), ["one"]);
        assert!(block_comment("/**/").is_empty());
        assert!(block_comment("/***/").is_empty());
    }

    #[test]
    fn semicolons_go_from_the_end_only() {
        assert_eq!(without_semicolon(vec!["return a;".into()]), ["return a"]);
        assert_eq!(without_semicolon(vec!["x".into(), "y ;".into()]), ["x", "y"]);
    }
}
