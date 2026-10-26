//! What the Python front-end reads from each fixture, compared with `expected.ir` next to the
//! sources. Run with `UPDATE_SNAPSHOTS=1` to write the snapshots after a deliberate change, and
//! review the diff.

mod common;

use umlcraft_reverse::SourceLanguage;

fn check(case: &str) {
    common::check_ir(SourceLanguage::Python, case);
}

#[test]
fn modules() {
    check("modules");
}

#[test]
fn kinds() {
    check("kinds");
}

#[test]
fn visibility() {
    check("visibility");
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
fn type_hints() {
    check("type_hints");
}

#[test]
fn init() {
    check("init");
}

#[test]
fn methods() {
    check("methods");
}

#[test]
fn resolution() {
    check("resolution");
}

#[test]
fn associations() {
    check("associations");
}

#[test]
fn statements() {
    check("statements");
}

#[test]
fn syntax_error() {
    check("syntax_error");
}

#[test]
fn encoding() {
    check("encoding");
}
