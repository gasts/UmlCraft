//! Compares the object graphs of two `.asta` files, to see what Astah changes or creates for an
//! edit: `asta-diff a.asta b.asta`.

use std::path::PathBuf;

fn main() {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [a, b] = &files[..] else {
        eprintln!("usage: asta-diff a.asta b.asta");
        std::process::exit(2);
    };
    let load = |p: &PathBuf| {
        let bytes = umlcraft_asta::read_entity_store(p).expect("read");
        umlcraft_asta::parse_entity_store(&bytes).expect("parse")
    };
    let (a, b) = (load(a), load(b));
    let lines = umlcraft_asta::diff_graphs(&a, &b).expect("compare");
    for line in &lines {
        println!("{line}");
    }
    println!("{} differences", lines.len());
}
