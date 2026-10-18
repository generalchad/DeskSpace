//! Paints the zoom pop-up (`render::zoom_bar`): a translucent rounded bar
//! with zoom out, the percentage, zoom in, a divider, and Reset.

use super::*;
use crate::input::ZoomAction;
use crate::render::zoom_bar::{ZoomBar, ZoomBarLayout};

/// Segoe Fluent Icons / Segoe MDL2 Assets "Remove" and "Add".
/// https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
const ZOOM_OUT_GLYPH: &[u16] = &[0xE738];
const ZOOM_IN_GLYPH: &[u16] = &[0xE710];
/// Used instead when neither icon font is installed: a minus sign and a plus.
const ZOOM_OUT_TEXT: &[u16] = &[0x2212];
const ZOOM_IN_TEXT: &[u16] = &[0x002B];
const RESET_TEXT: &[u16] = &[0x52, 0x65, 0x73, 0x65, 0x74];
/// Opacity of a disabled button's glyph or text.
const DISABLED_ALPHA: u8 = 90;
const DIVIDER_ALPHA: u8 = 60;
/// How far the divider stops short of the top and bottom edges.
const DIVIDER_INSET: f32 = 10.0;
const HOVER_RADIUS: f32 = 4.0;

#[derive(Default)]
pub(crate) struct ZoomBarPainter {
    canvas: HwndCanvas,
}

impl ZoomBarPainter {
    /// Paints `bar` in response to `WM_PAINT`. `label` is `bar.label()` as
    /// UTF-16, `hovered` the button under the pointer, and `scale` the
    /// monitor's DPI factor (1.0 at 96 DPI).
    pub(crate) fn paint(
        &mut self,
        hwnd: HWND,
        bar: &ZoomBar,
        label: &[u16],
        hovered: Option<ZoomAction>,
        scale: f32,
    ) -> Result<(), PlatformError> {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: BeginPaint/EndPaint bracket the paint for this window; the
        // render target draws to the same HWND in between.
        unsafe { BeginPaint(hwnd, &mut ps) };
        let result = self.render(hwnd, bar, label, hovered, scale);
        // SAFETY: matches the BeginPaint above.
        unsafe {
            let _ = EndPaint(hwnd, &ps);
        }
        result
    }

    fn render(
        &mut self,
        hwnd: HWND,
        bar: &ZoomBar,
        label: &[u16],
        hovered: Option<ZoomAction>,
        scale: f32,
    ) -> Result<(), PlatformError> {
        let graphics = graphics()?;
        let (target, brush) = self.canvas.ensure(&graphics, hwnd)?;
        let layout = ZoomBarLayout::new();
        let whole = rect_f(0.0, 0.0, layout.width as f32, layout.height as f32);
        // DWM rounds and borders the window when it can; otherwise the bar
        // does it itself, as `space_window::Frame` decides for a space.
        let dwm_frame = bar.style.dwm_draws_frame();
        let radius = if dwm_frame {
            0.0
        } else {
            bar.style.corner_radius.max(0) as f32
        };

        // SAFETY: all calls are on live COM objects owned by this painter,
        // between BeginDraw and EndDraw on the UI thread.
        let end = unsafe {
            target.BeginDraw();
            // ClearType needs an opaque background to blend against.
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            target.Clear(Some(&color_f(Color::TRANSPARENT)));
            // Draw in 96-DPI pixels; the transform scales to the monitor.
            target.SetTransform(&Matrix3x2 {
                M11: scale,
                M12: 0.0,
                M21: 0.0,
                M22: scale,
                M31: 0.0,
                M32: 0.0,
            });
            fill_rounded(&target, &brush, whole, radius, bar.tint);
            for action in [ZoomAction::Out, ZoomAction::In, ZoomAction::Reset] {
                draw_button(&graphics, &target, &brush, bar, &layout, action, hovered);
            }
            draw_text(
                &target,
                &brush,
                label,
                &graphics.zoom_percent_format,
                to_rect_f(layout.percent),
                bar.text,
                None,
            );
            let divider_x = layout.divider_x as f32 + 0.5;
            brush.SetColor(&color_f(bar.text.fade(DIVIDER_ALPHA)));
            target.DrawLine(
                Vector2 {
                    X: divider_x,
                    Y: DIVIDER_INSET,
                },
                Vector2 {
                    X: divider_x,
                    Y: layout.height as f32 - DIVIDER_INSET,
                },
                &brush,
                1.0,
                None,
            );
            if !dwm_frame && bar.style.border && bar.border.a > 0 {
                brush.SetColor(&color_f(bar.border));
                target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: inset(whole, 0.5),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &brush,
                    1.0,
                    None,
                );
            }
            target.EndDraw(None, None)
        };
        if let Err(error) = end {
            if error.code() == D2DERR_RECREATE_TARGET {
                tracing::debug!("direct2d target lost; recreating on next paint");
                self.canvas.release(hwnd);
                return Ok(());
            }
            return Err(error.into());
        }
        Ok(())
    }
}

/// Draws one button: a hover fill when it is enabled and hovered, then its
/// glyph or text, dimmed when disabled.
unsafe fn draw_button(
    graphics: &Graphics,
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    bar: &ZoomBar,
    layout: &ZoomBarLayout,
    action: ZoomAction,
    hovered: Option<ZoomAction>,
) {
    let rect = to_rect_f(layout.button(action));
    let enabled = bar.enabled(action);
    let color = if enabled {
        bar.text
    } else {
        bar.text.fade(DISABLED_ALPHA)
    };
    let (text, format) = match (action, &graphics.glyph_format) {
        (ZoomAction::Out, Some(glyphs)) => (ZOOM_OUT_GLYPH, glyphs),
        (ZoomAction::In, Some(glyphs)) => (ZOOM_IN_GLYPH, glyphs),
        (ZoomAction::Out, None) => (ZOOM_OUT_TEXT, &graphics.zoom_percent_format),
        (ZoomAction::In, None) => (ZOOM_IN_TEXT, &graphics.zoom_percent_format),
        (ZoomAction::Reset, _) => (RESET_TEXT, &graphics.zoom_label_format),
    };
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        if enabled && hovered == Some(action) {
            fill_rounded(target, brush, rect, HOVER_RADIUS, bar.hover);
        }
        draw_text(target, brush, text, format, rect, color, None);
    }
}
