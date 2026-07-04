//! Rendering + interaction for individual canvas objects (notes, drawings,
//! images) including selection frame, resize/rotate handles and note colors.

use dioxus::prelude::*;

use super::canvas::{editor_interactive, points_attr};
use super::{DragState, EditorState, Tool};
use crate::store::{ObjectKind, NOTE_COLORS};

const RESIZE_DIRS: [(&str, f64, f64, &str); 8] = [
    ("nw", 0.0, 0.0, "nwse-resize"),
    ("n", 0.5, 0.0, "ns-resize"),
    ("ne", 1.0, 0.0, "nesw-resize"),
    ("e", 1.0, 0.5, "ew-resize"),
    ("se", 1.0, 1.0, "nwse-resize"),
    ("s", 0.5, 1.0, "ns-resize"),
    ("sw", 0.0, 1.0, "nesw-resize"),
    ("w", 0.0, 0.5, "ew-resize"),
];

#[component]
pub fn ObjectView(id: u64) -> Element {
    let mut state = use_context::<EditorState>();

    let doc = state.doc.read();
    let Some(obj) = doc.object(id) else {
        return rsx! {};
    };
    let (x, y, w, h, rotation) = (obj.x, obj.y, obj.w, obj.h, obj.rotation);
    let kind = obj.kind.clone();
    let is_image = matches!(kind, ObjectKind::Image { .. });
    drop(doc);

    let interactive = editor_interactive(&state);
    let selected = interactive && *state.selected.read() == Some(id);
    let editing = *state.editing_note.read() == Some(id);
    let tool = *state.tool.read();
    let zoom = *state.zoom.read();

    let on_body_down = move |evt: Event<MouseData>| {
        if !interactive || tool != Tool::Select {
            return;
        }
        evt.stop_propagation();
        state.menu_open.set(false);
        if *state.editing_note.peek() == Some(id) {
            // Editing this note's text: let the textarea take the event.
            return;
        }
        state.select_only(id);
        state.focus_canvas();
        let coords = evt.client_coordinates();
        let start_world = state.screen_to_world(coords.x, coords.y);
        let orig_pos = {
            let doc = state.doc.peek();
            let o = doc.object(id).unwrap();
            (o.x, o.y)
        };
        state.drag.set(DragState::MoveObject {
            id,
            start_world,
            orig_pos,
        });
    };

    let hs = 10.0 / zoom;
    let rot_off = 28.0 / zoom;

    rsx! {
        div {
            class: "obj",
            style: "left: {x}px; top: {y}px; width: {w}px; height: {h}px; transform: rotate({rotation}deg);",
            onmousedown: on_body_down,
            ondoubleclick: move |evt| {
                if !interactive || tool != Tool::Select {
                    return;
                }
                evt.stop_propagation();
                if matches!(
                    state.doc.peek().object(id).map(|o| &o.kind),
                    Some(ObjectKind::Note { .. })
                ) {
                    state.selected.set(Some(id));
                    state.editing_note.set(Some(id));
                }
            },

            match kind {
                ObjectKind::Note { ref text, ref color } => rsx! {
                    div {
                        class: "note-body",
                        style: "background: {color};",
                        if editing {
                            textarea {
                                class: "note-text",
                                value: "{text}",
                                placeholder: "Type a note...",
                                spellcheck: "false",
                                onmounted: move |evt| {
                                    let data = evt.data();
                                    spawn(async move {
                                        let _ = data.set_focus(true).await;
                                    });
                                },
                                onmousedown: move |evt| evt.stop_propagation(),
                                oninput: move |evt| {
                                    let mut doc = state.doc.write();
                                    if let Some(o) = doc.object_mut(id) {
                                        if let ObjectKind::Note { text, .. } = &mut o.kind {
                                            *text = evt.value();
                                        }
                                    }
                                },
                            }
                        } else {
                            div {
                                class: "note-text",
                                style: "white-space: pre-wrap; overflow: hidden;",
                                "{text}"
                            }
                        }
                    }
                },
                ObjectKind::Drawing { ref points, vw, vh, ref stroke, stroke_width } => rsx! {
                    svg {
                        class: "drawing-svg",
                        view_box: "0 0 {vw} {vh}",
                        preserve_aspect_ratio: "none",
                        polyline {
                            points: points_attr(points),
                            fill: "none",
                            stroke: "{stroke}",
                            stroke_width: "{stroke_width}",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                        }
                    }
                },
                ObjectKind::Image { .. } => {
                    let url = state.image_cache.read().get(&id).cloned().unwrap_or_default();
                    rsx! {
                        img { class: "obj-img", src: "{url}", draggable: "false" }
                    }
                }
            }

            if selected {
                div { class: "sel-frame" }

                if !editing {
                    // Resize handles.
                    for (dir, fx, fy, cursor) in RESIZE_DIRS {
                        div {
                            class: "h",
                            style: "left: {fx * w}px; top: {fy * h}px; width: {hs}px; height: {hs}px; cursor: {cursor};",
                            onmousedown: move |evt| {
                                evt.stop_propagation();
                                let coords = evt.client_coordinates();
                                let start_world = state.screen_to_world(coords.x, coords.y);
                                let (orig, rot) = {
                                    let doc = state.doc.peek();
                                    let o = doc.object(id).unwrap();
                                    ((o.x, o.y, o.w, o.h), o.rotation)
                                };
                                let aspect_ratio = if is_image && orig.3 > 0.0 {
                                    Some(orig.2 / orig.3)
                                } else {
                                    None
                                };
                                state.drag.set(DragState::Resize {
                                    id,
                                    dir,
                                    start_world,
                                    orig,
                                    rotation: rot,
                                    aspect_ratio,
                                });
                            },
                            title: if is_image { "Hold Shift to keep image ratio" },
                        }
                    }

                    // Rotate handle above the top edge.
                    div {
                        class: "h rot",
                        style: "left: {w / 2.0}px; top: {-rot_off}px; width: {hs * 1.2}px; height: {hs * 1.2}px;",
                        onmousedown: move |evt| {
                            evt.stop_propagation();
                            let (center, orig_rotation) = {
                                let doc = state.doc.peek();
                                let o = doc.object(id).unwrap();
                                ((o.x + o.w / 2.0, o.y + o.h / 2.0), o.rotation)
                            };
                            let center_screen = state.world_to_screen(center.0, center.1);
                            let coords = evt.client_coordinates();
                            let start_angle = (coords.y - center_screen.1)
                                .atan2(coords.x - center_screen.0)
                                .to_degrees();
                            state.drag.set(DragState::Rotate {
                                id,
                                center_screen,
                                start_angle,
                                orig_rotation,
                            });
                        },
                    }

                    // Note color palette.
                    if matches!(
                        state.doc.read().object(id).map(|o| &o.kind),
                        Some(ObjectKind::Note { .. })
                    ) {
                        div {
                            class: "color-row",
                            style: "left: 4px; top: {h + 12.0 / zoom}px; transform: scale({1.0 / zoom}); transform-origin: top left;",
                            for color in NOTE_COLORS {
                                div {
                                    class: "color-dot",
                                    style: "background: {color};",
                                    onmousedown: move |evt| {
                                        evt.stop_propagation();
                                        let mut doc = state.doc.write();
                                        if let Some(o) = doc.object_mut(id) {
                                            if let ObjectKind::Note { color: c, .. } = &mut o.kind {
                                                *c = color.to_string();
                                            }
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
