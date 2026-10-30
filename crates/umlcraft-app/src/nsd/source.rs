//! File › Structogram from source file …: choose a `.java`, `.cs` or `.py` file, then one or
//! more of its methods in a dialog (grouped by class, with a search field); each opens as an
//! unsaved structogram whose file name is suggested as `Class.method.nsd` (`file.function.nsd`
//! outside classes) next to the source file.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Sizable, StyledExt, WindowExt, v_flex};
use gpui_kit::*;
use rust_i18n::t;
use umlcraft_nsd_code::{Language, Made, MethodRef, NoteKind, Report, Source};

/// More windows than this at once would rather be a mistake.
const MAX_WINDOWS: usize = 20;

/// Asks for a Java, C# or Python file, reads it in the background and offers its methods.
/// Errors come back to `failed` as message and detail.
pub fn choose(
    window: &mut Window,
    cx: &mut App,
    failed: impl Fn(String, String, &mut Window, &mut App) + 'static,
) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: None,
    });
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let Some(language) = Language::of(&path) else {
                cx.update(|window, cx| {
                    let message = t!("nsd.source_unknown").to_string();
                    failed(message, path.display().to_string(), window, cx);
                })
                .ok();
                return;
            };
            let read_path = path.clone();
            let parsed = cx
                .background_executor()
                .spawn(async move {
                    let bytes = std::fs::read(&read_path)?;
                    let source = Source::parse(language, &umlcraft_nsd_code::decode(&bytes));
                    let methods = source.methods();
                    std::io::Result::Ok((source, methods))
                })
                .await;
            cx.update(|window, cx| match parsed {
                Ok((_, methods)) if methods.is_empty() => failed(
                    t!("nsd.source_no_methods").to_string(),
                    path.display().to_string(),
                    window,
                    cx,
                ),
                Ok((source, methods)) => offer(path, Rc::new(source), methods, window, cx),
                Err(err) => {
                    failed(t!("nsd.source_failed").to_string(), err.to_string(), window, cx)
                }
            })
            .ok();
        })
        .detach();
}

/// The dialog listing the methods.
fn offer(
    path: PathBuf,
    source: Rc<Source>,
    methods: Vec<MethodRef>,
    window: &mut Window,
    cx: &mut App,
) {
    if window.has_active_dialog(cx) {
        return;
    }
    let search = cx.new(|cx| InputState::new(window, cx).placeholder(t!("nsd.source_search")));
    search.update(cx, |s, cx| s.focus(window, cx));
    let chosen: Rc<RefCell<BTreeSet<usize>>> = Rc::default();
    let methods = Rc::new(methods);
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let path = Rc::new(path);

    window.open_dialog(cx, move |dialog, _, cx| {
        let filter = search.read(cx).value().to_lowercase();
        let muted = cx.theme().muted_foreground;
        let mut list = v_flex().gap_0p5();
        let mut class: Option<String> = None;
        for (i, m) in methods.iter().enumerate() {
            let label = format!("{} {}", m.class_name(), m.signature).to_lowercase();
            if !filter.is_empty() && !label.contains(&filter) {
                continue;
            }
            if class.as_deref() != Some(m.class_name().as_str()) {
                class = Some(m.class_name());
                // Python functions of the module stand under the file's name.
                let heading = match m.class.is_empty() {
                    true => file_name.clone(),
                    false => m.class_name(),
                };
                list = list.child(div().pt_2().text_sm().font_semibold().child(heading));
            }
            let checked = chosen.borrow().contains(&i);
            let (toggle, open) = (chosen.clone(), opener(&source, &methods, &path, vec![i]));
            list = list.child(
                div()
                    .id(("nsd-source-method", i))
                    .pl_3()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(Checkbox::new(("nsd-source-check", i)).checked(checked).on_click(
                        move |&on, window, _| {
                            let mut set = toggle.borrow_mut();
                            if on {
                                set.insert(i);
                            } else {
                                set.remove(&i);
                            }
                            window.refresh();
                        },
                    ))
                    .child(div().text_sm().child(m.signature.clone()))
                    .child(div().text_xs().text_color(muted).child(format!("{}", m.line)))
                    .on_click(move |ev, window, cx| {
                        if ev.click_count() >= 2 {
                            open(window, cx);
                        }
                    }),
            );
        }
        let ok = {
            let (chosen, source, methods, path) =
                (chosen.clone(), source.clone(), methods.clone(), path.clone());
            move |window: &mut Window, cx: &mut App| {
                let picked: Vec<usize> = chosen.borrow().iter().copied().collect();
                if !picked.is_empty() {
                    opener(&source, &methods, &path, picked)(window, cx);
                }
            }
        };
        let ok_click = ok.clone();
        dialog
            .title(t!("nsd.source_title", file = file_name).to_string())
            .width(px(560.0))
            .child(
                v_flex()
                    .gap_2()
                    .child(Input::new(&search).small())
                    .child(
                        div().text_xs().text_color(muted).child(t!("nsd.source_hint").to_string()),
                    )
                    .child(
                        div()
                            .id("nsd-source-methods")
                            .max_h(px(380.0))
                            .overflow_y_scroll()
                            .child(list),
                    ),
            )
            .on_ok(move |_, window, cx| {
                ok(window, cx);
                false
            })
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("nsd-source-cancel")
                            .label(t!("dialog.cancel").to_string())
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("nsd-source-open")
                            .primary()
                            .label(t!("nsd.source_open").to_string())
                            .on_click(move |_, window, cx| ok_click(window, cx)),
                    ),
            )
    });
}

