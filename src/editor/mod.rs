//! The shared canvas editor, used by both the overlay (edit mode) and the
//! standalone window.

mod canvas;
mod chrome;
mod objects;

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::store::{self, CanvasObject, Document, ObjectKind, DEFAULT_NOTE_COLOR};

/// Where the editor is hosted.
#[derive(Clone, Copy, PartialEq)]
pub enum EditorHost {
    Overlay,
    Standalone,
}

/// Overlay view mode. The standalone window is always `Edit`.
#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Overview,
    Edit,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Tool {
    #[default]
    Select,
    Note,
    Draw,
}

#[derive(Clone, PartialEq, Default)]
pub enum DragState {
    #[default]
    None,
    Pan {
        start_mouse: (f64, f64),
        start_pan: (f64, f64),
        moved: bool,
    },
    MoveObject {
        id: u64,
        start_world: (f64, f64),
        orig_pos: (f64, f64),
    },
    Resize {
        id: u64,
        dir: &'static str,
        start_world: (f64, f64),
        orig: (f64, f64, f64, f64),
        rotation: f64,
    },
    Rotate {
        id: u64,
        center_screen: (f64, f64),
        start_angle: f64,
        orig_rotation: f64,
    },
    DrawStroke,
}

#[derive(Clone)]
pub struct PendingScreenshot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

#[derive(Clone, Copy)]
pub struct EditorState {
    pub host: EditorHost,
    /// HWND of the attached game window (overlay only), for screenshots.
    pub game_hwnd: Option<isize>,
    pub mode: Signal<ViewMode>,
    pub doc: Signal<Document>,
    pub pan: Signal<(f64, f64)>,
    pub zoom: Signal<f64>,
    pub tool: Signal<Tool>,
    pub selected: Signal<Option<u64>>,
    pub editing_note: Signal<Option<u64>>,
    pub drag: Signal<DragState>,
    pub live_points: Signal<Vec<[f64; 2]>>,
    pub stroke_color: Signal<String>,
    pub stroke_width: Signal<f64>,
    pub menu_open: Signal<bool>,
    pub shot_mode: Signal<bool>,
    pub pending_shot: Signal<Option<PendingScreenshot>>,
    pub toast: Signal<Option<String>>,
    /// Object id -> data URL, for image objects.
    pub image_cache: Signal<HashMap<u64, String>>,
    /// Mounted handle of the canvas viewport, used to restore keyboard focus.
    pub viewport_mount: Signal<Option<std::rc::Rc<MountedData>>>,
}

impl EditorState {
    pub fn create(host: EditorHost, game_hwnd: Option<isize>, doc: Document) -> Self {
        let mut cache = HashMap::new();
        for obj in &doc.objects {
            if let ObjectKind::Image { file } = &obj.kind {
                if let Some(url) = store::image_data_url(&doc, file) {
                    cache.insert(obj.id, url);
                }
            }
        }
        Self {
            host,
            game_hwnd,
            mode: Signal::new(ViewMode::Edit),
            doc: Signal::new(doc),
            pan: Signal::new((0.0, 0.0)),
            zoom: Signal::new(1.0),
            tool: Signal::new(Tool::Select),
            selected: Signal::new(None),
            editing_note: Signal::new(None),
            drag: Signal::new(DragState::None),
            live_points: Signal::new(Vec::new()),
            stroke_color: Signal::new("#7aa2ff".to_string()),
            stroke_width: Signal::new(3.0),
            menu_open: Signal::new(false),
            shot_mode: Signal::new(false),
            pending_shot: Signal::new(None),
            toast: Signal::new(None),
            image_cache: Signal::new(cache),
            viewport_mount: Signal::new(None),
        }
    }

    /// Return keyboard focus to the canvas viewport (e.g. after clicking an
    /// object, so Delete/Escape keep working).
    pub fn focus_canvas(&self) {
        if let Some(mount) = self.viewport_mount.peek().clone() {
            spawn(async move {
                let _ = mount.set_focus(true).await;
            });
        }
    }

    pub fn is_edit_mode(&self) -> bool {
        self.host == EditorHost::Standalone || *self.mode.read() == ViewMode::Edit
    }

