//! Launcher window: pick a running game, pick (or create) a document, then
//! open it as an in-game overlay or as a standalone window.

use dioxus::desktop::WeakDesktopContext;
use dioxus::prelude::*;

use crate::platform::{process, tracker};
use crate::store::{self, DocMeta};
use crate::ui;

#[component]
pub fn Launcher() -> Element {
    let mut processes = use_signal(process::list_game_windows);
    let mut selected_game = use_signal(|| None::<process::GameWindow>);
    let mut docs = use_signal(Vec::<DocMeta>::new);
    let mut selected_doc = use_signal(|| None::<String>);
    let mut new_doc_name = use_signal(String::new);
    let mut overlay_ctx = use_signal(|| None::<WeakDesktopContext>);
    let mut attached_to = use_signal(|| None::<(String, String, String)>); // (game title, exe, doc id)
    let mut status = use_signal(String::new);
    // Doc id awaiting a second delete click for confirmation.
    let mut confirm_delete = use_signal(|| None::<String>);

    // Poll whether the overlay window is still alive so the "Attached"
    // indicator and Detach button stay truthful.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            let gone = {
                let guard = overlay_ctx.peek();
                match guard.as_ref() {
                    Some(weak) => weak.upgrade().is_none(),
                    None => false,
                }
            };
            if gone {
                overlay_ctx.set(None);
                attached_to.set(None);
            }
        }
    });

    let mut refresh_docs = move |game: &process::GameWindow| {
        docs.set(store::list_documents(&game.exe));
        selected_doc.set(None);
        confirm_delete.set(None);
    };

    let mut delete_doc = move |doc_id: String| {
        let Some(game) = selected_game.peek().clone() else {
            return;
        };
        if attached_to
            .peek()
            .as_ref()
            .is_some_and(|(_, _, id)| id == &doc_id)
        {
            status.set("Detach the overlay before deleting this document".into());
            confirm_delete.set(None);
            return;
        }
        if confirm_delete.peek().as_deref() != Some(doc_id.as_str()) {
            confirm_delete.set(Some(doc_id));
            status.set("Click delete again to confirm".into());
            return;
        }
        match store::delete_document(&game.exe, &doc_id) {
            Ok(()) => {
                if selected_doc.peek().as_deref() == Some(doc_id.as_str()) {
                    selected_doc.set(None);
                }
                confirm_delete.set(None);
                docs.set(store::list_documents(&game.exe));
                status.set("Document deleted".into());
            }
            Err(e) => {
                confirm_delete.set(None);
                status.set(format!("Failed to delete document: {e}"));
            }
        }
    };

    // Resolve the document to open: the selected one, or a new one named
    // after the input (falling back to "Untitled").
    let mut ensure_doc = move || -> Option<(String, String)> {
        let game = selected_game.peek().clone()?;
        if let Some(id) = selected_doc.peek().clone() {
            return Some((game.exe.clone(), id));
        }
        let name = {
            let n = new_doc_name.peek().trim().to_string();
            if n.is_empty() {
                "Untitled".to_string()
            } else {
                n
            }
        };
        let doc = store::Document::new(&game.exe, &name);
        if let Err(e) = store::save_document(&doc) {
            status.set(format!("Failed to create document: {e}"));
            return None;
        }
        let id = doc.id.clone();
        docs.set(store::list_documents(&game.exe));
        selected_doc.set(Some(id.clone()));
        new_doc_name.set(String::new());
        Some((game.exe, id))
    };

    let mut close_overlay = move || {
        if let Some(weak) = overlay_ctx.peek().clone() {
            if let Some(ctx) = weak.upgrade() {
                ctx.close();
            }
        }
        overlay_ctx.set(None);
        attached_to.set(None);
    };

    let open_standalone = move |game_exe: String, doc_id: String| {
        let doc_name = store::load_document(&game_exe, &doc_id)
            .map(|d| d.name)
            .unwrap_or_else(|| "Untitled".into());
        let dom = VirtualDom::new_with_props(
            ui::standalone::StandaloneRoot,
            ui::standalone::StandaloneRootProps { game_exe, doc_id },
        );
        let _ = dioxus::desktop::window().new_window(dom, ui::standalone_config(&doc_name));
    };

    let open_window = move |_| {
        if let Some((game_exe, doc_id)) = ensure_doc() {
            open_standalone(game_exe, doc_id);
            status.set("Opened standalone window".into());
        }
    };

    let open_overlay = move |_| {
        let Some(game) = selected_game.peek().clone() else {
            return;
        };
        let Some((game_exe, doc_id)) = ensure_doc() else {
            return;
        };
        let desktop = process::is_desktop(&game);
        let rect = if desktop {
            tracker::virtual_screen_rect()
        } else {
            let Some(rect) = tracker::client_rect_on_screen(game.hwnd) else {
                status.set("Game window is gone - refresh the list".into());
                processes.set(process::list_game_windows());
                return;
            };
            rect
        };
        // Only one overlay at a time.
        close_overlay();

        let dom = VirtualDom::new_with_props(
            ui::overlay::OverlayRoot,
            ui::overlay::OverlayRootProps {
                game_hwnd: game.hwnd,
                game_exe: game_exe.clone(),
                doc_id: doc_id.clone(),
            },
        );
        let pending = dioxus::desktop::window().new_window(dom, ui::overlay_config(rect));
        spawn(async move {
            if let Ok(ctx) = pending.try_resolve().await {
                overlay_ctx.set(Some(std::rc::Rc::downgrade(&ctx)));
                attached_to.set(Some((game.title.clone(), game_exe, doc_id)));
                let settings = store::load_settings();
                status.set(if desktop {
                    format!(
                        "Desktop overlay - {} toggles overview/edit",
                        settings.overlay_toggle_shortcut.label
                    )
                } else {
                    format!(
                        "Overlay attached - {} toggles overview/edit, {} captures to crop",
                        settings.overlay_toggle_shortcut.label,
                        settings.overlay_screenshot_shortcut.label
                    )
                });
            }
        });
    };

    let detach = move |_| {
        let Some((_, game_exe, doc_id)) = attached_to.peek().clone() else {
            return;
        };
        close_overlay();
        open_standalone(game_exe, doc_id);
        status.set("Overlay detached - editing in standalone window".into());
    };

    let games = processes.read().clone();
    let selected = selected_game.read().clone();
    let selected_hwnd = selected.as_ref().map(|g| g.hwnd);
    let selected_is_desktop = selected.as_ref().is_some_and(process::is_desktop);
    let doc_list = docs.read().clone();
    let selected_doc_id = selected_doc.read().clone();
    let has_game = selected.is_some();
    let attached = attached_to.read().clone();
    let status_text = status.read().clone();
    let new_name = new_doc_name.read().clone();
    let pending_delete = confirm_delete.read().clone();

    rsx! {
        document::Stylesheet { href: asset!("/assets/style.css") }
        div { class: "launcher",
            div { class: "launcher-header",
                div { class: "logo", "O" }
                div {
                    h1 { "Overnotes" }
                    div { class: "sub", "Pin notes over your game" }
                }
            }

            div { class: "launcher-body",
                // -------- game processes --------
                div { class: "launcher-col",
                    div { class: "col-head",
                        span { "Targets" }
                        button {
                            class: "icon-btn",
                            class: "has-tooltip",
                            aria_label: "Refresh",
                            onclick: move |_| {
                                processes.set(process::list_game_windows());
                            },
                            svg { width: "16", height: "16", view_box: "0 0 24 24", fill: "none",
                                stroke: "currentColor", stroke_width: "2", stroke_linecap: "round",
                                path { d: "M20 11 A8 8 0 1 0 18.6 15.5 M20 5 V11 H14" }
                            }
                        }
                    }
                    div { class: "col-list",
                        for game in games {
                            div {
                                class: "list-item",
                                class: if selected_hwnd == Some(game.hwnd)
                                    && selected_is_desktop == process::is_desktop(&game)
                                {
                                    "selected"
                                },
                                class: if process::is_desktop(&game) { "desktop-item" },
                                onclick: {
                                    let game = game.clone();
                                    move |_| {
                                        refresh_docs(&game);
                                        selected_game.set(Some(game.clone()));
                                    }
                                },
                                span { class: "title", "{game.title}" }
                                if !process::is_desktop(&game) {
                                    span { class: "meta", "{game.exe} - pid {game.pid}" }
                                } else {
                                    span { class: "meta", "All monitors - not tied to a process" }
                                }
                            }
                        }
                    }
                }

                // -------- documents --------
                div { class: "launcher-col",
                    div { class: "col-head",
                        span { "Documents" }
                    }
                    div { class: "col-list",
                        if !has_game {
                            div { class: "list-empty", "Select a game to see its documents." }
                        } else if doc_list.is_empty() {
                            div { class: "list-empty", "No documents yet - create one below." }
                        }
                        for meta in doc_list {
                            div {
                                class: "list-item doc-item",
                                class: if selected_doc_id.as_deref() == Some(meta.id.as_str()) { "selected" },
                                class: if pending_delete.as_deref() == Some(meta.id.as_str()) { "confirm-delete" },
                                onclick: {
                                    let id = meta.id.clone();
                                    move |_| {
                                        confirm_delete.set(None);
                                        selected_doc.set(Some(id.clone()));
                                    }
                                },
                                span { class: "title", "{meta.name}" }
                                button {
                                    class: "icon-btn delete-btn",
                                    class: "has-tooltip",
                                    class: if pending_delete.as_deref() == Some(meta.id.as_str()) { "confirm" },
                                    aria_label: if pending_delete.as_deref() == Some(meta.id.as_str()) {
                                        "Confirm delete"
                                    } else {
                                        "Delete document"
                                    },
                                    onclick: {
                                        let id = meta.id.clone();
                                        move |evt| {
                                            evt.stop_propagation();
                                            delete_doc(id.clone());
                                        }
                                    },
                                    svg {
                                        width: "15",
                                        height: "15",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        path { d: "M3 6h18" }
                                        path { d: "M8 6V4h8v2" }
                                        path { d: "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" }
                                        path { d: "M10 11v6" }
                                        path { d: "M14 11v6" }
                                    }
                                }
                            }
                        }
                    }
                    if has_game {
                        div { class: "new-doc-row",
                            input {
                                r#type: "text",
                                placeholder: "New document name...",
                                value: "{new_name}",
                                oninput: move |evt| new_doc_name.set(evt.value()),
                            }
                            button {
                                class: "btn ghost",
                                onclick: move |_| {
                                    selected_doc.set(None);
                                    let _ = ensure_doc();
                                },
                                "Create"
                            }
                        }
                    }
                }
            }

            div { class: "launcher-footer",
                div { class: "launcher-status",
                    if let Some((title, _, _)) = attached.as_ref() {
                        span { class: "attached", "Attached to {title}" }
                        span { " - {status_text}" }
                    } else {
                        span { "{status_text}" }
                    }
                }
                if attached.is_some() {
                    button { class: "btn", onclick: detach, "Detach overlay" }
                }
                button {
                    class: "btn",
                    disabled: !has_game,
                    onclick: open_window,
                    "Open Window"
                }
                button {
                    class: "btn primary",
                    disabled: !has_game,
                    onclick: open_overlay,
                    "Open Overlay"
                }
            }
        }
    }
}
