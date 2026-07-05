//! Document model + JSON persistence.
//!
//! Documents live in `%APPDATA%\overnotes\documents\<game_exe>\<doc_id>\doc.json`,
//! with pasted/captured images saved as PNG files in the same folder.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_TOGGLE_SHORTCUT: &str = "ctrl+shift+KeyE";
pub const DEFAULT_TOGGLE_SHORTCUT_LABEL: &str = "Ctrl+Shift+E";
pub const DEFAULT_SCREENSHOT_SHORTCUT: &str = "ctrl+shift+KeyS";
pub const DEFAULT_SCREENSHOT_SHORTCUT_LABEL: &str = "Ctrl+Shift+S";
pub const DEFAULT_NOTE_COLOR: &str = "#e8c95c";
pub const DEFAULT_NOTE_FONT_SIZE: f64 = 15.0;
pub const DEFAULT_SUBGRAPH_COLOR: &str = "#d8a84d";
pub const NOTE_COLORS: [&str; 5] = ["#e8c95c", "#8fd18a", "#8db8f2", "#eb9bb9", "transparent"];
pub const SUBGRAPH_COLORS: [&str; 5] = ["#d8a84d", "#7aa2ff", "#7fd48a", "#c792ea", "#ff8a65"];
pub const STROKE_COLORS: [&str; 6] = [
    "#ffffff", "#7aa2ff", "#ff6b6b", "#7fd48a", "#ffd166", "#c792ea",
];

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AppSettings {
    #[serde(default = "default_toggle_shortcut")]
    pub overlay_toggle_shortcut: KeyboardShortcut,
    #[serde(default = "default_screenshot_shortcut")]
    pub overlay_screenshot_shortcut: KeyboardShortcut,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            overlay_toggle_shortcut: default_toggle_shortcut(),
            overlay_screenshot_shortcut: default_screenshot_shortcut(),
        }
    }
}

