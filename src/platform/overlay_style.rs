//! Extended window styles for the overlay window.
//!
//! The overlay is a borderless, transparent, topmost window (created by tao).
//! On top of that we add:
//! - `WS_EX_TOOLWINDOW`: keep the overlay out of the taskbar and Alt-Tab.
//! - `WS_EX_NOACTIVATE` (overview mode only): never steal focus from the game.
//!
//! Click-through (`WS_EX_TRANSPARENT` + `WS_EX_LAYERED`) is toggled through
//! tao's `set_ignore_cursor_events`, not here.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetForegroundWindow, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW,
};

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut std::ffi::c_void)
}

/// Apply the always-on styles for an overlay window.
pub fn apply_overlay_base(raw_hwnd: isize) {
    unsafe {
        let hwnd = hwnd(raw_hwnd);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize);
    }
}

/// Toggle `WS_EX_NOACTIVATE`. Enabled in overview mode so the overlay can never
/// take focus; disabled in edit mode so the user can type into notes.
pub fn set_noactivate(raw_hwnd: isize, on: bool) {
    unsafe {
        let hwnd = hwnd(raw_hwnd);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let flag = WS_EX_NOACTIVATE.0 as isize;
        let new = if on { ex | flag } else { ex & !flag };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
    }
}

/// Give focus back to a window (used to return focus to the game when leaving
/// edit mode).
pub fn focus_window(raw_hwnd: isize) {
    unsafe {
        let _ = SetForegroundWindow(hwnd(raw_hwnd));
    }
}
