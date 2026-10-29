//! The view and editor of one structogram: its drawing, zoom and panning, selection, editing,
//! saving and export.
//!
//! The diagram is drawn by `umlcraft_nsd`'s SVG export and rasterized with resvg, so that the
//! window shows exactly what an export holds, turned texts and hatching included. The image is
//! made again when the diagram, the zoom or the screen's scale changes. Selection and the place
//! a drag would drop to are drawn over it, so they never reach an export. The diagram stays a
//! white sheet in the dark theme too, as in Structorizer; the area around it follows the theme.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, Theme, h_flex, v_flex};
use gpui_kit::*;
use resvg::{tiny_skia, usvg};
use rust_i18n::t;
use umlcraft_nsd::layout::Layout;
use umlcraft_nsd::path::{self, Path as NsdPath, Selection, Slot, Span};
use umlcraft_nsd::{Color, Diagram, EditError, Editor, ElementKind, clip, edit};

use super::dialog::{self, Edited, Target};
use super::{
    AddBranch, EditElement, ExtendDown, ExtendUp, InsertElement, MoveDown, MoveUp, PALETTE,
    RemoveBranch, SelectDown, SelectLeft, SelectRight, SelectUp, SetColor, ToggleDisabled,
    kind_name,
};
use crate::{
    Cancel, Copy, Cut, DeleteSelection, ExportPng, ExportSvg, Paste, Quit, Redo, ResetZoom,
    SaveProject, SaveProjectAs, Undo, ZoomIn, ZoomOut, ZoomToFit,
};

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 4.0;
/// Space around the diagram at the start and when fitted.
const MARGIN: f32 = 20.0;
/// How far the mouse moves before pressing on an element becomes dragging it.
const DRAG_START: f32 = 4.0;

thread_local! {
    /// The system's fonts, loaded once for all rasterizing.
    static OPTIONS: Rc<usvg::Options<'static>> = {
        let mut options = usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        Rc::new(options)
    };
}

/// A window size that shows the whole diagram where it fits on a usual screen.
pub fn window_size(diagram: &Diagram) -> (f32, f32) {
    let (w, h) = Layout::of(diagram, umlcraft_nsd::Metrics::for_platform()).size;
    // Room for the margins and the two toolbars.
    let w = (w as f32 + 2.0 * MARGIN + 20.0).clamp(640.0, 1200.0);
    let h = (h as f32 + 2.0 * MARGIN + 100.0).clamp(400.0, 900.0);
    (w, h)
}

/// The window title: the file's name (or "Untitled"), `*` with unsaved changes.
pub fn title(path: Option<&Path>, dirty: bool) -> String {
    let name = path
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| t!("nsd.untitled").to_string());
    let dirty = if dirty { "*" } else { "" };
    format!("{name}{dirty} – UmlCraft")
}

/// How a window starts.
#[derive(Default)]
pub struct Opening {
    /// The file, or `None` for a diagram not saved yet.
    pub path: Option<PathBuf>,
    /// Where an unsaved diagram is suggested to go (it names the window, too).
    pub draft: Option<PathBuf>,
    /// What an import could not show as written, one line each.
    pub notes: Vec<String>,
    /// Made from something else (code): closing asks even before a change.
    pub unsaved: bool,
}

impl Opening {
    pub fn name(&self) -> Option<&Path> {
        self.path.as_deref().or(self.draft.as_deref())
    }
}

/// The diagram rasterized for one scale.
struct Raster {
    scale: f32,
    image: Arc<RenderImage>,
    /// Its size in pixels of the image.
    size: (u32, u32),
}

enum Drag {
    /// Panning: where it started, in window coordinates, and the pan then.
    Pan { start: Point<Pixels>, pan: Point<Pixels> },
    /// Pressed on selected elements: where, whether the mouse has gone far enough to drag
    /// them, and the place they would drop to.
    Elements { span: Span, start: Point<Pixels>, moving: bool, slot: Option<Slot> },
}

/// What happens once a question about unsaved changes is answered.
pub type Continuation = Box<dyn FnOnce(&mut NsdView, &mut Window, &mut Context<NsdView>)>;

pub struct NsdView {
    /// The file, or `None` for a new diagram never saved.
    path: Option<PathBuf>,
    /// Where a diagram never saved is suggested to go.
    draft: Option<PathBuf>,
    /// What an import could not show as written, and whether that list is shown.
    notes: Vec<String>,
    notes_open: bool,
    editor: Editor,
    layout: Layout,
    /// The diagram as SVG, drawn again after each change.
    svg: String,
    /// The diagram's size at 100 %.
    size: (f32, f32),
    zoom: f32,
    /// Where the diagram's top left corner is in the drawing area.
    pan: Point<Pixels>,
    raster: Option<Raster>,
    /// The drawing area, as painted last.
    area: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<Drag>,
    /// Unsaved changes were discarded on closing or quitting: don't ask again.
    discarded: bool,
    focus: FocusHandle,
    _theme: Subscription,
}

