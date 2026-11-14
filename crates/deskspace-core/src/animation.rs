//! Animations (ROADMAP D12): the effects, the kinds of moment that play
//! them, and their settings. Pure data: the app turns a kind's settings into
//! something to play, and the platform plays it.
//!
//! Two tables carry the design, so a new effect or kind is one row:
//! `EFFECTS` says which options each effect uses, and `KINDS` says which
//! effects and options each kind offers.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::lenient::known_or_default;

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

/// Smoothstep: 0 at 0 and 1 at 1, slow at both ends, and symmetric, so
/// played backwards it traces the same curve.
pub fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// How much of the fall's distance its speed at the start covers: the space
/// crosses the edge already moving, so an Out leaves the screen rather than
/// hanging at its edge.
const BOUNCE_ENTRY: f32 = 0.25;
/// How much of its speed each landing keeps.
const BOUNCE_KEPT: f32 = 1.0 / 3.0;
/// How much of the play the first rebound lasts, per unit of the fall.
const BOUNCE_REBOUND: f32 = BOUNCE_KEPT * (2.0 - BOUNCE_ENTRY) / (1.0 - BOUNCE_ENTRY);
/// When the space lands: the end of the fall and of the first rebound. The
/// second rebound lands at 1.
const BOUNCE_LANDINGS: [f32; 2] = {
    let fall = 1.0 / (1.0 + BOUNCE_REBOUND + BOUNCE_REBOUND * BOUNCE_KEPT);
    [fall, fall * (1.0 + BOUNCE_REBOUND)]
};
/// The acceleration that carries the fall, which the rebounds share.
const BOUNCE_GRAVITY: f32 = 2.0 * (1.0 - BOUNCE_ENTRY) / (BOUNCE_LANDINGS[0] * BOUNCE_LANDINGS[0]);

/// A fall that lands and settles in two shrinking rebounds, one parabola
/// each under one gravity. Penner's easeOutBounce (four arcs, a first rebound
/// a quarter of the fall) left its last arcs a frame or two at the usual
/// durations and read as jitter.
fn bounce_ease(t: f32) -> f32 {
    let [fall, first] = BOUNCE_LANDINGS;
    if t < fall {
        let t = t / fall;
        return BOUNCE_ENTRY * t + (1.0 - BOUNCE_ENTRY) * t * t;
    }
    let (start, end) = if t < first {
        (fall, first)
    } else {
        (first, 1.0)
    };
    let half = (end - start) / 2.0;
    let from_peak = t - (start + half);
    1.0 - BOUNCE_GRAVITY / 2.0 * (half * half - from_peak * from_peak)
}

/// How an effect's shown progress moves from 0 (gone) to 1 (whole).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    /// Slow at both ends (`smoothstep`).
    Smooth,
    /// Starts slowly when going out (`1 - (1 - u)²`) and races to the end,
    /// so fire catches before it sweeps away.
    Kindle,
    /// Drops in and settles in two shrinking rebounds.
    Bounce,
}

impl Curve {
    /// Maps linear shown progress `u` (0 to 1) to eased shown progress.
    pub fn apply(self, u: f32) -> f32 {
        match self {
            Self::Smooth => smoothstep(u),
            Self::Kindle => 1.0 - (1.0 - u) * (1.0 - u),
            Self::Bounce => bounce_ease(u),
        }
    }
}

/// How long an effect plays relative to the chosen duration, and the curve
/// its progress follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pace {
    /// How long it plays, in percent of the chosen duration, so an effect
    /// that needs more time takes it without a special case.
    pub stretch_percent: u16,
    pub curve: Curve,
}

pub const PACE_NORMAL: Pace = Pace {
    stretch_percent: 100,
    curve: Curve::Smooth,
};

pub const PACE_STRETCH_RANGE: (u16, u16) = (50, 300);

impl Pace {
    /// How long the effect plays when `duration` is the chosen duration.
    pub fn stretch(self, duration: Duration) -> Duration {
        duration * u32::from(self.stretch_percent) / 100
    }
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
    pub pace: Pace,
}

use EffectOption::{Detail, Direction};

/// Every effect, in the order the settings page lists them.
pub const EFFECTS: &[EffectSpec] = &[
    effect(Effect::None, "none", &[], PACE_NORMAL),
    effect(Effect::Roll, "roll", &[Direction], PACE_NORMAL),
    effect(Effect::Fade, "fade", &[], PACE_NORMAL),
    effect(Effect::Pixelate, "pixelate", &[Detail], PACE_NORMAL),
    effect(Effect::Gradient, "gradient", &[Direction], PACE_NORMAL),
    effect(Effect::Dissolve, "dissolve", &[Detail], PACE_NORMAL),
    effect(Effect::Fly, "fly", &[Direction], PACE_NORMAL).auto_edge(AutoEdge::Nearest),
    effect(Effect::Crt, "crt", &[], PACE_NORMAL),
    effect(
        Effect::Burn,
        "burn",
        &[Detail],
        Pace {
            stretch_percent: 200,
            curve: Curve::Kindle,
        },
    ),
    effect(
        Effect::Bounce,
        "bounce",
        &[Direction],
        Pace {
            stretch_percent: 200,
            curve: Curve::Bounce,
        },
    ),
];

