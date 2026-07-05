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

fn arg_value_or_exit<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let i = args.iter().position(|a| a == name)?;
    let Some(value) = args.get(i + 1) else {
        eprintln!("{name} requires a value");
        std::process::exit(1);
    };
    if value.starts_with("--") {
        eprintln!("{name} requires a value");
        std::process::exit(1);
    }
    Some(value)
}

/// Explicit document id/name from `--doc`, or the default document if none is provided.
fn direct_doc_for(game_exe: &str, doc_ref: Option<&str>) -> String {
    let Some(doc_ref) = doc_ref else {
        return default_doc_for(game_exe);
    };
    if store::load_document(game_exe, doc_ref).is_some() {
        return doc_ref.to_string();
    }

    let docs = store::list_documents(game_exe);
    if let Some(meta) = docs.iter().find(|meta| meta.name == doc_ref) {
        return meta.id.clone();
    }
    if let Some(meta) = docs
        .iter()
        .find(|meta| meta.name.eq_ignore_ascii_case(doc_ref))
    {
        return meta.id.clone();
    }

    let available = if docs.is_empty() {
        "none".to_string()
    } else {
        docs.iter()
            .map(|meta| format!("{:?} ({})", meta.name, meta.id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    eprintln!("No document id or name {doc_ref:?} found for game {game_exe:?}. Available: {available}");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let doc_arg = arg_value_or_exit(&args, "--doc");

    // `overnotes --overlay <window-title-substring | hwnd> [--doc <doc-id>]`
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
        let doc_id = direct_doc_for(&game.exe, doc_arg);
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

    // `overnotes --window <game-exe> [--doc <doc-id>]`
    if let Some(i) = args.iter().position(|a| a == "--window") {
        let game_exe = args.get(i + 1).cloned().unwrap_or_else(|| "unknown".into());
        let doc_id = direct_doc_for(&game_exe, doc_arg);
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
