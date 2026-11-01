//! `App` animations (ROADMAP D12): what each kind of moment plays, from the
//! settings and Windows' "Animation effects" switch. The platform plays it.

use std::num::NonZeroU32;
use std::time::Duration;

use deskspace_core::animation::{Animations, Kind, StartupOrder};
use deskspace_core::{Rect, SpaceId};
use deskspace_platform::{Play, Ui, Way};

use super::App;
use super::effect::Effect;

/// The longest an exit may take to animate before DeskSpace tears down
/// anyway (ROADMAP D13).
pub(super) const EXIT_LIMIT: Duration = Duration::from_secs(2);

/// The debug window's animation tweaks (ROADMAP D14), for this session
/// only: slow motion, and playing even while Windows' switch is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AnimationTweak {
    /// How many times slower everything plays; 1 is normal.
    pub speed: NonZeroU32,
    /// Play as if Windows' "Animation effects" were on.
    pub ignore_windows: bool,
}

impl Default for AnimationTweak {
    fn default() -> Self {
        Self {
            speed: NonZeroU32::MIN,
            ignore_windows: false,
        }
    }
}

impl AnimationTweak {
    /// `time` stretched by the speed.
    fn slow(self, time: Duration) -> Duration {
        time * self.speed.get()
    }
}

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
            self.animation_tweak,
            kind,
            way,
        )
    }

    /// The configured stagger between spaces, stretched by the tweak's speed.
    fn stagger(&self) -> Duration {
        let stagger_ms = self.config.window.animations.stagger_ms;
        self.animation_tweak
            .slow(Duration::from_millis(stagger_ms.into()))
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
            Arrival::Startup => {
                staggered(reading_order(spaces), animations.startup, self.stagger())
            }
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
    pub(super) fn exit(&mut self, ui: &mut Ui) {
        let effects = self.exit_effects();
        self.run(ui, effects);
    }

    /// What exiting asks of the shell: quit after the shown spaces' way out,
    /// staggered so the last one ends within `EXIT_LIMIT`.
    fn exit_effects(&self) -> Vec<Effect> {
        if !self.visible {
            return vec![Effect::QuitAfter(Vec::new())];
        }
        let spaces: Vec<(SpaceId, Rect)> = self
            .config
            .desktop
            .spaces()
            .iter()
            .map(|space| (space.id, space.visible_rect()))
            .collect();
        let play = self.play(Kind::Startup, Way::Out);
        let animations = &self.config.window.animations;
        let plays = exit_delays(&spaces, animations.startup, self.stagger(), play.duration)
            .into_iter()
            .map(|(id, delay)| (id, Play { delay, ..play }))
            .collect();
        vec![Effect::QuitAfter(plays)]
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

/// The `Play` for `kind` going `way`: what the settings say, at the pace the
/// effect's row sets, or nothing at all while Windows' "Animation effects" is
/// off (`system_on`), unless the debug `tweak` ignores that switch; its speed
/// stretches the duration.
fn play_for(
    animations: &Animations,
    system_on: bool,
    tweak: AnimationTweak,
    kind: Kind,
    way: Way,
) -> Play {
    if !system_on && !tweak.ignore_windows {
        return Play::instant(way);
    }
    let resolved = animations.resolve(kind);
    let chosen = Duration::from_millis(resolved.duration_ms.into());
    Play {
        effect: resolved.effect,
        way,
        duration: tweak.slow(resolved.effect.spec().stretched(chosen)),
        delay: Duration::ZERO,
        detail: resolved.detail,
        edge: resolved.direction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::effect::Effect as AppEffect;
    use deskspace_core::animation::{Edge, Effect, EffectChoice};

    #[test]
    fn an_effects_row_stretches_the_chosen_duration() {
        let normal = AnimationTweak::default();
        let duration_of = |effect| {
            let mut animations = Animations::default();
            animations.startup_exit.effect = EffectChoice::Effect(effect);
            animations.startup_exit.duration_ms = 250;
            play_for(&animations, true, normal, Kind::Startup, Way::In).duration
        };
        assert_eq!(duration_of(Effect::Roll), Duration::from_millis(250));
        assert_eq!(duration_of(Effect::Bounce), Duration::from_millis(500));
    }

    #[test]
    fn the_settings_become_the_play() {
        let mut animations = Animations::default();
        animations.startup_exit.effect = EffectChoice::Effect(Effect::Fly);
        animations.startup_exit.duration_ms = 400;
        animations.startup_exit.direction = Edge::Left;
        let normal = AnimationTweak::default();
        let play = play_for(&animations, true, normal, Kind::Startup, Way::Out);
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
        let play = play_for(
            &animations,
            true,
            AnimationTweak::default(),
            Kind::Delete,
            Way::Out,
        );
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

    /// An app with two shown spaces, the lower one made first, that animates
    /// whatever Windows' switch says.
    fn app_with_two_spaces() -> (App, SpaceId, SpaceId) {
        let store = deskspace_config::ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(deskspace_config::Config::new(), store, false, false);
        let desktop = &mut app.config.desktop;
        let lower = crate::fixtures::create_space(desktop, "Lower", Rect::new(20, 400, 200, 150));
        let upper = crate::fixtures::create_space(desktop, "Upper", Rect::new(20, 20, 200, 150));
        app.visible = true;
        app.animation_tweak.ignore_windows = true;
        (app, lower, upper)
    }

    #[test]
    fn exiting_plays_every_shown_space_out_in_reverse_reading_order() {
        let (app, lower, upper) = app_with_two_spaces();
        let effects = app.exit_effects();
        let [AppEffect::QuitAfter(plays)] = effects.as_slice() else {
            panic!("exit asks for one QuitAfter");
        };
        let order: Vec<SpaceId> = plays.iter().map(|(id, _)| *id).collect();
        let lower_turn = order.iter().position(|id| *id == lower).unwrap();
        let upper_turn = order.iter().position(|id| *id == upper).unwrap();
        assert!(lower_turn < upper_turn, "the lower space leaves first");
        assert!(plays.iter().all(|(_, play)| play.way == Way::Out));
    }

    #[test]
    fn exiting_while_hidden_quits_at_once() {
        let (mut app, _, _) = app_with_two_spaces();
        app.visible = false;
        assert_eq!(app.exit_effects(), [AppEffect::QuitAfter(Vec::new())]);
    }

    #[test]
    fn nothing_plays_while_windows_animations_are_off() {
        let normal = AnimationTweak::default();
        let play = play_for(
            &Animations::default(),
            false,
            normal,
            Kind::Startup,
            Way::In,
        );
        assert_eq!(play, Play::instant(Way::In));
    }

    #[test]
    fn slow_motion_stretches_the_duration_and_the_stagger() {
        let mut animations = Animations::default();
        animations.startup_exit.effect = EffectChoice::Effect(Effect::Fly);
        animations.startup_exit.duration_ms = 300;
        let quarter = AnimationTweak {
            speed: NonZeroU32::new(4).unwrap(),
            ignore_windows: false,
        };
        let play = play_for(&animations, true, quarter, Kind::Startup, Way::In);
        assert_eq!(play.duration, Duration::from_millis(1200));
        assert_eq!(
            quarter.slow(Duration::from_millis(50)),
            Duration::from_millis(200)
        );
    }

    #[test]
    fn ignoring_windows_plays_with_its_switch_off() {
        let mut animations = Animations::default();
        animations.startup_exit.effect = EffectChoice::Effect(Effect::Fly);
        let ignore = AnimationTweak {
            ignore_windows: true,
            ..AnimationTweak::default()
        };
        let play = play_for(&animations, false, ignore, Kind::Startup, Way::In);
        assert_eq!(play.effect, Effect::Fly);
        assert!(!play.duration.is_zero());
    }

    #[test]
    fn the_default_tweak_changes_nothing() {
        let animations = Animations::default();
        let tweaked = play_for(
            &animations,
            true,
            AnimationTweak::default(),
            Kind::Startup,
            Way::In,
        );
        assert_eq!(
            tweaked.duration,
            Duration::from_millis(animations.startup_exit.duration_ms.into())
        );
        assert_eq!(
            AnimationTweak::default().slow(Duration::from_millis(70)),
            Duration::from_millis(70)
        );
    }
}
