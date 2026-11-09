//! Renders the app's windows offscreen with Slint's software renderer, so the UI can be
//! checked without a display or permissions. Writes PNGs and a GIF to the folder given:
//!
//!   cargo run --example screenshots -- <out-dir>

slint::include_modules!();

use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Default)]
struct Offscreen {
    last: Rc<RefCell<Option<Rc<MinimalSoftwareWindow>>>>,
}

impl Platform for Offscreen {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        *self.last.borrow_mut() = Some(window.clone());
        Ok(window)
    }
}

const DESKTOP: [u8; 3] = [0xd8, 0xdc, 0xe3];

struct Shot {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

fn render(window: &MinimalSoftwareWindow, logical: (f32, f32), scale: f32) -> Shot {
    // Lets property animations settle before the frame is drawn.
    std::thread::sleep(std::time::Duration::from_millis(160));
    window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: scale });
    let (width, height) = ((logical.0 * scale) as u32, (logical.1 * scale) as u32);
    window.set_size(slint::PhysicalSize::new(width, height));
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let mut buffer = vec![PremultipliedRgbaColor::default(); (width * height) as usize];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, width as usize);
    });
    // The renderer leaves see-through pixels premultiplied; lay them over the desktop color.
    let over = |c: u8, d: u8, a: u8| (c as u32 + d as u32 * (255 - a as u32) / 255).min(255) as u8;
    let rgba = buffer
        .iter()
        .flat_map(|p| {
            [
                over(p.red, DESKTOP[0], p.alpha),
                over(p.green, DESKTOP[1], p.alpha),
                over(p.blue, DESKTOP[2], p.alpha),
                255,
            ]
        })
        .collect();
    Shot { rgba, width, height }
}

fn save_png(shot: &Shot, path: &Path) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), shot.width, shot.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&shot.rgba).unwrap();
}

fn hints(items: &[(&str, &str)]) -> ModelRc<Hint> {
    ModelRc::new(VecModel::from(
        items.iter().map(|(key, label)| Hint { key: (*key).into(), label: (*label).into() }).collect::<Vec<_>>(),
    ))
}

const REPLY: &str =
    "Chắc chắn rồi, chiều nay mình sẽ xem PR của bạn. Nếu thấy chỗ nào cần sửa mình sẽ để lại comment nhé.";