/// Opens the methods `picked` (at most [`MAX_WINDOWS`]) and closes the dialog.
fn opener(
    source: &Rc<Source>,
    methods: &Rc<Vec<MethodRef>>,
    path: &Rc<PathBuf>,
    picked: Vec<usize>,
) -> impl Fn(&mut Window, &mut App) + Clone + 'static {
    let (source, methods, path) = (source.clone(), methods.clone(), path.clone());
    move |window, cx| {
        window.close_dialog(cx);
        let file_name =
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let language = source.language().name();
        let origin = format!("{} / {language}: {file_name}", super::origin());
        let (author, date) = (super::user_name(), super::today());
        let made = Made { author: &author, date: &date, origin: &origin };
        let directory = path.parent().map(PathBuf::from).unwrap_or_default();
        for &i in picked.iter().take(MAX_WINDOWS) {
            let m = &methods[i];
            let Some((diagram, report)) = source.diagram(m, &made) else { continue };
            let draft = match m.class.is_empty() {
                // `shop.area.nsd` for the function `area` of `shop.py`.
                true => {
                    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
                    directory.join(format!("{}.{}", stem.unwrap_or_default(), m.file_name()))
                }
                false => directory.join(m.file_name()),
            };
            if let Err(err) = super::open_imported(diagram, draft, notes(&report), cx) {
                eprintln!("structogram window: {err}");
            }
        }
        if picked.len() > MAX_WINDOWS {
            let message = t!("nsd.source_too_many", max = MAX_WINDOWS).to_string();
            let rx =
                window.prompt(PromptLevel::Info, &message, None, &[t!("dialog.ok").as_ref()], cx);
            cx.spawn(async move |_| rx.await.ok()).detach();
        }
    }
}

/// The report in the UI's language, one line per note.
pub fn notes(report: &Report) -> Vec<String> {
    report
        .notes
        .iter()
        .map(|n| {
            let what = match n.kind {
                NoteKind::Continue => t!("nsd.note_continue"),
                NoteKind::Label => t!("nsd.note_label"),
                NoteKind::LocalClass => t!("nsd.note_local_class"),
                NoteKind::Synchronized => t!("nsd.note_synchronized"),
                NoteKind::Fallthrough => t!("nsd.note_fallthrough"),
                NoteKind::SyntaxError => t!("nsd.note_syntax_error"),
                NoteKind::Using => t!("nsd.note_using"),
                NoteKind::Lock => t!("nsd.note_lock"),
                NoteKind::Yield => t!("nsd.note_yield"),
                NoteKind::Goto => t!("nsd.note_goto"),
                NoteKind::LocalFunction => t!("nsd.note_local_function"),
                NoteKind::Conditional => t!("nsd.note_conditional"),
                NoteKind::With => t!("nsd.note_with"),
                NoteKind::LoopElse => t!("nsd.note_loop_else"),
                NoteKind::TryElse => t!("nsd.note_try_else"),
                NoteKind::NestedFunction => t!("nsd.note_nested_function"),
            };
            t!("nsd.note_line", line = n.line, what = what).to_string()
        })
        .collect()
}
