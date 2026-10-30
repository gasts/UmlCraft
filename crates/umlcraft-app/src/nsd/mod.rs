//! Structograms (Nassi-Shneiderman diagrams in Structorizer's `.nsd` format) in windows of
//! their own. They are not part of the UML project; each file opens its own window, and a file
//! that is open already brings its window to the front.

mod dialog;
mod source;
mod view;

use std::path::{Path, PathBuf};

use gpui_kit::*;
use rust_i18n::t;
use umlcraft_nsd::{Diagram, ElementKind};

pub use source::choose as choose_source_method;
use view::{NsdView, Opening};

actions!(
    nsd,
    [
        EditElement,
        MoveUp,
        MoveDown,
        SelectUp,
        SelectDown,
        SelectLeft,
        SelectRight,
        ExtendUp,
        ExtendDown,
        ToggleDisabled,
        AddBranch,
        RemoveBranch,
    ]
);

/// Inserts a new element of `kind` after (or before) the selection, through the dialog.
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = nsd, no_json)]
pub struct InsertElement {
    pub kind: ElementKind,
    pub after: bool,
}

/// Dyes the selected elements (white is Structorizer's "no colour").
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = nsd, no_json)]
pub struct SetColor {
    pub color: [u8; 3],
}

/// The paint box of a fresh Structorizer installation.
pub const PALETTE: [[u8; 3]; 10] = [
    [0xff, 0xff, 0xff],
    [0xff, 0x80, 0x80],
    [0xff, 0xff, 0x80],
    [0x80, 0xff, 0x80],
    [0x80, 0xff, 0xff],
    [0x00, 0x80, 0xff],
    [0xff, 0x80, 0xc0],
    [0xc0, 0xc0, 0xc0],
    [0xff, 0x80, 0x00],
    [0x80, 0x80, 0xff],
];

/// The keys of structogram windows.
pub fn bind_keys(cx: &mut App) {
    let c = Some("NsdView");
    cx.bind_keys([
        KeyBinding::new("enter", EditElement, c),
        KeyBinding::new("delete", crate::DeleteSelection, c),
        KeyBinding::new("secondary-c", crate::Copy, c),
        KeyBinding::new("secondary-x", crate::Cut, c),
        KeyBinding::new("shift-delete", crate::Cut, c),
        KeyBinding::new("secondary-v", crate::Paste, c),
        KeyBinding::new("escape", crate::Cancel, c),
        KeyBinding::new("up", SelectUp, c),
        KeyBinding::new("down", SelectDown, c),
        KeyBinding::new("left", SelectLeft, c),
        KeyBinding::new("right", SelectRight, c),
        KeyBinding::new("shift-up", ExtendUp, c),
        KeyBinding::new("shift-down", ExtendDown, c),
        KeyBinding::new("secondary-up", MoveUp, c),
        KeyBinding::new("secondary-down", MoveDown, c),
    ]);
}

/// An element kind's name in the UI's language.
pub fn kind_name(kind: ElementKind) -> String {
    t!(format!("nsd.kind_{}", kind.tag())).to_string()
}

/// A structogram window: its view, so that "already open" and quitting can ask it.
struct Open {
    handle: AnyWindowHandle,
    view: WeakEntity<NsdView>,
}

/// The windows of the structograms that are open.
#[derive(Default)]
struct Structograms(Vec<Open>);

impl Global for Structograms {}

/// What structogram windows ask of the main window: to remember a saved file among the
/// recent ones, and to quit (asking about its own unsaved changes first). `quit` answers
/// whether the main window was still there to do it.
struct MainHooks {
    saved: Box<SavedHook>,
    quit: Box<dyn Fn(&mut App) -> bool>,
}

type SavedHook = dyn Fn(&Path, &mut App);

impl Global for MainHooks {}

/// Connects structogram windows to the main window.
pub fn connect(
    cx: &mut App,
    saved: impl Fn(&Path, &mut App) + 'static,
    quit: impl Fn(&mut App) -> bool + 'static,
) {
    cx.set_global(MainHooks { saved: Box::new(saved), quit: Box::new(quit) });
}

fn saved(path: &Path, cx: &mut App) {
    if cx.has_global::<MainHooks>() {
        cx.update_global::<MainHooks, _>(|hooks, cx| (hooks.saved)(path, cx));
    }
}

/// Quits through the main window, or right away (after asking) when it is closed.
fn quit(cx: &mut App) {
    let handled = cx.has_global::<MainHooks>()
        && cx.update_global::<MainHooks, _>(|hooks, cx| (hooks.quit)(cx));
    if !handled {
        confirm_all(cx, Box::new(|cx| cx.quit()));
    }
}

/// Whether a file is a structogram, by its extension.
pub fn is_structogram(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nsd"))
}

/// The open structogram windows, without those closed meanwhile.
fn open_windows(cx: &mut App) -> Vec<(AnyWindowHandle, Entity<NsdView>)> {
    if !cx.has_global::<Structograms>() {
        cx.set_global(Structograms::default());
    }
    let alive: Vec<(AnyWindowHandle, Entity<NsdView>)> = cx
        .global::<Structograms>()
        .0
        .iter()
        .filter_map(|o| Some((o.handle, o.view.upgrade()?)))
        .filter(|(handle, _)| cx.windows().contains(handle))
        .collect();
    cx.global_mut::<Structograms>().0 = alive
        .iter()
        .map(|(handle, view)| Open { handle: *handle, view: view.downgrade() })
        .collect();
    alive
}

