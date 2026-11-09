//! What the app needs from the operating system. Screen units are points on macOS and
//! physical pixels on Windows; `Display` converts them to logical units around a monitor.

use crate::geometry::{Point, Rect};
use crate::selection_detector::MouseInfo;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

pub enum HookEvent {
    MouseDown(MouseInfo),
    MouseUp(MouseInfo),
    /// Any key went down, modifiers included.
    KeyDown,
}

#[derive(Clone, Copy, Debug)]
pub struct Display {
    /// The monitor's top-left corner, the fixed point of the scaling.
    pub origin: Point,
    pub work_area: Rect,
    /// Screen units per logical unit.
    pub scale: f64,
}

impl Display {
    pub fn to_logical(&self, point: Point) -> Point {
        Point {
            x: self.origin.x + (point.x - self.origin.x) / self.scale,
            y: self.origin.y + (point.y - self.origin.y) / self.scale,
        }
    }

    pub fn to_screen(&self, point: Point) -> Point {
        Point {
            x: self.origin.x + (point.x - self.origin.x) * self.scale,
            y: self.origin.y + (point.y - self.origin.y) * self.scale,
        }
    }

    pub fn logical_work_area(&self) -> Rect {
        let top_left = self.to_logical(Point { x: self.work_area.x, y: self.work_area.y });
        Rect {
            x: top_left.x,
            y: top_left.y,
            width: self.work_area.width / self.scale,
            height: self.work_area.height / self.scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_around_the_monitor_origin() {
        let display = Display {
            origin: Point { x: 1920.0, y: 0.0 },
            work_area: Rect { x: 1920.0, y: 0.0, width: 3000.0, height: 1900.0 },
            scale: 1.5,
        };
        let point = Point { x: 2220.0, y: 300.0 };
        assert_eq!(display.to_logical(point), Point { x: 2120.0, y: 200.0 });
        assert_eq!(display.to_screen(display.to_logical(point)), point);
        assert_eq!(display.logical_work_area(), Rect { x: 1920.0, y: 0.0, width: 2000.0, height: 1900.0 / 1.5 });
    }
}
