//! Main window: toolbar, model browser, diagram canvas, properties and status bar.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::tree::{TreeEvent, TreeItem, TreeState, tree};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, StyledExt, Theme, ThemeMode, h_flex, h_resizable,
    resizable_panel, v_flex,
};
use gpui_kit::*;
use rust_i18n::t;
use umlcraft_model::sample::order_system;
use umlcraft_model::{Element, Id, Model};

use crate::canvas::DiagramCanvas;
use crate::workspace::Workspace;
use crate::{Redo, ToggleLanguage, ToggleTheme, Undo};

pub struct MainView {
    workspace: Entity<Workspace>,
    canvas: Entity<DiagramCanvas>,
    tree: Entity<TreeState>,
    /// Metaclass label per tree item id, for the row renderer.
    tree_types: Rc<HashMap<SharedString, String>>,
    collapsed: HashSet<SharedString>,
    tree_revision: Option<(u64, String)>,
    tree_selection: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl MainView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (doc, ids) = order_system();
        let workspace = cx.new(|_| Workspace::new(doc, Some(ids.class_diagram)));
        let canvas = cx.new(|cx| DiagramCanvas::new(workspace.clone(), cx));
        let tree = cx.new(|cx| TreeState::new(cx));

        let subscriptions = vec![
            cx.subscribe(&tree, |this, _, event: &TreeEvent, _| match event {
                TreeEvent::Expanded(id) => {
                    this.collapsed.remove(id);
                }
                TreeEvent::Collapsed(id) => {
                    this.collapsed.insert(id.clone());
                }
            }),
            cx.observe(&tree, |this, tree, cx| this.on_tree_changed(tree, cx)),
            cx.observe_in(&workspace, window, |this, _, window, cx| {
                this.sync_from_workspace(window, cx);
            }),
        ];

