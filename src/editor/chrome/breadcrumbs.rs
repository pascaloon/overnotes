//! Path from the root graph down to the container currently being edited.

use dioxus::prelude::*;

use crate::editor::EditorState;

#[component]
pub fn Breadcrumbs() -> Element {
    let mut state = use_context::<EditorState>();
    let path = state.current_graph_path.read().clone();
    let names = state.doc.read().breadcrumb_names(&path);

    rsx! {
        div { class: "breadcrumbs",
            button {
                class: "crumb",
                class: if path.is_empty() { "current" },
                aria_label: "Root graph",
                onclick: move |_| state.navigate_to_graph_depth(0),
                "Root"
            }
            for (i, name) in names.iter().enumerate() {
                span { class: "crumb-sep", "/" }
                button {
                    class: "crumb",
                    class: if i + 1 == path.len() { "current" },
                    aria_label: "{name}",
                    onclick: move |_| state.navigate_to_graph_depth(i + 1),
                    "{name}"
                }
            }
        }
    }
}
