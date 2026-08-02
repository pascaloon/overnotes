//! Pure, bounded snapshot history for editor document transactions.
//!
//! The history owns only model/navigation data. Live Dioxus state is restored by
//! `EditorState`, keeping this module deterministic and independently testable.

use std::collections::VecDeque;

use crate::store::{CanvasObject, Document};

pub const HISTORY_CAPACITY: usize = 100;
/// Approximate memory retained by undo/redo snapshots, including an active baseline.
pub const HISTORY_BYTE_BUDGET: usize = 64 * 1024 * 1024;

#[derive(Clone, PartialEq, Debug)]
pub struct Checkpoint {
    pub document: Document,
    pub graph_path: Vec<u64>,
    pub selection: Vec<u64>,
}

impl Checkpoint {
    pub fn new(document: Document, graph_path: Vec<u64>, selection: Vec<u64>) -> Self {
        Self {
            document,
            graph_path,
            selection,
        }
    }
}

/// Snapshot history with an optional transaction baseline.
///
/// `begin` is idempotent so nested helpers participate in the outer user action.
/// `commit` records the baseline only if the final checkpoint differs. Consequently
/// no-op actions preserve the redo branch.
#[derive(Clone, Debug)]
pub struct History {
    undo: VecDeque<Checkpoint>,
    redo: Vec<Checkpoint>,
    pending: Option<Checkpoint>,
    capacity: usize,
    byte_budget: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::with_limits(HISTORY_CAPACITY, HISTORY_BYTE_BUDGET)
    }
}

impl History {
    #[cfg(test)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_limits(capacity, HISTORY_BYTE_BUDGET)
    }

    pub fn with_limits(capacity: usize, byte_budget: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            pending: None,
            capacity,
            byte_budget,
        }
    }

    #[cfg(test)]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[cfg(test)]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[cfg(test)]
    /// Whether undo would restore a different checkpoint. An untouched active
    /// transaction does not make undo appear available.
    pub fn can_undo_from(&self, current: &Checkpoint) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|before| before != current)
            || self.can_undo()
    }

    #[cfg(test)]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    #[cfg(test)]
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn begin(&mut self, current: Checkpoint) -> bool {
        if self.pending.is_some() {
            false
        } else {
            self.pending = Some(current);
            true
        }
    }

    /// Commit the active transaction. Returns whether an edit was recorded.
    pub fn commit(&mut self, current: &Checkpoint) -> bool {
        let Some(before) = self.pending.take() else {
            return false;
        };
        if before == *current || self.capacity == 0 {
            return false;
        }
        self.undo.push_back(before);
        self.redo.clear();
        self.trim_to_limits();
        true
    }

    /// Discard the transaction and return its baseline for restoration.
    pub fn cancel(&mut self) -> Option<Checkpoint> {
        self.pending.take()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.pending = None;
    }

    pub fn undo(&mut self, current: Checkpoint) -> Option<Checkpoint> {
        self.pending = None;
        let mut target = self.undo.pop_back()?;
        // IDs are never reused, even when undo restores an older document.
        target.document.next_object_id = target
            .document
            .next_object_id
            .max(current.document.next_object_id);
        self.redo.push(current);
        self.trim_to_limits();
        Some(target)
    }

    pub fn redo(&mut self, current: Checkpoint) -> Option<Checkpoint> {
        self.pending = None;
        let mut target = self.redo.pop()?;
        target.document.next_object_id = target
            .document
            .next_object_id
            .max(current.document.next_object_id);
        self.undo.push_back(current);
        self.trim_to_limits();
        Some(target)
    }

    /// Evict the farthest/oldest checkpoints first. The active transaction is
    /// included in the estimate and is never evicted; otherwise at least one
    /// committed checkpoint is retained even when it alone exceeds the budget.
    fn trim_to_limits(&mut self) {
        while self.undo.len() + self.redo.len() > self.capacity {
            if self.undo.pop_front().is_none() && !self.redo.is_empty() {
                self.redo.remove(0);
            }
        }

        while self.estimated_bytes() > self.byte_budget {
            let committed = self.undo.len() + self.redo.len();
            let minimum_committed = usize::from(self.pending.is_none());
            if committed <= minimum_committed {
                break;
            }
            if self.undo.pop_front().is_none() && !self.redo.is_empty() {
                self.redo.remove(0);
            }
        }
    }

    fn estimated_bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(self.redo.iter())
            .chain(self.pending.iter())
            .map(Checkpoint::estimated_bytes)
            .sum()
    }
}

impl Checkpoint {
    fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + estimated_document_bytes(&self.document)
            + self.graph_path.capacity() * std::mem::size_of::<u64>()
            + self.selection.capacity() * std::mem::size_of::<u64>()
    }
}

fn estimated_document_bytes(document: &Document) -> usize {
    std::mem::size_of::<Document>()
        + document.id.capacity()
        + document.name.capacity()
        + document.game_exe.capacity()
        + estimated_objects_bytes(&document.objects)
}

