//! macOS: the global event tap, the copy shortcut, the pasteboard, screens and focus.

use super::{Display, HookEvent};
use crate::capture::ClipboardPort;
use crate::geometry::{Point, Rect};
use crate::held_keys::HeldKeys;
use crate::selection_detector::MouseInfo;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEvent, CGEventField, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSScreen,
    NSWindow, NSWindowCollectionBehavior,
};
use objc2_foundation::{NSArray, NSData, NSString};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

/// Tags the events this app posts, so the tap can ignore them.
const OWN_EVENT: i64 = 0x464c_5545_4e54;
const KEY_C: u16 = 8;
/// Electron's 'screen-saver' level, which stays above full-screen apps.
const BAR_LEVEL: isize = 1000;

static TAP_PORT: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

// MARK: Hook

fn modifier_flag(keycode: i64) -> Option<CGEventFlags> {
    match keycode {
        54 | 55 => Some(CGEventFlags::CGEventFlagCommand),
        56 | 60 => Some(CGEventFlags::CGEventFlagShift),
        58 | 61 => Some(CGEventFlags::CGEventFlagAlternate),
        59 | 62 => Some(CGEventFlags::CGEventFlagControl),
        _ => None,
    }
}

fn mouse(event: &CGEvent, button: u8) -> MouseInfo {
    let location = event.location();
    let clicks = event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE);
    MouseInfo { point: Point { x: location.x, y: location.y }, button, clicks: clicks.max(0) as u32 }
}

/// Listens to mouse and key events on a thread of its own. Fails without Accessibility access.
pub fn start_hook(held: Arc<HeldKeys>, on_event: impl Fn(HookEvent) + Send + 'static) -> Result<(), String> {
    let (ready, started) = mpsc::channel();
    thread::Builder::new()
        .name("fluent-hook".into())
        .spawn(move || {
            let callback = move |_proxy, kind: CGEventType, event: &CGEvent| {
                if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == OWN_EVENT {
                    return CallbackResult::Keep;
                }
                match kind {
                    CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                        let port = TAP_PORT.load(Ordering::Relaxed);
                        if !port.is_null() {
                            unsafe { CGEventTapEnable(port as CFMachPortRef, true) };
                        }
                    }
                    CGEventType::LeftMouseDown => on_event(HookEvent::MouseDown(mouse(event, 1))),
                    CGEventType::RightMouseDown => on_event(HookEvent::MouseDown(mouse(event, 2))),
                    CGEventType::OtherMouseDown => on_event(HookEvent::MouseDown(mouse(event, 3))),
                    CGEventType::LeftMouseUp => on_event(HookEvent::MouseUp(mouse(event, 1))),
                    CGEventType::RightMouseUp => on_event(HookEvent::MouseUp(mouse(event, 2))),
                    CGEventType::OtherMouseUp => on_event(HookEvent::MouseUp(mouse(event, 3))),
                    CGEventType::KeyDown => on_event(HookEvent::KeyDown),
                    CGEventType::FlagsChanged => {
                        let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                        if let Some(flag) = modifier_flag(keycode) {
                            if event.get_flags().contains(flag) {
                                held.key_down(keycode as u32);
                                on_event(HookEvent::KeyDown);
                            } else {
                                held.key_up(keycode as u32);
                            }
                        }
                    }
                    _ => {}
                }
                CallbackResult::Keep
            };
            let tap = CGEventTap::new(
                CGEventTapLocation::Session,
                CGEventTapPlacement::TailAppendEventTap,
                CGEventTapOptions::ListenOnly,
                vec![
                    CGEventType::LeftMouseDown,
                    CGEventType::LeftMouseUp,
                    CGEventType::RightMouseDown,
                    CGEventType::RightMouseUp,
                    CGEventType::OtherMouseDown,
                    CGEventType::OtherMouseUp,
                    CGEventType::KeyDown,
                    CGEventType::FlagsChanged,
                ],
                callback,
            );
            let Ok(tap) = tap else {
                let _ = ready.send(Err("The event tap could not be created.".to_string()));
                return;
            };
            TAP_PORT.store(tap.mach_port().as_concrete_TypeRef() as *mut _, Ordering::Relaxed);
            let Ok(source) = tap.mach_port().create_runloop_source(0) else {
                let _ = ready.send(Err("The event tap has no run loop source.".to_string()));
                return;
            };
            CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
            tap.enable();
            let _ = ready.send(Ok(()));
            CFRunLoop::run_current();
        })
        .map_err(|e| e.to_string())?;
    started.recv().map_err(|e| e.to_string())?
}

/// Presses Command+C in the frontmost app.
pub fn send_copy() -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "No event source for the copy shortcut.".to_string())?;
    for down in [true, false] {
        let event = CGEvent::new_keyboard_event(source.clone(), KEY_C, down)
            .map_err(|_| "The copy shortcut could not be created.".to_string())?;
        event.set_flags(CGEventFlags::CGEventFlagCommand);
        event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA as CGEventField, OWN_EVENT);
        event.post(CGEventTapLocation::HID);
    }
    Ok(())
}

// MARK: Clipboard

/// Every item on the pasteboard with the data of each of its types.
pub struct Snapshot(Vec<Vec<(Retained<NSString>, Retained<NSData>)>>, String);

pub struct Clipboard {
    board: Retained<NSPasteboard>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self { board: NSPasteboard::generalPasteboard() }
    }
}

impl ClipboardPort for Clipboard {
    type Snapshot = Snapshot;

    fn save(&self) -> Snapshot {
        let mut items = Vec::new();
        if let Some(list) = self.board.pasteboardItems() {
            for item in list.iter() {
                let mut data = Vec::new();
                for kind in item.types().iter() {
                    if let Some(bytes) = item.dataForType(&kind) {
                        data.push((kind.clone(), bytes));
                    }
                }
                items.push(data);
            }
        }
        Snapshot(items, self.read_text())
    }

