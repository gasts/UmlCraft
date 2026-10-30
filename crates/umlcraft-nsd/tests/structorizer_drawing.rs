//! Our drawing has the same lines as Structorizer's.
//!
//! With `UMLCRAFT_NSD_REFS` set (see `docs/architecture.md`), every reference diagram is drawn,
//! rasterized with resvg and compared with Structorizer's PNG export of it (made with its GUI
//! defaults and without syntax highlighting, `plain/…`): the straight dark lines of at least
//! 12 px must be the same, their ends within 2 px (anti-aliased diagonals meet them). Texts
//! are not compared; they are rendered by different engines. Files whose difference is
//! understood are listed with the reason in `tests/structorizer_deviations.txt`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use resvg::{tiny_skia, usvg};
use umlcraft_nsd::{Metrics, read, svg};

/// Reference sets: diagrams as Structorizer saved them, and its PNG exports of them on Linux.
const SETS: &[(&str, &str)] = &[
    ("korpus-kanonisch", "plain/korpus-kanonisch/png"),
    ("breiten-kanonisch", "plain/breiten/png"),
    ("struktur-kanonisch", "plain/struktur/png"),
    ("mess2-kanonisch", "plain/mess2/png"),
];

/// The same diagrams exported on Windows (by Ulf, with the same settings).
const WINDOWS_SETS: &[(&str, &str)] = &[
    ("korpus-kanonisch", "windows/korpus/png"),
    ("breiten-kanonisch", "windows/breiten/png"),
    ("struktur-kanonisch", "windows/struktur/png"),
];

/// Runs of dark pixels: (row, from, to) and (column, from, to).
type Runs = (BTreeSet<(u32, u32, u32)>, BTreeSet<(u32, u32, u32)>);

fn runs(pixmap: &tiny_skia::Pixmap) -> Runs {
    let (w, h) = (pixmap.width(), pixmap.height());
    let dark = |x: u32, y: u32| {
        let p = pixmap.pixel(x, y).unwrap().demultiply();
        p.alpha() > 0 && u32::from(p.red()) + u32::from(p.green()) + u32::from(p.blue()) < 200
    };
    let collect = |len: u32, across: u32, at: &dyn Fn(u32, u32) -> bool| {
        let mut out = BTreeSet::new();
        for a in 0..across {
            let mut start = None;
            for b in 0..=len {
                match (b < len && at(a, b), start) {
                    (true, None) => start = Some(b),
                    (false, Some(s)) => {
                        if b - s >= 12 {
                            out.insert((a, s, b - 1));
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        out
    };
    (collect(w, h, &|y, x| dark(x, y)), collect(h, w, &|x, y| dark(x, y)))
}

fn unmatched(a: &BTreeSet<(u32, u32, u32)>, b: &BTreeSet<(u32, u32, u32)>) -> Vec<(u32, u32, u32)> {
    a.iter()
        .filter(|l| {
            !b.iter().any(|m| l.0 == m.0 && l.1.abs_diff(m.1) <= 2 && l.2.abs_diff(m.2) <= 2)
        })
        .copied()
        .collect()
}

#[test]
fn drawings_match_structorizer() {
    compare(SETS, Metrics::linux(), "");
}

#[test]
fn drawings_match_structorizer_on_windows() {
    compare(WINDOWS_SETS, Metrics::windows(), "windows:");
}

/// Draws the diagrams of `sets` with `metrics` and compares them with the images; deviations
/// of these sets are listed with `prefix`.
fn compare(sets: &[(&str, &str)], metrics: &Metrics, prefix: &str) {
    let Some(refs) = std::env::var_os("UMLCRAFT_NSD_REFS") else {
        eprintln!("skipped: UMLCRAFT_NSD_REFS is not set");
        return;
    };
    let refs = PathBuf::from(refs);
    if !refs.join(sets[0].1).is_dir() {
        eprintln!("skipped: {} is missing", sets[0].1);
        return;
    }
    let deviations = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structorizer_deviations.txt"),
    )
    .unwrap();
    let allowed: Vec<&str> = deviations
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let (mut checked, mut failures) = (0, Vec::new());
    for (diagrams, images) in sets {
        let mut files: Vec<PathBuf> = std::fs::read_dir(refs.join(diagrams))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "nsd"))
            .collect();
        files.sort();
        for file in files {
            let name = file.file_stem().unwrap().to_string_lossy().into_owned();
            let png = refs.join(images).join(format!("{name}.png"));
            // Structorizer cannot draw some (a case with only a default branch).
            let Ok(expected) = tiny_skia::Pixmap::load_png(&png) else { continue };
            let diagram = read(&std::fs::read_to_string(&file).unwrap()).unwrap();
            let tree = usvg::Tree::from_str(&svg::render(&diagram, metrics), &options).unwrap();
            let size = tree.size().to_int_size();
            let mut ours = tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
            resvg::render(&tree, tiny_skia::Transform::identity(), &mut ours.as_mut());
            checked += 1;
            let key = format!("{prefix}{diagrams}/{name}");
            let mut problems = Vec::new();
            if (ours.width(), ours.height()) != (expected.width(), expected.height()) {
                problems.push(format!(
                    "size {}x{} instead of {}x{}",
                    ours.width(),
                    ours.height(),
                    expected.width(),
                    expected.height()
                ));
            } else {
                let (ours, expected) = (runs(&ours), runs(&expected));
                let missing = [unmatched(&expected.0, &ours.0), unmatched(&expected.1, &ours.1)];
                let extra = [unmatched(&ours.0, &expected.0), unmatched(&ours.1, &expected.1)];
                if missing.iter().chain(&extra).any(|l| !l.is_empty()) {
                    problems.push(format!("missing {missing:?}, extra {extra:?}"));
                }
            }
            match (problems.is_empty(), allowed.contains(&key.as_str())) {
                (false, false) => failures.push(format!("{key}: {}", problems.join("; "))),
                (true, true) => failures.push(format!("{key}: matches, drop it from the list")),
                _ => {}
            }
        }
    }
    eprintln!("{checked} drawings compared");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
