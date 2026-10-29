//! Python imports into a fresh model, one per fixture: the model as text is compared with
//! `expected.model` (write it with `UPDATE_SNAPSHOTS=1` and review the diff), and the rules each
//! fixture is about are checked directly.

mod common;

use common::{at, attribute_type, links_to};
use umlcraft_model::{Document, ElementKind, Id, Model, Multiplicity};
use umlcraft_reverse::lang::Skip;
use umlcraft_reverse::{Report, SourceLanguage};

const PYTHON: SourceLanguage = SourceLanguage::Python;

fn import(doc: &mut Document, case: &str) -> Report {
    common::import(doc, PYTHON, case)
}

fn fresh(case: &str) -> (Document, Report) {
    common::fresh(PYTHON, case)
}

fn check(case: &str, model: &Model) {
    common::check_model(PYTHON, case, model)
}

fn kind(model: &Model, path: &str) -> &'static str {
    model.get(at(model, path)).unwrap().kind().type_name()
}

fn skipped(report: &Report, skip: Skip) -> usize {
    report.skipped.get(&skip).copied().unwrap_or(0)
}

/// The far end of the association that replaced `attribute` of `class`.
fn association_end(model: &Model, class: &str, attribute: &str) -> umlcraft_model::AssociationEnd {
    let owner = at(model, class);
    model
        .elements()
        .find_map(|e| match e.kind() {
            ElementKind::Association(p) => (0..2)
                .find(|&i| p.ends[i].role == attribute && p.ends[1 - i].participant == owner)
                .map(|i| p.ends[i].clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no association for {class}.{attribute}"))
}

#[test]
fn modules_are_packages() {
    let (doc, _) = fresh("modules");
    let model = doc.model();
    at(model, "shop.Catalog");
    at(model, "shop.orders.Order");
    at(model, "shop.billing.invoice.Invoice");
    // `scripts` has no `__init__.py`.
    at(model, "tool.Tool");
    check("modules", model);
}

#[test]
fn kinds_follow_the_bases() {
    let (doc, _) = fresh("kinds");
    let model = doc.model();
    assert_eq!(kind(model, "kinds.Plain"), "class");
    assert_eq!(kind(model, "kinds.Shape"), "interface");
    assert_eq!(kind(model, "kinds.Color"), "enumeration");
    assert_eq!(kind(model, "kinds.Level"), "enumeration");
    let base = model.get(at(model, "kinds.Base")).unwrap();
    assert!(matches!(base.kind(), ElementKind::Class(p) if p.is_abstract));
    check("kinds", model);
}

#[test]
fn names_tell_the_visibility() {
    let (doc, _) = fresh("visibility");
    check("visibility", doc.model());
}

#[test]
fn docstrings_are_documentation() {
    let (doc, _) = fresh("docs");
    let model = doc.model();
    let documentation =
        |path: &str| model.get(at(model, path)).unwrap().data().documentation.clone();
    assert_eq!(documentation("docs.Document"), "A document.\n\nWith details,\n  indented.");
    assert_eq!(documentation("docs.Document.save"), "Saves it.");
    check("docs", model);
}

#[test]
fn bases_are_generalizations() {
    let (doc, _) = fresh("inheritance");
    check("inheritance", doc.model());
}

#[test]
fn generics() {
    let (doc, _) = fresh("generics");
    check("generics", doc.model());
}

#[test]
fn type_hints() {
    let (doc, _) = fresh("type_hints");
    let model = doc.model();
    assert_eq!(attribute_type(model, "hints.Hints", "either"), "int | str");
    assert!(links_to(model, "hints.Hints", "forward", "hints.Line"));
    check("type_hints", model);
}

#[test]
fn init_assigns_attributes() {
    let (doc, _) = fresh("init");
    let model = doc.model();
    at(model, "init.Point.x");
    at(model, "init.Point._cache");
    check("init", model);
}

#[test]
fn methods() {
    let (doc, _) = fresh("methods");
    check("methods", doc.model());
}

#[test]
fn names_resolve_through_imports_and_reexports() {
    let (doc, _) = fresh("resolution");
    let model = doc.model();
    check("resolution", model);
    assert!(links_to(model, "app.views.View", "order", "shop.orders.Order"));
    assert!(links_to(model, "app.views.View", "line", "shop.lines.Line"));
    assert!(links_to(model, "app.views.View", "aliased", "shop.orders.Order"));
    assert!(links_to(model, "app.views.View", "qualified", "shop.orders.Order"));
    assert!(links_to(model, "app.views.View", "checked", "shop.lines.Line"));
    assert!(links_to(model, "app.views.View", "helper", "app.helpers.Helper"));
    assert!(links_to(model, "app.views.View", "missing", "Missing"));
}

#[test]
fn attributes_of_class_types_become_associations() {
    let (doc, report) = fresh("associations");
    let model = doc.model();
    let end = |attribute: &str| association_end(model, "associations.Order", attribute);
    check("associations", model);
    assert_eq!(end("customer").multiplicity, None);
    assert_eq!(end("payer").multiplicity, Some(Multiplicity::OPTIONAL));
    assert_eq!(end("reader").multiplicity, Some(Multiplicity::OPTIONAL));
    assert_eq!(end("lines").constraints, ["ordered"]);
    assert_eq!(end("legacy").constraints, ["ordered"]);
    assert_eq!(end("queue").constraints, ["ordered"]);
    assert_eq!(end("tags").constraints, ["unique"]);
    assert_eq!(end("by_name").multiplicity, None);
    assert_eq!(end("old_map").multiplicity, None);
    // Without an annotation there is no type, so no association.
    at(model, "associations.Order.untyped");
    at(model, "associations.Order.unannotated");
    at(model, "associations.Order.note");
    assert_eq!(report.associations, 9);
}

#[test]
fn statements_outside_classes_are_counted() {
    let (doc, report) = fresh("statements");
    at(doc.model(), "statements.Kept");
    assert_eq!(skipped(&report, Skip::FreeMember), 2);
    assert_eq!(skipped(&report, Skip::Statement), 2);
}

#[test]
fn syntax_errors_skip_only_their_file() {
    let (doc, report) = fresh("syntax_error");
    at(doc.model(), "fine.Fine");
    assert_eq!(report.failed.len(), 1);
}

#[test]
fn encodings_and_stubs() {
    let (doc, report) = fresh("encoding");
    let model = doc.model();
    let documentation = model.get(at(model, "legacy.Legacy")).unwrap().data().documentation.clone();
    assert_eq!(documentation, "Grüße aus München.");
    at(model, "stub.Stub.run");
    assert_eq!(report.guessed_encodings.len(), 1);
}

#[test]
fn every_import_is_one_undo_step_and_repeats_without_change() {
    let cases = [
        "modules",
        "kinds",
        "visibility",
        "docs",
        "inheritance",
        "generics",
        "type_hints",
        "init",
        "methods",
        "resolution",
        "associations",
        "statements",
    ];
    for case in cases {
        let mut doc = Document::new(Model::new("t"));
        let before = doc.model().clone();
        import(&mut doc, case);
        let after = doc.model().clone();
        assert!(doc.undo());
        assert_eq!(*doc.model(), before, "{case}: undo");
        assert!(doc.redo());
        import(&mut doc, case);
        assert_eq!(*doc.model(), after, "{case}: second import");
    }
}

#[test]
fn a_new_import_matches_what_is_there() {
    let mut doc = Document::new(Model::new("t"));
    import(&mut doc, "reimport/v1");
    let model = doc.model();
    let order = at(model, "model.Order");
    let number = at(model, "model.Order.number");
    let ship = at(model, "model.Order.ship");
    import(&mut doc, "reimport/v2");
    let model = doc.model();
    assert_eq!(at(model, "model.Order"), order);
    assert_eq!(at(model, "model.Order.number"), number);
    assert_eq!(attribute_type(model, "model.Order", "number"), "str");
    let operations: Vec<Id> = model
        .children(order)
        .iter()
        .copied()
        .filter(|&c| matches!(model.get(c).unwrap().kind(), ElementKind::Operation(_)))
        .collect();
    assert_eq!(operations.len(), 2);
    assert!(operations.contains(&ship));
    check("reimport", model);
}

/// A parameter typed by a name imported under another name matches the operation on a new
/// import, which therefore keeps its id.
#[test]
fn parameters_typed_by_import_aliases_keep_their_operation() {
    let (mut doc, _) = fresh("alias_param");
    let add = at(doc.model(), "shop.cart.Cart.add");
    import(&mut doc, "alias_param");
    assert_eq!(at(doc.model(), "shop.cart.Cart.add"), add);
}

/// An import used only to qualify names (`nn.Module` after `import torch.nn as nn`) names a
/// module: `torch.nn` becomes a package, not also a class.
#[test]
fn imports_used_as_qualifiers_make_packages_only() {
    let (doc, _) = fresh("qualifier_import");
    let model = doc.model();
    let named = |owner: Id, name: &str| -> Vec<&'static str> {
        model
            .children(owner)
            .iter()
            .filter_map(|&c| model.get(c))
            .filter(|e| e.name() == name)
            .map(|e| e.kind().type_name())
            .collect()
    };
    assert_eq!(named(at(model, "torch"), "nn"), ["package"]);
    assert_eq!(named(at(model, "django"), "db"), ["package"]);
    at(model, "torch.nn.Module");
    at(model, "django.db.models.Model");
}
