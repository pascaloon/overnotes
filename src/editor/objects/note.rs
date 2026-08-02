//! Sticky notes: editable text with a background color and adjustable font
//! size.

use dioxus::prelude::*;

use super::frame;
use super::registry::{CanvasElement, ObjectCtx, ToolSpec};
use crate::editor::{EditorState, TransactionKind};
use crate::store::{
    CanvasObject, DEFAULT_NOTE_COLOR, DEFAULT_NOTE_FONT_SIZE, NOTE_COLORS, ObjectKind,
};

pub const ID: &str = "note";

const FONT_SIZE_MIN: f64 = 10.0;
const FONT_SIZE_MAX: f64 = 128.0;
const DEFAULT_SIZE: (f64, f64) = (200.0, 140.0);

pub struct Note;

impl CanvasElement for Note {
    fn matches(&self, kind: &ObjectKind) -> bool {
        matches!(kind, ObjectKind::Note { .. })
    }

    fn body(&self, cx: &ObjectCtx) -> Element {
        let ObjectKind::Note {
            text,
            color,
            font_size,
        } = &cx.kind
        else {
            return rsx! {};
        };
        let (id, mut state, editing) = (cx.id, cx.state, cx.editing);
        let (text, color, font_size) = (text.clone(), color.clone(), *font_size);

        rsx! {
            div {
                class: "note-body",
                class: if color == "transparent" { "note-transparent" },
                style: "background: {color}; font-size: {font_size}px;",
                if editing {
                    textarea {
                        class: "note-text",
                        value: "{text}",
                        placeholder: "Type a note...",
                        spellcheck: "false",
                        onmounted: move |evt| {
                            state.begin_transaction(TransactionKind::ObjectText(id));
                            let data = evt.data();
                            spawn(async move {
                                let _ = data.set_focus(true).await;
                            });
                        },
                        onblur: move |_| {
                            state.commit_transaction();
                        },
                        onmousedown: move |evt| evt.stop_propagation(),
                        oninput: move |evt| {
                            state.begin_transaction(TransactionKind::ObjectText(id));
                            set_text(&mut state, id, evt.value());
                        },
                        onkeydown: move |evt| {
                            if evt.key() == Key::Escape {
                                evt.stop_propagation();
                                state.stop_editing();
                                state.focus_canvas();
                            }
                        },
                    }
                } else {
                    div {
                        class: "note-text",
                        style: "white-space: pre-wrap; overflow: hidden;",
                        "{text}"
                    }
                }
            }
        }
    }

    fn toolbar(&self, cx: &ObjectCtx) -> Option<Element> {
        let ObjectKind::Note { font_size, .. } = &cx.kind else {
            return None;
        };
        let (id, mut state) = (cx.id, cx.state);
        let orig_font_size = *font_size;
        let swatches = frame::color_swatches(cx, &NOTE_COLORS);

        Some(rsx! {
            button {
                class: "floating-object-toolbar-format",
                class: "has-tooltip",
                aria_label: "Drag to resize text",
                onmousedown: move |evt| {
                    evt.stop_propagation();
                    evt.prevent_default();
                    let coords = evt.client_coordinates();
                    state.begin_transaction(TransactionKind::Gesture);
                    state
                        .drag
                        .set(crate::editor::DragState::NoteFontSize {
                            id,
                            start_mouse_x: coords.x,
                            orig_font_size,
                        });
                },
                svg {
                    width: "18",
                    height: "18",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    path { d: "M4 19 L9 5 L14 19" }
                    path { d: "M6 14 H12" }
                }
            }
            div { class: "floating-object-toolbar-divider" }
            {swatches}
        })
    }

    fn on_activate(&self, state: &mut EditorState, id: u64) {
        state.start_editing(id);
    }

    fn tool(&self) -> Option<ToolSpec> {
        Some(ToolSpec {
            id: ID,
            tooltip: "Add note",
            cursor_class: "tool-note",
            icon,
            options: None,
            on_press: add,
        })
    }

    fn style(&self) -> Option<Asset> {
        Some(asset!("/assets/objects/note.css"))
    }
}

fn icon() -> Element {
    rsx! {
        svg {
            width: "20",
            height: "20",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linejoin: "round",
            path { d: "M4 4 H20 V14 L14 20 H4 Z" }
            path { d: "M14 20 V14 H20" }
        }
    }
}

/// Create a note centred on `world` and open its text editor.
fn add(state: &mut EditorState, world: (f64, f64)) {
    let (w, h) = DEFAULT_SIZE;
    let template = CanvasObject {
        id: 0,
        x: world.0 - w / 2.0,
        y: world.1 - h / 2.0,
        w,
        h,
        rotation: 0.0,
        opacity_override: None,
        kind: ObjectKind::Note {
            text: String::new(),
            color: DEFAULT_NOTE_COLOR.to_string(),
            font_size: DEFAULT_NOTE_FONT_SIZE,
        },
    };
    if let Some(id) = state.insert_object(template, "Could not create note in this subgraph") {
        state.start_editing(id);
        state.activate_tool(super::Tool::Select);
    }
}

fn set_text(state: &mut EditorState, id: u64, value: String) {
    let path = state.current_graph_path.read().clone();
    let mut doc = state.doc.write();
    if let Some(obj) = doc.object_at_path_mut(&path, id)
        && let ObjectKind::Note { text, .. } = &mut obj.kind
    {
        *text = value;
    }
}

/// Live font scaling driven by the toolbar's drag handle.
pub fn apply_font_size_drag(state: &mut EditorState, id: u64, next_size: f64) {
    let next_size = next_size.clamp(FONT_SIZE_MIN, FONT_SIZE_MAX);
    let path = state.current_graph_path.read().clone();
    let mut doc = state.doc.write();
    if let Some(obj) = doc.object_at_path_mut(&path, id)
        && let ObjectKind::Note { font_size, .. } = &mut obj.kind
    {
        *font_size = next_size;
    }
}
