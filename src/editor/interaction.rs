//! The canvas pointer state machine: what a drag currently means, and how
//! pointer movement, release and interruption are applied to the document.

use dioxus::prelude::*;

use super::geometry::{self, resized_bounds, to_local};
use super::objects::{drawing, note};
use super::{EditorState, TransactionKind};

#[derive(Clone, PartialEq, Default)]
pub enum DragState {
    #[default]
    None,
    Pan {
        start_mouse: (f64, f64),
        start_pan: (f64, f64),
        moved: bool,
    },
    MoveObjects {
        anchor_id: u64,
        start_world: (f64, f64),
        orig_positions: Vec<(u64, (f64, f64))>,
    },
    BoxSelect {
        start_screen: (f64, f64),
        current_screen: (f64, f64),
    },
    Resize {
        id: u64,
        dir: &'static str,
        start_world: (f64, f64),
        orig: (f64, f64, f64, f64),
        rotation: f64,
        aspect_ratio: Option<f64>,
    },
    ResizeSelection {
        dir: &'static str,
        start_world: (f64, f64),
        orig_bounds: (f64, f64, f64, f64),
        orig_objects: Vec<(u64, (f64, f64, f64, f64))>,
        aspect_ratio: Option<f64>,
    },
    Rotate {
        id: u64,
        center_screen: (f64, f64),
        start_angle: f64,
        orig_rotation: f64,
    },
    /// Live text scaling from the note's floating toolbar.
    NoteFontSize {
        id: u64,
        start_mouse_x: f64,
        orig_font_size: f64,
    },
    /// A freehand stroke is being drawn into `live_points`.
    DrawStroke,
}

impl DragState {
    /// Ids the gesture is currently moving, if any.
    pub fn moving_ids(&self) -> Vec<u64> {
        match self {
            DragState::MoveObjects { orig_positions, .. } => {
                orig_positions.iter().map(|(id, _)| *id).collect()
            }
            _ => Vec::new(),
        }
    }
}

pub fn pointer_move(state: &mut EditorState, screen: (f64, f64), shift: bool) {
    let (sx, sy) = screen;
    let drag = state.drag.peek().clone();
    match drag {
        DragState::None => {}
        DragState::Pan {
            start_mouse,
            start_pan,
            moved,
        } => {
            let dx = sx - start_mouse.0;
            let dy = sy - start_mouse.1;
            state.set_pan((start_pan.0 + dx, start_pan.1 + dy));
            if !moved && (dx.abs() > 3.0 || dy.abs() > 3.0) {
                state.drag.set(DragState::Pan {
                    start_mouse,
                    start_pan,
                    moved: true,
                });
            }
        }
        DragState::MoveObjects {
            start_world,
            orig_positions,
            ..
        } => {
            let (wx, wy) = state.screen_to_world(sx, sy);
            let path = state.current_graph_path.read().clone();
            {
                let mut doc = state.doc.write();
                for (id, orig_pos) in &orig_positions {
                    if let Some(obj) = doc.object_at_path_mut(&path, *id) {
                        obj.x = orig_pos.0 + (wx - start_world.0);
                        obj.y = orig_pos.1 + (wy - start_world.1);
                    }
                }
            }
            let moving_ids = orig_positions.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            state.update_drop_target(&moving_ids, (sx, sy));
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
            let (dx, dy) = to_local(wx - start_world.0, wy - start_world.1, rotation);
            let keep_ratio = if shift { aspect_ratio } else { None };
            let (x, y, w, h) = resized_bounds(dir, dx, dy, orig, keep_ratio);
            let path = state.current_graph_path.read().clone();
            let mut doc = state.doc.write();
            if let Some(obj) = doc.object_at_path_mut(&path, id) {
                obj.x = x;
                obj.y = y;
                obj.w = w;
                obj.h = h;
            }
        }
        DragState::ResizeSelection {
            dir,
            start_world,
            orig_bounds,
            orig_objects,
            aspect_ratio,
        } => {
            let (ox, oy, ow, oh) = orig_bounds;
            if ow <= 0.0 || oh <= 0.0 {
                return;
            }
            let (wx, wy) = state.screen_to_world(sx, sy);
            let keep_ratio = if shift { aspect_ratio } else { None };
            let (x, y, w, h) = resized_bounds(
                dir,
                wx - start_world.0,
                wy - start_world.1,
                orig_bounds,
                keep_ratio,
            );
            let scale_x = w / ow;
            let scale_y = h / oh;
            let path = state.current_graph_path.read().clone();
            let mut doc = state.doc.write();
            for (id, (obj_x, obj_y, obj_w, obj_h)) in orig_objects {
                if let Some(obj) = doc.object_at_path_mut(&path, id) {
                    obj.x = x + (obj_x - ox) * scale_x;
                    obj.y = y + (obj_y - oy) * scale_y;
                    obj.w = obj_w * scale_x;
                    obj.h = obj_h * scale_y;
                }
            }
        }
        DragState::Rotate {
            id,
            center_screen,
            start_angle,
            orig_rotation,
        } => {
            let angle = geometry::angle_at(center_screen, (sx, sy));
            let rotation = geometry::snap_rotation(orig_rotation + (angle - start_angle));
            let path = state.current_graph_path.read().clone();
            let mut doc = state.doc.write();
            if let Some(obj) = doc.object_at_path_mut(&path, id) {
                obj.rotation = rotation;
            }
        }
        DragState::NoteFontSize {
            id,
            start_mouse_x,
            orig_font_size,
        } => note::apply_font_size_drag(state, id, orig_font_size + (sx - start_mouse_x) * 0.5),
        DragState::DrawStroke => drawing::extend_stroke(state, (sx, sy)),
    }
}

