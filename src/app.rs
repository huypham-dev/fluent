//! The app: the tray, the hotkey, the icon bar, the popup and the settings window.
//! Everything here runs on the UI thread; slow work runs on worker threads and posts back.

use crate::actions::ActionId;
use crate::capture::{Capture, CaptureOptions, capture_selection, new_marker};
use crate::cli::{CliErrorKind, CliEvent, CliOptions, CliRun, run_cli};
use crate::geometry::{BAR_SIZE, Point, PopupSide, bar_position, popup_centered, popup_placement, selection_anchor};
use crate::held_keys::HeldKeys;
use crate::platform::{self, HookEvent};
use crate::selection_detector::{MouseInfo, SelectionDetector};
use crate::session::{ActionSession, Phase, Run, SessionConfig};
use crate::settings::{self, HOTKEYS, Model, Settings};
use crate::store::{StyleStore, log_event};
use crate::{Bar, Hint, Popup, SettingsWindow};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::{Cell, OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

const COPIED_CLOSE_DELAY: Duration = Duration::from_millis(450);
const BAR_TIMEOUT: Duration = Duration::from_millis(4000);
const HOTKEY_KEY_GRACE: Duration = Duration::from_millis(600);
const HOTKEY_RELEASE_TIMEOUT: Duration = Duration::from_millis(1000);
const PERMISSION_POLL: Duration = Duration::from_millis(2000);
const REFOCUS_SETTLE: Duration = Duration::from_millis(80);
const COPY_RESEND: Duration = Duration::from_millis(150);
const REFOCUSED_COPY_TIMEOUT: Duration = Duration::from_millis(1000);
const CLI_INACTIVITY: Duration = Duration::from_secs(30);
const IS_MAC: bool = cfg!(target_os = "macos");

#[derive(Clone)]
enum Message {
    Accessibility,
    Settings,
    None,
}

struct PopupMessage {
    title: &'static str,
    detail: String,
    button: Message,
}

fn permission_message() -> PopupMessage {
    PopupMessage {
        title: "Fluent needs Accessibility access",
        detail: "It uses this to detect selections and copy your selected text. Turn on Fluent in System Settings, then try again.".into(),
        button: Message::Accessibility,
    }
}

pub struct App {
    data_dir: PathBuf,
    settings: RefCell<Settings>,
    bar: Bar,
    popup: Popup,
    settings_window: SettingsWindow,
    bar_visible: Cell<bool>,
    popup_open: Cell<bool>,
    settings_open: Cell<bool>,
    bar_timer: Timer,
    session: RefCell<Option<ActionSession>>,
    /// Counts sessions, so a delayed close only closes the session it was meant for.
    session_number: Cell<u64>,
    cli: RefCell<Option<CliRun>>,
    last_phase: Cell<Option<Phase>>,
    copied: Cell<bool>,
    source_app: Arc<std::sync::Mutex<Option<String>>>,
    /// Milliseconds since `started`; key presses before then do not hide the bar.
    ignore_keys_until: Arc<AtomicU64>,
    started: Instant,
    hook_running: Cell<bool>,
    capturing: Arc<AtomicBool>,
    held: Arc<HeldKeys>,
    detector: RefCell<SelectionDetector>,
    /// Where the last mouse press started, to find the middle of a dragged selection.
    press_point: Cell<Option<Point>>,
    /// Where the selection the bar acts on is on screen; the popup points at it.
    selection_point: Cell<Option<Point>>,
    /// An outside click already moved focus where the user wanted it.
    refocus_on_close: Cell<bool>,
    hotkeys: RefCell<Option<GlobalHotKeyManager>>,
    hotkey: RefCell<Option<HotKey>>,
    tray: RefCell<Option<TrayIcon>>,
    permission_timer: Timer,
}

thread_local! {
    static APP: OnceCell<Rc<App>> = const { OnceCell::new() };
}

fn with_app(f: impl FnOnce(&Rc<App>)) {
    let app = APP.with(|cell| cell.get().cloned());
    if let Some(app) = app {
        f(&app);
    }
}

/// Runs `f` on the UI thread.
fn post(f: impl FnOnce(&Rc<App>) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || with_app(f));
}