impl NsdView {
    pub fn new(
        opening: Opening,
        diagram: Diagram,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let Opening { path, draft, notes, unsaved } = opening;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |this, cx| this.should_close(window, cx)).unwrap_or(true)
        });
        // A new, empty diagram counts as saved until it is changed: closing it untouched
        // loses nothing. One made from code would be lost.
        let editor = Editor::new(diagram, !unsaved);
        let metrics = umlcraft_nsd::Metrics::for_platform();
        let mut view = NsdView {
            path,
            draft,
            notes,
            notes_open: false,
            layout: Layout::of(editor.diagram(), metrics),
            editor,
            svg: String::new(),
            size: (0.0, 0.0),
            zoom: 1.0,
            pan: point(px(MARGIN), px(MARGIN)),
            raster: None,
            area: Rc::default(),
            drag: None,
            discarded: false,
            focus,
            // The area around the diagram follows the theme the main window switches.
            _theme: cx.observe_global::<Theme>(|_, cx| cx.notify()),
        };
        view.redraw();
        view
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The file or the suggested one, for the title.
    fn name(&self) -> Option<&Path> {
        self.path.as_deref().or(self.draft.as_deref())
    }

    /// Whether closing would lose changes.
    pub fn needs_saving(&self) -> bool {
        self.editor.is_dirty() && !self.discarded
    }

    /// Lays out and draws the diagram again after a change.
    fn redraw(&mut self) {
        let metrics = umlcraft_nsd::Metrics::for_platform();
        self.layout = Layout::of(self.editor.diagram(), metrics);
        self.svg = umlcraft_nsd::svg::render_layout(self.editor.diagram(), &self.layout, metrics);
        let (w, h) = self.layout.size;
        self.size = ((w + 1) as f32, (h + 1) as f32);
        self.raster = None;
    }

    /// Makes one undoable change and shows it. Changes that are not possible (e.g. removing
    /// a case's last branch) do nothing.
    fn change(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Diagram) -> Result<Selection, EditError>,
    ) {
        if self.editor.apply(f).is_ok() {
            self.redraw();
        }
        cx.notify();
    }

    fn select(&mut self, selection: Selection, cx: &mut Context<Self>) {
        self.editor.select(selection);
        cx.notify();
    }

    // ----- Zoom and pan ----------------------------------------------------------------------

    /// The diagram as an image at `scale` pixels per diagram pixel.
    fn raster(&mut self, scale: f32) -> Option<(Arc<RenderImage>, (u32, u32))> {
        if self.raster.as_ref().is_none_or(|r| (r.scale - scale).abs() > 1e-4) {
            self.raster =
                rasterize(&self.svg, scale).map(|(image, size)| Raster { scale, image, size });
        }
        self.raster.as_ref().map(|r| (r.image.clone(), r.size))
    }

    fn set_zoom(&mut self, zoom: f32, anchor: Point<Pixels>, cx: &mut Context<Self>) {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        // Keep the point under `anchor` (relative to the area) where it is.
        let factor = zoom / self.zoom;
        self.pan = point(
            anchor.x - (anchor.x - self.pan.x) * factor,
            anchor.y - (anchor.y - self.pan.y) * factor,
        );
        self.zoom = zoom;
        cx.notify();
    }

    fn centre(&self) -> Point<Pixels> {
        let area = self.area.get();
        point(area.size.width / 2.0, area.size.height / 2.0)
    }

    fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom * 1.25, self.centre(), cx);
    }

    fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom / 1.25, self.centre(), cx);
    }

    fn reset_zoom(&mut self, _: &ResetZoom, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        self.pan = point(px(MARGIN), px(MARGIN));
        cx.notify();
    }

    fn zoom_to_fit(&mut self, _: &ZoomToFit, _: &mut Window, cx: &mut Context<Self>) {
        let area = self.area.get().size;
        let fit_w = (f32::from(area.width) - 2.0 * MARGIN) / self.size.0;
        let fit_h = (f32::from(area.height) - 2.0 * MARGIN) / self.size.1;
        self.zoom = fit_w.min(fit_h).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = point(
            (area.width - px(self.size.0 * self.zoom)) / 2.0,
            (area.height - px(self.size.1 * self.zoom)) / 2.0,
        );
        cx.notify();
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = ev.delta.pixel_delta(px(20.0));
        if ev.modifiers.secondary() {
            let origin = self.area.get().origin;
            let anchor = point(ev.position.x - origin.x, ev.position.y - origin.y);
            let factor = (1.0 + f32::from(delta.y) / 200.0).clamp(0.5, 2.0);
            self.set_zoom(self.zoom * factor, anchor, cx);
        } else {
            self.pan = point(self.pan.x + delta.x, self.pan.y + delta.y);
            cx.notify();
        }
    }

    // ----- Mouse ------------------------------------------------------------------------------

    /// A window position in diagram pixels.
    fn to_diagram(&self, position: Point<Pixels>) -> (i32, i32) {
        let origin = self.area.get().origin;
        let x = f32::from(position.x - origin.x - self.pan.x) / self.zoom;
        let y = f32::from(position.y - origin.y - self.pan.y) / self.zoom;
        (x.floor() as i32, y.floor() as i32)
    }

    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let (x, y) = self.to_diagram(ev.position);
        let Some(hit) = self.layout.hit(x, y) else {
            self.drag = Some(Drag::Pan { start: ev.position, pan: self.pan });
            self.select(Selection::Nothing, cx);
            return;
        };
        if ev.click_count >= 2 {
            self.drag = None;
            match hit {
                Selection::Empty(seq) => {
                    // As in Structorizer: a double click into an empty branch adds an
                    // instruction there.
                    let slot = Slot { seq, index: 0 };
                    self.open_new(slot, ElementKind::Instruction, window, cx);
                }
                hit => {
                    self.select(hit, cx);
                    self.edit_element(&EditElement, window, cx);
                }
            }
            return;
        }
        let current = self.editor.selection().clone();
        if ev.modifiers.shift
            && let (Some(span), Some(path)) = (current.span(), hit.single())
            && path.len() == span.seq.len() + 1
            && path.starts_with(&span.seq)
        {
            let to = *path.last().unwrap_or(&0);
            self.select(Selection::Elements(span.to(to)), cx);
            return;
        }
        // Pressing on what is selected keeps it, so that several elements can be dragged.
        let keep = match (&current, &hit) {
            (Selection::Elements(span), Selection::Elements(h)) => span.covers(&h.first()),
            _ => false,
        };
        let selection = if keep { current } else { hit };
        if let Selection::Elements(span) = &selection {
            self.drag = Some(Drag::Elements {
                span: span.clone(),
                start: ev.position,
                moving: false,
                slot: None,
            });
        }
        self.select(selection, cx);
    }

    /// A right click selects what it hits, unless that is selected already, so that the
    /// context menu acts on it.
    fn on_right_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let (x, y) = self.to_diagram(ev.position);
        let hit = self.layout.hit(x, y).unwrap_or_default();
        let inside = match (self.editor.selection(), &hit) {
            (Selection::Elements(span), Selection::Elements(h)) => span.covers(&h.first()),
            (current, hit) => current == hit,
        };
        if !inside {
            self.select(hit, cx);
        }
    }

    fn on_middle_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.drag = Some(Drag::Pan { start: ev.position, pan: self.pan });
    }

    fn on_mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if ev.pressed_button.is_none() {
            if self.drag.take().is_some() {
                cx.notify();
            }
            return;
        }
        let target = self.to_diagram(ev.position);
        let layout = &self.layout;
        match &mut self.drag {
            Some(Drag::Pan { start, pan }) => {
                self.pan = point(pan.x + ev.position.x - start.x, pan.y + ev.position.y - start.y);
                cx.notify();
            }
            Some(Drag::Elements { span, start, moving, slot }) => {
                let moved = ev.position - *start;
                if !*moving && f32::from(moved.x.abs().max(moved.y.abs())) < DRAG_START {
                    return;
                }
                *moving = true;
                *slot = layout.slot_at(target.0, target.1).filter(|s| drops_somewhere(span, s));
                cx.notify();
            }
            None => {}
        }
    }

    fn on_mouse_up(&mut self, ev: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(Drag::Elements { span, moving: true, slot: Some(slot), .. }) = self.drag.take()
        else {
            cx.notify();
            return;
        };
        // With Ctrl (Cmd) held the elements are copied, otherwise moved.
        if ev.modifiers.secondary() {
            self.change(cx, |d| {
                let copies = edit::copy(d, &span)?;
                edit::insert(d, &slot, copies)
            });
        } else {
            self.change(cx, |d| edit::move_span(d, &span, &slot));
        }
    }

    // ----- Editing ---------------------------------------------------------------------------

    fn span(&self) -> Option<Span> {
        self.editor.selection().span().cloned()
    }

    /// Where something new goes: after the selection, or at the end of the diagram.
    fn slot(&self, after: bool) -> Slot {
        self.editor.selection().slot(after).unwrap_or_else(|| Slot {
            seq: Vec::new(),
            index: self.editor.diagram().children.elements.len(),
        })
    }

    fn open_new(
        &mut self,
        slot: Slot,
        kind: ElementKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = Edited {
            text: edit::default_text(kind),
            comment: Vec::new(),
            disabled: false,
            kind: self.editor.diagram().kind(),
        };
        let target = Target::New(slot, kind);
        dialog::open(cx.entity().downgrade(), target, Some(kind), start, window, cx);
    }

    fn edit_element(&mut self, _: &EditElement, window: &mut Window, cx: &mut Context<Self>) {
        let d = self.editor.diagram();
        let (target, kind, start) = match self.editor.selection() {
            Selection::Root => (
                Target::Root,
                None,
                Edited { text: d.text(), comment: d.comment(), disabled: false, kind: d.kind() },
            ),
            selection => {
                let Some(p) = selection.single() else { return };
                let Some(e) = path::element(d, &p) else { return };
                let start = Edited {
                    text: e.text(),
                    comment: e.comment(),
                    disabled: e.disabled(),
                    kind: d.kind(),
                };
                (Target::Element(p), Some(e.kind), start)
            }
        };
        dialog::open(cx.entity().downgrade(), target, kind, start, window, cx);
    }

    /// Takes what the element dialog hands back.
    pub fn apply_dialog(
        &mut self,
        target: Target,
        edited: Edited,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Edited { text, comment, disabled, kind } = edited;
        match target {
            Target::New(slot, element_kind) => self.change(cx, |d| {
                let mut e = edit::new_element(d, element_kind, &text);
                e.set_comment(&comment);
                e.set_disabled(disabled);
                edit::insert(d, &slot, vec![e])
            }),
            Target::Element(at) => self.change(cx, |d| {
                if edit::has_text(path::element(d, &at).ok_or(EditError::NoSuchPlace)?.kind) {
                    edit::set_text(d, &at, &text)?;
                }
                edit::set_comment(d, &at, &comment)?;
                path::element_mut(d, &at).ok_or(EditError::NoSuchPlace)?.set_disabled(disabled);
                Ok(Selection::element(&at))
            }),
            Target::Root => self.change(cx, |d| edit::set_root(d, &text, &comment, kind)),
        }
        window.focus(&self.focus, cx);
    }

    fn insert_element(&mut self, a: &InsertElement, window: &mut Window, cx: &mut Context<Self>) {
        let slot = self.slot(a.after);
        self.open_new(slot, a.kind, window, cx);
    }

    fn delete(&mut self, _: &DeleteSelection, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(span) = self.span() {
            self.change(cx, |d| edit::remove(d, &span).map(|(_, selection)| selection));
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let d = self.editor.diagram();
        let text = match self.editor.selection() {
            Selection::Root => clip::copy_diagram(d),
            Selection::Elements(span) => match edit::copy(d, span) {
                Ok(elements) => clip::copy(d, elements),
                Err(_) => return,
            },
            _ => return,
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.span().is_some() {
            self.copy(&Copy, window, cx);
            self.delete(&DeleteSelection, window, cx);
        }
    }

    /// Pastes after the selection (into an empty branch when that is selected).
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else { return };
        let elements = clip::paste(&text);
        if elements.is_empty() {
            return;
        }
        let slot = self.slot(true);
        self.change(cx, |d| edit::insert(d, &slot, elements));
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        self.drag = None;
        self.select(Selection::Nothing, cx);
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.undo() {
            self.redraw();
            cx.notify();
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.redo() {
            self.redraw();
            cx.notify();
        }
    }

    fn move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(span) = self.span() {
            self.change(cx, |d| edit::shift(d, &span, true));
        }
    }

    fn move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(span) = self.span() {
            self.change(cx, |d| edit::shift(d, &span, false));
        }
    }

    fn set_color(&mut self, a: &SetColor, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(span) = self.span() {
            self.change(cx, |d| edit::set_color(d, &span, Color(a.color)));
        }
    }

    fn toggle_disabled(&mut self, _: &ToggleDisabled, _: &mut Window, cx: &mut Context<Self>) {
        let Some(span) = self.span() else { return };
        let d = self.editor.diagram();
        let all = (span.start..span.end)
            .all(|i| path::element(d, &span.element(i)).is_some_and(|e| e.disabled()));
        self.change(cx, |d| edit::set_disabled(d, &span, !all));
    }

    /// The case or parallel element whose branches the selection is about, and the branch
    /// meant: a selected empty branch, or none for the element itself.
    fn branching(&self) -> Option<(NsdPath, Option<usize>)> {
        let d = self.editor.diagram();
        let branching = |p: &[usize]| {
            path::element(d, p)
                .is_some_and(|e| matches!(e.kind, ElementKind::Case | ElementKind::Parallel))
        };
        match self.editor.selection() {
            Selection::Empty(seq) => {
                let owner = edit::owner(seq)?;
                branching(&owner).then(|| (owner, seq.last().copied()))
            }
            selection => selection.single().filter(|p| branching(p)).map(|p| (p, None)),
        }
    }

    fn add_branch(&mut self, _: &AddBranch, _: &mut Window, cx: &mut Context<Self>) {
        let Some((at, _)) = self.branching() else { return };
        let Some(e) = path::element(self.editor.diagram(), &at) else { return };
        // A new case goes before the default branch.
        let index = match e.kind {
            ElementKind::Case => e.branches.len() - 1,
            _ => e.branches.len(),
        };
        self.change(cx, |d| edit::add_branch(d, &at, index));
    }

    fn remove_branch(&mut self, _: &RemoveBranch, _: &mut Window, cx: &mut Context<Self>) {
        let Some((at, branch)) = self.branching() else { return };
        let Some(e) = path::element(self.editor.diagram(), &at) else { return };
        let last = match e.kind {
            ElementKind::Case => e.branches.len().saturating_sub(2),
            _ => e.branches.len().saturating_sub(1),
        };
        let index = branch.unwrap_or(last);
        self.change(cx, |d| edit::remove_branch(d, &at, index));
    }

    // ----- Keyboard selection ----------------------------------------------------------------

    /// The first element of a sequence, or the sequence when it is empty.
    fn first_in(&self, seq: NsdPath) -> Selection {
        let empty =
            path::sequence(self.editor.diagram(), &seq).is_none_or(|s| s.elements.is_empty());
        if empty {
            Selection::Empty(seq)
        } else {
            let mut p = seq;
            p.push(0);
            Selection::element(&p)
        }
    }

    /// The element after `p`, climbing out of branches at their end.
    fn after(&self, p: &[usize]) -> Option<Selection> {
        let (last, seq) = p.split_last()?;
        let len = path::sequence(self.editor.diagram(), seq)?.elements.len();
        if last + 1 < len {
            let mut next = seq.to_vec();
            next.push(last + 1);
            return Some(Selection::element(&next));
        }
        self.after(&edit::owner(seq)?)
    }

    /// The branch next to the one holding `seq` (`step` -1 or 1).
    fn beside(&self, seq: &[usize], step: isize) -> Option<Selection> {
        let owner = edit::owner(seq)?;
        let branch = seq.last()?.checked_add_signed(step)?;
        let e = path::element(self.editor.diagram(), &owner)?;
        // A case without default hides its last branch.
        let shown = match e.kind {
            ElementKind::Case if e.text().last().is_some_and(|l| l.trim() == "%") => {
                e.branches.len() - 1
            }
            _ => e.branches.len(),
        };
        (branch < shown).then(|| {
            let mut s = owner;
            s.push(branch);
            self.first_in(s)
        })
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let next = match self.editor.selection() {
            Selection::Elements(span) if span.start > 0 => {
                Some(Selection::element(&span.element(span.start - 1)))
            }
            Selection::Elements(Span { seq, .. }) | Selection::Empty(seq) => {
                edit::owner(seq).map(|o| Selection::element(&o))
            }
            Selection::Nothing => Some(self.first_in(Vec::new())),
            Selection::Root => None,
        };
        if let Some(next) = next {
            self.select(next, cx);
        }
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let next = match self.editor.selection() {
            Selection::Elements(span) => self.after(&span.element(span.end - 1)),
            Selection::Empty(seq) => edit::owner(seq).and_then(|o| self.after(&o)),
            Selection::Nothing | Selection::Root => Some(self.first_in(Vec::new())),
        };
        if let Some(next) = next {
            self.select(next, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        let seq = match self.editor.selection() {
            Selection::Elements(span) => span.seq.clone(),
            Selection::Empty(seq) => seq.clone(),
            _ => return,
        };
        let next =
            self.beside(&seq, -1).or_else(|| edit::owner(&seq).map(|o| Selection::element(&o)));
        if let Some(next) = next {
            self.select(next, cx);
        }
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        let next = match self.editor.selection() {
            Selection::Elements(span) => {
                let p = span.first();
                let branches =
                    path::element(self.editor.diagram(), &p).map_or(0, |e| e.branches.len());
                if span.len() == 1 && branches > 0 {
                    let mut s = p;
                    s.push(0);
                    Some(self.first_in(s))
                } else {
                    self.beside(&span.seq, 1)
                }
            }
            Selection::Empty(seq) => self.beside(seq, 1),
            _ => None,
        };
        if let Some(next) = next {
            self.select(next, cx);
        }
    }

    fn extend(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some(span) = self.span() else { return };
        let len = path::sequence(self.editor.diagram(), &span.seq).map_or(0, |s| s.elements.len());
        let span = match up {
            true if span.start > 0 => Span { start: span.start - 1, ..span },
            false if span.end < len => Span { end: span.end + 1, ..span },
            _ => return,
        };
        self.select(Selection::Elements(span), cx);
    }

    fn extend_up(&mut self, _: &ExtendUp, _: &mut Window, cx: &mut Context<Self>) {
        self.extend(true, cx);
    }

    fn extend_down(&mut self, _: &ExtendDown, _: &mut Window, cx: &mut Context<Self>) {
        self.extend(false, cx);
    }

    // ----- Files -----------------------------------------------------------------------------

    fn save_action(&mut self, _: &SaveProject, window: &mut Window, cx: &mut Context<Self>) {
        self.save(false, window, cx, None);
    }

    fn save_as_action(&mut self, _: &SaveProjectAs, window: &mut Window, cx: &mut Context<Self>) {
        self.save(true, window, cx, None);
    }

    /// Saves to the file, or under a new name (always for a new diagram); then `then`.
    fn save(
        &mut self,
        save_as: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: Option<Continuation>,
    ) {
        if let Some(path) = self.path.clone().filter(|_| !save_as) {
            if self.write(path, window, cx)
                && let Some(then) = then
            {
                then(self, window, cx);
            }
            return;
        }
        let suggested = self
            .path
            .as_deref()
            .or(self.draft.as_deref())
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}.nsd", t!("nsd.untitled_stem")));
        let directory = self
            .path
            .as_deref()
            .or(self.draft.as_deref())
            .and_then(Path::parent)
            .map(PathBuf::from)
            .or_else(crate::main_view::documents_dir)
            .unwrap_or_default();
        let rx = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = rx.await else { return };
            let path = if super::is_structogram(&path) { path } else { path.with_extension("nsd") };
            this.update_in(cx, move |this, window, cx| {
                if this.write(path, window, cx)
                    && let Some(then) = then
                {
                    then(this, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Writes the diagram to `path`: through a temporary file, so that a failed write leaves
    /// the old file whole. An unchanged diagram is written as it was read.
    fn write(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let unchanged_here = !self.editor.is_dirty() && self.path.as_deref() == Some(&path);
        if unchanged_here && path.exists() {
            return true;
        }
        let diagram = self.editor.to_save(&super::user_name(), &super::today());
        let text = umlcraft_nsd::write(&diagram);
        let mut temporary = path.clone().into_os_string();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);
        let written = std::fs::write(&temporary, text).and_then(|()| {
            std::fs::rename(&temporary, &path).inspect_err(|_| {
                std::fs::remove_file(&temporary).ok();
            })
        });
        match written {
            Ok(()) => {
                self.editor.saved(diagram);
                self.path = Some(path.clone());
                self.discarded = false;
                self.redraw();
                super::saved(&path, cx);
                cx.notify();
                true
            }
            Err(err) => {
                let rx = window.prompt(
                    PromptLevel::Critical,
                    &t!("nsd.save_failed"),
                    Some(&err.to_string()),
                    &[t!("dialog.ok").as_ref()],
                    cx,
                );
                cx.spawn(async move |_, _| rx.await.ok()).detach();
                false
            }
        }
    }

    /// Runs `then` once unsaved changes are saved or discarded; nothing on cancel.
    pub fn confirm_discard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: Continuation,
    ) {
        if !self.needs_saving() {
            then(self, window, cx);
            return;
        }
        let message = t!("nsd.unsaved", name = title(self.name(), false)).to_string();
        let detail = t!("dialog.unsaved_detail").to_string();
        let answers = [
            t!("dialog.save").to_string(),
            t!("dialog.discard").to_string(),
            t!("dialog.cancel").to_string(),
        ];
        let answers: Vec<&str> = answers.iter().map(String::as_str).collect();
        let rx = window.prompt(PromptLevel::Warning, &message, Some(&detail), &answers, cx);
        cx.spawn_in(window, async move |this, cx| {
            let Ok(answer) = rx.await else { return };
            this.update_in(cx, move |this, window, cx| match answer {
                0 => this.save(false, window, cx, Some(then)),
                1 => {
                    this.discarded = true;
                    then(this, window, cx);
                }
                _ => {}
            })
            .ok();
        })
        .detach();
    }

    fn should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.needs_saving() {
            return true;
        }
        self.confirm_discard(window, cx, Box::new(|_, window, _| window.remove_window()));
        false
    }

    /// Quitting from a structogram window quits like the main window does.
    fn quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.defer(super::quit);
    }

    // ----- Export ----------------------------------------------------------------------------

    fn export(&mut self, png: bool, window: &mut Window, cx: &mut Context<Self>) {
        let extension = if png { "png" } else { "svg" };
        let stem = self.name().and_then(Path::file_stem);
        let stem = stem.map_or_else(
            || t!("nsd.untitled_stem").to_string(),
            |s| s.to_string_lossy().into_owned(),
        );
        let suggested = format!("{stem}.{extension}");
        let directory = self.name().and_then(Path::parent).map(PathBuf::from);
        let directory = directory.or_else(crate::main_view::documents_dir).unwrap_or_default();
        let rx = cx.prompt_for_new_path(&directory, Some(&suggested));
        let svg = self.svg.clone();
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(path))) = rx.await else { return };
            let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)) {
                path
            } else {
                path.with_extension(extension)
            };
            let written = cx
                .background_executor()
                .spawn(async move {
                    let bytes = if png {
                        // `from_svg` takes images made at Astah's 4/3 pixels per unit; ours
                        // have one.
                        let scale = crate::png::SCALE * umlcraft_notation::svg::SCALE;
                        crate::png::from_svg(&svg, scale)?
                    } else {
                        svg.into_bytes()
                    };
                    anyhow::Ok(std::fs::write(&path, bytes)?)
                })
                .await;
            if let Err(err) = written {
                cx.update(|window, cx| {
                    let rx = window.prompt(
                        PromptLevel::Critical,
                        &t!("dialog.image_failed"),
                        Some(&err.to_string()),
                        &[t!("dialog.ok").as_ref()],
                        cx,
                    );
                    cx.spawn(async move |_| rx.await.ok()).detach();
                })
                .ok();
            }
        })
        .detach();
    }

    fn export_svg(&mut self, _: &ExportSvg, window: &mut Window, cx: &mut Context<Self>) {
        self.export(false, window, cx);
    }

    fn export_png(&mut self, _: &ExportPng, window: &mut Window, cx: &mut Context<Self>) {
        self.export(true, window, cx);
    }

    // ----- Drawing ---------------------------------------------------------------------------

    /// What to draw over the diagram, in diagram pixels: the selection's boxes, and where a
    /// drag would drop (a line between elements, or a box for an empty branch).
    fn overlay(&self) -> (Vec<[i32; 4]>, Option<[i32; 4]>) {
        let l = &self.layout;
        let boxes = match self.editor.selection() {
            Selection::Nothing => Vec::new(),
            Selection::Root => vec![[0, 0, l.size.0, l.size.1]],
            Selection::Empty(seq) => {
                l.sequence(seq).map(|s| vec![rect(s.rect)]).unwrap_or_default()
            }
            Selection::Elements(span) => {
                let first = l.element(&span.first());
                let last = l.element(&span.element(span.end - 1));
                match (first, last) {
                    (Some(a), Some(b)) => {
                        vec![[a.rect.x, a.rect.y, a.rect.w, b.rect.bottom() - a.rect.y]]
                    }
                    _ => Vec::new(),
                }
            }
        };
        let drop = match &self.drag {
            Some(Drag::Elements { moving: true, slot: Some(slot), .. }) => {
                l.sequence(&slot.seq).map(|s| match s.elements.get(slot.index) {
                    _ if s.elements.is_empty() => rect(s.rect),
                    Some(e) => [s.rect.x, e.rect.y - 1, s.rect.w, 3],
                    None => {
                        let bottom = s.elements.last().map_or(s.rect.y, |e| e.rect.bottom());
                        [s.rect.x, bottom - 1, s.rect.w, 3]
                    }
                })
            }
            _ => None,
        };
        (boxes, drop)
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = (self.zoom * 100.0).round();
        let separator = || div().w(px(1.0)).h(px(18.0)).mx_1().bg(cx.theme().border);
        let action_button = |id: &'static str, label: String, action: Box<dyn Action>| {
            Button::new(id).ghost().small().label(label).on_click(move |_, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx);
            })
        };
        h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .flex_wrap()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(action_button("save", t!("toolbar.save").into(), Box::new(SaveProject)))
            .child(action_button("save-as", t!("menu.save_as").into(), Box::new(SaveProjectAs)))
            .child(separator())
            .child(
                action_button("undo", t!("toolbar.undo").into(), Box::new(Undo))
                    .disabled(!self.editor.can_undo()),
            )
            .child(
                action_button("redo", t!("toolbar.redo").into(), Box::new(Redo))
                    .disabled(!self.editor.can_redo()),
            )
            .child(separator())
            .child(action_button("export-svg", t!("menu.export_svg").into(), Box::new(ExportSvg)))
            .child(action_button("export-png", t!("menu.export_png").into(), Box::new(ExportPng)))
            .child(separator())
            .child(action_button("zoom-out", "−".into(), Box::new(ZoomOut)))
            .child(
                action_button("zoom-reset", format!("{zoom} %"), Box::new(ResetZoom))
                    .tooltip(t!("toolbar.zoom_reset")),
            )
            .child(action_button("zoom-in", "+".into(), Box::new(ZoomIn)))
            .child(action_button("zoom-fit", t!("toolbar.zoom_fit").into(), Box::new(ZoomToFit)))
    }

    /// The second bar: a button per kind of element (Shift inserts before the selection, as
    /// in Structorizer) and Structorizer's paint box.
    fn insert_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_span = self.span().is_some();
        let kinds = ElementKind::ALL.into_iter().enumerate().map(|(i, kind)| {
            Button::new(("nsd-insert", i))
                .ghost()
                .small()
                .label(kind_name(kind))
                .tooltip(t!("nsd.insert_tooltip"))
                .on_click(move |ev, window, cx| {
                    let after = !ev.modifiers().shift;
                    window.dispatch_action(Box::new(InsertElement { kind, after }), cx);
                })
        });
        let swatches = PALETTE.into_iter().enumerate().map(|(i, [r, g, b])| {
            let color =
                Rgba { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 };
            div()
                .id(("nsd-color", i))
                .size(px(16.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(color)
                .cursor_pointer()
                .opacity(if has_span { 1.0 } else { 0.4 })
                .on_click(move |_, window, cx| {
                    window.dispatch_action(Box::new(SetColor { color: [r, g, b] }), cx);
                })
        });
        h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .flex_wrap()
            .border_b_1()
            .border_color(cx.theme().border)
            .children(kinds)
            .child(div().w(px(1.0)).h(px(18.0)).mx_1().bg(cx.theme().border))
            .children(swatches)
    }

    /// What an import could not show as written: a line that opens into the list.
    fn notes_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.notes.is_empty() {
            return None;
        }
        let summary = t!("nsd.notes", count = self.notes.len()).to_string();
        let arrow = if self.notes_open { "▾" } else { "▸" };
        let list = self.notes_open.then(|| {
            v_flex()
                .id("nsd-notes")
                .max_h(px(140.0))
                .overflow_y_scroll()
                .pl_4()
                .children(self.notes.iter().map(|n| div().text_xs().child(n.clone())))
        });
        Some(
            v_flex()
                .px_2()
                .py_1()
                .border_t_1()
                .border_color(cx.theme().border)
                .text_sm()
                .child(
                    div()
                        .id("nsd-notes-toggle")
                        .cursor_pointer()
                        .child(format!("{arrow} {summary}"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.notes_open = !this.notes_open;
                            cx.notify();
                        })),
                )
                .children(list),
        )
    }

    /// The context menu: what the selection allows.
    fn context_menu(
        view: &Entity<Self>,
        menu: PopupMenu,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let (selection, branching, can_undo, can_redo, disabled, focus) = {
            let this = view.read(cx);
            let span = this.span();
            let d = this.editor.diagram();
            let disabled = span.as_ref().is_some_and(|s| {
                (s.start..s.end)
                    .all(|i| path::element(d, &s.element(i)).is_some_and(|e| e.disabled()))
            });
            (
                this.editor.selection().clone(),
                this.branching().is_some(),
                this.editor.can_undo(),
                this.editor.can_redo(),
                disabled,
                this.focus.clone(),
            )
        };
        let editable = matches!(selection, Selection::Root) || selection.single().is_some();
        let has_span = selection.span().is_some();
        let copyable = has_span || matches!(selection, Selection::Root);
        let mut menu = menu
            .action_context(focus)
            .menu_with_disabled(t!("nsd.edit"), Box::new(EditElement), !editable)
            .separator();
        for (title, after) in [("nsd.insert_after", true), ("nsd.insert_before", false)] {
            menu = menu.submenu(t!(title), window, cx, move |menu, _, _| {
                ElementKind::ALL.into_iter().fold(menu, |menu, kind| {
                    menu.menu(kind_name(kind), Box::new(InsertElement { kind, after }))
                })
            });
        }
        menu = menu
            .separator()
            .menu_with_disabled(t!("menu.cut"), Box::new(Cut), !has_span)
            .menu_with_disabled(t!("menu.copy"), Box::new(Copy), !copyable)
            .menu(t!("nsd.paste"), Box::new(Paste))
            .menu_with_disabled(t!("nsd.delete"), Box::new(DeleteSelection), !has_span)
            .separator()
            .menu_with_disabled(t!("nsd.move_up"), Box::new(MoveUp), !has_span)
            .menu_with_disabled(t!("nsd.move_down"), Box::new(MoveDown), !has_span)
            .separator();
        if has_span {
            menu = menu.submenu(t!("nsd.color"), window, cx, |menu, _, _| {
                PALETTE.into_iter().enumerate().fold(menu, |menu, (i, color)| {
                    menu.menu(t!(format!("nsd.color_{i}")), Box::new(SetColor { color }))
                })
            });
            menu = menu.menu_with_check(t!("nsd.disabled"), disabled, Box::new(ToggleDisabled));
        }
        if branching {
            menu = menu
                .menu(t!("nsd.add_branch"), Box::new(AddBranch))
                .menu(t!("nsd.remove_branch"), Box::new(RemoveBranch));
        }
        menu.separator()
            .menu_with_disabled(t!("toolbar.undo"), Box::new(Undo), !can_undo)
            .menu_with_disabled(t!("toolbar.redo"), Box::new(Redo), !can_redo)
    }
}

