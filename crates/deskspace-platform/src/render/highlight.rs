//! The fade of the settings window's highlight outline (ROADMAP D26): the
//! level the painter draws it at, eased between off and on.

use std::time::Duration;

use crate::render::math::{lerp, smoothstep};

/// How long a full fade in takes.
pub(crate) const HIGHLIGHT_FADE_IN: Duration = Duration::from_millis(150);
/// How long a full fade out takes.
pub(crate) const HIGHLIGHT_FADE_OUT: Duration = Duration::from_millis(250);

/// The highlight outline's level, 0 at rest off and 1 at rest on, eased
/// between the two. A fade's length is its full duration times the distance
/// it travels, so a fade turned around half-way starts with no jump and ends
/// in proportion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HighlightFade {
    from: f32,
    to: f32,
    start: Duration,
}

impl HighlightFade {
    /// The level held at `on`, with no fade running.
    pub(crate) fn at_rest(on: bool) -> Self {
        let level = if on { 1.0 } else { 0.0 };
        Self {
            from: level,
            to: level,
            start: Duration::ZERO,
        }
    }

    /// The level at `now`.
    pub(crate) fn level(&self, now: Duration) -> f32 {
        let length = self.length();
        if length.is_zero() {
            return self.to;
        }
        let traveled = now.saturating_sub(self.start).min(length);
        let t = traveled.as_secs_f32() / length.as_secs_f32();
        lerp(self.from, self.to, smoothstep(t))
    }

    /// Starts fading toward `on` from the level at `now`.
    pub(crate) fn toward(&mut self, on: bool, now: Duration) {
        self.from = self.level(now);
        self.to = if on { 1.0 } else { 0.0 };
        self.start = now;
    }

    /// Switches to `on` at once, with no fade.
    pub(crate) fn jump(&mut self, on: bool) {
        *self = Self::at_rest(on);
    }

    /// Whether the fade has reached its end at `now`.
    pub(crate) fn is_done(&self, now: Duration) -> bool {
        now.saturating_sub(self.start) >= self.length()
    }

    /// How long this fade takes: the full duration of its direction times how
    /// far it travels.
    fn length(&self) -> Duration {
        let full = if self.to > self.from {
            HIGHLIGHT_FADE_IN
        } else {
            HIGHLIGHT_FADE_OUT
        };
        full.mul_f32((self.to - self.from).abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn at_rest_it_stays_off_or_on() {
        let off = HighlightFade::at_rest(false);
        let on = HighlightFade::at_rest(true);
        for now in [Duration::ZERO, ms(10), ms(150), ms(1000)] {
            assert_eq!(off.level(now), 0.0);
            assert_eq!(on.level(now), 1.0);
            assert!(off.is_done(now) && on.is_done(now));
        }
    }

    #[test]
    fn a_full_fade_reaches_its_end_at_its_duration() {
        let mut fade = HighlightFade::at_rest(false);
        fade.toward(true, ms(100));
        assert_eq!(fade.level(ms(100)), 0.0);
        assert_eq!(fade.level(ms(250)), 1.0, "150 ms after it started");
        assert!(fade.is_done(ms(250)));
        assert!(!fade.is_done(ms(249)));

        let mut out = HighlightFade::at_rest(true);
        out.toward(false, Duration::ZERO);
        assert!(out.level(ms(249)) > 0.0);
        assert_eq!(out.level(ms(250)), 0.0, "250 ms to fade out");
        assert!(out.is_done(ms(250)));
    }

    #[test]
    fn the_level_only_moves_the_way_the_fade_goes() {
        let mut fade = HighlightFade::at_rest(false);
        fade.toward(true, Duration::ZERO);
        let mut previous = 0.0;
        for step in 0..=20 {
            let level = fade.level(ms(step * 5));
            assert!(level >= previous, "rose at {step}");
            assert!((0.0..=1.0).contains(&level));
            previous = level;
        }
        let mut out = HighlightFade::at_rest(true);
        out.toward(false, Duration::ZERO);
        let mut previous = 1.0;
        for step in 0..=20 {
            let level = out.level(ms(step * 5));
            assert!(level <= previous, "fell at {step}");
            assert!((0.0..=1.0).contains(&level));
            previous = level;
        }
    }

    #[test]
    fn a_fade_turned_around_starts_where_it_was_and_ends_early() {
        let mut fade = HighlightFade::at_rest(false);
        fade.toward(true, Duration::ZERO);
        let half = fade.level(ms(75));
        assert!(half > 0.4 && half < 0.6, "{half}");
        fade.toward(false, ms(75));
        assert_eq!(fade.level(ms(75)), half, "no jump on the turn");
        // A quarter of a full fade in is distance 0.5, and a fade out of that
        // distance takes half of 250 ms.
        assert!(fade.level(ms(150)) > 0.0);
        assert_eq!(fade.level(ms(75) + ms(125)), 0.0);
        assert!(fade.is_done(ms(75) + ms(125)));
    }
}