/// Opens `path` in a window of its own, or brings its window to the front. The error is the
/// reason in the UI's language.
pub fn open(path: &Path, cx: &mut App) -> Result<(), String> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    for (handle, view) in open_windows(cx) {
        if view.read(cx).path().is_some_and(|open| same_file(open, &path)) {
            handle.update(cx, |_, window, _| window.activate_window()).ok();
            return Ok(());
        }
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| t!("error.nsd_not_utf8").to_string())?;
    let diagram = umlcraft_nsd::read(&text).map_err(|e| crate::errors::nsd_error(&e))?;
    show(Opening { path: Some(path), ..Opening::default() }, diagram, cx)
}

/// Whether `a` and `b` name the same file, also written differently (`Sort.nsd` is `sort.nsd`
/// on Windows).
fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// Opens a window with a new, empty structogram.
pub fn new(cx: &mut App) -> Result<(), String> {
    let diagram = umlcraft_nsd::save::new_diagram(&user_name(), &today(), &origin());
    show(Opening::default(), diagram, cx)
}

/// Opens a structogram made from code: unsaved, to be saved as `draft` (suggested), with what
/// the import could not show as written (`notes`).
pub fn open_imported(
    diagram: Diagram,
    draft: PathBuf,
    notes: Vec<String>,
    cx: &mut App,
) -> Result<(), String> {
    let opening = Opening { path: None, draft: Some(draft), notes, unsaved: true };
    show(opening, diagram, cx)
}

fn show(opening: Opening, diagram: Diagram, cx: &mut App) -> Result<(), String> {
    let (w, h) = view::window_size(&diagram);
    let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(view::title(opening.name(), false).into()),
            ..Default::default()
        }),
        window_min_size: Some(size(px(480.0), px(320.0))),
        app_id: Some("com.umlcraft.app".into()),
        ..Default::default()
    };
    let (handle, view) = gpui_kit::open_window(options, cx, move |window, cx| {
        cx.new(|cx| NsdView::new(opening, diagram, window, cx))
    })
    .map_err(|e| e.to_string())?;
    open_windows(cx);
    cx.global_mut::<Structograms>().0.push(Open { handle, view: view.downgrade() });
    Ok(())
}

/// Asks every structogram window with unsaved changes whether to save them, one after the
/// other, and then runs `then` (quitting). Cancelling in one stops there.
pub fn confirm_all(cx: &mut App, then: Box<dyn FnOnce(&mut App)>) {
    // What an open element dialog holds counts as a change. Dialogs in windows not asked yet
    // stay open: cancelling there leaves them as they were.
    let dirty = open_windows(cx).into_iter().find(|(handle, view)| {
        handle
            .update(cx, |_, window, cx| view.update(cx, |view, cx| view.flush_dialog(window, cx)))
            .ok();
        view.read(cx).needs_saving()
    });
    let Some((handle, view)) = dirty else {
        then(cx);
        return;
    };
    handle
        .update(cx, |_, window, cx| {
            window.activate_window();
            view.update(cx, |view, cx| {
                view.confirm_discard(
                    window,
                    cx,
                    Box::new(move |_, _, cx| {
                        cx.defer(move |cx| confirm_all(cx, then));
                    }),
                );
            });
        })
        .ok();
}

/// The message for a structogram that could not be opened.
pub fn open_failed() -> String {
    t!("dialog.structogram_open_failed").to_string()
}

/// Who saves: the system's user name, as Structorizer writes it.
fn user_name() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "???".to_owned())
}

/// Today here (not in UTC: just after midnight it is already tomorrow) as `YYYY-MM-DD`.
fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// What new structograms name as their origin.
fn origin() -> String {
    format!("UmlCraft {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui_kit brings a `test` attribute of its own.
    use std::path::Path;

    use super::{is_structogram, origin, same_file};

    #[test]
    fn structograms_go_by_their_extension() {
        assert!(is_structogram(Path::new("sort.nsd")));
        assert!(is_structogram(Path::new(r"C:\Diagramme\SORT.NSD")));
        assert!(!is_structogram(Path::new("shop.ucraft")));
        assert!(!is_structogram(Path::new("shop.asta")));
        assert!(!is_structogram(Path::new("nsd")));
    }

    #[test]
    fn a_file_is_found_however_it_is_written() {
        let dir = std::env::temp_dir().join(format!("umlcraft-same-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Sort.nsd");
        std::fs::write(&file, "").unwrap();
        assert!(same_file(&file, &dir.join(".").join("Sort.nsd")));
        #[cfg(windows)]
        assert!(same_file(&file, &dir.join("SORT.NSD")));
        assert!(!same_file(&file, &dir.join("Other.nsd")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_structograms_name_umlcraft_as_their_origin() {
        assert!(origin().starts_with("UmlCraft "));
    }
}
