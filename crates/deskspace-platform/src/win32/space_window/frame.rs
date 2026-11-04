//! A space's frame: DWM's corners, the painted border, its window region, and
//! the shadow it casts.

use deskspace_core::theme::Backdrop;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

use crate::render::shadow::ShadowLook;
use crate::render::{BorderStroke, PaintFrame, ShadowStyle, SpaceStyle};
use crate::win32::dpi::scale_for;
use crate::win32::dwm;

use super::with_state;

/// Who draws a space's corners, and how its window is clipped. The painter
/// always draws the border (`dwm::BorderOwner::Painter`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Frame {
    /// Desktop-hosted windows get no per-pixel alpha; paint them opaque.
    opaque: bool,
    /// The radius the painter rounds the body to; 0 where DWM rounds the
    /// window or the corners are square.
    radius: i32,
    /// The border's thickness at 96 DPI, 0 for none.
    border_width: i32,
    /// The radius DWM rounds the window to, at 96 DPI: 0 where it does not
    /// round (its own square corners, or Windows 10, which has none). DWM
    /// scales it with the window's DPI, so the border follows it there.
    dwm_radius: i32,
    /// Clip the window to a rounded region of this radius. Needed for opaque
    /// desktop-hosted windows, whose pixels outside the rounded corners
    /// would otherwise show. Regions are not anti-aliased, so the painter
    /// still draws smooth corners inside them. Accent-blurred top-level
    /// spaces use DWM's corners instead, because the blur material ignores
    /// regions (`Theme::effective_corner_radius`); composition spaces clip
    /// themselves in their visual tree (`dwm::Material::dwm_rounds`).
    region_radius: Option<i32>,
    /// The shadow the window draws itself (ROADMAP D15): the style's, unless
    /// DWM casts the Windows 11 shadow with its corners, or the space has
    /// no edge to cast from (Clear, chrome-less) or is desktop-hosted.
    shadow: Option<ShadowStyle>,
}

impl Frame {
    /// `material` is what draws the window's blur, `None` for a
    /// desktop-hosted window, which has none.
    pub(super) fn for_style(
        style: SpaceStyle,
        material: Option<dwm::Material>,
        windows_11: bool,
    ) -> Self {
        let hosted = material.is_none();
        // A chrome-less window (the desktop layer) has no rounded corners or
        // border; it fills its whole rectangle.
        let radius = if style.chrome {
            style.corner_radius.max(0)
        } else {
            0
        };
        let border_width = if style.chrome && style.border {
            style.border_width.max(1)
        } else {
            0
        };
        let shadow = style
            .shadow
            .filter(|_| style.chrome && style.backdrop != Backdrop::Clear && !hosted);
        if material.is_some_and(dwm::Material::dwm_rounds) && style.dwm_draws_frame() {
            // `dwm_draws_frame` allows only radii DWM has (0, 4, 8).
            let dwm_radius = if windows_11 { radius } else { 0 };
            return Self {
                opaque: false,
                radius: 0,
                border_width,
                dwm_radius,
                region_radius: None,
                // DWM's rounded corners bring its own shadow.
                shadow: shadow.filter(|_| dwm_radius == 0),
            };
        }
        Self {
            opaque: hosted,
            radius,
            border_width,
            dwm_radius: 0,
            region_radius: (radius > 0 && hosted).then_some(radius),
            shadow,
        }
    }

    /// What the painter draws, for a window at `dpi`.
    pub(super) fn paint(&self, dpi: u32) -> PaintFrame {
        let scale = scale_for(dpi);
        // Whole pixels keep the line crisp at any scale.
        let width = (self.border_width as f32 * scale).round().max(1.0);
        PaintFrame {
            opaque: self.opaque,
            radius: self.radius,
            outline_radius: self.outline_radius(scale),
            border: (self.border_width > 0).then_some(BorderStroke { width }),
        }
    }

