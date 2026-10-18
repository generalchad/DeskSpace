//! The frame math of an animation (ROADMAP D12): what a space window looks
//! like at any moment of a `Play`. Pure, with no Win32, so every effect is
//! tested here and the window side only applies frames.
//!
//! A transition tracks one linear value, `u`, from 0 (gone) to 1 (fully
//! there), and a direction. What is drawn is a function of `ease(u)` alone,
//! with one symmetric curve for both directions, so an Out is the In played
//! backwards and reversing at any moment just turns the direction around:
//! the next frame equals the current one.

use std::time::Duration;

use deskspace_core::animation::{DETAIL_RANGE, Edge, Effect};
use deskspace_core::geometry::{Point, Rect, Size};

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
}

/// Where a window is and what it may cover, in desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// The window's rect when fully there.
    pub rect: Rect,
    /// The rect Roll collapses to: nothing at an edge when hiding, the title
    /// bar when folding.
    pub rolled: Rect,
    /// The work area of the window's monitor; Fly never leaves it.
    pub work_area: Rect,
}

impl Geometry {
    /// A window that hides and shows (or comes and goes): Roll collapses it
    /// into `edge` (`Auto` is the top), and Fly uses `work_area`.
    pub fn for_visibility(rect: Rect, work_area: Rect, edge: Edge) -> Self {
        let rolled = match edge {
            Edge::Auto | Edge::Top => Rect::new(rect.x, rect.y, rect.width, 0),
            Edge::Bottom => Rect::new(rect.x, rect.bottom(), rect.width, 0),
            Edge::Left => Rect::new(rect.x, rect.y, 0, rect.height),
            Edge::Right => Rect::new(rect.right(), rect.y, 0, rect.height),
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
    /// A soft edge sweeping in from `edge`. Along the axis from that edge
    /// (0 at the edge, 1 at the far side), a pixel is hidden before
    /// `position - feather`, fades in over `feather`, and shows from
    /// `position` on.
    Gradient {
        edge: Edge,
        position: f32,
        feather: f32,
    },
    /// Grain-sized cells, each shown while its noise value (0 to 1) is at or
    /// above `threshold`. `glow` is the width of the band above the threshold
    /// drawn as embers (Burn); 0 for none.
    Noise {
        threshold: f32,
        grain: u8,
        glow: f32,
    },
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
    pub mask: Mask,
    /// The pixel block size: 1 draws normally (Pixelate).
    pub block: u32,
    /// How strongly a CRT's bright line glows over the contents, 0 to 1.
    pub glow: f32,
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

    pub fn way(&self) -> Way {
        self.way
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
        frame(&self.play, &self.geometry, ease(self.u(now)))
    }
}

/// Smoothstep: slow at both ends and symmetric, so playing it backwards is
/// the same curve.
fn ease(u: f32) -> f32 {
    u * u * (3.0 - 2.0 * u)
}

/// How much of the space the gradient's soft edge spans.
const GRADIENT_FEATHER: f32 = 0.25;
/// Pixelate's largest block per step of detail, in pixels.
const PIXELATE_BLOCK_PER_DETAIL: f32 = 4.0;
/// Pixelate fades out over the last part of hiding (the first of showing).
const PIXELATE_FADE: f32 = 0.35;
/// Burn's ember band per step of detail, as a fraction of the noise range.
const BURN_GLOW_PER_DETAIL: f32 = 0.012;
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
        block: 1,
        glow: 0.0,
    };
    let gone = 1.0 - shown;
    match play.effect {
        Effect::None => TransitionFrame {
            opacity: if shown > 0.0 { 1.0 } else { 0.0 },
            ..whole
        },
        Effect::Roll => {
            let rect = lerp_rect(geometry.rolled, geometry.rect, shown);
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
                block: (1.0 + (largest - 1.0) * gone).round() as u32,
                opacity: (shown / PIXELATE_FADE).min(1.0),
                ..whole
            }
        }
        Effect::Gradient => TransitionFrame {
            mask: Mask::Gradient {
                edge: resolve_sweep(play.edge),
                position: gone * (1.0 + GRADIENT_FEATHER),
                feather: GRADIENT_FEATHER,
            },
            ..whole
        },
        Effect::Dissolve => TransitionFrame {
            mask: noise(play, gone, 0.0),
            ..whole
        },
        Effect::Burn => TransitionFrame {
            mask: noise(play, gone, f32::from(play.detail) * BURN_GLOW_PER_DETAIL),
            ..whole
        },
        Effect::Fly => {
            let edge = resolve_fly(play.edge, geometry.rect, geometry.work_area);
            let moved = fly_rect(geometry.rect, geometry.work_area, edge, gone);
            let rect = intersection(moved, geometry.work_area);
            TransitionFrame {
                rect,
                content_offset: offset_of(moved, rect),
                ..whole
            }
        }
        Effect::Crt => crt(geometry.rect, shown, whole),
    }
}

