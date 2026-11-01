//! The frame math of an animation (ROADMAP D12): what a space window looks
//! like at any moment of a `Play`. Pure, with no Win32, so every effect is
//! tested here and the window side only applies frames.
//!
//! A transition tracks one linear value, `u`, from 0 (gone) to 1 (fully
//! there), and a direction. What is drawn is a function of the effect's
//! curve of `u` alone, smoothstep for every effect but Bounce's
//! `bounce_ease`, one symmetric curve for both directions, so an Out is the
//! In played backwards and reversing at any moment just turns the direction
//! around: the next frame equals the current one.

use std::time::Duration;

use deskspace_core::animation::{AutoEdge, DETAIL_RANGE, Edge, Effect};
use deskspace_core::geometry::{Point, Rect, Size};

use super::burn;
use super::math::{lerp, smoothstep};

/// Which way a play runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// Appearing: shows, unfolds, or a new space.
    In,
    /// Going away: hides, folds, or a deleted space.
    Out,
}

/// What the app asks a window to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Play {
    pub effect: Effect,
    pub way: Way,
    pub duration: Duration,
    /// How long to hold the first frame before starting (startup's stagger).
    pub delay: Duration,
    /// `DETAIL_RANGE`: the block size, grain, or edge width.
    pub detail: u8,
    pub edge: Edge,
}

impl Play {
    /// Nothing to animate: the change happens at once.
    pub fn instant(way: Way) -> Self {
        Self {
            effect: Effect::None,
            way,
            duration: Duration::ZERO,
            delay: Duration::ZERO,
            detail: DETAIL_RANGE.0,
            edge: Edge::Auto,
        }
    }

    /// The side this play moves toward or sweeps from, for a window at
    /// `rect` on `work_area`: its own edge, or for Automatic the one its
    /// effect's row names (`EffectSpec::auto_edge`).
    pub fn side(&self, rect: Rect, work_area: Rect) -> Side {
        match self.edge {
            Edge::Top => Side::Top,
            Edge::Bottom => Side::Bottom,
            Edge::Left => Side::Left,
            Edge::Right => Side::Right,
            Edge::Auto => match self.effect.spec().auto_edge {
                AutoEdge::Top => Side::Top,
                AutoEdge::Nearest => nearest_side(rect, work_area),
            },
        }
    }
}

/// An edge of a space, with "Automatic" already decided (`Play::side`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// The side of `work_area` nearest `rect`.
fn nearest_side(rect: Rect, work_area: Rect) -> Side {
    [
        (rect.top() - work_area.top(), Side::Top),
        (work_area.bottom() - rect.bottom(), Side::Bottom),
        (rect.left() - work_area.left(), Side::Left),
        (work_area.right() - rect.right(), Side::Right),
    ]
    .into_iter()
    .min_by_key(|(distance, _)| *distance)
    .map_or(Side::Top, |(_, side)| side)
}

/// Where a window is and what it may cover, in desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// The window's rect when fully there.
    pub rect: Rect,
    /// The rect Roll collapses to: nothing at an edge when hiding, the title
    /// bar when folding.
    pub rolled: Rect,
    /// The work area of the window's monitor; Fly and Bounce never leave it.
    pub work_area: Rect,
}

impl Geometry {
    /// A window that hides and shows (or comes and goes) with `play`: Roll
    /// collapses it into the play's side, and Fly and Bounce use `work_area`.
    pub fn for_visibility(rect: Rect, work_area: Rect, play: &Play) -> Self {
        let rolled = match play.side(rect, work_area) {
            Side::Top => Rect::new(rect.x, rect.y, rect.width, 0),
            Side::Bottom => Rect::new(rect.x, rect.bottom(), rect.width, 0),
            Side::Left => Rect::new(rect.x, rect.y, 0, rect.height),
            Side::Right => Rect::new(rect.right(), rect.y, 0, rect.height),
        };
        Self {
            rect,
            rolled,
            work_area,
        }
    }

    /// A space folding between `unfolded` and its title bar, `folded`.
    pub fn for_fold(unfolded: Rect, folded: Rect, work_area: Rect) -> Self {
        Self {
            rect: unfolded,
            rolled: folded,
            work_area,
        }
    }
}

