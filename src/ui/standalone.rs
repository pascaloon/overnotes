//! Standalone editor window: the same editor as the overlay's edit mode,
//! hosted in a regular decorated window.

use dioxus::prelude::*;

use crate::editor::{Editor, EditorHost, EditorState};
use crate::store;

#[component]
pub fn StandaloneRoot(game_exe: String, doc_id: String) -> Element {
    let state = use_context_provider(|| {
        let doc = store::load_document(&game_exe, &doc_id)
            .unwrap_or_else(|| store::Document::new(&game_exe, "Untitled"));
        EditorState::create(EditorHost::Standalone, None, doc)
    });

    // Keep the native window title in sync with the document name.
    use_effect(move || {
        let name = state.doc.read().name.clone();
        dioxus::desktop::window().set_title(&format!("Overnotes - {name}"));
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/style.css") }
        Editor {}
    }
}
