//! Localized names for model elements.

use rust_i18n::t;
use umlcraft_model::{Element, ElementKind, Id, Model, Visibility};
use umlcraft_notation::palette::NodeTool;
use umlcraft_notation::sequence::{interaction_use_text, message_text, operator_keyword};

/// The element's metaclass; activity nodes, pseudostates and instances say which kind they are.
pub fn kind_label(e: &Element) -> String {
    let key = match e.kind() {
        ElementKind::ActivityNode(p) => NodeTool::for_activity_node(p.node).key(),
        ElementKind::Pseudostate(p) => NodeTool::for_pseudostate(p.pseudostate).key(),
        ElementKind::Instance(p) => NodeTool::for_instance(p.instance).key(),
        _ if umlcraft_notation::palette::is_subsystem(e) => NodeTool::Subsystem.key(),
        kind => kind.type_name(),
    };
    t!(format!("kind.{key}")).to_string()
}

/// The element's name on one line, or its kind in parentheses when it has none.
pub fn display_name(e: &Element) -> String {
    if e.name().is_empty() { format!("({})", kind_label(e)) } else { one_line(e.name()) }
}

/// Like [`display_name`], but unnamed sequence elements and flows show what the diagram shows:
/// a message its label, a fragment its operator, an operand or flow its guard. Always one line:
/// line breaks, as in use case names, become spaces.
pub fn tree_name(model: &Model, e: &Element) -> String {
    one_line(&shown_name(model, e))
}

fn shown_name(model: &Model, e: &Element) -> String {
    let guard =
        |g: &str| if g.trim().is_empty() { String::new() } else { format!("[{}]", g.trim()) };
    let shown = match e.kind() {
        ElementKind::Slot(p) => format!("{} = {}", e.name(), p.value),
        _ if !e.name().is_empty() => String::new(),
        ElementKind::Message(_) => message_text(model, e.id()),
        ElementKind::CombinedFragment(p) => operator_keyword(p.operator).to_owned(),
        ElementKind::InteractionUse(_) => interaction_use_text(model, e.id()),
        ElementKind::InteractionOperand(p) => guard(&p.guard),
        ElementKind::ActivityEdge(p) => guard(&p.guard),
        ElementKind::Transition(p) => p.label(),
        ElementKind::InternalTransition(p) => p.label(),
        ElementKind::Class(_) => umlcraft_notation::member::display_name(model, e.id()),
        _ => String::new(),
    };
    if shown.is_empty() { display_name(e) } else { shown }
}

pub fn owner_path(model: &Model, id: Id) -> String {
    let names: Vec<String> = model
        .ancestors(id)
        .iter()
        .rev()
        .filter_map(|&a| model.get(a))
        .map(|e| tree_name(model, e))
        .collect();
    if names.is_empty() { "—".to_owned() } else { names.join(" › ") }
}

pub const VISIBILITIES: [Visibility; 4] =
    [Visibility::Public, Visibility::Protected, Visibility::Package, Visibility::Private];

pub fn visibility_label(v: Visibility) -> String {
    let key = match v {
        Visibility::Public => "visibility.public",
        Visibility::Protected => "visibility.protected",
        Visibility::Package => "visibility.package",
        Visibility::Private => "visibility.private",
    };
    format!("{} {}", v.symbol(), t!(key))
}

/// `text` with every line break turned into a space.
fn one_line(text: &str) -> String {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::one_line;

    #[test]
    fn line_breaks_become_spaces() {
        assert_eq!(
            one_line("Wareneingang\naus Einkauf \r\n\nbearbeiten"),
            "Wareneingang aus Einkauf bearbeiten"
        );
        assert_eq!(one_line("Kunde"), "Kunde");
    }
}