/// What a frame hides of the window's pixels, beyond its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mask {
    None,
    /// A soft edge sweeping in from `side`. Along the axis from that side
    /// (0 at the side, 1 at the far side), a pixel is hidden before
    /// `position - feather`, fades in over `feather`, and shows from
    /// `position` on.
    Gradient {
        side: Side,
        position: f32,
        feather: f32,
    },
    /// Grain-sized cells, each shown while its noise value (0 to 1) is at or
    /// above `threshold` (Dissolve).
    Noise {
        threshold: f32,
        grain: u8,
    },
    /// Paper burning up from the bottom (`render::burn`): `front` is how far
    /// the burn has got, and `detail` how ragged its edge is.
    Burn {
        front: f32,
        detail: u8,
    },
}

impl Mask {
    /// Whether the mask leaves every pixel showing.
    pub fn hides_nothing(self) -> bool {
        match self {
            Mask::None => true,
            Mask::Gradient {
                position, feather, ..
            } => position - feather <= 0.0,
            Mask::Noise { threshold, .. } => threshold <= 0.0,
            Mask::Burn { front, .. } => front <= -burn::BANDS,
        }
    }
}

/// How a window looks at one moment of a transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionFrame {
    /// How far along the window is: 0 is gone (or folded), 1 fully there.
    pub shown: f32,
    /// The window's rect, in desktop coordinates; empty when nothing shows.
    pub rect: Rect,
    /// The size of the whole space, which the painter draws at whatever the
    /// window's size, shifted by `content_offset`.
    pub space: Size,
    /// Where the space's own top-left lands, relative to the window's, so
    /// content can stay put while the window around it moves or crops.
    pub content_offset: Point,
    /// The opacity of the tint and the contents, 0 to 1.
    pub opacity: f32,
    /// What of the contents shows.
    pub mask: Mask,
    /// How the contents are drawn.
    pub content: Content,
    /// The work area cuts this frame's window short of the space (Fly,
    /// Bounce), so it changes size however fast the space moves.
    pub clipped: bool,
}

impl TransitionFrame {
    /// Whether the frame's contents are as opaque as at rest over the whole
    /// window, so nothing behind them shows through: what moves is only the
    /// window's rect (Roll, Fly, Bounce, CRT).
    pub fn covers_window(&self) -> bool {
        self.opacity >= 1.0 && self.content.covers() && self.mask.hides_nothing()
    }

    /// How strongly the space casts its shadow, 0 to 1 (ROADMAP D15). A
    /// frame that only moves the window casts it whole, since the shadow
    /// follows the rect. One that leaves the window see-through fades it as
    /// far as the space has faded, or as far as it has gone. A clipped one
    /// casts none: a shadow of a new size reaches the screen a frame after
    /// its window, and it is hollow, so a space crossing the edge left an
    /// empty outlined box behind it (seen 2026-10-31).
    pub fn shadow_strength(&self) -> f32 {
        if self.clipped {
            0.0
        } else if self.covers_window() {
            1.0
        } else {
            self.opacity.min(self.shown).clamp(0.0, 1.0)
        }
    }
}

/// How a frame draws the space's contents (ROADMAP D12, amended
/// 2026-10-19): one variant per way an effect changes them, rather than a
/// frame field per effect. An effect that draws them some new way adds a
/// variant, and the painter's `match` asks for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Content {
    /// As at rest.
    Whole,
    /// In square blocks `block` pixels wide, more than 1 (Pixelate).
    Pixelated { block: u32 },
    /// Whole, under a bright line's light, `glow` from above 0 to 1 (CRT).
    Lit { glow: f32 },
}

impl Content {
    /// Blocks `block` pixels wide; 1 or less is `Whole`.
    pub fn pixelated(block: u32) -> Self {
        if block <= 1 {
            Content::Whole
        } else {
            Content::Pixelated { block }
        }
    }

    /// Lit by `glow`; none is `Whole`.
    pub fn lit(glow: f32) -> Self {
        if glow <= 0.0 {
            Content::Whole
        } else {
            Content::Lit { glow }
        }
    }

