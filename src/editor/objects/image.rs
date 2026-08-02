//! Pasted and captured images. The PNG lives next to the document on disk;
//! the canvas renders a cached data URL.

use dioxus::prelude::*;

use super::registry::{ActionSpec, CanvasElement, ObjectCtx};
use crate::editor::state::{AsyncOperationOrigin, png_data_url, viewport_size};
use crate::editor::{EditorState, TransactionKind};
use crate::store::{self, CanvasObject, ObjectKind};

/// Largest initial placement of a freshly inserted image, in world units.
const MAX_PLACED: (f64, f64) = (480.0, 360.0);
const MIN_PLACED: (f64, f64) = (40.0, 30.0);

pub struct Image;

impl CanvasElement for Image {
    fn matches(&self, kind: &ObjectKind) -> bool {
        matches!(kind, ObjectKind::Image { .. })
    }

    fn body(&self, cx: &ObjectCtx) -> Element {
        let url = cx
            .state
            .image_cache
            .read()
            .get(&cx.id)
            .cloned()
            .unwrap_or_default();
        rsx! {
            img { class: "obj-img", src: "{url}", draggable: "false" }
        }
    }

    fn locks_aspect_ratio(&self) -> bool {
        true
    }

    fn action(&self) -> Option<ActionSpec> {
        Some(ActionSpec {
            tooltip: "Paste image from clipboard (Ctrl+V)",
            icon,
            run: paste_from_clipboard,
        })
    }

    fn style(&self) -> Option<Asset> {
        Some(asset!("/assets/objects/image.css"))
    }
}

fn icon() -> Element {
    rsx! {
        svg {
            width: "20",
            height: "20",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linejoin: "round",
            rect { x: "3", y: "4", width: "18", height: "16", rx: "2" }
            circle { cx: "9", cy: "10", r: "2" }
            path { d: "M3 17 L9 13 L13 16 L17 12 L21 15" }
        }
    }
}

/// Paste an image into the canvas center as it existed when paste began.
pub fn paste_from_clipboard(state: &mut EditorState) {
    let origin = state.async_operation_origin();
    let (vw, vh) = viewport_size();
    let coords = state.screen_to_world(vw / 2.0, vh / 2.0);
    let mut state = *state;
    spawn(async move {
        let result = tokio::task::spawn_blocking(read_clipboard_png).await;
        if !state.operation_context_is_current(&origin) {
            return;
        }
        match result {
            Ok(Ok(png)) => {
                insert_png(&mut state, &origin, coords, &png);
            }
            Ok(Err(e)) => state.show_toast(&e),
            Err(_) => state.show_toast("Clipboard read failed"),
        }
    });
}

fn read_clipboard_png() -> Result<Vec<u8>, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let img = clipboard
        .get_image()
        .map_err(|_| "No image in clipboard".to_string())?;
    crate::platform::capture::encode_png(img.bytes.as_ref(), img.width as u32, img.height as u32)
}

/// Insert PNG bytes at the exact graph/world position captured when the
/// asynchronous operation started. Stale completions are ignored.
pub fn insert_png(
    state: &mut EditorState,
    origin: &AsyncOperationOrigin,
    coords: (f64, f64),
    png_bytes: &[u8],
) -> bool {
    if !state.operation_context_is_current(origin)
        || state
            .doc
            .peek()
            .objects_at_path(&origin.graph_path)
            .is_none()
    {
        return false;
    }

    let (iw, ih) = match image::load_from_memory(png_bytes) {
        Ok(img) => (img.width() as f64, img.height() as f64),
        Err(_) => {
            state.show_toast("Could not decode image");
            return false;
        }
    };

    let file = {
        let doc = state.doc.read();
        match store::save_image_asset(&doc, png_bytes) {
            Ok(file) => file,
            Err(e) => {
                drop(doc);
                state.show_toast(&format!("Failed to save image: {e}"));
                return false;
            }
        }
    };

    // Scale down large images for initial placement.
    let scale = (MAX_PLACED.0 / iw).min(MAX_PLACED.1 / ih).min(1.0);
    let w = (iw * scale).max(MIN_PLACED.0);
    let h = (ih * scale).max(MIN_PLACED.1);
    let url = png_data_url(png_bytes);

    state.begin_transaction(TransactionKind::Gesture);
    let mut doc = state.doc.write();
    let id = doc.alloc_object_id();
    let Some(objects) = doc.objects_at_path_mut(&origin.graph_path) else {
        drop(doc);
        state.cancel_transaction();
        return false;
    };
    objects.push(CanvasObject {
        id,
        x: coords.0 - w / 2.0,
        y: coords.1 - h / 2.0,
        w,
        h,
        rotation: 0.0,
        opacity_override: None,
        kind: ObjectKind::Image { file },
    });
    drop(doc);
    state.commit_transaction();
    state.image_cache.write().insert(id, url);
    state.selected.set(vec![id]);
    state.editing.set(None);
    true
}
