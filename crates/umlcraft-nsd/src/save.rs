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

/// The date (`YYYY-MM-DD`, UTC) of a time in seconds since 1970.
pub fn date(unix_seconds: u64) -> String {
    // Days to civil date, after Howard Hinnant's `civil_from_days`.
    let z = (unix_seconds / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read, write};

    #[test]
    fn dates_are_civil() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(date(1_793_232_000), "2026-10-29");
        assert_eq!(date(1_793_318_399), "2026-10-29");
    }

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