    /// Whether the contents cover every pixel they cover at rest.
    fn covers(self) -> bool {
        match self {
            Content::Whole | Content::Lit { .. } => true,
            // A block takes its whole area's color, so its edges blend.
            Content::Pixelated { .. } => false,
        }
    }
}

/// A running `Play`. Every method takes `now`, a time on any one clock, so
/// it is tested without waiting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    play: Play,
    geometry: Geometry,
    /// `u` at `anchor`, and the direction it moves in from there.
    anchor_u: f32,
    anchor: Duration,
    way: Way,
    /// Nothing moves before this time.
    start: Duration,
}

impl Transition {
    pub fn new(play: Play, geometry: Geometry, now: Duration) -> Self {
        let anchor_u = match play.way {
            Way::In => 0.0,
            Way::Out => 1.0,
        };
        Self {
            play,
            geometry,
            anchor_u,
            anchor: now,
            way: play.way,
            start: now + play.delay,
        }
    }

    /// The way it runs now, which `reverse` turns around.
    pub fn way(&self) -> Way {
        self.way
    }

    /// The play it started with; its `way` is the one it started in.
    pub fn play(&self) -> Play {
        self.play
    }

    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    /// Changes where the window ends up (it moved, or its rect changed while
    /// animating), keeping the progress.
    pub fn set_geometry(&mut self, geometry: Geometry) {
        self.geometry = geometry;
    }

    /// The linear progress at `now`, 0 to 1.
    fn u(&self, now: Duration) -> f32 {
        if self.play.effect == Effect::None || self.play.duration.is_zero() {
            return self.end_u();
        }
        let moving = now.saturating_sub(self.start.max(self.anchor));
        let step = moving.as_secs_f32() / self.play.duration.as_secs_f32();
        let u = match self.way {
            Way::In => self.anchor_u + step,
            Way::Out => self.anchor_u - step,
        };
        u.clamp(0.0, 1.0)
    }

    fn end_u(&self) -> f32 {
        match self.way {
            Way::In => 1.0,
            Way::Out => 0.0,
        }
    }

    /// Whether the play is still in its delay, holding its first frame.
    pub fn is_waiting(&self, now: Duration) -> bool {
        now < self.start
    }

    pub fn is_done(&self, now: Duration) -> bool {
        self.u(now) == self.end_u()
    }

    /// Turns the transition around at `now`, from exactly where it is. A
    /// transition still in its delay has not moved, so it simply ends there.
    pub fn reverse(&mut self, now: Duration) {
        self.anchor_u = self.u(now);
        self.anchor = now;
        self.start = now;
        self.way = match self.way {
            Way::In => Way::Out,
            Way::Out => Way::In,
        };
    }

    pub fn frame(&self, now: Duration) -> TransitionFrame {
        let u = self.u(now);
        // Bounce brings its own curve; every other effect shares smoothstep.
        let shown = match self.play.effect {
            Effect::Bounce => bounce_ease(u),
            _ => smoothstep(u),
        };
        frame(&self.play, &self.geometry, shown)
    }
}

/// How much of the space the gradient's soft edge spans.
const GRADIENT_FEATHER: f32 = 0.25;
/// Pixelate's largest block per step of detail, in pixels.
const PIXELATE_BLOCK_PER_DETAIL: f32 = 4.0;
/// Pixelate fades out over the last part of hiding (the first of showing).
const PIXELATE_FADE: f32 = 0.35;
/// CRT spends this much of its time drawing the line, and the rest opening it.
const CRT_LINE: f32 = 0.3;
/// The thickness of CRT's line, and the size of its dot, in pixels.
const CRT_LINE_THICKNESS: i32 = 2;

