use std::thread;
use std::time::{Duration, Instant};

pub trait ClipboardPort {
    type Snapshot;
    fn save(&self) -> Self::Snapshot;
    fn restore(&self, snapshot: Self::Snapshot);
    fn write_text(&self, text: &str);
    fn read_text(&self) -> String;
}

#[derive(Debug, PartialEq)]
pub enum Capture {
    Text(String),
    NoSelection,
    NoText,
}

pub struct CaptureOptions {
    pub timeout: Duration,
    pub poll: Duration,
    /// Sends copy again at this interval while the marker stays. Off by default.
    pub resend: Option<Duration>,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self { timeout: Duration::from_millis(500), poll: Duration::from_millis(10), resend: None }
    }
}

/// Reads the selection by triggering a copy. A unique marker shows whether the copy
/// replaced the clipboard's content. The clipboard is restored afterwards.
pub fn capture_selection<C: ClipboardPort>(
    clipboard: &C,
    send_copy: &mut dyn FnMut() -> Result<(), String>,
    marker: &str,
    options: &CaptureOptions,
) -> Result<Capture, String> {
    let snapshot = clipboard.save();
    let result = (|| {
        clipboard.write_text(marker);
        send_copy()?;
        let deadline = Instant::now() + options.timeout;
        let mut next_send = options.resend.map(|r| Instant::now() + r);
        let mut text = clipboard.read_text();
        // Some apps clear the clipboard before writing, so empty text may be temporary.
        while (text == marker || text.is_empty()) && Instant::now() < deadline {
            thread::sleep(options.poll);
            text = clipboard.read_text();
            if let (Some(resend), Some(at)) = (options.resend, next_send) {
                if text == marker && Instant::now() >= at {
                    send_copy()?;
                    next_send = Some(Instant::now() + resend);
                }
            }
        }
        Ok(if text == marker {
            Capture::NoSelection
        } else if text.is_empty() {
            Capture::NoText
        } else if text.trim().is_empty() {
            Capture::NoSelection
        } else {
            Capture::Text(text)
        })
    })();
    clipboard.restore(snapshot);
    result
}

/// A marker no app would put on the clipboard.
pub fn new_marker() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("fluent-{}-{nanos}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    const MARKER: &str = "marker-123";

    #[derive(Clone)]
    struct Fake(Arc<Mutex<String>>);

    impl Fake {
        fn new(text: &str) -> Self {
            Self(Arc::new(Mutex::new(text.into())))
        }
        fn set(&self, text: &str) {
            *self.0.lock().unwrap() = text.into();
        }
        fn get(&self) -> String {
            self.0.lock().unwrap().clone()
        }
        fn set_later(&self, text: &'static str, ms: u64) {
            let fake = self.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(ms));
                fake.set(text);
            });
        }
    }

    impl ClipboardPort for Fake {
        type Snapshot = String;
        fn save(&self) -> String {
            self.get()
        }
        fn restore(&self, snapshot: String) {
            self.set(&snapshot);
        }
        fn write_text(&self, text: &str) {
            self.set(text);
        }
        fn read_text(&self) -> String {
            self.get()
        }
    }

    fn options() -> CaptureOptions {
        CaptureOptions { timeout: Duration::from_millis(80), poll: Duration::from_millis(2), resend: None }
    }

    fn run(fake: &Fake, options: CaptureOptions, mut copy: impl FnMut(&Fake)) -> Result<Capture, String> {
        let clipboard = fake.clone();
        capture_selection(
            fake,
            &mut || {
                copy(&clipboard);
                Ok(())
            },
            MARKER,
            &options,
        )
    }

    #[test]
    fn returns_copied_text_and_restores() {
        let fake = Fake::new("old");
        assert_eq!(run(&fake, options(), |f| f.set("selected")), Ok(Capture::Text("selected".into())));
        assert_eq!(fake.get(), "old");
    }

    #[test]
    fn writes_the_marker_before_sending_copy() {
        let fake = Fake::new("old");
        let mut seen = String::new();
        run(&fake, options(), |f| seen = f.get()).unwrap();
        assert_eq!(seen, MARKER);
    }

    #[test]
    fn waits_when_an_app_clears_the_clipboard_first() {
        let fake = Fake::new("old");
        let result = run(&fake, options(), |f| {
            f.set("");
            f.set_later("selected", 15);
        });
        assert_eq!(result, Ok(Capture::Text("selected".into())));
    }

    #[test]
    fn sees_a_late_copy() {
        let fake = Fake::new("old");
        assert_eq!(run(&fake, options(), |f| f.set_later("late", 15)), Ok(Capture::Text("late".into())));
    }

    #[test]
    fn sends_copy_again_when_an_app_ignored_the_first() {
        let fake = Fake::new("old");
        let mut sends = 0;
        let opts = CaptureOptions { resend: Some(Duration::from_millis(10)), ..options() };
        let result = run(&fake, opts, |f| {
            sends += 1;
            if sends == 2 {
                f.set("selected");
            }
        });
        assert_eq!(result, Ok(Capture::Text("selected".into())));
        assert_eq!(sends, 2);
    }

    #[test]
    fn does_not_resend_while_an_app_is_writing() {
        let fake = Fake::new("old");
        let mut sends = 0;
        let opts = CaptureOptions { resend: Some(Duration::from_millis(5)), ..options() };
        run(&fake, opts, |f| {
            sends += 1;
            f.set("");
            f.set_later("selected", 30);
        })
        .unwrap();
        assert_eq!(sends, 1);
    }

    #[test]
    fn sends_copy_once_by_default() {
        let fake = Fake::new("old");
        let mut sends = 0;
        run(&fake, options(), |_| sends += 1).unwrap();
        assert_eq!(sends, 1);
    }

    #[test]
    fn reports_no_selection_when_the_marker_stays() {
        let fake = Fake::new("old");
        assert_eq!(run(&fake, options(), |_| {}), Ok(Capture::NoSelection));
        assert_eq!(fake.get(), "old");
    }

    #[test]
    fn treats_whitespace_as_no_selection() {
        let fake = Fake::new("old");
        assert_eq!(run(&fake, options(), |f| f.set(" \n ")), Ok(Capture::NoSelection));
    }

    #[test]
    fn reports_no_text_for_non_text_content() {
        let fake = Fake::new("old");
        assert_eq!(run(&fake, options(), |f| f.set("")), Ok(Capture::NoText));
        assert_eq!(fake.get(), "old");
    }

    #[test]
    fn restores_when_sending_copy_fails() {
        let fake = Fake::new("old");
        let result = capture_selection(&fake, &mut || Err("hook failed".into()), MARKER, &options());
        assert_eq!(result, Err("hook failed".into()));
        assert_eq!(fake.get(), "old");
    }

    #[test]
    fn markers_are_unique() {
        assert_ne!(new_marker(), new_marker());
    }
}
