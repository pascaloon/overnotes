//! The tool rail. Everything but the select tool comes from the canvas
//! element registry, so a new element gets a button for free.

use dioxus::prelude::*;

use crate::editor::EditorState;
use crate::editor::objects::{Tool, registry};

#[component]
pub fn Toolbar() -> Element {
    let mut state = use_context::<EditorState>();
    let tool = *state.tool.read();
    let options = registry::tool_spec(tool)
        .and_then(|spec| spec.options)
        .map(|options| options(state));

    rsx! {
        div { class: "toolbar",
            button {
                class: "tool-btn has-tooltip",
                class: if tool == Tool::Select { "active" },
                aria_label: "Select / move (Esc)",
                onclick: move |_| {
                    state.activate_tool(Tool::Select);
                    state.focus_canvas();
                },
                svg {
                    width: "20",
                    height: "20",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linejoin: "round",
                    path { d: "M5 3 L19 12 L12 13.5 L9.5 20 Z" }
                }
            }
            for spec in registry::tools() {
                button {
                    key: "{spec.id}",
                    class: "tool-btn has-tooltip",
                    class: if tool.is(spec.id) { "active" },
                    aria_label: "{spec.tooltip}",
                    onclick: move |_| {
                        state.activate_tool(Tool::Element(spec.id));
                        state.focus_canvas();
                    },
                    {(spec.icon)()}
                }
            }
            for action in registry::actions() {
                button {
                    key: "{action.tooltip}",
                    class: "tool-btn has-tooltip",
                    aria_label: "{action.tooltip}",
                    onclick: move |_| {
                        (action.run)(&mut state);
                        state.focus_canvas();
                    },
                    {(action.icon)()}
                }
            }
        }

        if let Some(options) = options {
            div { class: "tool-opts", {options} }
        }
    }
}
