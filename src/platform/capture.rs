//! One-shot screenshot of a game window region via the Windows Graphics
//! Capture API (`windows-capture` crate).

use std::sync::mpsc::{sync_channel, SyncSender};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window as CaptureWindow;

struct RawFrame {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Handler that grabs the first frame it sees, sends it back, and stops.
struct OneShot {
    tx: SyncSender<RawFrame>,
    done: bool,
}

impl GraphicsCaptureApiHandler for OneShot {
    type Flags = SyncSender<RawFrame>;
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            tx: ctx.flags,
            done: false,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if !self.done {
            self.done = true;
            let width = frame.width();
            let height = frame.height();
            let buffer = frame.buffer().map_err(|e| e.to_string())?;
            let mut nopad = Vec::new();
            let rgba = buffer.as_nopadding_buffer(&mut nopad).to_vec();
            let _ = self.tx.send(RawFrame {
                rgba,
                width,
                height,
            });
            capture_control.stop();
        }
        Ok(())
    }
}

/// Capture a region of a window's *client area* and return PNG bytes.
///
/// `(cx, cy, cw, ch)` are in client-area pixel coordinates, which for an
/// overlay aligned to the game's client rect are simply the overlay-local
/// coordinates.
pub fn capture_window_region(
    raw_hwnd: isize,
    cx: i32,
    cy: i32,
    cw: i32,
    ch: i32,
) -> Result<Vec<u8>, String> {
    if cw < 2 || ch < 2 {
        return Err("selection too small".into());
    }

    let hwnd = HWND(raw_hwnd as *mut std::ffi::c_void);

    // The captured frame covers the whole window (including any frame /
    // title bar), so translate client coordinates into frame coordinates.
    let (win_rect, client_origin) = unsafe {
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).map_err(|e| e.to_string())?;
        let mut origin = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(hwnd, &mut origin);
        (rect, origin)
    };

    let (tx, rx) = sync_channel::<RawFrame>(1);

    let item = CaptureWindow::from_raw_hwnd(raw_hwnd as *mut std::ffi::c_void);
    let settings = Settings::new(
        item,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Exclude,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        tx,
    );

    let control = OneShot::start_free_threaded(settings).map_err(|e| e.to_string())?;
    let frame = rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "capture timed out".to_string())?;
    let _ = control.stop();

    // Map client coordinates to frame coordinates. The frame usually matches
    // GetWindowRect dimensions; scale if it doesn't (DPI mismatch).
    let rect_w = (win_rect.right - win_rect.left).max(1);
    let rect_h = (win_rect.bottom - win_rect.top).max(1);
    let sx = frame.width as f64 / rect_w as f64;
    let sy = frame.height as f64 / rect_h as f64;

    let off_x = (client_origin.x - win_rect.left) as f64;
    let off_y = (client_origin.y - win_rect.top) as f64;

    let fx = ((off_x + cx as f64) * sx).round() as i64;
    let fy = ((off_y + cy as f64) * sy).round() as i64;
    let fw = (cw as f64 * sx).round() as i64;
    let fh = (ch as f64 * sy).round() as i64;

    let fx = fx.clamp(0, frame.width as i64 - 1) as u32;
    let fy = fy.clamp(0, frame.height as i64 - 1) as u32;
    let fw = fw.clamp(1, (frame.width - fx) as i64) as u32;
    let fh = fh.clamp(1, (frame.height - fy) as i64) as u32;

    // Crop out of the raw RGBA buffer.
    let stride = frame.width as usize * 4;
    let mut cropped = Vec::with_capacity((fw * fh * 4) as usize);
    for row in fy..fy + fh {
        let start = row as usize * stride + fx as usize * 4;
        let end = start + fw as usize * 4;
        cropped.extend_from_slice(&frame.rgba[start..end]);
    }

    encode_png(&cropped, fw, fh)
}

/// Encode raw RGBA pixels as PNG bytes.
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let img: image::RgbaImage = image::ImageBuffer::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| "invalid image buffer".to_string())?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