        let mut view = Self {
            workspace,
            canvas,
            tree,
            tree_types: Rc::default(),
            collapsed: HashSet::new(),
            tree_revision: None,
            tree_selection: None,
            _subscriptions: subscriptions,
        };
        view.sync_from_workspace(window, cx);
        view
    }

    /// Keeps tree, selection and window title in line with the workspace.
    fn sync_from_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.workspace.update(cx, |ws, _| ws.validate_selection());
        let ws = self.workspace.read(cx);
        let revision = (ws.doc.revision(), rust_i18n::locale().to_string());
        if self.tree_revision.as_ref() != Some(&revision) {
            let mut types = HashMap::new();
            let root =
                tree_item(ws.doc.model(), ws.doc.model().root(), &self.collapsed, &mut types);
            self.tree_types = Rc::new(types);
            self.tree_selection = None;
            self.tree.update(cx, |tree, cx| tree.set_items(vec![root], cx));
            self.tree_revision = Some(revision);
        }

        let ws = self.workspace.read(cx);
        let wanted =
            ws.selected_element.or(ws.active_diagram).map(|id| SharedString::from(id.to_string()));
        if wanted != self.tree_selection {
            self.tree_selection = wanted.clone();
            self.tree.update(cx, |tree, cx| {
                let item = wanted.map(|id| TreeItem::new(id, ""));
                tree.set_selected_item(item.as_ref(), cx);
            });
        }

        let ws = self.workspace.read(cx);
        let model = ws.doc.model();
        let dirty = if ws.doc.is_dirty() { "*" } else { "" };
        window.set_window_title(&format!(
            "{}{dirty} – UmlCraft",
            model.element(model.root()).map_or("", Element::name)
        ));
        cx.notify();
    }

    fn on_tree_changed(&mut self, tree: Entity<TreeState>, cx: &mut Context<Self>) {
        let selected = tree.read(cx).selected_item().map(|item| item.id.clone());
        if selected == self.tree_selection {
            return;
        }
        self.tree_selection = selected.clone();
        let Some(id) = selected.and_then(|s| s.parse::<Id>().ok()) else {
            return;
        };
        self.workspace.update(cx, |ws, cx| {
            let is_diagram = ws.doc.model().view(id).is_some();
            if is_diagram {
                ws.active_diagram = Some(id);
                ws.clear_selection();
            } else {
                ws.select_element(id);
            }
            cx.notify();
        });
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.workspace.update(cx, |ws, cx| {
            if ws.doc.undo() {
                cx.notify();
            }
        });
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.workspace.update(cx, |ws, cx| {
            if ws.doc.redo() {
                cx.notify();
            }
        });
    }

    fn toggle_theme(&mut self, _: &ToggleTheme, window: &mut Window, cx: &mut Context<Self>) {
        let mode = if cx.theme().is_dark() { ThemeMode::Light } else { ThemeMode::Dark };
        Theme::change(mode, Some(window), cx);
    }

    fn toggle_language(&mut self, _: &ToggleLanguage, window: &mut Window, cx: &mut Context<Self>) {
        let next = if &*rust_i18n::locale() == "de" { "en" } else { "de" };
        rust_i18n::set_locale(next);
        self.sync_from_workspace(window, cx);
        window.refresh();
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let (can_undo, can_redo) = (ws.doc.can_undo(), ws.doc.can_redo());
        let canvas = self.canvas.clone();
        let canvas2 = self.canvas.clone();
        let canvas3 = self.canvas.clone();
        let separator = || div().w(px(1.0)).h(px(18.0)).mx_1().bg(cx.theme().border);
        h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("undo")
                    .ghost()
                    .small()
                    .label(t!("toolbar.undo"))
                    .disabled(!can_undo)
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Undo), cx)),
            )
            .child(
                Button::new("redo")
                    .ghost()
                    .small()
                    .label(t!("toolbar.redo"))
                    .disabled(!can_redo)
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Redo), cx)),
            )
            .child(separator())
            .child(Button::new("zoom-out").ghost().small().label("−").on_click(move |_, _, cx| {
                canvas.update(cx, |c, cx| c.zoom_by(1.0 / 1.2, cx));
            }))
            .child(Button::new("zoom-reset").ghost().small().label("100 %").on_click(
                move |_, _, cx| {
                    canvas2.update(cx, |c, cx| c.reset_zoom(cx));
                },
            ))
            .child(Button::new("zoom-in").ghost().small().label("+").on_click(move |_, _, cx| {
                canvas3.update(cx, |c, cx| c.zoom_by(1.2, cx));
            }))
            .child(div().flex_1())
            .child(
                Button::new("theme")
                    .ghost()
                    .small()
                    .label(t!("toolbar.theme"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleTheme), cx)),
            )
            .child(
                Button::new("language")
                    .ghost()
                    .small()
                    .label(t!("toolbar.language"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleLanguage), cx)),
            )
    }

    fn model_browser(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let types = self.tree_types.clone();
        let muted = cx.theme().muted_foreground;
        v_flex().size_full().child(panel_header(t!("panel.model"), cx)).child(
            div().flex_1().min_h_0().child(tree(&self.tree, move |ix, entry, selected, _, _| {
                let item = entry.item();
                let kind = types.get(&item.id).cloned().unwrap_or_default();
                ListItem::new(ix)
                    .selected(selected)
                    .pl(px(8.0 + 14.0 * entry.depth() as f32))
                    .child(
                        h_flex()
                            .gap_2()
                            .overflow_hidden()
                            .child(div().text_sm().child(item.label.clone()))
                            .child(div().text_xs().text_color(muted).child(kind)),
                    )
            })),
        )
    }

    fn properties(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let model = ws.doc.model();
        let muted = cx.theme().muted_foreground;
        let row = |label: SharedString, value: String| {
            v_flex()
                .gap_0p5()
                .child(div().text_xs().text_color(muted).child(label))
                .child(div().text_sm().child(value))
        };
        let mut body = v_flex().gap_3().p_3();
        match ws.selected_element.and_then(|id| model.get(id)) {
            Some(e) => {
                body = body
                    .child(row(t!("prop.name").into(), display_name(e)))
                    .child(row(t!("prop.type").into(), kind_label(e)))
                    .child(row(
                        t!("prop.visibility").into(),
                        format!("{:?}", e.data().visibility).to_lowercase(),
                    ))
                    .child(row(t!("prop.owner").into(), owner_path(model, e.id())));
                if !e.data().documentation.is_empty() {
                    body = body.child(row(
                        t!("prop.documentation").into(),
                        e.data().documentation.clone(),
                    ));
                }
                let bounds = ws
                    .selected_presentation
                    .zip(ws.active_diagram)
                    .and_then(|(p, d)| model.view(d)?.get(p)?.bounds());
                if let Some(b) = bounds {
                    body = body.child(row(
                        t!("prop.bounds").into(),
                        format!("x {}, y {}, {} × {}", b.x, b.y, b.width, b.height),
                    ));
                }
            }
            None => {
                body = body
                    .child(div().text_sm().text_color(muted).child(t!("prop.nothing_selected")));
            }
        }
        v_flex().size_full().child(panel_header(t!("panel.properties"), cx)).child(body)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let model = ws.doc.model();
        let diagram = ws.active_diagram.and_then(|d| model.get(d)).map(Element::name).unwrap_or("");
        let zoom = (self.canvas.read(cx).zoom() * 100.0).round();
        let state = if ws.doc.is_dirty() { t!("status.modified") } else { t!("status.saved") };
        h_flex()
            .gap_4()
            .px_3()
            .py_1()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .border_t_1()
            .border_color(cx.theme().border)
            .child(format!("{}: {diagram}", t!("status.diagram")))
            .child(format!("{zoom} %"))
            .child(state.to_string())
            .child(div().flex_1())
            .child(t!("status.hint").to_string())
    }
}

