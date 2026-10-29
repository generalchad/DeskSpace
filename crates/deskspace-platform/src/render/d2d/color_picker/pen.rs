//! The picker painter's drawing kit: its colors per theme, its text formats,
//! and `Pen`, which draws rounded fills, outlines, circles, text, glyphs,
//! gradients, and the checkerboard behind translucent colors.

use windows::Win32::Graphics::Direct2D::Common::D2D1_GRADIENT_STOP;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_EXTEND_MODE_WRAP,
    D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::core::Interface;

use super::super::graphics::make_format;
use super::super::*;

/// One checkerboard cell behind translucent colors.
const CHECKER_CELL: f32 = 5.0;

/// The picker's colors for one theme.
#[derive(Clone, Copy)]
pub(super) struct Tones {
    pub(super) body: Color,
    pub(super) text: Color,
    pub(super) secondary: Color,
    /// Buttons and empty swatch slots.
    pub(super) control: Color,
    pub(super) control_hover: Color,
    pub(super) border: Color,
    pub(super) field: Color,
    pub(super) error: Color,
    pub(super) checker: (Color, Color),
}

impl Tones {
    pub(super) fn new(dark: bool, acrylic: bool) -> Self {
        // Over acrylic the body only deepens the material; without it, the
        // body is the window's background and must be opaque.
        let body_alpha = if acrylic { 0x70 } else { 0xFF };
        if dark {
            Self {
                body: Color::rgba(0x20, 0x20, 0x20, body_alpha),
                text: Color::rgb(0xFF, 0xFF, 0xFF),
                secondary: Color::rgba(0xFF, 0xFF, 0xFF, 0xA8),
                control: Color::rgba(0xFF, 0xFF, 0xFF, 0x12),
                control_hover: Color::rgba(0xFF, 0xFF, 0xFF, 0x22),
                border: Color::rgba(0xFF, 0xFF, 0xFF, 0x20),
                field: Color::rgba(0x00, 0x00, 0x00, 0x48),
                error: Color::rgb(0xFF, 0x99, 0xA4),
                checker: (Color::rgb(0x60, 0x60, 0x60), Color::rgb(0x3A, 0x3A, 0x3A)),
            }
        } else {
            Self {
                body: Color::rgba(0xF3, 0xF3, 0xF3, body_alpha),
                text: Color::rgb(0x1A, 0x1A, 0x1A),
                secondary: Color::rgba(0x00, 0x00, 0x00, 0x9C),
                control: Color::rgba(0x00, 0x00, 0x00, 0x0A),
                control_hover: Color::rgba(0x00, 0x00, 0x00, 0x16),
                border: Color::rgba(0x00, 0x00, 0x00, 0x22),
                field: Color::rgba(0xFF, 0xFF, 0xFF, 0xD0),
                error: Color::rgb(0xC4, 0x2B, 0x1C),
                checker: (Color::rgb(0xFF, 0xFF, 0xFF), Color::rgb(0xCC, 0xCC, 0xCC)),
            }
        }
    }
}

/// The picker's text formats, made once per painter.
pub(super) struct Formats {
    pub(super) title: IDWriteTextFormat,
    pub(super) heading: IDWriteTextFormat,
    pub(super) label: IDWriteTextFormat,
    pub(super) label_trailing: IDWriteTextFormat,
    pub(super) value: IDWriteTextFormat,
    pub(super) button: IDWriteTextFormat,
    pub(super) glyph: Option<IDWriteTextFormat>,
}

impl Formats {
    pub(super) unsafe fn new(graphics: &Graphics) -> Result<Self, PlatformError> {
        let text = graphics.text_family;
        let semi = DWRITE_FONT_WEIGHT_SEMI_BOLD;
        let normal = DWRITE_FONT_WEIGHT_NORMAL;
        let leading = DWRITE_TEXT_ALIGNMENT_LEADING;
        // SAFETY: plain format creation from the shared factory.
        unsafe {
            Ok(Self {
                title: line_format(graphics, text, semi, 20.0, leading)?,
                heading: line_format(graphics, text, semi, 14.0, leading)?,
                label: line_format(graphics, text, normal, 12.0, leading)?,
                label_trailing: line_format(
                    graphics,
                    text,
                    normal,
                    12.0,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                )?,
                value: line_format(graphics, text, normal, 14.0, leading)?,
                button: line_format(graphics, text, semi, 14.0, DWRITE_TEXT_ALIGNMENT_CENTER)?,
                glyph: match graphics.glyph_family {
                    Some(family) => Some(line_format(
                        graphics,
                        family,
                        normal,
                        14.0,
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                    )?),
                    None => None,
                },
            })
        }
    }
}

/// A one-line format, centered vertically.
unsafe fn line_format(
    graphics: &Graphics,
    family: PCWSTR,
    weight: DWRITE_FONT_WEIGHT,
    size: f32,
    alignment: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat, PlatformError> {
    // SAFETY: as in `make_format`.
    unsafe {
        let format = make_format(&graphics.dwrite, family, weight, size)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetTextAlignment(alignment)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        Ok(format)
    }
}

/// What every drawing helper needs, gathered so the helpers stay short.
pub(super) struct Pen<'a> {
    pub(super) target: &'a ID2D1RenderTarget,
    pub(super) brush: &'a ID2D1SolidColorBrush,
    pub(super) dwrite: &'a IDWriteFactory,
    pub(super) formats: &'a Formats,
    pub(super) tones: Tones,
    /// `None` if the bitmap could not be made; translucent colors are then
    /// drawn over the body alone.
    pub(super) checker: Option<ID2D1Brush>,
}

