use umlcraft_model::{Document, Id};

/// The open project and what the user is looking at. Shared by all panels as a GPUI entity.
pub struct Workspace {
    pub doc: Document,
    pub active_diagram: Option<Id>,
    pub selected_element: Option<Id>,
    pub selected_presentation: Option<Id>,
}

impl Workspace {
    pub fn new(doc: Document, active_diagram: Option<Id>) -> Self {
        Self { doc, active_diagram, selected_element: None, selected_presentation: None }
    }

    pub fn clear_selection(&mut self) {
        self.selected_element = None;
        self.selected_presentation = None;
    }

    /// Selects a presentation on the active diagram, and the element it shows.
    pub fn select_presentation(&mut self, presentation: Id) {
        let element = self
            .active_diagram
            .and_then(|d| self.doc.model().view(d))
            .and_then(|v| v.get(presentation))
            .and_then(|p| p.element);
        self.selected_presentation = Some(presentation);
        self.selected_element = element;
    }

    /// Selects an element and, if the active diagram shows it, its first presentation.
    pub fn select_element(&mut self, element: Id) {
        self.selected_element = Some(element);
        self.selected_presentation = self
            .active_diagram
            .and_then(|d| self.doc.model().view(d))
            .and_then(|v| v.presentations_of(element).next())
            .map(|p| p.id);
    }

    /// Drops selections that no longer exist, e.g. after undo.
    pub fn validate_selection(&mut self) {
        let model = self.doc.model();
        if self.active_diagram.is_some_and(|d| model.view(d).is_none()) {
            self.active_diagram = model.views().map(|(d, _)| d).next();
        }
        if self.selected_element.is_some_and(|e| !model.contains(e)) {
            self.selected_element = None;
        }
        let view = self.active_diagram.and_then(|d| model.view(d));
        if self.selected_presentation.is_some_and(|p| view.is_none_or(|v| v.get(p).is_none())) {
            self.selected_presentation = None;
        }
    }
}
