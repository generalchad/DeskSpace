//! How far a settings page taller than its window is scrolled, and where each
//! scroll input takes it. Pure: the settings window turns the offsets into
//! `ScrollWindowEx` calls and scroll-bar positions.

/// What moved the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollInput {
    /// A scroll-bar arrow: one line up or down.
    LineUp,
    LineDown,
    /// PageUp or PageDown, or a click in the scroll bar's track.
    PageUp,
    PageDown,
    /// Home or End, or the scroll bar's own Top and Bottom.
    Top,
    Bottom,
    /// The thumb dragged to this offset.
    To(i32),
    /// Mouse-wheel travel, in `WHEEL_DELTA` units (120 a notch; up is
    /// positive).
    Wheel(i32),
}

/// One notch of the wheel (`WHEEL_DELTA`, WinUser.h).
const WHEEL_DELTA: i32 = 120;
/// The lines one notch scrolls, Windows' default for "lines to scroll".
const WHEEL_LINES: i32 = 3;

/// One page's content height, the height of the area showing it, and how far
/// it is scrolled, all in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PageScroll {
    content: i32,
    view: i32,
    offset: i32,
}

impl PageScroll {
    /// A page at its top.
    pub(crate) fn new(content: i32, view: i32) -> Self {
        Self {
            content: content.max(0),
            view: view.max(0),
            offset: 0,
        }
    }

    pub(crate) fn content(self) -> i32 {
        self.content
    }

    pub(crate) fn view(self) -> i32 {
        self.view
    }

    pub(crate) fn offset(self) -> i32 {
        self.offset
    }

    /// Whether the content is taller than the view, so the page scrolls and
    /// shows a scroll bar.
    pub(crate) fn overflows(self) -> bool {
        self.content > self.view
    }

    fn max_offset(self) -> i32 {
        (self.content - self.view).max(0)
    }

    /// Moves for `input`, `line` being one arrow's step, and returns how far
    /// the content moved up (negative when it moved down). Clamped, so the
    /// page never scrolls past its top or bottom.
    pub(crate) fn apply(&mut self, input: ScrollInput, line: i32) -> i32 {
        // A page step keeps one line of the old view in sight.
        let page = (self.view - line).max(line);
        let to = match input {
            ScrollInput::LineUp => self.offset - line,
            ScrollInput::LineDown => self.offset + line,
            ScrollInput::PageUp => self.offset - page,
            ScrollInput::PageDown => self.offset + page,
            ScrollInput::Top => 0,
            ScrollInput::Bottom => self.max_offset(),
            ScrollInput::To(to) => to,
            ScrollInput::Wheel(delta) => self.offset - delta * line * WHEEL_LINES / WHEEL_DELTA,
        };
        let to = to.clamp(0, self.max_offset());
        let moved = to - self.offset;
        self.offset = to;
        moved
    }
}

/// The height of the area that shows a page: the tallest page's, unless that
/// leaves no room on the screen, in which case what the screen leaves, but
/// never under `minimum`.
pub(crate) fn view_height(tallest: i32, available: i32, minimum: i32) -> i32 {
    tallest.min(available.max(minimum))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: i32 = 40;

    #[test]
    fn a_page_that_fits_does_not_scroll() {
        let mut scroll = PageScroll::new(300, 400);
        assert!(!scroll.overflows());
        for input in [
            ScrollInput::LineDown,
            ScrollInput::PageDown,
            ScrollInput::Bottom,
            ScrollInput::Wheel(-WHEEL_DELTA),
            ScrollInput::To(50),
        ] {
            assert_eq!(scroll.apply(input, LINE), 0, "{input:?}");
            assert_eq!(scroll.offset(), 0);
        }
    }

    #[test]
    fn scrolling_stops_at_the_top_and_the_bottom() {
        let mut scroll = PageScroll::new(1000, 400);
        assert!(scroll.overflows());
        assert_eq!(scroll.apply(ScrollInput::LineUp, LINE), 0);
        assert_eq!(scroll.apply(ScrollInput::Bottom, LINE), 600);
        assert_eq!(scroll.offset(), 600);
        assert_eq!(scroll.apply(ScrollInput::LineDown, LINE), 0);
        assert_eq!(scroll.apply(ScrollInput::To(9999), LINE), 0);
        assert_eq!(scroll.apply(ScrollInput::Top, LINE), -600);
        assert_eq!(scroll.apply(ScrollInput::To(-5), LINE), 0);
    }

    #[test]
    fn a_page_step_keeps_a_line_of_the_old_view() {
        let mut scroll = PageScroll::new(2000, 400);
        assert_eq!(scroll.apply(ScrollInput::PageDown, LINE), 360);
        assert_eq!(scroll.apply(ScrollInput::PageUp, LINE), -360);
    }

    #[test]
    fn the_wheel_moves_three_lines_a_notch_and_up_is_back() {
        let mut scroll = PageScroll::new(2000, 400);
        assert_eq!(
            scroll.apply(ScrollInput::Wheel(-2 * WHEEL_DELTA), LINE),
            240
        );
        assert_eq!(scroll.apply(ScrollInput::Wheel(WHEEL_DELTA), LINE), -120);
        // A precision touchpad sends parts of a notch.
        assert_eq!(scroll.apply(ScrollInput::Wheel(-40), LINE), 40);
    }

    #[test]
    fn the_view_is_the_tallest_page_unless_the_screen_is_shorter() {
        assert_eq!(view_height(800, 1000, 200), 800);
        assert_eq!(view_height(800, 600, 200), 600);
        assert_eq!(view_height(800, 100, 200), 200, "never under the minimum");
        assert_eq!(
            view_height(150, 100, 200),
            150,
            "never taller than the page"
        );
    }
}
