//! Files Structorizer 3.32-35 wrote read and write back byte for byte.
//!
//! `tests/fixtures/structorizer` holds our own diagrams, each showing one feature, as
//! Structorizer saved them (`12-alt-gui.nsd` through its GUI, the others through its own XML
//! writer). With `UMLCRAFT_NSD_REFS` pointing at a folder of further reference files (see
//! `docs/architecture.md`), `reference_files_round_trip` checks all of those too.

use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use umlcraft_nsd::{read, write};

fn nsd_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            nsd_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "nsd") {
            out.push(path);
        }
    }
}

fn round_trip(path: &Path) {
    let file = std::fs::read_to_string(path).unwrap();
    let diagram = read(&file).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(write(&diagram), file, "{}", path.display());
}

#[test]
fn fixtures_round_trip() {
    let mut files = Vec::new();
    nsd_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/structorizer"),
        &mut files,
    );
    assert_eq!(files.len(), 44);
    for path in &files {
        round_trip(path);
    }
}

/// The folders of `UMLCRAFT_NSD_REFS` that hold files Structorizer wrote.
const REFERENCE_FOLDERS: &[&str] = &[
    "samples",
    "java",
    "korpus-kanonisch",
    "korpus-gui",
    "breiten-kanonisch",
    "struktur-kanonisch",
];

#[test]
fn reference_files_round_trip() {
    let Some(refs) = std::env::var_os("UMLCRAFT_NSD_REFS") else {
        eprintln!("skipped: UMLCRAFT_NSD_REFS is not set");
        return;
    };
    let mut files = Vec::new();
    for folder in REFERENCE_FOLDERS {
        let dir = Path::new(&refs).join(folder);
        assert!(dir.is_dir(), "{} is missing", dir.display());
        nsd_files(&dir, &mut files);
    }
    for path in &files {
        round_trip(path);
    }
    eprintln!("{} reference files round-trip", files.len());
}
