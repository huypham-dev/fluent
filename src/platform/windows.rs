//! Windows: low-level hooks, the copy shortcut, the clipboard, monitors and files.
//! Screen units here are physical pixels; the process is per-monitor DPI aware.

use super::{Display, HookEvent};
use crate::capture::ClipboardPort;
use crate::geometry::{Point, Rect};
use crate::held_keys::HeldKeys;
use crate::selection_detector::MouseInfo;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;
use std::time::Instant;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetDoubleClickTime, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_C, VK_CONTROL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Tags the input this app sends, so the hook can ignore it.
const OWN_EVENT: usize = 0x464c_5545;

struct HookState {
    held: Arc<HeldKeys>,
    on_event: Box<dyn Fn(HookEvent) + Send + Sync>,
    clicks: Mutex<ClickCounter>,
}

static HOOK: OnceLock<HookState> = OnceLock::new();

/// Windows reports no click count, so double-clicks are counted here.
#[derive(Default)]
struct ClickCounter {
    last: Option<(Instant, Point, u8)>,
    count: u32,
}

impl ClickCounter {
    fn press(&mut self, point: Point, button: u8) -> u32 {
        let (time, width, height) = unsafe {
            (
                GetDoubleClickTime() as u128,
                GetSystemMetrics(SM_CXDOUBLECLK) as f64,
                GetSystemMetrics(SM_CYDOUBLECLK) as f64,
            )
        };
        let now = Instant::now();
        let repeat = self.last.is_some_and(|(at, last, b)| {
            b == button
                && now.duration_since(at).as_millis() <= time
                && (point.x - last.x).abs() <= width / 2.0
                && (point.y - last.y).abs() <= height / 2.0
        });
        self.count = if repeat { self.count + 1 } else { 1 };
        self.last = Some((now, point, button));
        self.count
    }
}

const MODIFIERS: [u32; 8] = [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x5B, 0x5C];

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(state) = HOOK.get() {
            let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
            let point = Point { x: info.pt.x as f64, y: info.pt.y as f64 };
            let (button, down) = match wparam as u32 {
                WM_LBUTTONDOWN => (1, true),
                WM_LBUTTONUP => (1, false),
                WM_RBUTTONDOWN => (2, true),
                WM_RBUTTONUP => (2, false),
                WM_MBUTTONDOWN => (3, true),
                WM_MBUTTONUP => (3, false),
                _ => (0, false),
            };
            if button != 0 {
                let mut clicks = state.clicks.lock().unwrap();
                let count = if down { clicks.press(point, button) } else { clicks.count };
                let event = MouseInfo { point, button, clicks: count };
                drop(clicks);
                (state.on_event)(if down { HookEvent::MouseDown(event) } else { HookEvent::MouseUp(event) });
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(state) = HOOK.get() {
            let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            if info.dwExtraInfo != OWN_EVENT {
                let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                let modifier = MODIFIERS.contains(&info.vkCode);
                if modifier && down {
                    state.held.key_down(info.vkCode);
                } else if modifier {
                    state.held.key_up(info.vkCode);
                }
                if down {
                    (state.on_event)(HookEvent::KeyDown);
                }
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

pub fn start_hook(held: Arc<HeldKeys>, on_event: impl Fn(HookEvent) + Send + Sync + 'static) -> Result<(), String> {
    HOOK.set(HookState { held, on_event: Box::new(on_event), clicks: Mutex::default() })
        .map_err(|_| "The hook is already running.".to_string())?;
    let (ready, started) = mpsc::channel();
    thread::Builder::new()
        .name("fluent-hook".into())
        .spawn(move || unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
            let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
            if mouse.is_null() || keyboard.is_null() {
                let _ = ready.send(Err("The mouse hook could not be installed.".to_string()));
                return;
            }
            let _ = ready.send(Ok(()));
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        })
        .map_err(|e| e.to_string())?;
    started.recv().map_err(|e| e.to_string())?
}

fn key(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: OWN_EVENT,
            },
        },
    }
}

/// Presses Control+C in the foreground app.
pub fn send_copy() -> Result<(), String> {
    let inputs = [key(VK_CONTROL, false), key(VK_C, false), key(VK_C, true), key(VK_CONTROL, true)];
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() { Ok(()) } else { Err("The copy shortcut could not be sent.".into()) }
}

// MARK: Clipboard

