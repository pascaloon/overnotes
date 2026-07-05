//! Editor chrome: left toolbar, hamburger menu, bottom bar, and the
//! screenshot region selector.

use dioxus::prelude::*;

use super::{EditorHost, EditorState, Tool, ViewMode};
use crate::store::{self, KeyboardShortcut, ObjectKind, STROKE_COLORS};

#[component]
pub fn Toolbar() -> Element {
    let mut state = use_context::<EditorState>();
    let tool = *state.tool.read();

    let draw_active = tool == Tool::Draw;
    let stroke_color = state.stroke_color.read().clone();
    let stroke_width = *state.stroke_width.read();

    rsx! {
        div { class: "toolbar",
            button {
                class: "tool-btn",
                class: "has-tooltip",
                class: if tool == Tool::Select { "active" },
                aria_label: "Select / move (Esc)",
                onclick: move |_| state.tool.set(Tool::Select),
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    path { d: "M5 3 L19 12 L12 13.5 L9.5 20 Z" }
                }
            }
            button {
                class: "tool-btn",
                class: "has-tooltip",
                class: if tool == Tool::Note { "active" },
                aria_label: "Add note",
                onclick: move |_| state.tool.set(Tool::Note),
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    path { d: "M4 4 H20 V14 L14 20 H4 Z" }
                    path { d: "M14 20 V14 H20" }
                }
            }
            button {
                class: "tool-btn",
                class: "has-tooltip",
                class: if tool == Tool::Subgraph { "active" },
                aria_label: "Add subgraph",
                onclick: move |_| state.tool.set(Tool::Subgraph),
                svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    path { d: "M3 7 H9 L11 10 H21 V19 H3 Z" }
                    path { d: "M3 7 V5 H9 L11 8 H21 V10" }
                }
            }
            button {
                class: "tool-btn",
                class: "has-tooltip",
                class: if draw_active { "active" },
                aria_label: "Draw",
                onclick: move |_| state.tool.set(Tool::Draw),
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    path { d: "M4 20 L5 15.5 L16.5 4 L20 7.5 L8.5 19 Z" }
                }
            }
            button {
                class: "tool-btn",
                class: "has-tooltip",
                aria_label: "Paste image from clipboard (Ctrl+V)",
                onclick: move |_| state.paste_image_from_clipboard(),
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    rect { x: "3", y: "4", width: "18", height: "16", rx: "2" }
                    circle { cx: "9", cy: "10", r: "2" }
                    path { d: "M3 17 L9 13 L13 16 L17 12 L21 15" }
                }
            }
        }

        if draw_active {
            div { class: "draw-opts",
                span { class: "opt-label", "Stroke color" }
                div { class: "swatch-row",
                    for color in STROKE_COLORS {
                        div {
                            class: "swatch",
                            class: if stroke_color == color { "active" },
                            style: "background: {color};",
                            onclick: move |_| state.stroke_color.set(color.to_string()),
                        }
                    }
                }
                span { class: "opt-label", "Width: {stroke_width:.0}px" }
                input {
                    r#type: "range",
                    min: "1",
                    max: "16",
                    step: "1",
                    value: "{stroke_width}",
                    oninput: move |evt| {
                        if let Ok(v) = evt.value().parse::<f64>() {
                            state.stroke_width.set(v);
                        }
                    },
                }
            }
        }
    }
}

#[component]
pub fn Breadcrumbs() -> Element {
    let mut state = use_context::<EditorState>();
    let path = state.current_graph_path.read().clone();
    let names = state.doc.read().breadcrumb_names(&path);

    rsx! {
        div { class: "breadcrumbs",
            button {
                class: "crumb",
                class: if path.is_empty() { "current" },
                aria_label: "Root graph",
                onclick: move |_| state.navigate_to_graph_depth(0),
                "Root"
            }
            for (i, name) in names.iter().enumerate() {
                span { class: "crumb-sep", "/" }
                button {
                    class: "crumb",
                    class: if i + 1 == path.len() { "current" },
                    aria_label: "{name}",
                    onclick: move |_| state.navigate_to_graph_depth(i + 1),
                    "{name}"
                }
            }
        }
    }
}