impl AppSettings {
    fn normalized(mut self) -> Self {
        if !is_supported_shortcut(&self.overlay_toggle_shortcut) {
            self.overlay_toggle_shortcut = default_toggle_shortcut();
        }
        if !is_supported_shortcut(&self.overlay_screenshot_shortcut)
            || self.overlay_screenshot_shortcut.accelerator
                == self.overlay_toggle_shortcut.accelerator
        {
            self.overlay_screenshot_shortcut = default_screenshot_shortcut();
        }
        if self.overlay_screenshot_shortcut.accelerator == self.overlay_toggle_shortcut.accelerator
        {
            self.overlay_toggle_shortcut = default_toggle_shortcut();
        }
        self
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct KeyboardShortcut {
    pub accelerator: String,
    pub label: String,
}

impl KeyboardShortcut {
    pub fn new(accelerator: String, label: String) -> Option<Self> {
        let shortcut = Self { accelerator, label };
        is_supported_shortcut(&shortcut).then_some(shortcut)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub game_exe: String,
    pub overview_opacity: f64,
    pub edit_opacity: f64,
    #[serde(default)]
    pub root_view: GraphView,
    #[serde(default)]
    pub next_object_id: u64,
    #[serde(default)]
    pub objects: Vec<CanvasObject>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct GraphView {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

impl Default for GraphView {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl GraphView {
    pub fn new(pan: (f64, f64), zoom: f64) -> Self {
        Self {
            pan_x: pan.0,
            pan_y: pan.1,
            zoom,
        }
    }

    pub fn pan(self) -> (f64, f64) {
        (self.pan_x, self.pan_y)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct CanvasObject {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Rotation in degrees, applied around the object's center.
    pub rotation: f64,
    /// Per-object override for overview opacity. `None` follows the document setting.
    #[serde(default)]
    pub opacity_override: Option<f64>,
    pub kind: ObjectKind,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type")]
pub enum ObjectKind {
    Note {
        text: String,
        color: String,
        #[serde(default = "default_note_font_size")]
        font_size: f64,
    },
    Drawing {
        /// Points in the drawing's own coordinate space (`0..vw`, `0..vh`).
        points: Vec<[f64; 2]>,
        /// Original (viewBox) size; resizing the object scales the stroke.
        vw: f64,
        vh: f64,
        stroke: String,
        stroke_width: f64,
    },
    Image {
        /// PNG filename inside the document folder.
        file: String,
    },
    Subgraph {
        name: String,
        color: String,
        #[serde(default)]
        view: GraphView,
        #[serde(default)]
        objects: Vec<CanvasObject>,
    },
}

fn default_note_font_size() -> f64 {
    DEFAULT_NOTE_FONT_SIZE
}

#[derive(Clone, PartialEq, Debug)]
pub struct DocMeta {
    pub id: String,
    pub name: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SubgraphDestination {
    pub path: Vec<u64>,
    pub label: String,
}

impl Document {
    pub fn new(game_exe: &str, name: &str) -> Self {
        Self {
            id: fresh_id(),
            name: name.to_string(),
            game_exe: game_exe.to_string(),
            overview_opacity: 0.6,
            edit_opacity: 0.95,
            root_view: GraphView::default(),
            next_object_id: 1,
            objects: Vec::new(),
        }
    }

    pub fn alloc_object_id(&mut self) -> u64 {
        let id = self.next_object_id;
        self.next_object_id += 1;
        id
    }

    pub fn objects_at_path(&self, path: &[u64]) -> Option<&[CanvasObject]> {
        let mut objects = self.objects.as_slice();
        for id in path {
            let obj = objects.iter().find(|o| o.id == *id)?;
            let ObjectKind::Subgraph { objects: child, .. } = &obj.kind else {
                return None;
            };
            objects = child.as_slice();
        }
        Some(objects)
    }

    pub fn objects_at_path_mut(&mut self, path: &[u64]) -> Option<&mut Vec<CanvasObject>> {
        let mut objects = &mut self.objects;
        for id in path {
            let pos = objects.iter().position(|o| o.id == *id)?;
            let ObjectKind::Subgraph { objects: child, .. } = &mut objects[pos].kind else {
                return None;
            };
            objects = child;
        }
        Some(objects)
    }

    pub fn view_at_path(&self, path: &[u64]) -> Option<GraphView> {
        if path.is_empty() {
            return Some(self.root_view);
        }
        let mut objects = self.objects.as_slice();
        for (i, id) in path.iter().enumerate() {
            let obj = objects.iter().find(|o| o.id == *id)?;
            let ObjectKind::Subgraph {
                view,
                objects: child,
                ..
            } = &obj.kind
            else {
                return None;
            };
            if i + 1 == path.len() {
                return Some(*view);
            }
            objects = child.as_slice();
        }
        None
    }

    pub fn set_view_at_path(&mut self, path: &[u64], view: GraphView) -> bool {
        if path.is_empty() {
            self.root_view = view;
            return true;
        }
        let Some(id) = path.last().copied() else {
            return false;
        };
        let parent_path = &path[..path.len().saturating_sub(1)];
        let Some(obj) = self.object_at_path_mut(parent_path, id) else {
            return false;
        };
        let ObjectKind::Subgraph { view: target, .. } = &mut obj.kind else {
            return false;
        };
        *target = view;
        true
    }

    pub fn object_at_path(&self, path: &[u64], id: u64) -> Option<&CanvasObject> {
        self.objects_at_path(path)?.iter().find(|o| o.id == id)
    }

    pub fn object_at_path_mut(&mut self, path: &[u64], id: u64) -> Option<&mut CanvasObject> {
        self.objects_at_path_mut(path)?
            .iter_mut()
            .find(|o| o.id == id)
    }

    pub fn remove_object_at_path(&mut self, path: &[u64], id: u64) -> Option<CanvasObject> {
        let objects = self.objects_at_path_mut(path)?;
        let pos = objects.iter().position(|o| o.id == id)?;
        Some(objects.remove(pos))
    }

    pub fn move_object_up_at_path(&mut self, path: &[u64], id: u64) -> bool {
        let Some(objects) = self.objects_at_path_mut(path) else {
            return false;
        };
        let Some(pos) = objects.iter().position(|o| o.id == id) else {
            return false;
        };
        if pos + 1 >= objects.len() {
            return false;
        }
        objects.swap(pos, pos + 1);
        true
    }

    pub fn move_object_to_top_at_path(&mut self, path: &[u64], id: u64) -> bool {
        let Some(objects) = self.objects_at_path_mut(path) else {
            return false;
        };
        let Some(pos) = objects.iter().position(|o| o.id == id) else {
            return false;
        };
        if pos + 1 >= objects.len() {
            return false;
        }
        let obj = objects.remove(pos);
        objects.push(obj);
        true
    }

    pub fn move_object_down_at_path(&mut self, path: &[u64], id: u64) -> bool {
        let Some(objects) = self.objects_at_path_mut(path) else {
            return false;
        };
        let Some(pos) = objects.iter().position(|o| o.id == id) else {
            return false;
        };
        if pos == 0 {
            return false;
        }
        objects.swap(pos, pos - 1);
        true
    }

    pub fn move_object_to_bottom_at_path(&mut self, path: &[u64], id: u64) -> bool {
        let Some(objects) = self.objects_at_path_mut(path) else {
            return false;
        };
        let Some(pos) = objects.iter().position(|o| o.id == id) else {
            return false;
        };
        if pos == 0 {
            return false;
        }
        let obj = objects.remove(pos);
        objects.insert(0, obj);
        true
    }

    pub fn move_objects_into_subgraph(
        &mut self,
        path: &[u64],
        ids: &[u64],
        target_id: u64,
    ) -> bool {
        if ids.is_empty() || ids.contains(&target_id) {
            return false;
        }
        let Some(objects) = self.objects_at_path_mut(path) else {
            return false;
        };
        if !matches!(
            objects.iter().find(|o| o.id == target_id).map(|o| &o.kind),
            Some(ObjectKind::Subgraph { .. })
        ) {
            return false;
        }
        let mut moved = Vec::new();
        let mut i = 0;
        while i < objects.len() {
            if ids.contains(&objects[i].id) {
                moved.push(objects.remove(i));
            } else {
                i += 1;
            }
        }
        if moved.is_empty() {
            return false;
        }
        let Some(target) = objects.iter_mut().find(|o| o.id == target_id) else {
            objects.extend(moved);
            return false;
        };
        let ObjectKind::Subgraph { objects: child, .. } = &mut target.kind else {
            objects.extend(moved);
            return false;
        };
        child.extend(moved);
        true
    }

    pub fn move_object_to_graph(
        &mut self,
        source_path: &[u64],
        id: u64,
        target_path: &[u64],
    ) -> bool {
        if source_path == target_path || target_path.contains(&id) {
            return false;
        }
        let Some(obj) = self.remove_object_at_path(source_path, id) else {
            return false;
        };
        let Some(target_objects) = self.objects_at_path_mut(target_path) else {
            let Some(source_objects) = self.objects_at_path_mut(source_path) else {
                return false;
            };
            source_objects.push(obj);
            return false;
        };
        target_objects.push(obj);
        true
    }

    pub fn breadcrumb_names(&self, path: &[u64]) -> Vec<String> {
        let mut names = Vec::new();
        let mut objects = self.objects.as_slice();
        for id in path {
            let Some(obj) = objects.iter().find(|o| o.id == *id) else {
                break;
            };
            let ObjectKind::Subgraph {
                name,
                objects: child,
                ..
            } = &obj.kind
            else {
                break;
            };
            names.push(name.clone());
            objects = child.as_slice();
        }
        names
    }

    pub fn image_objects(&self) -> Vec<(u64, String)> {
        let mut out = Vec::new();
        collect_image_objects(&self.objects, &mut out);
        out
    }

    pub fn subgraph_destinations(
        &self,
        moving_id: u64,
        source_path: &[u64],
    ) -> Vec<SubgraphDestination> {
        let mut out = Vec::new();
        if !source_path.is_empty() {
            out.push(SubgraphDestination {
                path: Vec::new(),
                label: "Root".to_string(),
            });
        }
        collect_subgraph_destinations(
            &self.objects,
            &mut Vec::new(),
            &mut Vec::new(),
            moving_id,
            source_path,
            &mut out,
        );
        out
    }
}

impl CanvasObject {
    pub fn image_ids_recursive(&self) -> Vec<u64> {
        let mut out = Vec::new();
        collect_image_ids_in_object(self, &mut out);
        out
    }
}

fn collect_image_objects(objects: &[CanvasObject], out: &mut Vec<(u64, String)>) {
    for obj in objects {
        match &obj.kind {
            ObjectKind::Image { file } => out.push((obj.id, file.clone())),
            ObjectKind::Subgraph { objects, .. } => collect_image_objects(objects, out),
            ObjectKind::Note { .. } | ObjectKind::Drawing { .. } => {}
        }
    }
}

fn collect_image_ids_in_object(obj: &CanvasObject, out: &mut Vec<u64>) {
    match &obj.kind {
        ObjectKind::Image { .. } => out.push(obj.id),
        ObjectKind::Subgraph { objects, .. } => {
            for child in objects {
                collect_image_ids_in_object(child, out);
            }
        }
        ObjectKind::Note { .. } | ObjectKind::Drawing { .. } => {}
    }
}

fn collect_subgraph_destinations(
    objects: &[CanvasObject],
    path: &mut Vec<u64>,
    names: &mut Vec<String>,
    moving_id: u64,
    source_path: &[u64],
    out: &mut Vec<SubgraphDestination>,
) {
    for obj in objects {
        if let ObjectKind::Subgraph {
            name,
            objects: child,
            ..
        } = &obj.kind
        {
            path.push(obj.id);
            names.push(name.clone());

            if obj.id != moving_id && !path.contains(&moving_id) && path.as_slice() != source_path {
                out.push(SubgraphDestination {
                    path: path.clone(),
                    label: names.join(" / "),
                });
            }

            collect_subgraph_destinations(child, path, names, moving_id, source_path, out);
            names.pop();
            path.pop();
        }
    }
}

fn fresh_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:x}")
}

fn app_data_root() -> PathBuf {
    directories::ProjectDirs::from("", "", "overnotes")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn documents_root() -> PathBuf {
    app_data_root().join("documents")
}

fn settings_path() -> PathBuf {
    app_data_root().join("settings.json")
}

/// Sanitize a game exe name into a folder-safe key ("Game.exe" -> "game").
pub fn game_key(game_exe: &str) -> String {
    let stem = game_exe
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(game_exe)
        .trim_end_matches(".exe")
        .to_lowercase();
    stem.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn doc_dir(game_exe: &str, doc_id: &str) -> PathBuf {
    documents_root().join(game_key(game_exe)).join(doc_id)
}

pub fn list_documents(game_exe: &str) -> Vec<DocMeta> {
    let dir = documents_root().join(game_key(game_exe));
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let doc_path = entry.path().join("doc.json");
        if let Ok(raw) = std::fs::read_to_string(&doc_path) {
            if let Ok(doc) = serde_json::from_str::<Document>(&raw) {
                out.push(DocMeta {
                    id: doc.id,
                    name: doc.name,
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn load_document(game_exe: &str, doc_id: &str) -> Option<Document> {
    let path = doc_dir(game_exe, doc_id).join("doc.json");
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_document(doc: &Document) -> std::io::Result<()> {
    let dir = doc_dir(&doc.game_exe, &doc.id);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(doc)?;
    std::fs::write(dir.join("doc.json"), json)
}

/// Save PNG bytes as an asset of the document, returning the stored filename.
pub fn save_image_asset(doc: &Document, png_bytes: &[u8]) -> std::io::Result<String> {
    let dir = doc_dir(&doc.game_exe, &doc.id);
    std::fs::create_dir_all(&dir)?;
    let name = format!("img_{}.png", fresh_id());
    std::fs::write(dir.join(&name), png_bytes)?;
    Ok(name)
}

/// Load an image asset and return it as a `data:` URL for the webview.
pub fn image_data_url(doc: &Document, file: &str) -> Option<String> {
    use base64::Engine;
    let path = doc_dir(&doc.game_exe, &doc.id).join(file);
    let bytes = std::fs::read(path).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn load_settings() -> AppSettings {
    let raw = std::fs::read_to_string(settings_path()).ok();
    raw.and_then(|raw| serde_json::from_str::<AppSettings>(&raw).ok())
        .unwrap_or_default()
        .normalized()
}

pub fn save_settings(settings: &AppSettings) -> std::io::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&settings.clone().normalized())?;
    std::fs::write(path, json)
}

fn default_toggle_shortcut() -> KeyboardShortcut {
    KeyboardShortcut {
        accelerator: DEFAULT_TOGGLE_SHORTCUT.to_string(),
        label: DEFAULT_TOGGLE_SHORTCUT_LABEL.to_string(),
    }
}

fn default_screenshot_shortcut() -> KeyboardShortcut {
    KeyboardShortcut {
        accelerator: DEFAULT_SCREENSHOT_SHORTCUT.to_string(),
        label: DEFAULT_SCREENSHOT_SHORTCUT_LABEL.to_string(),
    }
}

fn is_supported_shortcut(shortcut: &KeyboardShortcut) -> bool {
    let tokens = shortcut.accelerator.split('+').collect::<Vec<_>>();
    if tokens.is_empty() || shortcut.label.trim().is_empty() {
        return false;
    }
    let Some(key) = tokens.last() else {
        return false;
    };
    if key.trim().is_empty() {
        return false;
    }
    let modifier_tokens = &tokens[..tokens.len().saturating_sub(1)];
    if modifier_tokens
        .iter()
        .any(|token| !matches!(*token, "ctrl" | "shift" | "alt" | "super"))
    {
        return false;
    }
    if !modifier_tokens
        .iter()
        .any(|token| matches!(*token, "ctrl" | "alt" | "super"))
    {
        return false;
    }
    matches!(
        *key,
        "Backquote"
            | "Backslash"
            | "BracketLeft"
            | "BracketRight"
            | "Comma"
            | "Digit0"
            | "Digit1"
            | "Digit2"
            | "Digit3"
            | "Digit4"
            | "Digit5"
            | "Digit6"
            | "Digit7"
            | "Digit8"
            | "Digit9"
            | "Equal"
            | "KeyA"
            | "KeyB"
            | "KeyC"
            | "KeyD"
            | "KeyE"
            | "KeyF"
            | "KeyG"
            | "KeyH"
            | "KeyI"
            | "KeyJ"
            | "KeyK"
            | "KeyL"
            | "KeyM"
            | "KeyN"
            | "KeyO"
            | "KeyP"
            | "KeyQ"
            | "KeyR"
            | "KeyS"
            | "KeyT"
            | "KeyU"
            | "KeyV"
            | "KeyW"
            | "KeyX"
            | "KeyY"
            | "KeyZ"
            | "Minus"
            | "Period"
            | "Quote"
            | "Semicolon"
            | "Slash"
            | "Backspace"
            | "Enter"
            | "Space"
            | "Tab"
            | "Delete"
            | "End"
            | "Home"
            | "Insert"
            | "PageDown"
            | "PageUp"
            | "PrintScreen"
            | "ScrollLock"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "ArrowUp"
            | "Numpad0"
            | "Numpad1"
            | "Numpad2"
            | "Numpad3"
            | "Numpad4"
            | "Numpad5"
            | "Numpad6"
            | "Numpad7"
            | "Numpad8"
            | "Numpad9"
            | "NumpadAdd"
            | "NumpadDecimal"
            | "NumpadDivide"
            | "NumpadEnter"
            | "NumpadEqual"
            | "NumpadMultiply"
            | "NumpadSubtract"
            | "Escape"
            | "F1"
            | "F2"
            | "F3"
            | "F4"
            | "F5"
            | "F6"
            | "F7"
            | "F8"
            | "F9"
            | "F10"
            | "F11"
            | "F12"
    )
}
