//! The pan/zoom canvas viewport and its interaction state machine.

use dioxus::prelude::*;

use super::objects::ObjectView;
use super::{DragState, EditorState, Tool, ViewMode};

const MIN_ZOOM: f64 = 0.2;
const MAX_ZOOM: f64 = 4.0;
const MIN_W: f64 = 40.0;
const MIN_H: f64 = 30.0;

/// Rotate a vector by `-angle_deg` (world -> object-local space).
fn to_local(dx: f64, dy: f64, angle_deg: f64) -> (f64, f64) {
    let a = angle_deg.to_radians();
    (dx * a.cos() + dy * a.sin(), -dx * a.sin() + dy * a.cos())
}

#[component]
pub fn Canvas() -> Element {
    let mut state = use_context::<EditorState>();

    let (pan_x, pan_y) = *state.pan.read();
    let zoom = *state.zoom.read();
    let tool = *state.tool.read();
    let interactive = state.is_edit_mode();

    let tool_class = match tool {
        Tool::Select => "tool-select",
        Tool::Note => "tool-note",
        Tool::Draw => "tool-draw",
        Tool::Subgraph => "tool-subgraph",
    };
    let panning = matches!(*state.drag.read(), DragState::Pan { .. });

    let graph_path = state.current_graph_path.read().clone();
    let object_ids: Vec<u64> = state
        .doc
        .read()
        .objects_at_path(&graph_path)
        .map(|objects| objects.iter().map(|o| o.id).collect())
        .unwrap_or_default();

    let live = state.live_points.read().clone();
    let live_stroke = state.stroke_color.read().clone();
    let live_width = *state.stroke_width.read();
    let drop_target = state.drop_target.read().clone();
    let marquee_rect = match state.drag.read().clone() {
        DragState::BoxSelect {
            start_screen,
            current_screen,
        } => {
            let x = start_screen.0.min(current_screen.0);
            let y = start_screen.1.min(current_screen.1);
            let w = (start_screen.0 - current_screen.0).abs();
            let h = (start_screen.1 - current_screen.1).abs();
            Some((x, y, w, h))
        }
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
                if !is_primary && !is_middle {
                    return;
                }
                if tool == Tool::Select && is_primary && evt.modifiers().shift() {
                    state.drag.set(DragState::BoxSelect {
                        start_screen: (sx, sy),
                        current_screen: (sx, sy),
                    });
                    return;
                }
                if is_middle || tool == Tool::Select {
                    state.drag.set(DragState::Pan {
                        start_mouse: (sx, sy),
                        start_pan: *state.pan.peek(),
                        moved: false,
                    });
                    return;
                }
                match tool {
                    Tool::Note => {
                        let (wx, wy) = state.screen_to_world(sx, sy);
                        state.add_note(wx, wy);
                    }
                    Tool::Draw => {
                        let (wx, wy) = state.screen_to_world(sx, sy);
                        state.live_points.set(vec![[wx, wy]]);
                        state.drag.set(DragState::DrawStroke);
                    }
                    Tool::Subgraph => {
                        let (wx, wy) = state.screen_to_world(sx, sy);
                        state.add_subgraph(wx, wy);
                    }
                    Tool::Select => {}
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
                let (sx, sy) = (coords.x, coords.y);
                let drag = state.drag.peek().clone();
                match drag {
                    DragState::None => {}
                    DragState::Pan { start_mouse, start_pan, moved } => {
                        let dx = sx - start_mouse.0;
                        let dy = sy - start_mouse.1;
                        state.set_pan((start_pan.0 + dx, start_pan.1 + dy));
                        if !moved && (dx.abs() > 3.0 || dy.abs() > 3.0) {
                            state.drag.set(DragState::Pan { start_mouse, start_pan, moved: true });
                        }
                    }
                    DragState::MoveObjects {
                        anchor_id,
                        start_world,
                        orig_positions,
                    } => {
                        let (wx, wy) = state.screen_to_world(sx, sy);
                        let path = state.current_graph_path.read().clone();
                        let mut doc = state.doc.write();
                        for (id, orig_pos) in orig_positions {
                            if let Some(obj) = doc.object_at_path_mut(&path, id) {
                                obj.x = orig_pos.0 + (wx - start_world.0);
                                obj.y = orig_pos.1 + (wy - start_world.1);
                            }
                        }
                        drop(doc);
                        if state.selected.read().len() == 1 {
                            state.update_subgraph_drop_target(anchor_id, (sx, sy));
                        }
                    }
                    DragState::BoxSelect { start_screen, .. } => {
                        state.drag.set(DragState::BoxSelect {
                            start_screen,
                            current_screen: (sx, sy),
                        });
                    }
                    DragState::Resize {
                        id,
                        dir,
                        start_world,
                        orig,
                        rotation,
                        aspect_ratio,
                    } => {
                        let (wx, wy) = state.screen_to_world(sx, sy);
                        let (ldx, ldy) = to_local(wx - start_world.0, wy - start_world.1, rotation);
                        let (ox, oy, ow, oh) = orig;
                        let mut x = ox;
                        let mut y = oy;
                        let mut w = ow;
                        let mut h = oh;
                        if dir.contains('e') {
                            w = (ow + ldx).max(MIN_W);
                        }
                        if dir.contains('w') {
                            let ldx = ldx.min(ow - MIN_W);
                            x = ox + ldx;
                            w = ow - ldx;
                        }
                        if dir.contains('s') {
                            h = (oh + ldy).max(MIN_H);
                        }
                        if dir.contains('n') {
                            let ldy = ldy.min(oh - MIN_H);
                            y = oy + ldy;
                            h = oh - ldy;
                        }
                        if evt.modifiers().shift() {
                            if let Some(ratio) = aspect_ratio {
                                let scale = aspect_scale(dir, w, h, ow, oh);
                                w = (ow * scale).max(MIN_W);
                                h = (w / ratio).max(MIN_H);
                                if h > oh * scale {
                                    w = h * ratio;
                                }

                                if dir.contains('w') {
                                    x = ox + ow - w;
                                } else if !dir.contains('e') {
                                    x = ox + (ow - w) / 2.0;
                                } else {
                                    x = ox;
                                }

                                if dir.contains('n') {
                                    y = oy + oh - h;
                                } else if !dir.contains('s') {
                                    y = oy + (oh - h) / 2.0;
                                } else {
                                    y = oy;
                                }
                            }
                        }
                        let path = state.current_graph_path.read().clone();
                        let mut doc = state.doc.write();
                        if let Some(obj) = doc.object_at_path_mut(&path, id) {
                            obj.x = x;
                            obj.y = y;
                            obj.w = w;
                            obj.h = h;
                        }
                    }
                    DragState::Rotate { id, center_screen, start_angle, orig_rotation } => {
                        let angle = (sy - center_screen.1).atan2(sx - center_screen.0).to_degrees();
                        let mut rotation = orig_rotation + (angle - start_angle);
                        // Snap near the cardinal angles.
                        let snapped = (rotation / 90.0).round() * 90.0;
                        if (rotation - snapped).abs() < 4.0 {
                            rotation = snapped;
                        }
                        let path = state.current_graph_path.read().clone();
                        let mut doc = state.doc.write();
                        if let Some(obj) = doc.object_at_path_mut(&path, id) {
                            obj.rotation = rotation.rem_euclid(360.0);
                        }
                    }
                    DragState::DrawStroke => {
                        let (wx, wy) = state.screen_to_world(sx, sy);
                        let mut pts = state.live_points.write();
                        let push = pts
                            .last()
                            .map(|p| {
                                let dx = p[0] - wx;
                                let dy = p[1] - wy;
                                (dx * dx + dy * dy).sqrt() > 0.75
                            })
                            .unwrap_or(true);
                        if push {
                            pts.push([wx, wy]);
                        }
                    }
                }
            },

            onmouseup: move |_| {
                if !interactive {
                    return;
                }
                let drag = state.drag.peek().clone();
                match drag {
                    DragState::Pan { moved, .. } => {
                        if !moved {
                            state.deselect();
                        }
                    }
                    DragState::DrawStroke => state.finish_stroke(),
                    DragState::MoveObjects {
                        anchor_id,
                        orig_positions,
                        ..
                    } => {
                        if orig_positions.len() == 1 {
                            state.try_drop_object_into_subgraph(anchor_id);
                        }
                        state.drop_target.set(None);
                    }
                    DragState::BoxSelect {
                        start_screen,
                        current_screen,
                    } => {
                        if (start_screen.0 - current_screen.0).abs() > 3.0
                            || (start_screen.1 - current_screen.1).abs() > 3.0
                        {
                            state.select_objects_in_world_rect(
                                state.screen_to_world(start_screen.0, start_screen.1),
                                state.screen_to_world(current_screen.0, current_screen.1),
                            );
                        }
                    }
                    _ => {}
                }
                state.drag.set(DragState::None);
            },

            onmouseleave: move |_| {
                if matches!(*state.drag.peek(), DragState::DrawStroke) {
                    state.finish_stroke();
                }
                state.drop_target.set(None);
                state.drag.set(DragState::None);
            },

            onwheel: move |evt| {
                if !interactive {
                    return;
                }
                evt.prevent_default();
                let coords = evt.client_coordinates();
                let (mx, my) = (coords.x, coords.y);
                let dy = match evt.delta() {
                    dioxus::html::geometry::WheelDelta::Pixels(v) => v.y,
                    dioxus::html::geometry::WheelDelta::Lines(v) => v.y * 100.0,
                    dioxus::html::geometry::WheelDelta::Pages(v) => v.y * 800.0,
                };
                let old_zoom = *state.zoom.peek();
                let new_zoom = (old_zoom * (-dy * 0.0012).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
                let (px, py) = *state.pan.peek();
                let wx = (mx - px) / old_zoom;
                let wy = (my - py) / old_zoom;
                state.set_camera((mx - wx * new_zoom, my - wy * new_zoom), new_zoom);
            },

            onkeydown: move |evt| {
                if !interactive {
                    return;
                }
                let editing_note = state.editing_note.read().is_some();
                let editing_subgraph = state.editing_subgraph.read().is_some();
                let editing = editing_note || editing_subgraph;
                match evt.key() {
                    Key::Delete | Key::Backspace if !editing => {
                        state.delete_selected();
                    }
                    Key::F2 if !editing => {
                        state.rename_selected_subgraph();
                    }
                    Key::Escape => {
                        if *state.shot_mode.peek() {
                            state.cancel_region_screenshot();
                        } else if editing_note {
                            state.editing_note.set(None);
                        } else if editing_subgraph {
                            state.editing_subgraph.set(None);
                        } else if state.context_menu.read().is_some() {
                            state.close_context_menu();
                        } else if *state.menu_open.peek() {
                            state.menu_open.set(false);
                        } else {
                            state.deselect();
                            state.tool.set(Tool::Select);
                        }
                    }
                    Key::Character(c)
                        if !editing
                            && evt.modifiers().ctrl()
                            && c.eq_ignore_ascii_case("v") =>
                    {
                        state.paste_image_from_clipboard();
                    }
                    _ => {}
                }
            },

            div {
                class: "world",
                style: "transform: translate({pan_x}px, {pan_y}px) scale({zoom});",

                for id in object_ids {
                    ObjectView { key: "{id}", id }
                }

                if !live.is_empty() {
                    svg {
                        class: "live-stroke",
                        polyline {
                            points: points_attr(&live),
                            fill: "none",
                            stroke: "{live_stroke}",
                            stroke_width: "{live_width}",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                        }
                    }
                }
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

fn aspect_scale(dir: &str, w: f64, h: f64, ow: f64, oh: f64) -> f64 {
    let min_scale = (MIN_W / ow).max(MIN_H / oh);
    let sx = (w / ow).max(min_scale);
    let sy = (h / oh).max(min_scale);

    if dir == "e" || dir == "w" {
        sx
    } else if dir == "n" || dir == "s" {
        sy
    } else if (sx - 1.0).abs() >= (sy - 1.0).abs() {
        sx
    } else {
        sy
    }
}

pub fn points_attr(points: &[[f64; 2]]) -> String {
    let mut out = String::with_capacity(points.len() * 12);
    for p in points {
        out.push_str(&format!("{:.2},{:.2} ", p[0], p[1]));
    }
    out
}

/// Whether the editor is currently interactive (guards object handlers too).
pub fn editor_interactive(state: &EditorState) -> bool {
    state.host == super::EditorHost::Standalone || *state.mode.read() == ViewMode::Edit
}
