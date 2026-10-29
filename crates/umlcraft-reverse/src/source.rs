//! Finding source files and reading them as text.

use std::io;
use std::path::{Path, PathBuf};

/// The files among `paths` with one of `extensions` (without the dot, compared ignoring case):
/// files as given, directories searched recursively. Sorted and without duplicates, so imports
/// run in the same order on every system.
///
/// The search leaves out hidden folders (`.git`, `.venv`), virtual environments (folders with a
/// `pyvenv.cfg`) and the usual folders of build output and fetched dependencies
/// ([`SKIPPED_FOLDERS`]). A folder below the given ones that cannot be read is listed as it is,
/// so that reading it fails and is reported with the files that could not be read; the others
/// are imported anyway. Where a Python module has a stub (`orders.pyi` next to `orders.py`),
/// only the stub is taken: both declare the same module.
pub fn collect_sources(paths: &[PathBuf], extensions: &[&str]) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for path in paths {
        if path.is_dir() {
            walk(path, extensions, &mut found)?;
        } else if has_extension(path, extensions) {
            found.push(path.clone());
        }
    }
    found.sort();
    found.dedup();
    let stubs: std::collections::HashSet<PathBuf> = found
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pyi")))
        .map(|p| p.with_extension(""))
        .collect();
    found.retain(|p| {
        !(p.extension().is_some_and(|e| e.eq_ignore_ascii_case("py"))
            && stubs.contains(&p.with_extension("")))
    });
    Ok(found)
}

/// Folders of build output, packaging and fetched dependencies, not searched for sources.
pub const SKIPPED_FOLDERS: &[&str] =
    &["node_modules", "venv", "site-packages", "__pycache__", "bin", "obj", "build", "_deps"];

fn skipped_folder(dir: &Path) -> bool {
    let Some(name) = dir.file_name().and_then(|n| n.to_str()) else { return false };
    name.starts_with('.') || SKIPPED_FOLDERS.contains(&name) || dir.join("pyvenv.cfg").is_file()
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

fn walk(dir: &Path, extensions: &[&str], found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            if skipped_folder(&path) {
                continue;
            }
            // Reading the folder as a file fails too, with the folder's path in the report.
            if walk(&path, extensions, found).is_err() {
                found.push(path);
            }
        } else if has_extension(&path, extensions) {
            found.push(path);
        }
    }
    Ok(())
}

/// The Python module a file is: its name without extension (none for `__init__`) under the
/// folders above it that are packages, which hold an `__init__.py` (or `.pyi`), outermost first.
/// `shop/orders.py` is `shop.orders`, `shop/__init__.py` is `shop`. The first folder that is no
/// package ends the path, so the files of a namespace package start a module of their own.
pub fn python_module(path: &Path) -> Vec<String> {
    let mut module = Vec::new();
    match path.file_stem().and_then(|s| s.to_str()) {
        Some("__init__") | None => {}
        Some(stem) => module.push(stem.to_owned()),
    }
    let is_package = |dir: &Path| {
        !dir.as_os_str().is_empty()
            && (dir.join("__init__.py").is_file() || dir.join("__init__.pyi").is_file())
    };
    let mut dir = path.parent();
    while let Some(d) = dir.filter(|d| is_package(d)) {
        let Some(name) = d.file_name().and_then(|n| n.to_str()) else { break };
        module.push(name.to_owned());
        dir = d.parent();
    }
    module.reverse();
    module
}

/// A source file as text, with line ends normalized to `\n`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub text: String,
    /// Set when the encoding had to be guessed: the name of the encoding used.
    pub guessed: Option<&'static str>,
}

/// Decodes a source file: a byte order mark decides between UTF-8 and UTF-16; without one,
/// valid UTF-8 is taken as such and anything else as Windows-1252, the usual legacy encoding of
/// sources written on Windows.
pub fn decode(bytes: &[u8]) -> Decoded {
    let (text, guessed) = match encoding_rs::Encoding::for_bom(bytes) {
        Some((encoding, bom)) => (encoding.decode_without_bom_handling(&bytes[bom..]).0, None),
        None => match std::str::from_utf8(bytes) {
            Ok(text) => (text.into(), None),
            Err(_) => {
                let encoding = encoding_rs::WINDOWS_1252;
                (encoding.decode_without_bom_handling(bytes).0, Some(encoding.name()))
            }
        },
    };
    let text = if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.into_owned()
    };
    Decoded { text, guessed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_by_bom_then_utf8_then_windows_1252() {
        assert_eq!(decode(b"\xEF\xBB\xBFclass \xC3\x84 {}").text, "class Ä {}");
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("Ä\r\n".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(decode(&utf16), Decoded { text: "Ä\n".into(), guessed: None });
        assert_eq!(decode("Ä\r\nb\rc".as_bytes()).text, "Ä\nb\nc");
        let legacy = decode(b"// Gr\xFC\xDFe");
        assert_eq!(legacy.text, "// Grüße");
        assert_eq!(legacy.guessed, Some("windows-1252"));
    }

    #[test]
    fn collects_recursively_by_extension_in_order() {
        let dir = std::env::temp_dir().join(format!("umlcraft-collect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("b/c")).unwrap();
        for f in ["b/c/Z.java", "b/A.JAVA", "b/readme.txt", "Top.java"] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let found = collect_sources(&[dir.clone(), dir.join("Top.java")], &["java"]).unwrap();
        let names: Vec<String> = found
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(names, ["Top.java", "b/A.JAVA", "b/c/Z.java"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn skips_build_and_vendor_folders_and_prefers_stubs() {
        let dir = std::env::temp_dir().join(format!("umlcraft-skip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["shop", ".git", "obj/Debug", "env", "node_modules/x"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        for f in [
            "shop/orders.py",
            "shop/orders.pyi",
            "shop/cart.py",
            ".git/hook.py",
            "obj/Debug/gen.py",
            "env/pyvenv.cfg",
            "env/lib.py",
            "node_modules/x/y.py",
        ] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let found = collect_sources(std::slice::from_ref(&dir), &["py", "pyi"]).unwrap();
        let names: Vec<String> = found
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(names, ["shop/cart.py", "shop/orders.pyi"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn python_modules_follow_the_packages_above() {
        let dir = std::env::temp_dir().join(format!("umlcraft-module-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src/shop/billing")).unwrap();
        std::fs::create_dir_all(dir.join("src/loose")).unwrap();
        for f in [
            "src/shop/__init__.py",
            "src/shop/orders.py",
            "src/shop/billing/__init__.pyi",
            "src/shop/billing/tax.pyi",
            "src/loose/helper.py",
        ] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let module = |f: &str| python_module(&dir.join(f)).join(".");
        assert_eq!(module("src/shop/orders.py"), "shop.orders");
        assert_eq!(module("src/shop/__init__.py"), "shop");
        assert_eq!(module("src/shop/billing/tax.pyi"), "shop.billing.tax");
        // `src` and `loose` hold no `__init__.py`.
        assert_eq!(module("src/loose/helper.py"), "helper");
        assert_eq!(python_module(Path::new("main.py")), ["main"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
