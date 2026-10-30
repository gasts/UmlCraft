//! Structograms (Nassi-Shneiderman diagrams in Structorizer's `.nsd` format) in windows of
//! their own. They are not part of the UML project; each file opens its own window, and a file
//! that is open already brings its window to the front.

mod view;

use std::path::{Path, PathBuf};

use gpui_kit::*;
use rust_i18n::t;

use view::NsdView;

/// The windows of the structograms that are open.
#[derive(Default)]
struct Structograms(Vec<(PathBuf, AnyWindowHandle)>);

impl Global for Structograms {}

/// Whether a file is a structogram, by its extension.
pub fn is_structogram(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nsd"))
}

/// Opens `path` in a window of its own, or brings its window to the front. The error is the
/// reason in the UI's language.
pub fn open(path: &Path, cx: &mut App) -> Result<(), String> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if !cx.has_global::<Structograms>() {
        cx.set_global(Structograms::default());
    }
    let open: Vec<(PathBuf, AnyWindowHandle)> = cx.global::<Structograms>().0.clone();
    let mut alive = Vec::new();
    let mut shown = false;
    for (p, handle) in open {
        let front = p == path;
        if handle
            .update(cx, |_, window, _| {
                if front {
                    window.activate_window()
                }
            })
            .is_ok()
        {
            shown |= front;
            alive.push((p, handle));
        }
    }
    cx.global_mut::<Structograms>().0 = alive;
    if shown {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| t!("error.nsd_not_utf8").to_string())?;
    let diagram = umlcraft_nsd::read(&text).map_err(|e| crate::errors::nsd_error(&e))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (w, h) = view::window_size(&diagram);
    let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(format!("{name} – UmlCraft").into()),
            ..Default::default()
        }),
        window_min_size: Some(size(px(360.0), px(240.0))),
        app_id: Some("com.umlcraft.app".into()),
        ..Default::default()
    };
    let window_path = path.clone();
    let (handle, _) = gpui_kit::open_window(options, cx, move |window, cx| {
        cx.new(|cx| NsdView::new(window_path, diagram, window, cx))
    })
    .map_err(|e| e.to_string())?;
    cx.global_mut::<Structograms>().0.push((path, handle));
    Ok(())
}

/// The message for a structogram that could not be opened.
pub fn open_failed() -> String {
    t!("dialog.structogram_open_failed").to_string()
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui_kit brings a `test` attribute of its own.
    use std::path::Path;

    use super::is_structogram;

    #[test]
    fn structograms_go_by_their_extension() {
        assert!(is_structogram(Path::new("sort.nsd")));
        assert!(is_structogram(Path::new(r"C:\Diagramme\SORT.NSD")));
        assert!(!is_structogram(Path::new("shop.ucraft")));
        assert!(!is_structogram(Path::new("shop.asta")));
        assert!(!is_structogram(Path::new("nsd")));
    }
}
