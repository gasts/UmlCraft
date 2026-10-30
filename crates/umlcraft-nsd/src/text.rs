//! Structorizer's text lists and its way of escaping attribute values.
//!
//! A `text` or `comment` attribute holds lines as quoted strings separated by commas,
//! `"first","second"`, with quotes inside a line doubled. No lines is the empty value.
//! In the file, Structorizer escapes `"` as `&#34;`, `<` as `&#60;`, `>` as `&#62;`, `&` as
//! `&amp;` and line feeds as `&#10;`; everything else, tabs included, stays as it is. Control
//! characters that XML does not allow, not even escaped, are written as U+FFFD.

/// The lines of a list, or where it stops being one.
pub fn decode_lines(value: &str) -> Result<Vec<String>, usize> {
    let mut lines = Vec::new();
    if value.is_empty() {
        return Ok(lines);
    }
    let mut chars = value.char_indices().peekable();
    loop {
        match chars.next() {
            Some((_, '"')) => {}
            Some((at, _)) => return Err(at),
            None => return Err(value.len()),
        }
        let mut line = String::new();
        loop {
            match chars.next() {
                Some((_, '"')) if chars.peek().is_some_and(|(_, c)| *c == '"') => {
                    chars.next();
                    line.push('"');
                }
                Some((_, '"')) => break,
                Some((_, c)) => line.push(c),
                None => return Err(value.len()),
            }
        }
        lines.push(line);
        match chars.next() {
            None => return Ok(lines),
            Some((_, ',')) => {}
            Some((at, _)) => return Err(at),
        }
    }
}

/// The lines of a list; a value that is not one counts as a single line.
pub fn decode_lines_lossy(value: &str) -> Vec<String> {
    decode_lines(value).unwrap_or_else(|_| vec![value.to_owned()])
}

pub fn encode_lines(lines: &[String]) -> String {
    let quoted: Vec<String> =
        lines.iter().map(|l| format!("\"{}\"", l.replace('"', "\"\""))).collect();
    quoted.join(",")
}

/// An attribute value as Structorizer writes it between double quotes. Characters XML 1.0
/// does not allow (control characters but tab, line feed and carriage return; U+FFFE and
/// U+FFFF) become U+FFFD, so that the file stays XML: strings in source code can hold them.
pub fn escape_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => out.push_str("&#34;"),
            '<' => out.push_str("&#60;"),
            '>' => out.push_str("&#62;"),
            '&' => out.push_str("&amp;"),
            '\n' => out.push_str("&#10;"),
            '\t' | '\r' => out.push(c),
            '\0'..='\u{1F}' | '\u{FFFE}' | '\u{FFFF}' => out.push('\u{FFFD}'),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn no_lines_is_the_empty_value() {
        assert_eq!(decode_lines(""), Ok(vec![]));
        assert_eq!(encode_lines(&[]), "");
    }

    #[test]
    fn one_empty_line_is_two_quotes() {
        assert_eq!(decode_lines("\"\""), Ok(lines(&[""])));
        assert_eq!(encode_lines(&lines(&[""])), "\"\"");
    }

    #[test]
    fn lines_round_trip() {
        for case in [
            lines(&["a <- 1"]),
            lines(&["a", "b", "c"]),
            lines(&["x, y", "f(a, b)"]),
            lines(&["say \"hi\"", "\"\""]),
            lines(&["", "after an empty line"]),
            lines(&["äöü € ≤ 😀", "tab\there"]),
        ] {
            let encoded = encode_lines(&case);
            assert_eq!(decode_lines(&encoded), Ok(case));
        }
    }

    #[test]
    fn doubled_quotes_and_commas_inside_lines() {
        assert_eq!(decode_lines(r#""a ""b"", c","d""#), Ok(lines(&[r#"a "b", c"#, "d"])));
    }

    #[test]
    fn broken_lists_report_where() {
        assert_eq!(decode_lines("plain"), Err(0));
        assert_eq!(decode_lines("\"open"), Err(5));
        assert_eq!(decode_lines("\"a\"x"), Err(3));
        assert_eq!(decode_lines_lossy("plain"), lines(&["plain"]));
    }

    #[test]
    fn escapes_like_structorizer() {
        assert_eq!(
            escape_attr("\"a\" < b & c > d\nnext\t'x' ä"),
            "&#34;a&#34; &#60; b &amp; c &#62; d&#10;next\t'x' ä"
        );
    }

    #[test]
    fn characters_xml_forbids_are_replaced() {
        assert_eq!(escape_attr("a\u{2}b\0c\rd\u{FFFF}"), "a\u{FFFD}b\u{FFFD}c\rd\u{FFFD}");
    }
}
