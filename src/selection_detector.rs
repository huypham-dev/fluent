use crate::geometry::Point;

pub const LEFT_BUTTON: u8 = 1;
const DEFAULT_DRAG_THRESHOLD: f64 = 10.0;

#[derive(Clone, Copy, Debug)]
pub struct MouseInfo {
    pub point: Point,
    /// 1 is left, 2 is right, 3 is middle.
    pub button: u8,
    pub clicks: u32,
}

/// Decides from mouse events alone whether the user probably selected text.
/// It never reads the selection, so a wrong guess costs only a brief icon bar.
pub struct SelectionDetector {
    threshold: f64,
    press: Option<Point>,
}

impl Default for SelectionDetector {
    fn default() -> Self {
        Self { threshold: DEFAULT_DRAG_THRESHOLD, press: None }
    }
}

impl SelectionDetector {
    pub fn mouse_down(&mut self, event: MouseInfo) {
        self.press = (event.button == LEFT_BUTTON).then_some(event.point);
    }

    /// `own_window` says whether the release is over one of this app's windows.
    pub fn mouse_up(&mut self, event: MouseInfo, own_window: bool) -> bool {
        let Some(start) = self.press.take() else { return false };
        if event.button != LEFT_BUTTON || own_window {
            return false;
        }
        let dragged = (event.point.x - start.x).hypot(event.point.y - start.y) > self.threshold;
        dragged || event.clicks >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(x: f64, y: f64, button: u8, clicks: u32) -> MouseInfo {
        MouseInfo { point: Point { x, y }, button, clicks }
    }

    fn drag(from: MouseInfo, to: MouseInfo) -> bool {
        let mut detector = SelectionDetector::default();
        detector.mouse_down(from);
        detector.mouse_up(to, false)
    }

    #[test]
    fn shows_after_a_left_drag_over_10px() {
        assert!(drag(ev(0.0, 0.0, 1, 1), ev(11.0, 0.0, 1, 1)));
    }

    #[test]
    fn ignores_a_drag_of_exactly_10px() {
        assert!(!drag(ev(0.0, 0.0, 1, 1), ev(10.0, 0.0, 1, 1)));
    }

    #[test]
    fn measures_diagonal_drags_by_distance() {
        assert!(drag(ev(0.0, 0.0, 1, 1), ev(8.0, 8.0, 1, 1)));
    }

    #[test]
    fn shows_after_a_double_click() {
        assert!(drag(ev(5.0, 5.0, 1, 2), ev(5.0, 5.0, 1, 2)));
    }

    #[test]
    fn ignores_a_single_click() {
        assert!(!drag(ev(5.0, 5.0, 1, 1), ev(5.0, 5.0, 1, 1)));
    }

    #[test]
    fn ignores_a_right_button_drag() {
        assert!(!drag(ev(0.0, 0.0, 2, 1), ev(100.0, 0.0, 2, 1)));
    }

    #[test]
    fn ignores_a_release_with_no_press() {
        assert!(!SelectionDetector::default().mouse_up(ev(100.0, 0.0, 1, 1), false));
    }

    #[test]
    fn does_not_reuse_a_finished_press() {
        let mut detector = SelectionDetector::default();
        detector.mouse_down(ev(0.0, 0.0, 1, 1));
        detector.mouse_up(ev(100.0, 0.0, 1, 1), false);
        assert!(!detector.mouse_up(ev(200.0, 0.0, 1, 1), false));
    }

    #[test]
    fn ignores_a_drag_ending_over_an_own_window() {
        let mut detector = SelectionDetector::default();
        detector.mouse_down(ev(0.0, 0.0, 1, 1));
        assert!(!detector.mouse_up(ev(100.0, 0.0, 1, 1), true));
    }
}
