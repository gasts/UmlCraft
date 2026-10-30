//! Reverse engineering: reads declarations from source files (Java, C#, C++, Python) into the
//! model, as
//! Astah's "Import Java" does.
//!
//! Each file is read ([`source::read`]) and parsed with tree-sitter once ([`parse_tree`]); a
//! front-end per language lowers the tree into a language-neutral IR ([`ir`], [`lower`]); one
//! mapping for all languages resolves names and merges the declarations into the model. Parsing
//! needs nothing but the files and may run in the background ([`parse_sources`]); the mapping
//! runs in a single edit of the document ([`apply`]), so one undo takes the whole import back.
//! The structograms of `umlcraft-nsd-code` read method bodies from the same [`SourceTree`].

pub mod associations;
mod comment;
pub mod cpp;
pub mod csharp;
pub mod ir;
pub mod java;
pub mod lang;
mod library;
mod merge;
pub mod preprocessor;
pub mod python;
pub mod report;
mod resolve;
pub mod source;
mod tree;

use std::path::{Path, PathBuf};

use lang::{Frontend, Rules, Skipped, SyntaxError};
use umlcraft_model::{Document, Result, Tx};

pub use report::Report;
pub use tree::{SourceTree, parse_tree};

/// The languages the import reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceLanguage {
    Java,
    CSharp,
    Cpp,
    Python,
}

impl SourceLanguage {
    /// Every language the import reads.
    pub const ALL: [SourceLanguage; 4] =
        [SourceLanguage::Java, SourceLanguage::CSharp, SourceLanguage::Cpp, SourceLanguage::Python];

    /// File extensions, without the dot. C and C headers are read as C++.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            SourceLanguage::Java => &["java"],
            SourceLanguage::CSharp => &["cs"],
            SourceLanguage::Cpp => {
                &["h", "hh", "hpp", "hxx", "c", "cpp", "cc", "cxx", "c++", "inl", "ipp", "tpp"]
            }
            SourceLanguage::Python => &["py", "pyi", "pyw"],
        }
    }

    /// The language of a file by its extension (compared ignoring case), if it is one of
    /// [`SourceLanguage::extensions`].
    pub fn of(path: &Path) -> Option<SourceLanguage> {
        let extension = path.extension()?.to_str()?;
        SourceLanguage::ALL
            .into_iter()
            .find(|l| l.extensions().iter().any(|e| e.eq_ignore_ascii_case(extension)))
    }

    /// The language's name as people write it: `Java`, `C#`, `C++`, `Python`.
    pub fn name(self) -> &'static str {
        match self {
            SourceLanguage::Java => "Java",
            SourceLanguage::CSharp => "C#",
            SourceLanguage::Cpp => "C++",
            SourceLanguage::Python => "Python",
        }
    }

    fn frontend(self) -> &'static dyn Frontend {
        match self {
            SourceLanguage::Java => &java::Java,
            SourceLanguage::CSharp => &csharp::CSharp,
            SourceLanguage::Cpp => &cpp::Cpp,
            SourceLanguage::Python => &python::Python,
        }
    }

    fn rules(self) -> &'static Rules {
        match self {
            SourceLanguage::Java => &java::rules::RULES,
            SourceLanguage::CSharp => &csharp::rules::RULES,
            SourceLanguage::Cpp => &cpp::rules::RULES,
            SourceLanguage::Python => &python::rules::RULES,
        }
    }

    /// The module the file at `path` is, for languages whose files are modules; see
    /// [`source::python_module`]. A package's `__init__` file adds `__init__`, so the front-end
    /// knows the package relative imports start from. Empty for Java, C# and C++, whose files
    /// declare their namespaces.
    fn module_path(self, path: &Path) -> Vec<String> {
        match self {
            SourceLanguage::Java | SourceLanguage::CSharp | SourceLanguage::Cpp => Vec::new(),
            SourceLanguage::Python => {
                let mut module = source::python_module(path);
                if path.file_stem().is_some_and(|s| s == "__init__") {
                    module.push("__init__".to_owned());
                }
                module
            }
        }
    }

    /// Tagged values of this language's profile start with it.
    fn tag_prefix(self) -> &'static str {
        match self {
            SourceLanguage::Java => "java.",
            SourceLanguage::CSharp => "csharp.",
            SourceLanguage::Cpp => "cpp.",
            SourceLanguage::Python => "umlcraft.python.",
        }
    }
}

