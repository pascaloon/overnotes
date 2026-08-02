//! Subgraphs: folder-shaped containers holding their own graph of objects,
//! with their own camera. Double-clicking one descends into it.

use dioxus::prelude::*;

use super::frame;
use super::registry::{CanvasElement, ObjectCtx, ToolSpec};
use crate::editor::{EditorState, TransactionKind};
use crate::store::{CanvasObject, DEFAULT_SUBGRAPH_COLOR, GraphView, ObjectKind, SUBGRAPH_COLORS};

pub const ID: &str = "subgraph";

const DEFAULT_SIZE: (f64, f64) = (120.0, 110.0);
/// Offset from the pointer to the new folder's top-left. The folder icon sits
/// above its label, so it is anchored a little above the vertical centre.
const DEFAULT_ANCHOR: (f64, f64) = (60.0, 45.0);

pub struct Subgraph;

impl CanvasElement for Subgraph {
    fn matches(&self, kind: &ObjectKind) -> bool {
        matches!(kind, ObjectKind::Subgraph { .. })
    }

    fn body(&self, cx: &ObjectCtx) -> Element {
        let ObjectKind::Subgraph { name, color, .. } = &cx.kind else {
            return rsx! {};
        };
        let (id, mut state, editing) = (cx.id, cx.state, cx.editing);
        let (name, color) = (name.clone(), color.clone());

        rsx! {
            div { class: "subgraph-body",
                svg {
                    class: "folder-icon",
                    view_box: "0 0 96 72",
                    preserve_aspect_ratio: "xMidYMid meet",
                    path {
                        d: "M8 18 H36 L43 28 H88 V60 Q88 66 82 66 H14 Q8 66 8 60 Z",
                        fill: "{color}",
                    }
                    path {
                        d: "M8 18 Q8 12 14 12 H31 Q35 12 38 16 L43 23 H82 Q88 23 88 29 V34 H8 Z",
                        fill: "{color}",
                        opacity: "0.82",
                    }
                    path {
                        d: "M8 34 H88 V60 Q88 66 82 66 H14 Q8 66 8 60 Z",
                        fill: "{color}",
                    }
                    path {
                        d: "M12 37 H84",
                        stroke: "rgba(255,255,255,0.28)",
                        stroke_width: "2",
                    }
                }
                if editing {
                    input {
                        class: "subgraph-name subgraph-name-input",
                        r#type: "text",
                        value: "{name}",
                        spellcheck: "false",
                        onmounted: move |evt| {
                            state.begin_transaction(TransactionKind::ObjectText(id));
                            let data = evt.data();
                            spawn(async move {
                                let _ = data.set_focus(true).await;
                            });
                        },
                        onblur: move |_| state.stop_editing(),
                        onmousedown: move |evt| evt.stop_propagation(),
                        oninput: move |evt| {
                            state.begin_transaction(TransactionKind::ObjectText(id));
                            set_name(&mut state, id, evt.value());
                        },
                        onkeydown: move |evt| {
                            if matches!(evt.key(), Key::Enter | Key::Escape) {
                                evt.stop_propagation();
                                state.stop_editing();
                                state.focus_canvas();
                            }
                        },
                    }
                } else {
                    div {
                        class: "subgraph-name",
                        aria_label: "Double-click to rename",
                        ondoubleclick: move |evt| {
                            evt.stop_propagation();
                            state.start_editing(id);
                        },
                        "{name}"
                    }
                }
            }
        }
    }

    fn toolbar(&self, cx: &ObjectCtx) -> Option<Element> {
        self.matches(&cx.kind)
            .then(|| frame::color_swatches(cx, &SUBGRAPH_COLORS))
    }

    fn on_activate(&self, state: &mut EditorState, id: u64) {
        state.enter_graph(id);
    }

    fn tool(&self) -> Option<ToolSpec> {
        Some(ToolSpec {
            id: ID,
            tooltip: "Add subgraph",
            cursor_class: "tool-subgraph",
            icon,
            options: None,
            on_press: add,
        })
    }

    fn style(&self) -> Option<Asset> {
        Some(asset!("/assets/objects/subgraph.css"))
    }
}

fn icon() -> Element {
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linejoin: "round",
            path { d: "M3 7 H9 L11 10 H21 V19 H3 Z" }
            path { d: "M3 7 V5 H9 L11 8 H21 V10" }
        }
    }
}

/// Create a subgraph centred on `world` and open its name editor.
fn add(state: &mut EditorState, world: (f64, f64)) {
    let (w, h) = DEFAULT_SIZE;
    let template = CanvasObject {
        id: 0,
        x: world.0 - DEFAULT_ANCHOR.0,
        y: world.1 - DEFAULT_ANCHOR.1,
        w,
        h,
        rotation: 0.0,
        opacity_override: None,
        kind: ObjectKind::Subgraph {
            name: "New subgraph".to_string(),
            color: DEFAULT_SUBGRAPH_COLOR.to_string(),
            view: GraphView::default(),
            objects: Vec::new(),
        },
    };
    if let Some(id) = state.insert_object(template, "Could not create subgraph here") {
        state.start_editing(id);
        state.activate_tool(super::Tool::Select);
    }
}

fn set_name(state: &mut EditorState, id: u64, value: String) {
    let path = state.current_graph_path.read().clone();
    let mut doc = state.doc.write();
    if let Some(obj) = doc.object_at_path_mut(&path, id)
        && let ObjectKind::Subgraph { name, .. } = &mut obj.kind
    {
        *name = value;
    }
}
