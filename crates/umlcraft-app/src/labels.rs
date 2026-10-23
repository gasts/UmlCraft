//! Localized names for model elements.

use rust_i18n::t;
use umlcraft_model::{Element, Id, Model, Visibility};

pub fn kind_label(e: &Element) -> String {
    t!(format!("kind.{}", e.kind().type_name())).to_string()
}

/// The element's name, or its kind in parentheses when it has none.
pub fn display_name(e: &Element) -> String {
    if e.name().is_empty() { format!("({})", kind_label(e)) } else { e.name().to_owned() }
}

pub fn owner_path(model: &Model, id: Id) -> String {
    let names: Vec<&str> =
        model.ancestors(id).iter().rev().filter_map(|&a| model.get(a)).map(Element::name).collect();
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