/// Formats backed by handles other than global memory, which cannot be copied as bytes.
const HANDLE_FORMATS: [u32; 8] = [2, 3, 9, 14, 0x80, 0x82, 0x83, 0x8E];
/// Formats Windows synthesizes from Unicode text.
const SYNTHESIZED: [u32; 3] = [1, 7, 16];

pub struct Snapshot(Vec<(u32, Vec<u8>)>, String);

pub struct Clipboard;

impl Clipboard {
    pub fn new() -> Self {
        Clipboard
    }
}

impl ClipboardPort for Clipboard {
    type Snapshot = Snapshot;

    fn save(&self) -> Snapshot {
        let text = self.read_text();
        let mut formats = Vec::new();
        if let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) {
            let all: Vec<u32> = clipboard_win::raw::EnumFormats::new().collect();
            let has_unicode = all.contains(&13);
            for format in all {
                if HANDLE_FORMATS.contains(&format) || (has_unicode && SYNTHESIZED.contains(&format)) {
                    continue;
                }
                let mut data = Vec::new();
                if clipboard_win::raw::get_vec(format, &mut data).is_ok() {
                    formats.push((format, data));
                }
            }
        }
        Snapshot(formats, text)
    }

    fn restore(&self, Snapshot(formats, text): Snapshot) {
        let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) else { return };
        let _ = clipboard_win::raw::empty();
        let mut restored = false;
        for (format, data) in &formats {
            restored |= clipboard_win::raw::set_without_clear(*format, data).is_ok();
        }
        if !restored && !text.is_empty() {
            let _ = clipboard_win::raw::set_string(&text);
        }
    }

    fn write_text(&self, text: &str) {
        let _ = clipboard_win::set_clipboard_string(text);
    }

    fn read_text(&self) -> String {
        clipboard_win::get_clipboard_string().unwrap_or_default()
    }
}

pub fn write_clipboard_text(text: &str) {
    Clipboard.write_text(text);
}

// MARK: Monitors

pub fn cursor_position() -> Point {
    let mut point = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut point) };
    Point { x: point.x as f64, y: point.y as f64 }
}

fn rect(r: RECT) -> Rect {
    Rect { x: r.left as f64, y: r.top as f64, width: (r.right - r.left) as f64, height: (r.bottom - r.top) as f64 }
}

pub fn display_at(point: Point) -> Display {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: point.x as i32, y: point.y as i32 }, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(monitor, &mut info);
        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let monitor_rect = rect(info.rcMonitor);
        Display {
            origin: Point { x: monitor_rect.x, y: monitor_rect.y },
            work_area: rect(info.rcWork),
            scale: dpi_x.max(1) as f64 / 96.0,
        }
    }
}

// MARK: Permissions, apps and files

pub fn has_permission(_prompt: bool) -> bool {
    true
}

pub fn open_path(path: &Path) {
    let _ = Command::new("explorer").arg(path).spawn().map(|mut child| thread::spawn(move || child.wait()));
}

pub fn open_accessibility_settings() {}

pub fn frontmost_bundle_id() -> Option<String> {
    None
}

pub fn activate_app(_bundle_id: &str) -> bool {
    false
}

pub fn hide_dock_icon() {}

pub fn activate_self() {}

fn hwnd(window: &slint::Window) -> Option<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::WinitWindowAccessor;
    window
        .with_winit_window(|winit| {
            let RawWindowHandle::Win32(win32) = winit.window_handle().ok()?.as_raw() else { return None };
            Some(win32.hwnd.get() as HWND)
        })
        .flatten()
}

/// Clicking the bar must leave the other app in the foreground with its selection.
pub fn configure_bar(window: &slint::Window) {
    let Some(hwnd) = hwnd(window) else { return };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST) as isize);
    }
}

pub fn configure_floating(window: &slint::Window) {
    let Some(hwnd) = hwnd(window) else { return };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW as isize);
    }
}

pub fn window_position(point: Point, _display: &Display) -> slint::WindowPosition {
    slint::WindowPosition::Physical(slint::PhysicalPosition::new(point.x.round() as i32, point.y.round() as i32))
}

pub fn window_frame(window: &slint::Window) -> Rect {
    let position = window.position();
    let size = window.size();
    Rect { x: position.x as f64, y: position.y as f64, width: size.width as f64, height: size.height as f64 }
}

pub const OWN_BUNDLE_ID: &str = "";
