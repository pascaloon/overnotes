//! Right-click menu for a single object: rename, z-order, per-object
//! transparency, and moving it to another graph.

use dioxus::prelude::*;

use crate::editor::{EditorState, TransactionKind};

#[component]
pub fn ObjectMenu() -> Element {
    let mut state = use_context::<EditorState>();
    let Some(menu) = state.context_menu.read().clone() else {
        return rsx! {};
    };

    let doc = state.doc.read();
    let obj = doc.object_at_path(&menu.source_path, menu.id);
    let can_rename = obj.is_some_and(|obj| obj.kind.is_container());
    let overview_opacity = doc.overview_opacity;
    let object_opacity = obj
        .and_then(|obj| obj.opacity_override)
        .unwrap_or(overview_opacity);
    let uses_default_opacity = obj.is_none_or(|obj| obj.opacity_override.is_none());
    let destinations = doc.graph_destinations(menu.id, &menu.source_path);
    let has_destinations = !destinations.is_empty();
    drop(doc);

    rsx! {
        div {
            class: "object-menu",
            style: "left: {menu.x}px; top: {menu.y}px;",
            onmousedown: move |evt| evt.stop_propagation(),
            oncontextmenu: move |evt| {
                evt.prevent_default();
                evt.stop_propagation();
            },
            if can_rename {
                button {
                    class: "object-menu-item",
                    onclick: move |_| state.rename_context_object(),
                    "Rename"
                }
            }
            div { class: "object-menu-order-row",
                OrderButton {
                    label: "Move to top",
                    path: "M6 5 H18|M12 19 V9|M7 14 L12 9 L17 14",
                    action: OrderAction::Top,
                }
                OrderButton {
                    label: "Move up",
                    path: "M12 19 V5|M7 10 L12 5 L17 10",
                    action: OrderAction::Up,
                }
                OrderButton {
                    label: "Move down",
                    path: "M12 5 V19|M7 14 L12 19 L17 14",
                    action: OrderAction::Down,
                }
                OrderButton {
                    label: "Move to bottom",
                    path: "M6 19 H18|M12 5 V15|M7 10 L12 15 L17 10",
                    action: OrderAction::Bottom,
                }
            }
            div { class: "object-menu-divider" }
            div { class: "object-menu-control",
                div { class: "object-menu-control-head",
                    span { "Transparency" }
                    span { class: "slider-value", "{(object_opacity * 100.0):.0}%" }
                }
                div { class: "object-menu-slider-row",
                    input {
                        r#type: "range",
                        min: "0",
                        max: "1",
                        step: "0.05",
                        value: "{object_opacity}",
                        onmousedown: move |_| {
                            state.begin_transaction(TransactionKind::ObjectOpacity(menu.id));
                        },
                        oninput: move |evt| {
                            state.begin_transaction(TransactionKind::ObjectOpacity(menu.id));
                            if let Ok(v) = evt.value().parse::<f64>() {
                                state.set_context_object_opacity(v);
                            }
                        },
                        onmouseup: move |_| {
                            state.commit_transaction();
                        },
                        onblur: move |_| {
                            state.commit_transaction();
                        },
                        onchange: move |_| {
                            state.commit_transaction();
                        },
                    }
                    if !uses_default_opacity {
                        button {
                            class: "object-menu-reset-icon has-tooltip",
                            aria_label: "Reset to overview transparency",
                            onclick: move |_| state.reset_context_object_opacity(),
                            svg {
                                width: "16",
                                height: "16",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                path { d: "M3 12 A9 9 0 1 0 6 5.3" }
                                path { d: "M3 4 V10 H9" }
                            }
                        }
                    }
                }
            }
            div { class: "object-menu-divider" }
            div {
                class: "object-menu-item object-menu-parent",
                class: if !has_destinations { "disabled" },
                "Move to"
                span { class: "object-menu-arrow", ">" }
                if has_destinations {
                    div { class: "object-submenu",
                        for destination in destinations.iter().cloned() {
                            button {
                                class: "object-menu-item",
                                aria_label: "{destination.label}",
                                onclick: move |_| {
                                    state.move_context_object_to_graph(destination.path.clone())
                                },
                                "{destination.label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum OrderAction {
    Top,
    Up,
    Down,
    Bottom,
}

/// One z-order button. `path` holds the icon's subpaths, separated by `|`.
#[component]
fn OrderButton(label: &'static str, path: &'static str, action: OrderAction) -> Element {
    let mut state = use_context::<EditorState>();

    rsx! {
        button {
            class: "object-menu-icon-btn has-tooltip",
            aria_label: "{label}",
            onclick: move |_| {
                match action {
                    OrderAction::Top => state.move_context_object_to_top(),
                    OrderAction::Up => state.move_context_object_up(),
                    OrderAction::Down => state.move_context_object_down(),
                    OrderAction::Bottom => state.move_context_object_to_bottom(),
                }
            },
            svg {
                width: "16",
                height: "16",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                for d in path.split('|') {
                    path { key: "{d}", d: "{d}" }
                }
            }
        }
    }
}
