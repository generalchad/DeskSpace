//! Animations (ROADMAP D12): the effects, the kinds of moment that play
//! them, and their settings. Pure data: the app turns a kind's settings into
//! something to play, and the platform plays it.
//!
//! Two tables carry the design, so a new effect or kind is one row:
//! `EFFECTS` says which options each effect uses, and `KINDS` says which
//! effects and options each kind offers.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// How a space appears or goes away. Every effect has an "in" and an "out",
/// the out being the in played backwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    /// No animation: the change is instant.
    None,
    /// Rolls into an edge (the top by default) and back out.
    Roll,
    Fade,
    /// Breaks into ever larger blocks.
    Pixelate,
    /// A soft edge that sweeps across, from the title bar by default.
    Gradient,
    /// Disappears grain by grain.
    Dissolve,
    /// Flies to or from an edge of its monitor's work area.
    Fly,
    /// An old tube turned on: a bright line that opens to the whole space,
    /// and closes to a line, then a dot.
    Crt,
    /// A dissolve whose edge glows as it burns.
    Burn,
    /// Drops in from beyond an edge and settles in shrinking bounces.
    Bounce,
}

/// An option an effect can use, beyond the duration every effect has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectOption {
    /// The block size, the grain, or the edge width (`DETAIL_RANGE`).
    Detail,
    /// Which edge the effect moves toward or sweeps from.
    Direction,
}

/// Where an effect with `EffectOption::Direction` goes when its direction
/// is `Edge::Auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoEdge {
    Top,
    /// The edge of the space's monitor work area nearest the space.
    Nearest,
}

/// One row of `EFFECTS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectSpec {
    pub effect: Effect,
    /// The name in the config file. Never change one: it is stored.
    pub key: &'static str,
    /// The options the effect uses; the settings page greys the rest.
    pub options: &'static [EffectOption],
    /// Where "Automatic" goes, for an effect that uses `Direction`: the top
    /// unless the row says otherwise.
    pub auto_edge: AutoEdge,
    /// How long it plays, in percent of the chosen duration, so an effect
    /// that needs more time takes it without a special case.
    pub stretch_percent: u16,
}

use EffectOption::{Detail, Direction};

/// Every effect, in the order the settings page lists them.
pub const EFFECTS: &[EffectSpec] = &[
    effect(Effect::None, "none", &[]),
    effect(Effect::Roll, "roll", &[Direction]),
    effect(Effect::Fade, "fade", &[]),
    effect(Effect::Pixelate, "pixelate", &[Detail]),
    effect(Effect::Gradient, "gradient", &[Direction]),
    effect(Effect::Dissolve, "dissolve", &[Detail]),
    effect(Effect::Fly, "fly", &[Direction]).auto_edge(AutoEdge::Nearest),
    effect(Effect::Crt, "crt", &[]),
    effect(Effect::Burn, "burn", &[Detail]),
    effect(Effect::Bounce, "bounce", &[Direction]).stretch_percent(200),
];

const fn effect(effect: Effect, key: &'static str, options: &'static [EffectOption]) -> EffectSpec {
    EffectSpec {
        effect,
        key,
        options,
        auto_edge: AutoEdge::Top,
        stretch_percent: 100,
    }
}

impl EffectSpec {
    const fn auto_edge(self, auto_edge: AutoEdge) -> Self {
        Self { auto_edge, ..self }
    }

    const fn stretch_percent(self, stretch_percent: u16) -> Self {
        Self {
            stretch_percent,
            ..self
        }
    }

    /// How long the effect plays when `chosen` is the chosen duration.
    pub fn stretched(&self, chosen: Duration) -> Duration {
        chosen * u32::from(self.stretch_percent) / 100
    }
}

impl Effect {
    pub fn spec(self) -> &'static EffectSpec {
        EFFECTS
            .iter()
            .find(|spec| spec.effect == self)
            .expect("every effect has an EFFECTS row")
    }

    pub fn uses(self, option: EffectOption) -> bool {
        self.spec().options.contains(&option)
    }

    fn from_key(key: &str) -> Option<Self> {
        EFFECTS
            .iter()
            .find(|spec| spec.key == key)
            .map(|spec| spec.effect)
    }
}

