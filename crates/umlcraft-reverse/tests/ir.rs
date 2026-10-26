//! What the Java front-end reads from each fixture, compared with `expected.ir` next to the
//! sources. Run with `UPDATE_SNAPSHOTS=1` to write the snapshots after a deliberate change, and
//! review the diff.

mod common;

use umlcraft_reverse::SourceLanguage;

fn check(case: &str) {
    common::check_ir(SourceLanguage::Java, case);
}

#[test]
fn packages() {
    check("packages");
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
fn javadoc() {
    check("javadoc");
}

#[test]
fn annotations() {
    check("annotations");
}

#[test]
fn initializers() {
    check("initializers");
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
fn arrays_varargs() {
    check("arrays_varargs");
}

#[test]
fn syntax_error() {
    check("syntax_error");
}

#[test]
fn encoding() {
    check("encoding");
}
