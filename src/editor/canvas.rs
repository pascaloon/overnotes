//! The pan/zoom canvas viewport: it turns pointer and keyboard events into
//! [`interaction`] gestures and renders the world layer.

use dioxus::prelude::*;

use super::geometry::{MAX_ZOOM, MIN_ZOOM, selection_bounds};
use super::interaction::{self, DragState};
use super::objects::{ObjectView, Tool, drawing, frame, registry};
use super::{EditorState, TransactionKind};

/// How long the camera keeps coalescing wheel events into one undo step.
const WHEEL_COMMIT_DELAY_MS: u64 = 220;

#[component]
pub fn Canvas() -> Element {
    let mut state = use_context::<EditorState>();

    let (pan_x, pan_y) = *state.pan.read();
    let zoom = *state.zoom.read();
    let tool = *state.tool.read();
    let interactive = state.is_edit_mode();
    let tool_class = registry::cursor_class(tool);
    let panning = matches!(*state.drag.read(), DragState::Pan { .. });

    let graph_path = state.current_graph_path.read().clone();
    let selected_ids = state.selected.read().clone();
    let object_ids: Vec<u64> = state
        .doc
        .read()
        .objects_at_path(&graph_path)
        .map(|objects| objects.iter().map(|o| o.id).collect())
        .unwrap_or_default();
    let group_bounds = (selected_ids.len() > 1)
        .then(|| {
            state
                .doc
                .read()
                .objects_at_path(&graph_path)
                .and_then(|objects| selection_bounds(objects, &selected_ids))
        })
        .flatten();

    let drop_target = state.drop_target.read().clone();
    let marquee_rect = match state.drag.read().clone() {
        DragState::BoxSelect {
            start_screen,
            current_screen,
        } => Some((
            start_screen.0.min(current_screen.0),
            start_screen.1.min(current_screen.1),
            (start_screen.0 - current_screen.0).abs(),
            (start_screen.1 - current_screen.1).abs(),
        )),
        _ => None,
    };

    rsx! {
        div {
            class: "viewport {tool_class}",
            class: if panning { "panning" },
            tabindex: "0",

            onmounted: move |evt| {
                let data = evt.data();
                state.viewport_mount.set(Some(data.clone()));
                spawn(async move {
                    let _ = data.set_focus(true).await;
                });
            },
            onblur: move |_| interaction::finalize_lost_gesture(&mut state),
            onpointercancel: move |_| interaction::finalize_lost_gesture(&mut state),

            onmousedown: move |evt| {
                if !interactive {
                    return;
                }
                let coords = evt.client_coordinates();
                let (sx, sy) = (coords.x, coords.y);
                state.menu_open.set(false);
                state.close_context_menu();

                let button = evt.trigger_button();
                let is_primary = button == Some(dioxus::html::input_data::MouseButton::Primary);
                let is_middle = button == Some(dioxus::html::input_data::MouseButton::Auxiliary);
                if is_middle {
                    state.begin_transaction(TransactionKind::Gesture);
                    state
                        .drag
                        .set(DragState::Pan {
                            start_mouse: (sx, sy),
                            start_pan: *state.pan.peek(),
                            moved: false,
                        });
                    return;
                }
                if !is_primary {
                    return;
                }
                match registry::tool_spec(tool) {
                    None => {
                        state
                            .drag
                            .set(DragState::BoxSelect {
                                start_screen: (sx, sy),
                                current_screen: (sx, sy),
                            });
                    }
                    Some(spec) => {
                        let world = state.screen_to_world(sx, sy);
                        (spec.on_press)(&mut state, world);
                    }
                }
            },

            oncontextmenu: move |evt| {
                evt.prevent_default();
                state.close_context_menu();
            },

            onmousemove: move |evt| {
                if !interactive {
                    return;
                }
                let coords = evt.client_coordinates();
                interaction::pointer_move(
                    &mut state,
                    (coords.x, coords.y),
                    evt.modifiers().shift(),
                );
            },

            onmouseup: move |_| {
                if interactive {
                    interaction::pointer_up(&mut state);
                }
            },

            onmouseleave: move |_| interaction::pointer_leave(&mut state),

            onwheel: move |evt| {
                if !interactive {
                    return;
                }
                evt.prevent_default();
                let coords = evt.client_coordinates();
                zoom_at(&mut state, (coords.x, coords.y), wheel_delta_y(&evt));
            },

            onkeydown: move |evt| {
                if interactive {
                    handle_key(&mut state, &evt);
                }
            },

            div {
                class: "world",
                style: "transform: translate({pan_x}px, {pan_y}px) scale({zoom});",

                for id in object_ids {
                    ObjectView { key: "{id}", id }
                }

                if interactive {
                    if let Some(bounds) = group_bounds {
                        {frame::group_chrome(state, bounds, zoom)}
                    }
                }

                {drawing::live_stroke(&state)}
            }

            if let Some((x, y, w, h)) = marquee_rect {
                div {
                    class: "marquee-rect",
                    style: "left: {x}px; top: {y}px; width: {w}px; height: {h}px;",
                }
            }

            if let Some(target) = drop_target {
                div {
                    class: "drop-label",
                    style: "left: {target.screen_pos.0}px; top: {target.screen_pos.1}px;",
                    "Move to {target.name}"
                }
            }
        }
    }
}

