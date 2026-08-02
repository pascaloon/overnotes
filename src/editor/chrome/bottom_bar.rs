//! Overlay-only action bar: screenshot, hide/show, close, and the overflow
//! menu with detach and quit.

use dioxus::prelude::*;

use crate::editor::EditorState;

#[component]
pub fn BottomBar() -> Element {
    let mut state = use_context::<EditorState>();
    let settings = state.settings.read().clone();
    let screenshot_label = settings.overlay_screenshot_shortcut.label;
    let toggle_label = settings.overlay_toggle_shortcut.label;
    let overview_hidden = *state.overview_hidden.read();
    let hide_label = if overview_hidden { "Show" } else { "Hide" };
    let hide_tooltip = if overview_hidden {
        "Show overview overlay"
    } else {
        "Hide overview overlay"
    };
    let mut more_menu_open = use_signal(|| false);

    let can_screenshot = state.game_hwnd.is_some();

    rsx! {
        div { class: "bottombar",
            if can_screenshot {
                button {
                    class: "bar-btn has-tooltip",
                    aria_label: "Capture the game, then crop it ({screenshot_label})",
                    onclick: move |_| {
                        more_menu_open.set(false);
                        state.start_region_screenshot();
                    },
                    svg {
                        width: "18",
                        height: "18",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linejoin: "round",
                        path { d: "M4 8 H7 L9 5 H15 L17 8 H20 V19 H4 Z" }
                        circle { cx: "12", cy: "13", r: "3.5" }
                    }
                    "Screenshot"
                }
                div { class: "divider" }
            }
            button {
                class: "bar-btn has-tooltip",
                class: if overview_hidden { "active" },
                aria_label: "{hide_tooltip}",
                onclick: move |_| {
                    more_menu_open.set(false);
                    state.toggle_overview_hidden();
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
                    if overview_hidden {
                        path { d: "M2 12 S5.5 5 12 5 S22 12 22 12 S18.5 19 12 19 S2 12 2 12" }
                        circle { cx: "12", cy: "12", r: "3" }
                    } else {
                        path { d: "M3 3 L21 21" }
                        path { d: "M10.6 10.6 A3 3 0 0 0 13.4 13.4" }
                        path { d: "M9.5 5.4 A10.6 10.6 0 0 1 12 5 C18.5 5 22 12 22 12 A18.9 18.9 0 0 1 18.6 16.6" }
                        path { d: "M6.1 6.1 A18.9 18.9 0 0 0 2 12 S5.5 19 12 19 A10.7 10.7 0 0 0 15.3 18.5" }
                    }
                }
                "{hide_label}"
            }
            button {
                class: "bar-btn danger has-tooltip",
                aria_label: "Back to overview ({toggle_label})",
                onclick: move |_| {
                    more_menu_open.set(false);
                    state.return_to_overview();
                },
                svg {
                    width: "18",
                    height: "18",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    path { d: "M6 6 L18 18 M18 6 L6 18" }
                }
                "Close"
            }
            div { class: "bar-more",
                button {
                    class: "bar-btn bar-more-caret has-tooltip",
                    class: if *more_menu_open.read() { "active" },
                    aria_label: "More overlay actions",
                    aria_expanded: (*more_menu_open.read()).to_string(),
                    onclick: move |_| {
                        let next = !*more_menu_open.peek();
                        more_menu_open.set(next);
                    },
                    svg {
                        width: "12",
                        height: "12",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2.5",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M6 9 L12 15 L18 9" }
                    }
                }
                if *more_menu_open.read() {
                    div { class: "bar-menu",
                        button {
                            class: "bar-menu-item has-tooltip",
                            aria_label: "Detach overlay into a standalone window",
                            onclick: move |_| {
                                more_menu_open.set(false);
                                state.detach_overlay();
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
                                path { d: "M14 4 H20 V10" }
                                path { d: "M20 4 L10 14" }
                                path { d: "M10 6 H6 A2 2 0 0 0 4 8 V18 A2 2 0 0 0 6 20 H16 A2 2 0 0 0 18 18 V14" }
                            }
                            "Detach"
                        }
                        button {
                            class: "bar-menu-item danger has-tooltip",
                            aria_label: "Save and close the overlay",
                            onclick: move |_| {
                                more_menu_open.set(false);
                                state.quit_overlay();
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
                                path { d: "M18 20 V6 A2 2 0 0 0 16 4 H8 A2 2 0 0 0 6 6 V20" }
                                path { d: "M2 20 H22" }
                                circle { cx: "14", cy: "12", r: "1" }
                            }
                            "Quit"
                        }
                    }
                }
            }
        }
    }
}
