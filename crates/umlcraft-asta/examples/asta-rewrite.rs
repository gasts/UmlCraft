//! Reads an `.asta` file and writes its object graph to a new file, to check that Astah opens
//! what the stream writer produces: `asta-rewrite in.asta out.asta`.

use std::path::PathBuf;

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [input, output] = &args[..] else {
        eprintln!("usage: asta-rewrite in.asta out.asta");
        std::process::exit(2);
    };
    let bytes = umlcraft_asta::read_entity_store(input).expect("read");
    let graph = umlcraft_asta::parse_entity_store(&bytes).expect("parse");
    let written = umlcraft_asta::write_entity_store(&graph).expect("write");
    umlcraft_asta::write_asta(output, &written).expect("save");
    println!(
        "{} objects, {} bytes, unchanged: {}",
        graph.objects.len(),
        written.len(),
        written == bytes
    );
}
