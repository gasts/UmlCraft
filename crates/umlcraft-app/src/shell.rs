//! The window's frame: the backdrop the panels float on, the cards they are drawn as, and
//! buttons that show an icon.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::*;

/// Space between the cards and around them.
pub const GAP: Pixels = px(8.0);
pub const RADIUS: Pixels = px(10.0);

/// The surface the cards float on: a grey under white cards in the light theme; in the dark
/// one the theme's background, with the cards a shade lighter (`card_background`).
///
/// Always opaque. A system backdrop (Mica, the blurred desktop) was tried and dropped: with a
/// non-opaque window GPUI renders text without ClearType, and on Windows its antialiased
/// edges come out too dark over the backdrop.
pub fn backdrop(cx: &App) -> Hsla {
    let theme = cx.theme();
    if theme.is_dark() {
        theme.background
    } else {
        theme.background.blend(theme.foreground.opacity(0.07))
    }
}

pub fn card_background(cx: &App) -> Hsla {
    let theme = cx.theme();
    if theme.is_dark() { theme.background.blend(white().opacity(0.06)) } else { theme.background }
}

/// A rounded panel with a hairline border and a soft shadow: a tight, darker one at the edge
/// and a wide, faint one.
pub fn card(cx: &App) -> Div {
    let theme = cx.theme();
    let shadow = |alpha: f32, y: f32, blur: f32| BoxShadow {
        color: black().opacity(if theme.is_dark() { alpha * 3.0 } else { alpha }),
        offset: point(px(0.0), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.0),
        inset: false,
    };
    div()
        .bg(card_background(cx))
        .rounded(RADIUS)
        .border_1()
        .border_color(theme.border.opacity(0.8))
        .shadow(vec![shadow(0.08, 1.0, 2.0), shadow(0.06, 2.0, 8.0)])
}

/// An icon from the app's assets, e.g. `icons::fluent("save")`.
pub fn icon(path: impl Into<SharedString>) -> Icon {
    Icon::empty().path(path)
}

/// A borderless button with an icon that dispatches `action`; `tooltip` names it.
/// Wraps a button of the title bar. To Windows the whole bar is the caption: a press nobody
/// handles starts moving the window, and the button never sees the release. Handling the press
/// here keeps it in the app; the button below has seen it already.
pub fn in_title_bar(button: impl IntoElement) -> Div {
    div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(button)
}

pub fn icon_button(
    id: impl Into<ElementId>,
    path: impl Into<SharedString>,
    tooltip: impl Into<SharedString>,
    action: Box<dyn Action>,
) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .icon(icon(path))
        .tooltip(tooltip)
        .on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
}
