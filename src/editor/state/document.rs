//! Document-level commands: creating objects, switching documents, and
//! leaving the overlay.

use dioxus::prelude::*;

use super::{EditorHost, EditorState, TransactionKind, ViewMode, assets};
use crate::store::{self, CanvasObject};

impl EditorState {
    /// Add `object` to the current graph as one history step, assigning it a
    /// fresh id. Returns `None` (and shows `failure`) when the current graph
    /// has gone away, e.g. because it was undone from under the gesture.
    pub fn insert_object(&mut self, mut object: CanvasObject, failure: &str) -> Option<u64> {
        let path = self.current_graph_path.read().clone();
        self.begin_transaction(TransactionKind::Gesture);
        let mut doc = self.doc.write();
        let id = doc.alloc_object_id();
        let Some(objects) = doc.objects_at_path_mut(&path) else {
            drop(doc);
            self.cancel_transaction();
            self.current_graph_path.set(Vec::new());
            self.show_toast(failure);
            return None;
        };
        object.id = id;
        objects.push(object);
        drop(doc);
        self.commit_transaction();
        Some(id)
    }

    /// Switch to another document of the same game.
    pub fn load_document(&mut self, doc_id: &str) {
        let game_exe = self.doc.read().game_exe.clone();
        // Persist current before switching.
        self.commit_transaction();
        self.persist_current_graph_view();
        let _ = store::save_document(&self.doc.read());
        let Some(new_doc) = store::load_document(&game_exe, doc_id) else {
            self.show_toast("Could not load document");
            return;
        };
        self.image_cache.set(assets::build_image_cache(&new_doc));
        self.doc.set(new_doc);
        self.history.write().clear();
        self.transaction_kind.set(None);
        let next_sequence = *self.wheel_sequence.peek() + 1;
        self.wheel_sequence.set(next_sequence);
        self.invalidate_async_operations();
        self.current_graph_path.set(Vec::new());
        self.clear_transient_ui();
        self.deselect();
        self.load_current_graph_view();
    }

    pub fn return_to_overview(&mut self) {
        self.commit_transaction();
        self.deselect();
        self.menu_open.set(false);
        self.cancel_region_screenshot();
        self.mode.set(ViewMode::Overview);
    }

    pub fn toggle_overview_hidden(&mut self) {
        let next = !*self.overview_hidden.peek();
        self.overview_hidden.set(next);
        self.return_to_overview();
    }

    /// Move the overlay's document into a standalone window.
    pub fn detach_overlay(&mut self) {
        if self.host != EditorHost::Overlay {
            return;
        }
        self.commit_transaction();
        let doc = self.doc.peek().clone();
        let game_exe = doc.game_exe.clone();
        let doc_id = doc.id.clone();
        let doc_name = doc.name.clone();
        let _ = store::save_document(&doc);
        let dom = VirtualDom::new_with_props(
            crate::ui::standalone::StandaloneRoot,
            crate::ui::standalone::StandaloneRootProps { game_exe, doc_id },
        );
        let _ = dioxus::desktop::window().new_window(dom, crate::ui::standalone_config(&doc_name));
        dioxus::desktop::window().close();
    }

    /// Save and close the overlay without opening a standalone window.
    pub fn quit_overlay(&mut self) {
        if self.host != EditorHost::Overlay {
            return;
        }
        self.commit_transaction();
        let _ = store::save_document(&self.doc.peek());
        dioxus::desktop::window().close();
    }
}
