//! Renders the README demo GIF: a scripted run of the real icon bar and popup over a
//! mock chat app. The app's windows are drawn offscreen with Slint's software renderer
//! on a simulated clock, so their animations play at demo speed and every run is the same.
//!
//!   cargo run --release --example demo -- docs/demo.gif
//!   cargo run --release --example demo -- <out.png> <seconds>   (one still frame)

slint::include_modules!();

mod scene {
    slint::slint! {
        component Msg inherits Rectangle {
            in property <string> who;
            in property <string> time;
            in property <string> body;
            in property <color> avatar;
            in property <string> initial;
            in property <float> sel;
            out property <length> text-x: body-text.absolute-position.x;
            out property <length> text-y: body-text.absolute-position.y;
            out property <length> text-w: body-text.width;
            out property <length> text-h: body-text.height;
            height: 46px;
            Rectangle {
                width: 34px;
                height: 34px;
                x: 0;
                y: 2px;
                border-radius: 8px;
                background: root.avatar;
                Text { text: root.initial; color: white; font-size: 14px; font-weight: 600; }
            }
            Text {
                x: 46px;
                y: 0;
                text: root.who;
                font-size: 13.5px;
                font-weight: 600;
                color: #1d1d1f;
            }
            Text {
                x: 46px + who-width.preferred-width + 8px;
                y: 1.5px;
                text: root.time;
                font-size: 12px;
                color: #8a8a90;
            }
            who-width := Text { visible: false; text: root.who; font-size: 13.5px; font-weight: 600; }
            Rectangle {
                x: body-text.x;
                y: body-text.y;
                width: body-text.width * root.sel;
                height: body-text.height;
                background: #b4d5fe;
            }
            body-text := Text {
                x: 46px;
                y: 22px;
                width: self.preferred-width;
                text: root.body;
                font-size: 15px;
                color: #1d1d1f;
            }
        }

        export component Desktop inherits Window {
            in property <string> caption;
            in property <float> caption-opacity: 1;
            in property <float> sel1;
            in property <float> sel2;
            in property <float> sel3;
            out property <length> m1-x: m1.text-x;
            out property <length> m1-y: m1.text-y;
            out property <length> m1-w: m1.text-w;
            out property <length> m1-h: m1.text-h;
            out property <length> m2-x: m2.text-x;
            out property <length> m2-y: m2.text-y;
            out property <length> m2-w: m2.text-w;
            out property <length> m2-h: m2.text-h;
            out property <length> m3-x: m3.text-x;
            out property <length> m3-y: m3.text-y;
            out property <length> m3-w: m3.text-w;
            out property <length> m3-h: m3.text-h;
            width: 1120px;
            height: 720px;
            background: @linear-gradient(135deg, #7d9cf2 0%, #a993ea 55%, #e2a3c8 100%);

            Rectangle {
                x: 110px;
                y: 22px;
                width: 900px;
                height: 630px;
                border-radius: 12px;
                background: white;
                drop-shadow-blur: 40px;
                drop-shadow-offset-y: 16px;
                drop-shadow-color: #00000040;
                clip: true;

                Rectangle {
                    width: 200px;
                    x: 0;
                    background: #f3f3f5;
                    Rectangle { x: 199px; width: 1px; background: #e3e3e6; }
                    for c[i] in [#ff5f57, #febc2e, #28c840]: Rectangle {
                        x: 16px + i * 20px;
                        y: 16px;
                        width: 12px;
                        height: 12px;
                        border-radius: 6px;
                        background: c;
                    }
                    VerticalLayout {
                        x: 12px;
                        y: 48px;
                        width: 176px;
                        height: self.preferred-height;
                        spacing: 2px;
                        for item in [
                            { label: "CHANNELS", kind: 0 },
                            { label: "# general", kind: 1 },
                            { label: "# product-team", kind: 2 },
                            { label: "# releases", kind: 1 },
                            { label: "", kind: 3 },
                            { label: "DIRECT MESSAGES", kind: 0 },
                            { label: "Anna Lee", kind: 1 },
                            { label: "Linh Nguyen", kind: 1 },
                        ]: Rectangle {
                            height: item.kind == 3 ? 10px : 30px;
                            border-radius: 6px;
                            background: item.kind == 2 ? #dfe7fb : transparent;
                            Text {
                                x: 8px;
                                text: item.label;
                                font-size: item.kind == 0 ? 11px : 13px;
                                font-weight: item.kind == 0 ? 600 : item.kind == 2 ? 500 : 400;
                                color: item.kind == 0 ? #8a8a90 : item.kind == 2 ? #1d1d1f : #55555b;
                                vertical-alignment: center;
                            }
                        }
                    }
                }

                Rectangle {
                    x: 200px;
                    width: 700px;
                    Rectangle {
                        y: 0;
                        height: 52px;
                        Text { x: 20px; text: "# product-team"; font-size: 14px; font-weight: 600; color: #1d1d1f; vertical-alignment: center; }
                        Rectangle { y: 51px; height: 1px; background: #ececef; }
                    }
                    m1 := Msg {
                        x: 24px; y: 72px; width: 650px;
                        who: "Anna Lee"; initial: "A"; time: "9:41 AM"; avatar: #e07a5f; sel: root.sel1;
                        body: "Can you review my PR this afternoon? It touches the migration step.";
                    }
                    m2 := Msg {
                        x: 24px; y: 136px; width: 650px;
                        who: "Minh Tran"; initial: "M"; time: "10:02 AM"; avatar: #3d85c6; sel: root.sel2;
                        body: "The deploy failed because of a race condition in the migration step.";
                    }
                    Msg {
                        x: 24px; y: 200px; width: 650px;
                        who: "Jordan Kim"; initial: "J"; time: "10:15 AM"; avatar: #81b29a;
                        body: "Nice catch. I'll keep an eye on CI today.";
                    }
                    m3 := Msg {
                        x: 24px; y: 488px; width: 650px;
                        who: "Linh Nguyen"; initial: "L"; time: "11:20 AM"; avatar: #9b72cf; sel: root.sel3;
                        body: "Mình đã deploy bản fix lên staging, bạn test lại rồi merge PR giúp mình nhé";
                    }
                    Rectangle {
                        x: 24px;
                        y: 562px;
                        width: 652px;
                        height: 42px;
                        border-radius: 10px;
                        border-width: 1px;
                        border-color: #dcdce0;
                        Text { x: 14px; text: "Message #product-team"; font-size: 13.5px; color: #a0a0a6; vertical-alignment: center; }
                    }
                }
            }

            Rectangle {
                y: 668px;
                height: 34px;
                width: caption-text.preferred-width + 34px;
                x: (parent.width - self.width) / 2;
                border-radius: 17px;
                background: #141418c0;
                opacity: root.caption-opacity;
                caption-text := Text {
                    text: root.caption;
                    font-size: 14px;
                    font-weight: 500;
                    color: white;
                }
            }
        }

        export component ArrowCursor inherits Window {
            width: 24px;
            height: 24px;
            background: transparent;
            Path {
                x: 0; y: 0; width: 24px; height: 24px;
                viewbox-width: 24; viewbox-height: 24;
                commands: "M 2 1.5 L 2 18.7 L 6.4 14.5 L 9.3 21.1 L 11.9 20 L 9.1 13.5 L 15.2 13.5 Z";
                fill: black;
                stroke: white;
                stroke-width: 1.3px;
            }
        }

        export component IBeamCursor inherits Window {
            width: 24px;
            height: 24px;
            background: transparent;
            for w[i] in [3.2, 1.3]: Path {
                x: 0; y: 0; width: 24px; height: 24px;
                viewbox-width: 24; viewbox-height: 24;
                commands: "M 8.5 4 L 10.5 4 L 12 5 L 13.5 4 L 15.5 4 M 12 5 L 12 19 M 8.5 20 L 10.5 20 L 12 19 L 13.5 20 L 15.5 20";
                stroke: i == 0 ? white : black;
                stroke-width: w * 1px;
                stroke-line-cap: round;
            }
        }
    }
}

use scene::{ArrowCursor, Desktop, IBeamCursor};
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, SharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// Hands out offscreen windows and a clock that only moves when the demo says so.
struct Offscreen {
    last: Rc<RefCell<Option<Rc<MinimalSoftwareWindow>>>>,
    clock: Rc<Cell<Duration>>,
}

impl Platform for Offscreen {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        *self.last.borrow_mut() = Some(window.clone());
        Ok(window)
    }

    fn duration_since_start(&self) -> Duration {
        self.clock.get()
    }
}

const WIDTH: usize = 1120;
const HEIGHT: usize = 720;
const FPS: f32 = 15.0;

struct Surface {
    window: Rc<MinimalSoftwareWindow>,
    width: usize,
    height: usize,
}

impl Surface {
    fn new(window: Rc<MinimalSoftwareWindow>, width: usize, height: usize) -> Self {
        window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
        window.set_size(slint::PhysicalSize::new(width as u32, height as u32));
        Surface { window, width, height }
    }

    fn draw(&self) -> Vec<PremultipliedRgbaColor> {
        let mut buffer = vec![PremultipliedRgbaColor::default(); self.width * self.height];
        self.window.request_redraw();
        self.window.draw_if_needed(|renderer| {
            renderer.render(&mut buffer, self.width);
        });
        buffer
    }

    fn hover(&self, at: Option<(f32, f32)>) {
        self.window.dispatch_event(match at {
            Some((x, y)) => WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) },
            None => WindowEvent::PointerExited,
        });
    }
}

/// Lays a premultiplied layer over an opaque RGB canvas.
fn blend(canvas: &mut [u8], layer: &[PremultipliedRgbaColor], width: usize, height: usize, at: (i32, i32)) {
    for y in 0..height {
        let cy = at.1 + y as i32;
        if cy < 0 || cy >= HEIGHT as i32 {
            continue;
        }
        for x in 0..width {
            let cx = at.0 + x as i32;
            if cx < 0 || cx >= WIDTH as i32 {
                continue;
            }
            let p = layer[y * width + x];
            if p.alpha == 0 {
                continue;
            }
            let i = (cy as usize * WIDTH + cx as usize) * 3;
            let keep = 255 - p.alpha as u32;
            for (c, s) in [p.red, p.green, p.blue].into_iter().enumerate() {
                canvas[i + c] = (s as u32 + canvas[i + c] as u32 * keep / 255).min(255) as u8;
            }
        }
    }
}

fn ease(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    if p < 0.5 { 2.0 * p * p } else { 1.0 - (-2.0 * p + 2.0).powi(2) / 2.0 }
}

fn progress(t: f32, a: f32, b: f32) -> f32 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

fn lerp(a: (f32, f32), b: (f32, f32), p: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * p, a.1 + (b.1 - a.1) * p)
}

fn hints(items: &[(&str, &str)]) -> ModelRc<Hint> {
    ModelRc::new(VecModel::from(
        items.iter().map(|(key, label)| Hint { key: (*key).into(), label: (*label).into() }).collect::<Vec<_>>(),
    ))
}

/// One selection-to-result run.
struct Scene {
    msg: usize,
    action: &'static str,
    caption: &'static str,
    /// Results in order: the first answer, then one per quick rewrite.
    results: &'static [&'static str],
    /// The quick chip clicked after the first answer, as a point inside the popup window.
    chip: Option<(f32, f32)>,
    copy: bool,
    rate: f32,
}

