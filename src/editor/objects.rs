//! Rendering + interaction for individual canvas objects (notes, drawings,
//! images) including selection frame, resize/rotate handles and note colors.

use dioxus::prelude::*;

use super::canvas::{editor_interactive, points_attr};
use super::{DragState, EditorHost, EditorState, Tool, ViewMode};
use crate::store::{NOTE_COLORS, ObjectKind, SUBGRAPH_COLORS};

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

    let graph_path = state.current_graph_path.read().clone();
    let doc = state.doc.read();
    let Some(obj) = doc.object_at_path(&graph_path, id) else {
        return rsx! {};
    };
    let (x, y, w, h, rotation) = (obj.x, obj.y, obj.w, obj.h, obj.rotation);
    let opacity_override = obj.opacity_override;
    let kind = obj.kind.clone();
    let is_image = matches!(kind, ObjectKind::Image { .. });
    let overview_opacity = doc.overview_opacity;
    drop(doc);

    let interactive = editor_interactive(&state);
    let selected = interactive && state.is_selected(id);
    let single_selected = interactive && state.single_selected() == Some(id);
    let editing = *state.editing_note.read() == Some(id);
    let editing_subgraph = *state.editing_subgraph.read() == Some(id);
    let tool = *state.tool.read();
    let zoom = *state.zoom.read();
    let previewing_opacity = state
        .context_menu
        .read()
        .as_ref()
        .is_some_and(|menu| menu.id == id && menu.source_path == graph_path);
    let object_opacity = if (state.host == EditorHost::Overlay
        && *state.mode.read() == ViewMode::Overview)
        || previewing_opacity
    {
        opacity_override.unwrap_or(overview_opacity)
    } else {
        1.0
    };
    let drop_target = state.drop_target.read().clone();
    let is_drop_target = drop_target.as_ref().is_some_and(|target| target.id == id);

    let on_body_down = move |evt: Event<MouseData>| {
        if !interactive || tool != Tool::Select {
            return;
        }
        evt.stop_propagation();
        if evt.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
            return;
        }
        state.menu_open.set(false);
        state.close_context_menu();
        if *state.editing_note.peek() == Some(id) {
            // Editing this note's text: let the textarea take the event.
            return;
        }
        let already_selected = state.is_selected(id);
        if !already_selected {
            state.select_only(id);
        }
        state.focus_canvas();
        let coords = evt.client_coordinates();
        let start_world = state.screen_to_world(coords.x, coords.y);
        let orig_positions = {
            let doc = state.doc.peek();
            let path = state.current_graph_path.peek().clone();
            let moving_ids = if already_selected {
                state.selected.peek().clone()
            } else {
                vec![id]
            };
            moving_ids
                .into_iter()
                .filter_map(|moving_id| {
                    doc.object_at_path(&path, moving_id)
                        .map(|o| (moving_id, (o.x, o.y)))
                })
                .collect::<Vec<_>>()
        };
        state.drag.set(DragState::MoveObjects {
            anchor_id: id,
            start_world,
            orig_positions,
        });
    };

    let hs = 10.0 / zoom;
    let rot_off = 28.0 / zoom;

    rsx! {
        div {
            class: "obj",
            class: if is_drop_target { "drop-target" },
            style: "left: {x}px; top: {y}px; width: {w}px; height: {h}px; transform: rotate({rotation}deg); opacity: {object_opacity};",
            onmousedown: on_body_down,
            oncontextmenu: move |evt| {
                if !interactive || tool != Tool::Select {
                    return;
                }
                evt.prevent_default();
                evt.stop_propagation();
                let coords = evt.client_coordinates();
                state.open_object_context_menu(id, coords.x, coords.y);
            },
            ondoubleclick: move |evt| {
                if !interactive || tool != Tool::Select {
                    return;
                }
                evt.stop_propagation();
                if matches!(
                    state
                        .doc
                        .peek()
                        .object_at_path(&state.current_graph_path.peek(), id)
                        .map(|o| &o.kind),
                    Some(ObjectKind::Note { .. })
                ) {
                    state.select_only(id);
                    state.editing_note.set(Some(id));
                    state.editing_subgraph.set(None);
                } else if matches!(
                    state
                        .doc
                        .peek()
                        .object_at_path(&state.current_graph_path.peek(), id)
                        .map(|o| &o.kind),
                    Some(ObjectKind::Subgraph { .. })
                ) {
                    state.enter_subgraph(id);
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
                                    let path = state.current_graph_path.read().clone();
                                    let mut doc = state.doc.write();
                                    if let Some(o) = doc.object_at_path_mut(&path, id) {
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
                },
                ObjectKind::Subgraph { ref name, ref color, .. } => rsx! {
                    div { class: "subgraph-body",
                        svg {
                            class: "folder-icon",
                            view_box: "0 0 96 72",
                            preserve_aspect_ratio: "xMidYMid meet",
                            path {
                                d: "M8 18 H36 L43 28 H88 V60 Q88 66 82 66 H14 Q8 66 8 60 Z",
                                fill: "{color}",
                            }
                            path {
                                d: "M8 18 Q8 12 14 12 H31 Q35 12 38 16 L43 23 H82 Q88 23 88 29 V34 H8 Z",
                                fill: "{color}",
                                opacity: "0.82",
                            }
                            path {
                                d: "M8 34 H88 V60 Q88 66 82 66 H14 Q8 66 8 60 Z",
                                fill: "{color}",
                            }
                            path {
                                d: "M12 37 H84",
                                stroke: "rgba(255,255,255,0.28)",
                                stroke_width: "2",
                            }
                        }
                        if editing_subgraph {
                            input {
                                class: "subgraph-name subgraph-name-input",
                                r#type: "text",
                                value: "{name}",
                                spellcheck: "false",
                                onmounted: move |evt| {
                                    let data = evt.data();
                                    spawn(async move {
                                        let _ = data.set_focus(true).await;
                                    });
                                },
                                onmousedown: move |evt| evt.stop_propagation(),
                                oninput: move |evt| {
                                    let path = state.current_graph_path.read().clone();
                                    let mut doc = state.doc.write();
                                    if let Some(o) = doc.object_at_path_mut(&path, id) {
                                        if let ObjectKind::Subgraph { name, .. } = &mut o.kind {
                                            *name = evt.value();
                                        }
                                    }
                                },
                                onkeydown: move |evt| {
                                    evt.stop_propagation();
                                    if matches!(evt.key(), Key::Enter | Key::Escape) {
                                        state.editing_subgraph.set(None);
                                        state.focus_canvas();
                                    }
                                },
                            }
                        } else {
                            div {
                                class: "subgraph-name",
                                title: "Double-click to rename",
                                ondoubleclick: move |evt| {
                                    evt.stop_propagation();
                                    state.select_only(id);
                                    state.editing_note.set(None);
                                    state.editing_subgraph.set(Some(id));
                                },
                                "{name}"
                            }
                        }
                    }
                }
            }

            if selected {
                div { class: "sel-frame" }

                if single_selected && !editing {
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
                                    let path = state.current_graph_path.peek().clone();
                                    let o = doc.object_at_path(&path, id).unwrap();
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
                                let path = state.current_graph_path.peek().clone();
                                let o = doc.object_at_path(&path, id).unwrap();
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
                        state
                            .doc
                            .read()
                            .object_at_path(&state.current_graph_path.read(), id)
                            .map(|o| &o.kind),
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
                                        let path = state.current_graph_path.read().clone();
                                        let mut doc = state.doc.write();
                                        if let Some(o) = doc.object_at_path_mut(&path, id) {
                                            if let ObjectKind::Note { color: c, .. } = &mut o.kind {
                                                *c = color.to_string();
                                            }
                                        }
                                    },
                                }
                            }
                        }
                    } else if matches!(
                        state
                            .doc
                            .read()
                            .object_at_path(&state.current_graph_path.read(), id)
                            .map(|o| &o.kind),
                        Some(ObjectKind::Subgraph { .. })
                    ) {
                        div {
                            class: "color-row",
                            style: "left: 4px; top: {h + 12.0 / zoom}px; transform: scale({1.0 / zoom}); transform-origin: top left;",
                            for color in SUBGRAPH_COLORS {
                                div {
                                    class: "color-dot",
                                    style: "background: {color};",
                                    onmousedown: move |evt| {
                                        evt.stop_propagation();
                                        let path = state.current_graph_path.read().clone();
                                        let mut doc = state.doc.write();
                                        if let Some(o) = doc.object_at_path_mut(&path, id) {
                                            if let ObjectKind::Subgraph { color: c, .. } = &mut o.kind {
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