const fn effect(
    effect: Effect,
    key: &'static str,
    options: &'static [EffectOption],
    pace: Pace,
) -> EffectSpec {
    EffectSpec {
        effect,
        key,
        options,
        auto_edge: AutoEdge::Top,
        pace,
    }
}

impl EffectSpec {
    const fn auto_edge(self, auto_edge: AutoEdge) -> Self {
        Self { auto_edge, ..self }
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
    #[serde(deserialize_with = "known_or_default")]
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
    #[serde(deserialize_with = "known_or_default")]
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

    #[test]
    fn smoothstep_runs_from_0_to_1_and_is_symmetric() {
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert_eq!(smoothstep(0.5), 0.5);
        for step in 0..=10 {
            let t = step as f32 / 10.0;
            assert!((smoothstep(t) + smoothstep(1.0 - t) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn bounce_ease_lands_at_least_three_times_and_stays_in_range() {
        assert_eq!(bounce_ease(0.0), 0.0);
        assert!((bounce_ease(1.0) - 1.0).abs() < 1e-6);
        for step in 0..=1000 {
            let t = step as f32 / 1000.0;
            let value = bounce_ease(t);
            assert!((0.0..=1.0).contains(&value), "at {t}: {value}");
        }
        let [fall, first] = BOUNCE_LANDINGS;
        assert!(bounce_ease((fall + first) / 2.0) < 1.0, "rebounds");
        assert!(bounce_ease((first + 1.0) / 2.0) < 1.0, "rebounds again");
        let landings = [fall, first, 1.0];
        let touched = landings
            .iter()
            .filter(|&&t| (bounce_ease(t) - 1.0).abs() < 1e-5)
            .count();
        assert_eq!(touched, 3, "touched 1 only {touched} times");
    }

    #[test]
    fn bounce_ease_moves_at_its_ends_and_every_landing_keeps_the_same_speed() {
        // The slope of `bounce_ease` at `t`, measured just inside its arc.
        let slope = |t: f32, sign: f32| (bounce_ease(t + sign * 1e-3) - bounce_ease(t)) / 1e-3;
        assert!(slope(0.0, 1.0) > 0.2, "enters already moving");
        let [fall, first] = BOUNCE_LANDINGS;
        let (lands, leaves) = (slope(fall, -1.0), -slope(fall, 1.0));
        assert!(
            (leaves / lands + BOUNCE_KEPT).abs() < 0.02,
            "{lands} {leaves}"
        );
        let (lands, leaves) = (slope(first, -1.0), -slope(first, 1.0));
        assert!(
            (leaves / lands + BOUNCE_KEPT).abs() < 0.02,
            "{lands} {leaves}"
        );
    }

    #[test]
    fn every_bounce_is_long_enough_to_see_at_the_default_duration() {
        // At 60 Hz, the slowest common refresh, a shorter arc shows as a
        // twitch rather than a bounce.
        const REFRESH_MS: f32 = 1000.0 / 60.0;
        const FRAMES: f32 = 3.0;
        let chosen = Duration::from_millis(DURATION_MS_DEFAULT.into());
        let played = Effect::Bounce.spec().pace.stretch(chosen).as_secs_f32() * 1000.0;
        let [fall, first] = BOUNCE_LANDINGS;
        for arc in [fall, first - fall, 1.0 - first] {
            assert!(arc * played >= FRAMES * REFRESH_MS, "{arc} of {played} ms");
        }
    }

    #[test]
    fn every_effect_pace_is_in_range_and_curves_behave() {
        for spec in EFFECTS {
            assert!(
                (PACE_STRETCH_RANGE.0..=PACE_STRETCH_RANGE.1).contains(&spec.pace.stretch_percent),
                "{:?}: {}",
                spec.effect,
                spec.pace.stretch_percent
            );
        }
        for curve in [Curve::Smooth, Curve::Kindle, Curve::Bounce] {
            assert_eq!(curve.apply(0.0), 0.0, "{curve:?} at 0");
            assert!((curve.apply(1.0) - 1.0).abs() < 1e-6, "{curve:?} at 1");
        }
        for curve in [Curve::Smooth, Curve::Kindle] {
            let mut prev = 0.0_f32;
            for step in 0..=100 {
                let u = step as f32 / 100.0;
                let value = curve.apply(u);
                assert!(value >= prev, "{curve:?} went backward at {u}");
                prev = value;
            }
        }
        assert!(
            Curve::Kindle.apply(0.9) > 0.98,
            "a burn barely moves in its first tenth"
        );
    }
}
