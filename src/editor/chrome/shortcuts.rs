//! Capture control for the two global overlay shortcuts, and the mapping from
//! physical key codes to the accelerator strings the hotkey library expects.

use dioxus::prelude::*;

use crate::editor::EditorState;
use crate::store::{self, KeyboardShortcut};

#[derive(Clone, Copy, PartialEq)]
pub enum ShortcutKind {
    ToggleEditMode,
    Screenshot,
}

#[component]
pub fn ShortcutCapture(
    title: &'static str,
    shortcut: KeyboardShortcut,
    kind: ShortcutKind,
) -> Element {
    let mut state = use_context::<EditorState>();

    rsx! {
        div { class: "shortcut-row",
            span { class: "shortcut-title", "{title}" }
            input {
                class: "shortcut-input",
                r#type: "text",
                readonly: true,
                value: "{shortcut.label}",
                onkeydown: move |evt| {
                    evt.prevent_default();
                    evt.stop_propagation();
                    if is_modifier_code(evt.code()) {
                        return;
                    }
                    let Some(new_shortcut) = shortcut_from_event(&evt) else {
                        state.show_toast("Use Ctrl, Alt, or Win with another key");
                        return;
                    };
                    let mut settings = state.settings.peek().clone();
                    let conflict = match kind {
                        ShortcutKind::ToggleEditMode => {
                            settings.overlay_screenshot_shortcut.accelerator
                                == new_shortcut.accelerator
                        }
                        ShortcutKind::Screenshot => {
                            settings.overlay_toggle_shortcut.accelerator == new_shortcut.accelerator
                        }
                    };
                    if conflict {
                        state.show_toast("That shortcut is already in use");
                        return;
                    }
                    match kind {
                        ShortcutKind::ToggleEditMode => {
                            settings.overlay_toggle_shortcut = new_shortcut;
                        }
                        ShortcutKind::Screenshot => {
                            settings.overlay_screenshot_shortcut = new_shortcut;
                        }
                    }
                    if let Err(e) = store::save_settings(&settings) {
                        state.show_toast(&format!("Could not save shortcut: {e}"));
                        return;
                    }
                    state.settings.set(settings);
                    state.show_toast("Shortcut saved");
                },
            }
        }
    }
}

fn shortcut_from_event(evt: &KeyboardEvent) -> Option<KeyboardShortcut> {
    let mods = evt.modifiers();
    if !(mods.ctrl() || mods.alt() || mods.meta()) {
        return None;
    }

    let (code, key_label) = shortcut_key_parts(evt.code())?;
    let mut accelerator = Vec::new();
    let mut label = Vec::new();
    if mods.ctrl() {
        accelerator.push("ctrl");
        label.push("Ctrl");
    }
    if mods.shift() {
        accelerator.push("shift");
        label.push("Shift");
    }
    if mods.alt() {
        accelerator.push("alt");
        label.push("Alt");
    }
    if mods.meta() {
        accelerator.push("super");
        label.push("Win");
    }
    accelerator.push(code);
    label.push(key_label);

    KeyboardShortcut::new(accelerator.join("+"), label.join("+"))
}

fn is_modifier_code(code: Code) -> bool {
    matches!(
        code,
        Code::AltLeft
            | Code::AltRight
            | Code::ControlLeft
            | Code::ControlRight
            | Code::MetaLeft
            | Code::MetaRight
            | Code::ShiftLeft
            | Code::ShiftRight
    )
}