/// A noise mask that hides everything at `gone == 1`. The threshold runs
/// slightly past 1 with a glow, so the last embers burn out too.
fn noise(play: &Play, gone: f32, glow: f32) -> Mask {
    Mask::Noise {
        threshold: gone * (1.0 + glow),
        grain: play.detail,
        glow,
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
        glow,
        ..whole
    }
}

fn resolve_sweep(edge: Edge) -> Edge {
    match edge {
        Edge::Auto => Edge::Top,
        edge => edge,
    }
}

/// `Auto` flies to the work area's edge nearest the space.
fn resolve_fly(edge: Edge, rect: Rect, work_area: Rect) -> Edge {
    if edge != Edge::Auto {
        return edge;
    }
    [
        (rect.top() - work_area.top(), Edge::Top),
        (work_area.bottom() - rect.bottom(), Edge::Bottom),
        (rect.left() - work_area.left(), Edge::Left),
        (work_area.right() - rect.right(), Edge::Right),
    ]
    .into_iter()
    .min_by_key(|(distance, _)| *distance)
    .map_or(Edge::Top, |(_, edge)| edge)
}

/// `rect` moved `gone` of the way past `edge` of `work_area`: at 1 it is
/// just outside.
fn fly_rect(rect: Rect, work_area: Rect, edge: Edge, gone: f32) -> Rect {
    let travel = |distance: i32| (distance as f32 * gone).round() as i32;
    match edge {
        Edge::Auto | Edge::Top => rect.offset(0, -travel(rect.bottom() - work_area.top())),
        Edge::Bottom => rect.offset(0, travel(work_area.bottom() - rect.top())),
        Edge::Left => rect.offset(-travel(rect.right() - work_area.left()), 0),
        Edge::Right => rect.offset(travel(work_area.right() - rect.left()), 0),
    }
}

/// Where the space's content starts: the offset of `content` (the rect the
/// whole space would fill) from `window`'s top-left.
fn offset_of(content: Rect, window: Rect) -> Point {
    Point::new(content.x - window.x, content.y - window.y)
}

