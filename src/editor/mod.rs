//! The shared canvas editor, used by both the overlay (edit mode) and the
//! standalone window.

mod canvas;
mod chrome;
mod objects;

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::store::{
    self, CanvasObject, DEFAULT_NOTE_COLOR, DEFAULT_SUBGRAPH_COLOR, Document, GraphView, ObjectKind,
};

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
    Subgraph,
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

#[derive(Clone, PartialEq)]
pub struct ObjectContextMenu {
    pub id: u64,
    pub source_path: Vec<u64>,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, PartialEq)]
pub struct SubgraphDropTarget {
    pub id: u64,
    pub name: String,
    pub screen_pos: (f64, f64),
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
    pub selected: Signal<Vec<u64>>,
    pub editing_note: Signal<Option<u64>>,
    pub editing_subgraph: Signal<Option<u64>>,
    pub current_graph_path: Signal<Vec<u64>>,
    pub drag: Signal<DragState>,
    pub live_points: Signal<Vec<[f64; 2]>>,
    pub stroke_color: Signal<String>,
    pub stroke_width: Signal<f64>,
    pub menu_open: Signal<bool>,
    pub shot_mode: Signal<bool>,
    pub pending_shot: Signal<Option<PendingScreenshot>>,
    pub toast: Signal<Option<String>>,
    pub context_menu: Signal<Option<ObjectContextMenu>>,
    pub drop_target: Signal<Option<SubgraphDropTarget>>,
    /// Object id -> data URL, for image objects.
    pub image_cache: Signal<HashMap<u64, String>>,
    /// Mounted handle of the canvas viewport, used to restore keyboard focus.
    pub viewport_mount: Signal<Option<std::rc::Rc<MountedData>>>,
}

impl EditorState {
    pub fn create(host: EditorHost, game_hwnd: Option<isize>, doc: Document) -> Self {
        let cache = build_image_cache(&doc);
        let view = doc.view_at_path(&[]).unwrap_or_default();
        Self {
            host,
            game_hwnd,
            mode: Signal::new(ViewMode::Edit),
            doc: Signal::new(doc),
            pan: Signal::new(view.pan()),
            zoom: Signal::new(view.zoom),
            tool: Signal::new(Tool::Select),
            selected: Signal::new(Vec::new()),
            editing_note: Signal::new(None),
            editing_subgraph: Signal::new(None),
            current_graph_path: Signal::new(Vec::new()),
            drag: Signal::new(DragState::None),
            live_points: Signal::new(Vec::new()),
            stroke_color: Signal::new("#7aa2ff".to_string()),
            stroke_width: Signal::new(3.0),
            menu_open: Signal::new(false),
            shot_mode: Signal::new(false),
            pending_shot: Signal::new(None),
            toast: Signal::new(None),
            context_menu: Signal::new(None),
            drop_target: Signal::new(None),
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

    pub fn set_pan(&mut self, pan: (f64, f64)) {
        self.pan.set(pan);
        self.persist_current_graph_view();
    }

    pub fn set_camera(&mut self, pan: (f64, f64), zoom: f64) {
        self.pan.set(pan);
        self.zoom.set(zoom);
        self.persist_current_graph_view();
    }

    pub fn persist_current_graph_view(&mut self) {
        let path = self.current_graph_path.read().clone();
        let pan = *self.pan.read();
        let zoom = *self.zoom.read();
        self.doc
            .write()
            .set_view_at_path(&path, GraphView::new(pan, zoom));
    }

    fn load_current_graph_view(&mut self) {
        let path = self.current_graph_path.read().clone();
        let view = self.doc.read().view_at_path(&path).unwrap_or_default();
        self.pan.set(view.pan());
        self.zoom.set(view.zoom);
    }

    pub fn select_only(&mut self, id: u64) {
        self.selected.set(vec![id]);
        if *self.editing_note.read() != Some(id) {
            self.editing_note.set(None);
        }
        if *self.editing_subgraph.read() != Some(id) {
            self.editing_subgraph.set(None);
        }
    }

    pub fn set_selection(&mut self, ids: Vec<u64>) {
        self.selected.set(ids);
        self.editing_note.set(None);
        self.editing_subgraph.set(None);
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
        self.editing_note.set(None);
        self.editing_subgraph.set(None);
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
        self.editing_note.set(None);
        self.editing_subgraph.set(None);
        self.menu_open.set(false);
        self.context_menu.set(Some(ObjectContextMenu {
            id,
            source_path,
            x,
            y,
        }));
    }

    pub fn move_context_object_up(&mut self) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        self.doc
            .write()
            .move_object_up_at_path(&menu.source_path, menu.id);
        self.close_context_menu();
    }

    pub fn move_context_object_to_top(&mut self) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        self.doc
            .write()
            .move_object_to_top_at_path(&menu.source_path, menu.id);
        self.close_context_menu();
    }

