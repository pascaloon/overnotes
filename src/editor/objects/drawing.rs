//! Freehand strokes: drawn live into the canvas, then frozen into an object
//! whose SVG viewBox scales with the object's size.

use dioxus::prelude::*;

use super::registry::{CanvasElement, ObjectCtx, ToolSpec};
use crate::editor::EditorState;
use crate::editor::interaction::DragState;
use crate::store::{CanvasObject, ObjectKind, STROKE_COLORS};

pub const ID: &str = "draw";

/// Minimum world-space distance between two recorded stroke points.
const POINT_SPACING: f64 = 0.75;

pub struct Drawing;

impl CanvasElement for Drawing {
    fn matches(&self, kind: &ObjectKind) -> bool {
        matches!(kind, ObjectKind::Drawing { .. })
    }

    fn body(&self, cx: &ObjectCtx) -> Element {
        let ObjectKind::Drawing {
            points,
            vw,
            vh,
            stroke,
            stroke_width,
        } = &cx.kind
        else {
            return rsx! {};
        };

        rsx! {
            svg {
                class: "drawing-svg",
                view_box: "0 0 {vw} {vh}",
                preserve_aspect_ratio: "none",
                polyline {
                    points: points_attr(points),
                    fill: "none",
                    stroke: "{stroke}",
                    stroke_width: "{stroke_width}",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                }
            }
        }
    }

    fn tool(&self) -> Option<ToolSpec> {
        Some(ToolSpec {
            id: ID,
            tooltip: "Draw",
            cursor_class: "tool-draw",
            icon,
            options: Some(options),
            on_press: begin_stroke,
        })
    }

    fn style(&self) -> Option<Asset> {
        Some(asset!("/assets/objects/drawing.css"))
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
            path { d: "M4 20 L5 15.5 L16.5 4 L20 7.5 L8.5 19 Z" }
        }
    }
}

/// Stroke color and width, shown under the toolbar while the tool is active.
fn options(mut state: EditorState) -> Element {
    let stroke_color = state.stroke_color.read().clone();
    let stroke_width = *state.stroke_width.read();

    rsx! {
        span { class: "opt-label", "Stroke color" }
        div { class: "swatch-row",
            for color in STROKE_COLORS {
                div {
                    class: "swatch",
                    class: if stroke_color == color { "active" },
                    style: "background: {color};",
                    onclick: move |_| state.stroke_color.set(color.to_string()),
                }
            }
        }
        span { class: "opt-label", "Width: {stroke_width:.0}px" }
        input {
            r#type: "range",
            min: "1",
            max: "16",
            step: "1",
            value: "{stroke_width}",
            oninput: move |evt| {
                if let Ok(v) = evt.value().parse::<f64>() {
                    state.stroke_width.set(v);
                }
            },
        }
    }
}

/// The stroke currently being drawn, rendered in world space by the canvas.
pub fn live_stroke(state: &EditorState) -> Element {
    let points = state.live_points.read().clone();
    if points.is_empty() {
        return rsx! {};
    }
    let stroke = state.stroke_color.read().clone();
    let width = *state.stroke_width.read();

    rsx! {
        svg { class: "live-stroke",
            polyline {
                points: points_attr(&points),
                fill: "none",
                stroke: "{stroke}",
                stroke_width: "{width}",
                stroke_linecap: "round",
                stroke_linejoin: "round",
            }
        }
    }
}

fn begin_stroke(state: &mut EditorState, world: (f64, f64)) {
    state.live_points.set(vec![[world.0, world.1]]);
    state.drag.set(DragState::DrawStroke);
}

pub fn extend_stroke(state: &mut EditorState, screen: (f64, f64)) {
    let (wx, wy) = state.screen_to_world(screen.0, screen.1);
    let mut points = state.live_points.write();
    let far_enough = points.last().is_none_or(|p| {
        let (dx, dy) = (p[0] - wx, p[1] - wy);
        (dx * dx + dy * dy).sqrt() > POINT_SPACING
    });
    if far_enough {
        points.push([wx, wy]);
    }
}

/// Freeze the live stroke into a Drawing object sized to its bounding box.
pub fn finish_stroke(state: &mut EditorState) {
    let points = std::mem::take(&mut *state.live_points.write());
    if points.len() < 2 {
        return;
    }
    let stroke = state.stroke_color.read().clone();
    let stroke_width = *state.stroke_width.read();

    let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
    let (mut max_x, mut max_y) = (f64::MIN, f64::MIN);
    for p in &points {
        min_x = min_x.min(p[0]);
        min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]);
        max_y = max_y.max(p[1]);
    }
    let pad = stroke_width / 2.0 + 2.0;
    let x = min_x - pad;
    let y = min_y - pad;
    let w = (max_x - min_x) + pad * 2.0;
    let h = (max_y - min_y) + pad * 2.0;
    let relative = points.iter().map(|p| [p[0] - x, p[1] - y]).collect();

    state.insert_object(
        CanvasObject {
            id: 0,
            x,
            y,
            w,
            h,
            rotation: 0.0,
            opacity_override: None,
            kind: ObjectKind::Drawing {
                points: relative,
                vw: w,
                vh: h,
                stroke,
                stroke_width,
            },
        },
        "Could not finish drawing in this subgraph",
    );
}

pub fn points_attr(points: &[[f64; 2]]) -> String {
    let mut out = String::with_capacity(points.len() * 12);
    for p in points {
        out.push_str(&format!("{:.2},{:.2} ", p[0], p[1]));
    }
    out
}
