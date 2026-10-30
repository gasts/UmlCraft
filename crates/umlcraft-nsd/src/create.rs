//! New diagrams and elements in the form Structorizer 3.32-35 gives them, so that the editor
//! and the code import only make what Structorizer would write itself.

use crate::model::{
    Attrs, Color, Diagram, Element, ElementKind, Lines, ROOT_ORDER, RootKind, Sequence,
};
use crate::text::encode_lines;

/// The version whose format this crate writes.
pub const STRUCTORIZER_VERSION: &str = "3.32-35";

/// The parser keywords of a fresh Structorizer installation as its GUI saves them (two with
/// a trailing space).
const DEFAULT_KEYWORDS: &[(&str, &str)] = &[
    ("preRepeat", "until "),
    ("postFor", "to"),
    ("preReturn", "return"),
    ("postForIn", "in"),
    ("preWhile", "while "),
    ("output", "OUTPUT"),
    ("input", "INPUT"),
    ("preFor", "for"),
    ("preExit", "exit"),
    ("preLeave", "leave"),
    ("ignoreCase", "true"),
    ("preThrow", "throw"),
    ("preForIn", "foreach"),
    ("stepFor", "by"),
];

/// Who made a new diagram, when (`YYYY-MM-DD`) and with what (`origin`).
pub struct Provenance<'a> {
    pub author: &'a str,
    pub date: &'a str,
    pub origin: &'a str,
}

impl Diagram {
    /// An empty diagram with Structorizer's default header.
    pub fn new(kind: RootKind, text: &[String], made: &Provenance) -> Diagram {
        let mut attrs = Attrs(Vec::new());
        attrs.set("xmlns:nsd", "https://structorizer.fisch.lu", ROOT_ORDER);
        attrs.set("version", STRUCTORIZER_VERSION, ROOT_ORDER);
        for (name, value) in DEFAULT_KEYWORDS {
            attrs.set(name, *value, ROOT_ORDER);
        }
        attrs.set("author", made.author, ROOT_ORDER);
        attrs.set("created", made.date, ROOT_ORDER);
        attrs.set("origin", made.origin, ROOT_ORDER);
        attrs.set("text", encode_lines(text), ROOT_ORDER);
        attrs.set("comment", "", ROOT_ORDER);
        attrs.set("color", Color::WHITE.to_hex(), ROOT_ORDER);
        attrs.set("type", kind.as_str(), ROOT_ORDER);
        attrs.set("style", "nice", ROOT_ORDER);
        Diagram { attrs, children: Sequence::new(Vec::new()) }
    }
}

/// The header of a new for loop.
pub enum ForLoop<'a> {
    /// `for i <- 1 to 10 by 2`: counter, start, end, step.
    Counting { var: &'a str, start: &'a str, end: &'a str, step: &'a str },
    /// `foreach x in list`: the loop variable.
    Traversing { var: &'a str },
    /// Any other text.
    Free,
}

impl Element {
    /// An element of `kind` with Structorizer's attributes and empty branches (one for a case
    /// or parallel element; use the dedicated constructors for more).
    fn with(kind: ElementKind, text: Option<&[String]>, branches: usize) -> Element {
        let mut e = Element { kind, attrs: Attrs::default(), branches: Vec::new() };
        if let Some(text) = text {
            e.set_attr("text", crate::model::text_value(text));
        }
        e.set_attr("comment", "");
        e.set_attr("color", Color::WHITE.to_hex());
        match kind {
            ElementKind::Instruction => e.set_attr("rotated", "0"),
            ElementKind::Call => e.set_attr("ismethoddecl", "0"),
            ElementKind::Alternative | ElementKind::Case => {
                e.set_attr("branch_colors", ",".repeat(branches.max(1) - 1));
            }
            _ => {}
        }
        e.set_attr("disabled", "0");
        e.branches = (0..branches).map(|_| Sequence::new(Vec::new())).collect();
        e
    }

    pub fn instruction(text: &[String]) -> Element {
        Self::with(ElementKind::Instruction, Some(text), 0)
    }

    pub fn call(text: &[String]) -> Element {
        Self::with(ElementKind::Call, Some(text), 0)
    }

    pub fn jump(text: &[String]) -> Element {
        Self::with(ElementKind::Jump, Some(text), 0)
    }

    pub fn alternative(condition: &[String]) -> Element {
        Self::with(ElementKind::Alternative, Some(condition), 2)
    }

    /// `lines`: the discriminator, one line per case, then the default label (`%` = none).
    /// Gets a branch per case and one for the default.
    pub fn case(lines: &[String]) -> Element {
        let branches = lines.len().saturating_sub(1).max(2);
        Self::with(ElementKind::Case, Some(lines), branches)
    }

