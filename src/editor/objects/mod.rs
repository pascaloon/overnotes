//! Canvas elements.
//!
//! [`ObjectView`] renders the parts every object shares - placement, rotation,
//! opacity, selection, dragging and the context menu - and delegates the rest
//! to the element that owns the object's kind. See [`registry`] for how a new
//! element is added.

pub mod drawing;
pub mod frame;
pub mod image;
pub mod note;
pub mod registry;
pub mod subgraph;

pub use registry::{ObjectCtx, Tool, element_for, styles};

use dioxus::prelude::*;

use crate::editor::interaction::DragState;
use crate::editor::{EditorHost, EditorState, TransactionKind, ViewMode};

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
    let overview_opacity = doc.overview_opacity;
    drop(doc);

    let element = element_for(&kind);
    let is_container = kind.is_container();
    let interactive = state.is_edit_mode();
    let selected = interactive && state.is_selected(id);
    let single_selected = interactive && state.single_selected() == Some(id);
    let editing = state.is_editing(id);
    let select_tool = *state.tool.read() == Tool::Select;
    let zoom = *state.zoom.read();

    // Previewing per-object transparency from the context menu shows the
    // overview opacity even while editing.
    let previewing_opacity = state
        .context_menu
        .read()
        .as_ref()
        .is_some_and(|menu| menu.id == id && menu.source_path == graph_path);
    let overview = state.host == EditorHost::Overlay && *state.mode.read() == ViewMode::Overview;
    let object_opacity = if overview || previewing_opacity {
        opacity_override.unwrap_or(overview_opacity)
    } else {
        1.0
    };

    let is_drop_target = state
        .drop_target
        .read()
        .as_ref()
        .is_some_and(|target| target.id == id);
    let drag_state = state.drag.read().clone();
    let moving = matches!(drag_state, DragState::MoveObjects { .. });
    let is_being_moved = drag_state.moving_ids().contains(&id);
    // Containers lift above the rest while something is dragged over them.
    let drop_candidate = is_container && moving && !is_being_moved;

    let ctx = ObjectCtx {
        id,
        kind,
        state,
        editing,
    };
    let body = element.body(&ctx);
    let toolbar = element.toolbar(&ctx);

    rsx! {
        div {
            class: "obj",
            class: if selected { "selected" },
            class: if drop_candidate { "drop-candidate" },
            class: if is_drop_target { "drop-target" },
            style: "left: {x}px; top: {y}px; width: {w}px; height: {h}px; transform: rotate({rotation}deg); opacity: {object_opacity};",

            onmousedown: move |evt| {
                if !interactive || !select_tool {
                    return;
                }
                evt.stop_propagation();
                if evt.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
                    return;
                }
                state.menu_open.set(false);
                state.close_context_menu();
                if state.is_editing(id) {
                    // The inline editor takes the event instead.
                    return;
                }
                let coords = evt.client_coordinates();
                begin_move(&mut state, id, (coords.x, coords.y));
            },

            oncontextmenu: move |evt| {
                if !interactive || !select_tool {
                    return;
                }
                evt.prevent_default();
                evt.stop_propagation();
                let coords = evt.client_coordinates();
                state.open_object_context_menu(id, coords.x, coords.y);
            },

            ondoubleclick: move |evt| {
                if !interactive || !select_tool {
                    return;
                }
                evt.stop_propagation();
                element.on_activate(&mut state, id);
            },

            {body}

            if selected {
                {
                    frame::selection_chrome(
                        frame::Frame {
                            state,
                            id,
                            w,
                            h,
                            zoom,
                            single_selected,
                            editing,
                            locks_aspect_ratio: element.locks_aspect_ratio(),
                        },
                        toolbar,
                    )
                }
            }
        }
    }
}

/// Start dragging the object under the pointer, together with the rest of the
/// selection when it was already selected.
fn begin_move(state: &mut EditorState, id: u64, screen: (f64, f64)) {
    let already_selected = state.is_selected(id);
    if !already_selected {
        state.select_only(id);
    }
    state.focus_canvas();
    let start_world = state.screen_to_world(screen.0, screen.1);
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
    state.begin_transaction(TransactionKind::Gesture);
    state.drag.set(DragState::MoveObjects {
        anchor_id: id,
        start_world,
        orig_positions,
    });
}