fn frame(play: &Play, geometry: &Geometry, shown: f32) -> TransitionFrame {
    let whole = TransitionFrame {
        shown,
        rect: geometry.rect,
        space: geometry.rect.size(),
        content_offset: Point::new(0, 0),
        opacity: 1.0,
        mask: Mask::None,
        content: Content::Whole,
        clipped: false,
    };
    let gone = 1.0 - shown;
    match play.effect {
        Effect::None => TransitionFrame {
            opacity: if shown > 0.0 { 1.0 } else { 0.0 },
            ..whole
        },
        Effect::Roll => {
            let rect = geometry.rolled.lerp(geometry.rect, shown);
            TransitionFrame {
                rect,
                content_offset: offset_of(geometry.rect, rect),
                ..whole
            }
        }
        Effect::Fade => TransitionFrame {
            opacity: shown,
            ..whole
        },
        Effect::Pixelate => {
            let largest = f32::from(play.detail) * PIXELATE_BLOCK_PER_DETAIL;
            TransitionFrame {
                content: Content::pixelated((1.0 + (largest - 1.0) * gone).round() as u32),
                opacity: (shown / PIXELATE_FADE).min(1.0),
                ..whole
            }
        }
        Effect::Gradient => TransitionFrame {
            mask: Mask::Gradient {
                side: play.side(geometry.rect, geometry.work_area),
                position: gone * (1.0 + GRADIENT_FEATHER),
                feather: GRADIENT_FEATHER,
            },
            ..whole
        },
        Effect::Dissolve => TransitionFrame {
            mask: Mask::Noise {
                threshold: gone,
                grain: play.detail,
            },
            ..whole
        },
        Effect::Burn => TransitionFrame {
            mask: Mask::Burn {
                front: burn::front(gone),
                detail: play.detail,
            },
            ..whole
        },
        Effect::Fly | Effect::Bounce => {
            let side = play.side(geometry.rect, geometry.work_area);
            let moved = fly_rect(geometry.rect, geometry.work_area, side, gone);
            let rect = moved.intersection(geometry.work_area);
            TransitionFrame {
                rect,
                content_offset: offset_of(moved, rect),
                clipped: rect != moved,
                ..whole
            }
        }
        Effect::Crt => crt(geometry.rect, shown, whole),
    }
}

fn crt(rect: Rect, shown: f32, whole: TransitionFrame) -> TransitionFrame {
    let center = rect.center();
    let (width, height, glow) = if shown < CRT_LINE {
        let along = shown / CRT_LINE;
        let width = lerp(CRT_LINE_THICKNESS as f32, rect.width as f32, along);
        (width, CRT_LINE_THICKNESS as f32, 1.0)
    } else {
        let along = (shown - CRT_LINE) / (1.0 - CRT_LINE);
        let height = lerp(CRT_LINE_THICKNESS as f32, rect.height as f32, along);
        (rect.width as f32, height, 1.0 - along)
    };
    let (width, height) = if shown == 0.0 {
        (0, 0)
    } else {
        (width.round() as i32, height.round() as i32)
    };
    let opened = Rect::new(center.x - width / 2, center.y - height / 2, width, height);
    TransitionFrame {
        rect: opened,
        content_offset: offset_of(rect, opened),
        content: Content::lit(glow),
        ..whole
    }
}

/// `rect` moved `gone` of the way past `side` of `work_area`: at 1 it is
/// just outside.
fn fly_rect(rect: Rect, work_area: Rect, side: Side, gone: f32) -> Rect {
    let travel = |distance: i32| (distance as f32 * gone).round() as i32;
    match side {
        Side::Top => rect.offset(0, -travel(rect.bottom() - work_area.top())),
        Side::Bottom => rect.offset(0, travel(work_area.bottom() - rect.top())),
        Side::Left => rect.offset(-travel(rect.right() - work_area.left()), 0),
        Side::Right => rect.offset(travel(work_area.right() - rect.left()), 0),
    }
}

