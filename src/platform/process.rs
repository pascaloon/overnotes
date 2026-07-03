//! Enumerate visible top-level windows that can be attached to.

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

#[derive(Clone, PartialEq, Debug)]
pub struct GameWindow {
    pub hwnd: isize,
    pub title: String,
    pub pid: u32,
    /// Executable file name, e.g. `dummy_game.exe`.
    pub exe: String,
}

pub fn list_game_windows() -> Vec<GameWindow> {
    let mut windows_list: Vec<GameWindow> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut windows_list as *mut Vec<GameWindow> as isize),
        );
    }
    windows_list.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    windows_list
}

unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = unsafe { &mut *(lparam.0 as *mut Vec<GameWindow>) };

    if let Some(win) = unsafe { inspect_window(hwnd) } {
        list.push(win);
    }
    TRUE
}

unsafe fn inspect_window(hwnd: HWND) -> Option<GameWindow> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return None;
        }

        // Skip tool windows (floating palettes, etc).
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }

        // Skip cloaked windows (UWP ghosts, hidden virtual-desktop windows).
        let mut cloaked: u32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return None;
        }

        // Skip zero-sized windows.
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err()
            || rect.right - rect.left < 120
            || rect.bottom - rect.top < 90
        {
            return None;
        }

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        if len == 0 {
            return None;
        }
        let title = String::from_utf16_lossy(&title_buf[..len as usize]);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == GetCurrentProcessId() {
            return None;
        }

        let exe = process_exe_name(pid)?;
        let exe_lower = exe.to_lowercase();
        // Filter obvious system surfaces.
        const IGNORED: &[&str] = &[
            "explorer.exe",
            "textinputhost.exe",
            "applicationframehost.exe",
            "systemsettings.exe",
            "shellexperiencehost.exe",
            "searchhost.exe",
            "startmenuexperiencehost.exe",
        ];
        if IGNORED.contains(&exe_lower.as_str()) {
            return None;
        }

        Some(GameWindow {
            hwnd: hwnd.0 as isize,
            title,
            pid,
            exe,
        })
    }
}

fn process_exe_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        result.ok()?;
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        full.rsplit(['\\', '/']).next().map(str::to_string)
    }
}
