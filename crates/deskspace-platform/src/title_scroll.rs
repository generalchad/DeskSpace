//! A long settings header title's scroll: pure frame math.

use std::time::Duration;

use crate::render::math::smoothstep;

/// How fast a title traverses, in 96-DPI pixels per second.
const TITLE_SCROLL_SPEED: f32 = 30.0;
/// How long the title rests at each end before it moves again.
const TITLE_SCROLL_PAUSE: Duration = Duration::from_millis(1500);
/// The shortest traverse, so a one-pixel overflow still moves visibly.
const MIN_TRAVEL: Duration = Duration::from_millis(400);

/// One title's scroll: how far its text overflows its box, and how long one
/// traverse takes. `offset` runs from 0 through the first pause, eases to
/// `overflow`, pauses, and eases back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TitleScroll {
    /// The part of the text that does not fit, in pixels; never below 0.
    pub(crate) overflow: f32,
    /// One traverse's duration.
    pub(crate) travel: Duration,
}

impl TitleScroll {
    /// Measures a title of `text_width` in a box of `box_width` at `scale`
    /// (the display DPI over 96).
    pub(crate) fn new(text_width: f32, box_width: f32, scale: f32) -> Self {
        let overflow = (text_width - box_width).max(0.0);
        let travel = if overflow <= 0.0 {
            Duration::ZERO
        } else {
            let speed = TITLE_SCROLL_SPEED * scale.max(0.01);
            Duration::from_secs_f32(overflow / speed).max(MIN_TRAVEL)
        };
        Self { overflow, travel }
    }

    /// The title's x offset at `elapsed` since the scroll started.
    pub(crate) fn offset(&self, elapsed: Duration) -> f32 {
        if self.overflow <= 0.0 {
            return 0.0;
        }
        let t = self.phase_time(elapsed);
        let pause = TITLE_SCROLL_PAUSE.as_secs_f64();
        let travel = self.travel.as_secs_f64();
        if t < pause {
            0.0
        } else if t < pause + travel {
            self.overflow * smoothstep(((t - pause) / travel) as f32)
        } else if t < 2.0 * pause + travel {
            self.overflow
        } else {
            let progress = (t - (2.0 * pause + travel)) / travel;
            self.overflow * smoothstep((1.0 - progress) as f32)
        }
    }

    /// Whether a traverse is running at `elapsed`; false through either pause.
    pub(crate) fn is_moving(&self, elapsed: Duration) -> bool {
        if self.overflow <= 0.0 {
            return false;
        }
        let t = self.phase_time(elapsed);
        let pause = TITLE_SCROLL_PAUSE.as_secs_f64();
        let travel = self.travel.as_secs_f64();
        (t >= pause && t < pause + travel)
            || (t >= 2.0 * pause + travel && t < 2.0 * (pause + travel))
    }

    /// The time left until the next traverse, or `None` while one is running.
    /// A caller uses this to set a timer during a pause instead of taking
    /// frames it does not need.
    pub(crate) fn next_move(&self, elapsed: Duration) -> Option<Duration> {
        if self.overflow <= 0.0 {
            return None;
        }
        let t = self.phase_time(elapsed);
        let pause = TITLE_SCROLL_PAUSE.as_secs_f64();
        let travel = self.travel.as_secs_f64();
        if t < pause {
            Some(Duration::from_secs_f64(pause - t))
        } else if t < pause + travel {
            None
        } else if t < 2.0 * pause + travel {
            Some(Duration::from_secs_f64(2.0 * pause + travel - t))
        } else {
            None
        }
    }

    /// `elapsed` within one full start-pause, traverse, end-pause, and return.
    fn phase_time(&self, elapsed: Duration) -> f64 {
        let period = 2.0 * (TITLE_SCROLL_PAUSE.as_secs_f64() + self.travel.as_secs_f64());
        elapsed.as_secs_f64() % period
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scroll(overflow: f32, scale: f32) -> TitleScroll {
        TitleScroll::new(100.0 + overflow, 100.0, scale)
    }

    #[test]
    fn no_overflow_means_no_motion() {
        let scroll = scroll(0.0, 1.0);
        for elapsed in [
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::from_secs(10),
        ] {
            assert_eq!(scroll.offset(elapsed), 0.0);
            assert!(!scroll.is_moving(elapsed));
            assert_eq!(scroll.next_move(elapsed), None);
        }
    }

    #[test]
    fn the_offset_pauses_traverses_and_comes_back() {
        let scroll = scroll(200.0, 1.0);
        let pause = TITLE_SCROLL_PAUSE;
        let travel = scroll.travel;
        let period = 2 * (pause + travel);

        assert_eq!(scroll.offset(Duration::ZERO), 0.0);
        assert_eq!(scroll.offset(pause / 2), 0.0);
        assert!(scroll.is_moving(pause + travel / 2));
        assert!((scroll.offset(pause + travel) - scroll.overflow).abs() < 0.001);
        assert!(!scroll.is_moving(pause + travel));
        assert_eq!(
            scroll.offset(pause + travel + pause / 2),
            scroll.overflow,
            "the end pause holds the overflow"
        );
        assert!(scroll.is_moving(2 * pause + travel + travel / 2));
        assert!((scroll.offset(period) - 0.0).abs() < 0.001);
    }

    #[test]
    fn the_offset_never_leaves_its_range() {
        let scroll = scroll(300.0, 1.0);
        let period = 2 * (TITLE_SCROLL_PAUSE + scroll.travel);
        let mut elapsed = Duration::ZERO;
        let step = Duration::from_millis(10);
        while elapsed < period {
            let offset = scroll.offset(elapsed);
            assert!(
                (0.0..=scroll.overflow).contains(&offset),
                "{offset} at {elapsed:?}"
            );
            elapsed += step;
        }
    }

    #[test]
    fn the_travel_scales_with_dpi_and_has_a_floor() {
        let slow = scroll(300.0, 1.0);
        let fast = scroll(300.0, 2.0);
        assert!(
            fast.travel < slow.travel,
            "a denser display moves the same pixels faster"
        );
        assert_eq!(slow.travel, Duration::from_secs(10));
        assert_eq!(fast.travel, Duration::from_secs(5));
        assert_eq!(scroll(1.0, 1.0).travel, MIN_TRAVEL);
    }

    #[test]
    fn next_move_counts_down_through_each_pause() {
        let scroll = scroll(200.0, 1.0);
        let pause = TITLE_SCROLL_PAUSE;
        let travel = scroll.travel;
        assert_eq!(scroll.next_move(Duration::ZERO), Some(pause));
        assert_eq!(scroll.next_move(pause / 2), Some(pause / 2));
        assert_eq!(scroll.next_move(pause + travel), Some(pause));
        assert_eq!(
            scroll.next_move(pause + travel + pause / 2),
            Some(pause / 2)
        );
        assert_eq!(scroll.next_move(pause + travel / 2), None);
    }
}
