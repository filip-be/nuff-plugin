//! The Windows shell side of the widget, on its own thread with a hidden
//! window and message loop: the tray icon (alert badge, blinking while a
//! session needs you, menu), the global hotkey, the full-screen check and
//! display changes. It reports to the UI through a callback and never
//! touches the UI's windows itself.

use crate::desktop::{self, wide};
use std::cell::RefCell;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetMonitorInfoW, MonitorFromWindow, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, MONITORINFO, MONITOR_DEFAULTTONULL,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT};
use windows_sys::Win32::UI::Shell::{
    SHQueryUserNotificationState, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, FindWindowW, GetClassNameW, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetSystemMetrics, GetWindowRect, GetWindowThreadProcessId, IsZoomed, KillTimer, PostMessageW, PostQuitMessage,
    RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow, SetTimer, TrackPopupMenu, TranslateMessage, HICON,
    ICONINFO, MF_CHECKED, MF_SEPARATOR, MF_STRING, MSG, SM_CXSMICON, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_APP, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_HOTKEY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW,
};

const CLASS: &str = "nuff-pulse-shell";
const WM_TRAY: u32 = WM_APP + 1;
const WM_UPDATE: u32 = WM_APP + 2;
const WM_REHOTKEY: u32 = WM_APP + 3;
/// Sent by a second nuff-pulse instance: show the bar (wparam 0) or open
/// the app window (wparam 1).
const WM_SHOW_INSTANCE: u32 = WM_APP + 4;
const TIMER_BLINK: usize = 1;
const TIMER_FULLSCREEN: usize = 2;
const BLINK_MS: u32 = 600;
const FULLSCREEN_MS: u32 = 2000;
const HOTKEY_ID: i32 = 1;

const CMD_SHOW_BAR: usize = 1;
const CMD_OPEN_APP: usize = 2;
const CMD_SETTINGS: usize = 3;
const CMD_AUTOSTART: usize = 4;
const CMD_QUIT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEvent {
    /// Tray "Show bar" or left click: show or hide the bar.
    ToggleBar,
    /// Another instance was started: show the bar.
    ShowBar,
    OpenApp,
    OpenSettings,
    /// The global hotkey: pill ↔ list.
    ToggleMode,
    /// A full-screen app came to the front (true) or left (false).
    Fullscreen(bool),
    /// Monitors were added, removed or rearranged.
    LayoutChanged,
    /// A hotkey change was registered or refused (see `Shell::hotkey_ok`).
    HotkeyRegistered,
    Quit,
}

/// What the tray shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrayState {
    pub alerts: usize,
    pub blink: bool,
    pub tooltip: String,
    /// For the check mark on "Show bar".
    pub bar_visible: bool,
}

#[derive(Default)]
struct Inner {
    tray: TrayState,
    hotkey: Option<(u32, u32)>,
    hotkey_ok: Option<bool>,
    /// The hotkey `hotkey_ok` is the outcome for.
    tried: Option<(u32, u32)>,
    /// The hotkey registered now.
    registered: Option<(u32, u32)>,
}

/// Handle to the shell thread.
pub struct Shell {
    hwnd: isize,
    inner: Arc<Mutex<Inner>>,
}

