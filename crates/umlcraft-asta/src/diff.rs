//! Compares two object graphs, to see what one Astah project has that another lacks.
//!
//! Both graphs are walked together from their roots; objects met at the same place are paired,
//! and every difference is reported with its path. Fields that only identify objects or count
//! changes (`id`, `changeId` …) are skipped, and sets are compared by size only, since their
//! order follows hash codes.

use std::collections::HashMap;

use crate::jser::{Content, Graph, ObjId, Object, Value};

/// Fields that identify objects or record when they changed.
const NOISE: &[&str] = &["ex_changed", "version", "uid", "changeId", "id", "lastModifiedTime"];

/// The differences between `a` and `b`, one line each.
pub fn diff(a: &Graph, b: &Graph) -> Vec<String> {
    let mut diff = Diff { a, b, pairs: HashMap::new(), out: Vec::new() };
    let roots = |g: &Graph| -> Vec<Value> {
        g.roots
            .iter()
            .filter_map(|c| match c {
                Content::Value(v) => Some(v.clone()),
                _ => None,
            })
            .collect()
    };
    diff.list("", &roots(a), &roots(b));
    diff.out
}

struct Diff<'a> {
    a: &'a Graph,
    b: &'a Graph,
    pairs: HashMap<ObjId, ObjId>,
    out: Vec<String>,
}

fn short(class: &str) -> &str {
    class.rsplit('.').next().unwrap_or(class)
}

fn values(items: &[Content]) -> Vec<Value> {
    items
        .iter()
        .filter_map(|c| match c {
            Content::Value(v) => Some(v.clone()),
            _ => None,
        })
        .collect()
}

fn blocks(items: &[Content]) -> Vec<u8> {
    items
        .iter()
        .flat_map(|c| match c {
            Content::Block(b) => b.clone(),
            _ => Vec::new(),
        })
        .collect()
}

impl Diff<'_> {
    fn describe(g: &Graph, v: &Value) -> String {
        match v {
            Value::Object(id) => match g.object(*id) {
                Object::String(s) => format!("{s:?}"),
                Object::Enum { constant, .. } => constant.clone(),
                _ => format!("#{} {}", id.0, short(g.class_name(*id).unwrap_or("?"))),
            },
            other => format!("{other:?}"),
        }
    }

    fn report(&mut self, path: &str, a: &Value, b: &Value) {
        let line =
            format!("{path}: {} -> {}", Self::describe(self.a, a), Self::describe(self.b, b));
        self.out.push(line);
    }

    fn value(&mut self, path: &str, a: &Value, b: &Value) {
        let (Value::Object(x), Value::Object(y)) = (a, b) else {
            if a != b {
                self.report(path, a, b);
            }
            return;
        };
        // Strings count by their text; whether two places share one string object is chance.
        if let (Some(s), Some(t)) = (self.a.string(*x), self.b.string(*y)) {
            if s != t {
                self.report(path, a, b);
            }
            return;
        }
        if let Some(&paired) = self.pairs.get(x) {
            if paired != *y {
                self.report(path, a, b);
            }
            return;
        }
        self.pairs.insert(*x, *y);
        match (self.a.object(*x), self.b.object(*y)) {
            (Object::String(s), Object::String(t)) => {
                if s != t {
                    self.report(path, a, b);
                }
            }
            (Object::Enum { constant: s, .. }, Object::Enum { constant: t, .. }) => {
                if s != t {
                    self.report(path, a, b);
                }
            }
            (Object::Array { items: s, .. }, Object::Array { items: t, .. }) => {
                self.list(path, s, t);
            }
            (Object::Instance { data: da, .. }, Object::Instance { data: db, .. })
                if self.a.class_name(*x) == self.b.class_name(*y) =>
            {
                let class = self.a.class_name(*x).unwrap_or("?");
                if class == "java.util.HashSet" {
                    let (n, m) = (self.a.collection(*x).len(), self.b.collection(*y).len());
                    if n != m {
                        self.out.push(format!("{path}: set of {n} -> {m}"));
                    }
                    return;
                }
                let class = short(class).to_owned();
                for (ca, cb) in da.iter().zip(db) {
                    for ((name, va), (_, vb)) in ca.fields.iter().zip(&cb.fields) {
                        if !NOISE.contains(&&**name) {
                            self.value(&format!("{path}.{name}"), va, vb);
                        }
                    }
                    // Hash maps record their capacity, which depends on how they grew.
                    if !class.ends_with("HashMap")
                        && blocks(&ca.annotation) != blocks(&cb.annotation)
                    {
                        self.out.push(format!("{path}[{}]: block data differs", short(&ca.class)));
                    }
                    let path = format!("{path}[{class}]");
                    self.list(&path, &values(&ca.annotation), &values(&cb.annotation));
                }
            }
            _ => self.report(path, a, b),
        }
    }

    fn list(&mut self, path: &str, a: &[Value], b: &[Value]) {
        if a.len() != b.len() {
            self.out.push(format!("{path}: {} items -> {}", a.len(), b.len()));
        }
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            self.value(&format!("{path}/{i}"), x, y);
        }
        for (i, y) in b.iter().enumerate().skip(a.len()) {
            self.out.push(format!("{path}/{i}: added {}", Self::describe(self.b, y)));
        }
        for (i, x) in a.iter().enumerate().skip(b.len()) {
            self.out.push(format!("{path}/{i}: removed {}", Self::describe(self.a, x)));
        }
    }
}