pub fn pointer_up(state: &mut EditorState) {
    let drag = state.drag.peek().clone();
    match &drag {
        DragState::DrawStroke => drawing::finish_stroke(state),
        DragState::MoveObjects { .. } => {
            state.drop_into_container(&drag.moving_ids());
            state.drop_target.set(None);
        }
        DragState::BoxSelect {
            start_screen,
            current_screen,
        } => {
            if (start_screen.0 - current_screen.0).abs() > 3.0
                || (start_screen.1 - current_screen.1).abs() > 3.0
            {
                let from = state.screen_to_world(start_screen.0, start_screen.1);
                let to = state.screen_to_world(current_screen.0, current_screen.1);
                state.select_objects_in_world_rect(from, to);
            } else {
                state.deselect();
            }
        }
        _ => {}
    }
    end_gesture(state);
}

pub fn pointer_leave(state: &mut EditorState) {
    let drag = state.drag.peek().clone();
    match &drag {
        DragState::DrawStroke => drawing::finish_stroke(state),
        DragState::MoveObjects { .. } => {
            state.drop_into_container(&drag.moving_ids());
        }
        _ => {}
    }
    state.drop_target.set(None);
    end_gesture(state);
}

/// Finalize an interrupted gesture so focus loss or pointer cancellation
/// cannot leave a stale transaction behind for a later action.
pub fn finalize_lost_gesture(state: &mut EditorState) {
    if matches!(*state.drag.peek(), DragState::DrawStroke) {
        drawing::finish_stroke(state);
    }
    state.drop_target.set(None);
    end_gesture(state);
}

/// Escape cancels an in-progress gesture and restores its baseline.
pub fn cancel_gesture(state: &mut EditorState) -> bool {
    if matches!(*state.drag.peek(), DragState::None) {
        return false;
    }
    if state.transaction_kind.peek().as_ref() == Some(&TransactionKind::Gesture) {
        state.cancel_transaction();
    }
    state.live_points.set(Vec::new());
    state.drop_target.set(None);
    state.drag.set(DragState::None);
    true
}

fn end_gesture(state: &mut EditorState) {
    if state.transaction_kind.peek().as_ref() == Some(&TransactionKind::Gesture) {
        state.commit_transaction();
    }
    state.drag.set(DragState::None);
}
