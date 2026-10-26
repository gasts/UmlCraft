//! Reads every `.asta` file that ships with Astah Professional, plus the local examples, and
//! writes its entity store back: the bytes must come out unchanged. Skipped when Astah is not
//! installed.

use std::path::{Path, PathBuf};

const ASTAH: &str = r"C:\Program Files\astah-professional";

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
    asta_files(Path::new(ASTAH), &mut files);
    if files.is_empty() {
        eprintln!("skipped: no Astah installation at {ASTAH}");
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