    pub fn move_context_object_down(&mut self) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        self.doc
            .write()
            .move_object_down_at_path(&menu.source_path, menu.id);
        self.close_context_menu();
    }

    pub fn move_context_object_to_bottom(&mut self) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        self.doc
            .write()
            .move_object_to_bottom_at_path(&menu.source_path, menu.id);
        self.close_context_menu();
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
        if let Some(obj) = self
            .doc
            .write()
            .object_at_path_mut(&menu.source_path, menu.id)
        {
            obj.opacity_override = None;
        }
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
        let path = self.current_graph_path.read().clone();
        let mut doc = self.doc.write();
        let id = doc.alloc_object_id();
        let Some(objects) = doc.objects_at_path_mut(&path) else {
            drop(doc);
            self.current_graph_path.set(Vec::new());
            self.show_toast("Could not create note in this subgraph");
            return;
        };
        objects.push(CanvasObject {
            id,
            x: wx - 100.0,
            y: wy - 70.0,
            w: 200.0,
            h: 140.0,
            rotation: 0.0,
            opacity_override: None,
            kind: ObjectKind::Note {
                text: String::new(),
                color: DEFAULT_NOTE_COLOR.to_string(),
            },
        });
        drop(doc);
        self.selected.set(vec![id]);
        self.editing_note.set(Some(id));
        self.editing_subgraph.set(None);
        self.tool.set(Tool::Select);
    }

    pub fn add_subgraph(&mut self, wx: f64, wy: f64) {
        let path = self.current_graph_path.read().clone();
        let mut doc = self.doc.write();
        let id = doc.alloc_object_id();
        let Some(objects) = doc.objects_at_path_mut(&path) else {
            drop(doc);
            self.current_graph_path.set(Vec::new());
            self.show_toast("Could not create subgraph here");
            return;
        };
        objects.push(CanvasObject {
            id,
            x: wx - 60.0,
            y: wy - 45.0,
            w: 120.0,
            h: 110.0,
            rotation: 0.0,
            opacity_override: None,
            kind: ObjectKind::Subgraph {
                name: "New subgraph".to_string(),
                color: DEFAULT_SUBGRAPH_COLOR.to_string(),
                view: GraphView::default(),
                objects: Vec::new(),
            },
        });
        drop(doc);
        self.selected.set(vec![id]);
        self.editing_note.set(None);
        self.editing_subgraph.set(Some(id));
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
        let rel: Vec<[f64; 2]> = points.iter().map(|p| [p[0] - x, p[1] - y]).collect();

        let path = self.current_graph_path.read().clone();
        let mut doc = self.doc.write();
        let id = doc.alloc_object_id();
        let Some(objects) = doc.objects_at_path_mut(&path) else {
            drop(doc);
            self.current_graph_path.set(Vec::new());
            self.show_toast("Could not finish drawing in this subgraph");
            return;
        };
        objects.push(CanvasObject {
            id,
            x,
            y,
            w,
            h,
            rotation: 0.0,
            opacity_override: None,
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

        let path = self.current_graph_path.read().clone();
        let mut doc = self.doc.write();
        let id = doc.alloc_object_id();
        let Some(objects) = doc.objects_at_path_mut(&path) else {
            drop(doc);
            self.current_graph_path.set(Vec::new());
            self.show_toast("Could not add image in this subgraph");
            return;
        };
        objects.push(CanvasObject {
            id,
            x: wx - w / 2.0,
            y: wy - h / 2.0,
            w,
            h,
            rotation: 0.0,
            opacity_override: None,
            kind: ObjectKind::Image { file },
        });
        drop(doc);
        self.image_cache.write().insert(id, url);
        self.selected.set(vec![id]);
        self.editing_note.set(None);
        self.editing_subgraph.set(None);
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
        let selected = self.selected.read().clone();
        if !selected.is_empty() {
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
        }
    }

    pub fn enter_subgraph(&mut self, id: u64) {
        self.persist_current_graph_view();
        let path = self.current_graph_path.read().clone();
        if !matches!(
            self.doc.read().object_at_path(&path, id).map(|o| &o.kind),
            Some(ObjectKind::Subgraph { .. })
        ) {
            return;
        }
        let mut next = path;
        next.push(id);
        self.current_graph_path.set(next);
        self.restore_graph_view();
    }

    pub fn rename_selected_subgraph(&mut self) {
        let Some(id) = self.single_selected() else {
            return;
        };
        let path = self.current_graph_path.read().clone();
        if !matches!(
            self.doc.read().object_at_path(&path, id).map(|o| &o.kind),
            Some(ObjectKind::Subgraph { .. })
        ) {
            return;
        }
        self.editing_note.set(None);
        self.editing_subgraph.set(Some(id));
    }

    pub fn rename_context_subgraph(&mut self) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        if !matches!(
            self.doc
                .read()
                .object_at_path(&menu.source_path, menu.id)
                .map(|o| &o.kind),
            Some(ObjectKind::Subgraph { .. })
        ) {
            self.close_context_menu();
            return;
        }
        self.persist_current_graph_view();
        self.current_graph_path.set(menu.source_path.clone());
        self.load_current_graph_view();
        self.selected.set(vec![menu.id]);
        self.editing_note.set(None);
        self.editing_subgraph.set(Some(menu.id));
        self.close_context_menu();
        self.focus_canvas();
    }

    pub fn move_context_object_to_graph(&mut self, target_path: Vec<u64>) {
        let Some(menu) = self.context_menu.read().clone() else {
            return;
        };
        let moved = self
            .doc
            .write()
            .move_object_to_graph(&menu.source_path, menu.id, &target_path);
        self.close_context_menu();
        if moved {
            if self.selected.read().contains(&menu.id) {
                self.deselect();
            }
        } else {
            self.show_toast("Could not move object");
        }
    }

    pub fn navigate_to_graph_depth(&mut self, depth: usize) {
        self.persist_current_graph_view();
        let mut path = self.current_graph_path.read().clone();
        path.truncate(depth);
        self.current_graph_path.set(path);
        self.restore_graph_view();
    }

    fn restore_graph_view(&mut self) {
        self.deselect();
        self.drag.set(DragState::None);
        self.drop_target.set(None);
        self.menu_open.set(false);
        self.close_context_menu();
        self.load_current_graph_view();
        self.tool.set(Tool::Select);
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
                            let obj_right = obj.x + obj.w;
                            let obj_bottom = obj.y + obj.h;
                            obj.x <= right
                                && obj_right >= left
                                && obj.y <= bottom
                                && obj_bottom >= top
                        })
                        .map(|obj| obj.id)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        self.set_selection(ids);
    }

    pub fn update_subgraph_drop_target(&mut self, moving_ids: &[u64], screen_pos: (f64, f64)) {
        let path = self.current_graph_path.read().clone();
        let cursor_world = self.screen_to_world(screen_pos.0, screen_pos.1);
        let target = self
            .find_subgraph_drop_target(&path, moving_ids, cursor_world)
            .map(|mut target| {
                target.screen_pos = screen_pos;
                target
            });
        self.drop_target.set(target);
    }

    fn find_subgraph_drop_target(
        &self,
        path: &[u64],
        moving_ids: &[u64],
        cursor_world: (f64, f64),
    ) -> Option<SubgraphDropTarget> {
        let doc = self.doc.read();
        let (cx, cy) = cursor_world;
        doc.objects_at_path(path).and_then(|objects| {
            objects.iter().rev().find_map(|candidate| {
                if moving_ids.contains(&candidate.id)
                    || cx < candidate.x
                    || cx > candidate.x + candidate.w
                    || cy < candidate.y
                    || cy > candidate.y + candidate.h
                {
                    return None;
                }
                if let ObjectKind::Subgraph { name, .. } = &candidate.kind {
                    Some(SubgraphDropTarget {
                        id: candidate.id,
                        name: name.clone(),
                        screen_pos: (0.0, 0.0),
                    })
                } else {
                    None
                }
            })
        })
    }

    pub fn try_drop_objects_into_subgraph(&mut self, ids: &[u64]) -> bool {
        let path = self.current_graph_path.read().clone();
        let target_id = self.drop_target.peek().as_ref().map(|target| target.id);

        let Some(target_id) = target_id else {
            return false;
        };
        if self
            .doc
            .write()
            .move_objects_into_subgraph(&path, ids, target_id)
        {
            self.deselect();
            return true;
        }
        false
    }

    /// Switch to another document of the same game.
    pub fn load_document(&mut self, doc_id: &str) {
        let game_exe = self.doc.read().game_exe.clone();
        // Persist current before switching.
        self.persist_current_graph_view();
        let _ = store::save_document(&self.doc.read());
        let Some(new_doc) = store::load_document(&game_exe, doc_id) else {
            self.show_toast("Could not load document");
            return;
        };
        let cache = build_image_cache(&new_doc);
        self.image_cache.set(cache);
        self.doc.set(new_doc);
        self.current_graph_path.set(Vec::new());
        self.deselect();
        self.load_current_graph_view();
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

fn build_image_cache(doc: &Document) -> HashMap<u64, String> {
    let mut cache = HashMap::new();
    for (id, file) in doc.image_objects() {
        if let Some(url) = store::image_data_url(doc, &file) {
            cache.insert(id, url);
        }
    }
    cache
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
    crate::platform::capture::encode_png(img.bytes.as_ref(), img.width as u32, img.height as u32)
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
                1.0
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
                chrome::Breadcrumbs {}
                chrome::MainMenu {}
                chrome::ObjectContextMenu {}
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
