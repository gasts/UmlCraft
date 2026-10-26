//! How Python names resolve and which of its types are values.

use crate::lang::Rules;
use crate::library::TableLibrary;

/// The containers and value types of the standard library, kept by hand in `stdlib.txt`.
pub static STDLIB: TableLibrary = TableLibrary::new(include_str!("stdlib.txt"));

pub static RULES: Rules = Rules {
    primitives: &["int", "float", "complex", "str", "bytes", "bool", "object", "None"],
    implicit_imports: &[&["builtins"]],
    library: &STDLIB,
    // `typing` and `collections` hold containers, whose elements count, and helpers such as
    // `Any` that are no association ends.
    value_packages: &[
        &["builtins"],
        &["typing"],
        &["collections"],
        &["collections", "abc"],
        &["datetime"],
        &["decimal"],
        &["uuid"],
        &["pathlib"],
        &["enum"],
    ],
    split_qualified,
    enclosing_namespaces: false,
    base_list_by_target: false,
    optional_modifiers: &[" | None"],
    type_qualifiers: &[],
    reexports: true,
};

/// PEP 8: modules are lower case, classes start with a capital letter. The module ends before
/// the first segment that starts with a capital letter.
fn split_qualified(path: &[String]) -> usize {
    path.iter()
        .position(|s| s.starts_with(|c: char| c.is_uppercase()))
        .unwrap_or(path.len().saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{Container, Library};

    fn s(path: &[&str]) -> Vec<String> {
        path.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn knows_containers() {
        let list = STDLIB.get(&s(&["builtins"]), &s(&["list"])).unwrap();
        assert_eq!(list.container, Container::Collection { ordered: true, unique: false });
        let optional = STDLIB.get(&s(&["typing"]), &s(&["Optional"])).unwrap();
        assert_eq!(optional.container, Container::Optional);
        assert!(STDLIB.is_package(&s(&["collections", "abc"])));
        assert_eq!((RULES.split_qualified)(&s(&["shop", "orders", "Order", "Line"])), 2);
    }
}
