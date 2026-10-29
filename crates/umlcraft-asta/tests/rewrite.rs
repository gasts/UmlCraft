//! Reads every `.asta` file that ships with Astah Professional, plus the local examples, and
//! writes its entity store back: the bytes must come out unchanged. Skipped when Astah is not
//! installed.

use std::path::{Path, PathBuf};

/// Astah's install folder: `ASTAH_HOME`, else where Astah Professional installs on Windows.
fn astah_home() -> PathBuf {
    std::env::var_os("ASTAH_HOME")
        .map_or_else(|| PathBuf::from(r"C:\Program Files\astah-professional"), PathBuf::from)
}

/// Reports a test that cannot run here. With `UMLCRAFT_REQUIRE_ASTAH` set, as on a machine that
/// has Astah, a skip fails instead, so a missing installation cannot pass for a green run.
fn skip(reason: &str) {
    assert!(std::env::var_os("UMLCRAFT_REQUIRE_ASTAH").is_none(), "not skipped: {reason}");
    eprintln!("skipped: {reason}");
}

fn asta_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            asta_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "asta") {
            out.push(path);
        }
    }
}

#[test]
fn entity_stores_are_written_back_unchanged() {
    let mut files = Vec::new();
    asta_files(&astah_home(), &mut files);
    if files.is_empty() {
        skip(&format!("no Astah installation at {}", astah_home().display()));
        return;
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    asta_files(&repo.join("examples"), &mut files);
    asta_files(&repo.join("docs"), &mut files);
    for path in &files {
        let bytes = umlcraft_asta::read_entity_store(path).unwrap();
        let graph = umlcraft_asta::parse_entity_store(&bytes).unwrap();
        let written = umlcraft_asta::write_entity_store(&graph).unwrap();
        if let Some(at) = bytes.iter().zip(&written).position(|(a, b)| a != b) {
            panic!("{}: differs at byte {at} of {}", path.display(), bytes.len());
        }
        assert_eq!(written.len(), bytes.len(), "{}", path.display());
    }
}
