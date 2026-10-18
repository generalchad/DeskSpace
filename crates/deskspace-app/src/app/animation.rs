//! `App` animations (ROADMAP D12): what each kind of moment plays, from the
//! settings and Windows' "Animation effects" switch. The platform plays it.

use std::time::Duration;

use deskspace_core::animation::{Animations, Kind, StartupOrder};
use deskspace_core::{Rect, SpaceId};
use deskspace_platform::{Play, Way};

use super::App;

/// The longest an exit may take to animate before DeskSpace tears down
/// anyway (ROADMAP D13).
pub(super) const EXIT_LIMIT: Duration = Duration::from_secs(2);

/// Why space windows are appearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Arrival {
    /// A space came into being: a menu, the hotkey, a drop, or undo.
    New,
    /// DeskSpace started: every space appears, in the startup order.
    Startup,
}

impl App {
    /// What `kind` plays going `way` now.
    pub(super) fn play(&self, kind: Kind, way: Way) -> Play {
        play_for(
            &self.config.window.animations,
            self.system.animations(),
            kind,
            way,
        )
    }

    /// What each of `spaces` (with their rects) plays as it appears.
    pub(super) fn arrival_plays(
        &self,
        spaces: &[(SpaceId, Rect)],
        arrival: Arrival,
    ) -> Vec<(SpaceId, Play)> {
        let play = self.play(Kind::Startup, Way::In);
        let animations = &self.config.window.animations;
        let delays = match arrival {
            Arrival::New => spaces.iter().map(|(id, _)| (*id, Duration::ZERO)).collect(),
            Arrival::Startup => staggered(
                reading_order(spaces),
                animations.startup,
                Duration::from_millis(animations.stagger_ms.into()),
            ),
        };
        delays
            .into_iter()
            .map(|(id, delay)| (id, Play { delay, ..play }))
            .collect()
    }
}

impl App {
    /// Exits gracefully (ROADMAP D13): the shown spaces play Startup's Out,
    /// in reverse reading order, then DeskSpace tears down as before.
    pub(super) fn exit(&mut self, ui: &mut deskspace_platform::Ui) {
        let plays = if self.visible {
            let spaces: Vec<(SpaceId, Rect)> = self
                .config
                .desktop
                .spaces()
                .iter()
                .map(|space| (space.id, space.visible_rect()))
                .collect();
            let play = self.play(Kind::Startup, Way::Out);
            let animations = &self.config.window.animations;
            exit_delays(
                &spaces,
                animations.startup,
                Duration::from_millis(animations.stagger_ms.into()),
                play.duration,
            )
            .into_iter()
            .map(|(id, delay)| (id.0, Play { delay, ..play }))
            .collect()
        } else {
            Vec::new()
        };
        ui.quit_after(plays, EXIT_LIMIT);
    }
}

/// `spaces` in reading order: top to bottom, and left to right where tops
/// are level.
fn reading_order(spaces: &[(SpaceId, Rect)]) -> Vec<SpaceId> {
    let mut sorted = spaces.to_vec();
    sorted.sort_by_key(|(_, rect)| (rect.y, rect.x));
    sorted.into_iter().map(|(id, _)| id).collect()
}

/// How long each of `ids`, in turn, waits: all at once, or one after another
/// `stagger` apart.
fn staggered(
    ids: Vec<SpaceId>,
    order: StartupOrder,
    stagger: Duration,
) -> Vec<(SpaceId, Duration)> {
    ids.into_iter()
        .enumerate()
        .map(|(turn, id)| {
            let delay = match order {
                StartupOrder::AllAtOnce => Duration::ZERO,
                StartupOrder::OneAfterAnother => stagger * turn as u32,
            };
            (id, delay)
        })
        .collect()
}

/// When each space starts leaving at exit: startup's order in reverse, the
/// stagger shrunk so the last one, playing for `duration`, ends within
/// `EXIT_LIMIT`.
fn exit_delays(
    spaces: &[(SpaceId, Rect)],
    order: StartupOrder,
    stagger: Duration,
    duration: Duration,
) -> Vec<(SpaceId, Duration)> {
    let mut ids = reading_order(spaces);
    ids.reverse();
    let turns = ids.len().saturating_sub(1) as u32;
    let room = EXIT_LIMIT.saturating_sub(duration);
    let stagger = match turns {
        0 => stagger,
        turns => stagger.min(room / turns),
    };
    staggered(ids, order, stagger)
}

