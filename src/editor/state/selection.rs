//! Selection, deletion, and the per-object context menu commands.

use dioxus::prelude::*;

use super::{EditorState, TransactionKind};

#[derive(Clone, PartialEq)]
pub struct ObjectContextMenu {
    pub id: u64,
    pub source_path: Vec<u64>,
    pub x: f64,
    pub y: f64,
}

impl EditorState {
    pub fn select_only(&mut self, id: u64) {
        self.selected.set(vec![id]);
        if !self.is_editing(id) {
            self.editing.set(None);
        }
    }

    pub fn set_selection(&mut self, ids: Vec<u64>) {
        self.selected.set(ids);
        self.editing.set(None);
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.selected.read().contains(&id)
    }

    pub fn single_selected(&self) -> Option<u64> {
        let selected = self.selected.read();
        if selected.len() == 1 {
            selected.first().copied()
        } else {
            None
        }
    }

    pub fn deselect(&mut self) {
        self.selected.set(Vec::new());
        self.editing.set(None);
    }

    pub fn select_objects_in_world_rect(&mut self, a: (f64, f64), b: (f64, f64)) {
        let left = a.0.min(b.0);
        let right = a.0.max(b.0);
        let top = a.1.min(b.1);
        let bottom = a.1.max(b.1);
        let path = self.current_graph_path.read().clone();
        let ids = {
            let doc = self.doc.read();
            doc.objects_at_path(&path)
                .map(|objects| {
                    objects
                        .iter()
                        .filter(|obj| {
                            obj.x <= right
                                && obj.x + obj.w >= left
                                && obj.y <= bottom
                                && obj.y + obj.h >= top
                        })
                        .map(|obj| obj.id)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        self.set_selection(ids);
    }

    pub fn delete_selected(&mut self) {
        let selected = self.selected.read().clone();
        if selected.is_empty() {
            return;
        }
        self.begin_transaction(TransactionKind::Gesture);
        let path = self.current_graph_path.read().clone();
        let mut removed_objects = Vec::new();
        {
            let mut doc = self.doc.write();
            for id in selected {
                if let Some(removed) = doc.remove_object_at_path(&path, id) {
                    removed_objects.push(removed);
                }
            }
        }
        if !removed_objects.is_empty() {
            let mut cache = self.image_cache.write();
            for removed in removed_objects {
                for image_id in removed.image_ids_recursive() {
                    cache.remove(&image_id);
                }
            }
        }
        self.deselect();
        self.commit_transaction();
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu.set(None);
    }

    pub fn open_object_context_menu(&mut self, id: u64, x: f64, y: f64) {
        let source_path = self.current_graph_path.read().clone();
        if self.doc.read().object_at_path(&source_path, id).is_none() {
            return;
        }
        self.selected.set(vec![id]);
        self.editing.set(None);
        self.menu_open.set(false);
        self.context_menu.set(Some(ObjectContextMenu {
            id,
            source_path,
            x,
            y,
        }));
    }

    /// Run `edit` against the context menu's object as one history step.
    fn edit_context_object(&mut self, edit: impl FnOnce(&mut crate::store::Document, &[u64], u64)) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        let path = menu.source_path.clone();
        let id = menu.id;
        self.edit_document(move |doc| edit(doc, &path, id));
        self.close_context_menu();
    }

    pub fn move_context_object_up(&mut self) {
        self.edit_context_object(|doc, path, id| {
            doc.move_object_up_at_path(path, id);
        });
    }

    pub fn move_context_object_to_top(&mut self) {
        self.edit_context_object(|doc, path, id| {
            doc.move_object_to_top_at_path(path, id);
        });
    }

    pub fn move_context_object_down(&mut self) {
        self.edit_context_object(|doc, path, id| {
            doc.move_object_down_at_path(path, id);
        });
    }

    pub fn move_context_object_to_bottom(&mut self) {
        self.edit_context_object(|doc, path, id| {
            doc.move_object_to_bottom_at_path(path, id);
        });
    }

    pub fn set_context_object_opacity(&mut self, opacity: f64) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        if let Some(obj) = self
            .doc
            .write()
            .object_at_path_mut(&menu.source_path, menu.id)
        {
            obj.opacity_override = Some(opacity.clamp(0.0, 1.0));
        }
    }

    pub fn reset_context_object_opacity(&mut self) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        let path = menu.source_path;
        let id = menu.id;
        self.edit_document(move |doc| {
            if let Some(obj) = doc.object_at_path_mut(&path, id) {
                obj.opacity_override = None;
            }
        });
    }
}
