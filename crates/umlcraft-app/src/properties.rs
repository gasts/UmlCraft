//! Properties panel: edits name, visibility, stereotypes and documentation of the selected
//! element. Text fields commit on Enter or when they lose focus, so typing is one undo step.

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme, IndexPath, Sizable, v_flex};
use gpui_kit::*;
use rust_i18n::t;
use umlcraft_model::{ElementData, ElementKind, Id};

use crate::labels::{self, VISIBILITIES};
use crate::workspace::Workspace;

type VisibilitySelect = SelectState<Vec<SharedString>>;

#[derive(Clone, Copy)]
enum Field {
    Name,
    Stereotypes,
    Text,
}

pub struct PropertiesPanel {
    workspace: Entity<Workspace>,
    name: Entity<InputState>,
    stereotypes: Entity<InputState>,
    text: Entity<TextareaState>,
    visibility: Entity<VisibilitySelect>,
    /// Element, document revision and locale the fields currently show.
    shown: Option<(Id, u64, String)>,
    /// Element whose text fields are being edited. Edits always go back to this element, even
    /// if the selection has changed by the time the field loses focus.
    editing: Option<Id>,
    _subscriptions: Vec<Subscription>,
}

impl PropertiesPanel {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        let stereotypes = cx.new(|cx| InputState::new(window, cx).placeholder("entity, control"));
        let text = cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 12));
        let visibility = cx.new(|cx| SelectState::new(visibility_items(), None, window, cx));

        let subscriptions = vec![
            cx.observe_in(&workspace, window, |this, _, window, cx| this.sync(window, cx)),
            cx.subscribe_in(&name, window, |this, _, ev: &InputEvent, _, cx| {
                this.on_input(Field::Name, ev, cx);
            }),
            cx.subscribe_in(&stereotypes, window, |this, _, ev: &InputEvent, _, cx| {
                this.on_input(Field::Stereotypes, ev, cx);
            }),
            cx.subscribe_in(&text, window, |this, _, ev: &InputEvent, _, cx| {
                // Enter makes a new line in the text area; only leaving it commits.
                if !matches!(ev, InputEvent::PressEnter { .. }) {
                    this.on_input(Field::Text, ev, cx);
                }
            }),
            cx.subscribe_in(
                &visibility,
                window,
                |this, state, _: &SelectEvent<Vec<SharedString>>, _, cx| {
                    let Some(ix) = state.read(cx).selected_index(cx).map(|ix| ix.row) else {
                        return;
                    };
                    let (Some(&v), Some(id)) = (VISIBILITIES.get(ix), this.shown_id()) else {
                        return;
                    };
                    this.apply(id, cx, move |data| data.visibility = v);
                },
            ),
        ];

        let mut panel = Self {
            workspace,
            name,
            stereotypes,
            text,
            visibility,
            shown: None,
            editing: None,
            _subscriptions: subscriptions,
        };
        panel.sync(window, cx);
        panel
    }

    /// Puts the cursor into the name field, e.g. for "Rename" in the model tree.
    pub fn focus_name(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.name.update(cx, |state, cx| state.focus(window, cx));
    }

    fn shown_id(&self) -> Option<Id> {
        self.shown.as_ref().map(|(id, _, _)| *id)
    }

    fn on_input(&mut self, field: Field, ev: &InputEvent, cx: &mut Context<Self>) {
        match ev {
            InputEvent::Focus => self.editing = self.editing.or(self.shown_id()),
            InputEvent::PressEnter { .. } => self.commit(field, cx),
            InputEvent::Blur => {
                self.commit(field, cx);
                self.editing = None;
            }
            InputEvent::Change => {}
        }
    }

    /// Writes one field back to the element being edited.
    fn commit(&mut self, field: Field, cx: &mut Context<Self>) {
        let Some(id) = self.editing else { return };
        match field {
            Field::Name => {
                let value = self.name.read(cx).value().trim().to_owned();
                self.apply(id, cx, move |data| data.name = value);
            }
            Field::Stereotypes => {
                let list: Vec<String> = self
                    .stereotypes
                    .read(cx)
                    .value()
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                self.apply(id, cx, move |data| data.stereotypes = list);
            }
            Field::Text => {
                let value = self.text.read(cx).value().to_string();
                self.apply(id, cx, move |data| match &mut data.kind {
                    ElementKind::Comment(p) => p.body = value,
                    _ => data.documentation = value,
                });
            }
        }
    }

    /// Changes element `id` as one undo step, if the change changes anything.
    fn apply(&mut self, id: Id, cx: &mut Context<Self>, f: impl FnOnce(&mut ElementData)) {
        self.workspace.update(cx, |ws, cx| {
            let Some(element) = ws.doc.model().get(id) else { return };
            let mut data = element.data().clone();
            f(&mut data);
            if data != *element.data() {
                ws.update_element(id, |d| *d = data);
                cx.notify();
            }
        });
    }

    /// Loads the selected element into the fields when it (or the document) changed.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (selected, revision) = {
            let ws = self.workspace.read(cx);
            (ws.selected_element.filter(|&id| ws.doc.model().contains(id)), ws.doc.revision())
        };
        // The selection moved away while a field was being edited: save that edit first.
        if let Some(editing) = self.editing
            && selected != Some(editing)
        {
            for field in [Field::Name, Field::Stereotypes, Field::Text] {
                self.commit(field, cx);
            }
            self.editing = None;
        }
        let Some(id) = selected else {
            self.shown = None;
            cx.notify();
            return;
        };
        let ws = self.workspace.read(cx);
        let locale = rust_i18n::locale().to_string();
        let key = (id, revision, locale.clone());
        if self.shown.as_ref() == Some(&key) {
            return;
        }
        let locale_changed = self.shown.as_ref().is_none_or(|(_, _, l)| *l != locale);
        let data = ws.doc.model().get(id).expect("checked above").data().clone();
        self.shown = Some(key);

        self.name.update(cx, |s, cx| s.set_value(data.name.clone(), window, cx));
        self.stereotypes.update(cx, |s, cx| s.set_value(data.stereotypes.join(", "), window, cx));
        let text = match &data.kind {
            ElementKind::Comment(p) => p.body.clone(),
            _ => data.documentation.clone(),
        };
        self.text.update(cx, |s, cx| s.set_value(text, window, cx));
        let ix = VISIBILITIES.iter().position(|&v| v == data.visibility).map(IndexPath::new);
        self.visibility.update(cx, |s, cx| {
            if locale_changed {
                s.set_items(visibility_items(), window, cx);
            }
            s.set_selected_index(ix, window, cx);
        });
        cx.notify();
    }
}