fn intersection(a: Rect, b: Rect) -> Rect {
    let left = a.left().max(b.left());
    let top = a.top().max(b.top());
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    if right <= left || bottom <= top {
        return Rect::new(left, top, 0, 0);
    }
    Rect::new(left, top, right - left, bottom - top)
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn lerp_rect(from: Rect, to: Rect, t: f32) -> Rect {
    let at = |a: i32, b: i32| lerp(a as f32, b as f32, t).round() as i32;
    let left = at(from.left(), to.left());
    let top = at(from.top(), to.top());
    let right = at(from.right(), to.right());
    let bottom = at(from.bottom(), to.bottom());
    Rect::new(left, top, right - left, bottom - top)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_EFFECT: [Effect; 9] = [
        Effect::None,
        Effect::Roll,
        Effect::Fade,
        Effect::Pixelate,
        Effect::Gradient,
        Effect::Dissolve,
        Effect::Fly,
        Effect::Crt,
        Effect::Burn,
    ];

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

    fn geometry() -> Geometry {
        Geometry::for_visibility(Rect::new(300, 100, 240, 160), work_area(), Edge::Auto)
    }

    fn start(effect: Effect, way: Way) -> Transition {
        Transition::new(play(effect, way), geometry(), Duration::ZERO)
    }

    /// Whether `frame` draws the whole space as it is when fully there.
    fn is_whole(frame: &TransitionFrame) -> bool {
        frame.rect == geometry().rect
            && frame.content_offset == Point::new(0, 0)
            && frame.opacity == 1.0
            && frame.block == 1
            && frame.glow == 0.0
            && match frame.mask {
                Mask::None => true,
                Mask::Gradient {
                    position, feather, ..
                } => position - feather <= 0.0,
                Mask::Noise { threshold, .. } => threshold <= 0.0,
            }
    }

    /// Whether `frame` draws nothing at all.
    fn is_gone(frame: &TransitionFrame) -> bool {
        frame.rect.is_empty()
            || frame.opacity == 0.0
            || match frame.mask {
                Mask::None => false,
                Mask::Gradient { position, .. } => position >= 1.0,
                Mask::Noise {
                    threshold, glow, ..
                } => threshold >= 1.0 + glow,
            }
    }

    #[test]
    fn every_effect_starts_and_ends_where_it_should() {
        for effect in EVERY_EFFECT {
            let shown = start(effect, Way::In);
            assert!(shown.is_done(ms(200)), "{effect:?} In ends");
            assert!(is_whole(&shown.frame(ms(200))), "{effect:?} In ends whole");
            let hidden = start(effect, Way::Out);
            assert!(hidden.is_done(ms(200)), "{effect:?} Out ends");
            assert!(is_gone(&hidden.frame(ms(200))), "{effect:?} Out ends gone");
            if effect != Effect::None {
                assert!(is_gone(&shown.frame(ms(0))), "{effect:?} In starts gone");
                assert!(
                    is_whole(&hidden.frame(ms(0))),
                    "{effect:?} Out starts whole"
                );
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
    fn showing_only_ever_reveals_more() {
        for effect in EVERY_EFFECT {
            let transition = start(effect, Way::In);
            let mut last = transition.frame(ms(0));
            for step in 1..=40 {
                let frame = transition.frame(ms(step * 5));
                assert!(frame.shown >= last.shown, "{effect:?} at {step}");
                assert!(frame.opacity >= last.opacity, "{effect:?} at {step}");
                assert!(frame.block <= last.block, "{effect:?} at {step}");
                assert!(
                    frame.rect.width * frame.rect.height >= last.rect.width * last.rect.height
                        || effect == Effect::Crt,
                    "{effect:?} at {step}"
                );
                last = frame;
            }
        }
    }

    #[test]
    fn reversing_at_any_moment_continues_from_the_same_frame() {
        // None has no frames to continue: reversing it is instant.
        for effect in EVERY_EFFECT
            .into_iter()
            .filter(|&effect| effect != Effect::None)
        {
            for way in [Way::In, Way::Out] {
                for at in [0, 1, 37, 100, 150, 199, 200, 260] {
                    let mut transition = start(effect, way);
                    let before = transition.frame(ms(at));
                    transition.reverse(ms(at));
                    assert_eq!(
                        transition.frame(ms(at)),
                        before,
                        "{effect:?} {way:?} at {at}"
                    );
                    let next = transition.frame(ms(at + 5));
                    let shift = (next.shown - before.shown).abs();
                    assert!(shift < 0.1, "{effect:?} {way:?} at {at} jumps by {shift}");
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
        let transition = start(Effect::Roll, Way::Out);
        let frame = transition.frame(ms(100));
        assert_eq!(frame.rect.top(), rect.top(), "rolls into the top");
        assert!(frame.rect.height < rect.height);
        assert_eq!(frame.content_offset, Point::new(0, 0));

        let bottom = Geometry::for_visibility(rect, work_area(), Edge::Bottom);
        let transition = Transition::new(play(Effect::Roll, Way::Out), bottom, Duration::ZERO);
        let frame = transition.frame(ms(100));
        assert_eq!(frame.rect.bottom(), rect.bottom(), "rolls into the bottom");
        assert_eq!(frame.content_offset.y, rect.top() - frame.rect.top());
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
        let geometry = Geometry::for_visibility(near_right, work_area(), Edge::Auto);
        let transition = Transition::new(play(Effect::Fly, Way::Out), geometry, Duration::ZERO);
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
    fn crt_draws_a_line_then_opens_it() {
        let rect = geometry().rect;
        let transition = start(Effect::Crt, Way::In);
        // About 20 % in, eased: still a line, partly across.
        let line = transition.frame(ms(55));
        assert_eq!(line.rect.height, CRT_LINE_THICKNESS);
        assert!(line.rect.width < rect.width);
        assert_eq!(line.glow, 1.0);
        let opening = transition.frame(ms(140));
        assert_eq!(opening.rect.width, rect.width);
        assert!(opening.rect.height > CRT_LINE_THICKNESS && opening.rect.height < rect.height);
        assert!(opening.glow > 0.0 && opening.glow < 1.0);
        assert_eq!(opening.rect.center(), rect.center());
    }

    #[test]
    fn rects_intersect_or_come_out_empty() {
        let a = Rect::new(0, 0, 100, 100);
        assert_eq!(
            intersection(a, Rect::new(50, 60, 100, 100)),
            Rect::new(50, 60, 50, 40)
        );
        assert!(intersection(a, Rect::new(200, 0, 10, 10)).is_empty());
    }
}
