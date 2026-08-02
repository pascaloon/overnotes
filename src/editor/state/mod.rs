//! Live editor state: the signal bundle every editor component reads, plus
//! the commands that mutate it. Commands are grouped by concern into the
//! submodules below, all writing to the same [`EditorState`].

mod assets;
mod camera;
mod document;
mod navigation;
mod selection;
mod transaction;

pub use assets::{AsyncOperationOrigin, PendingScreenshot, png_data_url};
pub use navigation::DropTarget;
pub use selection::ObjectContextMenu;
pub use transaction::{TransactionKind, handle_history_shortcut};

use std::collections::HashMap;

use dioxus::prelude::*;

use super::history::History;
use super::interaction::DragState;
use super::objects::Tool;
use crate::store::{self, AppSettings, Document};

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

#[derive(Clone, Copy)]
pub struct EditorState {
    pub host: EditorHost,
    /// HWND of the attached game window (overlay only), for screenshots.
    pub game_hwnd: Option<isize>,
    pub mode: Signal<ViewMode>,
    pub doc: Signal<Document>,
    pub settings: Signal<AppSettings>,
    pub pan: Signal<(f64, f64)>,
    pub zoom: Signal<f64>,
    pub tool: Signal<Tool>,
    pub selected: Signal<Vec<u64>>,
    /// Object whose inline editor (note text, container name, ...) is open.
    pub editing: Signal<Option<u64>>,
    pub current_graph_path: Signal<Vec<u64>>,
    pub drag: Signal<DragState>,
    pub live_points: Signal<Vec<[f64; 2]>>,
    pub stroke_color: Signal<String>,
    pub stroke_width: Signal<f64>,
    pub menu_open: Signal<bool>,
    pub shot_mode: Signal<bool>,
    pub overview_hidden: Signal<bool>,
    pub pending_shot: Signal<Option<PendingScreenshot>>,
    pub toast: Signal<Option<String>>,
    pub context_menu: Signal<Option<ObjectContextMenu>>,
    pub drop_target: Signal<Option<DropTarget>>,
    /// Object id -> data URL, for image objects.
    pub image_cache: Signal<HashMap<u64, String>>,
    /// Mounted handle of the canvas viewport, used to restore keyboard focus.
    pub viewport_mount: Signal<Option<std::rc::Rc<MountedData>>>,
    pub history: Signal<History>,
    pub transaction_kind: Signal<Option<TransactionKind>>,
    /// Invalidates delayed wheel commits, including on document switches.
    pub wheel_sequence: Signal<u64>,
    /// Invalidates asynchronous image/capture work when its editor context changes.
    pub operation_generation: Signal<u64>,
    /// Desktop overlay: chrome is confined to this monitor rect (CSS px).
    pub chrome_bounds: Signal<Option<crate::platform::display::ChromeBounds>>,
    /// Desktop overlay: chrome is mid fade-out/in while switching monitors.
    pub chrome_fading: Signal<bool>,
}

impl EditorState {
    pub fn create(host: EditorHost, game_hwnd: Option<isize>, doc: Document) -> Self {
        let cache = assets::build_image_cache(&doc);
        let view = doc.view_at_path(&[]).unwrap_or_default();
        Self {
            host,
            game_hwnd,
            mode: Signal::new(ViewMode::Edit),
            doc: Signal::new(doc),
            settings: Signal::new(store::load_settings()),
            pan: Signal::new(view.pan()),
            zoom: Signal::new(view.zoom),
            tool: Signal::new(Tool::Select),
            selected: Signal::new(Vec::new()),
            editing: Signal::new(None),
            current_graph_path: Signal::new(Vec::new()),
            drag: Signal::new(DragState::None),
            live_points: Signal::new(Vec::new()),
            stroke_color: Signal::new("#7aa2ff".to_string()),
            stroke_width: Signal::new(3.0),
            menu_open: Signal::new(false),
            shot_mode: Signal::new(false),
            overview_hidden: Signal::new(false),
            pending_shot: Signal::new(None),
            toast: Signal::new(None),
            context_menu: Signal::new(None),
            drop_target: Signal::new(None),
            image_cache: Signal::new(cache),
            viewport_mount: Signal::new(None),
            history: Signal::new(History::default()),
            transaction_kind: Signal::new(None),
            wheel_sequence: Signal::new(0),
            operation_generation: Signal::new(0),
            chrome_bounds: Signal::new(None),
            chrome_fading: Signal::new(false),
        }
    }

    pub fn is_desktop_overlay(self) -> bool {
        self.host == EditorHost::Overlay && self.game_hwnd.is_none()
    }

    pub fn is_edit_mode(&self) -> bool {
        self.host == EditorHost::Standalone || *self.mode.read() == ViewMode::Edit
    }

    /// Whether the given object's inline editor is open.
    pub fn is_editing(&self, id: u64) -> bool {
        *self.editing.read() == Some(id)
    }

    /// Open the inline editor of `id`, selecting it on the way.
    pub fn start_editing(&mut self, id: u64) {
        self.selected.set(vec![id]);
        self.editing.set(Some(id));
    }

    /// Commit the in-flight text transaction and close the inline editor.
    pub fn stop_editing(&mut self) {
        self.commit_transaction();
        self.editing.set(None);
    }

    pub fn activate_tool(&mut self, tool: Tool) {
        if tool != Tool::Select {
            self.deselect();
        }
        self.close_context_menu();
        self.tool.set(tool);
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

/// Approximate viewport size in CSS pixels (window inner size / scale).
pub fn viewport_size() -> (f64, f64) {
    let win = dioxus::desktop::window();
    let size = win.inner_size();
    let scale = win.scale_factor();
    (size.width as f64 / scale, size.height as f64 / scale)
}
