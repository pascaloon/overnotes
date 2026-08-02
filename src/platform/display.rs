//! Multi-monitor helpers for Desktop overlay chrome.

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

use super::tracker::virtual_screen_rect;

/// Monitor work/rect in screen coordinates (physical pixels).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Chrome stage bounds in overlay CSS pixels (relative to the desktop overlay).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChromeBounds {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).ok()?;
        Some((pt.x, pt.y))
    }
}

pub fn monitor_from_point(x: i32, y: i32) -> Option<ScreenRect> {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        if monitor.is_invalid() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        Some(rect_from_win32(info.rcMonitor))
    }
}

fn rect_from_win32(r: RECT) -> ScreenRect {
    ScreenRect {
        x: r.left,
        y: r.top,
        w: r.right - r.left,
        h: r.bottom - r.top,
    }
}

/// Bounds for the chrome stage on the monitor under the cursor, in CSS pixels
/// relative to the virtual-screen overlay origin.
pub fn chrome_bounds_for_cursor(scale_factor: f64) -> Option<ChromeBounds> {
    let (cx, cy) = cursor_pos()?;
    let monitor = monitor_from_point(cx, cy)?;
    let (vx, vy, _, _) = virtual_screen_rect();
    let scale = scale_factor.max(0.1);
    Some(ChromeBounds {
        left: f64::from(monitor.x - vx) / scale,
        top: f64::from(monitor.y - vy) / scale,
        width: f64::from(monitor.w) / scale,
        height: f64::from(monitor.h) / scale,
    })
}