#[component]
pub fn ObjectContextMenu() -> Element {
    let mut state = use_context::<EditorState>();
    let Some(menu) = state.context_menu.read().clone() else {
        return rsx! {};
    };

    let doc = state.doc.read();
    let obj = doc.object_at_path(&menu.source_path, menu.id);
    let is_subgraph = matches!(obj.map(|obj| &obj.kind), Some(ObjectKind::Subgraph { .. }));
    let overview_opacity = doc.overview_opacity;
    let object_opacity = obj
        .and_then(|obj| obj.opacity_override)
        .unwrap_or(overview_opacity);
    let uses_default_opacity = obj
        .map(|obj| obj.opacity_override.is_none())
        .unwrap_or(true);
    let destinations = doc.subgraph_destinations(menu.id, &menu.source_path);
    let has_destinations = !destinations.is_empty();
    drop(doc);

    rsx! {
        div {
            class: "object-menu",
            style: "left: {menu.x}px; top: {menu.y}px;",
            onmousedown: move |evt| evt.stop_propagation(),
            oncontextmenu: move |evt| {
                evt.prevent_default();
                evt.stop_propagation();
            },
            if is_subgraph {
                button {
                    class: "object-menu-item",
                    onclick: move |_| state.rename_context_subgraph(),
                    "Rename"
                }
            }
            div { class: "object-menu-order-row",
                button {
                    class: "object-menu-icon-btn",
                    class: "has-tooltip",
                    aria_label: "Move to top",
                    onclick: move |_| state.move_context_object_to_top(),
                    svg {
                        width: "16",
                        height: "16",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M6 5 H18" }
                        path { d: "M12 19 V9" }
                        path { d: "M7 14 L12 9 L17 14" }
                    }
                }
                button {
                    class: "object-menu-icon-btn",
                    class: "has-tooltip",
                    aria_label: "Move up",
                    onclick: move |_| state.move_context_object_up(),
                    svg {
                        width: "16",
                        height: "16",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M12 19 V5" }
                        path { d: "M7 10 L12 5 L17 10" }
                    }
                }
                button {
                    class: "object-menu-icon-btn",
                    class: "has-tooltip",
                    aria_label: "Move down",
                    onclick: move |_| state.move_context_object_down(),
                    svg {
                        width: "16",
                        height: "16",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M12 5 V19" }
                        path { d: "M7 14 L12 19 L17 14" }
                    }
                }
                button {
                    class: "object-menu-icon-btn",
                    class: "has-tooltip",
                    aria_label: "Move to bottom",
                    onclick: move |_| state.move_context_object_to_bottom(),
                    svg {
                        width: "16",
                        height: "16",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M6 19 H18" }
                        path { d: "M12 5 V15" }
                        path { d: "M7 10 L12 15 L17 10" }
                    }
                }
            }
            div { class: "object-menu-divider" }
            div { class: "object-menu-control",
                div { class: "object-menu-control-head",
                    span { "Transparency" }
                    span { class: "slider-value", "{(object_opacity * 100.0):.0}%" }
                }
                div { class: "object-menu-slider-row",
                    input {
                        r#type: "range",
                        min: "0",
                        max: "1",
                        step: "0.05",
                        value: "{object_opacity}",
                        oninput: move |evt| {
                            if let Ok(v) = evt.value().parse::<f64>() {
                                state.set_context_object_opacity(v);
                            }
                        },
                    }
                    if !uses_default_opacity {
                        button {
                            class: "object-menu-reset-icon",
                            class: "has-tooltip",
                            aria_label: "Reset to overview transparency",
                            onclick: move |_| state.reset_context_object_opacity(),
                            svg {
                                width: "16",
                                height: "16",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                path { d: "M3 12 A9 9 0 1 0 6 5.3" }
                                path { d: "M3 4 V10 H9" }
                            }
                        }
                    }
                }
            }
            div { class: "object-menu-divider" }
            div {
                class: "object-menu-item object-menu-parent",
                class: if !has_destinations { "disabled" },
                "Move to"
                span { class: "object-menu-arrow", ">" }
                if has_destinations {
                    div { class: "object-submenu",
                        for destination in destinations.iter().cloned() {
                            button {
                                class: "object-menu-item",
                                aria_label: "{destination.label}",
                                onclick: move |_| state.move_context_object_to_graph(destination.path.clone()),
                                "{destination.label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn MainMenu() -> Element {
    let mut state = use_context::<EditorState>();
    let open = *state.menu_open.read();

    let doc_name = state.doc.read().name.clone();
    let game_exe = state.doc.read().game_exe.clone();
    let current_id = state.doc.read().id.clone();
    let overview_opacity = state.doc.read().overview_opacity;
    let edit_opacity = state.doc.read().edit_opacity;
    let settings = state.settings.read().clone();
    let toggle_shortcut = settings.overlay_toggle_shortcut.clone();
    let screenshot_shortcut = settings.overlay_screenshot_shortcut.clone();
    let show_overlay_shortcuts = state.host == EditorHost::Overlay;

    let docs = if open {
        store::list_documents(&game_exe)
    } else {
        Vec::new()
    };

    rsx! {
        button {
            class: "hamburger",
            class: "has-tooltip",
            aria_label: "Menu",
            onclick: move |_| {
                let now = *state.menu_open.peek();
                state.menu_open.set(!now);
            },
            svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                stroke: "currentColor", stroke_width: "2", stroke_linecap: "round",
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
                        oninput: move |evt| {
                            state.doc.write().name = evt.value();
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

                div { class: "menu-section",
                    span { class: "menu-label", "Overview transparency" }
                    input {
                        r#type: "range",
                        min: "0.1",
                        max: "1",
                        step: "0.05",
                        value: "{overview_opacity}",
                        oninput: move |evt| {
                            if let Ok(v) = evt.value().parse::<f64>() {
                                state.doc.write().overview_opacity = v;
                            }
                        },
                    }
                    span { class: "slider-value", "{(overview_opacity * 100.0):.0}%" }
                }

                div { class: "menu-section",
                    span { class: "menu-label", "Editing transparency" }
                    input {
                        r#type: "range",
                        min: "0.3",
                        max: "1",
                        step: "0.05",
                        value: "{edit_opacity}",
                        oninput: move |evt| {
                            if let Ok(v) = evt.value().parse::<f64>() {
                                state.doc.write().edit_opacity = v;
                            }
                        },
                    }
                    span { class: "slider-value", "{(edit_opacity * 100.0):.0}%" }
                }

                if show_overlay_shortcuts {
                    div { class: "menu-section",
                        span { class: "menu-label", "Overlay shortcuts" }
                        ShortcutCapture {
                            title: "Edit mode",
                            shortcut: toggle_shortcut,
                            kind: ShortcutKind::ToggleEditMode,
                        }
                        ShortcutCapture {
                            title: "Screenshot",
                            shortcut: screenshot_shortcut,
                            kind: ShortcutKind::Screenshot,
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ShortcutKind {
    ToggleEditMode,
    Screenshot,
}

#[component]
fn ShortcutCapture(title: &'static str, shortcut: KeyboardShortcut, kind: ShortcutKind) -> Element {
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
                            settings.overlay_screenshot_shortcut.accelerator == new_shortcut.accelerator
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

#[component]
pub fn BottomBar() -> Element {
    let mut state = use_context::<EditorState>();
    let settings = state.settings.read().clone();
    let screenshot_label = settings.overlay_screenshot_shortcut.label;
    let toggle_label = settings.overlay_toggle_shortcut.label;

    rsx! {
        div { class: "bottombar",
            button {
                class: "bar-btn",
                class: "has-tooltip",
                aria_label: "Capture the game, then crop it ({screenshot_label})",
                onclick: move |_| {
                    state.start_region_screenshot();
                },
                svg { width: "18", height: "18", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    path { d: "M4 8 H7 L9 5 H15 L17 8 H20 V19 H4 Z" }
                    circle { cx: "12", cy: "13", r: "3.5" }
                }
                "Screenshot"
            }
            div { class: "divider" }
            button {
                class: "bar-btn danger",
                class: "has-tooltip",
                aria_label: "Back to overview ({toggle_label})",
                onclick: move |_| {
                    state.deselect();
                    state.menu_open.set(false);
                    state.mode.set(ViewMode::Overview);
                },
                svg { width: "18", height: "18", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linecap: "round",
                    path { d: "M6 6 L18 18 M18 6 L6 18" }
                }
                "Close"
            }
        }
    }
}

/// Fullscreen rubber-band region selector for the screenshot tool.
#[component]
pub fn ShotOverlay() -> Element {
    let mut state = use_context::<EditorState>();
    let mut start = use_signal(|| None::<(f64, f64)>);
    let mut cur = use_signal(|| (0.0f64, 0.0f64));
    let shot = state.pending_shot.read().clone();

    let start_val: Option<(f64, f64)> = *start.read();
    let rect = start_val.map(|s| {
        let c = *cur.read();
        let x = s.0.min(c.0);
        let y = s.1.min(c.1);
        let w = (s.0 - c.0).abs();
        let h = (s.1 - c.1).abs();
        (x, y, w, h)
    });

    rsx! {
        div {
            class: "shot-overlay",
            onmousedown: move |evt| {
                let c = evt.client_coordinates();
                start.set(Some((c.x, c.y)));
                cur.set((c.x, c.y));
            },
            onmousemove: move |evt| {
                if start.peek().is_some() {
                    let c = evt.client_coordinates();
                    cur.set((c.x, c.y));
                }
            },
            onmouseup: move |_| {
                let Some((x, y, w, h)) = rect else {
                    state.cancel_region_screenshot();
                    return;
                };
                start.set(None);
                if w < 4.0 || h < 4.0 {
                    state.cancel_region_screenshot();
                    return;
                }
                let Some(shot) = state.pending_shot.peek().clone() else {
                    state.cancel_region_screenshot();
                    return;
                };
                state.cancel_region_screenshot();
                let win = dioxus::desktop::window();
                let size = win.inner_size();
                let scale = win.scale_factor();
                let vw = (size.width as f64 / scale).max(1.0);
                let vh = (size.height as f64 / scale).max(1.0);
                let (wx, wy) = state.screen_to_world(x + w / 2.0, y + h / 2.0);
                let mut st = state;
                // This component unmounts right away (shot_mode = false), so
                // the task must outlive the scope: spawn_forever, not spawn.
                dioxus::dioxus_core::spawn_forever(async move {
                    let (px, py, pw, ph) = (
                        (x / vw * shot.width as f64).round() as i32,
                        (y / vh * shot.height as f64).round() as i32,
                        (w / vw * shot.width as f64).round() as i32,
                        (h / vh * shot.height as f64).round() as i32,
                    );
                    let result = tokio::task::spawn_blocking(move || {
                        crate::platform::capture::crop_png_region(&shot.png, px, py, pw, ph)
                    })
                    .await;
                    match result {
                        Ok(Ok(png)) => st.add_image_png(&png, wx, wy),
                        Ok(Err(e)) => st.show_toast(&format!("Capture failed: {e}")),
                        Err(_) => st.show_toast("Capture failed"),
                    }
                });
            },

            if let Some(shot) = shot.as_ref() {
                img {
                    class: "shot-image",
                    src: "{shot.data_url}",
                    draggable: "false",
                }
            }

            div { class: "shot-hint", "Drag to crop the screenshot - Esc to cancel" }

            if let Some((x, y, w, h)) = rect {
                div {
                    class: "shot-rect",
                    style: "left: {x}px; top: {y}px; width: {w}px; height: {h}px;",
                }
            }
        }
    }
}