/// A moment that animates. Each has its own settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Spaces coming onto the screen and leaving it: DeskSpace starting and
    /// exiting, the spaces shown and hidden, and a new space appearing.
    Startup,
    /// A space going away.
    Delete,
    /// A space folding to its title bar and unfolding.
    Fold,
}

/// One row of `KINDS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindSpec {
    pub kind: Kind,
    /// The name in "same as" choices in the config file. Never change one:
    /// it is stored.
    pub key: &'static str,
    /// The effects the kind offers. Always includes `Effect::None` and the
    /// kind's default.
    pub offers: Offers,
    /// The options the kind lets the user change, when its effect uses them.
    pub options: &'static [EffectOption],
    /// A kind whose effect this one can follow ("Same as startup").
    pub follows: Option<Kind>,
    /// Whether the kind has the startup controls (order and delay).
    pub startup: bool,
    pub default: EffectChoice,
}

/// Which effects a kind offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offers {
    /// Every row of `EFFECTS`, so a new effect needs no edit here.
    Every,
    /// Only these, in `EFFECTS` order.
    Only(&'static [Effect]),
}

impl Offers {
    pub fn contains(self, effect: Effect) -> bool {
        match self {
            Offers::Every => true,
            Offers::Only(effects) => effects.contains(&effect),
        }
    }

    /// The effects offered, in `EFFECTS` order.
    pub fn effects(self) -> impl Iterator<Item = Effect> {
        EFFECTS
            .iter()
            .map(|spec| spec.effect)
            .filter(move |&effect| self.contains(effect))
    }
}

/// Every kind, in the order the settings page lists them.
pub const KINDS: &[KindSpec] = &[
    KindSpec {
        kind: Kind::Startup,
        key: "startup",
        offers: Offers::Every,
        options: &[Detail, Direction],
        follows: None,
        startup: true,
        default: EffectChoice::Effect(Effect::Roll),
    },
    KindSpec {
        kind: Kind::Delete,
        key: "delete",
        offers: Offers::Every,
        options: &[Detail, Direction],
        follows: Some(Kind::Startup),
        startup: false,
        default: EffectChoice::SameAs(Kind::Startup),
    },
    // A fold always collapses toward the title bar, so it has no direction.
    KindSpec {
        kind: Kind::Fold,
        key: "fold",
        offers: Offers::Only(&[Effect::None, Effect::Roll]),
        options: &[],
        follows: None,
        startup: false,
        default: EffectChoice::Effect(Effect::Roll),
    },
];

impl Kind {
    pub fn spec(self) -> &'static KindSpec {
        KINDS
            .iter()
            .find(|spec| spec.kind == self)
            .expect("every kind has a KINDS row")
    }

    fn offers(self, choice: EffectChoice) -> bool {
        let spec = self.spec();
        match choice {
            EffectChoice::Effect(effect) => spec.offers.contains(effect),
            EffectChoice::SameAs(kind) => spec.follows == Some(kind),
            EffectChoice::Unknown => false,
        }
    }
}

const SAME_AS_PREFIX: &str = "same_as_";

/// What a kind plays: an effect of its own, or whatever another kind plays.
/// Stored as a string (`"fade"`, `"same_as_startup"`); a name this version
/// does not know, from a newer one, loads as the kind's default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum EffectChoice {
    Effect(Effect),
    SameAs(Kind),
    /// A stored name this version does not know; `Animations::clamped`
    /// replaces it with the kind's default.
    Unknown,
}

impl From<String> for EffectChoice {
    fn from(key: String) -> Self {
        if let Some(kind) = key.strip_prefix(SAME_AS_PREFIX) {
            return KINDS
                .iter()
                .find(|spec| spec.key == kind)
                .map_or(Self::Unknown, |spec| Self::SameAs(spec.kind));
        }
        Effect::from_key(&key).map_or(Self::Unknown, Self::Effect)
    }
}

