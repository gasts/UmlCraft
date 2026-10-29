//! Draws .nsd files as SVG images next to them, or into a folder:
//! `cargo run -p umlcraft-nsd --example nsd-svg -- [-o FOLDER] FILE.nsd...`

use std::path::{Path, PathBuf};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut folder = None;
    if args.first().is_some_and(|a| a == "-o") {
        folder = Some(PathBuf::from(&args[1]));
        args.drain(..2);
    }
    let m = umlcraft_nsd::Metrics::linux();
    for file in args {
        let path = Path::new(&file);
        let text = std::fs::read_to_string(path).expect("readable file");
        match umlcraft_nsd::read(&text) {
            Ok(diagram) => {
                let target = match &folder {
                    Some(f) => f.join(path.file_name().unwrap()).with_extension("svg"),
                    None => path.with_extension("svg"),
                };
                std::fs::write(&target, umlcraft_nsd::svg::render(&diagram, m)).unwrap();
            }
            Err(e) => eprintln!("{file}: {e}"),
        }
    }
}
