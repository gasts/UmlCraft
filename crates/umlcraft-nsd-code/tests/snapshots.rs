//! Every method of the fixture files (Java and C#) as a structogram, written as a readable
//! tree and compared with the snapshot next to the file (`UPDATE_SNAPSHOTS=1` rewrites them).
//! Each diagram must also read back as it was written.

mod common;

use std::fmt::Write;
use std::path::Path;

use common::{MADE, tree};
use umlcraft_nsd::{read, write};
use umlcraft_nsd_code::{Language, Source};

fn snapshot(file: &Path) -> String {
    let text = std::fs::read_to_string(file).unwrap();
    let source = Source::parse(Language::of(file).unwrap(), &text);
    let mut out = String::new();
    for m in source.methods() {
        let (d, report) = source.diagram(&m, &MADE).unwrap();
        assert_eq!(read(&write(&d)).unwrap(), d, "{}", m.name);
        writeln!(out, "== {}.{} (line {})", m.class_name(), m.name, m.line).unwrap();
        out.push_str(&tree(&d));
        for note in &report.notes {
            writeln!(out, "! line {}: {:?}", note.line, note.kind).unwrap();
        }
        out.push('\n');
    }
    out
}

#[test]
fn fixtures_match_their_snapshots() {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    for (folder, extension, at_least) in [("java", "java", 10), ("csharp", "cs", 8)] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(folder);
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == extension))
            .collect();
        files.sort();
        assert!(files.len() >= at_least, "{folder}");
        for file in files {
            let got = snapshot(&file);
            let path = file.with_extension("txt");
            if update {
                std::fs::write(&path, &got).unwrap();
                continue;
            }
            let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
                panic!("{} is missing; run with UPDATE_SNAPSHOTS=1", path.display())
            });
            pretty_assertions::assert_eq!(expected, got, "{}", file.display());
        }
    }
}
