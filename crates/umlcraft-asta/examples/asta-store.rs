//! Lists what the entity root's `store` set of an `.asta` file holds, by class:
//! `asta-store file.asta`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use umlcraft_asta::jser::{Content, Value};

fn main() {
    let path = PathBuf::from(std::env::args_os().nth(1).expect("usage: asta-store file.asta"));
    let bytes = umlcraft_asta::read_entity_store(&path).expect("read");
    let graph = umlcraft_asta::parse_entity_store(&bytes).expect("parse");
    let root = graph
        .roots
        .iter()
        .filter_map(|c| match c {
            Content::Value(Value::Object(id)) => Some(*id),
            _ => None,
        })
        .find(|&id| graph.class_name(id) == Some("JP.co.esm.caddies.golf.model.EntityRoot"))
        .expect("no entity root");
    let store = graph.field(root, "store").and_then(Value::as_object).expect("no store");
    let mut counts = BTreeMap::<&str, usize>::new();
    let items = graph.collection(store);
    for v in &items {
        let class = v.as_object().and_then(|id| graph.class_name(id)).unwrap_or("?");
        *counts.entry(class.rsplit('.').next().unwrap()).or_default() += 1;
    }
    let mut instances = BTreeMap::<&str, usize>::new();
    for id in 0..graph.objects.len() {
        let id = umlcraft_asta::jser::ObjId(id);
        if graph.field(id, "id").is_some() {
            let class = graph.class_name(id).unwrap_or("?");
            *instances.entry(class.rsplit('.').next().unwrap()).or_default() += 1;
        }
    }
    println!(
        "store: {} items; objects with an id: {}",
        items.len(),
        instances.values().sum::<usize>()
    );
    for (class, n) in &instances {
        println!("  {class}: {} of {n}", counts.get(class).copied().unwrap_or(0));
    }
    for (class, n) in counts.iter().filter(|(c, _)| !instances.contains_key(*c)) {
        println!("  {class}: {n} (no id)");
    }
}