    fn restore(&self, Snapshot(items, text): Snapshot) {
        self.board.clearContents();
        if items.is_empty() {
            return;
        }
        let objects: Vec<Retained<ProtocolObject<dyn objc2_app_kit::NSPasteboardWriting>>> = items
            .iter()
            .map(|data| {
                let item = NSPasteboardItem::new();
                for (kind, bytes) in data {
                    item.setData_forType(bytes, kind);
                }
                ProtocolObject::from_retained(item)
            })
            .collect();
        if !self.board.writeObjects(&NSArray::from_retained_slice(&objects)) && !text.is_empty() {
            self.write_text(&text);
        }
    }

    fn write_text(&self, text: &str) {
        self.board.clearContents();
        self.board.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
    }

    fn read_text(&self) -> String {
        self.board.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string()).unwrap_or_default()
    }
}

pub fn write_clipboard_text(text: &str) {
    Clipboard::new().write_text(text);
}

// MARK: Screens

pub fn cursor_position() -> Point {
    CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .ok()
        .and_then(|source| CGEvent::new(source).ok())
        .map(|event| Point { x: event.location().x, y: event.location().y })
        .unwrap_or(Point { x: 0.0, y: 0.0 })
}

/// The display under a point, in top-left-origin points like the event tap reports.
pub fn display_at(point: Point) -> Display {
    let fallback = Rect { x: 0.0, y: 0.0, width: 1440.0, height: 900.0 };
    let Some(mtm) = MainThreadMarker::new() else {
        return Display { origin: Point { x: 0.0, y: 0.0 }, work_area: fallback, scale: 1.0 };
    };
    let screens = NSScreen::screens(mtm);
    let primary_height = screens.firstObject().map_or(fallback.height, |s| s.frame().size.height);
    let flip = |r: objc2_foundation::NSRect| Rect {
        x: r.origin.x,
        y: primary_height - (r.origin.y + r.size.height),
        width: r.size.width,
        height: r.size.height,
    };
    let chosen = screens.iter().find(|s| flip(s.frame()).contains(point)).or_else(|| screens.firstObject());
    let work_area = chosen.map_or(fallback, |s| flip(s.visibleFrame()));
    Display { origin: Point { x: work_area.x, y: work_area.y }, work_area, scale: 1.0 }
}

// MARK: Permissions, apps and files

pub fn has_permission(prompt: bool) -> bool {
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::from(prompt))]);
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) }
}

pub fn open_path(path: &Path) {
    let _ = Command::new("open").arg(path).spawn().map(|mut child| thread::spawn(move || child.wait()));
}

pub fn open_accessibility_settings() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .map(|mut child| thread::spawn(move || child.wait()));
}

pub fn frontmost_bundle_id() -> Option<String> {
    let front = Command::new("lsappinfo").arg("front").output().ok()?;
    let asn = String::from_utf8_lossy(&front.stdout).trim().to_string();
    let info = Command::new("lsappinfo").args(["info", "-only", "bundleid", &asn]).output().ok()?;
    crate::focus::parse_bundle_id(&String::from_utf8_lossy(&info.stdout))
}

/// Brings an app to the front and waits until macOS reports it there.
pub fn activate_app(bundle_id: &str) -> bool {
    let _ = Command::new("open").args(["-b", bundle_id]).spawn().map(|mut child| thread::spawn(move || child.wait()));
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if frontmost_bundle_id().as_deref() == Some(bundle_id) {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

/// A menu bar app has no Dock icon. The bundle says so too; this covers `cargo run`.
pub fn hide_dock_icon() {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

/// Lets this app take keyboard focus, which a menu bar app needs for its popup.
pub fn activate_self() {
    if let Some(mtm) = MainThreadMarker::new() {
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}

fn ns_window(window: &slint::Window) -> Option<Retained<NSWindow>> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::WinitWindowAccessor;
    window
        .with_winit_window(|winit| {
            let RawWindowHandle::AppKit(appkit) = winit.window_handle().ok()?.as_raw() else { return None };
            let view: &objc2_app_kit::NSView = unsafe { appkit.ns_view.cast().as_ref() };
            view.window()
        })
        .flatten()
}

/// Keeps the icon bar above other apps, on every Space, and out of the way of focus.
pub fn configure_bar(window: &slint::Window) {
    let Some(ns) = ns_window(window) else { return };
    ns.setLevel(BAR_LEVEL);
    ns.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    // A private AppKit switch, also used by Electron, so clicking the bar keeps the other app active.
    let sel = objc2::sel!(_setPreventsActivation:);
    let responds: bool = unsafe { objc2::msg_send![&*ns, respondsToSelector: sel] };
    if responds {
        let _: () = unsafe { objc2::msg_send![&*ns, _setPreventsActivation: true] };
    }
}

/// Keeps the popup and settings above other windows, as Electron's `alwaysOnTop` did.
pub fn configure_floating(window: &slint::Window) {
    if let Some(ns) = ns_window(window) {
        ns.setCollectionBehavior(NSWindowCollectionBehavior::FullScreenAuxiliary);
    }
}

pub fn window_position(point: Point, _display: &Display) -> slint::WindowPosition {
    slint::WindowPosition::Logical(slint::LogicalPosition::new(point.x as f32, point.y as f32))
}

/// A window's frame in the units the event tap reports.
pub fn window_frame(window: &slint::Window) -> Rect {
    let scale = window.scale_factor() as f64;
    let position = window.position();
    let size = window.size();
    Rect {
        x: position.x as f64 / scale,
        y: position.y as f64 / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
    }
}

pub const OWN_BUNDLE_ID: &str = "local.fluent.app";