    pub fn for_loop(text: &[String], header: ForLoop) -> Element {
        let mut e = Self::with(ElementKind::For, Some(text), 1);
        match header {
            ForLoop::Counting { var, start, end, step } => {
                e.set_attr("counterVar", var);
                e.set_attr("startValue", start);
                e.set_attr("endValue", end);
                e.set_attr("stepConst", step);
                e.set_attr("style", "COUNTER");
            }
            ForLoop::Traversing { var } => {
                e.set_attr("counterVar", var);
                e.set_attr("startValue", "");
                e.set_attr("stepConst", "");
                e.set_attr("style", "TRAVERSAL");
                e.set_attr("insep", "in");
            }
            ForLoop::Free => e.set_attr("style", "FREETEXT"),
        }
        e
    }

    pub fn while_loop(condition: &[String]) -> Element {
        Self::with(ElementKind::While, Some(condition), 1)
    }

    pub fn repeat_loop(condition: &[String]) -> Element {
        Self::with(ElementKind::Repeat, Some(condition), 1)
    }

    pub fn forever_loop() -> Element {
        Self::with(ElementKind::Forever, None, 1)
    }

    /// `variable`: the name the caught exception gets.
    pub fn try_block(variable: &str) -> Element {
        Self::with(ElementKind::Try, Some(&[variable.to_owned()]), 3)
    }

    /// Structorizer keeps the number of branches as the text.
    pub fn parallel(branches: usize) -> Element {
        let branches = branches.max(1);
        Self::with(ElementKind::Parallel, Some(&[branches.to_string()]), branches)
    }
}

/// Lines from string slices, for building diagrams in code.
pub fn lines(texts: &[&str]) -> Lines {
    texts.iter().map(|s| (*s).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read, write};

    fn put(seq: &mut Sequence, e: Element) {
        seq.elements.push(e);
        seq.fit_color();
    }

    /// One element of each kind, nested a little, as the editor or the import would build it.
    fn all_elements() -> Diagram {
        let made = Provenance { author: "tester", date: "2026-10-29", origin: "UmlCraft test" };
        let mut d = Diagram::new(RootKind::Sub, &lines(&["sum(a: int; b: int): int"]), &made);
        let c = &mut d.children;
        put(c, Element::instruction(&lines(&["s <- a + b", "say \"hi\" & <go>"])));
        put(c, Element::call(&lines(&["log(s)"])));
        let mut alt = Element::alternative(&lines(&["s > 10"]));
        put(&mut alt.branches[0], Element::jump(&lines(&["return s"])));
        put(c, alt);
        put(c, Element::case(&lines(&["s", "1", "2, 3", "default"])));
        put(c, Element::case(&lines(&["s", "1", "%"])));
        let mut counting = Element::for_loop(
            &lines(&["for i <- 1 to 10 by 2"]),
            ForLoop::Counting { var: "i", start: "1", end: "10", step: "2" },
        );
        put(&mut counting.branches[0], Element::instruction(&lines(&["s <- s + i"])));
        put(c, counting);
        put(c, Element::for_loop(&lines(&["foreach x in list"]), ForLoop::Traversing { var: "x" }));
        put(c, Element::for_loop(&lines(&["for every line"]), ForLoop::Free));
        put(c, Element::while_loop(&lines(&["s < 100"])));
        put(c, Element::repeat_loop(&lines(&["s >= 100"])));
        put(c, Element::forever_loop());
        put(c, Element::try_block("e"));
        put(c, Element::parallel(3));
        put(c, Element::jump(&[]));
        d
    }

    #[test]
    fn new_diagrams_match_the_snapshot() {
        let written = write(&all_elements());
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/new/all-elements.nsd");
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            std::fs::write(path, &written).unwrap();
            return;
        }
        let expected = std::fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("{path} is missing; run with UPDATE_SNAPSHOTS=1"));
        pretty_assertions::assert_eq!(expected, written);
    }

    #[test]
    fn new_diagrams_read_back_unchanged() {
        let d = all_elements();
        assert_eq!(read(&write(&d)).unwrap(), d);
    }

    #[test]
    fn constructors_give_structorizers_shape() {
        let case = Element::case(&lines(&["x", "1", "2", "default"]));
        assert_eq!(case.branches.len(), 3);
        assert_eq!(case.attr("branch_colors"), Some(",,"));
        let par = Element::parallel(4);
        assert_eq!(par.text(), ["4"]);
        assert_eq!(par.branches.len(), 4);
        assert_eq!(Element::forever_loop().attr("text"), None);
        assert!(Element::alternative(&[]).branches.iter().all(|b| b.attrs.get("color").is_some()));
    }
}