fn wheel_delta_y(evt: &Event<WheelData>) -> f64 {
    match evt.delta() {
        dioxus::html::geometry::WheelDelta::Pixels(v) => v.y,
        dioxus::html::geometry::WheelDelta::Lines(v) => v.y * 100.0,
        dioxus::html::geometry::WheelDelta::Pages(v) => v.y * 800.0,
    }
}

/// Zoom around the pointer, coalescing a burst of wheel events into one
/// undo step once the wheel goes quiet.
fn zoom_at(state: &mut EditorState, screen: (f64, f64), delta_y: f64) {
    let (mx, my) = screen;
    let old_zoom = *state.zoom.peek();
    let new_zoom = (old_zoom * (-delta_y * 0.0012).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
    let (px, py) = *state.pan.peek();
    let wx = (mx - px) / old_zoom;
    let wy = (my - py) / old_zoom;
    state.begin_transaction(TransactionKind::Wheel);
    state.set_camera((mx - wx * new_zoom, my - wy * new_zoom), new_zoom);

    let sequence = *state.wheel_sequence.peek() + 1;
    state.wheel_sequence.set(sequence);
    let document_id = state.doc.peek().id.clone();
    let mut state = *state;
    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(WHEEL_COMMIT_DELAY_MS)).await;
        if *state.wheel_sequence.peek() == sequence
            && state.doc.peek().id == document_id
            && state.transaction_kind.peek().as_ref() == Some(&TransactionKind::Wheel)
        {
            state.commit_transaction();
        }
    });
}

fn handle_key(state: &mut EditorState, evt: &Event<KeyboardData>) {
    let editing = state.editing.peek().is_some();
    match evt.key() {
        Key::Delete | Key::Backspace if !editing => state.delete_selected(),
        Key::F2 if !editing => state.rename_selected(),
        Key::Escape => {
            if interaction::cancel_gesture(state) {
                // Cancelling restored the pre-gesture checkpoint.
            } else if *state.shot_mode.peek() {
                state.cancel_region_screenshot();
            } else if editing {
                state.stop_editing();
            } else if state.context_menu.read().is_some() {
                state.close_context_menu();
            } else if *state.menu_open.peek() {
                state.menu_open.set(false);
            } else {
                state.deselect();
                state.tool.set(Tool::Select);
            }
        }
        Key::Character(c) if !editing && evt.modifiers().ctrl() && c.eq_ignore_ascii_case("v") => {
            super::objects::image::paste_from_clipboard(state);
        }
        _ => {}
    }
}