fn visibility_items() -> Vec<SharedString> {
    VISIBILITIES.iter().map(|&v| labels::visibility_label(v).into()).collect()
}

impl Render for PropertiesPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let field = |label: SharedString, content: AnyElement| {
            v_flex().gap_1().child(div().text_xs().text_color(muted).child(label)).child(content)
        };
        let ws = self.workspace.read(cx);
        let model = ws.doc.model();
        let body = v_flex().gap_3().p_3();
        let Some(e) = ws.selected_element.and_then(|id| model.get(id)) else {
            return body
                .child(div().text_sm().text_color(muted).child(t!("prop.nothing_selected")));
        };

        let kind = e.kind().clone();
        let is_comment = matches!(kind, ElementKind::Comment(_));
        let mut body = body.child(field(
            t!("prop.type").into(),
            div().text_sm().child(labels::kind_label(e)).into_any_element(),
        ));
        if !is_comment {
            body = body
                .child(field(
                    t!("prop.name").into(),
                    Input::new(&self.name).small().into_any_element(),
                ))
                .child(field(
                    t!("prop.visibility").into(),
                    Select::new(&self.visibility).small().into_any_element(),
                ))
                .child(field(
                    t!("prop.stereotypes").into(),
                    Input::new(&self.stereotypes).small().into_any_element(),
                ));
        }
        if let ElementKind::Class(p) = kind {
            let workspace = self.workspace.clone();
            let id = e.id();
            body = body.child(
                Checkbox::new("abstract")
                    .label(t!("prop.abstract").to_string())
                    .checked(p.is_abstract)
                    .on_click(move |checked: &bool, _, cx| {
                        let checked = *checked;
                        workspace.update(cx, |ws, cx| {
                            ws.update_element(id, |d| {
                                if let ElementKind::Class(p) = &mut d.kind {
                                    p.is_abstract = checked;
                                }
                            });
                            cx.notify();
                        });
                    }),
            );
        }
        let text_label = if is_comment { t!("prop.text") } else { t!("prop.documentation") };
        body = body
            .child(field(text_label.into(), Textarea::new(&self.text).small().into_any_element()))
            .child(field(
                t!("prop.owner").into(),
                div().text_sm().child(labels::owner_path(model, e.id())).into_any_element(),
            ));
        let bounds = match (ws.active_diagram, ws.selection.as_slice()) {
            (Some(d), [p]) => model.view(d).and_then(|v| v.get(*p)).and_then(|p| p.bounds()),
            _ => None,
        };
        if let Some(b) = bounds {
            body = body.child(field(
                t!("prop.bounds").into(),
                div()
                    .text_sm()
                    .child(format!("x {}, y {}, {} × {}", b.x, b.y, b.width, b.height))
                    .into_any_element(),
            ));
        }
        body
    }
}
