//! What saving changes in a diagram's header, as Structorizer does it: a changed diagram gets
//! the version whose format is written, who changed it and when. New diagrams start like
//! Structorizer's, named `???`.

use crate::create::{Provenance, STRUCTORIZER_VERSION};
use crate::model::{Diagram, RootKind};

/// Marks `d` as changed by `user` on `date` (`YYYY-MM-DD`).
pub fn stamp(d: &mut Diagram, user: &str, date: &str) {
    d.set_header("version", STRUCTORIZER_VERSION);
    d.set_header("changedby", user);
    d.set_header("changed", date);
}

/// A new, empty program made by `user` on `date` with `origin` (e.g. `UmlCraft 0.9.0`).
pub fn new_diagram(user: &str, date: &str, origin: &str) -> Diagram {
    let made = Provenance { author: user, date, origin };
    Diagram::new(RootKind::Program, &["???".to_owned()], &made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read, write};

    #[test]
    fn stamps_go_where_structorizer_writes_them() {
        let mut d = new_diagram("ann", "2026-10-01", "UmlCraft 0.9.0");
        d.set_header("version", "3.30");
        stamp(&mut d, "bob", "2026-10-29");
        let file = write(&d);
        assert!(file.contains(r#"version="3.32-35""#));
        assert!(file.contains(
            r#"author="ann" created="2026-10-01" changedby="bob" changed="2026-10-29" origin="UmlCraft 0.9.0" text="&#34;???&#34;""#
        ));
        assert_eq!(read(&file).unwrap(), d);
    }
}