/// Why a file was not imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    Read(String),
    Syntax(SyntaxError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileProblem {
    pub path: PathBuf,
    pub problem: Problem,
}

/// The parsed files of one import, ready to be applied to a model.
#[derive(Clone, Debug, Default)]
pub struct Parsed {
    pub units: Vec<ir::Unit>,
    pub failed: Vec<FileProblem>,
    pub skipped: Skipped,
    /// Files whose encoding was guessed, with the encoding used.
    pub guessed_encodings: Vec<(PathBuf, &'static str)>,
}

/// The source files of `language` among `paths` (files, or folders searched recursively).
pub fn collect_sources_for(
    language: SourceLanguage,
    paths: &[PathBuf],
) -> std::io::Result<Vec<PathBuf>> {
    source::collect_sources(paths, language.extensions())
}

/// Merges parsed sources into the model: packages and types are found by name or created,
/// their members matched with the sources, and attributes of class types turned into
/// associations. Run it inside one [`Document::edit`] so the import is one undo step.
pub fn apply(tx: &mut Tx<'_>, language: SourceLanguage, parsed: &Parsed) -> Result<Report> {
    let mut importer = merge::Importer::new(language.rules(), language.tag_prefix());
    let root = tx.model().root();
    for unit in &parsed.units {
        importer.declare_scope(tx, &unit.scope, root, &[])?;
    }
    importer.merge(tx)?;
    importer.associations(tx)?;
    let mut report = importer.report;
    report.files = parsed.units.len();
    for (&skip, &n) in &parsed.skipped {
        *report.skipped.entry(skip).or_default() += n;
    }
    report.failed = parsed.failed.clone();
    report.guessed_encodings = parsed.guessed_encodings.clone();
    Ok(report)
}

/// Collects, parses and applies `paths` in one edit of `doc` labelled `label`: what the app
/// does in two steps, for tests and tools.
pub fn import_into(
    doc: &mut Document,
    language: SourceLanguage,
    paths: &[PathBuf],
    label: &str,
) -> std::result::Result<Report, ImportError> {
    let files = collect_sources_for(language, paths)?;
    let parsed = parse_sources(language, &files);
    Ok(doc.edit(label, |tx| apply(tx, language, &parsed))?)
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Model(#[from] umlcraft_model::ModelError),
}

/// Parses one file's text; `path` also tells the module a Python file is.
pub fn parse_text(
    language: SourceLanguage,
    path: &Path,
    text: &str,
    skipped: &mut Skipped,
) -> Result<ir::Unit, SyntaxError> {
    lower(&parse_tree(language, text), path, skipped)
}

/// The declarations of a parsed file, counting what is left out in `skipped`; `path` also tells
/// the module a Python file is. A file with a syntax error has none.
pub fn lower(
    source: &SourceTree,
    path: &Path,
    skipped: &mut Skipped,
) -> Result<ir::Unit, SyntaxError> {
    let module = source.language().module_path(path);
    let scope = lower_scope(source, &module, skipped)?;
    Ok(ir::Unit { path: path.to_owned(), scope })
}

/// [`lower`] for the file that is `module`.
pub(crate) fn lower_scope(
    source: &SourceTree,
    module: &[String],
    skipped: &mut Skipped,
) -> Result<ir::Scope, SyntaxError> {
    if let Some(error) = source.syntax_error() {
        return Err(error);
    }
    let branches = source.left_out_branches();
    if branches > 0 {
        *skipped.entry(lang::Skip::PreprocessorBranch).or_default() += branches;
    }
    Ok(source.language().frontend().lower(source.root(), source.text(), module, skipped))
}

/// Reads and parses `files`. Files that cannot be read or parsed are reported in
/// [`Parsed::failed`]; the others are imported anyway.
pub fn parse_sources(language: SourceLanguage, files: &[PathBuf]) -> Parsed {
    let mut parsed = Parsed::default();
    for path in files {
        let decoded = match source::read(path) {
            Ok(decoded) => decoded,
            Err(err) => {
                let problem = Problem::Read(err.to_string());
                parsed.failed.push(FileProblem { path: path.clone(), problem });
                continue;
            }
        };
        if let Some(encoding) = decoded.guessed {
            parsed.guessed_encodings.push((path.clone(), encoding));
        }
        // Counted only for files that are imported.
        let mut skipped = Skipped::new();
        match lower(&parse_tree(language, decoded.text), path, &mut skipped) {
            Ok(unit) => {
                parsed.units.push(unit);
                for (skip, n) in skipped {
                    *parsed.skipped.entry(skip).or_default() += n;
                }
            }
            Err(error) => {
                let problem = Problem::Syntax(error);
                parsed.failed.push(FileProblem { path: path.clone(), problem });
            }
        }
    }
    parsed
}