/// Whether dropping `span` at `slot` would change anything and is possible.
fn drops_somewhere(span: &Span, slot: &Slot) -> bool {
    let in_place = slot.seq == span.seq && (span.start..=span.end).contains(&slot.index);
    !in_place && !span.covers(&slot.seq)
}

fn rect(r: umlcraft_nsd::layout::Rect) -> [i32; 4] {
    [r.x, r.y, r.w, r.h]
}

/// The SVG rasterized at `scale`, as GPUI takes images (BGRA, not premultiplied).
fn rasterize(svg: &str, scale: f32) -> Option<(Arc<RenderImage>, (u32, u32))> {
    let tree = OPTIONS.with(|options| usvg::Tree::from_str(svg, options)).ok()?;
    let size = tree.size().to_int_size().scale_by(scale)?;
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let (w, h) = (pixmap.width(), pixmap.height());
    let mut buffer = image::ImageBuffer::from_raw(w, h, pixmap.take())?;
    for pixel in buffer.as_chunks_mut::<4>().0 {
        swap_rgba_pa_to_bgra(pixel);
    }
    Some((Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])), (w, h)))
}

impl Render for NsdView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&title(self.name(), self.editor.is_dirty()));
        let this = cx.entity();
        let menu_owner = cx.entity();
        let area = self.area.clone();
        let surround = cx.theme().muted;
        let accent = cx.theme().blue;
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .track_focus(&self.focus)
            .key_context("NsdView")
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::reset_zoom))
            .on_action(cx.listener(Self::zoom_to_fit))
            .on_action(cx.listener(Self::export_svg))
            .on_action(cx.listener(Self::export_png))
            .on_action(cx.listener(Self::save_action))
            .on_action(cx.listener(Self::save_as_action))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::edit_element))
            .on_action(cx.listener(Self::insert_element))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::move_up))
            .on_action(cx.listener(Self::move_down))
            .on_action(cx.listener(Self::set_color))
            .on_action(cx.listener(Self::toggle_disabled))
            .on_action(cx.listener(Self::add_branch))
            .on_action(cx.listener(Self::remove_branch))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::extend_up))
            .on_action(cx.listener(Self::extend_down))
            .child(self.toolbar(cx))
            .child(self.insert_bar(cx))
            .child(
                div()
                    .id("structogram")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .bg(surround)
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_down))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up(
                        MouseButton::Middle,
                        cx.listener(|this, _: &MouseUpEvent, _, _| this.drag = None),
                    )
                    .child(
                        canvas(
                            move |bounds, _, _| area.set(bounds),
                            move |bounds, _, window, cx| {
                                let scale = window.scale_factor();
                                let (pan, zoom, raster, (boxes, drop)) =
                                    this.update(cx, |this, _| {
                                        let raster = this.raster(this.zoom * scale);
                                        (this.pan, this.zoom, raster, this.overlay())
                                    });
                                let Some((image, (w, h))) = raster else { return };
                                let origin =
                                    point(bounds.origin.x + pan.x, bounds.origin.y + pan.y);
                                let size = size(px(w as f32 / scale), px(h as f32 / scale));
                                let image_bounds = Bounds::new(origin, size);
                                let on_screen = |[x, y, w, h]: [i32; 4]| {
                                    Bounds::new(
                                        point(
                                            origin.x + px(x as f32 * zoom),
                                            origin.y + px(y as f32 * zoom),
                                        ),
                                        gpui_kit::size(px(w as f32 * zoom), px(h as f32 * zoom)),
                                    )
                                };
                                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                                    window
                                        .paint_image(
                                            image_bounds,
                                            image_bounds,
                                            Corners::default(),
                                            image,
                                            0,
                                            false,
                                        )
                                        .ok();
                                    for b in boxes {
                                        window.paint_quad(quad(
                                            on_screen(b),
                                            px(0.0),
                                            accent.opacity(0.12),
                                            px(2.0),
                                            accent,
                                            BorderStyle::Solid,
                                        ));
                                    }
                                    if let Some(d) = drop {
                                        window.paint_quad(fill(on_screen(d), accent));
                                    }
                                });
                            },
                        )
                        .size_full(),
                    )
                    .context_menu(move |menu, window, cx| {
                        Self::context_menu(&menu_owner, menu, window, cx)
                    }),
            )
            .children(self.notes_bar(cx))
    }
}

impl Focusable for NsdView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
