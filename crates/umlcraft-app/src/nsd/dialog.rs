//! The element editor: one dialog for an element's text, comment and whether it is disabled,
//! and for the diagram's signature, comment and kind. A new element is only inserted when the
//! dialog is confirmed, so cancelling leaves nothing to undo. Like Structorizer's, the dialog
//! selects the first `?` of a default text.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::radio::RadioGroup;
use gpui_kit::component::{ActiveTheme, Sizable, StyledExt, WindowExt, v_flex};
use gpui_kit::*;
use rust_i18n::t;
use umlcraft_nsd::path::{Path, Slot};
use umlcraft_nsd::{ElementKind, RootKind};

use super::kind_name;
use super::view::NsdView;

/// What the dialog edits.
#[derive(Clone, Debug)]
pub enum Target {
    Element(Path),
    /// A new element of `kind`, inserted at `slot` on OK.
    New(Slot, ElementKind),
    Root,
}

/// What the dialog hands back on OK.
pub struct Edited {
    pub text: Vec<String>,
    pub comment: Vec<String>,
    pub disabled: bool,
    pub kind: RootKind,
}

const ROOT_KINDS: [(RootKind, &str); 3] = [
    (RootKind::Program, "nsd.root_program"),
    (RootKind::Sub, "nsd.root_sub"),
    (RootKind::Includable, "nsd.root_includable"),
];

/// Opens the dialog for `target` (an element of `element_kind`, or the diagram for `None`)
/// with the values it starts from. `focus` is the structogram's, which gets the focus back.
pub fn open(
    view: WeakEntity<NsdView>,
    focus: &FocusHandle,
    target: Target,
    element_kind: Option<ElementKind>,
    start: Edited,
    window: &mut Window,
    cx: &mut App,
) {
    if window.has_active_dialog(cx) {
        return;
    }
    let has_text = element_kind != Some(ElementKind::Forever);
    let text_value = start.text.join("\n");
    // A new element's default text (ASCII) has its first gap selected, ready for typing.
    let question = matches!(target, Target::New(..)).then(|| text_value.find('?')).flatten();
    let text = cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 12));
    let comment = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 8));
    text.update(cx, |s, cx| s.set_value(text_value, window, cx));
    comment.update(cx, |s, cx| s.set_value(start.comment.join("\n"), window, cx));
    // Closing gives the focus back to what had it on opening. That must be the structogram
    // (`focus`), not the toolbar button that opened the dialog nor the dialog's own fields,
    // which are gone then and would leave keys and toolbar without a target. The fields get
    // the focus once the dialog is open.
    window.focus(focus, cx);
    let (first_text, first_comment) = (text.clone(), comment.clone());
    let disabled = Rc::new(Cell::new(start.disabled));
    let kind = Rc::new(Cell::new(start.kind));

    let title = match (&target, element_kind) {
        (Target::Root, _) => t!("nsd.edit_diagram").to_string(),
        (_, Some(k)) => kind_name(k),
        _ => String::new(),
    };
    let (text_label, hint) = match element_kind {
        None => (t!("nsd.signature"), None),
        Some(ElementKind::Case) => (t!("nsd.text"), Some(t!("nsd.case_hint"))),
        Some(ElementKind::Parallel) => (t!("nsd.branches"), None),
        Some(ElementKind::Try) => (t!("nsd.exception"), None),
        Some(ElementKind::For) => (t!("nsd.text"), Some(t!("nsd.for_hint"))),
        Some(_) => (t!("nsd.text"), None),
    };

    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        let ok = {
            let (view, target, text, comment) =
                (view.clone(), target.clone(), text.clone(), comment.clone());
            let (disabled, kind) = (disabled.clone(), kind.clone());
            move |window: &mut Window, cx: &mut App| {
                let lines = |s: &Entity<TextareaState>, cx: &App| -> Vec<String> {
                    let v = s.read(cx).value().to_string();
                    let v = v.replace("\r\n", "\n");
                    let v = v.trim_end_matches('\n');
                    v.split('\n').map(str::to_owned).collect()
                };
                let edited = Edited {
                    text: lines(&text, cx),
                    comment: lines(&comment, cx),
                    disabled: disabled.get(),
                    kind: kind.get(),
                };
                let target = target.clone();
                view.update(cx, |v, cx| v.apply_dialog(target, edited, window, cx)).ok();
                window.close_dialog(cx);
            }
        };
        let ok_click = ok.clone();

        let mut body = v_flex().gap_3().pb_1();
        if has_text {
            let mut field = v_flex()
                .gap_1()
                .child(div().text_sm().font_semibold().child(text_label.to_string()))
                .child(Textarea::new(&text).small());
            if let Some(hint) = &hint {
                field = field.child(div().text_xs().text_color(muted).child(hint.to_string()));
            }
            body = body.child(field);
        }
        body = body.child(
            v_flex()
                .gap_1()
                .child(div().text_sm().font_semibold().child(t!("nsd.comment").to_string()))
                .child(Textarea::new(&comment).small()),
        );
        if matches!(target, Target::Root) {
            let current = ROOT_KINDS.iter().position(|(k, _)| *k == kind.get());
            let kind = kind.clone();
            body = body.child(
                RadioGroup::horizontal("nsd-root-kind")
                    .children(ROOT_KINDS.map(|(_, key)| t!(key).to_string()))
                    .selected_index(current)
                    .on_change(move |&ix, window, _| {
                        kind.set(ROOT_KINDS[ix].0);
                        window.refresh();
                    }),
            );
        } else {
            let disabled = disabled.clone();
            body = body.child(
                Checkbox::new("nsd-disabled")
                    .label(t!("nsd.disabled").to_string())
                    .checked(disabled.get())
                    .on_click(move |&on, window, _| {
                        disabled.set(on);
                        window.refresh();
                    }),
            );
        }

        dialog
            .title(title.clone())
            .width(px(480.0))
            .child(body)
            .on_ok(move |_, window, cx| {
                ok(window, cx);
                false
            })
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("nsd-dialog-cancel")
                            .label(t!("dialog.cancel").to_string())
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("nsd-dialog-ok")
                            .primary()
                            .label(t!("dialog.ok").to_string())
                            .on_click(move |_, window, cx| ok_click(window, cx)),
                    ),
            )
    });
    if has_text {
        first_text.update(cx, |s, cx| {
            if let Some(at) = question {
                s.set_selected_range(at..at + 1, cx);
            }
            s.focus(window, cx);
        });
    } else {
        first_comment.update(cx, |s, cx| s.focus(window, cx));
    }
}
