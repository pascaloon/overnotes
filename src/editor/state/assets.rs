//! Image assets and the region-screenshot flow, plus the generation counter
//! that invalidates asynchronous work when the editor context moves on.

use std::collections::HashMap;

use dioxus::prelude::*;

use super::{EditorHost, EditorState, ViewMode};
use crate::store::{self, Document};

/// The editor context an asynchronous operation was started in. Completions
/// that no longer match it are dropped instead of writing into the wrong place.
#[derive(Clone, PartialEq, Debug)]
pub struct AsyncOperationOrigin {
    pub document_id: String,
    pub graph_path: Vec<u64>,
    pub generation: u64,
}

#[derive(Clone)]
pub struct PendingScreenshot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
    pub origin: AsyncOperationOrigin,
}

fn operation_context_matches(
    origin: &AsyncOperationOrigin,
    document_id: &str,
    graph_path: &[u64],
    generation: u64,
) -> bool {
    origin.document_id == document_id
        && origin.graph_path == graph_path
        && origin.generation == generation
}

pub(super) fn build_image_cache(doc: &Document) -> HashMap<u64, String> {
    let mut cache = HashMap::new();
    for (id, file) in doc.image_objects() {
        if let Some(url) = store::image_data_url(doc, &file) {
            cache.insert(id, url);
        }
    }
    cache
}

pub fn png_data_url(png_bytes: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes)
    )
}

impl EditorState {
    pub fn async_operation_origin(&self) -> AsyncOperationOrigin {
        AsyncOperationOrigin {
            document_id: self.doc.peek().id.clone(),
            graph_path: self.current_graph_path.peek().clone(),
            generation: *self.operation_generation.peek(),
        }
    }

    pub fn operation_context_is_current(&self, origin: &AsyncOperationOrigin) -> bool {
        operation_context_matches(
            origin,
            &self.doc.peek().id,
            &self.current_graph_path.peek(),
            *self.operation_generation.peek(),
        )
    }

    pub(super) fn invalidate_async_operations(&mut self) {
        let next = self.operation_generation.peek().wrapping_add(1);
        self.operation_generation.set(next);
    }

    /// Enter the region screenshot flow.
    pub fn start_region_screenshot(&mut self) {
        self.commit_transaction();
        let Some(game_hwnd) = self.game_hwnd else {
            self.show_toast("Screenshots need a game window (not available on Desktop)");
            return;
        };

        self.shot_mode.set(false);
        self.pending_shot.set(None);
        self.invalidate_async_operations();
        let origin = self.async_operation_origin();
        let task_origin = origin.clone();
        let mut state = *self;
        spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let png = crate::platform::capture::capture_window_client(game_hwnd)?;
                let img = image::load_from_memory(&png).map_err(|e| e.to_string())?;
                Ok::<_, String>(PendingScreenshot {
                    width: img.width(),
                    height: img.height(),
                    data_url: png_data_url(&png),
                    png,
                    origin: task_origin,
                })
            })
            .await;

            if !state.operation_context_is_current(&origin) {
                return;
            }
            match result {
                Ok(Ok(shot)) => {
                    if state.host == EditorHost::Overlay {
                        state.mode.set(ViewMode::Edit);
                    }
                    state.menu_open.set(false);
                    state.deselect();
                    state.pending_shot.set(Some(shot));
                    state.shot_mode.set(true);
                }
                Ok(Err(e)) => state.show_toast(&format!("Capture failed: {e}")),
                Err(_) => state.show_toast("Capture failed"),
            }
        });
    }

    pub fn cancel_region_screenshot(&mut self) {
        self.shot_mode.set(false);
        self.pending_shot.set(None);
        self.invalidate_async_operations();
    }

    /// Remove the crop overlay without invalidating the crop operation that is
    /// about to finish asynchronously.
    pub fn take_pending_screenshot(&mut self) -> Option<PendingScreenshot> {
        self.shot_mode.set(false);
        self.pending_shot.write().take()
    }
}

#[cfg(test)]
mod tests {
    use super::{AsyncOperationOrigin, operation_context_matches};

    #[test]
    fn async_completion_requires_exact_document_path_and_generation() {
        let origin = AsyncOperationOrigin {
            document_id: "doc-a".into(),
            graph_path: vec![7, 9],
            generation: 3,
        };
        assert!(operation_context_matches(&origin, "doc-a", &[7, 9], 3));
        assert!(!operation_context_matches(&origin, "doc-b", &[7, 9], 3));
        assert!(!operation_context_matches(&origin, "doc-a", &[7], 3));
        assert!(!operation_context_matches(&origin, "doc-a", &[7, 9], 4));
    }
}
