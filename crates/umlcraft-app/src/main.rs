#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod canvas;
mod main_view;
mod scene;
mod workspace;

use gpui_kit::*;

rust_i18n::i18n!("../../locales", fallback = "en");

actions!(umlcraft, [Undo, Redo, DeleteSelection, ResetZoom, ToggleTheme, ToggleLanguage]);

fn main() {
    let system = sys_locale::get_locale().unwrap_or_default();
    rust_i18n::set_locale(if system.starts_with("de") { "de" } else { "en" });

    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(|cx| {
        gpui_kit::init(cx);
        cx.bind_keys([
            KeyBinding::new("ctrl-z", Undo, None),
            KeyBinding::new("ctrl-y", Redo, None),
            KeyBinding::new("ctrl-shift-z", Redo, None),
            KeyBinding::new("delete", DeleteSelection, Some("Canvas")),
            KeyBinding::new("ctrl-0", ResetZoom, Some("Canvas")),
        ]);

        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("UmlCraft".into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(720.0), px(480.0))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| main_view::MainView::new(window, cx)),
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}