    pub fn screen_to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        let (px, py) = *self.pan.read();
        let z = *self.zoom.read();
        ((sx - px) / z, (sy - py) / z)
    }

    pub fn world_to_screen(&self, wx: f64, wy: f64) -> (f64, f64) {
        let (px, py) = *self.pan.read();
        let z = *self.zoom.read();
        (wx * z + px, wy * z + py)
    }

    pub fn select_only(&mut self, id: u64) {
        self.selected.set(Some(id));
        if *self.editing_note.read() != Some(id) {
            self.editing_note.set(None);
        }
        self.doc.write().raise_object(id);
    }

    pub fn deselect(&mut self) {
        self.selected.set(None);
        self.editing_note.set(None);
    }

    /// Enter the region screenshot flow.
    pub fn start_region_screenshot(&mut self) {
        let Some(game_hwnd) = self.game_hwnd else {
            self.show_toast("Screenshots are only available in overlay mode");
            return;
        };

        self.cancel_region_screenshot();
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
                })
            })
            .await;

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
    }

    pub fn add_note(&mut self, wx: f64, wy: f64) {
        let mut doc = self.doc.write();
        let id = doc.alloc_object_id();
        doc.objects.push(CanvasObject {
            id,
            x: wx - 100.0,
            y: wy - 70.0,
            w: 200.0,
            h: 140.0,
            rotation: 0.0,
            kind: ObjectKind::Note {
                text: String::new(),
                color: DEFAULT_NOTE_COLOR.to_string(),
            },
        });
        drop(doc);
        self.selected.set(Some(id));
        self.editing_note.set(Some(id));
        self.tool.set(Tool::Select);
    }

    /// Finalize the in-progress freehand stroke into a Drawing object.
    pub fn finish_stroke(&mut self) {
        let points = std::mem::take(&mut *self.live_points.write());
        if points.len() < 2 {
            return;
        }
        let stroke = self.stroke_color.read().clone();
        let width = *self.stroke_width.read();

        let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
        let (mut max_x, mut max_y) = (f64::MIN, f64::MIN);
        for p in &points {
            min_x = min_x.min(p[0]);
            min_y = min_y.min(p[1]);
            max_x = max_x.max(p[0]);
            max_y = max_y.max(p[1]);
        }
        let pad = width / 2.0 + 2.0;
        let x = min_x - pad;
        let y = min_y - pad;
        let w = (max_x - min_x) + pad * 2.0;
        let h = (max_y - min_y) + pad * 2.0;
        let rel: Vec<[f64; 2]> = points
            .iter()
            .map(|p| [p[0] - x, p[1] - y])
            .collect();

        let mut doc = self.doc.write();
        let id = doc.alloc_object_id();
        doc.objects.push(CanvasObject {
            id,
            x,
            y,
            w,
            h,
            rotation: 0.0,
            kind: ObjectKind::Drawing {
                points: rel,
                vw: w,
                vh: h,
                stroke,
                stroke_width: width,
            },
        });
    }

    /// Insert PNG bytes as an image object centered on the given world point.
    pub fn add_image_png(&mut self, png_bytes: &[u8], wx: f64, wy: f64) {
        let (iw, ih) = match image::load_from_memory(png_bytes) {
            Ok(img) => (img.width() as f64, img.height() as f64),
            Err(_) => {
                self.show_toast("Could not decode image");
                return;
            }
        };

        let file = {
            let doc = self.doc.read();
            match store::save_image_asset(&doc, png_bytes) {
                Ok(f) => f,
                Err(e) => {
                    drop(doc);
                    self.show_toast(&format!("Failed to save image: {e}"));
                    return;
                }
            }
        };

        // Scale down large images for initial placement.
        let scale = (480.0 / iw).min(360.0 / ih).min(1.0);
        let w = (iw * scale).max(40.0);
        let h = (ih * scale).max(30.0);

        let url = {
            use base64::Engine;
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png_bytes)
            )
        };

        let mut doc = self.doc.write();
        let id = doc.alloc_object_id();
        doc.objects.push(CanvasObject {
            id,
            x: wx - w / 2.0,
            y: wy - h / 2.0,
            w,
            h,
            rotation: 0.0,
            kind: ObjectKind::Image { file },
        });
        drop(doc);
        self.image_cache.write().insert(id, url);
        self.selected.set(Some(id));
    }

    /// Paste an image from the system clipboard into the canvas center.
    pub fn paste_image_from_clipboard(&mut self) {
        let mut state = *self;
        spawn(async move {
            let result = tokio::task::spawn_blocking(read_clipboard_png).await;
            match result {
                Ok(Ok(png)) => {
                    let (vw, vh) = viewport_size();
                    let (wx, wy) = state.screen_to_world(vw / 2.0, vh / 2.0);
                    state.add_image_png(&png, wx, wy);
                }
                Ok(Err(e)) => state.show_toast(&e),
                Err(_) => state.show_toast("Clipboard read failed"),
            }
        });
    }

    pub fn delete_selected(&mut self) {
        let sel = *self.selected.read();
        if let Some(id) = sel {
            self.doc.write().remove_object(id);
            self.image_cache.write().remove(&id);
            self.deselect();
        }
    }

    /// Switch to another document of the same game.
    pub fn load_document(&mut self, doc_id: &str) {
        let game_exe = self.doc.read().game_exe.clone();
        // Persist current before switching.
        let _ = store::save_document(&self.doc.read());
        let Some(new_doc) = store::load_document(&game_exe, doc_id) else {
            self.show_toast("Could not load document");
            return;
        };
        let mut cache = HashMap::new();
        for obj in &new_doc.objects {
            if let ObjectKind::Image { file } = &obj.kind {
                if let Some(url) = store::image_data_url(&new_doc, file) {
                    cache.insert(obj.id, url);
                }
            }
        }
        self.image_cache.set(cache);
        self.doc.set(new_doc);
        self.deselect();
        self.pan.set((0.0, 0.0));
        self.zoom.set(1.0);
    }

    pub fn show_toast(&mut self, msg: &str) {
        let mut toast = self.toast;
        toast.set(Some(msg.to_string()));
        let shown = msg.to_string();
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2600)).await;
            if toast.peek().as_deref() == Some(shown.as_str()) {
                toast.set(None);
            }
        });
    }
}

