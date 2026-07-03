//! Document model + JSON persistence.
//!
//! Documents live in `%APPDATA%\overnotes\documents\<game_exe>\<doc_id>\doc.json`,
//! with pasted/captured images saved as PNG files in the same folder.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_NOTE_COLOR: &str = "#e8c95c";
pub const NOTE_COLORS: [&str; 4] = ["#e8c95c", "#8fd18a", "#8db8f2", "#eb9bb9"];
pub const STROKE_COLORS: [&str; 6] = [
    "#ffffff", "#7aa2ff", "#ff6b6b", "#7fd48a", "#ffd166", "#c792ea",
];

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub game_exe: String,
    pub overview_opacity: f64,
    pub edit_opacity: f64,
    #[serde(default)]
    pub next_object_id: u64,
    #[serde(default)]
    pub objects: Vec<CanvasObject>,
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
    pub kind: ObjectKind,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type")]
pub enum ObjectKind {
    Note {
        text: String,
        color: String,
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
}

#[derive(Clone, PartialEq, Debug)]
pub struct DocMeta {
    pub id: String,
    pub name: String,
}

impl Document {
    pub fn new(game_exe: &str, name: &str) -> Self {
        Self {
            id: fresh_id(),
            name: name.to_string(),
            game_exe: game_exe.to_string(),
            overview_opacity: 0.6,
            edit_opacity: 0.95,
            next_object_id: 1,
            objects: Vec::new(),
        }
    }

    pub fn alloc_object_id(&mut self) -> u64 {
        let id = self.next_object_id;
        self.next_object_id += 1;
        id
    }

    pub fn object_mut(&mut self, id: u64) -> Option<&mut CanvasObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn object(&self, id: u64) -> Option<&CanvasObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn remove_object(&mut self, id: u64) {
        self.objects.retain(|o| o.id != id);
    }

    /// Move an object to the end of the list so it renders on top.
    pub fn raise_object(&mut self, id: u64) {
        if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
            let obj = self.objects.remove(pos);
            self.objects.push(obj);
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

fn documents_root() -> PathBuf {
    let base = directories::ProjectDirs::from("", "", "overnotes")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("documents")
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
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
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