fn estimated_objects_bytes(objects: &[CanvasObject]) -> usize {
    std::mem::size_of_val(objects)
        + objects
            .iter()
            .map(|object| {
                object.kind.heap_bytes() + estimated_objects_bytes(object.kind.children())
            })
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{CanvasObject, GraphView, ObjectKind};

    fn checkpoint(name: &str) -> Checkpoint {
        let mut doc = Document::new("game.exe", name);
        doc.id = "test".into();
        Checkpoint::new(doc, vec![], vec![])
    }

    fn note(id: u64, text: &str) -> CanvasObject {
        CanvasObject {
            id,
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
            rotation: 0.0,
            opacity_override: None,
            kind: ObjectKind::Note {
                text: text.into(),
                color: "yellow".into(),
                font_size: 15.0,
            },
        }
    }

    #[test]
    fn commit_undo_and_redo_round_trip() {
        let before = checkpoint("before");
        let after = checkpoint("after");
        let mut history = History::default();
        history.begin(before.clone());
        assert!(history.commit(&after));
        assert!(history.can_undo());
        assert_eq!(history.undo(after.clone()).unwrap().document.name, "before");
        assert!(history.can_redo());
        assert_eq!(history.redo(before).unwrap().document.name, "after");
    }

    #[test]
    fn no_op_transaction_adds_nothing_and_preserves_redo() {
        let a = checkpoint("a");
        let b = checkpoint("b");
        let mut history = History::default();
        history.begin(a.clone());
        history.commit(&b);
        let restored = history.undo(b).unwrap();
        assert_eq!(history.redo_len(), 1);
        history.begin(restored.clone());
        assert!(!history.commit(&restored));
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.redo_len(), 1);
    }

    #[test]
    fn actual_edit_invalidates_redo() {
        let a = checkpoint("a");
        let b = checkpoint("b");
        let c = checkpoint("c");
        let mut history = History::default();
        history.begin(a.clone());
        history.commit(&b);
        let restored = history.undo(b).unwrap();
        history.begin(restored);
        assert!(history.commit(&c));
        assert!(!history.can_redo());
    }

    #[test]
    fn capacity_evicts_oldest_checkpoint() {
        let mut history = History::with_capacity(2);
        let mut current = checkpoint("0");
        for i in 1..=3 {
            history.begin(current.clone());
            current.document.name = i.to_string();
            history.commit(&current);
        }
        assert_eq!(history.undo_len(), 2);
        current = history.undo(current).unwrap();
        assert_eq!(current.document.name, "2");
        current = history.undo(current).unwrap();
        assert_eq!(current.document.name, "1");
        assert!(history.undo(current).is_none());
    }

    #[test]
    fn byte_budget_evicts_oldest_but_preserves_newest_checkpoint() {
        let mut history = History::with_limits(100, 900);
        let mut current = checkpoint("0");
        for i in 1..=4 {
            history.begin(current.clone());
            current.document.name = format!("{i}{}", "x".repeat(500));
            history.commit(&current);
        }
        assert_eq!(history.undo_len(), 1);
        assert!(history.undo(current).is_some());

        let mut huge_only = History::with_limits(100, 1);
        let before = checkpoint(&"z".repeat(2_000));
        let mut after = before.clone();
        after.document.name.push('!');
        huge_only.begin(before);
        huge_only.commit(&after);
        assert_eq!(huge_only.undo_len(), 1);
    }

    #[test]
    fn untouched_pending_transaction_does_not_claim_undo() {
        let current = checkpoint("same");
        let mut history = History::default();
        history.begin(current.clone());
        assert!(!history.can_undo_from(&current));
        let mut changed = current;
        changed.document.name.push('!');
        assert!(history.can_undo_from(&changed));
    }

    #[test]
    fn next_object_id_never_moves_back_on_restore() {
        let mut old = checkpoint("old");
        old.document.next_object_id = 2;
        let mut current = checkpoint("current");
        current.document.next_object_id = 9;
        let mut history = History::default();
        history.begin(old);
        history.commit(&current);
        let restored = history.undo(current).unwrap();
        assert_eq!(restored.document.next_object_id, 9);
    }

    #[test]
    fn nested_document_mutations_are_snapshotted() {
        let mut before = checkpoint("nested");
        before.document.next_object_id = 3;
        before.document.objects.push(CanvasObject {
            id: 1,
            x: 0.0,
            y: 0.0,
            w: 120.0,
            h: 100.0,
            rotation: 0.0,
            opacity_override: None,
            kind: ObjectKind::Subgraph {
                name: "folder".into(),
                color: "orange".into(),
                view: GraphView::default(),
                objects: vec![note(2, "before")],
            },
        });
        let mut after = before.clone();
        if let ObjectKind::Note { text, .. } =
            &mut after.document.object_at_path_mut(&[1], 2).unwrap().kind
        {
            *text = "after".into();
        }
        after.graph_path = vec![1];
        after.selection = vec![2];

        let mut history = History::default();
        history.begin(before.clone());
        history.commit(&after);
        assert_eq!(history.undo(after).unwrap(), before);
    }

    #[test]
    fn begin_does_not_replace_an_active_transaction_baseline() {
        let a = checkpoint("a");
        let b = checkpoint("b");
        let c = checkpoint("c");
        let mut history = History::default();

        assert!(history.begin(a.clone()));
        assert!(!history.begin(b));
        assert!(history.commit(&c));
        assert_eq!(history.undo(c), Some(a));
    }

    #[test]
    fn cancel_returns_baseline_without_touching_stacks() {
        let a = checkpoint("a");
        let mut history = History::default();
        history.begin(a.clone());
        assert_eq!(history.cancel(), Some(a));
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn beginning_no_op_or_cancelled_transaction_never_evicts_existing_history() {
        let mut history = History::with_limits(100, 1);
        let a = checkpoint(&"a".repeat(2_000));
        let mut b = a.clone();
        b.document.name.push('b');
        history.begin(a);
        history.commit(&b);
        assert_eq!(history.undo_len(), 1);

        history.begin(b.clone());
        assert!(!history.commit(&b));
        assert_eq!(history.undo_len(), 1);

        history.begin(b);
        history.cancel();
        assert_eq!(history.undo_len(), 1);
    }
}
