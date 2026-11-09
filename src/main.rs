// A release build on Windows is a GUI app with no console window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod actions;
mod app;
mod capture;
mod cli;
mod focus;
mod geometry;
mod held_keys;
mod platform;
mod prompt;
mod selection_detector;
mod session;
mod settings;
mod store;

slint::include_modules!();

use slint::winit_030::winit::window::WindowAttributes;
use std::cell::Cell;
use std::fs::File;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
pub enum WindowKind {
    Other,
    Bar,
    Popup,
    Settings,
}

thread_local! {
    static NEXT_WINDOW: Cell<WindowKind> = const { Cell::new(WindowKind::Other) };
}

/// Creates a window with the attributes its kind needs; Slint calls the hook while `create` runs.
pub fn with_window_kind<T>(kind: WindowKind, create: impl FnOnce() -> T) -> T {
    NEXT_WINDOW.with(|next| next.set(kind));
    let window = create();
    NEXT_WINDOW.with(|next| next.set(WindowKind::Other));
    window
}

fn window_attributes(attributes: WindowAttributes) -> WindowAttributes {
    match NEXT_WINDOW.with(Cell::get) {
        WindowKind::Bar => attributes.with_active(false).with_resizable(false),
        WindowKind::Popup | WindowKind::Settings => attributes.with_resizable(false),
        WindowKind::Other => attributes,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = dirs::config_dir().ok_or("no data folder")?;
    let data_dir = config.join("Fluent");
    std::fs::create_dir_all(&data_dir)?;

    // A second copy would add a second tray icon and fight over the hotkey.
    let lock = File::create(data_dir.join("instance.lock"))?;
    if lock.try_lock().is_err() {
        return Ok(());
    }

    let migration = store::migrate_legacy_data(&config.join(store::LEGACY_DATA_FOLDER), &data_dir);
    let settings = settings::load_settings(&data_dir.join("settings.json"), &settings::defaults());
    let log_path = data_dir.join("diagnostics.log");
    store::log_event(
        &log_path,
        "started",
        &[
            ("platform", std::env::consts::OS.into()),
            ("arch", std::env::consts::ARCH.into()),
            ("version", env!("CARGO_PKG_VERSION").into()),
        ],
    );
    if !migration.copied.is_empty() || !migration.failed.is_empty() {
        store::log_event(
            &log_path,
            "legacy-data-migrated",
            &[("copied", migration.copied.join(",")), ("failed", migration.failed.join(","))],
        );
    }

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(window_attributes)
        .select()?;

    let app = app::App::new(data_dir, settings)?;
    slint::Timer::single_shot(Duration::ZERO, move || app.start());
    slint::run_event_loop_until_quit()?;
    drop(lock);
    Ok(())
}