const SCENES: [Scene; 3] = [
    Scene {
        msg: 0,
        action: "draft",
        caption: "Draft reply  ·  in the language of the message, in your style",
        results: &[
            "Sure, I'll review it this afternoon. I'll look closely at the migration step and leave comments if anything needs a change.",
            "Sure, I'll review it this afternoon.",
        ],
        chip: Some((CHIP_X, CHIP_Y)),
        copy: true,
        rate: 60.0,
    },
    Scene {
        msg: 1,
        action: "explain",
        caption: "Explain  ·  in your primary language, with follow-up questions",
        results: &[
            "A race condition happens when two tasks run at the same time and the result depends on which one finishes first.\n\nHere, two parts of the deploy changed the database during the migration step (the step that updates the database structure) and got in each other's way, so the deploy failed.",
        ],
        chip: None,
        copy: false,
        rate: 120.0,
    },
    Scene {
        msg: 2,
        action: "translate",
        caption: "Translate  ·  into your primary language, or from it into your secondary one",
        results: &["I deployed the fix to staging. Can you test it again and then merge the PR for me?"],
        chip: None,
        copy: true,
        rate: 55.0,
    },
];

// Points inside the 536x446 popup window, measured from a rendered frame.
const CHIP_X: f32 = 46.0;
const CHIP_Y: f32 = 313.0;
const COPY_X: f32 = 448.0;
const COPY_Y: f32 = 398.0;