fn session(popup: &Popup, action: &str, heading: &str, editable: bool, label: &str) {
    popup.set_kind("session".into());
    popup.set_side("below".into());
    popup.set_arrow_x(200.0);
    popup.set_action(action.into());
    popup.set_heading(heading.into());
    popup.set_editable(editable);
    popup.set_input_label(label.into());
    popup.set_failed(false);
    popup.set_copied(false);
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("usage: screenshots <out-dir>"));
    std::fs::create_dir_all(&out).unwrap();
    let platform = Offscreen::default();
    let last = platform.last.clone();
    slint::platform::set_platform(Box::new(platform)).unwrap();
    let take = || last.borrow().clone().unwrap();

    // Icon bar
    let bar = Bar::new().unwrap();
    bar.show().unwrap();
    bar.set_shown(true);
    std::thread::sleep(std::time::Duration::from_millis(300));
    save_png(&render(&take(), (132.0, 40.0), 2.0), &out.join("1-bar.png"));
    let with_vocab: Vec<slint::SharedString> = ["draft", "explain", "translate", "vocab"].map(Into::into).to_vec();
    bar.set_actions(ModelRc::new(VecModel::from(with_vocab)));
    save_png(&render(&take(), (172.0, 40.0), 2.0), &out.join("1-bar-vocab.png"));

    let popup = Popup::new().unwrap();
    popup.show().unwrap();
    popup.set_shown(true);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let popup_window = take();
    let size = (536.0, 386.0);

    // A draft streaming in, then ready, then copied: the frames of the GIF.
    session(&popup, "draft", "Draft reply", true, "Rewrite: shorter, decline politely…");
    let mut frames = Vec::new();
    let words: Vec<&str> = REPLY.split(' ').collect();
    for count in (0..=words.len()).step_by(3) {
        popup.set_generating(true);
        popup.set_result(words[..count].join(" ").into());
        popup.set_hints(hints(&[("esc", "Close")]));
        frames.push((render(&popup_window, size, 1.5), 18));
        if count == 9 {
            save_png(&render(&popup_window, size, 2.0), &out.join("2-draft-writing.png"));
        }
    }
    popup.set_generating(false);
    popup.set_result(REPLY.into());
    popup.set_hints(hints(&[("⏎", "Copy"), ("⇧⏎", "New line"), ("esc", "Close")]));
    save_png(&render(&popup_window, size, 2.0), &out.join("2-draft-ready.png"));
    frames.push((render(&popup_window, size, 1.5), 150));
    popup.set_copied(true);
    save_png(&render(&popup_window, size, 2.0), &out.join("3-draft-copied.png"));
    frames.push((render(&popup_window, size, 1.5), 150));

    // Explain is read-only, with a follow-up question in the input.
    session(&popup, "explain", "Explain", false, "Ask a follow-up question…");
    popup.set_side("above".into());
    popup.set_arrow_x(380.0);
    popup.set_generating(false);
    popup.set_result("Câu này nói rằng lần deploy bị lỗi vì hai tác vụ chạy cùng lúc và tranh nhau trong bước migration (cập nhật cấu trúc cơ sở dữ liệu).".into());
    popup.set_hints(hints(&[("esc", "Close")]));
    save_png(&render(&popup_window, size, 2.0), &out.join("4-explain.png"));
    frames.push((render(&popup_window, size, 1.5), 150));

    // A failed run that needs the settings.
    session(&popup, "translate", "Translate", true, "Rewrite: more casual, shorter…");
    popup.set_side("".into());
    popup.set_failed(true);
    popup.set_result("".into());
    popup.set_error_text("Claude CLI not found at /Users/a/.local/bin/claude.".into());
    popup.set_error_needs_settings(true);
    save_png(&render(&popup_window, size, 2.0), &out.join("5-translate-failed.png"));

    // The permission message.
    popup.set_kind("message".into());
    popup.set_message_title("Fluent needs Accessibility access".into());
    popup.set_message_detail("It uses this to detect selections and copy your selected text. Turn on Fluent in System Settings, then try again.".into());
    popup.set_message_button("accessibility".into());
    save_png(&render(&popup_window, size, 2.0), &out.join("6-permission.png"));
    frames.push((render(&popup_window, size, 1.5), 150));

    // Add to vocabulary: suggesting, ready, then a failed save.
    popup.set_kind("vocab".into());
    popup.set_side("below".into());
    popup.set_arrow_x(268.0);
    popup.set_vocab_phrase("ubiquitous".into());
    popup.set_vocab_suggesting(true);
    save_png(&render(&popup_window, size, 2.0), &out.join("10-vocab-suggesting.png"));
    popup.set_vocab_suggesting(false);
    popup.set_vocab_meaning("có mặt ở khắp nơi".into());
    popup.set_vocab_example("Today smartphones are ubiquitous in daily life.".into());
    popup.set_vocab_example_vi("Ngày nay điện thoại thông minh có mặt ở khắp nơi trong đời sống.".into());
    popup.set_vocab_can_save(true);
    save_png(&render(&popup_window, size, 2.0), &out.join("11-vocab-ready.png"));
    popup.set_vocab_error("The inbox returned 500. Your entry is kept; try again.".into());
    save_png(&render(&popup_window, size, 2.0), &out.join("12-vocab-failed.png"));
    popup.set_vocab_error("".into());
    popup.set_vocab_saved(true);
    save_png(&render(&popup_window, size, 2.0), &out.join("13-vocab-saved.png"));
    popup.set_vocab_saved(false);

    // Dark mode.
    popup.global::<Palette>().set_color_scheme(slint::language::ColorScheme::Dark);
    session(&popup, "translate", "Translate", true, "Rewrite: more casual, shorter…");
    popup.set_generating(false);
    popup.set_result("I deployed the fix to staging. Can you test it again and then merge the PR for me?".into());
    popup.set_hints(hints(&[("⏎", "Copy"), ("⇧⏎", "New line"), ("esc", "Close")]));
    save_png(&render(&popup_window, size, 2.0), &out.join("7-translate-dark.png"));
    frames.push((render(&popup_window, size, 1.5), 150));

    // Settings, in light and dark.
    let settings = SettingsWindow::new().unwrap();
    settings.show().unwrap();
    let settings_window = take();
    settings.set_hotkey_labels(ModelRc::new(VecModel::from(vec![
        "Control + Option + R".into(),
        "Control + Option + D".into(),
        "Control + Shift + Space".into(),
    ])));
    settings
        .set_model_labels(ModelRc::new(VecModel::from(vec!["Sonnet: better writing".into(), "Haiku: faster".into()])));
    settings.set_auto_bar(true);
    settings.set_primary_language("Vietnamese".into());
    settings.set_secondary_language("English".into());
    settings.set_cli_path("/Users/a/.local/bin/claude".into());
    settings.set_inbox_url("https://abc123.mockapi.io/api/v1/inbox".into());
    settings.set_status("Saved".into());
    settings.set_status_kind("saved".into());
    save_png(&render(&settings_window, (500.0, 640.0), 2.0), &out.join("8-settings.png"));
    settings.global::<Palette>().set_color_scheme(slint::language::ColorScheme::Dark);
    settings.set_hook_running(false);
    settings.set_can_save(true);
    settings.set_status("".into());
    settings.set_status_kind("".into());
    save_png(&render(&settings_window, (500.0, 640.0), 2.0), &out.join("9-settings-dark.png"));

    // The GIF, from the popup frames.
    let first = &frames[0].0;
    let file = std::fs::File::create(out.join("popup.gif")).unwrap();
    let mut encoder = gif::Encoder::new(file, first.width as u16, first.height as u16, &[]).unwrap();
    encoder.set_repeat(gif::Repeat::Infinite).unwrap();
    for (mut shot, delay) in frames {
        let mut frame = gif::Frame::from_rgba_speed(shot.width as u16, shot.height as u16, &mut shot.rgba, 10);
        frame.delay = delay;
        encoder.write_frame(&frame).unwrap();
    }
    println!("{}", out.display());
}
