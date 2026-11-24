//! Windows desktop helpers for the widget: monitors, window placement,
//! showing without activation, autostart (HKCU Run) and single instance.

use windows_sys::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, POINT, RECT,
};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONULL, MONITORINFO,
    MonitorFromPoint,
};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegDeleteKeyValueW,
    RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, HWND_TOPMOST, IsWindowVisible, SPI_GETWORKAREA, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOSIZE, SetWindowPos, ShowWindow, SystemParametersInfoW,
};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Monitor rectangles (left, top, right, bottom), physical pixels.
pub fn monitors() -> Vec<[i32; 4]> {
    unsafe extern "system" fn each(m: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> i32 {
        unsafe {
            let out = &mut *(lp as *mut Vec<[i32; 4]>);
            let mut mi: MONITORINFO = std::mem::zeroed();
            mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(m, &mut mi) != 0 {
                let r = mi.rcMonitor;
                out.push([r.left, r.top, r.right, r.bottom]);
            }
            1
        }
    }
    let mut out: Vec<[i32; 4]> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(each),
            &mut out as *mut _ as LPARAM,
        )
    };
    out
}

/// The primary monitor's work area (without the taskbar).
pub fn work_area() -> [i32; 4] {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut RECT as *mut _, 0) };
    [r.left, r.top, r.right, r.bottom]
}

/// True when the point lies on some monitor.
pub fn on_screen(x: i32, y: i32) -> bool {
    unsafe { !MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONULL).is_null() }
}

pub fn window_rect(hwnd: isize) -> Option<[i32; 4]> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetWindowRect(hwnd as HWND, &mut r) } != 0;
    ok.then_some([r.left, r.top, r.right, r.bottom])
}

/// Moves a window (physical pixels) without resizing or activating it, and
/// keeps it topmost.
pub fn move_window(hwnd: isize, x: i32, y: i32) {
    unsafe {
        SetWindowPos(
            hwnd as HWND,
            HWND_TOPMOST,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
}

/// Shows (without taking focus from the app in front) or hides a window.
pub fn set_visible(hwnd: isize, visible: bool) {
    unsafe {
        ShowWindow(
            hwnd as HWND,
            if visible { SW_SHOWNOACTIVATE } else { SW_HIDE },
        )
    };
}

/// Windows 11 rounded corners for a borderless window (no-op before).
pub fn round_corners(hwnd: isize) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    let pref = DWMWCP_ROUND;
    unsafe {
        DwmSetWindowAttribute(
            hwnd as HWND,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &pref as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&pref) as u32,
        )
    };
}

pub fn is_visible(hwnd: isize) -> bool {
    unsafe { IsWindowVisible(hwnd as HWND) != 0 }
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "nuff-pulse";

/// True when HKCU Run starts nuff-pulse.
pub fn autostart_enabled() -> bool {
    let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
    let mut size = 0u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        ) == 0
    }
}

/// Adds or removes the HKCU Run entry for the running executable.
pub fn set_autostart(on: bool) -> bool {
    let key_path = wide(RUN_KEY);
    let value = wide(RUN_VALUE);
    unsafe {
        if !on {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, key_path.as_ptr(), value.as_ptr());
            return r == 0 || r == 2; // ERROR_FILE_NOT_FOUND: already off
        }
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let data = wide(&format!("\"{}\"", exe.display()));
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        ) != 0
        {
            return false;
        }
        let r = RegSetValueExW(
            key,
            value.as_ptr(),
            0,
            REG_SZ,
            data.as_ptr() as *const u8,
            (data.len() * 2) as u32,
        );
        RegCloseKey(key);
        r == 0
    }
}

/// Claims the per-session single-instance mutex; false when another
/// nuff-pulse widget already runs. The handle lives as long as the process.
pub fn claim_single_instance() -> bool {
    let name = wide("Local\\nuff-pulse-widget");
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        !h.is_null() && GetLastError() != ERROR_ALREADY_EXISTS
    }
}
