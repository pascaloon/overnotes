//! Undo/redo transactions: how a user action is bracketed into exactly one
//! history checkpoint, and how a restored checkpoint is applied back to the
//! live signals.

use dioxus::prelude::*;

use super::{EditorState, assets};
use crate::editor::history::Checkpoint;
use crate::editor::interaction::DragState;
use crate::editor::objects::Tool;
use crate::store::Document;

/// Identifies the user action a transaction belongs to. Repeated events of the
/// same kind (typing, dragging a slider) coalesce into one undo step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransactionKind {
    Gesture,
    Wheel,
    /// Inline text editing of one object (note body, container name).
    ObjectText(u64),
    DocumentName,
    ObjectOpacity(u64),
    OverviewOpacity,
    EditOpacity,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HistoryShortcut {
    Undo,
    Redo,
}

fn history_shortcut(key: &Key, ctrl: bool, shift: bool) -> Option<HistoryShortcut> {
    if !ctrl {
        return None;
    }
    match key {
        Key::Character(c) if c.eq_ignore_ascii_case("z") && shift => Some(HistoryShortcut::Redo),
        Key::Character(c) if c.eq_ignore_ascii_case("z") => Some(HistoryShortcut::Undo),
        Key::Character(c) if c.eq_ignore_ascii_case("y") => Some(HistoryShortcut::Redo),
        _ => None,
    }
}

/// Handle history keys from the viewport or an editable document control.
/// Shortcut-capture inputs intentionally stop propagation before calling this.
pub fn handle_history_shortcut(evt: &KeyboardEvent, state: &mut EditorState) -> bool {
    let Some(action) =
        history_shortcut(&evt.key(), evt.modifiers().ctrl(), evt.modifiers().shift())
    else {
        return false;
    };
    evt.prevent_default();
    evt.stop_propagation();
    match action {
        HistoryShortcut::Undo => state.undo(),
        HistoryShortcut::Redo => state.redo(),
    }
    true
}

/// Return the deepest valid graph path, used when restoring snapshots whose
/// selected container was removed by a later edit.
fn validated_graph_path(document: &Document, path: &[u64]) -> Vec<u64> {
    let mut valid = path.to_vec();
    while document.view_at_path(&valid).is_none() {
        if valid.pop().is_none() {
            break;
        }
    }
    valid
}

impl EditorState {
    fn checkpoint(&self) -> Checkpoint {
        Checkpoint::new(
            self.doc.peek().clone(),
            self.current_graph_path.peek().clone(),
            self.selected.peek().clone(),
        )
    }

    pub fn begin_transaction(&mut self, kind: TransactionKind) {
        if self.transaction_kind.peek().as_ref() == Some(&kind) {
            return;
        }
        self.commit_transaction();
        let checkpoint = self.checkpoint();
        self.history.write().begin(checkpoint);
        self.transaction_kind.set(Some(kind));
    }

    pub fn commit_transaction(&mut self) -> bool {
        if self.transaction_kind.peek().is_none() {
            return false;
        }
        let checkpoint = self.checkpoint();
        let changed = self.history.write().commit(&checkpoint);
        self.transaction_kind.set(None);
        changed
    }

    pub fn cancel_transaction(&mut self) {
        let next_object_id = self.doc.peek().next_object_id;
        let baseline = self.history.write().cancel();
        self.transaction_kind.set(None);
        if let Some(mut checkpoint) = baseline {
            checkpoint.document.next_object_id =
                checkpoint.document.next_object_id.max(next_object_id);
            self.restore_checkpoint(checkpoint);
        }
    }

    /// Apply one direct user action as exactly one history transaction.
    pub fn edit_document(&mut self, edit: impl FnOnce(&mut Document)) -> bool {
        self.commit_transaction();
        let before = self.checkpoint();
        self.history.write().begin(before);
        edit(&mut self.doc.write());
        let after = self.checkpoint();
        self.history.write().commit(&after)
    }

    pub fn undo(&mut self) {
        self.commit_transaction();
        let current = self.checkpoint();
        let target = self.history.write().undo(current);
        if let Some(target) = target {
            self.restore_checkpoint(target);
        }
    }

    pub fn redo(&mut self) {
        self.commit_transaction();
        let current = self.checkpoint();
        let target = self.history.write().redo(current);
        if let Some(target) = target {
            self.restore_checkpoint(target);
        }
    }

    fn restore_checkpoint(&mut self, mut checkpoint: Checkpoint) {
        checkpoint.graph_path = validated_graph_path(&checkpoint.document, &checkpoint.graph_path);
        checkpoint.selection.retain(|id| {
            checkpoint
                .document
                .object_at_path(&checkpoint.graph_path, *id)
                .is_some()
        });
        let view = checkpoint
            .document
            .view_at_path(&checkpoint.graph_path)
            .unwrap_or_default();
        let cache = assets::build_image_cache(&checkpoint.document);
        self.doc.set(checkpoint.document);
        self.current_graph_path.set(checkpoint.graph_path);
        self.selected.set(checkpoint.selection);
        self.pan.set(view.pan());
        self.zoom.set(view.zoom);
        self.image_cache.set(cache);
        self.clear_transient_ui();
        self.transaction_kind.set(None);
        let next_sequence = *self.wheel_sequence.peek() + 1;
        self.wheel_sequence.set(next_sequence);
        self.invalidate_async_operations();
        self.tool.set(Tool::Select);
        self.focus_canvas();
    }

    /// Reset everything that must not survive a document/graph swap.
    pub(super) fn clear_transient_ui(&mut self) {
        self.editing.set(None);
        self.drag.set(DragState::None);
        self.live_points.set(Vec::new());
        self.drop_target.set(None);
        self.context_menu.set(None);
        self.menu_open.set(false);
        self.shot_mode.set(false);
        self.pending_shot.set(None);
    }
}

#[cfg(test)]
mod tests {
    use super::{HistoryShortcut, history_shortcut, validated_graph_path};
    use crate::store::{CanvasObject, Document, GraphView, ObjectKind};
    use dioxus::prelude::Key;

    #[test]
    fn recognizes_standard_history_shortcuts() {
        assert_eq!(
            history_shortcut(&Key::Character("z".into()), true, false),
            Some(HistoryShortcut::Undo)
        );
        assert_eq!(
            history_shortcut(&Key::Character("Z".into()), true, true),
            Some(HistoryShortcut::Redo)
        );
        assert_eq!(
            history_shortcut(&Key::Character("y".into()), true, false),
            Some(HistoryShortcut::Redo)
        );
    }

    #[test]
    fn ignores_unmodified_or_unrelated_keys() {
        assert_eq!(
            history_shortcut(&Key::Character("z".into()), false, false),
            None
        );
        assert_eq!(
            history_shortcut(&Key::Character("x".into()), true, false),
            None
        );
    }

    #[test]
    fn graph_path_validation_falls_back_to_deepest_existing_parent() {
        let mut document = Document::new("game.exe", "paths");
        document.objects.push(CanvasObject {
            id: 7,
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
            rotation: 0.0,
            opacity_override: None,
            kind: ObjectKind::Subgraph {
                name: "valid".into(),
                color: "orange".into(),
                view: GraphView::default(),
                objects: Vec::new(),
            },
        });
        assert_eq!(validated_graph_path(&document, &[7]), vec![7]);
        assert_eq!(validated_graph_path(&document, &[7, 99]), vec![7]);
        assert!(validated_graph_path(&document, &[99]).is_empty());
    }
}
