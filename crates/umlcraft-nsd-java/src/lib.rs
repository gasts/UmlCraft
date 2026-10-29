//! Java methods as structograms: the methods and constructors of a `.java` file, and one of
//! them as a Nassi-Shneiderman diagram in Structorizer's `.nsd` form.
//!
//! Texts stay as in the code (conditions without their outer parentheses); calls of the
//! class's own methods (`f(x);`, `this.f(x);`) become call elements, other statements
//! instructions. What has no place in a structogram (`continue`, labels, local classes …) is
//! named in the [`Report`]; nothing is left out silently. Structorizer's own Java import was
//! only compared by its results, never by its code.

mod build;
mod methods;
mod text;

use tree_sitter::{Parser, Tree};
use umlcraft_nsd::Diagram;

pub use methods::MethodRef;

/// Who makes the diagram, when, and from which file (for the header).
pub struct Made<'a> {
    pub author: &'a str,
    pub date: &'a str,
    /// The `origin` attribute, e.g. `UmlCraft 0.9.0 / Java: Shop.java`.
    pub origin: &'a str,
}

/// Something of the method that the diagram does not show as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// 1-based line in the file.
    pub line: usize,
    pub kind: NoteKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteKind {
    /// `continue` has no element: kept as a red, disabled instruction.
    Continue,
    /// A label (`outer:`) is left out; `break outer` keeps its text.
    Label,
    /// A class, record, enum or interface declared in the method is left out.
    LocalClass,
    /// `synchronized (x) { … }`: the body stands in its place, the lock as a comment.
    Synchronized,
    /// A `switch` case runs on into the next: its statements are copied there.
    Fallthrough,
    /// A part the parser could not read: kept as a red, disabled instruction.
    SyntaxError,
}

/// What the import of one method did not show as written.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub notes: Vec<Note>,
}

impl Report {
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }
}

/// A parsed `.java` file.
pub struct Source {
    text: String,
    tree: Tree,
}

impl Source {
    pub fn parse(text: &str) -> Source {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .expect("the Java grammar matches the tree-sitter version");
        let tree = parser.parse(text, None).expect("parsing without a timeout always finishes");
        Source { text: text.to_owned(), tree }
    }

    /// The methods and constructors with a body, in file order, nested classes included
    /// (but not classes inside methods).
    pub fn methods(&self) -> Vec<MethodRef> {
        methods::methods(self.tree.root_node(), &self.text)
    }

    /// The diagram of `method` (one of [`Source::methods`]).
    pub fn diagram(&self, method: &MethodRef, made: &Made) -> Option<(Diagram, Report)> {
        let node = methods::find(self.tree.root_node(), method)?;
        Some(build::diagram(node, &self.text, made))
    }
}

/// Reads a source file's bytes as text (UTF-8, UTF-16 with BOM, else Windows-1252).
pub fn decode(bytes: &[u8]) -> String {
    umlcraft_reverse::source::decode(bytes).text
}