fn parse_hotkey(hotkey: &str) -> HotKey {
    match hotkey {
        "Control+Alt+D" => HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD),
        "Control+Shift+Space" => HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space),
        _ => HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyR),
    }
}

fn hotkey_label(hotkey: &str) -> String {
    let label = hotkey.replace('+', " + ");
    if IS_MAC { label.replace("Alt", "Option") } else { label }
}

impl App {
    pub fn new(data_dir: PathBuf, settings: Settings) -> Result<Rc<Self>, slint::PlatformError> {
        let app = Rc::new(Self {
            data_dir,
            settings: RefCell::new(settings),
            bar: crate::with_window_kind(crate::WindowKind::Bar, Bar::new)?,
            popup: crate::with_window_kind(crate::WindowKind::Popup, Popup::new)?,
            settings_window: crate::with_window_kind(crate::WindowKind::Settings, SettingsWindow::new)?,
            bar_visible: Cell::new(false),
            popup_open: Cell::new(false),
            settings_open: Cell::new(false),
            bar_timer: Timer::default(),
            session: RefCell::new(None),
            session_number: Cell::new(0),
            cli: RefCell::new(None),
            last_phase: Cell::new(None),
            copied: Cell::new(false),
            source_app: Arc::default(),
            ignore_keys_until: Arc::new(AtomicU64::new(0)),
            started: Instant::now(),
            hook_running: Cell::new(false),
            capturing: Arc::new(AtomicBool::new(false)),
            held: Arc::default(),
            detector: RefCell::default(),
            press_point: Cell::new(None),
            selection_point: Cell::new(None),
            refocus_on_close: Cell::new(true),
            hotkeys: RefCell::new(None),
            hotkey: RefCell::new(None),
            tray: RefCell::new(None),
            permission_timer: Timer::default(),
        });
        APP.with(|cell| {
            let _ = cell.set(app.clone());
        });
        app.wire_callbacks();
        Ok(app)
    }

    fn log(&self, event: &str, details: &[(&str, String)]) {
        log_event(&self.log_path(), event, details);
    }

    fn log_path(&self) -> PathBuf {
        self.data_dir.join("diagnostics.log")
    }

    fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    fn style(&self) -> StyleStore {
        StyleStore { path: self.data_dir.join("style.md") }
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn ignore_keys_for(&self, duration: Duration) {
        self.ignore_keys_until.store(self.now_ms() + duration.as_millis() as u64, Ordering::Relaxed);
    }

    // MARK: Startup

    /// Runs once the event loop is running, which the tray and hotkey need on macOS.
    pub fn start(self: &Rc<Self>) {
        platform::hide_dock_icon();
        self.install_style_template_once();
        self.create_tray();
        self.wait_for_permission_then_start_hook();
        self.apply_hotkey();
        GlobalHotKeyEvent::set_event_handler(Some(|event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                post(|app| app.on_hotkey());
            }
        }));
    }

    fn install_style_template_once(&self) {
        if self.settings.borrow().style_template_installed {
            return;
        }
        match self.style().install_template() {
            Ok(()) => {
                self.settings.borrow_mut().style_template_installed = true;
                let _ = settings::save_settings(&self.settings_path(), &self.settings.borrow());
            }
            Err(error) => self.log("style-template-failed", &[("error", error.to_string())]),
        }
    }

    fn create_tray(self: &Rc<Self>) {
        let settings_item = MenuItem::new("Settings…", true, None);
        let style_item = MenuItem::new("Edit Style…", true, None);
        let log_item = MenuItem::new("Open Diagnostic Log", true, None);
        let quit_item = MenuItem::new("Quit Fluent", true, None);
        let menu = Menu::new();
        let _ =
            menu.append_items(&[&settings_item, &style_item, &log_item, &PredefinedMenuItem::separator(), &quit_item]);
        let ids = [settings_item.id().clone(), style_item.id().clone(), log_item.id().clone(), quit_item.id().clone()];
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let index = ids.iter().position(|id| *id == event.id);
            post(move |app| match index {
                Some(0) => app.open_settings(),
                Some(1) => app.edit_style(),
                Some(2) => platform::open_path(&app.log_path()),
                Some(3) => app.quit(),
                _ => {}
            });
        }));

        let icon = decode_png(include_bytes!("../static/tray@2x.png"))
            .and_then(|(rgba, w, h)| tray_icon::Icon::from_rgba(rgba, w, h).ok());
        let mut builder = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Fluent");
        if let Some(icon) = icon {
            // The logo is in color, as in the Electron app. A template icon would keep only
            // its shape and show a plain square.
            builder = builder.with_icon(icon);
        }
        match builder.build() {
            Ok(tray) => *self.tray.borrow_mut() = Some(tray),
            Err(error) => self.log("tray-failed", &[("error", error.to_string())]),
        }
    }

    fn quit(&self) {
        if let Some(manager) = self.hotkeys.borrow().as_ref() {
            if let Some(hotkey) = self.hotkey.borrow_mut().take() {
                let _ = manager.unregister(hotkey);
            }
        }
        self.tray.borrow_mut().take();
        let _ = slint::quit_event_loop();
    }

    fn wire_callbacks(self: &Rc<Self>) {
        self.bar.on_run(|action| {
            if let Some(action) = ActionId::from_key(&action) {
                with_app(|app| app.run_action(action));
            }
        });

        let popup = &self.popup;
        popup.on_dismiss(|| with_app(|app| app.close_popup()));
        popup.on_copy(|| with_app(|app| app.copy_result()));
        popup.on_edited(|text| {
            with_app(|app| {
                if let Some(session) = app.session.borrow_mut().as_mut() {
                    session.set_result(&text);
                }
                app.render_session();
            })
        });
        popup.on_submit(|text| {
            let mut sent = false;
            with_app(|app| {
                let run = app.session.borrow_mut().as_mut().and_then(|s| s.submit_input(&text));
                if let Some(run) = run {
                    sent = true;
                    app.launch(run);
                }
            });
            sent
        });
        popup.on_retry(|| {
            with_app(|app| {
                let run = app.session.borrow_mut().as_mut().map(ActionSession::retry);
                if let Some(run) = run {
                    app.launch(run);
                }
            })
        });
        popup.on_open_settings(|| with_app(|app| app.open_settings()));
        popup.on_open_accessibility(platform::open_accessibility_settings);
        popup.on_start_drag(|| {
            with_app(|app| {
                // Once dragged away, the arrow no longer points at the selection.
                app.popup.set_side(SharedString::new());
                app.popup.window().with_winit_window(|window| {
                    let _ = window.drag_window();
                });
            })
        });

        let settings = &self.settings_window;
        settings.on_changed(|| with_app(|app| app.refresh_settings_form(None)));
        settings.on_save(|| with_app(|app| app.save_settings()));
        settings.on_edit_style(|| with_app(|app| app.edit_style()));
        settings.window().on_close_requested(|| {
            with_app(|app| app.settings_open.set(false));
            slint::CloseRequestResponse::HideWindow
        });
    }

    // MARK: Hook and permission

    fn wait_for_permission_then_start_hook(self: &Rc<Self>) {
        if platform::has_permission(false) {
            self.start_hook();
            return;
        }
        self.log("permission-missing", &[]);
        platform::has_permission(true);
        self.permission_timer.start(TimerMode::Repeated, PERMISSION_POLL, || {
            with_app(|app| {
                if !platform::has_permission(false) {
                    return;
                }
                app.permission_timer.stop();
                app.log("permission-granted", &[]);
                app.start_hook();
            })
        });
    }

    fn start_hook(&self) {
        let result = platform::start_hook(self.held.clone(), |event| {
            post(move |app| match event {
                HookEvent::MouseDown(info) => app.on_mouse_down(info),
                HookEvent::MouseUp(info) => app.on_mouse_up(info),
                HookEvent::KeyDown => {
                    if app.now_ms() >= app.ignore_keys_until.load(Ordering::Relaxed) {
                        app.hide_bar();
                    }
                }
            })
        });
        match result {
            Ok(()) => {
                self.hook_running.set(true);
                self.log("hook-started", &[]);
            }
            Err(error) => {
                self.log("hook-failed", &[("error", error)]);
                self.show_message(
                    PopupMessage {
                        title: "The icon bar is turned off",
                        detail: "Fluent could not monitor the mouse, which security software can block. You can still select text and press the hotkey.".into(),
                        button: Message::Settings,
                    },
                    None,
                );
            }
        }
    }

    fn is_inside_own_window(&self, point: Point) -> bool {
        let inside = |visible: bool, window: &slint::Window| visible && platform::window_frame(window).contains(point);
        inside(self.bar_visible.get(), self.bar.window())
            || inside(self.popup_open.get(), self.popup.window())
            || inside(self.settings_open.get(), self.settings_window.window())
    }

    fn on_mouse_down(&self, info: MouseInfo) {
        self.detector.borrow_mut().mouse_down(info);
        self.press_point.set(Some(info.point));
        if !(self.bar_visible.get() && platform::window_frame(self.bar.window()).contains(info.point)) {
            self.hide_bar();
        }
        // Like a page's translate bubble, a click anywhere else dismisses the popup.
        if self.popup_open.get() && !self.is_inside_own_window(info.point) {
            self.log("popup-dismissed", &[]);
            self.refocus_on_close.set(false);
            self.close_popup();
        }
    }

    fn on_mouse_up(&self, info: MouseInfo) {
        let own = self.is_inside_own_window(info.point);
        let selected = self.detector.borrow_mut().mouse_up(info, own);
        if selected && self.settings.borrow().auto_bar && !self.capturing.load(Ordering::Relaxed) {
            self.show_bar(info.point, selection_anchor(self.press_point.get(), info.point));
        }
    }

    // MARK: Icon bar

    fn show_bar(&self, pointer: Point, anchor: Point) {
        self.selection_point.set(Some(anchor));
        if IS_MAC {
            let source = self.source_app.clone();
            thread::spawn(move || *source.lock().unwrap() = platform::frontmost_bundle_id());
        }
        let display = platform::display_at(pointer);
        let at = bar_position(display.to_logical(pointer), BAR_SIZE, display.logical_work_area());
        self.bar.window().set_position(platform::window_position(display.to_screen(at), &display));
        let _ = self.bar.show();
        platform::configure_bar(self.bar.window());
        self.bar.set_shown(true);
        self.bar_visible.set(true);
        self.log("bar-shown", &[]);
        self.bar_timer.start(TimerMode::SingleShot, BAR_TIMEOUT, || with_app(|app| app.hide_bar()));
    }

    fn hide_bar(&self) {
        self.bar_timer.stop();
        if self.bar_visible.replace(false) {
            self.bar.set_shown(false);
            let _ = self.bar.hide();
        }
    }

    fn on_hotkey(self: &Rc<Self>) {
        self.ignore_keys_for(HOTKEY_KEY_GRACE);
        if !platform::has_permission(false) {
            self.show_message(permission_message(), None);
            return;
        }
        self.show_bar_for_hotkey();
    }

    /// The hotkey is pressed on purpose, so it can afford a copy to check for a selection first.
    /// On Windows that copy is Ctrl+C, which would interrupt a command in a terminal.
    /// If the check cannot run, the bar shows, and choosing an action checks again.
    fn show_bar_for_hotkey(self: &Rc<Self>) {
        if !IS_MAC || !self.hook_running.get() || self.popup_open.get() {
            let cursor = platform::cursor_position();
            self.show_bar(cursor, cursor);
            return;
        }
        if self.capturing.swap(true, Ordering::Relaxed) {
            return;
        }
        let held = self.held.clone();
        let capturing = self.capturing.clone();
        let ignore = self.ignore_keys_until.clone();
        let started = self.started;
        thread::spawn(move || {
            // A held Control or Alt would turn the simulated copy into a different shortcut.
            let show = if held.wait_released(HOTKEY_RELEASE_TIMEOUT) {
                ignore.store((started.elapsed() + HOTKEY_KEY_GRACE).as_millis() as u64, Ordering::Relaxed);
                let clipboard = platform::Clipboard::new();
                match capture_selection(&clipboard, &mut platform::send_copy, &new_marker(), &CaptureOptions::default())
                {
                    Ok(capture) => {
                        let text = matches!(capture, Capture::Text(_));
                        post(move |app| app.log("hotkey-capture-finished", &[("kind", kind_name(text).into())]));
                        text
                    }
                    Err(error) => {
                        post(move |app| app.log("hotkey-capture-failed", &[("error", error)]));
                        true
                    }
                }
            } else {
                post(|app| app.log("hotkey-modifiers-held", &[]));
                true
            };
            capturing.store(false, Ordering::Relaxed);
            if show {
                post(|app| {
                    let cursor = platform::cursor_position();
                    app.show_bar(cursor, cursor);
                });
            }
        });
    }

    // MARK: Running an action

    fn run_action(self: &Rc<Self>, action: ActionId) {
        self.log("action-clicked", &[("action", action.key().into())]);
        self.hide_bar();
        if self.popup_open.get() && self.session.borrow().is_some() {
            self.focus_popup();
            return;
        }
        if self.capturing.load(Ordering::Relaxed) {
            return;
        }
        if !platform::has_permission(false) {
            self.show_message(permission_message(), None);
            return;
        }
        self.capturing.store(true, Ordering::Relaxed);
        let capturing = self.capturing.clone();
        let source = self.source_app.lock().unwrap().clone();
        thread::spawn(move || {
            let refocused = return_focus_to(source.as_deref());
            let started = Instant::now();
            let mut sends = 0;
            let options = if refocused {
                // A reactivated app can report itself frontmost before it accepts the copy shortcut.
                CaptureOptions { timeout: REFOCUSED_COPY_TIMEOUT, resend: Some(COPY_RESEND), ..Default::default() }
            } else {
                CaptureOptions::default()
            };
            let clipboard = platform::Clipboard::new();
            let result = capture_selection(
                &clipboard,
                &mut || {
                    sends += 1;
                    platform::send_copy()
                },
                &new_marker(),
                &options,
            );
            let ms = started.elapsed().as_millis();
            capturing.store(false, Ordering::Relaxed);
            post(move |app| match result {
                Ok(capture) => {
                    let length = if let Capture::Text(text) = &capture { text.chars().count() } else { 0 };
                    let kind = match &capture {
                        Capture::Text(_) => "text",
                        Capture::NoSelection => "noSelection",
                        Capture::NoText => "noText",
                    };
                    app.log(
                        "capture-finished",
                        &[
                            ("kind", kind.into()),
                            ("length", length.to_string()),
                            ("copySends", sends.to_string()),
                            ("ms", ms.to_string()),
                        ],
                    );
                    match capture {
                        Capture::Text(text) => app.start_session(action, text),
                        Capture::NoText => app.show_message(
                            PopupMessage {
                                title: "The selection cannot be read",
                                detail: "The copied content has no text.".into(),
                                button: Message::None,
                            },
                            app.selection_point.get(),
                        ),
                        Capture::NoSelection => {}
                    }
                }
                Err(error) => {
                    app.log("capture-failed", &[("error", error.clone())]);
                    app.show_message(
                        PopupMessage {
                            title: "The selection could not be copied",
                            detail: error,
                            button: Message::None,
                        },
                        app.selection_point.get(),
                    );
                }
            });
        });
    }

    fn start_session(self: &Rc<Self>, action: ActionId, selected_text: String) {
        let settings = self.settings.borrow().clone();
        let style = self.style();
        let mut session = ActionSession::new(SessionConfig {
            action,
            selected_text,
            primary_language: settings.primary_language,
            secondary_language: settings.secondary_language,
            load_style: Box::new(move || style.load()),
        });
        let run = session.start();
        *self.session.borrow_mut() = Some(session);
        self.session_number.set(self.session_number.get() + 1);
        self.copied.set(false);
        self.last_phase.set(None);
        let info = action.info();
        self.popup.set_kind("session".into());
        self.popup.set_heading(info.title.into());
        self.popup.set_action(action.key().into());
        self.popup.set_editable(info.editable);
        self.popup.set_input_label(info.input_label.into());
        self.render_session();
        self.open_popup(self.selection_point.get());
        self.launch(run);
    }

    /// Starts the CLI for a run, stopping any run still going.
    fn launch(&self, run: Run) {
        self.render_session();
        if let Some(previous) = self.cli.borrow_mut().take() {
            previous.cancel();
        }
        let Some(action) = self.session.borrow().as_ref().map(ActionSession::action) else { return };
        let model = action.model(self.settings.borrow().model);
        let options = CliOptions {
            executable: self.settings.borrow().cli_path.clone(),
            prefix_args: Vec::new(),
            model: model.key().into(),
            system: run.request.system,
            prompt: run.request.prompt,
            cwd: self.data_dir.clone(),
            inactivity: CLI_INACTIVITY,
        };
        let id = run.id;
        let cli = run_cli(options, move |event| {
            post(move |app| {
                let changed = match app.session.borrow_mut().as_mut() {
                    Some(session) => match event {
                        CliEvent::Text(text) => session.on_text(id, &text),
                        CliEvent::Done(result) => session.on_done(id, result),
                    },
                    None => false,
                };
                if changed {
                    app.render_session();
                }
            })
        });
        *self.cli.borrow_mut() = Some(cli);
    }

    fn render_session(&self) {
        let session = self.session.borrow();
        let Some(session) = session.as_ref() else { return };
        let state = &session.state;
        let generating = state.phase == Phase::Generating;
        if generating {
            self.copied.set(false);
        }
        let popup = &self.popup;
        // Edits live in the text area, so only generation updates may replace its content.
        if generating || self.last_phase.get() != Some(state.phase) {
            popup.set_result(state.result.as_str().into());
            if generating {
                popup.invoke_scroll_to_end();
            }
        }
        popup.set_generating(generating);
        popup.set_failed(state.phase == Phase::Failed);
        let error = state.error.as_ref();
        popup.set_error_text(error.map(|e| e.message.as_str()).unwrap_or_default().into());
        popup.set_error_needs_settings(
            error.is_some_and(|e| matches!(e.kind, CliErrorKind::NotFound | CliErrorKind::Unsupported)),
        );
        popup.set_copied(self.copied.get());
        let editable = session.action().info().editable;
        let can_copy = !generating && !popup.get_result().trim().is_empty();
        let hints: Vec<Hint> = if editable && can_copy {
            vec![hint("⏎", "Copy"), hint("⇧⏎", "New line"), hint("esc", "Close")]
        } else {
            vec![hint("esc", "Close")]
        };
        popup.set_hints(ModelRc::new(VecModel::from(hints)));
        if self.last_phase.get() == Some(Phase::Generating) && state.phase == Phase::Ready {
            popup.invoke_focus_after_ready();
        }
        self.last_phase.set(Some(state.phase));
    }

    fn copy_result(&self) {
        if self.copied.get() {
            return;
        }
        let text = self.popup.get_result().to_string();
        let generating = self.session.borrow().as_ref().is_none_or(|s| s.state.phase == Phase::Generating);
        if generating || text.trim().is_empty() {
            return;
        }
        platform::write_clipboard_text(&text);
        self.copied.set(true);
        self.render_session();
        // Leave the "Copied" confirmation on screen briefly before closing.
        let number = self.session_number.get();
        Timer::single_shot(COPIED_CLOSE_DELAY, move || {
            with_app(|app| {
                if app.session_number.get() == number && app.session.borrow().is_some() {
                    app.close_popup();
                }
            })
        });
    }

    // MARK: Popup

    fn show_message(&self, message: PopupMessage, anchor: Option<Point>) {
        if self.popup_open.get() && self.session.borrow().is_some() {
            return;
        }
        self.popup.set_kind("message".into());
        self.popup.set_message_title(message.title.into());
        self.popup.set_message_detail(message.detail.into());
        self.popup.set_message_button(
            match message.button {
                Message::Accessibility => "accessibility",
                Message::Settings => "settings",
                Message::None => "",
            }
            .into(),
        );
        self.open_popup(anchor);
    }

    fn focus_popup(&self) {
        platform::activate_self();
        let _ = self.popup.show();
        self.popup.window().with_winit_window(|window| window.focus_window());
    }

    /// Points the popup at the selection, or centers it without an arrow when there is none.
    fn open_popup(&self, anchor: Option<Point>) {
        if self.popup_open.get() {
            self.focus_popup();
            return;
        }
        let point = anchor.unwrap_or_else(platform::cursor_position);
        let display = platform::display_at(point);
        let work_area = display.logical_work_area();
        let (position, side, arrow_x) = match anchor {
            Some(anchor) => {
                let placement = popup_placement(display.to_logical(anchor), work_area);
                let side = if placement.side == PopupSide::Below { "below" } else { "above" };
                (Point { x: placement.x, y: placement.y }, side, placement.arrow_x)
            }
            None => (popup_centered(work_area), "", 0.0),
        };
        self.popup.set_side(side.into());
        self.popup.set_arrow_x(arrow_x as f32);
        self.popup.window().set_position(platform::window_position(display.to_screen(position), &display));
        self.refocus_on_close.set(true);
        self.popup_open.set(true);
        let _ = self.popup.show();
        platform::configure_floating(self.popup.window());
        self.popup.set_shown(true);
        self.focus_popup();
    }

    fn close_popup(&self) {
        if !self.popup_open.replace(false) {
            return;
        }
        self.popup.set_shown(false);
        let _ = self.popup.hide();
        self.session.borrow_mut().take();
        if let Some(cli) = self.cli.borrow_mut().take() {
            cli.cancel();
        }
        self.copied.set(false);
        if self.refocus_on_close.get() {
            let source = self.source_app.lock().unwrap().clone();
            thread::spawn(move || return_focus_to(source.as_deref()));
        }
    }

    // MARK: Settings

    fn apply_hotkey(&self) {
        let mut manager = self.hotkeys.borrow_mut();
        if manager.is_none() {
            match GlobalHotKeyManager::new() {
                Ok(created) => *manager = Some(created),
                Err(error) => self.log("hotkey-error", &[("error", error.to_string())]),
            }
        }
        let Some(manager) = manager.as_ref() else { return };
        if let Some(previous) = self.hotkey.borrow_mut().take() {
            let _ = manager.unregister(previous);
        }
        let name = self.settings.borrow().hotkey.clone();
        let hotkey = parse_hotkey(&name);
        let registered = manager.register(hotkey).is_ok();
        self.log("hotkey-registered", &[("hotkey", name.clone()), ("registered", registered.to_string())]);
        if registered {
            *self.hotkey.borrow_mut() = Some(hotkey);
        } else {
            self.show_message(
                PopupMessage {
                    title: "The hotkey is not available",
                    detail: format!("Another app may be using {name}. Choose a different hotkey in Settings."),
                    button: Message::Settings,
                },
                None,
            );
        }
    }

    fn open_settings(&self) {
        if !self.settings_open.get() {
            let window = &self.settings_window;
            let settings = self.settings.borrow().clone();
            let hotkeys: Vec<SharedString> = HOTKEYS.iter().map(|h| hotkey_label(h).into()).collect();
            let models: Vec<SharedString> = Model::ALL.iter().map(|m| m.label().into()).collect();
            window.set_hotkey_labels(ModelRc::new(VecModel::from(hotkeys)));
            window.set_model_labels(ModelRc::new(VecModel::from(models)));
            window.set_hook_running(self.hook_running.get());
            self.fill_settings_form(&settings);
            self.refresh_settings_form(None);
            self.settings_open.set(true);
        }
        let _ = self.settings_window.show();
        platform::configure_floating(self.settings_window.window());
        platform::activate_self();
        self.settings_window.window().with_winit_window(|window| window.focus_window());
    }

    fn fill_settings_form(&self, settings: &Settings) {
        let window = &self.settings_window;
        window.set_hotkey_index(HOTKEYS.iter().position(|h| *h == settings.hotkey).unwrap_or(0) as i32);
        window.set_model_index(Model::ALL.iter().position(|m| *m == settings.model).unwrap_or(0) as i32);
        window.set_auto_bar(settings.auto_bar);
        window.set_primary_language(settings.primary_language.as_str().into());
        window.set_secondary_language(settings.secondary_language.as_str().into());
        window.set_cli_path(settings.cli_path.as_str().into());
    }

    fn read_settings_form(&self) -> Settings {
        let window = &self.settings_window;
        Settings {
            hotkey: HOTKEYS.get(window.get_hotkey_index() as usize).unwrap_or(&HOTKEYS[0]).to_string(),
            model: *Model::ALL.get(window.get_model_index() as usize).unwrap_or(&Model::Sonnet),
            cli_path: window.get_cli_path().trim().to_string(),
            primary_language: window.get_primary_language().trim().to_string(),
            secondary_language: window.get_secondary_language().trim().to_string(),
            auto_bar: window.get_auto_bar(),
            style_template_installed: self.settings.borrow().style_template_installed,
        }
    }

    /// Refreshes the validation state and the Save button after any change.
    fn refresh_settings_form(&self, status: Option<(&str, &str)>) {
        let next = self.read_settings_form();
        let missing = [
            (next.primary_language.is_empty(), "Primary language"),
            (next.secondary_language.is_empty(), "Secondary language"),
            (next.cli_path.is_empty(), "Claude CLI path"),
        ]
        .into_iter()
        .find(|(empty, _)| *empty)
        .map(|(_, name)| name);
        let changed = next != *self.settings.borrow();
        let window = &self.settings_window;
        window.set_can_save(changed && missing.is_none());
        match (missing, status) {
            (Some(name), _) => {
                window.set_status(format!("{name} is required.").into());
                window.set_status_kind("error".into());
            }
            (None, Some((text, kind))) => {
                window.set_status(text.into());
                window.set_status_kind(kind.into());
            }
            (None, None) => {
                window.set_status(SharedString::new());
                window.set_status_kind(SharedString::new());
            }
        }
    }

    fn save_settings(&self) {
        let next = self.read_settings_form();
        let previous_hotkey = self.settings.borrow().hotkey.clone();
        match settings::save_settings(&self.settings_path(), &next) {
            Ok(()) => {
                *self.settings.borrow_mut() = next.clone();
                self.fill_settings_form(&next);
                self.refresh_settings_form(Some(("Saved", "saved")));
                if next.hotkey != previous_hotkey {
                    self.apply_hotkey();
                }
            }
            Err(error) => self.refresh_settings_form(Some((&format!("Could not save: {error}"), "error"))),
        }
    }

    fn edit_style(&self) {
        let store = self.style();
        let _ = store.install_template();
        platform::open_path(&store.path);
    }
}

fn hint(key: &str, label: &str) -> Hint {
    Hint { key: key.into(), label: label.into() }
}

fn kind_name(text: bool) -> &'static str {
    if text { "text" } else { "none" }
}

/// Clicking the bar can make this app frontmost on macOS, which would send the copy here.
fn return_focus_to(source: Option<&str>) -> bool {
    if !IS_MAC {
        return false;
    }
    let current = platform::frontmost_bundle_id();
    let refocus = crate::focus::should_refocus(source, current.as_deref(), platform::OWN_BUNDLE_ID);
    let details = [
        ("source", source.unwrap_or("unknown").to_string()),
        ("current", current.clone().unwrap_or_else(|| "unknown".into())),
        ("refocus", refocus.to_string()),
    ];
    post(move |app| app.log("refocus-checked", &details));
    if !refocus {
        return false;
    }
    if let Some(source) = source {
        platform::activate_app(source);
    }
    thread::sleep(REFOCUS_SETTLE);
    true
}

fn decode_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    buffer.truncate(info.buffer_size());
    Some((buffer, info.width, info.height))
}