fn panel_header(title: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .px_3()
        .py_1p5()
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .border_b_1()
        .border_color(cx.theme().border)
        .child(title.into())
}

fn kind_label(e: &Element) -> String {
    let key = format!("kind.{}", e.kind().type_name());
    t!(key.as_str()).to_string()
}

fn display_name(e: &Element) -> String {
    if e.name().is_empty() { format!("({})", kind_label(e)) } else { e.name().to_owned() }
}

fn owner_path(model: &Model, id: Id) -> String {
    let mut names: Vec<&str> =
        model.ancestors(id).iter().rev().filter_map(|&a| model.get(a)).map(Element::name).collect();
    if names.is_empty() {
        names.push("—");
    }
    names.join(" › ")
}

fn tree_item(
    model: &Model,
    id: Id,
    collapsed: &HashSet<SharedString>,
    types: &mut HashMap<SharedString, String>,
) -> TreeItem {
    let e = model.get(id).expect("tree only visits existing elements");
    let key = SharedString::from(id.to_string());
    types.insert(key.clone(), kind_label(e));
    let children: Vec<TreeItem> =
        e.children().iter().map(|&c| tree_item(model, c, collapsed, types)).collect();
    TreeItem::new(key.clone(), display_name(e))
        .expanded(!collapsed.contains(&key))
        .children(children)
}

impl Render for MainView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .id("main")
            .key_context("UmlCraft")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::toggle_theme))
            .on_action(cx.listener(Self::toggle_language))
            .child(self.toolbar(cx))
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("main-split")
                        .child(
                            resizable_panel()
                                .size(px(260.0))
                                .size_range(px(160.0)..px(520.0))
                                .child(self.model_browser(cx)),
                        )
                        .child(resizable_panel().child(self.canvas.clone()))
                        .child(
                            resizable_panel()
                                .size(px(260.0))
                                .size_range(px(160.0)..px(520.0))
                                .child(self.properties(cx)),
                        ),
                ),
            )
            .child(self.status_bar(cx))
    }
}
