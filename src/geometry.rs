//! Placement of the icon bar and the popup on screen, in logical units.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x && point.x < self.x + self.width && point.y >= self.y && point.y < self.y + self.height
    }
}

pub const BAR_SIZE: Size = Size { width: 132.0, height: 40.0 };
const POINTER_GAP: f64 = 12.0;

/// The visible card, and the transparent room around it for the arrow and the shadow.
pub const POPUP_CARD: Size = Size { width: 500.0, height: 340.0 };
pub const POPUP_MARGIN: f64 = 18.0;
pub const POPUP_ARROW: f64 = 10.0;
pub const POPUP_WINDOW: Size =
    Size { width: POPUP_CARD.width + POPUP_MARGIN * 2.0, height: POPUP_CARD.height + POPUP_MARGIN * 2.0 + POPUP_ARROW };

/// The pointer sits mid-line, so the arrow tip moves this far to clear the selected line.
const LINE_GAP: f64 = 12.0;
const SAME_LINE: f64 = 10.0;
/// Keeps the arrow off the card's rounded corners.
const ARROW_INSET: f64 = POPUP_MARGIN + 24.0;

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

/// Places the bar above the pointer, or below it near the top, inside the work area.
pub fn bar_position(pointer: Point, bar: Size, work_area: Rect) -> Point {
    let above = pointer.y - POINTER_GAP - bar.height;
    let y = if above >= work_area.y { above } else { pointer.y + POINTER_GAP };
    Point {
        x: clamp(pointer.x - bar.width / 2.0, work_area.x, work_area.x + work_area.width - bar.width),
        y: clamp(y, work_area.y, work_area.y + work_area.height - bar.height),
    }
}

/// Where the selection probably is, from the drag that made it.
pub fn selection_anchor(press: Option<Point>, release: Point) -> Point {
    match press {
        Some(press) if (release.y - press.y).abs() <= SAME_LINE => {
            Point { x: ((press.x + release.x) / 2.0).round(), y: release.y }
        }
        _ => release,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupSide {
    Below,
    Above,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupPlacement {
    pub x: f64,
    pub y: f64,
    pub side: PopupSide,
    pub arrow_x: f64,
}

/// Places the popup window under the selection, or above it near the bottom of the work area.
pub fn popup_placement(anchor: Point, work_area: Rect) -> PopupPlacement {
    let bottom = work_area.y + work_area.height;
    let below_y = anchor.y + LINE_GAP - POPUP_MARGIN;
    let above_y = anchor.y - LINE_GAP + POPUP_MARGIN - POPUP_WINDOW.height;
    let fits_below = below_y + POPUP_WINDOW.height <= bottom;
    let more_room_above = anchor.y - work_area.y > bottom - anchor.y;
    let side = if !fits_below && more_room_above { PopupSide::Above } else { PopupSide::Below };
    let y = clamp(if side == PopupSide::Below { below_y } else { above_y }, work_area.y, bottom - POPUP_WINDOW.height);
    let x = clamp(anchor.x - POPUP_WINDOW.width / 2.0, work_area.x, work_area.x + work_area.width - POPUP_WINDOW.width);
    let arrow_x = clamp(anchor.x - x, ARROW_INSET, POPUP_WINDOW.width - ARROW_INSET);
    PopupPlacement { x: x.round(), y: y.round(), side, arrow_x: arrow_x.round() }
}

/// Centers the popup in the work area, for messages with no selection to point at.
pub fn popup_centered(work_area: Rect) -> Point {
    Point {
        x: (work_area.x + (work_area.width - POPUP_WINDOW.width) / 2.0).round(),
        y: (work_area.y + (work_area.height - POPUP_WINDOW.height) / 2.0).round(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    const BAR: Size = Size { width: 120.0, height: 40.0 };
    const AREA: Rect = Rect { x: 0.0, y: 0.0, width: 1000.0, height: 800.0 };

    #[test]
    fn centers_the_bar_12px_above_the_pointer() {
        assert_eq!(bar_position(p(500.0, 400.0), BAR, AREA), p(440.0, 348.0));
    }

    #[test]
    fn flips_the_bar_below_when_no_room_above() {
        assert_eq!(bar_position(p(500.0, 20.0), BAR, AREA), p(440.0, 32.0));
    }

    #[test]
    fn keeps_the_bar_inside_the_work_area_at_every_corner() {
        for pointer in [p(0.0, 0.0), p(1000.0, 0.0), p(0.0, 800.0), p(1000.0, 800.0)] {
            let at = bar_position(pointer, BAR, AREA);
            assert!(at.x >= 0.0 && at.y >= 0.0);
            assert!(at.x + BAR.width <= 1000.0 && at.y + BAR.height <= 800.0);
        }
    }

    #[test]
    fn bar_respects_a_display_with_a_non_zero_origin() {
        let second = Rect { x: -1200.0, y: 100.0, width: 1200.0, height: 700.0 };
        assert_eq!(bar_position(p(-1200.0, 100.0), BAR, second), p(-1200.0, 112.0));
    }

    #[test]
    fn anchor_centers_under_a_one_line_drag() {
        assert_eq!(selection_anchor(Some(p(200.0, 300.0)), p(600.0, 304.0)), p(400.0, 304.0));
    }

    #[test]
    fn anchor_uses_release_for_a_multi_line_drag() {
        assert_eq!(selection_anchor(Some(p(200.0, 300.0)), p(600.0, 380.0)), p(600.0, 380.0));
    }

    #[test]
    fn anchor_uses_release_when_press_is_unknown() {
        assert_eq!(selection_anchor(None, p(600.0, 380.0)), p(600.0, 380.0));
    }

    const WIDE: Rect = Rect { x: 0.0, y: 0.0, width: 1600.0, height: 1000.0 };

    #[test]
    fn popup_opens_below_with_the_arrow_under_the_anchor() {
        let placement = popup_placement(p(800.0, 300.0), WIDE);
        assert_eq!(placement.side, PopupSide::Below);
        assert_eq!(placement.y + POPUP_MARGIN, 312.0);
        assert_eq!(placement.x + placement.arrow_x, 800.0);
    }

    #[test]
    fn popup_opens_above_when_no_room_below() {
        let placement = popup_placement(p(800.0, 900.0), WIDE);
        assert_eq!(placement.side, PopupSide::Above);
        assert_eq!(placement.y + POPUP_WINDOW.height - POPUP_MARGIN, 888.0);
    }

    #[test]
    fn popup_stays_inside_near_an_edge_and_keeps_the_arrow_on_the_card() {
        let placement = popup_placement(p(1590.0, 300.0), WIDE);
        assert!(placement.x + POPUP_WINDOW.width <= 1600.0);
        assert!(placement.arrow_x < POPUP_WINDOW.width - 30.0);
    }

    #[test]
    fn popup_respects_a_display_with_a_non_zero_origin() {
        let second = Rect { x: -1200.0, y: 100.0, width: 1200.0, height: 900.0 };
        let placement = popup_placement(p(-1190.0, 120.0), second);
        assert!(placement.x >= -1200.0);
        assert_eq!(placement.side, PopupSide::Below);
    }
}