struct Ctx {
    on_event: Box<dyn Fn(ShellEvent)>,
    inner: Arc<Mutex<Inner>>,
    taskbar_created: u32,
    shown: Option<(usize, bool)>,
    tip: String,
    blink_on: bool,
    blinking: bool,
    icon: HICON,
    fullscreen: bool,
    tray_added: bool,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

impl Shell {
    /// Starts the shell thread. `on_event` runs on that thread.
    pub fn spawn(on_event: impl Fn(ShellEvent) + Send + 'static, hotkey: Option<(u32, u32)>, tray: TrayState) -> Option<Shell> {
        let inner = Arc::new(Mutex::new(Inner { tray, hotkey, hotkey_ok: None, tried: None, registered: None }));
        let (tx, rx) = channel();
        let thread_inner = inner.clone();
        std::thread::Builder::new()
            .name("nuff-pulse-shell".into())
            .spawn(move || {
                let hwnd = unsafe { create_window() };
                let _ = tx.send(hwnd as isize);
                if hwnd.is_null() {
                    return;
                }
                let taskbar_created = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
                CTX.with(|c| {
                    *c.borrow_mut() = Some(Ctx {
                        on_event: Box::new(on_event),
                        inner: thread_inner,
                        taskbar_created,
                        shown: None,
                        tip: String::new(),
                        blink_on: true,
                        blinking: false,
                        icon: std::ptr::null_mut(),
                        fullscreen: false,
                        tray_added: false,
                    })
                });
                unsafe {
                    register_hotkey(hwnd);
                    refresh_tray(hwnd);
                    SetTimer(hwnd, TIMER_FULLSCREEN, FULLSCREEN_MS, None);
                    let mut msg: MSG = std::mem::zeroed();
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                CTX.with(|c| c.borrow_mut().take());
            })
            .ok()?;
        let hwnd = rx.recv().ok().filter(|h| *h != 0)?;
        Some(Shell { hwnd, inner })
    }

    /// Updates the tray when something it shows changed.
    pub fn set_tray(&self, tray: TrayState) {
        let mut inner = self.inner.lock().unwrap();
        if inner.tray != tray {
            inner.tray = tray;
            unsafe { PostMessageW(self.hwnd as HWND, WM_UPDATE, 0, 0) };
        }
    }

    /// Registers a new hotkey; see `hotkey_ok` for the outcome.
    pub fn set_hotkey(&self, hotkey: Option<(u32, u32)>) {
        let mut inner = self.inner.lock().unwrap();
        inner.hotkey = hotkey;
        inner.hotkey_ok = None;
        unsafe { PostMessageW(self.hwnd as HWND, WM_REHOTKEY, 0, 0) };
    }

    /// `Some(false)` when the hotkey could not be registered (taken by
    /// another app; the previous one then stays active); `None` while
    /// pending or when there is none.
    pub fn hotkey_ok(&self) -> Option<bool> {
        self.inner.lock().unwrap().hotkey_ok
    }

    /// The outcome of the last registration together with the hotkey it was
    /// for, so a result is never taken for a later request's.
    pub fn hotkey_result(&self) -> (Option<(u32, u32)>, Option<bool>) {
        let i = self.inner.lock().unwrap();
        (i.tried, i.hotkey_ok)
    }

    /// Removes the tray icon and ends the thread.
    pub fn close(&self) {
        unsafe { PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0) };
    }
}

/// Asks a running instance to show its bar or open its app window; false
/// when none answers.
pub fn signal_running_instance(open_app: bool) -> bool {
    unsafe {
        let hwnd = FindWindowW(wide(CLASS).as_ptr(), std::ptr::null());
        !hwnd.is_null() && PostMessageW(hwnd, WM_SHOW_INSTANCE, open_app as usize, 0) != 0
    }
}

unsafe fn create_window() -> HWND {
    unsafe {
        let class = wide(CLASS);
        let hinst = GetModuleHandleW(std::ptr::null());
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = hinst;
        wc.lpszClassName = class.as_ptr();
        RegisterClassExW(&wc);
        // A hidden top-level window (not message-only: it must receive the
        // TaskbarCreated broadcast and display changes).
        CreateWindowExW(
            0,
            class.as_ptr(),
            wide("nuff-pulse").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        )
    }
}

fn emit(ev: ShellEvent) {
    CTX.with(|c| {
        if let Some(ctx) = c.borrow().as_ref() {
            (ctx.on_event)(ev);
        }
    });
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        let taskbar_created = CTX.with(|c| c.borrow().as_ref().map_or(u32::MAX, |x| x.taskbar_created));
        match msg {
            WM_TRAY => {
                match lp as u32 {
                    WM_LBUTTONUP => emit(ShellEvent::ToggleBar),
                    WM_RBUTTONUP => show_menu(hwnd),
                    _ => {}
                }
                0
            }
            WM_UPDATE => {
                refresh_tray(hwnd);
                0
            }
            WM_REHOTKEY => {
                register_hotkey(hwnd);
                emit(ShellEvent::HotkeyRegistered);
                0
            }
            WM_SHOW_INSTANCE => {
                emit(if wp == 1 { ShellEvent::OpenApp } else { ShellEvent::ShowBar });
                0
            }
            WM_HOTKEY => {
                if wp as i32 == HOTKEY_ID {
                    emit(ShellEvent::ToggleMode);
                }
                0
            }
            WM_TIMER => {
                match wp {
                    TIMER_BLINK => {
                        CTX.with(|c| {
                            if let Some(ctx) = c.borrow_mut().as_mut() {
                                ctx.blink_on = !ctx.blink_on;
                            }
                        });
                        refresh_tray(hwnd);
                    }
                    TIMER_FULLSCREEN => {
                        let now = fullscreen_in_front();
                        let changed = CTX.with(|c| {
                            c.borrow_mut().as_mut().is_some_and(|ctx| std::mem::replace(&mut ctx.fullscreen, now) != now)
                        });
                        if changed {
                            emit(ShellEvent::Fullscreen(now));
                        }
                    }
                    _ => {}
                }
                0
            }
            WM_DISPLAYCHANGE => {
                emit(ShellEvent::LayoutChanged);
                DefWindowProcW(hwnd, msg, wp, lp)
            }
            WM_CLOSE => {
                remove_tray(hwnd);
                UnregisterHotKey(hwnd, HOTKEY_ID);
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            m if m == taskbar_created => {
                // Explorer restarted: the icon is gone, add it again.
                CTX.with(|c| {
                    if let Some(ctx) = c.borrow_mut().as_mut() {
                        ctx.tray_added = false;
                        ctx.shown = None;
                    }
                });
                refresh_tray(hwnd);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

unsafe fn register_hotkey(hwnd: HWND) {
    unsafe {
        let Some(inner) = CTX.with(|c| c.borrow().as_ref().map(|x| x.inner.clone())) else { return };
        let (wanted, previous) = {
            let i = inner.lock().unwrap();
            (i.hotkey, i.registered)
        };
        UnregisterHotKey(hwnd, HOTKEY_ID);
        let register = |(mods, vk): (u32, u32)| RegisterHotKey(hwnd, HOTKEY_ID, mods | MOD_NOREPEAT, vk) != 0;
        let ok = wanted.map(register);
        let registered = match ok {
            Some(true) => wanted,
            // Taken by another app: keep the one that worked.
            _ => previous.filter(|p| wanted.is_some() && register(*p)),
        };
        let mut i = inner.lock().unwrap();
        // A newer request arrived meanwhile: its own message reports.
        if i.hotkey == wanted {
            i.hotkey_ok = ok;
            i.tried = wanted;
        }
        i.registered = registered;
    }
}

fn copy_tip(dst: &mut [u16; 128], s: &str) {
    for (i, u) in s.encode_utf16().take(127).enumerate() {
        dst[i] = u;
    }
}

fn nid(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut n: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    n.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = hwnd;
    n.uID = 1;
    n
}

/// Adds or updates the tray icon from the shared state.
unsafe fn refresh_tray(hwnd: HWND) {
    unsafe {
        CTX.with(|c| {
            let mut b = c.borrow_mut();
            let Some(ctx) = b.as_mut() else { return };
            let tray = ctx.inner.lock().unwrap().tray.clone();
            // Blink timer runs only while a session needs you.
            if tray.blink != ctx.blinking {
                ctx.blinking = tray.blink;
                ctx.blink_on = true;
                if tray.blink {
                    SetTimer(hwnd, TIMER_BLINK, BLINK_MS, None);
                } else {
                    KillTimer(hwnd, TIMER_BLINK);
                }
            }
            let badge_on = !ctx.blinking || ctx.blink_on;
            let want = (tray.alerts, badge_on);
            let mut n = nid(hwnd);
            n.uFlags = NIF_MESSAGE | NIF_TIP;
            n.uCallbackMessage = WM_TRAY;
            copy_tip(&mut n.szTip, &tray.tooltip);
            if ctx.shown != Some(want) || !ctx.tray_added {
                let size = GetSystemMetrics(SM_CXSMICON).clamp(16, 64) as u32;
                let icon = make_icon(size, &crate::icon::render(size, tray.alerts, badge_on));
                if !icon.is_null() {
                    n.uFlags |= NIF_ICON;
                    n.hIcon = icon;
                }
                let ok = if ctx.tray_added { Shell_NotifyIconW(NIM_MODIFY, &n) } else { Shell_NotifyIconW(NIM_ADD, &n) } != 0;
                if !ctx.icon.is_null() {
                    DestroyIcon(ctx.icon);
                }
                ctx.icon = icon;
                ctx.tray_added |= ok;
                ctx.shown = ok.then_some(want);
                ctx.tip = tray.tooltip;
            } else if ctx.tip != tray.tooltip {
                Shell_NotifyIconW(NIM_MODIFY, &n);
                ctx.tip = tray.tooltip;
            }
        });
    }
}

unsafe fn remove_tray(hwnd: HWND) {
    unsafe {
        let n = nid(hwnd);
        Shell_NotifyIconW(NIM_DELETE, &n);
        CTX.with(|c| {
            if let Some(ctx) = c.borrow_mut().as_mut() {
                if !ctx.icon.is_null() {
                    DestroyIcon(ctx.icon);
                    ctx.icon = std::ptr::null_mut();
                }
                ctx.tray_added = false;
            }
        });
    }
}

/// An HICON from straight-alpha RGBA pixels (`size`×`size`×4 bytes).
unsafe fn make_icon(size: u32, rgba: &[u8]) -> HICON {
    unsafe {
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = size as i32;
        bi.bmiHeader.biHeight = -(size as i32); // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let color = CreateDIBSection(std::ptr::null_mut(), &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if color.is_null() || bits.is_null() {
            return std::ptr::null_mut();
        }
        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
        for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(rgba.as_chunks::<4>().0) {
            *d = [s[2], s[1], s[0], s[3]];
        }
        let mask_bytes = vec![0u8; (size.div_ceil(16) * 2 * size) as usize];
        let mask = CreateBitmap(size as i32, size as i32, 1, 1, mask_bytes.as_ptr() as *const _);
        let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}

unsafe fn show_menu(hwnd: HWND) {
    unsafe {
        let bar_visible = CTX.with(|c| c.borrow().as_ref().is_some_and(|x| x.inner.lock().unwrap().tray.bar_visible));
        let autostart = desktop::autostart_enabled();
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let item = |id: usize, text: &str, checked: bool| {
            let flags = MF_STRING | if checked { MF_CHECKED } else { 0 };
            AppendMenuW(menu, flags, id, wide(text).as_ptr());
        };
        item(CMD_SHOW_BAR, "Show bar", bar_visible);
        item(CMD_OPEN_APP, "Open app", false);
        item(CMD_SETTINGS, "Settings", false);
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        item(CMD_AUTOSTART, "Start with Windows", autostart);
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        item(CMD_QUIT, "Quit", false);
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        // Without this the menu does not close when clicking elsewhere.
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        match cmd as usize {
            CMD_SHOW_BAR => emit(ShellEvent::ToggleBar),
            CMD_OPEN_APP => emit(ShellEvent::OpenApp),
            CMD_SETTINGS => emit(ShellEvent::OpenSettings),
            CMD_AUTOSTART => {
                desktop::set_autostart(!autostart);
            }
            CMD_QUIT => emit(ShellEvent::Quit),
            _ => {}
        }
    }
}

/// A full-screen app (game, presentation, video, F11 browser) is in front:
/// the foreground window of another process covers its whole monitor and is
/// not merely maximized.
fn fullscreen_in_front() -> bool {
    unsafe {
        let mut q = 0;
        if SHQueryUserNotificationState(&mut q) == 0 && (q == QUNS_RUNNING_D3D_FULL_SCREEN || q == QUNS_PRESENTATION_MODE) {
            return true;
        }
        let fg = GetForegroundWindow();
        if fg.is_null() || IsZoomed(fg) != 0 {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, &mut pid);
        if pid == GetCurrentProcessId() {
            return false;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(fg, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..n]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return false;
        }
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONULL);
        if mon.is_null() || GetWindowRect(fg, &mut r) == 0 {
            return false;
        }
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return false;
        }
        let m = mi.rcMonitor;
        r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
    }
}
