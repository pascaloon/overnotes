//! The shared canvas editor, used by both the overlay (edit mode) and the
//! standalone window.
//!
//! - [`state`] holds the live signals and every command that mutates them.
//! - [`objects`] holds one module per canvas element (note, subgraph, ...).
//! - [`canvas`] and [`chrome`] render the viewport and the surrounding UI.

mod canvas;
mod chrome;
mod geometry;
mod history;
mod interaction;
mod objects;
pub mod state;

pub use interaction::DragState;
pub use state::{EditorHost, EditorState, TransactionKind, ViewMode, handle_history_shortcut};

use dioxus::prelude::*;

use crate::store;

/// How long the document sits unchanged before it is written to disk.
const AUTOSAVE_DELAY_MS: u64 = 400;
/// Polling interval for the monitor the desktop overlay's chrome lives on.
const CHROME_POLL_MS: u64 = 50;
const CHROME_FADE_OUT_MS: u64 = 90;
const CHROME_FADE_IN_MS: u64 = 16;

/// The shared editor surface. Expects an [`EditorState`] in context.
#[component]
pub fn Editor() -> Element {
    let mut state = use_context::<EditorState>();

    use_autosave(state);
    use_desktop_chrome_tracking(state);

    let edit = state.is_edit_mode();
    let shot_active = *state.shot_mode.read();
    let host_class = match state.host {
        EditorHost::Overlay => "overlay",
        EditorHost::Standalone => "standalone",
    };
    let mode_class = if edit { "mode-edit" } else { "mode-overview" };
    let canvas_opacity = canvas_opacity(&state, edit, shot_active);
    let toast = state.toast.read().clone();
    let chrome_bounds = *state.chrome_bounds.read();
    let chrome_fading = *state.chrome_fading.read();
    let chrome_style = match chrome_bounds {
        Some(b) => format!(
            "left: {}px; top: {}px; width: {}px; height: {}px;",
            b.left, b.top, b.width, b.height
        ),
        None => String::new(),
    };

    rsx! {
        for style in objects::styles() {
            document::Stylesheet { href: style }
        }
        div {
            class: "editor-root {host_class} {mode_class}",
            onkeydown: move |evt| {
                handle_history_shortcut(&evt, &mut state);
            },
            div {
                class: "editor-canvas-layer",
                style: "opacity: {canvas_opacity};",
                canvas::Canvas {}
            }
            if edit && !shot_active {
                div {
                    class: "chrome-stage",
                    class: if chrome_bounds.is_some() { "pinned" },
                    class: if chrome_fading { "fading" },
                    style: "{chrome_style}",
                    chrome::Toolbar {}
                    chrome::Breadcrumbs {}
                    chrome::MainMenu {}
                    if state.host == EditorHost::Overlay {
                        chrome::BottomBar {}
                    }
                    if let Some(msg) = toast.clone() {
                        div { class: "editor-toast", "{msg}" }
                    }
                }
                chrome::ObjectMenu {}
            }
            if shot_active {
                chrome::ShotOverlay {}
            }
            if !edit || shot_active {
                if let Some(msg) = toast {
                    div { class: "editor-toast", "{msg}" }
                }
            }
        }
    }
}

/// How see-through the canvas is: fully opaque while cropping a screenshot or
/// in the standalone window, otherwise driven by the document's settings.
fn canvas_opacity(state: &EditorState, edit: bool, shot_active: bool) -> f64 {
    match state.host {
        EditorHost::Standalone => 1.0,
        EditorHost::Overlay => {
            let doc = state.doc.read();
            if shot_active {
                1.0
            } else if !edit && *state.overview_hidden.read() {
                0.0
            } else if edit {
                doc.edit_opacity
            } else {
                1.0
            }
        }
    }
}

/// Debounced autosave whenever the document changes.
fn use_autosave(state: EditorState) {
    let doc = state.doc;
    let mut save_seq = use_signal(|| 0u64);
    use_effect(move || {
        let snapshot = doc.read().clone();
        let seq = *save_seq.peek() + 1;
        save_seq.set(seq);
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(AUTOSAVE_DELAY_MS)).await;
            if *save_seq.peek() == seq {
                let _ = store::save_document(&snapshot);
            }
        });
    });
}

/// Desktop overlay: keep the chrome on the monitor under the cursor, fading
/// it out and back in while it jumps between monitors.
fn use_desktop_chrome_tracking(mut state: EditorState) {
    let desktop = state.is_desktop_overlay();
    if desktop {
        use_hook(|| {
            let scale = dioxus::desktop::window().scale_factor();
            if let Some(bounds) = crate::platform::display::chrome_bounds_for_cursor(scale) {
                state.chrome_bounds.set(Some(bounds));
            }
        });
    }
    use_future(move || async move {
        if !desktop {
            return;
        }
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(CHROME_POLL_MS)).await;
            if *state.mode.peek() != ViewMode::Edit {
                continue;
            }
            let scale = dioxus::desktop::window().scale_factor();
            let Some(bounds) = crate::platform::display::chrome_bounds_for_cursor(scale) else {
                continue;
            };
            let changed = state.chrome_bounds.peek().as_ref().is_none_or(|prev| {
                (prev.left - bounds.left).abs() > 0.5
                    || (prev.top - bounds.top).abs() > 0.5
                    || (prev.width - bounds.width).abs() > 0.5
                    || (prev.height - bounds.height).abs() > 0.5
            });
            if !changed {
                continue;
            }

            state.chrome_fading.set(true);
            tokio::time::sleep(std::time::Duration::from_millis(CHROME_FADE_OUT_MS)).await;
            let scale = dioxus::desktop::window().scale_factor();
            let latest = crate::platform::display::chrome_bounds_for_cursor(scale);
            state.chrome_bounds.set(Some(latest.unwrap_or(bounds)));
            // Let layout apply the new rect before fading back in.
            tokio::time::sleep(std::time::Duration::from_millis(CHROME_FADE_IN_MS)).await;
            state.chrome_fading.set(false);
        }
    });
}