/// The `Play` for `kind` going `way`: what the settings say, or nothing at
/// all while Windows' "Animation effects" is off (`system_on`).
fn play_for(animations: &Animations, system_on: bool, kind: Kind, way: Way) -> Play {
    if !system_on {
        return Play::instant(way);
    }
    let resolved = animations.resolve(kind);
    Play {
        effect: resolved.effect,
        way,
        duration: Duration::from_millis(resolved.duration_ms.into()),
        delay: Duration::ZERO,
        detail: resolved.detail,
        edge: resolved.direction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::animation::{Edge, Effect, EffectChoice};

    #[test]
    fn the_settings_become_the_play() {
        let mut animations = Animations::default();
        animations.startup_exit.effect = EffectChoice::Effect(Effect::Fly);
        animations.startup_exit.duration_ms = 400;
        animations.startup_exit.direction = Edge::Left;
        let play = play_for(&animations, true, Kind::Startup, Way::Out);
        assert_eq!(play.effect, Effect::Fly);
        assert_eq!(play.way, Way::Out);
        assert_eq!(play.duration, Duration::from_millis(400));
        assert_eq!(play.edge, Edge::Left);
        assert_eq!(play.delay, Duration::ZERO);
    }

    #[test]
    fn delete_plays_the_startup_effect_at_its_own_speed() {
        let mut animations = Animations::default();
        animations.startup_exit.effect = EffectChoice::Effect(Effect::Crt);
        animations.delete.duration_ms = 600;
        let play = play_for(&animations, true, Kind::Delete, Way::Out);
        assert_eq!(play.effect, Effect::Crt);
        assert_eq!(play.duration, Duration::from_millis(600));
    }

    #[test]
    fn startup_staggers_spaces_in_reading_order() {
        let spaces = [
            (SpaceId(1), Rect::new(500, 300, 100, 100)),
            (SpaceId(2), Rect::new(600, 20, 100, 100)),
            (SpaceId(3), Rect::new(20, 20, 100, 100)),
        ];
        let stagger = Duration::from_millis(50);
        assert_eq!(
            staggered(
                reading_order(&spaces),
                StartupOrder::OneAfterAnother,
                stagger
            ),
            [
                (SpaceId(3), Duration::ZERO),
                (SpaceId(2), Duration::from_millis(50)),
                (SpaceId(1), Duration::from_millis(100)),
            ]
        );
        let together = staggered(reading_order(&spaces), StartupOrder::AllAtOnce, stagger);
        assert!(together.iter().all(|(_, delay)| delay.is_zero()));
    }

    #[test]
    fn exit_leaves_in_reverse_and_within_the_limit() {
        let spaces = [
            (SpaceId(1), Rect::new(20, 20, 100, 100)),
            (SpaceId(2), Rect::new(600, 20, 100, 100)),
            (SpaceId(3), Rect::new(500, 300, 100, 100)),
        ];
        let stagger = Duration::from_millis(50);
        let duration = Duration::from_millis(250);
        assert_eq!(
            exit_delays(&spaces, StartupOrder::OneAfterAnother, stagger, duration),
            [
                (SpaceId(3), Duration::ZERO),
                (SpaceId(2), Duration::from_millis(50)),
                (SpaceId(1), Duration::from_millis(100)),
            ]
        );
        // Forty spaces at 500 ms apart would take 20 s; the stagger shrinks
        // so the last one ends by the limit.
        let many: Vec<(SpaceId, Rect)> = (0..40)
            .map(|index| (SpaceId(index), Rect::new(index as i32 * 10, 0, 50, 50)))
            .collect();
        let delays = exit_delays(
            &many,
            StartupOrder::OneAfterAnother,
            Duration::from_millis(500),
            Duration::from_millis(1000),
        );
        let last = delays.iter().map(|(_, delay)| *delay).max().unwrap();
        assert!(last + Duration::from_millis(1000) <= EXIT_LIMIT, "{last:?}");
        assert!(last > Duration::ZERO, "still staggered");
    }

    #[test]
    fn nothing_plays_while_windows_animations_are_off() {
        let play = play_for(&Animations::default(), false, Kind::Startup, Way::In);
        assert_eq!(play, Play::instant(Way::In));
    }
}