impl From<EffectChoice> for String {
    fn from(choice: EffectChoice) -> Self {
        match choice {
            EffectChoice::Effect(effect) => effect.spec().key.to_string(),
            EffectChoice::SameAs(kind) => format!("{SAME_AS_PREFIX}{}", kind.spec().key),
            // Never stored: loading replaces it.
            EffectChoice::Unknown => Effect::None.spec().key.to_string(),
        }
    }
}

/// Which edge an effect moves toward or sweeps from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    /// The effect's own (`EffectSpec::auto_edge`).
    #[default]
    Auto,
    Top,
    Bottom,
    Left,
    Right,
}

/// The order spaces appear in at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupOrder {
    AllAtOnce,
    /// One after another by position, `Animations::stagger_ms` apart.
    #[default]
    OneAfterAnother,
}

/// The inclusive range of an animation's duration, in milliseconds.
pub const DURATION_MS_RANGE: (u16, u16) = (100, 1000);
pub const DURATION_MS_DEFAULT: u16 = 250;
/// The inclusive range of `AnimationSettings::detail`: 1 is the finest
/// block, grain, or edge, 10 the coarsest.
pub const DETAIL_RANGE: (u8, u8) = (1, 10);
pub const DETAIL_DEFAULT: u8 = 5;
/// The inclusive range of the delay between spaces at startup, in
/// milliseconds.
pub const STAGGER_MS_RANGE: (u16, u16) = (10, 500);
pub const STAGGER_MS_DEFAULT: u16 = 50;

/// One kind's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimationSettings {
    pub effect: EffectChoice,
    pub duration_ms: u16,
    /// Used by effects with `EffectOption::Detail`.
    pub detail: u8,
    /// Used by effects with `EffectOption::Direction`.
    pub direction: Edge,
}

impl AnimationSettings {
    fn default_for(kind: Kind) -> Self {
        Self {
            effect: kind.spec().default,
            duration_ms: DURATION_MS_DEFAULT,
            detail: DETAIL_DEFAULT,
            direction: Edge::Auto,
        }
    }

    fn clamped(self, kind: Kind) -> Self {
        let effect = if kind.offers(self.effect) {
            self.effect
        } else {
            kind.spec().default
        };
        Self {
            effect,
            duration_ms: self
                .duration_ms
                .clamp(DURATION_MS_RANGE.0, DURATION_MS_RANGE.1),
            detail: self.detail.clamp(DETAIL_RANGE.0, DETAIL_RANGE.1),
            direction: self.direction,
        }
    }
}

impl Default for AnimationSettings {
    fn default() -> Self {
        Self::default_for(Kind::Startup)
    }
}

/// What a kind plays once "same as" is followed: always an effect of its
/// own, with the options that go with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    pub effect: Effect,
    pub duration_ms: u16,
    pub detail: u8,
    pub direction: Edge,
}

/// Every kind's settings, and the startup order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Animations {
    /// Startup's settings. Read from `hide_show` too, the key before Hide
    /// and show and New space merged into Startup (D12, amended
    /// 2026-10-18); a stored `new_space` table is ignored.
    #[serde(alias = "hide_show")]
    pub startup_exit: AnimationSettings,
    pub delete: AnimationSettings,
    pub fold: AnimationSettings,
    pub startup: StartupOrder,
    pub stagger_ms: u16,
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            startup_exit: AnimationSettings::default_for(Kind::Startup),
            delete: AnimationSettings::default_for(Kind::Delete),
            fold: AnimationSettings::default_for(Kind::Fold),
            startup: StartupOrder::default(),
            stagger_ms: STAGGER_MS_DEFAULT,
        }
    }
}

impl Animations {
    pub fn get(&self, kind: Kind) -> &AnimationSettings {
        match kind {
            Kind::Startup => &self.startup_exit,
            Kind::Delete => &self.delete,
            Kind::Fold => &self.fold,
        }
    }

