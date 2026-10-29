//! A diagram being edited: undo and redo over snapshots of the whole diagram (structograms are
//! small) and whether it differs from what was saved last.

use crate::edit::EditError;
use crate::model::Diagram;
use crate::path::Selection;

/// How many steps undo keeps.
const LIMIT: usize = 200;

pub struct Editor {
    diagram: Diagram,
    selection: Selection,
    undo: Vec<(Diagram, Selection)>,
    redo: Vec<(Diagram, Selection)>,
    /// The diagram as saved or loaded last; `None` for a new one never saved.
    saved: Option<Diagram>,
}

impl Editor {
    /// `saved`: whether the diagram is what a file holds (loaded), or new.
    pub fn new(diagram: Diagram, saved: bool) -> Editor {
        Editor {
            saved: saved.then(|| diagram.clone()),
            diagram,
            selection: Selection::Nothing,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn diagram(&self) -> &Diagram {
        &self.diagram
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// Selects without an undo step.
    pub fn select(&mut self, selection: Selection) {
        if selection.is_valid(&self.diagram) {
            self.selection = selection;
        }
    }

    /// Makes one undoable change. A change that fails, or changes nothing, leaves no step.
    pub fn apply(
        &mut self,
        change: impl FnOnce(&mut Diagram) -> Result<Selection, EditError>,
    ) -> Result<(), EditError> {
        let mut changed = self.diagram.clone();
        let selection = change(&mut changed)?;
        if changed != self.diagram {
            let before = std::mem::replace(&mut self.diagram, changed);
            self.undo.push((before, self.selection.clone()));
            if self.undo.len() > LIMIT {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        self.select(selection);
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some((diagram, selection)) = self.undo.pop() else { return false };
        let current = std::mem::replace(&mut self.diagram, diagram);
        self.redo.push((current, std::mem::replace(&mut self.selection, selection)));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some((diagram, selection)) = self.redo.pop() else { return false };
        let current = std::mem::replace(&mut self.diagram, diagram);
        self.undo.push((current, std::mem::replace(&mut self.selection, selection)));
        true
    }

    /// Whether the diagram differs from the file (always for a new one).
    pub fn is_dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.diagram)
    }

    /// The file content to save: the diagram as loaded when it is unchanged (so the file stays
    /// byte for byte), otherwise stamped as changed by `user` on `date` (see
    /// [`crate::save::stamp`]). Call [`Editor::saved`] with it once it is written.
    pub fn to_save(&self, user: &str, date: &str) -> Diagram {
        let mut d = self.diagram.clone();
        if self.saved.as_ref() != Some(&d) {
            crate::save::stamp(&mut d, user, date);
        }
        d
    }

    /// Takes `written` (from [`Editor::to_save`]) as the diagram and as saved. The stamp is no
    /// undo step of its own: undo goes back to before the last change.
    pub fn saved(&mut self, written: Diagram) {
        self.diagram = written.clone();
        self.saved = Some(written);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::{Slot, Span};
    use crate::{Element, Provenance, RootKind, edit, lines};

    fn editor() -> Editor {
        let made = Provenance { author: "t", date: "2026-10-01", origin: "test" };
        Editor::new(Diagram::new(RootKind::Program, &lines(&["demo"]), &made), true)
    }

    fn add(e: &mut Editor, text: &str) {
        let slot = Slot { seq: vec![], index: e.diagram().children.elements.len() };
        e.apply(|d| edit::insert(d, &slot, vec![Element::instruction(&lines(&[text]))])).unwrap();
    }

    #[test]
    fn undo_and_redo_step_by_step() {
        let mut e = editor();
        add(&mut e, "a");
        add(&mut e, "b");
        assert_eq!(e.selection(), &Selection::element(&[1]));
        assert!(e.undo());
        assert_eq!(e.diagram().children.elements.len(), 1);
        assert_eq!(e.selection(), &Selection::element(&[0]));
        assert!(e.redo());
        assert_eq!(e.diagram().children.elements.len(), 2);
        assert!(!e.redo());
        e.undo();
        add(&mut e, "c");
        assert!(!e.can_redo());
    }

    #[test]
    fn failures_and_no_ops_leave_no_step() {
        let mut e = editor();
        add(&mut e, "a");
        let span = Span { seq: vec![], start: 0, end: 1 };
        assert!(
            e.apply(|d| edit::remove(d, &Span { start: 3, end: 4, ..span.clone() }).map(|r| r.1))
                .is_err()
        );
        e.apply(|d| edit::shift(d, &span, true)).unwrap();
        e.undo();
        assert!(!e.can_undo());
    }

    #[test]
    fn dirty_until_saved_and_clean_again_by_undo() {
        let mut e = editor();
        assert!(!e.is_dirty());
        add(&mut e, "a");
        assert!(e.is_dirty());
        e.undo();
        assert!(!e.is_dirty());
        // Saving the unchanged diagram writes it as loaded.
        assert_eq!(&e.to_save("me", "2026-10-29"), e.diagram());
        e.redo();
        let written = e.to_save("me", "2026-10-29");
        assert_eq!(written.header("changedby"), Some("me"));
        e.saved(written);
        assert!(!e.is_dirty());
        e.undo();
        assert!(e.is_dirty());
        assert!(Editor::new(e.diagram().clone(), false).is_dirty());
    }
}
