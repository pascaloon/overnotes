//! Block input from reaching the game while the overlay is in edit mode.
//!
//! The game stays the foreground window (so it keeps running) while the overlay
//! captures mouse hits directly (topmost, non-click-through). `EnableWindow`
//! stops Win32 delivery to the game; a low-level keyboard hook catches keys
//! that would still land on the game HWND. System shortcuts (Alt+Tab, Win key,
//! etc.) always pass through.

use std::sync::mpsc::channel;
use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION,
    KBDLLHOOKSTRUCT, KBDLLHOOKSTRUCT_FLAGS, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
    MSG,
};

const VK_E: u32 = 0x45;
const VK_TAB: u32 = 0x09;
const VK_ESCAPE: u32 = 0x1B;
const VK_F4: u32 = 0x73;
const LLKHF_ALTDOWN: KBDLLHOOKSTRUCT_FLAGS = KBDLLHOOKSTRUCT_FLAGS(0x20);

struct ShieldState {
    game: isize,
    game_pid: u32,
    active: bool,
}

static STATE: Mutex<Option<ShieldState>> = Mutex::new(None);

#[derive(Clone, Copy)]
pub struct InputShieldHandle {
    thread_id: u32,
}

impl InputShieldHandle {
    pub fn set_active(&self, active: bool) {
        let mut guard = STATE.lock().unwrap();
        let Some(state) = guard.as_mut() else {
            return;
        };
        state.active = active;
        set_game_input_enabled(state.game, !active);
    }

    pub fn stop(self) {
        if let Some(state) = STATE.lock().unwrap().as_ref() {
            set_game_input_enabled(state.game, true);
        }
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut std::ffi::c_void)
}

fn set_game_input_enabled(raw_hwnd: isize, enabled: bool) {
    unsafe {
        let _ = EnableWindow(hwnd(raw_hwnd), enabled);
    }
}

/// Start the keyboard backstop thread. Call [`InputShieldHandle::set_active`]
/// when entering/leaving overlay edit mode.
pub fn start_input_shield(game_hwnd: isize) -> InputShieldHandle {
    let mut game_pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd(game_hwnd), Some(&mut game_pid));
    }

    *STATE.lock().unwrap() = Some(ShieldState {
        game: game_hwnd,
        game_pid,
        active: false,
    });

    let (tid_tx, tid_rx) = channel();

    std::thread::Builder::new()
        .name("input-shield".into())
        .spawn(move || unsafe {
            let _ = tid_tx.send(GetCurrentThreadId());

            let hook =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_ll_proc), None, 0).expect("keyboard hook");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let _ = UnhookWindowsHookEx(hook);
            *STATE.lock().unwrap() = None;
        })
        .expect("spawn input shield");

    let thread_id = tid_rx.recv().expect("shield thread start");
    InputShieldHandle { thread_id }
}

unsafe extern "system" fn keyboard_ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let guard = STATE.lock().unwrap();
        if let Some(state) = guard.as_ref() {
            if state.active && foreground_is_game(state) {
                let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
                let key_msg = wparam.0 as u32;
                if key_msg == WM_KEYDOWN || key_msg == WM_SYSKEYDOWN {
                    if pass_keyboard_to_system(kb) {
                        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
                    }
                }
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Keys that must reach Windows even while the game is foreground.
fn pass_keyboard_to_system(kb: &KBDLLHOOKSTRUCT) -> bool {
    if is_toggle_hotkey(kb.vkCode) {
        return true;
    }

    let vk = kb.vkCode;

    // Win key, Start menu shortcuts.
    if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
        return true;
    }

    // Alt+Tab, Alt+Esc, Alt+F4.
    if kb.flags.0 & LLKHF_ALTDOWN.0 != 0 && (vk == VK_TAB || vk == VK_ESCAPE || vk == VK_F4) {
        return true;
    }

    // Ctrl+Esc (Start menu).
    if vk == VK_ESCAPE && key_down(VK_CONTROL) && !key_down(VK_MENU) {
        return true;
    }

    false
}

fn key_down(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 }
}

fn foreground_is_game(state: &ShieldState) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0 as isize == state.game {
            return true;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        pid == state.game_pid
    }
}

fn is_toggle_hotkey(vk: u32) -> bool {
    if vk != VK_E {
        return false;
    }
    key_down(VK_CONTROL) && key_down(VK_SHIFT)
}
