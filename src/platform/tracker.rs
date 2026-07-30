//! Keeps the overlay window glued to a target.
//!
//! For games: a dedicated thread installs WinEvent hooks (`SetWinEventHook`,
//! `WINEVENT_OUTOFCONTEXT`) for the game process and pumps a message loop.
//! On every relevant event (move/resize, foreground change, minimize/restore,
//! destroy) it repositions the overlay over the game's client area, hides it
//! when the game is neither foreground nor visible, and notifies the UI when
//! the game window goes away.
//!
//! For Desktop: the overlay is pinned to the virtual screen (all monitors)
//! and stays visible; there is no game window to follow or close on.
//!
//! A low-frequency `WM_TIMER` acts as a safety net for missed events (and
//! picks up display-layout changes for Desktop).

use std::sync::mpsc::channel;
use std::sync::Mutex;

use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetClientRect, GetForegroundWindow, GetMessageW, GetSystemMetrics,
    GetWindowThreadProcessId, IsIconic, IsWindow, KillTimer, PostThreadMessageW, SetTimer,
    SetWindowPos, TranslateMessage, EVENT_OBJECT_DESTROY, EVENT_OBJECT_LOCATIONCHANGE,
    EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, HWND_TOPMOST, MSG, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_HIDEWINDOW, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, WINEVENT_OUTOFCONTEXT, WM_QUIT, WM_TIMER,
};

const OBJID_WINDOW: i32 = 0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GameEvent {
    /// The game window was destroyed; the overlay should close.
    GameClosed,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TrackTarget {
    Game(isize),
    Desktop,
}

struct TrackerState {
    target: TrackTarget,
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

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut std::ffi::c_void)
}

/// Bounding rect of the virtual screen (all monitors) in screen coordinates
/// `(x, y, w, h)`. Origin can be negative when a secondary monitor is left or
/// above the primary.
pub fn virtual_screen_rect() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

/// Start tracking `game_hwnd`, repositioning `overlay_hwnd` over its client
/// area. Only one tracker can be active at a time (one overlay per app).
pub fn start_tracking(
    game_hwnd: isize,
    overlay_hwnd: isize,
    tx: UnboundedSender<GameEvent>,
) -> TrackerHandle {
    start_tracker(TrackTarget::Game(game_hwnd), overlay_hwnd, tx)
}

/// Pin `overlay_hwnd` to the virtual screen (all monitors). Never emits
/// [`GameEvent::GameClosed`].
pub fn start_desktop_tracking(
    overlay_hwnd: isize,
    tx: UnboundedSender<GameEvent>,
) -> TrackerHandle {
    start_tracker(TrackTarget::Desktop, overlay_hwnd, tx)
}

fn start_tracker(
    target: TrackTarget,
    overlay_hwnd: isize,
    tx: UnboundedSender<GameEvent>,
) -> TrackerHandle {
    *STATE.lock().unwrap() = Some(TrackerState {
        target,
        overlay: overlay_hwnd,
        tx,
        closed_sent: false,
    });

    let (tid_tx, tid_rx) = channel::<u32>();
    let track_target = target;

    std::thread::Builder::new()
        .name("overlay-tracker".into())
        .spawn(move || unsafe {
            let _ = tid_tx.send(GetCurrentThreadId());

            let (hook_system, hook_game) = match track_target {
                TrackTarget::Desktop => (HWINEVENTHOOK::default(), HWINEVENTHOOK::default()),
                TrackTarget::Game(game_hwnd) => {
                    let mut game_pid = 0u32;
                    GetWindowThreadProcessId(hwnd(game_hwnd), Some(&mut game_pid));

                    // Foreground / minimize events, system-wide (foreground changes
                    // involve windows of other processes by definition).
                    let hook_system = SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_MINIMIZEEND,
                        None,
                        Some(win_event_proc),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT,
                    );

                    // Location / destroy events, scoped to the game's process.
                    let hook_game = SetWinEventHook(
                        EVENT_OBJECT_DESTROY,
                        EVENT_OBJECT_LOCATIONCHANGE,
                        None,
                        Some(win_event_proc),
                        game_pid,
                        0,
                        WINEVENT_OUTOFCONTEXT,
                    );
                    (hook_system, hook_game)
                }
            };

            // Safety net for missed events / display layout changes.
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
            if !hook_system.is_invalid() {
                let _ = UnhookWinEvent(hook_system);
            }
            if !hook_game.is_invalid() {
                let _ = UnhookWinEvent(hook_game);
            }
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
        let TrackTarget::Game(game) = state.target else {
            return;
        };
        let is_game = event_hwnd.0 as isize == game;
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

/// Recompute overlay position + visibility from the current track target.
fn update_overlay() {
    let mut guard = STATE.lock().unwrap();
    let Some(state) = guard.as_mut() else {
        return;
    };

    unsafe {
        let overlay = hwnd(state.overlay);

        match state.target {
            TrackTarget::Desktop => {
                let (x, y, w, h) = virtual_screen_rect();
                if w <= 0 || h <= 0 {
                    return;
                }
                let _ = SetWindowPos(
                    overlay,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    w,
                    h,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            TrackTarget::Game(game_raw) => {
                let game = hwnd(game_raw);

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
                let visible =
                    !IsIconic(game).as_bool() && (fg == game_raw || fg == state.overlay);

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
