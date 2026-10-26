//! Documentation comments as documentation text: block and Javadoc comments, and runs of `///`
//! or `//!` line comments (C#'s XML documentation, Doxygen).

/// The text of a `/** … */`, `/*! … */` or `/* … */` comment: without the delimiters, and on
/// every line without the leading whitespace, stars and the blanks after them (Astah removes
/// `\**[ \t]*` at line starts). Blank lines at the start and end are dropped.
pub fn clean(comment: &str) -> String {
    let inner = comment.strip_prefix("/*!").or(comment.strip_prefix("/*")).unwrap_or(comment);
    let inner = inner.strip_suffix("*/").unwrap_or(inner);
    let lines = inner.lines().map(|line| {
        line.trim_start().trim_start_matches('*').trim_start_matches([' ', '\t']).trim_end()
    });
    join_trimmed(lines.collect())
}

/// The text of consecutive `///` or `//!` comments, one per line: without the slashes (and the
/// `!`) and the blanks after them, the rest (XML tags included) as written. Blank lines at the
/// start and end are dropped.
pub fn clean_lines<'a>(comments: impl IntoIterator<Item = &'a str>) -> String {
    let lines = comments.into_iter().map(|line| {
        let line = line.trim_start();
        let text = line.strip_prefix("///").or(line.strip_prefix("//!")).unwrap_or(line);
        text.trim_start_matches([' ', '\t']).trim_end()
    });
    join_trimmed(lines.collect())
}

fn join_trimmed(lines: Vec<&str>) -> String {
    let first = lines.iter().position(|l| !l.is_empty());
    let last = lines.iter().rposition(|l| !l.is_empty());
    match (first, last) {
        (Some(first), Some(last)) => lines[first..=last].join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_delimiters_stars_and_blank_edges() {
        let comment = "/**\n   * Holds an order.\n   *\n   *   @author Ulf\n   */";
        assert_eq!(clean(comment), "Holds an order.\n\n@author Ulf");
        assert_eq!(clean("/** One line. */"), "One line.");
        assert_eq!(clean("/***/"), "");
        assert_eq!(clean("/**/"), "");
        assert_eq!(clean("/* Plain. */"), "Plain.");
    }

    #[test]
    fn strips_the_slashes_of_line_comments() {
        let lines = ["///", "/// <summary>", "///   An order.", "/// </summary>", "///"];
        assert_eq!(clean_lines(lines), "<summary>\nAn order.\n</summary>");
        assert_eq!(clean_lines(["///"]), "");
        assert_eq!(clean_lines(["//! Doxygen.", "//!"]), "Doxygen.");
        assert_eq!(clean("/*! Qt style. */"), "Qt style.");
    }
}
