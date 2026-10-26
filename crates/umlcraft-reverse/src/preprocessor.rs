//! Conditional compilation (C#, C++): of `#if` … `#elif` … `#else`, only the first branch is
//! read; nothing is evaluated. Include guards (`#ifndef X_H`) keep the whole file.

/// The text as the first branches of its conditionals read it: the lines of `#if`, `#ifdef`,
/// `#ifndef`, `#elif`, `#elifdef`, `#elifndef`, `#else` and `#endif`, and the branches after
/// the first, blanked out, with the number of branches left out. Blanking keeps every byte's
/// place, so lines and columns stay as written, and declarations that the conditionals cut
/// apart parse as one. A blanked line that ends in `\` blanks the next one too.
pub(crate) fn first_branches(text: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    // Per open `#if`: whether the branch read now is left out.
    let mut open: Vec<bool> = Vec::new();
    let mut branches = 0;
    // Whether the line before was blanked and goes on in this one (`#if A || \`).
    let mut continued = false;
    for line in text.split_inclusive('\n') {
        let directive = line.trim_start().strip_prefix('#').map(str::trim_start);
        let directive = directive.filter(|_| !continued);
        let word = directive.map(|d| d.split(|c: char| !c.is_ascii_alphabetic()).next());
        let left_out = |open: &[bool]| open.iter().any(|&l| l);
        let blank = match word.flatten() {
            Some("if" | "ifdef" | "ifndef") => {
                open.push(false);
                true
            }
            Some("elif" | "elifdef" | "elifndef" | "else") => {
                let outer = open.len().saturating_sub(1);
                if !open.is_empty() && !left_out(&open[..outer]) {
                    branches += 1;
                }
                if let Some(last) = open.last_mut() {
                    *last = true;
                }
                true
            }
            Some("endif") => {
                open.pop();
                true
            }
            _ => continued || left_out(&open),
        };
        continued = blank && line.trim_end().ends_with('\\');
        if blank {
            for c in line.chars() {
                match c {
                    '\r' | '\n' => out.push(c),
                    _ => out.extend(std::iter::repeat_n(' ', c.len_utf8())),
                }
            }
        } else {
            out.push_str(line);
        }
    }
    (out, branches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_the_first_branches() {
        let text = "a\n#if A\nb\n  #if B\nc\n  #else\nd\n  #endif\n#elif C\ne\n#if D\nf\n#else\ng\n#endif\n#else\nh\n#endif\ni\n";
        let (kept, branches) = first_branches(text);
        let lines: Vec<&str> = kept.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        assert_eq!(lines, ["a", "b", "c", "i"]);
        assert_eq!(kept.len(), text.len());
        // `#else` of B, `#elif C` and `#else` of A; the `#else` of D is inside a branch left out.
        assert_eq!(branches, 3);
    }

    #[test]
    fn blanks_continued_directives() {
        let text = "#if defined(A) || \\\n    defined(B)\na\n#else\nb \\\nc\n#endif\nd\n";
        let (kept, branches) = first_branches(text);
        let lines: Vec<&str> = kept.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        assert_eq!(lines, ["a", "d"]);
        assert_eq!(branches, 1);
    }

    #[test]
    fn knows_the_c_directives() {
        let text = "#ifndef A_H\n#define A_H\na\n#ifdef B\nb\n#elifdef C\nc\n#endif\n#endif\n";
        let (kept, branches) = first_branches(text);
        let lines: Vec<&str> = kept.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        assert_eq!(lines, ["#define A_H", "a", "b"]);
        assert_eq!(branches, 1);
    }
}