/// Robert Penner's easeOutBounce: a fall that lands and settles in three
/// shrinking bounces. Four parabolic arcs, as on
/// https://easings.net/#easeOutBounce.
fn bounce_ease(t: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;
    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

/// Where the space's content starts: the offset of `content` (the rect the
/// whole space would fill) from `window`'s top-left.
fn offset_of(content: Rect, window: Rect) -> Point {
    Point::new(content.x - window.x, content.y - window.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::animation::EFFECTS;

    /// Every effect, from the table, so a new one is tested without an edit
    /// here.
    fn every_effect() -> impl Iterator<Item = Effect> {
        EFFECTS.iter().map(|spec| spec.effect)
    }

    /// Every effect that moves through frames: `None` has none.
    fn every_animated_effect() -> impl Iterator<Item = Effect> {
        every_effect().filter(|&effect| effect != Effect::None)
    }

    /// Every edge. The match makes a new `Edge` a compile error here.
    fn every_edge() -> [Edge; 5] {
        let edges = [Edge::Auto, Edge::Top, Edge::Bottom, Edge::Left, Edge::Right];
        for edge in edges {
            match edge {
                Edge::Auto | Edge::Top | Edge::Bottom | Edge::Left | Edge::Right => {}
            }
        }
        edges
    }

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    fn play(effect: Effect, way: Way) -> Play {
        Play {
            effect,
            way,
            duration: ms(200),
            delay: Duration::ZERO,
            detail: 5,
            edge: Edge::Auto,
        }
    }

    fn work_area() -> Rect {
        Rect::new(0, 0, 1920, 1040)
    }

    /// A Roll's geometry from the top, for the tests that only need a rect.
    fn geometry() -> Geometry {
        geometry_for(&play(Effect::Roll, Way::In))
    }

    fn geometry_for(play: &Play) -> Geometry {
        Geometry::for_visibility(Rect::new(300, 100, 240, 160), work_area(), play)
    }

    fn start(effect: Effect, way: Way) -> Transition {
        start_from(effect, way, Edge::Auto)
    }

    fn start_from(effect: Effect, way: Way, edge: Edge) -> Transition {
        let play = Play {
            edge,
            ..play(effect, way)
        };
        Transition::new(play, geometry_for(&play), Duration::ZERO)
    }

    /// Whether `frame` draws the whole space as it is when fully there.
    fn is_whole(frame: &TransitionFrame) -> bool {
        frame.rect == geometry().rect
            && frame.content_offset == Point::new(0, 0)
            && frame.opacity == 1.0
            && frame.content == Content::Whole
            && frame.mask.hides_nothing()
    }

    /// The block size `frame` draws its contents in: 1 when not pixelated.
    fn block(frame: TransitionFrame) -> u32 {
        match frame.content {
            Content::Pixelated { block } => block,
            Content::Whole | Content::Lit { .. } => 1,
        }
    }

    #[test]
    fn whole_contents_have_one_form() {
        assert_eq!(Content::pixelated(1), Content::Whole);
        assert_eq!(Content::pixelated(0), Content::Whole);
        assert_eq!(Content::pixelated(4), Content::Pixelated { block: 4 });
        assert_eq!(Content::lit(0.0), Content::Whole);
        assert_eq!(Content::lit(0.5), Content::Lit { glow: 0.5 });
    }

    /// Whether `frame` draws nothing at all.
    fn is_gone(frame: &TransitionFrame) -> bool {
        frame.rect.is_empty()
            || frame.opacity == 0.0
            || match frame.mask {
                Mask::None => false,
                Mask::Gradient { position, .. } => position >= 1.0,
                Mask::Noise { threshold, .. } => threshold >= 1.0,
                Mask::Burn { front, .. } => front >= 1.0,
            }
    }

    #[test]
    fn every_effect_starts_and_ends_where_it_should() {
        for effect in every_effect() {
            for edge in every_edge() {
                let case = format!("{effect:?} from {edge:?}");
                let shown = start_from(effect, Way::In, edge);
                assert!(shown.is_done(ms(200)), "{case} In ends");
                assert!(is_whole(&shown.frame(ms(200))), "{case} In ends whole");
                let hidden = start_from(effect, Way::Out, edge);
                assert!(hidden.is_done(ms(200)), "{case} Out ends");
                assert!(is_gone(&hidden.frame(ms(200))), "{case} Out ends gone");
                if effect != Effect::None {
                    assert!(is_gone(&shown.frame(ms(0))), "{case} In starts gone");
                    assert!(is_whole(&hidden.frame(ms(0))), "{case} Out starts whole");
                }
            }
        }
    }

    #[test]
    fn only_effects_that_change_pixels_uncover_the_window_midway() {
        // None has no midway: its Out is gone at once.
        for effect in every_animated_effect() {
            let transition = start(effect, Way::Out);
            let midway = transition.frame(ms(100));
            // The effects that fade or mask pixels; the rest only move or
            // crop the window. A new effect fails here until it is sorted.
            let pixels = matches!(
                effect,
                Effect::Fade
                    | Effect::Pixelate
                    | Effect::Gradient
                    | Effect::Dissolve
                    | Effect::Burn
            );
            assert_eq!(!midway.covers_window(), pixels, "{effect:?}");
            assert!(
                transition.frame(ms(0)).covers_window(),
                "{effect:?} starts whole"
            );
        }
    }

    #[test]
    fn a_shadow_stays_whole_while_the_window_moves_and_fades_with_its_pixels() {
        for effect in every_animated_effect() {
            let transition = start(effect, Way::Out);
            assert_eq!(transition.frame(ms(0)).shadow_strength(), 1.0, "{effect:?}");
            let midway = transition.frame(ms(100));
            let strength = midway.shadow_strength();
            if midway.clipped {
                assert_eq!(strength, 0.0, "{effect:?} is crossing the edge");
            } else if midway.covers_window() {
                assert_eq!(strength, 1.0, "{effect:?} only moves the window");
            } else {
                assert!((0.0..1.0).contains(&strength), "{effect:?}: {strength}");
            }
        }
    }

    #[test]
    fn a_space_casts_no_shadow_while_the_work_area_clips_it() {
        for effect in [Effect::Fly, Effect::Bounce] {
            let transition = start(effect, Way::Out);
            let mut clipped = 0;
            for step in 0..=200 {
                let frame = transition.frame(ms(step));
                let inside = frame.rect == geometry().rect.offset(0, frame.rect.y - 100);
                assert_eq!(frame.clipped, !inside, "{effect:?} at {step}: {frame:?}");
                if frame.clipped {
                    clipped += 1;
                    assert_eq!(frame.shadow_strength(), 0.0, "{effect:?} at {step}");
                } else {
                    assert_eq!(frame.shadow_strength(), 1.0, "{effect:?} at {step}");
                }
            }
            assert!(clipped > 0, "{effect:?} crosses the edge");
        }
    }

    #[test]
    fn only_the_work_area_clips_a_frame() {
        for effect in every_effect() {
            if matches!(effect, Effect::Fly | Effect::Bounce) {
                continue;
            }
            for step in 0..=40 {
                let frame = start(effect, Way::Out).frame(ms(step * 5));
                assert!(!frame.clipped, "{effect:?} at {step}");
            }
        }
    }

    #[test]
    fn none_is_done_at_once() {
        assert!(start(Effect::None, Way::In).is_done(Duration::ZERO));
        let out = start(Effect::None, Way::Out);
        assert!(out.is_done(Duration::ZERO));
        assert!(is_gone(&out.frame(Duration::ZERO)));
    }

    #[test]
    fn showing_only_ever_reveals_more_except_bounce() {
        // Bounce lands and rebounds, so its progress and its rect retreat
        // during a bounce. Its opacity and block size still only grow, as
        // every other effect's do.
        for effect in every_effect() {
            for edge in every_edge() {
                let case = format!("{effect:?} from {edge:?}");
                let transition = start_from(effect, Way::In, edge);
                let mut last = transition.frame(ms(0));
                for step in 1..=40 {
                    let frame = transition.frame(ms(step * 5));
                    assert!(frame.opacity >= last.opacity, "{case} at {step}");
                    assert!(block(frame) <= block(last), "{case} at {step}");
                    if effect != Effect::Bounce {
                        assert!(frame.shown >= last.shown, "{case} at {step}");
                        assert!(
                            frame.rect.width * frame.rect.height
                                >= last.rect.width * last.rect.height
                                || effect == Effect::Crt,
                            "{case} at {step}"
                        );
                    }
                    last = frame;
                }
            }
        }
    }

    #[test]
    fn reversing_at_any_moment_continues_from_the_same_frame() {
        // None has no frames to continue: reversing it is instant.
        for effect in every_animated_effect() {
            for edge in every_edge() {
                for way in [Way::In, Way::Out] {
                    for at in [0, 1, 37, 72, 73, 100, 150, 199, 200, 260] {
                        let case = format!("{effect:?} from {edge:?} {way:?} at {at}");
                        let mut transition = start_from(effect, way, edge);
                        let before = transition.frame(ms(at));
                        transition.reverse(ms(at));
                        assert_eq!(transition.frame(ms(at)), before, "{case}");
                        // Bounce is steepest just before its first landing,
                        // so give it a finer step to hold the same bound.
                        let step = if effect == Effect::Bounce { 1 } else { 5 };
                        let next = transition.frame(ms(at + step));
                        let shift = (next.shown - before.shown).abs();
                        assert!(shift < 0.1, "{case} jumps by {shift}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_reversed_show_goes_back_the_way_it_came() {
        let mut transition = start(Effect::Fade, Way::In);
        transition.reverse(ms(100));
        assert_eq!(transition.way(), Way::Out);
        // Back where it started after as long again as it had run.
        assert!(transition.is_done(ms(200)));
        assert_eq!(transition.frame(ms(200)).opacity, 0.0);
    }

    #[test]
    fn a_delay_holds_the_first_frame() {
        let mut delayed = play(Effect::Fade, Way::In);
        delayed.delay = ms(100);
        let transition = Transition::new(delayed, geometry(), Duration::ZERO);
        assert_eq!(transition.frame(ms(90)).opacity, 0.0);
        assert!(transition.is_waiting(ms(90)));
        assert!(!transition.is_waiting(ms(100)));
        assert!(!transition.is_done(ms(250)));
        assert!(transition.is_done(ms(300)));
        // Reversed during its delay, a show has not moved, so it is done.
        let mut early = Transition::new(delayed, geometry(), Duration::ZERO);
        early.reverse(ms(50));
        assert!(early.is_done(ms(50)));
    }

    #[test]
    fn roll_keeps_its_edge_and_its_content_in_place() {
        let rect = geometry().rect;
        for edge in every_edge() {
            let frame = start_from(Effect::Roll, Way::Out, edge).frame(ms(100));
            let kept = match edge {
                Edge::Auto | Edge::Top => frame.rect.top() == rect.top(),
                Edge::Bottom => frame.rect.bottom() == rect.bottom(),
                Edge::Left => frame.rect.left() == rect.left(),
                Edge::Right => frame.rect.right() == rect.right(),
            };
            assert!(kept, "rolls into {edge:?}: {:?}", frame.rect);
            assert!(frame.rect.width * frame.rect.height < rect.width * rect.height);
            // The content stays where it was while the window crops it.
            assert_eq!(
                frame.content_offset,
                Point::new(
                    rect.left() - frame.rect.left(),
                    rect.top() - frame.rect.top()
                ),
                "{edge:?}"
            );
        }
    }

    #[test]
    fn a_fold_stops_at_the_title_bar() {
        let unfolded = Rect::new(300, 100, 240, 160);
        let title = Rect::new(300, 100, 240, 28);
        let geometry = Geometry::for_fold(unfolded, title, work_area());
        let fold = Transition::new(play(Effect::Roll, Way::Out), geometry, Duration::ZERO);
        assert_eq!(fold.frame(ms(200)).rect, title);
        assert!(!is_gone(&fold.frame(ms(200))));
    }

    #[test]
    fn fly_leaves_by_the_nearest_edge_and_never_leaves_the_work_area() {
        let near_right = Rect::new(1600, 400, 240, 160);
        let fly = play(Effect::Fly, Way::Out);
        let geometry = Geometry::for_visibility(near_right, work_area(), &fly);
        let transition = Transition::new(fly, geometry, Duration::ZERO);
        for step in 0..=40 {
            let frame = transition.frame(ms(step * 5));
            let area = work_area();
            assert!(frame.rect.left() >= area.left() && frame.rect.right() <= area.right());
            assert!(frame.rect.top() >= area.top() && frame.rect.bottom() <= area.bottom());
            if !frame.rect.is_empty() {
                assert_eq!(frame.rect.top(), near_right.top(), "moves sideways only");
                // The content moves with the space, clipped at the edge.
                assert_eq!(frame.content_offset.x, 0);
            }
        }
        assert!(transition.frame(ms(100)).rect.left() > near_right.left());
    }

    #[test]
    fn automatic_takes_the_side_the_effect_row_names() {
        let near_right = Rect::new(1600, 400, 240, 160);
        let side = |effect| play(effect, Way::Out).side(near_right, work_area());
        assert_eq!(side(Effect::Fly), Side::Right, "the nearest side");
        assert_eq!(side(Effect::Roll), Side::Top);
        assert_eq!(side(Effect::Gradient), Side::Top);
        assert_eq!(side(Effect::Bounce), Side::Top);
        let left = Play {
            edge: Edge::Left,
            ..play(Effect::Fly, Way::Out)
        };
        assert_eq!(
            left.side(near_right, work_area()),
            Side::Left,
            "a set edge wins"
        );
    }

    #[test]
    fn fly_never_leaves_the_work_area_from_any_edge() {
        let area = work_area();
        for edge in every_edge() {
            let transition = start_from(Effect::Fly, Way::Out, edge);
            for step in 0..=40 {
                let rect = transition.frame(ms(step * 5)).rect;
                if rect.is_empty() {
                    continue;
                }
                assert_eq!(
                    rect.intersection(area),
                    rect,
                    "{edge:?} at {step}: {rect:?}"
                );
            }
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
        assert!(bounce_ease(0.5) < 1.0, "rebounds after the first landing");
        // The branch boundaries are the landings; the task asks for three.
        let landings = [1.0 / 2.75, 2.0 / 2.75, 2.5 / 2.75, 1.0];
        let touched = landings
            .iter()
            .filter(|&&t| (bounce_ease(t) - 1.0).abs() < 1e-5)
            .count();
        assert!(touched >= 3, "touched 1 only {touched} times");
    }

    #[test]
    fn bounce_drops_in_and_settles_without_leaving_the_work_area() {
        let whole = geometry().rect;
        let area = work_area();
        for edge in every_edge() {
            let transition = start_from(Effect::Bounce, Way::In, edge);
            // A frame carries only the part inside the work area, so gone
            // means empty here.
            assert!(is_gone(&transition.frame(ms(0))), "{edge:?} starts outside");
            for step in 0..=200 {
                let frame = transition.frame(ms(step));
                if frame.rect.is_empty() {
                    continue;
                }
                assert_eq!(
                    frame.rect.intersection(area),
                    frame.rect,
                    "{edge:?} at {step}: {:?}",
                    frame.rect
                );
                match edge {
                    Edge::Auto | Edge::Top | Edge::Bottom => {
                        assert_eq!(frame.rect.x, whole.x, "{edge:?} keeps x");
                    }
                    Edge::Left | Edge::Right => {
                        assert_eq!(frame.rect.y, whole.y, "{edge:?} keeps y");
                    }
                }
            }
            assert_eq!(transition.frame(ms(200)).rect, whole, "{edge:?} settles");
        }
    }

    #[test]
    fn bounce_lands_then_rebounds_before_settling() {
        let whole = geometry().rect;
        let transition = start(Effect::Bounce, Way::In);
        let landed = (0..200)
            .find(|&at| transition.frame(ms(at)).rect == whole)
            .expect("lands");
        let rebound = (landed..200).any(|at| transition.frame(ms(at)).rect != whole);
        assert!(rebound, "moves off again after landing");
        assert_eq!(transition.frame(ms(200)).rect, whole);
    }

    #[test]
    fn crt_draws_a_line_then_opens_it() {
        let rect = geometry().rect;
        let transition = start(Effect::Crt, Way::In);
        // About 20 % in, eased: still a line, partly across.
        let line = transition.frame(ms(55));
        assert_eq!(line.rect.height, CRT_LINE_THICKNESS);
        assert!(line.rect.width < rect.width);
        assert_eq!(line.content, Content::Lit { glow: 1.0 });
        let opening = transition.frame(ms(140));
        assert_eq!(opening.rect.width, rect.width);
        assert!(opening.rect.height > CRT_LINE_THICKNESS && opening.rect.height < rect.height);
        assert!(matches!(opening.content, Content::Lit { glow } if glow < 1.0));
        assert_eq!(opening.rect.center(), rect.center());
    }
}