    /// The shadow the window casts at `dpi`, and its opacity in percent.
    pub(super) fn shadow(&self, dpi: u32) -> Option<(ShadowLook, u8)> {
        let shadow = self.shadow?;
        let scale = scale_for(dpi);
        let look = ShadowLook {
            reach: ((shadow.size as f32 * scale).round() as i32).max(1),
            radius: self.outline_radius(scale),
        };
        Some((look, shadow.opacity))
    }

    /// The radius of the window's visible corners at `scale`: DWM's, which
    /// it scales with the window's DPI, or the one the space rounds itself
    /// to.
    fn outline_radius(&self, scale: f32) -> f32 {
        if self.dwm_radius > 0 {
            self.dwm_radius as f32 * scale
        } else {
            self.radius as f32
        }
    }
}

/// Applies the frame's rounded window region, or removes it.
pub(super) unsafe fn update_region(hwnd: HWND) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let Some(radius) = (unsafe { with_state(hwnd, |state| state.frame.region_radius) }) else {
        return;
    };
    let mut client = Default::default();
    // SAFETY: `hwnd` is live; the system owns the region after SetWindowRgn.
    unsafe {
        let Some(radius) = radius else {
            let _ = SetWindowRgn(hwnd, None, true);
            return;
        };
        if GetClientRect(hwnd, &mut client).is_err() {
            return;
        }
        // CreateRoundRectRgn excludes the right and bottom edges, hence +1.
        let region = CreateRoundRectRgn(
            0,
            0,
            client.right + 1,
            client.bottom + 1,
            radius * 2,
            radius * 2,
        );
        let _ = SetWindowRgn(hwnd, Some(region), true);
    }
}

