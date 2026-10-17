//! The zoom pop-up (ROADMAP F-57): what it shows, and where its buttons are.
//! Pure, so the layout and hit test are unit-tested; `win32/zoom_bar.rs`
//! positions and paints it, and the app fills it in.

use deskspace_core::geometry::{Point, Rect};
use deskspace_core::theme::Color;

use super::SpaceStyle;
use crate::input::ZoomAction;

/// What the zoom pop-up shows: the icon size and which buttons apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomBar {
    /// The icon size as a percentage of the 100 % base.
    pub percent: u32,
    pub can_zoom_out: bool,
    pub can_zoom_in: bool,
    /// Whether Reset would change anything (the size is not 100 %).
    pub can_reset: bool,
    /// Material, corners, border, and dark mode, as for a space.
    pub style: SpaceStyle,
    pub tint: Color,
    pub text: Color,
    /// The fill under a hovered button.
    pub hover: Color,
    pub border: Color,
}

impl ZoomBar {
    /// Whether `action`'s button does anything; a disabled one is drawn
    /// dimmed and ignores clicks.
    pub fn enabled(&self, action: ZoomAction) -> bool {
        match action {
            ZoomAction::Out => self.can_zoom_out,
            ZoomAction::In => self.can_zoom_in,
            ZoomAction::Reset => self.can_reset,
        }
    }

    /// The percentage as drawn, for example "110 %".
    pub fn label(&self) -> String {
        format!("{} %", self.percent)
    }
}

// The layout in 96-DPI pixels; the window scales it for its monitor.
const PADDING: i32 = 4;
const BUTTON_WIDTH: i32 = 36;
const PERCENT_WIDTH: i32 = 64;
/// Space on each side of the divider between the zoom buttons and Reset.
const DIVIDER_GAP: i32 = 6;
const RESET_WIDTH: i32 = 64;
const HEIGHT: i32 = 40;
/// How far below the top of the work area the pop-up sits.
pub(crate) const ZOOM_BAR_MARGIN: i32 = 12;

/// Where each part of the pop-up is, in 96-DPI pixels from its top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ZoomBarLayout {
    pub zoom_out: Rect,
    pub percent: Rect,
    pub zoom_in: Rect,
    /// The x of the divider between the zoom buttons and Reset.
    pub divider_x: i32,
    pub reset: Rect,
    pub width: i32,
    pub height: i32,
}

impl ZoomBarLayout {
    /// Lays the parts out left to right: zoom out, the percentage, zoom in,
    /// a divider, and Reset.
    pub(crate) fn new() -> Self {
        let button_height = HEIGHT - 2 * PADDING;
        let zoom_out = Rect::new(PADDING, PADDING, BUTTON_WIDTH, button_height);
        let percent = Rect::new(zoom_out.right(), PADDING, PERCENT_WIDTH, button_height);
        let zoom_in = Rect::new(percent.right(), PADDING, BUTTON_WIDTH, button_height);
        let divider_x = zoom_in.right() + DIVIDER_GAP;
        let reset = Rect::new(divider_x + DIVIDER_GAP, PADDING, RESET_WIDTH, button_height);
        Self {
            zoom_out,
            percent,
            zoom_in,
            divider_x,
            reset,
            width: reset.right() + PADDING,
            height: HEIGHT,
        }
    }

    pub(crate) fn button(&self, action: ZoomAction) -> Rect {
        match action {
            ZoomAction::Out => self.zoom_out,
            ZoomAction::In => self.zoom_in,
            ZoomAction::Reset => self.reset,
        }
    }

    /// The button under `point` (96-DPI pixels from the top-left), if any.
    pub(crate) fn hit(&self, point: Point) -> Option<ZoomAction> {
        [ZoomAction::Out, ZoomAction::In, ZoomAction::Reset]
            .into_iter()
            .find(|action| self.button(*action).contains(point))
    }
}

/// The pop-up's top-left corner: centered horizontally at the top of
/// `work_area`, `width` physical pixels wide, `margin` pixels down.
pub(crate) fn zoom_bar_origin(work_area: Rect, width: i32, margin: i32) -> Point {
    Point::new(
        work_area.x + (work_area.width - width) / 2,
        work_area.y + margin,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_sit_in_one_row_without_overlapping() {
        let layout = ZoomBarLayout::new();
        assert_eq!(layout.percent.x, layout.zoom_out.right());
        assert_eq!(layout.zoom_in.x, layout.percent.right());
        assert!(layout.divider_x > layout.zoom_in.right());
        assert!(layout.reset.x > layout.divider_x);
        assert_eq!(layout.width, layout.reset.right() + PADDING);
        for rect in [
            layout.zoom_out,
            layout.percent,
            layout.zoom_in,
            layout.reset,
        ] {
            assert_eq!(rect.y, PADDING);
            assert_eq!(rect.bottom(), layout.height - PADDING);
        }
    }

    #[test]
    fn a_point_hits_the_button_under_it() {
        let layout = ZoomBarLayout::new();
        for action in [ZoomAction::Out, ZoomAction::In, ZoomAction::Reset] {
            assert_eq!(layout.hit(layout.button(action).center()), Some(action));
        }
        assert_eq!(layout.hit(layout.percent.center()), None);
        assert_eq!(layout.hit(Point::new(layout.divider_x, 20)), None);
        assert_eq!(layout.hit(Point::new(0, 0)), None);
    }

    #[test]
    fn the_bar_is_centered_at_the_top_of_the_work_area() {
        let work_area = Rect::new(2560, -673, 1080, 1880);
        assert_eq!(
            zoom_bar_origin(work_area, 200, 12),
            Point::new(2560 + 440, -673 + 12)
        );
    }
}
