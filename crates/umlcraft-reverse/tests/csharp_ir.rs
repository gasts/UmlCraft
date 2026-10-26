//! What the C# front-end reads from each fixture, compared with `expected.ir` next to the
//! sources. Run with `UPDATE_SNAPSHOTS=1` to write the snapshots after a deliberate change, and
//! review the diff.

mod common;

use umlcraft_reverse::SourceLanguage;

fn check(case: &str) {
    common::check_ir(SourceLanguage::CSharp, case);
}

#[test]
fn namespaces() {
    check("namespaces");
}

#[test]
fn kinds() {
    check("kinds");
}

#[test]
fn modifiers() {
    check("modifiers");
}

#[test]
fn docs() {
    check("docs");
}

#[test]
fn inheritance() {
    check("inheritance");
}

#[test]
fn generics() {
    check("generics");
}

#[test]
fn arrays_nullable() {
    check("arrays_nullable");
}

#[test]
fn properties() {
    check("properties");
}

#[test]
fn partial() {
    check("partial");
}

#[test]
fn resolution() {
    check("resolution");
}

#[test]
fn preprocessor() {
    check("preprocessor");
}

#[test]
fn syntax_error() {
    check("syntax_error");
}

#[test]
fn encoding() {
    check("encoding");
}
