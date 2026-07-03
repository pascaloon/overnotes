#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod editor;
mod platform;
mod store;
mod ui;

use std::sync::OnceLock;

use dioxus::prelude::*;

/// Direct-launch parameters for `--window` / `--overlay` (also usable as a
/// fallback if the launcher flow is unavailable).
struct DirectLaunch {
    game_hwnd: isize,
    game_exe: String,
    doc_id: String,
}

static DIRECT: OnceLock<DirectLaunch> = OnceLock::new();

#[component]
fn DirectOverlay() -> Element {
    let d = DIRECT.get().unwrap();
    rsx! {
        ui::overlay::OverlayRoot {
            game_hwnd: d.game_hwnd,
            game_exe: d.game_exe.clone(),
            doc_id: d.doc_id.clone(),
        }
    }
}

#[component]
fn DirectStandalone() -> Element {
    let d = DIRECT.get().unwrap();
    rsx! {
        ui::standalone::StandaloneRoot {
            game_exe: d.game_exe.clone(),
            doc_id: d.doc_id.clone(),
        }
    }
}

/// First existing document for the game, or a fresh "Untitled" one.
fn default_doc_for(game_exe: &str) -> String {
    if let Some(meta) = store::list_documents(game_exe).into_iter().next() {
        return meta.id;
    }
    let doc = store::Document::new(game_exe, "Untitled");
    let _ = store::save_document(&doc);
    doc.id
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // `overnotes --overlay <window-title-substring | hwnd>`
    if let Some(i) = args.iter().position(|a| a == "--overlay") {
        let title = args.get(i + 1).cloned().unwrap_or_default();
        let game = if let Ok(raw) = title.parse::<isize>() {
            platform::process::list_game_windows()
                .into_iter()
                .find(|g| g.hwnd == raw)
        } else {
            platform::process::list_game_windows()
                .into_iter()
                .find(|g| g.title.to_lowercase().contains(&title.to_lowercase()))
        };
        let Some(game) = game else {
            eprintln!("No window matching {title:?} found");
            std::process::exit(1);
        };
        let rect =
            platform::tracker::client_rect_on_screen(game.hwnd).unwrap_or((100, 100, 1024, 640));
        let doc_id = default_doc_for(&game.exe);
        let _ = DIRECT.set(DirectLaunch {
            game_hwnd: game.hwnd,
            game_exe: game.exe,
            doc_id,
        });
        dioxus::LaunchBuilder::desktop()
            .with_cfg(ui::overlay_config(rect))
            .launch(DirectOverlay);
        return;
    }

    // `overnotes --window <game-exe>`
    if let Some(i) = args.iter().position(|a| a == "--window") {
        let game_exe = args.get(i + 1).cloned().unwrap_or_else(|| "unknown".into());
        let doc_id = default_doc_for(&game_exe);
        let _ = DIRECT.set(DirectLaunch {
            game_hwnd: 0,
            game_exe,
            doc_id,
        });
        dioxus::LaunchBuilder::desktop()
            .with_cfg(ui::standalone_config("Untitled"))
            .launch(DirectStandalone);
        return;
    }

    dioxus::LaunchBuilder::desktop()
        .with_cfg(ui::launcher_config())
        .launch(ui::launcher::Launcher);
}
