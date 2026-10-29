//! Nassi-Shneiderman diagrams (structograms) in the `.nsd` format of
//! [Structorizer](https://structorizer.fisch.lu).
//!
//! Files written by Structorizer 3.32-35 are read and written again byte for byte. The format
//! and Structorizer's way of writing it were worked out from its XML schema, its user guide and
//! the files it writes; Structorizer is GPL-licensed and none of its code is used here.

pub mod model;
pub mod text;

pub use model::{
    Attr, Attrs, Branches, Color, Diagram, Element, ElementKind, Lines, RootKind, RootStyle,
    Sequence,
};
