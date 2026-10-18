//! `App` animations (ROADMAP D12): what each kind of moment plays, from the
//! settings and Windows' "Animation effects" switch. The platform plays it.

use std::time::Duration;

use deskspace_core::animation::{Animations, Kind};
use deskspace_platform::{Play, Way};

use super::App;

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
        animations.hide_show.effect = EffectChoice::Effect(Effect::Fly);
        animations.hide_show.duration_ms = 400;
        animations.hide_show.direction = Edge::Left;
        let play = play_for(&animations, true, Kind::HideShow, Way::Out);
        assert_eq!(play.effect, Effect::Fly);
        assert_eq!(play.way, Way::Out);
        assert_eq!(play.duration, Duration::from_millis(400));
        assert_eq!(play.edge, Edge::Left);
        assert_eq!(play.delay, Duration::ZERO);
    }

    #[test]
    fn delete_plays_the_new_space_effect_at_its_own_speed() {
        let mut animations = Animations::default();
        animations.new_space.effect = EffectChoice::Effect(Effect::Crt);
        animations.delete.duration_ms = 600;
        let play = play_for(&animations, true, Kind::Delete, Way::Out);
        assert_eq!(play.effect, Effect::Crt);
        assert_eq!(play.duration, Duration::from_millis(600));
    }

    #[test]
    fn nothing_plays_while_windows_animations_are_off() {
        let play = play_for(&Animations::default(), false, Kind::HideShow, Way::In);
        assert_eq!(play, Play::instant(Way::In));
    }
}
