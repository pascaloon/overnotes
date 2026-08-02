//! Pan/zoom camera, and persisting it per graph so each container remembers
//! where the user left it.

use dioxus::prelude::*;

use super::EditorState;
use crate::store::GraphView;

impl EditorState {
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

    pub(super) fn load_current_graph_view(&mut self) {
        let path = self.current_graph_path.read().clone();
        let view = self.doc.read().view_at_path(&path).unwrap_or_default();
        self.pan.set(view.pan());
        self.zoom.set(view.zoom);
    }
}
