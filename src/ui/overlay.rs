//! The overlay window (game-attached or desktop-wide).
//!
//! Hosts the shared editor. Two modes:
//! - Overview: click-through, no chrome, overview opacity. The global
//!   edit-mode shortcut switches to edit mode.
//! - Edit: fully interactive, editing opacity.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::desktop::tao::platform::windows::WindowExtWindows;
use dioxus::desktop::{HotKeyState, ShortcutHandle};
use dioxus::prelude::*;
use global_hotkey::hotkey::HotKey;

use crate::editor::{Editor, EditorHost, EditorState, ViewMode};
use crate::platform::{overlay_style, process, tracker};
use crate::store;

#[component]
pub fn OverlayRoot(game_hwnd: isize, game_exe: String, doc_id: String) -> Element {
    let desktop = process::is_desktop_exe(&game_exe) || game_hwnd == 0;
    let state = use_context_provider(|| {
        let doc = store::load_document(&game_exe, &doc_id)
            .unwrap_or_else(|| store::Document::new(&game_exe, "Untitled"));
        let hwnd = if desktop { None } else { Some(game_hwnd) };
        EditorState::create(EditorHost::Overlay, hwnd, doc)
    });

    // One-time window setup: extended styles + tracker thread.
    let setup = use_hook(|| {
        let win = dioxus::desktop::window();
        let overlay_hwnd = win.window.hwnd();
        overlay_style::apply_overlay_base(overlay_hwnd);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<tracker::GameEvent>();
        let handle = if desktop {
            tracker::start_desktop_tracking(overlay_hwnd, tx)
        } else {
            tracker::start_tracking(game_hwnd, overlay_hwnd, tx)
        };
        Rc::new((handle, RefCell::new(Some(rx)), overlay_hwnd, desktop))
    });
    let overlay_hwnd = setup.2;
    let is_desktop = setup.3;

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

    // Apply mode side effects: click-through + focus policy.
    let mode = state.mode;
    use_effect(move || {
        let current = *mode.read();
        let win = dioxus::desktop::window();
        match current {
            ViewMode::Edit => {
                let _ = win.set_ignore_cursor_events(false);
                // tao rewrites GWL_EXSTYLE wholesale here, dropping our
                // TOOLWINDOW bit - re-apply it.
                overlay_style::apply_overlay_base(overlay_hwnd);
                overlay_style::set_noactivate(overlay_hwnd, false);
                overlay_style::focus_window(overlay_hwnd);
                win.set_focus();
            }
            ViewMode::Overview => {
                let _ = win.set_ignore_cursor_events(true);
                overlay_style::apply_overlay_base(overlay_hwnd);
                overlay_style::set_noactivate(overlay_hwnd, true);
                // Hand focus back to the game when there is one.
                if !is_desktop {
                    overlay_style::focus_window(game_hwnd);
                }
            }
        }
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/style.css") }
        OverlayShortcut {
            action: OverlayShortcutAction::ToggleEditMode,
        }
        if !is_desktop {
            OverlayShortcut {
                action: OverlayShortcutAction::Screenshot,
            }
        }
        Editor {}
    }
}

#[derive(Clone, Copy, PartialEq)]
enum OverlayShortcutAction {
    ToggleEditMode,
    Screenshot,
}

#[component]
fn OverlayShortcut(action: OverlayShortcutAction) -> Element {
    let state = use_context::<EditorState>();
    let mut registered = use_signal(|| None::<(String, ShortcutHandle)>);
    let shortcut_handler = use_callback(move |hk_state: HotKeyState| {
        if hk_state != HotKeyState::Pressed {
            return;
        }
        let mut state = state;
        match action {
            OverlayShortcutAction::ToggleEditMode => toggle_edit_mode(state),
            OverlayShortcutAction::Screenshot => state.start_region_screenshot(),
        }
    });

    use_effect(move || {
        let accelerator = {
            let settings = state.settings.read();
            match action {
                OverlayShortcutAction::ToggleEditMode => {
                    settings.overlay_toggle_shortcut.accelerator.clone()
                }
                OverlayShortcutAction::Screenshot => {
                    settings.overlay_screenshot_shortcut.accelerator.clone()
                }
            }
        };

        if registered
            .peek()
            .as_ref()
            .is_some_and(|(current, _)| current == &accelerator)
        {
            return;
        }
        if let Some((_, handle)) = registered.write().take() {
            handle.remove();
        }

        let Ok(hotkey) = accelerator.parse::<HotKey>() else {
            let mut state = state;
            state.show_toast("Invalid overlay shortcut");
            return;
        };

        match dioxus::desktop::window().create_shortcut(hotkey, move |hk_state| {
            shortcut_handler.call(hk_state);
        }) {
            Ok(handle) => registered.set(Some((accelerator.clone(), handle))),
            Err(_) => {
                let mut state = state;
                state.show_toast("Could not register overlay shortcut");
                return;
            }
        }
    });

    use_drop(move || {
        if let Some((_, handle)) = registered.write().take() {
            handle.remove();
        }
    });

    rsx! {}
}

fn toggle_edit_mode(mut state: EditorState) {
    let current = *state.mode.peek();
    let next = match current {
        ViewMode::Overview => ViewMode::Edit,
        ViewMode::Edit => ViewMode::Overview,
    };
    if next == ViewMode::Overview {
        state.return_to_overview();
    } else {
        state.mode.set(next);
    }
}