fn png_data_url(png_bytes: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes)
    )
}

fn read_clipboard_png() -> Result<Vec<u8>, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let img = clipboard
        .get_image()
        .map_err(|_| "No image in clipboard".to_string())?;
    crate::platform::capture::encode_png(
        img.bytes.as_ref(),
        img.width as u32,
        img.height as u32,
    )
}

/// Approximate viewport size in CSS pixels (window inner size / scale).
fn viewport_size() -> (f64, f64) {
    let win = dioxus::desktop::window();
    let size = win.inner_size();
    let scale = win.scale_factor();
    (size.width as f64 / scale, size.height as f64 / scale)
}

/// The shared editor surface. Expects an [`EditorState`] in context.
#[component]
pub fn Editor() -> Element {
    let state = use_context::<EditorState>();

    // Debounced autosave whenever the document changes.
    let doc = state.doc;
    let mut save_seq = use_signal(|| 0u64);
    use_effect(move || {
        let snapshot = doc.read().clone();
        let seq = *save_seq.peek() + 1;
        save_seq.set(seq);
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            if *save_seq.peek() == seq {
                let _ = store::save_document(&snapshot);
            }
        });
    });

    let edit = state.is_edit_mode();
    let shot_active = *state.shot_mode.read();
    let opacity = match state.host {
        EditorHost::Standalone => 1.0,
        EditorHost::Overlay => {
            let d = state.doc.read();
            if shot_active {
                1.0
            } else if edit {
                d.edit_opacity
            } else {
                d.overview_opacity
            }
        }
    };

    let host_class = match state.host {
        EditorHost::Overlay => "overlay",
        EditorHost::Standalone => "standalone",
    };
    let mode_class = if edit { "mode-edit" } else { "mode-overview" };
    let toast = state.toast.read().clone();

    rsx! {
        div {
            class: "editor-root {host_class} {mode_class}",
            style: "opacity: {opacity};",
            canvas::Canvas {}
            if edit && !shot_active {
                chrome::Toolbar {}
                chrome::MainMenu {}
                if state.host == EditorHost::Overlay {
                    chrome::BottomBar {}
                }
            }
            if shot_active {
                chrome::ShotOverlay {}
            }
            if let Some(msg) = toast {
                div { class: "editor-toast", "{msg}" }
            }
        }
    }
}
