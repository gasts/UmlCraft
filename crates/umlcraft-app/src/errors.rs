//! Error messages in the language of the UI, naming elements instead of showing their ids.

use rust_i18n::t;
use umlcraft_format::FormatError;
use umlcraft_model::{Id, Model, ModelError, ParseMultiplicityError};

use crate::labels::display_name;

/// An element by its name; `model` is the one the error came from, if it is still at hand.
fn named(model: Option<&Model>, id: Id) -> String {
    match model.and_then(|m| m.get(id)) {
        Some(e) => t!("error.named", name = display_name(e)).to_string(),
        None => t!("error.unnamed").to_string(),
    }
}

fn kind(type_name: &str) -> String {
    t!(format!("kind.{type_name}")).to_string()
}

/// Why the model refused an edit, or why a file holds no valid model.
pub fn model_error(model: Option<&Model>, err: &ModelError) -> String {
    let a = |id: Id| named(model, id);
    match err {
        ModelError::NotFound(id) => t!("error.not_found", a = a(*id)),
        ModelError::DuplicateId(_) => t!("error.duplicate_id"),
        ModelError::InvalidOwner { owner, owner_type, child, child_type } => t!(
            "error.invalid_owner",
            child = a(*child),
            child_kind = kind(child_type),
            owner = a(*owner),
            owner_kind = kind(owner_type)
        ),
        ModelError::IndexOutOfRange { .. } => t!("error.index_out_of_range"),
        ModelError::Root => t!("error.root"),
        ModelError::HasChildren(id) => t!("error.has_children", a = a(*id)),
        ModelError::StillReferenced { target, referrer } => {
            t!("error.still_referenced", a = a(*target), b = a(*referrer))
        }
        ModelError::StillShown { target, diagram } => {
            t!("error.still_shown", a = a(*target), b = a(*diagram))
        }
        ModelError::InvalidReference { from, .. } => t!("error.invalid_reference", a = a(*from)),
        ModelError::KindChanged(id) => t!("error.kind_changed", a = a(*id)),
        ModelError::OwnershipCycle(id) => t!("error.ownership_cycle", a = a(*id)),
        ModelError::GeneralizationCycle(_) => t!("error.generalization_cycle"),
        ModelError::NotADiagram(id) => t!("error.not_a_diagram", a = a(*id)),
        ModelError::DiagramNotEmpty(id) => t!("error.diagram_not_empty", a = a(*id)),
        ModelError::EdgesAttached(_) => t!("error.edges_attached"),
        ModelError::ShapeChanged(_) => t!("error.shape_changed"),
        ModelError::EdgeCycle(_) => t!("error.edge_cycle"),
        ModelError::ReferenceCycle(id) => t!("error.reference_cycle", a = a(*id)),
        ModelError::NotFinite(_) => t!("error.not_finite"),
        ModelError::TypeTooDeep(id) => {
            t!("error.type_too_deep", a = a(*id), max = umlcraft_model::MAX_TYPE_DEPTH)
        }
        ModelError::Invalid(detail) => t!("error.invalid", detail = detail),
    }
    .to_string()
}

/// Why a project file could not be read or written.
pub fn format_error(err: &FormatError) -> String {
    match err {
        // The system's own text, which Windows gives in the user's language.
        FormatError::Io(err) => err.to_string(),
        FormatError::Json(err) => {
            t!("error.malformed", line = err.line(), column = err.column()).to_string()
        }
        FormatError::NotAProject => t!("error.not_a_project").to_string(),
        FormatError::TooNew { found, supported } => {
            t!("error.too_new", found = found, supported = supported).to_string()
        }
        FormatError::Migration { from, reason } => {
            t!("error.migration", from = from, reason = reason).to_string()
        }
        FormatError::Model(err) => model_error(None, err),
    }
}

pub fn multiplicity_error(err: &ParseMultiplicityError) -> String {
    t!("error.multiplicity", text = err.0).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use umlcraft_model::sample::order_system;

    #[test]
    fn messages_name_elements_instead_of_ids() {
        let (doc, ids) = order_system();
        let err = ModelError::HasChildren(ids.order);
        let text = model_error(Some(doc.model()), &err);
        assert!(text.contains("Order") && !text.contains(&ids.order.to_string()), "{text}");
        let text = model_error(None, &err);
        assert!(!text.contains(&ids.order.to_string()) && !text.contains("error."), "{text}");
    }
}
