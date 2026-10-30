//! The same methods written in Java and in another language (`fixtures/java/X.java`,
//! `fixtures/csharp/X.cs`, `fixtures/python/X.py`, `fixtures/cpp/X.cpp`) become the same
//! structograms: one mapping
//! for every language. Differences on purpose are listed in `parity_deviations.txt` as
//! `folder/Class.method`.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::{MADE, tree};
use umlcraft_nsd_code::{Language, Source};

/// Every method of a file as its tree, by `Class.method`.
fn trees(path: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap();
    let source = Source::parse(Language::of(path).unwrap(), &text);
    source
        .methods()
        .iter()
        .map(|m| {
            let (d, _) = source.diagram(m, &MADE).unwrap();
            (format!("{}.{}", m.class_name(), m.name), tree(&d))
        })
        .collect()
}

fn deviations() -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/parity_deviations.txt");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_once(": ").map(|(k, v)| (k.to_owned(), v.to_owned())))
        .collect()
}

#[test]
fn every_language_gives_the_same_structograms_as_java() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let listed = deviations();
    let (mut differing, mut unexpected) = (Vec::new(), Vec::new());
    for (folder, extension) in [("csharp", "cs"), ("python", "py"), ("cpp", "cpp")] {
        let mut compared = 0;
        for entry in std::fs::read_dir(fixtures.join(folder)).unwrap() {
            let other = entry.unwrap().path();
            let java =
                fixtures.join("java").join(other.with_extension("java").file_name().unwrap());
            if other.extension().is_none_or(|e| e != extension) || !java.exists() {
                continue;
            }
            let (java, other) = (trees(&java), trees(&other));
            for (method, expected) in &java {
                let Some(got) = other.get(method) else { continue };
                compared += 1;
                let key = format!("{folder}/{method}");
                if got != expected {
                    if !listed.contains_key(&key) {
                        unexpected.push(format!("{key}\n--- Java\n{expected}--- {folder}\n{got}"));
                    }
                    differing.push(key);
                }
            }
        }
        assert!(compared >= 15, "{folder}: only {compared} methods compared");
    }
    assert!(unexpected.is_empty(), "differs from Java:\n{}", unexpected.join("\n"));
    let stale: Vec<&String> = listed.keys().filter(|k| !differing.contains(k)).collect();
    assert!(stale.is_empty(), "listed deviations that no longer differ: {stale:?}");
}
