//! Chrome drawn around selected objects: the selection frame, the eight
//! resize handles, the rotate handle, and the floating toolbar shell that
//! elements fill with their own controls.

use dioxus::prelude::*;

use super::registry::ObjectCtx;
use crate::editor::geometry::{RESIZE_DIRS, selection_bounds};
use crate::editor::interaction::DragState;
use crate::editor::{EditorState, TransactionKind};

pub struct Frame {
    pub state: EditorState,
    pub id: u64,
    pub w: f64,
    pub h: f64,
    pub zoom: f64,
    pub single_selected: bool,
    /// The object's inline editor is open, so handles would be in the way.
    pub editing: bool,
    pub locks_aspect_ratio: bool,
}

/// Selection frame plus, for a lone selection, the element's floating toolbar
/// and the resize/rotate handles.
pub fn selection_chrome(frame: Frame, toolbar: Option<Element>) -> Element {
    let Frame {
        mut state,
        id,
        w,
        h,
        zoom,
        single_selected,
        editing,
        locks_aspect_ratio,
    } = frame;
    let handle = 10.0 / zoom;
    let rotate_offset = 28.0 / zoom;

    rsx! {
        div { class: "sel-frame" }

        if single_selected {
            if let Some(toolbar) = toolbar {
                div {
                    class: "floating-object-toolbar",
                    style: "left: 50%; top: {-46.0 / zoom}px; transform: translateX(-50%) scale({1.0 / zoom}); transform-origin: top center;",
                    onmousedown: move |evt| evt.stop_propagation(),
                    oncontextmenu: move |evt| {
                        evt.prevent_default();
                        evt.stop_propagation();
                    },
                    {toolbar}
                }
            }

            if !editing {
                for (dir, fx, fy, cursor) in RESIZE_DIRS {
                    div {
                        class: "h",
                        style: "left: {fx * w}px; top: {fy * h}px; width: {handle}px; height: {handle}px; cursor: {cursor};",
                        aria_label: if locks_aspect_ratio { "Hold Shift to keep the original ratio" },
                        onmousedown: move |evt| {
                            evt.stop_propagation();
                            let coords = evt.client_coordinates();
                            let start_world = state.screen_to_world(coords.x, coords.y);
                            let Some((orig, rotation)) = ({
                                let doc = state.doc.peek();
                                let path = state.current_graph_path.peek().clone();
                                doc.object_at_path(&path, id)
                                    .map(|o| ((o.x, o.y, o.w, o.h), o.rotation))
                            }) else {
                                return;
                            };
                            let aspect_ratio = if locks_aspect_ratio && orig.3 > 0.0 {
                                Some(orig.2 / orig.3)
                            } else {
                                None
                            };
                            state.begin_transaction(TransactionKind::Gesture);
                            state
                                .drag
                                .set(DragState::Resize {
                                    id,
                                    dir,
                                    start_world,
                                    orig,
                                    rotation,
                                    aspect_ratio,
                                });
                        },
                    }
                }

                div {
                    class: "h rot",
                    style: "left: {w / 2.0}px; top: {-rotate_offset}px; width: {handle * 1.2}px; height: {handle * 1.2}px;",
                    onmousedown: move |evt| {
                        evt.stop_propagation();
                        let Some((center, orig_rotation)) = ({
                            let doc = state.doc.peek();
                            let path = state.current_graph_path.peek().clone();
                            doc.object_at_path(&path, id)
                                .map(|o| ((o.x + o.w / 2.0, o.y + o.h / 2.0), o.rotation))
                        }) else {
                            return;
                        };
                        let center_screen = state.world_to_screen(center.0, center.1);
                        let coords = evt.client_coordinates();
                        let start_angle = crate::editor::geometry::angle_at(
                            center_screen,
                            (coords.x, coords.y),
                        );
                        state.begin_transaction(TransactionKind::Gesture);
                        state
                            .drag
                            .set(DragState::Rotate {
                                id,
                                center_screen,
                                start_angle,
                                orig_rotation,
                            });
                    },
                }
            }
        }
    }
}

/// Dashed frame and handles around a multi-object selection.
pub fn group_chrome(mut state: EditorState, bounds: (f64, f64, f64, f64), zoom: f64) -> Element {
    let (gx, gy, gw, gh) = bounds;
    let handle = 10.0 / zoom;

    rsx! {
        div {
            class: "group-selection",
            style: "left: {gx}px; top: {gy}px; width: {gw}px; height: {gh}px;",
            div { class: "group-frame" }
            for (dir, fx, fy, cursor) in RESIZE_DIRS {
                div {
                    class: "h group-h",
                    style: "left: {fx * gw}px; top: {fy * gh}px; width: {handle}px; height: {handle}px; cursor: {cursor};",
                    aria_label: "Hold Shift to preserve group ratio",
                    onmousedown: move |evt| {
                        evt.stop_propagation();
                        if evt.trigger_button()
                            != Some(dioxus::html::input_data::MouseButton::Primary)
                        {
                            return;
                        }
                        state.menu_open.set(false);
                        state.close_context_menu();
                        state.focus_canvas();
                        let coords = evt.client_coordinates();
                        let start_world = state.screen_to_world(coords.x, coords.y);
                        let path = state.current_graph_path.peek().clone();
                        let ids = state.selected.peek().clone();
                        let Some((orig_bounds, orig_objects)) = ({
                            let doc = state.doc.peek();
                            doc.objects_at_path(&path)
                                .and_then(|objects| {
                                    selection_bounds(objects, &ids)
                                        .map(|bounds| {
                                            let rects = objects
                                                .iter()
                                                .filter(|obj| ids.contains(&obj.id))
                                                .map(|obj| (obj.id, (obj.x, obj.y, obj.w, obj.h)))
                                                .collect::<Vec<_>>();
                                            (bounds, rects)
                                        })
                                })
                        }) else {
                            return;
                        };
                        let aspect_ratio = if orig_bounds.3 > 0.0 {
                            Some(orig_bounds.2 / orig_bounds.3)
                        } else {
                            None
                        };
                        state.begin_transaction(TransactionKind::Gesture);
                        state
                            .drag
                            .set(DragState::ResizeSelection {
                                dir,
                                start_world,
                                orig_bounds,
                                orig_objects,
                                aspect_ratio,
                            });
                    },
                }
            }
        }
    }
}

/// A row of color swatches that recolor the object as one history step.
pub fn color_swatches(cx: &ObjectCtx, colors: &'static [&'static str]) -> Element {
    let (id, mut state) = (cx.id, cx.state);
    let active = cx.kind.color().map(str::to_string);

    rsx! {
        div { class: "floating-object-toolbar-colors",
            for color in colors.iter().copied() {
                div {
                    key: "{color}",
                    class: "color-dot",
                    class: if color == "transparent" { "transparent" },
                    class: if active.as_deref() == Some(color) { "active" },
                    style: "background-color: {color};",
                    onmousedown: move |evt| {
                        evt.stop_propagation();
                        evt.prevent_default();
                        let path = state.current_graph_path.read().clone();
                        state
                            .edit_document(move |doc| {
                                if let Some(obj) = doc.object_at_path_mut(&path, id) {
                                    obj.kind.set_color(color);
                                }
                            });
                    },
                }
            }
        }
    }
}