// Scene timing, in seconds from the scene start.
const REACH: f32 = 0.7;
const DRAG: (f32, f32) = (0.85, 2.0);
const TO_BUTTON: (f32, f32) = (2.35, 2.95);
const CLICK: f32 = 3.05;
const STREAM: f32 = 3.3;

struct Timing {
    /// When each result starts and finishes streaming.
    runs: Vec<(f32, f32)>,
    copy_at: Option<f32>,
    close_at: f32,
    duration: f32,
}

fn timing(scene: &Scene) -> Timing {
    let mut runs = Vec::new();
    let mut start = STREAM;
    for result in scene.results {
        let end = start + result.chars().count() as f32 / scene.rate;
        runs.push((start, end));
        start = end + 1.6;
    }
    let ready = runs.last().unwrap().1;
    let copy_at = scene.copy.then_some(ready + 1.1);
    let close_at = ready + 2.6;
    Timing { runs, copy_at, close_at, duration: close_at + 0.4 }
}

const INTRO: f32 = 0.5;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).expect("usage: demo <out.gif> | demo <out.png> <seconds>"));
    let still: Option<f32> = args.get(2).map(|s| s.parse().expect("seconds"));

    let clock = Rc::new(Cell::new(Duration::ZERO));
    let platform = Offscreen { last: Default::default(), clock: clock.clone() };
    let last = platform.last.clone();
    slint::platform::set_platform(Box::new(platform)).unwrap();
    let take = || last.borrow().clone().unwrap();

    let desktop = Desktop::new().unwrap();
    desktop.show().unwrap();
    let desktop_surface = Surface::new(take(), WIDTH, HEIGHT);
    let bar = Bar::new().unwrap();
    bar.show().unwrap();
    let bar_surface = Surface::new(take(), 132, 40);
    let popup = Popup::new().unwrap();
    popup.show().unwrap();
    let popup_surface = Surface::new(take(), 536, 446);
    let arrow = ArrowCursor::new().unwrap();
    arrow.show().unwrap();
    let arrow_layer = Surface::new(take(), 24, 24).draw();
    let ibeam = IBeamCursor::new().unwrap();
    ibeam.show().unwrap();
    let ibeam_layer = Surface::new(take(), 24, 24).draw();
    slint::platform::update_timers_and_animations();
    desktop_surface.draw();

    let texts = [
        (desktop.get_m1_x(), desktop.get_m1_y(), desktop.get_m1_w(), desktop.get_m1_h()),
        (desktop.get_m2_x(), desktop.get_m2_y(), desktop.get_m2_w(), desktop.get_m2_h()),
        (desktop.get_m3_x(), desktop.get_m3_y(), desktop.get_m3_w(), desktop.get_m3_h()),
    ];
    let timings: Vec<Timing> = SCENES.iter().map(timing).collect();
    let total = INTRO + timings.iter().map(|t| t.duration).sum::<f32>() + 0.5;

    let mut cursor = (1060.0, 640.0);
    let mut frames: Vec<(Vec<u8>, f32)> = Vec::new();
    let mut opened: Option<usize> = None;
    let mut current: Option<usize> = None;
    let mut scene_from = cursor;
    let count = (total * FPS).ceil() as usize;
    let range = match still {
        Some(t) => 0..((t * FPS) as usize + 1),
        None => 0..count,
    };
    for frame in range {
        let t = frame as f32 / FPS;
        clock.set(Duration::from_secs_f32(t));

        // Which scene, and the time inside it.
        let (mut index, mut start) = (0, INTRO);
        while index + 1 < SCENES.len() && t >= start + timings[index].duration {
            start += timings[index].duration;
            index += 1;
        }
        let (scene, time, lt) = (&SCENES[index], &timings[index], t - start);
        if current != Some(index) {
            current = Some(index);
            scene_from = cursor;
        }
        let (tx, ty, tw, th) = texts[scene.msg];
        let text_start = (tx, ty + th / 2.0);
        let release = (tx + tw, ty + th / 2.0);

        // Selection
        let sel = if lt >= time.close_at { 0.0 } else { ease(progress(lt, DRAG.0, DRAG.1)) };
        let sels = [0, 1, 2].map(|i| if i == scene.msg { sel } else { 0.0 });
        desktop.set_sel1(sels[0]);
        desktop.set_sel2(sels[1]);
        desktop.set_sel3(sels[2]);
        desktop.set_caption(scene.caption.into());
        desktop.set_caption_opacity(progress(t, 0.0, INTRO));

        // The bar sits above the release point, as geometry::bar_position places it.
        let bar_at = (release.0 - 66.0, release.1 - 12.0 - 40.0);
        let bar_visible = lt >= DRAG.1 && lt < CLICK;
        bar.set_shown(bar_visible);
        let button = ["draft", "explain", "translate"].iter().position(|a| *a == scene.action).unwrap();
        let button_point = (bar_at.0 + 4.0 + 20.0 + button as f32 * 42.0, bar_at.1 + 22.0);

        // The popup, placed as geometry::popup_placement does for a one-line selection.
        let anchor = ((text_start.0 + release.0) / 2.0, release.1);
        let below_y = anchor.1 + 12.0 - 18.0;
        let above = below_y + 446.0 > HEIGHT as f32;
        let popup_at = (
            (anchor.0 - 268.0).clamp(0.0, WIDTH as f32 - 536.0).round(),
            if above { anchor.1 - 12.0 + 18.0 - 446.0 } else { below_y }.round(),
        );
        if lt >= CLICK && lt < time.close_at {
            if opened != Some(index) {
                opened = Some(index);
                popup.set_kind("session".into());
                popup.set_side(if above { "above" } else { "below" }.into());
                popup.set_arrow_x((anchor.0 - popup_at.0).clamp(42.0, 494.0));
                popup.set_action(scene.action.into());
                popup.set_heading(
                    match scene.action {
                        "draft" => "Draft reply",
                        "explain" => "Explain",
                        _ => "Translate",
                    }
                    .into(),
                );
                let editable = scene.action == "draft";
                popup.set_editable(editable);
                popup.set_input_label(
                    if scene.action == "explain" {
                        "Ask a follow-up question…"
                    } else {
                        "Or say how to rewrite it…"
                    }
                    .into(),
                );
                let body = desktop_text(&desktop, scene.msg);
                popup.set_source(body);
                let quick: &[&str] = match scene.action {
                    "draft" => &["Shorter", "Friendlier", "More formal", "Decline politely"],
                    "explain" => &["Simpler", "Give an example", "Shorter"],
                    _ => &["More casual", "More formal", "Shorter", "Friendlier"],
                };
                let quick: Vec<SharedString> = quick.iter().map(|q| (*q).into()).collect();
                popup.set_quick(ModelRc::new(VecModel::from(quick)));
                popup.set_failed(false);
                popup.set_copied(false);
                popup.set_result("".into());
                popup.set_shown(true);
            }
            // Streams the current result in, or shows the last finished one.
            let run = time.runs.iter().rposition(|(s, _)| lt >= *s);
            let generating = match run {
                Some(r) => lt < time.runs[r].1,
                None => true,
            };
            let text = match run {
                Some(r) => {
                    let shown = ((lt - time.runs[r].0) * scene.rate) as usize;
                    scene.results[r].chars().take(shown).collect::<String>()
                }
                None => String::new(),
            };
            let was_generating = popup.get_generating();
            popup.set_generating(generating);
            popup.set_result(text.into());
            if generating {
                popup.invoke_scroll_to_end();
            }
            let editable = popup.get_editable();
            popup.set_hints(if editable && !generating {
                hints(&[("⏎", "Copy"), ("⇧⏎", "New line"), ("esc", "Close")])
            } else {
                hints(&[("esc", "Close")])
            });
            if was_generating && !generating {
                popup.invoke_focus_after_ready();
            }
            popup.set_copied(time.copy_at.is_some_and(|c| lt >= c));
        } else if opened == Some(index) && lt >= time.close_at {
            popup.set_shown(false);
        } else if lt < CLICK {
            popup.set_shown(false);
        }
        let popup_visible = lt >= CLICK && lt < time.close_at + 0.3;

        // Cursor path
        let in_popup = |p: (f32, f32)| (popup_at.0 + p.0, popup_at.1 + p.1);
        let first_ready = time.runs[0].1;
        let ready = time.runs.last().unwrap().1;
        let copy_point = (COPY_X, if above { COPY_Y - 10.0 } else { COPY_Y });
        cursor = if lt < REACH {
            lerp(scene_from, (text_start.0 - 1.0, text_start.1), ease(progress(lt, 0.0, REACH)))
        } else if lt < DRAG.0 {
            text_start
        } else if lt < DRAG.1 {
            (tx + tw * sel, release.1)
        } else if lt < TO_BUTTON.0 {
            release
        } else if lt < TO_BUTTON.1 {
            lerp(release, button_point, ease(progress(lt, TO_BUTTON.0, TO_BUTTON.1)))
        } else if lt < first_ready + 0.3 {
            button_point
        } else if let Some(chip) = scene.chip.filter(|_| lt < time.runs[1].0 + 0.3) {
            lerp(button_point, in_popup(chip), ease(progress(lt, first_ready + 0.3, time.runs[1].0 - 0.2)))
        } else if let Some(copy_at) = time.copy_at {
            let from = scene.chip.map(in_popup).unwrap_or(button_point);
            lerp(from, in_popup(copy_point), ease(progress(lt, ready + 0.3, copy_at - 0.15)))
        } else {
            lerp(
                button_point,
                (button_point.0 + 60.0, button_point.1 - 40.0),
                ease(progress(lt, ready + 0.3, ready + 0.9)),
            )
        };
        let ibeam_cursor = lt >= REACH - 0.25 && lt < DRAG.1 + 0.15;

        // Hover follows the pointer into the bar and the popup.
        let local = |at: (f32, f32), size: (f32, f32)| {
            let p = (cursor.0 - at.0, cursor.1 - at.1);
            (p.0 >= 0.0 && p.1 >= 0.0 && p.0 < size.0 && p.1 < size.1).then_some(p)
        };
        bar_surface.hover(if bar_visible { local(bar_at, (132.0, 40.0)) } else { None });
        popup_surface.hover(if popup_visible { local(popup_at, (536.0, 446.0)) } else { None });

        slint::platform::update_timers_and_animations();
        if still.is_some() && frame + 1 < range_end(still) {
            continue;
        }
        let mut canvas: Vec<u8> = desktop_surface.draw().iter().flat_map(|p| [p.red, p.green, p.blue]).collect();
        if bar_visible || lt < CLICK + 0.15 && lt >= DRAG.1 {
            blend(&mut canvas, &bar_surface.draw(), 132, 40, (bar_at.0 as i32, bar_at.1 as i32));
        }
        if popup_visible {
            blend(&mut canvas, &popup_surface.draw(), 536, 446, (popup_at.0 as i32, popup_at.1 as i32));
        }
        let (layer, hot) = if ibeam_cursor { (&ibeam_layer, (12.0, 12.0)) } else { (&arrow_layer, (2.0, 1.5)) };
        blend(&mut canvas, layer, 24, 24, ((cursor.0 - hot.0) as i32, (cursor.1 - hot.1) as i32));

        match frames.last_mut() {
            Some((last, delay)) if *last == canvas => *delay += 1.0 / FPS,
            _ => frames.push((canvas, 1.0 / FPS)),
        }
    }

    if still.is_some() {
        let (canvas, _) = frames.pop().unwrap();
        let file = std::fs::File::create(&out).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&canvas).unwrap();
        println!("{}", out.display());
        return;
    }
    write_gif(&out, &frames);
    let size = std::fs::metadata(&out).unwrap().len();
    println!("{}: {} frames, {:.2} MB, {:.1}s", out.display(), frames.len(), size as f64 / 1e6, total);
}

