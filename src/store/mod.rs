//! Document model + JSON persistence.
//!
//! Documents live in `%APPDATA%\overnotes\documents\<game_exe>\<doc_id>\doc.json`,
//! with pasted/captured images saved as PNG files in the same folder.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_NOTE_COLOR: &str = "#e8c95c";
pub const DEFAULT_SUBGRAPH_COLOR: &str = "#d8a84d";
pub const NOTE_COLORS: [&str; 4] = ["#e8c95c", "#8fd18a", "#8db8f2", "#eb9bb9"];
pub const SUBGRAPH_COLORS: [&str; 5] = ["#d8a84d", "#7aa2ff", "#7fd48a", "#c792ea", "#ff8a65"];
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
    Subgraph {
        name: String,
        color: String,
        #[serde(default)]
        objects: Vec<CanvasObject>,
    },
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

    /// Move an object to the end of the list so it renders on top.
    pub fn raise_object_at_path(&mut self, path: &[u64], id: u64) {
        let Some(objects) = self.objects_at_path_mut(path) else {
            return;
        };
        if let Some(pos) = objects.iter().position(|o| o.id == id) {
            let obj = objects.remove(pos);
            objects.push(obj);
        }
    }

    pub fn move_object_into_subgraph(&mut self, path: &[u64], id: u64, target_id: u64) -> bool {
        if id == target_id {
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
        let Some(pos) = objects.iter().position(|o| o.id == id) else {
            return false;
        };
        let obj = objects.remove(pos);
        let Some(target) = objects.iter_mut().find(|o| o.id == target_id) else {
            objects.insert(pos, obj);
            return false;
        };
        let ObjectKind::Subgraph { objects: child, .. } = &mut target.kind else {
            objects.insert(pos, obj);
            return false;
        };
        child.push(obj);
        true
    }

    pub fn move_object_to_graph(&mut self, source_path: &[u64], id: u64, target_path: &[u64]) -> bool {
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

    pub fn subgraph_destinations(&self, moving_id: u64, source_path: &[u64]) -> Vec<SubgraphDestination> {
        let mut out = Vec::new();
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
