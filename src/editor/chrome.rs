//! Editor chrome: left toolbar, hamburger menu, bottom bar, and the
//! screenshot region selector.

use dioxus::prelude::*;

use super::{EditorState, Tool, ViewMode};
use crate::store::{self, STROKE_COLORS};

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
                class: if tool == Tool::Select { "active" },
                title: "Select / move (Esc)",
                onclick: move |_| state.tool.set(Tool::Select),
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    path { d: "M5 3 L19 12 L12 13.5 L9.5 20 Z" }
                }
            }
            button {
                class: "tool-btn",
                class: if tool == Tool::Note { "active" },
                title: "Add note",
                onclick: move |_| state.tool.set(Tool::Note),
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    path { d: "M4 4 H20 V14 L14 20 H4 Z" }
                    path { d: "M14 20 V14 H20" }
                }
            }
            button {
                class: "tool-btn",
                class: if draw_active { "active" },
                title: "Draw",
                onclick: move |_| state.tool.set(Tool::Draw),
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                    stroke: "currentColor", stroke_width: "2", stroke_linejoin: "round",
                    path { d: "M4 20 L5 15.5 L16.5 4 L20 7.5 L8.5 19 Z" }
                }
            }
            button {
                class: "tool-btn",
                title: "Paste image from clipboard (Ctrl+V)",
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
pub fn MainMenu() -> Element {
    let mut state = use_context::<EditorState>();
    let open = *state.menu_open.read();

    let doc_name = state.doc.read().name.clone();
    let game_exe = state.doc.read().game_exe.clone();
    let current_id = state.doc.read().id.clone();
    let overview_opacity = state.doc.read().overview_opacity;
    let edit_opacity = state.doc.read().edit_opacity;

    let docs = if open {
        store::list_documents(&game_exe)
    } else {
        Vec::new()
    };

    rsx! {
        button {
            class: "hamburger",
            title: "Menu",
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
            }
        }
    }
}

#[component]
pub fn BottomBar() -> Element {
    let mut state = use_context::<EditorState>();

    rsx! {
        div { class: "bottombar",
            button {
                class: "bar-btn",
                title: "Capture the game, then crop it (Ctrl+Shift+S)",
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
                title: "Back to overview (Ctrl+Shift+E)",
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