fn range_end(still: Option<f32>) -> usize {
    still.map(|t| (t * FPS) as usize + 1).unwrap_or(usize::MAX)
}

fn desktop_text(_desktop: &Desktop, msg: usize) -> SharedString {
    [
        "Can you review my PR this afternoon? It touches the migration step.",
        "The deploy failed because of a race condition in the migration step.",
        "Mình đã deploy bản fix lên staging, bạn test lại rồi merge PR giúp mình nhé",
    ][msg]
        .into()
}

/// One palette for every frame keeps still areas from flickering. Index 255 marks pixels
/// unchanged since the previous frame, which compresses well.
fn write_gif(out: &PathBuf, frames: &[(Vec<u8>, f32)]) {
    const KEEP: u8 = 255;
    let mut sample = Vec::new();
    for (canvas, _) in frames.iter().step_by(4) {
        for px in canvas.chunks(3).step_by(7) {
            sample.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    let quant = color_quant::NeuQuant::new(10, 255, &sample);
    let mut palette = quant.color_map_rgb();
    palette.extend_from_slice(&[255, 0, 255]);

    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let file = std::fs::File::create(out).unwrap();
    let mut encoder = gif::Encoder::new(std::io::BufWriter::new(file), WIDTH as u16, HEIGHT as u16, &palette).unwrap();
    encoder.set_repeat(gif::Repeat::Infinite).unwrap();
    let mut cache: HashMap<[u8; 3], u8> = HashMap::new();
    let mut previous: Option<Vec<u8>> = None;
    for (canvas, delay) in frames {
        let indexed: Vec<u8> = canvas
            .chunks(3)
            .map(|px| {
                let key = [px[0], px[1], px[2]];
                *cache.entry(key).or_insert_with(|| quant.index_of(&[px[0], px[1], px[2], 255]) as u8)
            })
            .collect();
        let buffer: Vec<u8> = match &previous {
            Some(prev) => {
                indexed.iter().zip(prev).map(|(now, before)| if now == before { KEEP } else { *now }).collect()
            }
            None => indexed.clone(),
        };
        let frame = gif::Frame {
            width: WIDTH as u16,
            height: HEIGHT as u16,
            buffer: buffer.into(),
            delay: (delay * 100.0).round() as u16,
            dispose: gif::DisposalMethod::Keep,
            transparent: previous.is_some().then_some(KEEP),
            ..Default::default()
        };
        encoder.write_frame(&frame).unwrap();
        previous = Some(indexed);
    }
}
