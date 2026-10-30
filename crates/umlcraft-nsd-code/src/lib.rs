//! Methods of source files as structograms: the methods and constructors of a `.java` or `.cs`
//! file, and one of them as a Nassi-Shneiderman diagram in Structorizer's `.nsd` form. Each
//! language reads its own nodes (`java`, `csharp`); what they share is in `core` and `pseudo`.
//!
//! Texts are written as structograms write them: no types (the root is `name(a, b)`),
//! assignments as `x := value` (`i++` as `i := i + 1`), no `;`, counting loops as
//! `for i := 0 to n - 1` and `foreach x in c`, conditions as questions (`a > 0?`), returned
//! values as `gib x`; expressions and conditions stay as in the code otherwise (without their
//! outer parentheses). Calls of the class's own methods (`f(x);`, `this.f(x);`)
//! become call elements, other statements instructions. What has no place in a structogram
//! (`continue`, labels, local classes …) is named in the [`Report`]; nothing is left out
//! silently. Structorizer's own Java import was only compared by its results, never by its
//! code; Structorizer has no C# import.

mod core;
mod csharp;
mod java;
mod pseudo;
mod text;

use std::path::Path;

use tree_sitter::{Parser, Tree};
use umlcraft_nsd::Diagram;
use umlcraft_reverse::preprocessor::{Dialect, first_branches};

/// The languages whose methods become structograms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Java,
    CSharp,
}

impl Language {
    /// The language of a file by its extension (`.java`, `.cs`).
    pub fn of(path: &Path) -> Option<Language> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "java" => Some(Language::Java),
            "cs" => Some(Language::CSharp),
            _ => None,
        }
    }

    /// The language's name as people write it.
    pub fn name(self) -> &'static str {
        match self {
            Language::Java => "Java",
            Language::CSharp => "C#",
        }
    }
}

/// A method or constructor with a body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodRef {
    /// The classes it is in, outermost first.
    pub class: Vec<String>,
    pub name: String,
    /// As written, on one line, without modifiers, annotations and `throws`:
    /// `double getSpeed(SpeedUnit unit)`, `<T> T first(List<T> list)`, `Point(int x, int y)`;
    /// C# accessors as .NET names them: `int get_Count()`.
    pub signature: String,
    /// 1-based line of the declaration.
    pub line: usize,
    /// Where the declaration starts in the file (identifies it).
    pub start: usize,
}

impl MethodRef {
    /// `Outer.Inner`.
    pub fn class_name(&self) -> String {
        self.class.join(".")
    }

    /// A file name for its diagram: `Class.method.nsd`.
    pub fn file_name(&self) -> String {
        format!("{}.{}.nsd", self.class_name(), self.name)
    }
}

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
    /// C# `using`: the resource is assigned, the body stands in its place.
    Using,
    /// C# `lock (x) { … }`: the body stands in its place, the lock as a comment.
    Lock,
    /// C# `yield return`/`yield break`: kept as written; structograms have no iterators.
    Yield,
    /// C# `goto`: kept as a jump with its text.
    Goto,
    /// A function declared in the method is left out.
    LocalFunction,
    /// Of `#if` … `#else`, only the first branch is read.
    Conditional,
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

/// A parsed source file.
pub struct Source {
    language: Language,
    /// As parsed: in C#, the branches of `#if` after the first are blanked out (every byte
    /// keeps its place).
    text: String,
    /// As written, where it differs from `text`.
    written: Option<String>,
    tree: Tree,
}

impl Source {
    pub fn parse(language: Language, text: &str) -> Source {
        let (grammar, parsed) = match language {
            Language::Java => (tree_sitter_java::LANGUAGE, text.to_owned()),
            Language::CSharp => {
                (tree_sitter_c_sharp::LANGUAGE, first_branches(text, Dialect::CSharp).0)
            }
        };
        let mut parser = Parser::new();
        parser.set_language(&grammar.into()).expect("the grammar matches the tree-sitter version");
        let tree = parser.parse(&parsed, None).expect("parsing without a timeout always finishes");
        let written = (parsed != text).then(|| text.to_owned());
        Source { language, text: parsed, written, tree }
    }

    pub fn language(&self) -> Language {
        self.language
    }

    /// The methods and constructors with a body, in file order, nested classes included
    /// (but not classes inside methods); in C# also accessors, operators, finalizers and the
    /// top-level statements (as `Program.Main`).
    pub fn methods(&self) -> Vec<MethodRef> {
        let root = self.tree.root_node();
        match self.language {
            Language::Java => java::methods(root, &self.text),
            Language::CSharp => csharp::methods(root, &self.text),
        }
    }

    /// The diagram of `method` (one of [`Source::methods`]).
    pub fn diagram(&self, method: &MethodRef, made: &Made) -> Option<(Diagram, Report)> {
        let root = self.tree.root_node();
        match self.language {
            Language::Java => {
                let node = java::find(root, method)?;
                Some(java::diagram(node, &self.text, made))
            }
            Language::CSharp => {
                let member = csharp::find(root, &self.text, method)?;
                let (d, mut report) = csharp::diagram(&member, &self.text, made);
                if let Some(line) = self.left_out(member.range()) {
                    report.notes.insert(0, Note { line, kind: NoteKind::Conditional });
                }
                Some((d, report))
            }
        }
    }

    /// The line of the first `#elif` or `#else` in `range`, whose branch was left out.
    fn left_out(&self, range: std::ops::Range<usize>) -> Option<usize> {
        let written = self.written.as_ref()?;
        let before = written[..range.start].matches('\n').count();
        let at = written[range].lines().position(|line| {
            let directive = line.trim_start().strip_prefix('#').map(str::trim_start);
            directive.is_some_and(|d| d.starts_with("elif") || d.starts_with("else"))
        })?;
        Some(before + at + 1)
    }
}

/// Reads a source file's bytes as text (UTF-8, UTF-16 with BOM, else Windows-1252).
pub fn decode(bytes: &[u8]) -> String {
    umlcraft_reverse::source::decode(bytes).text
}
