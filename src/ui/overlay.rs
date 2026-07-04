//! The in-game overlay window.
//!
//! Hosts the shared editor over the game's client area. Two modes:
//! - Overview: click-through, no chrome, overview opacity. The global
//!   shortcut (Ctrl+Shift+E) switches to edit mode.
//! - Edit: fully interactive, editing opacity.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::desktop::tao::platform::windows::WindowExtWindows;
use dioxus::desktop::{use_global_shortcut, HotKeyState};
use dioxus::prelude::*;

use crate::editor::{Editor, EditorHost, EditorState, ViewMode};
use crate::platform::{overlay_style, tracker};
use crate::store;

pub const TOGGLE_SHORTCUT: &str = "ctrl+shift+KeyE";
pub const TOGGLE_SHORTCUT_LABEL: &str = "Ctrl+Shift+E";

#[component]
pub fn OverlayRoot(game_hwnd: isize, game_exe: String, doc_id: String) -> Element {
    let state = use_context_provider(|| {
        let doc = store::load_document(&game_exe, &doc_id)
            .unwrap_or_else(|| store::Document::new(&game_exe, "Untitled"));
        EditorState::create(EditorHost::Overlay, Some(game_hwnd), doc)
    });

    // One-time window setup: extended styles + game tracker thread.
    let setup = use_hook(|| {
        let win = dioxus::desktop::window();
        let overlay_hwnd = win.window.hwnd();
        overlay_style::apply_overlay_base(overlay_hwnd);
        overlay_style::set_noactivate(overlay_hwnd, true);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<tracker::GameEvent>();
        let handle = tracker::start_tracking(game_hwnd, overlay_hwnd, tx);
        // Overlay starts in edit mode.
        handle.shield.set_active(true);
        Rc::new((handle, RefCell::new(Some(rx)), overlay_hwnd))
    });
    let overlay_hwnd = setup.2;
    let shield = setup.0.shield;

    // Stop the tracker thread when the overlay goes away.
    {
        let setup = setup.clone();
        use_drop(move || setup.0.stop());
    }

    // React to tracker events (game window destroyed -> close overlay).
    {
        let setup = setup.clone();
        let doc = state.doc;
        use_future(move || {
            let rx = setup.1.borrow_mut().take();
            async move {
                let Some(mut rx) = rx else { return };
                while let Some(event) = rx.recv().await {
                    match event {
                        tracker::GameEvent::GameClosed => {
                            let _ = store::save_document(&doc.peek());
                            dioxus::desktop::window().close();
                        }
                    }
                }
            }
        });
    }

    // Apply mode side effects: click-through, input shield, keep game foreground.
    let mode = state.mode;
    use_effect(move || {
        let current = *mode.read();
        let win = dioxus::desktop::window();
        match current {
            ViewMode::Edit => {
                let _ = win.set_ignore_cursor_events(false);
                shield.set_active(true);
                overlay_style::apply_overlay_base(overlay_hwnd);
                overlay_style::set_noactivate(overlay_hwnd, true);
                overlay_style::focus_window(game_hwnd);
            }
            ViewMode::Overview => {
                let _ = win.set_ignore_cursor_events(true);
                shield.set_active(false);
                overlay_style::apply_overlay_base(overlay_hwnd);
                overlay_style::set_noactivate(overlay_hwnd, true);
                overlay_style::focus_window(game_hwnd);
            }
        }
    });

    // Typing into a note is the one case where the overlay needs keyboard focus.
    let editing_note = state.editing_note;
    use_effect(move || {
        let typing = editing_note.read().is_some();
        if typing {
            overlay_style::set_noactivate(overlay_hwnd, false);
            overlay_style::focus_window(overlay_hwnd);
            dioxus::desktop::window().set_focus();
        } else {
            overlay_style::set_noactivate(overlay_hwnd, true);
            overlay_style::focus_window(game_hwnd);
        }
    });

    // Global shortcut: toggle overview <-> edit.
    let mut toggle_state = state;
    let _ = use_global_shortcut(TOGGLE_SHORTCUT, move |hk_state| {
        if hk_state == HotKeyState::Pressed {
            let current = *toggle_state.mode.peek();
            let next = match current {
                ViewMode::Overview => ViewMode::Edit,
                ViewMode::Edit => ViewMode::Overview,
            };
            if next == ViewMode::Overview {
                toggle_state.deselect();
                toggle_state.menu_open.set(false);
                toggle_state.shot_mode.set(false);
            }
            toggle_state.mode.set(next);
        }
    });

    rsx! {
        document::Style { {super::STYLE} }
        Editor {}
    }
}
