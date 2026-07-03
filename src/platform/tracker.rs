//! Keeps the overlay window glued to the game window.
//!
//! A dedicated thread installs WinEvent hooks (`SetWinEventHook`,
//! `WINEVENT_OUTOFCONTEXT`) for the game process and pumps a message loop.
//! On every relevant event (move/resize, foreground change, minimize/restore,
//! destroy) it repositions the overlay over the game's client area, hides it
//! when the game is neither foreground nor visible, and notifies the UI when
//! the game window goes away. A low-frequency `WM_TIMER` acts as a safety net
//! for missed events.

use std::sync::mpsc::channel;
use std::sync::Mutex;

use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetClientRect, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
    IsIconic, IsWindow, KillTimer, SetTimer, SetWindowPos, TranslateMessage, EVENT_OBJECT_DESTROY,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, HWND_TOPMOST,
    MSG, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_SHOWWINDOW, WINEVENT_OUTOFCONTEXT, WM_QUIT, WM_TIMER,
};

const OBJID_WINDOW: i32 = 0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GameEvent {
    /// The game window was destroyed; the overlay should close.
    GameClosed,
}

struct TrackerState {
    game: isize,
    overlay: isize,
    tx: UnboundedSender<GameEvent>,
    closed_sent: bool,
}

static STATE: Mutex<Option<TrackerState>> = Mutex::new(None);

pub struct TrackerHandle {
    thread_id: u32,
}

impl TrackerHandle {
    pub fn stop(&self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut std::ffi::c_void)
}

/// Start tracking `game_hwnd`, repositioning `overlay_hwnd` over its client
/// area. Only one tracker can be active at a time (one overlay per app).
pub fn start_tracking(
    game_hwnd: isize,
    overlay_hwnd: isize,
    tx: UnboundedSender<GameEvent>,
) -> TrackerHandle {
    *STATE.lock().unwrap() = Some(TrackerState {
        game: game_hwnd,
        overlay: overlay_hwnd,
        tx,
        closed_sent: false,
    });

    let (tid_tx, tid_rx) = channel::<u32>();

    std::thread::Builder::new()
        .name("overlay-tracker".into())
        .spawn(move || unsafe {
            let _ = tid_tx.send(GetCurrentThreadId());

            let mut game_pid = 0u32;
            GetWindowThreadProcessId(hwnd(game_hwnd), Some(&mut game_pid));

            // Foreground / minimize events, system-wide (foreground changes
            // involve windows of other processes by definition).
            let hook_system: HWINEVENTHOOK = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_MINIMIZEEND,
                None,
                Some(win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );

            // Location / destroy events, scoped to the game's process.
            let hook_game: HWINEVENTHOOK = SetWinEventHook(
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_LOCATIONCHANGE,
                None,
                Some(win_event_proc),
                game_pid,
                0,
                WINEVENT_OUTOFCONTEXT,
            );

            // Safety net for missed events.
            let timer = SetTimer(None, 0, 250, None);

            update_overlay();

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.message == WM_TIMER {
                    update_overlay();
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let _ = KillTimer(None, timer);
            let _ = UnhookWinEvent(hook_system);
            let _ = UnhookWinEvent(hook_game);
            *STATE.lock().unwrap() = None;
        })
        .expect("failed to spawn tracker thread");

    let thread_id = tid_rx.recv().expect("tracker thread failed to start");
    TrackerHandle { thread_id }
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    event_hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _id_thread: u32,
    _time: u32,
) {
    let relevant = {
        let guard = STATE.lock().unwrap();
        let Some(state) = guard.as_ref() else {
            return;
        };
        let is_game = event_hwnd.0 as isize == state.game;
        match event {
            EVENT_SYSTEM_FOREGROUND => true,
            EVENT_OBJECT_DESTROY | EVENT_OBJECT_LOCATIONCHANGE => {
                is_game && id_object == OBJID_WINDOW
            }
            // Minimize start/end and everything in between (movesize, etc).
            _ => is_game,
        }
    };
    if relevant {
        update_overlay();
    }
}

/// Recompute overlay position + visibility from the current game window state.
fn update_overlay() {
    let mut guard = STATE.lock().unwrap();
    let Some(state) = guard.as_mut() else {
        return;
    };

    unsafe {
        let game = hwnd(state.game);
        let overlay = hwnd(state.overlay);

        if !IsWindow(Some(game)).as_bool() {
            let _ = SetWindowPos(
                overlay,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_HIDEWINDOW,
            );
            if !state.closed_sent {
                state.closed_sent = true;
                let _ = state.tx.send(GameEvent::GameClosed);
            }
            return;
        }

        let fg = GetForegroundWindow().0 as isize;
        let visible = !IsIconic(game).as_bool() && (fg == state.game || fg == state.overlay);

        let mut client = RECT::default();
        if GetClientRect(game, &mut client).is_err() {
            return;
        }
        let mut origin = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(game, &mut origin);

        let width = client.right - client.left;
        let height = client.bottom - client.top;

        let show_flag = if visible && width > 0 && height > 0 {
            SWP_SHOWWINDOW
        } else {
            SWP_HIDEWINDOW
        };

        let _ = SetWindowPos(
            overlay,
            Some(HWND_TOPMOST),
            origin.x,
            origin.y,
            width,
            height,
            SWP_NOACTIVATE | show_flag,
        );
    }
}

/// Current client-area rect of a window in screen coordinates
/// `(x, y, w, h)`.
pub fn client_rect_on_screen(raw_hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let h = hwnd(raw_hwnd);
        if !IsWindow(Some(h)).as_bool() {
            return None;
        }
        let mut client = RECT::default();
        GetClientRect(h, &mut client).ok()?;
        let mut origin = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(h, &mut origin);
        Some((
            origin.x,
            origin.y,
            client.right - client.left,
            client.bottom - client.top,
        ))
    }
}
