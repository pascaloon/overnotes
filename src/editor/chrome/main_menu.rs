//! Hamburger menu: document name, document switching, transparency sliders
//! and the overlay shortcuts.

use dioxus::prelude::*;

use super::shortcuts::{ShortcutCapture, ShortcutKind};
use crate::editor::{EditorHost, EditorState, TransactionKind};
use crate::store;

#[component]
pub fn MainMenu() -> Element {
    let mut state = use_context::<EditorState>();
    let open = *state.menu_open.read();

    let (doc_name, game_exe, current_id, overview_opacity, edit_opacity) = {
        let doc = state.doc.read();
        (
            doc.name.clone(),
            doc.game_exe.clone(),
            doc.id.clone(),
            doc.overview_opacity,
            doc.edit_opacity,
        )
    };
    let settings = state.settings.read().clone();
    let show_overlay_shortcuts = state.host == EditorHost::Overlay;

    let docs = if open {
        store::list_documents(&game_exe)
    } else {
        Vec::new()
    };

    rsx! {
        button {
            class: "hamburger has-tooltip",
            aria_label: "Menu",
            onclick: move |_| {
                let now = *state.menu_open.peek();
                state.menu_open.set(!now);
            },
            svg {
                width: "20",
                height: "20",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                path { d: "M4 6 H20 M4 12 H20 M4 18 H20" }
            }
        }

        if open {
            div { class: "menu-panel",
                div { class: "menu-section",
                    span { class: "menu-label", "Document name" }
                    input {
                        r#type: "text",
                        value: "{doc_name}",
                        onfocus: move |_| state.begin_transaction(TransactionKind::DocumentName),
                        oninput: move |evt| {
                            state.begin_transaction(TransactionKind::DocumentName);
                            state.doc.write().name = evt.value();
                        },
                        onblur: move |_| {
                            state.commit_transaction();
                        },
                        onkeydown: move |evt| {
                            if matches!(evt.key(), Key::Enter | Key::Escape) {
                                evt.stop_propagation();
                                state.commit_transaction();
                                state.focus_canvas();
                            }
                        },
                    }
                }

                div { class: "menu-section",
                    span { class: "menu-label", "Load another document" }
                    div { class: "doc-list",
                        if docs.len() <= 1 {
                            div { class: "list-empty", "No other documents for this game" }
                        }
                        for meta in docs.iter().filter(|d| d.id != current_id).cloned() {
                            div {
                                key: "{meta.id}",
                                class: "doc-row",
                                onclick: move |_| {
                                    state.load_document(&meta.id);
                                    state.menu_open.set(false);
                                },
                                "{meta.name}"
                            }
                        }
                    }
                }

                OpacitySlider {
                    label: "Overview transparency",
                    min: 0.1,
                    value: overview_opacity,
                    kind: TransactionKind::OverviewOpacity,
                }

                OpacitySlider {
                    label: "Editing transparency",
                    min: 0.3,
                    value: edit_opacity,
                    kind: TransactionKind::EditOpacity,
                }

                if show_overlay_shortcuts {
                    div { class: "menu-section",
                        span { class: "menu-label", "Overlay shortcuts" }
                        ShortcutCapture {
                            title: "Edit mode",
                            shortcut: settings.overlay_toggle_shortcut.clone(),
                            kind: ShortcutKind::ToggleEditMode,
                        }
                        ShortcutCapture {
                            title: "Screenshot",
                            shortcut: settings.overlay_screenshot_shortcut.clone(),
                            kind: ShortcutKind::Screenshot,
                        }
                    }
                }
            }
        }
    }
}

/// One of the document's two transparency settings.
#[component]
fn OpacitySlider(label: &'static str, min: f64, value: f64, kind: TransactionKind) -> Element {
    let mut state = use_context::<EditorState>();
    let mut apply = move |opacity: f64| match kind {
        TransactionKind::EditOpacity => state.doc.write().edit_opacity = opacity,
        _ => state.doc.write().overview_opacity = opacity,
    };

    rsx! {
        div { class: "menu-section",
            span { class: "menu-label", "{label}" }
            input {
                r#type: "range",
                min: "{min}",
                max: "1",
                step: "0.05",
                value: "{value}",
                onmousedown: move |_| state.begin_transaction(kind),
                oninput: move |evt| {
                    state.begin_transaction(kind);
                    if let Ok(v) = evt.value().parse::<f64>() {
                        apply(v);
                    }
                },
                onmouseup: move |_| {
                    state.commit_transaction();
                },
                onblur: move |_| {
                    state.commit_transaction();
                },
                onchange: move |_| {
                    state.commit_transaction();
                },
            }
            span { class: "slider-value", "{(value * 100.0):.0}%" }
        }
    }
}