    pub fn get_mut(&mut self, kind: Kind) -> &mut AnimationSettings {
        match kind {
            Kind::Startup => &mut self.startup_exit,
            Kind::Delete => &mut self.delete,
            Kind::Fold => &mut self.fold,
        }
    }

    /// What `kind` plays. Following another kind takes its effect, detail,
    /// and direction, but keeps this kind's own duration.
    pub fn resolve(&self, kind: Kind) -> Resolved {
        let own = self.get(kind);
        let source = match own.effect {
            EffectChoice::SameAs(other) if other != kind => self.get(other),
            _ => own,
        };
        let effect = match source.effect {
            EffectChoice::Effect(effect) => effect,
            // A chain or a stray value: `clamped` rules both out on load.
            EffectChoice::SameAs(_) | EffectChoice::Unknown => Effect::None,
        };
        Resolved {
            effect,
            duration_ms: own.duration_ms,
            detail: source.detail,
            direction: source.direction,
        }
    }

    /// The settings with every value in range and every kind's effect one it
    /// offers, so a hand-edited or newer file loads.
    pub fn clamped(self) -> Self {
        let mut out = self;
        for spec in KINDS {
            *out.get_mut(spec.kind) = self.get(spec.kind).clamped(spec.kind);
        }
        out.stagger_ms = self
            .stagger_ms
            .clamp(STAGGER_MS_RANGE.0, STAGGER_MS_RANGE.1);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every effect. The match makes a new `Effect` a compile error here, so
    /// it cannot miss its `EFFECTS` row.
    fn every_effect() -> [Effect; 10] {
        use Effect::*;
        let effects = [
            None, Roll, Fade, Pixelate, Gradient, Dissolve, Fly, Crt, Burn, Bounce,
        ];
        for effect in effects {
            match effect {
                None | Roll | Fade | Pixelate | Gradient | Dissolve => {}
                Fly | Crt | Burn | Bounce => {}
            }
        }
        effects
    }

    #[test]
    fn every_effect_has_one_row_and_a_unique_key() {
        assert_eq!(EFFECTS.len(), every_effect().len());
        for effect in every_effect() {
            assert_eq!(EFFECTS.iter().filter(|row| row.effect == effect).count(), 1);
            assert_eq!(Effect::from_key(effect.spec().key), Some(effect));
        }
    }

    #[test]
    fn effects_use_the_options_d12_gives_them() {
        let detail = [Effect::Pixelate, Effect::Dissolve, Effect::Burn];
        let direction = [Effect::Roll, Effect::Gradient, Effect::Fly, Effect::Bounce];
        for spec in EFFECTS {
            assert_eq!(spec.effect.uses(Detail), detail.contains(&spec.effect));
            assert_eq!(
                spec.effect.uses(Direction),
                direction.contains(&spec.effect)
            );
        }
    }

    #[test]
    fn automatic_is_the_top_but_for_fly() {
        for spec in EFFECTS {
            let expected = if spec.effect == Effect::Fly {
                AutoEdge::Nearest
            } else {
                AutoEdge::Top
            };
            assert_eq!(spec.auto_edge, expected, "{:?}", spec.effect);
        }
    }

    #[test]
    fn a_kind_offers_its_effects_in_effects_order() {
        let order = |effect: Effect| EFFECTS.iter().position(|spec| spec.effect == effect);
        for spec in KINDS {
            let offered: Vec<_> = spec.offers.effects().map(order).collect();
            assert!(offered.windows(2).all(|pair| pair[0] < pair[1]), "{spec:?}");
            if let Offers::Only(effects) = spec.offers {
                assert_eq!(offered.len(), effects.len(), "{spec:?} lists only effects");
            }
        }
        assert_eq!(Offers::Every.effects().count(), EFFECTS.len());
    }

    /// Every kind. The match makes a new `Kind` a compile error here, so it
    /// cannot miss its `KINDS` row.
    fn every_kind() -> [Kind; 3] {
        let kinds = [Kind::Startup, Kind::Delete, Kind::Fold];
        for kind in kinds {
            match kind {
                Kind::Startup | Kind::Delete | Kind::Fold => {}
            }
        }
        kinds
    }

    #[test]
    fn every_kind_has_one_row_and_offers_none_and_its_default() {
        assert_eq!(KINDS.len(), every_kind().len());
        for kind in every_kind() {
            let spec = kind.spec();
            assert_eq!(KINDS.iter().filter(|row| row.kind == kind).count(), 1);
            assert!(spec.offers.contains(Effect::None), "{kind:?}");
            assert!(kind.offers(spec.default), "{kind:?}");
        }
    }

    #[test]
    fn defaults_are_d12s() {
        let animations = Animations::default();
        assert_eq!(animations.resolve(Kind::Startup).effect, Effect::Roll);
        assert_eq!(
            animations.delete.effect,
            EffectChoice::SameAs(Kind::Startup)
        );
        assert_eq!(animations.resolve(Kind::Fold).effect, Effect::Roll);
        assert_eq!(animations.startup, StartupOrder::OneAfterAnother);
        assert_eq!(animations.stagger_ms, 50);
        for spec in KINDS {
            assert_eq!(animations.get(spec.kind).duration_ms, 250);
        }
    }

    #[test]
    fn following_takes_the_effect_and_options_but_not_the_duration() {
        let mut animations = Animations {
            startup_exit: AnimationSettings {
                effect: EffectChoice::Effect(Effect::Pixelate),
                duration_ms: 400,
                detail: 9,
                direction: Edge::Left,
            },
            ..Animations::default()
        };
        animations.delete.duration_ms = 600;
        let delete = animations.resolve(Kind::Delete);
        assert_eq!(delete.effect, Effect::Pixelate);
        assert_eq!((delete.detail, delete.direction), (9, Edge::Left));
        assert_eq!(delete.duration_ms, 600);
    }

    #[test]
    fn choices_round_trip_through_their_stored_names() {
        for choice in [
            EffectChoice::Effect(Effect::Crt),
            EffectChoice::SameAs(Kind::Startup),
        ] {
            assert_eq!(EffectChoice::from(String::from(choice)), choice);
        }
        assert_eq!(
            String::from(EffectChoice::SameAs(Kind::Startup)),
            "same_as_startup"
        );
        assert_eq!(
            EffectChoice::from("black_hole".to_string()),
            EffectChoice::Unknown
        );
        assert_eq!(
            EffectChoice::from("same_as_nothing".to_string()),
            EffectChoice::Unknown
        );
    }

    #[test]
    fn clamping_brings_every_value_into_range() {
        let mut animations = Animations::default();
        animations.startup_exit.effect = EffectChoice::Unknown;
        animations.startup_exit.duration_ms = 5;
        animations.startup_exit.detail = 99;
        animations.delete.effect = EffectChoice::SameAs(Kind::Delete);
        animations.fold.effect = EffectChoice::Effect(Effect::Burn);
        animations.fold.duration_ms = 60_000;
        animations.stagger_ms = 0;
        let clamped = animations.clamped();
        assert_eq!(
            clamped.startup_exit.effect,
            EffectChoice::Effect(Effect::Roll)
        );
        assert_eq!(clamped.startup_exit.duration_ms, DURATION_MS_RANGE.0);
        assert_eq!(clamped.startup_exit.detail, DETAIL_RANGE.1);
        assert_eq!(
            clamped.delete.effect,
            EffectChoice::SameAs(Kind::Startup),
            "Delete cannot follow itself"
        );
        assert_eq!(clamped.fold.effect, EffectChoice::Effect(Effect::Roll));
        assert_eq!(clamped.fold.duration_ms, DURATION_MS_RANGE.1);
        assert_eq!(clamped.stagger_ms, STAGGER_MS_RANGE.0);
    }
}
