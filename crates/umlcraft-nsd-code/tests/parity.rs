//! The same methods written in Java and in C# (`fixtures/java/X.java`, `fixtures/csharp/X.cs`)
//! become the same structograms: one mapping for both languages. Differences on purpose are
//! listed in `parity_deviations.txt`.

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
fn java_and_csharp_give_the_same_structograms() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let listed = deviations();
    let (mut compared, mut differing, mut unexpected) = (0, Vec::new(), Vec::new());
    for entry in std::fs::read_dir(fixtures.join("csharp")).unwrap() {
        let cs = entry.unwrap().path();
        let java = fixtures.join("java").join(cs.with_extension("java").file_name().unwrap());
        if cs.extension().is_none_or(|e| e != "cs") || !java.exists() {
            continue;
        }
        let (java, cs) = (trees(&java), trees(&cs));
        for (method, expected) in &java {
            let Some(got) = cs.get(method) else { continue };
            compared += 1;
            if got != expected {
                differing.push(method.clone());
                if !listed.contains_key(method) {
                    unexpected.push(format!("{method}\n--- Java\n{expected}--- C#\n{got}"));
                }
            }
        }
    }
    assert!(compared >= 20, "only {compared} methods compared");
    assert!(unexpected.is_empty(), "C# differs from Java:\n{}", unexpected.join("\n"));
    let stale: Vec<&String> = listed.keys().filter(|k| !differing.contains(k)).collect();
    assert!(stale.is_empty(), "listed deviations that no longer differ: {stale:?}");
}
