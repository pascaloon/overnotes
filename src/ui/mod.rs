//! Window roots: launcher, overlay, standalone editor.

pub mod launcher;
pub mod overlay;
pub mod standalone;

use dioxus::desktop::tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use dioxus::desktop::tao::platform::windows::WindowBuilderExtWindows;
use dioxus::desktop::{Config, WindowBuilder};

/// The single dark-theme stylesheet shared by every window.
pub const STYLE: &str = include_str!("../../assets/style.css");

pub fn launcher_config() -> Config {
    let window = WindowBuilder::new()
        .with_title("Overnotes")
        .with_inner_size(LogicalSize::new(960.0, 640.0))
        .with_min_inner_size(LogicalSize::new(760.0, 500.0))
        .with_always_on_top(false);
    Config::new()
        .with_window(window)
        .with_menu(None)
        .with_background_color((14, 16, 20, 255))
}

/// Borderless, transparent, topmost overlay window covering the game's
/// client rect (`rect` = x, y, w, h in physical pixels).
pub fn overlay_config(rect: (i32, i32, i32, i32)) -> Config {
    let window = WindowBuilder::new()
        .with_title("Overnotes Overlay")
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_undecorated_shadow(false)
        .with_skip_taskbar(true)
        .with_position(PhysicalPosition::new(rect.0, rect.1))
        .with_inner_size(PhysicalSize::new(rect.2, rect.3));
    Config::new().with_window(window).with_menu(None)
}

pub fn standalone_config(doc_name: &str) -> Config {
    let window = WindowBuilder::new()
        .with_title(format!("Overnotes - {doc_name}"))
        .with_inner_size(LogicalSize::new(1150.0, 760.0))
        .with_min_inner_size(LogicalSize::new(820.0, 560.0))
        .with_always_on_top(false);
    Config::new()
        .with_window(window)
        .with_menu(None)
        .with_background_color((14, 16, 20, 255))
}