/// A shadow's whole opacity, 0 to 255: its setting, in percent, times how
/// strongly the frame casts it.
pub(super) fn shadow_alpha(opacity: u8, strength: f32) -> u8 {
    (f32::from(opacity.min(100)) / 100.0 * strength.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(backdrop: deskspace_core::Backdrop, corner_radius: i32) -> SpaceStyle {
        SpaceStyle {
            backdrop,
            corner_radius,
            dark: false,
            border: true,
            border_width: 1,
            chrome: true,
            shadow: Some(SHADOW),
        }
    }

    /// The visible corners' radius and the border's width, if one is drawn.
    fn edge(paint: &PaintFrame) -> (f32, Option<f32>) {
        (
            paint.outline_radius,
            paint.border.map(|border| border.width),
        )
    }

    const ACCENT: Option<dwm::Material> = Some(dwm::Material::Accent);
    const COMPOSED: Option<dwm::Material> = Some(dwm::Material::Composition);
    const SHADOW: ShadowStyle = ShadowStyle {
        size: 16,
        opacity: 35,
    };

    #[test]
    fn a_composition_space_casts_its_own_shadow_at_its_corners() {
        let frame = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), COMPOSED, true);
        let (look, opacity) = frame.shadow(144).expect("composition casts no DWM shadow");
        assert_eq!(opacity, 35);
        assert_eq!(look.reach, 24, "the reach scales with the DPI");
        assert_eq!(look.radius, 8.0, "the radius the window clips itself to");
    }

    #[test]
    fn where_dwm_rounds_the_corners_its_own_shadow_stands() {
        let rounded = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), ACCENT, true);
        assert_eq!(rounded.shadow(96), None);
        // Square corners and Windows 10 bring no DWM shadow.
        let square = Frame::for_style(style(deskspace_core::Backdrop::Solid, 0), ACCENT, true);
        assert!(square.shadow(96).is_some());
        let ten = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), ACCENT, false);
        assert_eq!(ten.shadow(96).map(|(look, _)| look.radius), Some(0.0));
        // A radius DWM cannot draw: the space rounds itself, and casts.
        let painted = Frame::for_style(style(deskspace_core::Backdrop::Solid, 12), ACCENT, true);
        assert_eq!(painted.shadow(96).map(|(look, _)| look.radius), Some(12.0));
    }

    #[test]
    fn no_shadow_without_the_setting_an_edge_or_a_top_level_window() {
        let off = SpaceStyle {
            shadow: None,
            ..style(deskspace_core::Backdrop::Acrylic, 8)
        };
        assert_eq!(Frame::for_style(off, COMPOSED, true).shadow(96), None);
        let clear = style(deskspace_core::Backdrop::Clear, 8);
        assert_eq!(Frame::for_style(clear, COMPOSED, true).shadow(96), None);
        let layer = SpaceStyle {
            chrome: false,
            ..style(deskspace_core::Backdrop::Acrylic, 0)
        };
        assert_eq!(Frame::for_style(layer, COMPOSED, true).shadow(96), None);
        let hosted = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), None, true);
        assert_eq!(hosted.shadow(96), None);
    }

    #[test]
    fn a_shadow_fades_with_its_setting_and_its_frame() {
        assert_eq!(shadow_alpha(100, 1.0), 255);
        assert_eq!(shadow_alpha(35, 1.0), 89);
        assert_eq!(shadow_alpha(100, 0.5), 128);
        assert_eq!(shadow_alpha(35, 0.0), 0);
        assert_eq!(shadow_alpha(250, 2.0), 255, "both clamped");
    }

    #[test]
    fn a_composition_space_rounds_itself_where_dwm_would_have() {
        let composed = Some(dwm::Material::Composition);
        let frame = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), composed, true);
        let paint = frame.paint(144);
        assert!(!paint.opaque);
        assert_eq!((paint.radius, edge(&paint)), (8, (8.0, Some(2.0))));
        assert_eq!(frame.region_radius, None);
    }

    #[test]
    fn the_border_thickness_scales_to_whole_pixels() {
        let thick = SpaceStyle {
            border_width: 3,
            ..style(deskspace_core::Backdrop::Clear, 0)
        };
        let frame = Frame::for_style(thick, ACCENT, true);
        assert_eq!(frame.paint(96).border.unwrap().width, 3.0);
        assert_eq!(frame.paint(120).border.unwrap().width, 4.0);
        assert_eq!(frame.paint(144).border.unwrap().width, 5.0);
    }

    #[test]
    fn where_dwm_rounds_the_border_follows_its_corners_at_the_windows_dpi() {
        let frame = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), ACCENT, true);
        assert_eq!(frame.paint(96).radius, 0);
        assert_eq!(edge(&frame.paint(96)), (8.0, Some(1.0)));
        assert_eq!(edge(&frame.paint(144)), (12.0, Some(2.0)));
        let small = Frame::for_style(style(deskspace_core::Backdrop::Blur, 4), ACCENT, true);
        assert_eq!(edge(&small.paint(120)), (5.0, Some(1.0)));
    }

    #[test]
    fn windows_10_squares_the_painted_border_as_it_squares_the_window() {
        let frame = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), ACCENT, false);
        assert_eq!(edge(&frame.paint(144)), (0.0, Some(2.0)));
    }

    #[test]
    fn a_painter_rounded_space_borders_at_its_own_radius() {
        let clear = Frame::for_style(style(deskspace_core::Backdrop::Clear, 12), ACCENT, true);
        let paint = clear.paint(144);
        assert_eq!((paint.radius, edge(&paint)), (12, (12.0, Some(2.0))));
        let hosted = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), None, true);
        let paint = hosted.paint(96);
        assert!(paint.opaque);
        assert_eq!((paint.radius, edge(&paint)), (8, (8.0, Some(1.0))));
        assert_eq!(hosted.region_radius, Some(8));
    }

    #[test]
    fn no_border_without_the_setting_or_chrome() {
        let off = SpaceStyle {
            border: false,
            ..style(deskspace_core::Backdrop::Acrylic, 8)
        };
        assert_eq!(
            edge(&Frame::for_style(off, ACCENT, true).paint(96)),
            (8.0, None),
            "the highlight still follows DWM's corners"
        );
        let layer = SpaceStyle {
            chrome: false,
            ..style(deskspace_core::Backdrop::Clear, 0)
        };
        assert_eq!(Frame::for_style(layer, ACCENT, true).paint(96).border, None);
    }
}