/// `(accelerator token, human label)` for the keys that can be bound.
fn shortcut_key_parts(code: Code) -> Option<(&'static str, &'static str)> {
    Some(match code {
        Code::Backquote => ("Backquote", "`"),
        Code::Backslash => ("Backslash", "\\"),
        Code::BracketLeft => ("BracketLeft", "["),
        Code::BracketRight => ("BracketRight", "]"),
        Code::Comma => ("Comma", ","),
        Code::Digit0 => ("Digit0", "0"),
        Code::Digit1 => ("Digit1", "1"),
        Code::Digit2 => ("Digit2", "2"),
        Code::Digit3 => ("Digit3", "3"),
        Code::Digit4 => ("Digit4", "4"),
        Code::Digit5 => ("Digit5", "5"),
        Code::Digit6 => ("Digit6", "6"),
        Code::Digit7 => ("Digit7", "7"),
        Code::Digit8 => ("Digit8", "8"),
        Code::Digit9 => ("Digit9", "9"),
        Code::Equal => ("Equal", "="),
        Code::KeyA => ("KeyA", "A"),
        Code::KeyB => ("KeyB", "B"),
        Code::KeyC => ("KeyC", "C"),
        Code::KeyD => ("KeyD", "D"),
        Code::KeyE => ("KeyE", "E"),
        Code::KeyF => ("KeyF", "F"),
        Code::KeyG => ("KeyG", "G"),
        Code::KeyH => ("KeyH", "H"),
        Code::KeyI => ("KeyI", "I"),
        Code::KeyJ => ("KeyJ", "J"),
        Code::KeyK => ("KeyK", "K"),
        Code::KeyL => ("KeyL", "L"),
        Code::KeyM => ("KeyM", "M"),
        Code::KeyN => ("KeyN", "N"),
        Code::KeyO => ("KeyO", "O"),
        Code::KeyP => ("KeyP", "P"),
        Code::KeyQ => ("KeyQ", "Q"),
        Code::KeyR => ("KeyR", "R"),
        Code::KeyS => ("KeyS", "S"),
        Code::KeyT => ("KeyT", "T"),
        Code::KeyU => ("KeyU", "U"),
        Code::KeyV => ("KeyV", "V"),
        Code::KeyW => ("KeyW", "W"),
        Code::KeyX => ("KeyX", "X"),
        Code::KeyY => ("KeyY", "Y"),
        Code::KeyZ => ("KeyZ", "Z"),
        Code::Minus => ("Minus", "-"),
        Code::Period => ("Period", "."),
        Code::Quote => ("Quote", "'"),
        Code::Semicolon => ("Semicolon", ";"),
        Code::Slash => ("Slash", "/"),
        Code::Backspace => ("Backspace", "Backspace"),
        Code::Enter => ("Enter", "Enter"),
        Code::Space => ("Space", "Space"),
        Code::Tab => ("Tab", "Tab"),
        Code::Delete => ("Delete", "Delete"),
        Code::End => ("End", "End"),
        Code::Home => ("Home", "Home"),
        Code::Insert => ("Insert", "Insert"),
        Code::PageDown => ("PageDown", "PageDown"),
        Code::PageUp => ("PageUp", "PageUp"),
        Code::PrintScreen => ("PrintScreen", "PrintScreen"),
        Code::ScrollLock => ("ScrollLock", "ScrollLock"),
        Code::ArrowDown => ("ArrowDown", "Down"),
        Code::ArrowLeft => ("ArrowLeft", "Left"),
        Code::ArrowRight => ("ArrowRight", "Right"),
        Code::ArrowUp => ("ArrowUp", "Up"),
        Code::Numpad0 => ("Numpad0", "Numpad 0"),
        Code::Numpad1 => ("Numpad1", "Numpad 1"),
        Code::Numpad2 => ("Numpad2", "Numpad 2"),
        Code::Numpad3 => ("Numpad3", "Numpad 3"),
        Code::Numpad4 => ("Numpad4", "Numpad 4"),
        Code::Numpad5 => ("Numpad5", "Numpad 5"),
        Code::Numpad6 => ("Numpad6", "Numpad 6"),
        Code::Numpad7 => ("Numpad7", "Numpad 7"),
        Code::Numpad8 => ("Numpad8", "Numpad 8"),
        Code::Numpad9 => ("Numpad9", "Numpad 9"),
        Code::NumpadAdd => ("NumpadAdd", "Numpad +"),
        Code::NumpadDecimal => ("NumpadDecimal", "Numpad ."),
        Code::NumpadDivide => ("NumpadDivide", "Numpad /"),
        Code::NumpadEnter => ("NumpadEnter", "Numpad Enter"),
        Code::NumpadEqual => ("NumpadEqual", "Numpad ="),
        Code::NumpadMultiply => ("NumpadMultiply", "Numpad *"),
        Code::NumpadSubtract => ("NumpadSubtract", "Numpad -"),
        Code::Escape => ("Escape", "Esc"),
        Code::F1 => ("F1", "F1"),
        Code::F2 => ("F2", "F2"),
        Code::F3 => ("F3", "F3"),
        Code::F4 => ("F4", "F4"),
        Code::F5 => ("F5", "F5"),
        Code::F6 => ("F6", "F6"),
        Code::F7 => ("F7", "F7"),
        Code::F8 => ("F8", "F8"),
        Code::F9 => ("F9", "F9"),
        Code::F10 => ("F10", "F10"),
        Code::F11 => ("F11", "F11"),
        Code::F12 => ("F12", "F12"),
        _ => return None,
    })
}
