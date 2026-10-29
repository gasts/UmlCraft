#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod canvas;
mod errors;
mod labels;
mod main_view;
mod nsd;
mod png;
mod properties;
mod settings;
mod settings_dialog;
mod workspace;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::*;
use umlcraft_model::{Id, LineShape};
use umlcraft_reverse::SourceLanguage;

use crate::workspace::{EndChange, NewItem};

rust_i18n::i18n!("../../locales", fallback = "en");

actions!(
    umlcraft,
    [
        NewProject,
        OpenProject,
        OpenStructogram,
        NewStructogram,
        StructogramFromJava,
        ImportAstah,
        ExportAstah,
        ExportSvg,
        ExportPng,
        SaveProject,
        SaveProjectAs,
        Quit,
        Undo,
        Redo,
        DeleteSelection,
        RemoveFromDiagram,
        SelectAll,
        Cancel,
        ResetZoom,
        ZoomIn,
        ZoomOut,
        ZoomToFit,
        AlignLeft,
        AlignCenter,
        AlignRight,
        AlignTop,
        AlignMiddle,
        AlignBottom,
        ToggleTheme,
        ToggleLanguage,
        OpenSettings,
        OpenSample,
        Copy,
        Cut,
        Paste,
        PasteAsReference,
        WrapInFragment,
        ResetLineEnds,
        ResetLabels,
        AddExtensionPoint,
    ]
);

/// Changes one end of an association, from the canvas's context menu.
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = umlcraft, no_json)]
pub struct SetAssociationEnd {
    pub association: Id,
    pub end: usize,
    pub change: EndChange,
}

/// Opens an entry of the "recent files" list.
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = umlcraft, no_json)]
pub struct OpenRecent {
    pub index: usize,
}

/// Gives the selected edges a line style.
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = umlcraft, no_json)]
pub struct SetLineShape {
    pub shape: LineShape,
}

/// Imports source files, or a folder of them, into the open project (Tools menu).
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = umlcraft, no_json)]
pub struct ImportSource {
    pub language: SourceLanguage,
    /// Ask for a folder, searched recursively, instead of files.
    pub folder: bool,
}

/// The line styles in menu order, with their labels.
pub const LINE_SHAPES: [(LineShape, &str); 4] = [
    (LineShape::Line, "menu.line_straight"),
    (LineShape::RightAngle, "menu.line_right_angle"),
    (LineShape::Curve, "menu.line_curve"),
    (LineShape::CurveRightAngle, "menu.line_curve_right_angle"),
];

/// Commands of the model tree's context menu.
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = umlcraft, no_json)]
pub struct TreeCommand {
    pub target: Id,
    pub command: TreeCommandKind,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TreeCommandKind {
    New(NewItem),
    Open,
    Rename,
    Delete,
}

fn main() {
    let settings = settings::Settings::load();
    rust_i18n::set_locale(&settings.ui_language());

    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    // Files the system asks to open while UmlCraft runs (on macOS also at launch, instead of
    // the command line), picked up by the main view.
    let requested: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    let sink = requested.clone();
    app.on_open_urls(move |urls| {
        sink.borrow_mut().extend(urls.iter().filter_map(|u| file_path(u)))
    });
    app.run(move |cx| {
        gpui_kit::init(cx);
        // "secondary" is Cmd on macOS and Ctrl elsewhere.
        cx.bind_keys([
            KeyBinding::new("secondary-n", NewProject, None),
            KeyBinding::new("secondary-o", OpenProject, None),
            KeyBinding::new("secondary-s", SaveProject, None),
            KeyBinding::new("secondary-shift-s", SaveProjectAs, None),
            KeyBinding::new("secondary-z", Undo, None),
            KeyBinding::new("secondary-y", Redo, None),
            KeyBinding::new("secondary-shift-z", Redo, None),
            // Fields bind only Ctrl+Y for their own redo there; without this, Ctrl+Shift+Z in a
            // field would redo a model change and reload the field being typed in.
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("ctrl-shift-z", gpui_kit::component::input::Redo, Some("Input")),
            KeyBinding::new("delete", DeleteSelection, Some("Canvas")),
            KeyBinding::new("secondary-delete", RemoveFromDiagram, Some("Canvas")),
            KeyBinding::new("secondary-a", SelectAll, Some("Canvas")),
            KeyBinding::new("secondary-c", Copy, Some("Canvas")),
            KeyBinding::new("secondary-x", Cut, Some("Canvas")),
            KeyBinding::new("secondary-v", Paste, Some("Canvas")),
            KeyBinding::new("secondary-shift-v", PasteAsReference, Some("Canvas")),
            KeyBinding::new("escape", Cancel, Some("Canvas")),
            KeyBinding::new("secondary-g", WrapInFragment, Some("Canvas")),
            // Astah's shortcuts for the line styles.
            KeyBinding::new("secondary-l", SetLineShape { shape: LineShape::Line }, Some("Canvas")),
            KeyBinding::new(
                "secondary-e",
                SetLineShape { shape: LineShape::RightAngle },
                Some("Canvas"),
            ),
            KeyBinding::new("secondary-0", ResetZoom, None),
            KeyBinding::new("secondary-=", ZoomIn, None),
            KeyBinding::new("secondary--", ZoomOut, None),
            KeyBinding::new("secondary-shift-0", ZoomToFit, None),
            KeyBinding::new("secondary-,", OpenSettings, None),
        ]);
        nsd::bind_keys(cx);

        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("UmlCraft".into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(800.0), px(520.0))),
                // Matches packaging/linux/com.umlcraft.app.desktop, so that Wayland
                // desktops show the window with the app's icon.
                app_id: Some("com.umlcraft.app".into()),
                ..Default::default()
            },
            cx,
            move |window, cx| {
                cx.new(|cx| {
                    let mut view = main_view::MainView::new(settings, window, cx);
                    // A file given on the command line, e.g. by "Open with".
                    if let Some(path) = std::env::args_os().nth(1) {
                        view.open(path.into(), window, cx);
                    }
                    cx.spawn_in(window, async move |this, cx| {
                        loop {
                            cx.background_executor().timer(Duration::from_millis(250)).await;
                            let Some(path) = requested.borrow_mut().pop() else { continue };
                            let opened = this.update_in(cx, |view, window, cx| {
                                view.ask_to_open(path, window, cx)
                            });
                            if opened.is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                    view
                })
            },
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}

/// The local path of a `file://` URL, with `%XX` escapes decoded.
fn file_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        ) {
            (b'%', Some(h), Some(l)) => {
                decoded.push((h * 16 + l) as u8);
                i += 3;
            }
            (b, _, _) => {
                decoded.push(b);
                i += 1;
            }
        }
    }
    Some(PathBuf::from(String::from_utf8(decoded).ok()?))
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_urls_become_paths() {
        use super::file_path;
        use std::path::PathBuf;

        assert_eq!(
            file_path("file:///Users/ulf/My%20Shop.ucraft"),
            Some(PathBuf::from("/Users/ulf/My Shop.ucraft"))
        );
        assert_eq!(
            file_path("file:///tmp/%C3%9Cbung.asta"),
            Some(PathBuf::from("/tmp/Übung.asta"))
        );
        assert_eq!(file_path("https://example.com/a.ucraft"), None);
    }
}
