//! Fullscreen rubber-band region selector for the screenshot tool.

use dioxus::prelude::*;

use crate::editor::EditorState;
use crate::editor::objects::image;
use crate::editor::state::viewport_size;

/// Smallest crop, in CSS pixels, that is treated as intentional.
const MIN_CROP: f64 = 4.0;

#[component]
pub fn ShotOverlay() -> Element {
    let mut state = use_context::<EditorState>();
    let mut start = use_signal(|| None::<(f64, f64)>);
    let mut cur = use_signal(|| (0.0f64, 0.0f64));
    let shot = state.pending_shot.read().clone();

    let rect = (*start.read()).map(|(sx, sy)| {
        let (cx, cy) = *cur.read();
        (sx.min(cx), sy.min(cy), (sx - cx).abs(), (sy - cy).abs())
    });

    rsx! {
        div {
            class: "shot-overlay",
            onmousedown: move |evt| {
                let c = evt.client_coordinates();
                start.set(Some((c.x, c.y)));
                cur.set((c.x, c.y));
            },
            onmousemove: move |evt| {
                if start.peek().is_some() {
                    let c = evt.client_coordinates();
                    cur.set((c.x, c.y));
                }
            },
            onmouseup: move |_| {
                let Some((x, y, w, h)) = rect else {
                    state.cancel_region_screenshot();
                    return;
                };
                start.set(None);
                if w < MIN_CROP || h < MIN_CROP {
                    state.cancel_region_screenshot();
                    return;
                }
                let Some(shot) = state.take_pending_screenshot() else {
                    state.cancel_region_screenshot();
                    return;
                };
                let origin = shot.origin.clone();
                if !state.operation_context_is_current(&origin) {
                    return;
                }
                let (vw, vh) = viewport_size();
                let (vw, vh) = (vw.max(1.0), vh.max(1.0));
                let (wx, wy) = state.screen_to_world(x + w / 2.0, y + h / 2.0);
                let mut state = state;
                // This component unmounts right away (shot_mode = false), so
                // the task must outlive the scope: spawn_forever, not spawn.
                dioxus::dioxus_core::spawn_forever(async move {
                    let (px, py, pw, ph) = (
                        (x / vw * shot.width as f64).round() as i32,
                        (y / vh * shot.height as f64).round() as i32,
                        (w / vw * shot.width as f64).round() as i32,
                        (h / vh * shot.height as f64).round() as i32,
                    );
                    let result = tokio::task::spawn_blocking(move || {
                            crate::platform::capture::crop_png_region(&shot.png, px, py, pw, ph)
                        })
                        .await;
                    match result {
                        Ok(Ok(png)) => {
                            image::insert_png(&mut state, &origin, (wx, wy), &png);
                        }
                        Ok(Err(e)) if state.operation_context_is_current(&origin) => {
                            state.show_toast(&format!("Capture failed: {e}"));
                        }
                        Err(_) if state.operation_context_is_current(&origin) => {
                            state.show_toast("Capture failed");
                        }
                        _ => {}
                    }
                });
            },

            if let Some(shot) = shot.as_ref() {
                img { class: "shot-image", src: "{shot.data_url}", draggable: "false" }
            }

            div { class: "shot-hint", "Drag to crop the screenshot - Esc to cancel" }

            if let Some((x, y, w, h)) = rect {
                div {
                    class: "shot-rect",
                    style: "left: {x}px; top: {y}px; width: {w}px; height: {h}px;",
                }
            }
        }
    }
}