// SAFETY (for every method): callers are inside BeginDraw/EndDraw.
impl Pen<'_> {
    pub(super) unsafe fn fill(&self, rect: D2D_RECT_F, radius: f32, color: Color) {
        // SAFETY: see the impl.
        unsafe { fill_rounded(self.target, self.brush, rect, radius, color) };
    }

    pub(super) unsafe fn fill_with(&self, rect: D2D_RECT_F, radius: f32, brush: &ID2D1Brush) {
        // SAFETY: see the impl.
        unsafe {
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                brush,
            );
        }
    }

    /// A rounded outline of `width` drawn just inside `rect`.
    pub(super) unsafe fn outline(&self, rect: D2D_RECT_F, radius: f32, color: Color, width: f32) {
        // SAFETY: see the impl.
        unsafe {
            self.brush.SetColor(&color_f(color));
            self.target.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: inset(rect, width / 2.0),
                    radiusX: radius,
                    radiusY: radius,
                },
                self.brush,
                width,
                None,
            );
        }
    }

    /// Fills `rect` with `color`, over a checkerboard when it is translucent.
    pub(super) unsafe fn fill_color(&self, rect: D2D_RECT_F, radius: f32, color: Color) {
        // SAFETY: see the impl.
        unsafe {
            if color.a < 255
                && let Some(checker) = &self.checker
            {
                self.fill_with(rect, radius, checker);
            }
            self.fill(rect, radius, color);
        }
    }

    pub(super) unsafe fn circle(
        &self,
        x: f32,
        y: f32,
        radius: f32,
        fill: Option<Color>,
        ring: Option<(Color, f32)>,
    ) {
        let ellipse = |radius: f32| D2D1_ELLIPSE {
            point: Vector2 { X: x, Y: y },
            radiusX: radius,
            radiusY: radius,
        };
        // SAFETY: see the impl.
        unsafe {
            if let Some(fill) = fill {
                self.brush.SetColor(&color_f(fill));
                self.target.FillEllipse(&ellipse(radius), self.brush);
            }
            if let Some((color, width)) = ring {
                self.brush.SetColor(&color_f(color));
                self.target
                    .DrawEllipse(&ellipse(radius - width / 2.0), self.brush, width, None);
            }
        }
    }

    pub(super) unsafe fn text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        color: Color,
    ) {
        let wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: see the impl.
        unsafe { draw_text(self.target, self.brush, &wide, format, rect, color, None) };
    }

    pub(super) unsafe fn glyph(&self, glyph: &[u16], rect: D2D_RECT_F, color: Color) {
        if let Some(format) = &self.formats.glyph {
            // SAFETY: see the impl.
            unsafe { draw_text(self.target, self.brush, glyph, format, rect, color, None) };
        }
    }

    /// A linear gradient from `start` to `end` through `stops`.
    pub(super) unsafe fn gradient(
        &self,
        start: Vector2,
        end: Vector2,
        stops: &[(f32, Color)],
    ) -> Option<ID2D1Brush> {
        let stops: Vec<D2D1_GRADIENT_STOP> = stops
            .iter()
            .map(|(position, color)| D2D1_GRADIENT_STOP {
                position: *position,
                color: color_f(*color),
            })
            .collect();
        // SAFETY: see the impl; the stops outlive the call.
        unsafe {
            let collection = self
                .target
                .CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
                .ok()?;
            self.target
                .CreateLinearGradientBrush(
                    &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                        startPoint: start,
                        endPoint: end,
                    },
                    None,
                    &collection,
                )
                .ok()?
                .cast()
                .ok()
        }
    }

    /// A button's background: the hover fill when hovered, else `rest`.
    pub(super) unsafe fn button(&self, rect: D2D_RECT_F, radius: f32, rest: Color, hovered: bool) {
        let fill = if hovered {
            self.tones.control_hover
        } else {
            rest
        };
        // SAFETY: see the impl.
        unsafe { self.fill(rect, radius, fill) };
    }
}

/// A two-by-two checkerboard bitmap, tiled.
pub(super) unsafe fn checker_brush(target: &ID2D1RenderTarget, tones: Tones) -> Option<ID2D1Brush> {
    let pixel = |color: Color| {
        0xFF00_0000 | (color.r as u32) << 16 | (color.g as u32) << 8 | color.b as u32
    };
    let (light, dark) = tones.checker;
    let pixels = [pixel(light), pixel(dark), pixel(dark), pixel(light)];
    let properties = D2D1_BITMAP_PROPERTIES {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
    };
    // SAFETY: the pixels are opaque, so already premultiplied, and outlive
    // the copy `CreateBitmap` makes; the pitch is one row of two pixels.
    unsafe {
        let bitmap = target
            .CreateBitmap(
                D2D_SIZE_U {
                    width: 2,
                    height: 2,
                },
                Some(pixels.as_ptr().cast()),
                8,
                &properties,
            )
            .ok()?;
        let brush = target
            .CreateBitmapBrush(
                &bitmap,
                Some(&D2D1_BITMAP_BRUSH_PROPERTIES {
                    extendModeX: D2D1_EXTEND_MODE_WRAP,
                    extendModeY: D2D1_EXTEND_MODE_WRAP,
                    interpolationMode: D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                }),
                None,
            )
            .ok()?;
        brush.SetTransform(&Matrix3x2 {
            M11: CHECKER_CELL,
            M12: 0.0,
            M21: 0.0,
            M22: CHECKER_CELL,
            M31: 0.0,
            M32: 0.0,
        });
        brush.cast().ok()
    }
}

/// Black or white, whichever reads on `color`.
pub(super) fn ink_on(color: Color, tones: Tones) -> Color {
    if color.a < 128 {
        tones.text
    } else if color.luminance() > 0.4 {
        Color::rgb(0, 0, 0)
    } else {
        Color::rgb(255, 255, 255)
    }
}
