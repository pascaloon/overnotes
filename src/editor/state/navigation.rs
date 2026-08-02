//! Navigating the graph tree (entering and leaving containers) and moving
//! objects between graphs, including the drag-and-drop target search.

use dioxus::prelude::*;

use super::EditorState;
use crate::editor::interaction::DragState;
use crate::editor::objects::Tool;

/// A container the pointer is currently hovering while dragging objects.
#[derive(Clone, PartialEq)]
pub struct DropTarget {
    pub id: u64,
    pub name: String,
    pub screen_pos: (f64, f64),
}

impl EditorState {
    /// Descend into a container object.
    pub fn enter_graph(&mut self, id: u64) {
        self.commit_transaction();
        self.persist_current_graph_view();
        let path = self.current_graph_path.read().clone();
        if !self.is_container(&path, id) {
            return;
        }
        let mut next = path;
        next.push(id);
        self.current_graph_path.set(next);
        self.reset_for_graph_change();
        self.focus_canvas();
    }

    pub fn navigate_to_graph_depth(&mut self, depth: usize) {
        self.commit_transaction();
        self.persist_current_graph_view();
        let mut path = self.current_graph_path.read().clone();
        path.truncate(depth);
        self.current_graph_path.set(path);
        self.reset_for_graph_change();
        self.focus_canvas();
    }

    /// Open the inline name editor of the single selected container (F2).
    pub fn rename_selected(&mut self) {
        let Some(id) = self.single_selected() else {
            return;
        };
        let path = self.current_graph_path.read().clone();
        if !self.is_container(&path, id) {
            return;
        }
        self.editing.set(Some(id));
    }

    /// Rename from the context menu, which may target another graph.
    pub fn rename_context_object(&mut self) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        if !self.is_container(&menu.source_path, menu.id) {
            self.close_context_menu();
            return;
        }
        self.commit_transaction();
        self.persist_current_graph_view();
        self.current_graph_path.set(menu.source_path.clone());
        self.invalidate_async_operations();
        self.load_current_graph_view();
        self.start_editing(menu.id);
        self.close_context_menu();
        self.focus_canvas();
    }

    pub fn move_context_object_to_graph(&mut self, target_path: Vec<u64>) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        let id = menu.id;
        let source_path = menu.source_path.clone();
        let moved = self.edit_document(move |doc| {
            doc.move_object_to_graph(&source_path, id, &target_path);
        });
        self.close_context_menu();
        if !moved {
            self.show_toast("Could not move object");
        } else if self.selected.read().contains(&id) {
            self.deselect();
        }
    }

    pub fn update_drop_target(&mut self, moving_ids: &[u64], screen_pos: (f64, f64)) {
        let path = self.current_graph_path.read().clone();
        let cursor_world = self.screen_to_world(screen_pos.0, screen_pos.1);
        let target = self
            .find_drop_target(&path, moving_ids, cursor_world)
            .map(|mut target| {
                target.screen_pos = screen_pos;
                target
            });
        self.drop_target.set(target);
    }

    /// Topmost container under the cursor that is not itself being dragged.
    fn find_drop_target(
        &self,
        path: &[u64],
        moving_ids: &[u64],
        cursor_world: (f64, f64),
    ) -> Option<DropTarget> {
        let doc = self.doc.read();
        let (cx, cy) = cursor_world;
        doc.objects_at_path(path)?
            .iter()
            .rev()
            .find_map(|candidate| {
                if moving_ids.contains(&candidate.id)
                    || cx < candidate.x
                    || cx > candidate.x + candidate.w
                    || cy < candidate.y
                    || cy > candidate.y + candidate.h
                {
                    return None;
                }
                candidate.kind.container_label().map(|name| DropTarget {
                    id: candidate.id,
                    name: name.to_string(),
                    screen_pos: (0.0, 0.0),
                })
            })
    }

    /// Commit a drag onto the hovered container, if there is one.
    pub fn drop_into_container(&mut self, ids: &[u64]) -> bool {
        let path = self.current_graph_path.read().clone();
        let Some(target_id) = self.drop_target.peek().as_ref().map(|target| target.id) else {
            return false;
        };
        if self
            .doc
            .write()
            .move_objects_into_container(&path, ids, target_id)
        {
            self.deselect();
            return true;
        }
        false
    }

    fn is_container(&self, path: &[u64], id: u64) -> bool {
        self.doc
            .read()
            .object_at_path(path, id)
            .is_some_and(|obj| obj.kind.is_container())
    }

    /// Shared cleanup for switching to a different graph in the same document.
    fn reset_for_graph_change(&mut self) {
        self.invalidate_async_operations();
        self.deselect();
        self.drag.set(DragState::None);
        self.drop_target.set(None);
        self.menu_open.set(false);
        self.close_context_menu();
        self.load_current_graph_view();
        self.tool.set(Tool::Select);
    }
}
