//! Every method of the fixture files as a structogram, written as a readable tree and compared
//! with the snapshot next to the file (`UPDATE_SNAPSHOTS=1` rewrites them). Each diagram must
//! also read back as it was written.

use std::fmt::Write;
use std::path::Path;

use umlcraft_nsd::{Diagram, Element, ElementKind, Sequence, read, write};
use umlcraft_nsd_java::{Made, Source};

const MADE: Made = Made { author: "tester", date: "2026-10-29", origin: "UmlCraft test" };

fn tree(d: &Diagram) -> String {
    fn element(out: &mut String, e: &Element, depth: usize) {
        let pad = "  ".repeat(depth);
        let mut head = format!("{pad}{}", e.kind.tag());
        if e.color() != umlcraft_nsd::Color::WHITE {
            head.push_str(&format!(" color={}", e.color().to_hex()));
        }
        if e.disabled() {
            head.push_str(" disabled");
        }
        if e.kind == ElementKind::For {
            head.push_str(&format!(" style={}", e.attr("style").unwrap_or("")));
        }
        writeln!(out, "{head}").unwrap();
        for line in e.text() {
            writeln!(out, "{pad}  | {line}").unwrap();
        }
        for line in e.comment() {
            writeln!(out, "{pad}  # {line}").unwrap();
        }
        for b in &e.branches {
            writeln!(out, "{pad}  -").unwrap();
            sequence(out, b, depth + 2);
        }
    }
    fn sequence(out: &mut String, s: &Sequence, depth: usize) {
        for e in &s.elements {
            element(out, e, depth);
        }
    }
    let mut out = String::new();
    for line in d.text() {
        writeln!(out, "| {line}").unwrap();
    }
    for line in d.comment() {
        writeln!(out, "# {line}").unwrap();
    }
    sequence(&mut out, &d.children, 1);
    out
}

fn snapshot(java: &Path) -> String {
    let text = std::fs::read_to_string(java).unwrap();
    let source = Source::parse(&text);
    let mut out = String::new();
    for m in source.methods() {
        let (d, report) = source.diagram(&m, &MADE).unwrap();
        assert_eq!(read(&write(&d)).unwrap(), d, "{}", m.name);
        writeln!(out, "== {}.{} (line {})", m.class_name(), m.name, m.line).unwrap();
        out.push_str(&tree(&d));
        for note in &report.notes {
            writeln!(out, "! line {}: {:?}", note.line, note.kind).unwrap();
        }
        out.push('\n');
    }
    out
}

#[test]
fn fixtures_match_their_snapshots() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut javas: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "java"))
        .collect();
    javas.sort();
    assert!(javas.len() >= 10);
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    for java in javas {
        let got = snapshot(&java);
        let path = java.with_extension("txt");
        if update {
            std::fs::write(&path, &got).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("{} is missing; run with UPDATE_SNAPSHOTS=1", path.display())
        });
        pretty_assertions::assert_eq!(expected, got, "{}", java.display());
    }
}
