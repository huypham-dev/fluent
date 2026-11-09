//! Shows the icon bar and the popup with the real windowing backend and renderer, then
//! saves what the renderer drew. Checks rendering that the offscreen screenshots cannot.
//!
//!   cargo run --example snapshot -- <out-dir>

slint::include_modules!();

use slint::ComponentHandle;
use std::path::{Path, PathBuf};

fn save(window: &slint::Window, path: &Path) {
    let shot = window.take_snapshot().expect("snapshot");
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), shot.width(), shot.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(shot.as_bytes()).unwrap();
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("usage: snapshot <out-dir>"));
    std::fs::create_dir_all(&out).unwrap();
    let bar = Bar::new().unwrap();
    let popup = Popup::new().unwrap();
    bar.show().unwrap();
    bar.set_shown(true);
    popup.set_kind("session".into());
    popup.set_side("below".into());
    popup.set_action("translate".into());
    popup.set_heading("Translate".into());
    popup.set_editable(true);
    popup.set_input_label("Rewrite: more casual, shorter…".into());
    popup.set_result("Deploy bị lỗi do race condition ở bước migration.".into());
    popup.show().unwrap();
    popup.set_shown(true);
    let (bar_weak, popup_weak) = (bar.as_weak(), popup.as_weak());
    slint::Timer::single_shot(std::time::Duration::from_millis(800), move || {
        save(bar_weak.unwrap().window(), &out.join("bar.png"));
        save(popup_weak.unwrap().window(), &out.join("popup.png"));
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop().unwrap();
}
