//! Paints the color picker (`render::color_picker`): the saturation/value
//! square, the hue and opacity sliders, the preset swatches, the value box
//! with its format radio buttons, and the New / Current comparison.

use deskspace_core::theme::Hsv;
use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE;

use super::*;
use crate::render::color_picker::{
    FieldStatus, GRID_COLUMNS, GRID_ROWS, Grid, PALETTE, PickerLayout, PickerView, Region,
    ValueFormat,
};

mod pen;

use pen::{Formats, Pen, Tones, checker_brush, ink_on};

/// Segoe Fluent Icons / Segoe MDL2 Assets "ChromeClose", "Copy", and
/// "Eyedropper".
/// https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
const CLOSE_GLYPH: &[u16] = &[0xE8BB];
const COPY_GLYPH: &[u16] = &[0xE8C8];
const EYEDROPPER_GLYPH: &[u16] = &[0xEF3C];

const SQUARE_RADIUS: f32 = 8.0;
const SWATCH_RADIUS: f32 = 6.0;
const CONTROL_RADIUS: f32 = 6.0;
const BUTTON_RADIUS: f32 = 8.0;
const SQUARE_MARKER: f32 = 8.0;
const SLIDER_THUMB: f32 = 9.0;
const RADIO: f32 = 7.0;
/// Space between a value box's edge and its text.
const FIELD_INSET: f32 = 10.0;

#[derive(Default)]
pub(crate) struct ColorPickerPainter {
    canvas: HwndCanvas,
    formats: Option<Formats>,
}

impl ColorPickerPainter {
    /// Paints `view` in response to `WM_PAINT`, at `scale` (1.0 at 96 DPI).
    pub(crate) fn paint(
        &mut self,
        hwnd: HWND,
        view: &PickerView,
        layout: &PickerLayout,
        scale: f32,
    ) -> Result<(), PlatformError> {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: BeginPaint/EndPaint bracket the paint for this window; the
        // render target draws to the same HWND in between.
        unsafe { BeginPaint(hwnd, &mut ps) };
        let result = self.render(hwnd, view, layout, scale);
        // SAFETY: matches the BeginPaint above.
        unsafe {
            let _ = EndPaint(hwnd, &ps);
        }
        result
    }

    fn render(
        &mut self,
        hwnd: HWND,
        view: &PickerView,
        layout: &PickerLayout,
        scale: f32,
    ) -> Result<(), PlatformError> {
        let graphics = graphics()?;
        if self.formats.is_none() {
            // SAFETY: format creation on the UI thread.
            self.formats = Some(unsafe { Formats::new(&graphics)? });
        }
        let Self { canvas, formats } = self;
        let Some(formats) = formats.as_ref() else {
            return Ok(());
        };
        let (target, brush) = canvas.ensure(&graphics, hwnd)?;
        // SAFETY: all calls are on live COM objects owned by this painter,
        // between BeginDraw and EndDraw on the UI thread.
        let end = unsafe {
            target.BeginDraw();
            draw(&target, &brush, &graphics, formats, view, layout, scale);
            target.EndDraw(None, None)
        };
        if let Err(error) = end {
            if error.code() == D2DERR_RECREATE_TARGET {
                tracing::debug!("direct2d target lost; recreating on next paint");
                canvas.release(hwnd);
                return Ok(());
            }
            return Err(error.into());
        }
        Ok(())
    }
}

/// Draws the whole picker on `target`, between its BeginDraw and EndDraw.
unsafe fn draw(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    graphics: &Graphics,
    formats: &Formats,
    view: &PickerView,
    layout: &PickerLayout,
    scale: f32,
) {
    let tones = Tones::new(view.dark, view.acrylic);
    // SAFETY: the caller is inside BeginDraw/EndDraw on `target`.
    unsafe {
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
        let pen = Pen {
            target,
            brush,
            dwrite: &graphics.dwrite,
            formats,
            tones,
            checker: checker_brush(target, tones),
        };
        let whole = rect_f(0.0, 0.0, layout.size.0 as f32, layout.size.1 as f32);
        pen.fill(whole, 0.0, tones.body);
        paint_header(&pen, view, layout);
        paint_square(&pen, view, layout);
        paint_sliders(&pen, view, layout);
        paint_presets(&pen, view, layout);
        paint_value(&pen, view, layout);
        paint_footer(&pen, view, layout);
    }
}

fn hovered(view: &PickerView, region: Region) -> bool {
    view.hover == Some(region)
}

unsafe fn paint_header(pen: &Pen, view: &PickerView, layout: &PickerLayout) {
    let tones = pen.tones;
    let close = to_rect_f(layout.close);
    // SAFETY: inside BeginDraw/EndDraw.
    unsafe {
        pen.text(
            "Color Picker",
            &pen.formats.title,
            to_rect_f(layout.title),
            tones.text,
        );
        if hovered(view, Region::Close) {
            pen.fill(close, CONTROL_RADIUS, tones.control_hover);
        }
        pen.glyph(CLOSE_GLYPH, close, tones.text);
    }
}

unsafe fn paint_square(pen: &Pen, view: &PickerView, layout: &PickerLayout) {
    let square = to_rect_f(layout.saturation);
    let pure = Hsv {
        h: view.hsv.h,
        s: 1.0,
        v: 1.0,
    }
    .to_color(255);
    let horizontal = (
        Vector2 {
            X: square.left,
            Y: square.top,
        },
        Vector2 {
            X: square.right,
            Y: square.top,
        },
    );
    let vertical = (
        Vector2 {
            X: square.left,
            Y: square.top,
        },
        Vector2 {
            X: square.left,
            Y: square.bottom,
        },
    );
    // SAFETY: inside BeginDraw/EndDraw.
    unsafe {
        // The hue, whitened toward the left and darkened toward the bottom.
        pen.fill(square, SQUARE_RADIUS, pure);
        let white = Color::rgb(255, 255, 255);
        if let Some(fade) = pen.gradient(
            horizontal.0,
            horizontal.1,
            &[(0.0, white), (1.0, white.with_alpha(0))],
        ) {
            pen.fill_with(square, SQUARE_RADIUS, &fade);
        }
        let black = Color::rgb(0, 0, 0);
        if let Some(shade) = pen.gradient(
            vertical.0,
            vertical.1,
            &[(0.0, black.with_alpha(0)), (1.0, black)],
        ) {
            pen.fill_with(square, SQUARE_RADIUS, &shade);
        }
        pen.outline(square, SQUARE_RADIUS, pen.tones.border, 1.0);
        let x = square.left + view.hsv.s * (square.right - square.left);
        let y = square.top + (1.0 - view.hsv.v) * (square.bottom - square.top);
        marker(pen, x, y, SQUARE_MARKER, view.hsv.to_color(255));
    }
}

/// A round thumb: `fill` in a white ring with a faint dark edge, so it reads
/// on any color.
unsafe fn marker(pen: &Pen, x: f32, y: f32, radius: f32, fill: Color) {
    // SAFETY: inside BeginDraw/EndDraw.
    unsafe {
        pen.circle(
            x,
            y,
            radius + 1.0,
            None,
            Some((Color::rgba(0, 0, 0, 0x60), 1.0)),
        );
        pen.circle(
            x,
            y,
            radius,
            Some(fill),
            Some((Color::rgb(255, 255, 255), 2.5)),
        );
    }
}

unsafe fn paint_sliders(pen: &Pen, view: &PickerView, layout: &PickerLayout) {
    let tones = pen.tones;
    let hue = to_rect_f(layout.hue);
    let alpha = to_rect_f(layout.alpha);
    let radius = (hue.bottom - hue.top) / 2.0;
    let across = |rect: D2D_RECT_F| {
        (
            Vector2 {
                X: rect.left,
                Y: rect.top,
            },
            Vector2 {
                X: rect.right,
                Y: rect.top,
            },
        )
    };
    let opaque = view.hsv.to_color(255);
    // SAFETY: inside BeginDraw/EndDraw.
    unsafe {
        let stops: Vec<(f32, Color)> = (0..=6)
            .map(|step| {
                let position = step as f32 / 6.0;
                let color = Hsv {
                    h: position * 360.0,
                    s: 1.0,
                    v: 1.0,
                }
                .to_color(255);
                (position, color)
            })
            .collect();
        let (start, end) = across(hue);
        if let Some(spectrum) = pen.gradient(start, end, &stops) {
            pen.fill_with(hue, radius, &spectrum);
        }
        let hue_x = hue.left + view.hsv.h / 360.0 * (hue.right - hue.left);
        let pure = Hsv {
            h: view.hsv.h,
            s: 1.0,
            v: 1.0,
        }
        .to_color(255);
        marker(pen, hue_x, (hue.top + hue.bottom) / 2.0, SLIDER_THUMB, pure);

        let label = to_rect_f(layout.alpha_label);
        pen.text("Opacity", &pen.formats.label, label, tones.secondary);
        let percent = format!("{}%", (view.alpha as f32 / 2.55).round());
        pen.text(
            &percent,
            &pen.formats.label_trailing,
            label,
            tones.secondary,
        );
        if let Some(checker) = &pen.checker {
            pen.fill_with(alpha, radius, checker);
        }
        let (start, end) = across(alpha);
        if let Some(ramp) = pen.gradient(start, end, &[(0.0, opaque.with_alpha(0)), (1.0, opaque)])
        {
            pen.fill_with(alpha, radius, &ramp);
        }
        pen.outline(alpha, radius, tones.border, 1.0);
        let alpha_x = alpha.left + view.alpha as f32 / 255.0 * (alpha.right - alpha.left);
        marker(
            pen,
            alpha_x,
            (alpha.top + alpha.bottom) / 2.0,
            SLIDER_THUMB,
            opaque,
        );
    }
}

unsafe fn paint_presets(pen: &Pen, view: &PickerView, layout: &PickerLayout) {
    let tones = pen.tones;
    let eyedropper = to_rect_f(layout.eyedropper);
    // SAFETY: inside BeginDraw/EndDraw.
    unsafe {
        pen.text(
            "Presets",
            &pen.formats.heading,
            to_rect_f(layout.presets_label),
            tones.text,
        );
        pen.text(
            "Eyedropper",
            &pen.formats.label_trailing,
            to_rect_f(layout.eyedropper_label),
            tones.secondary,
        );
        if view.sampling {
            pen.fill(eyedropper, CONTROL_RADIUS, view.accent);
            pen.glyph(EYEDROPPER_GLYPH, eyedropper, ink_on(view.accent, tones));
        } else {
            pen.button(
                eyedropper,
                CONTROL_RADIUS,
                tones.control,
                hovered(view, Region::Eyedropper),
            );
            pen.glyph(EYEDROPPER_GLYPH, eyedropper, tones.text);
        }

        pen.text(
            "Recent Colors",
            &pen.formats.label,
            to_rect_f(layout.recent_label),
            tones.secondary,
        );
        pen.text(
            "Default Palette",
            &pen.formats.label,
            to_rect_f(layout.palette_label),
            tones.secondary,
        );
        let current = view.color();
        for index in 0..GRID_COLUMNS * GRID_ROWS {
            let slot = to_rect_f(layout.swatch_rect(Grid::Recent, index));
            match view.recent.get(index) {
                Some(color) => swatch(
                    pen,
                    view,
                    slot,
                    *color,
                    Region::Swatch(Grid::Recent, index),
                    current,
                ),
                None => pen.fill(slot, SWATCH_RADIUS, tones.control),
            }
        }
        for (index, color) in PALETTE.iter().enumerate() {
            let slot = to_rect_f(layout.swatch_rect(Grid::Palette, index));
            swatch(
                pen,
                view,
                slot,
                *color,
                Region::Swatch(Grid::Palette, index),
                current,
            );
        }
    }
}

/// One preset swatch, ringed when it is the current color or hovered.
unsafe fn swatch(
    pen: &Pen,
    view: &PickerView,
    slot: D2D_RECT_F,
    color: Color,
    region: Region,
    current: Color,
) {
    // SAFETY: inside BeginDraw/EndDraw.
    unsafe {
        pen.fill_color(slot, SWATCH_RADIUS, color);
        pen.outline(slot, SWATCH_RADIUS, pen.tones.border, 1.0);
        let ring = inset(slot, -3.0);
        if color == current {
            pen.outline(ring, SWATCH_RADIUS + 3.0, pen.tones.text, 2.0);
        } else if hovered(view, region) {
            pen.outline(ring, SWATCH_RADIUS + 3.0, pen.tones.secondary, 1.5);
        }
    }
}

unsafe fn paint_value(pen: &Pen, view: &PickerView, layout: &PickerLayout) {
    let tones = pen.tones;
    let field = to_rect_f(layout.field);
    let copy = to_rect_f(layout.copy);
    // SAFETY: inside BeginDraw/EndDraw.
    unsafe {
        for format in ValueFormat::ALL {
            radio(pen, view, layout, format);
        }

        pen.fill(field, CONTROL_RADIUS, tones.field);
        let invalid = matches!(view.status, FieldStatus::Invalid(_));
        if invalid {
            pen.outline(field, CONTROL_RADIUS, tones.error, 1.5);
        } else if view.field_focused {
            pen.outline(field, CONTROL_RADIUS, view.accent, 1.5);
        } else {
            pen.outline(field, CONTROL_RADIUS, tones.border, 1.0);
        }
        let text_area = rect_f(
            field.left + FIELD_INSET,
            field.top,
            copy.left - 4.0,
            field.bottom,
        );
        field_text(pen, view, text_area);
        if hovered(view, Region::Copy) {
            pen.fill(copy, CONTROL_RADIUS, tones.control_hover);
        }
        pen.glyph(COPY_GLYPH, copy, tones.text);

        let status = to_rect_f(layout.status);
        match view.status {
            FieldStatus::Invalid(message) => {
                pen.text(message, &pen.formats.label, status, tones.error)
            }
            FieldStatus::Copied => pen.text(
                "Copied to the clipboard",
                &pen.formats.label,
                status,
                tones.secondary,
            ),
            FieldStatus::None if view.sampling => pen.text(
                "Click to pick a color; Esc cancels",
                &pen.formats.label,
                status,
                tones.secondary,
            ),
            FieldStatus::None => {}
        }
    }
}

/// A format radio button: a circle, filled with the accent when chosen, and
/// the format's name.
unsafe fn radio(pen: &Pen, view: &PickerView, layout: &PickerLayout, format: ValueFormat) {
    let tones = pen.tones;
    let rect = to_rect_f(layout.format_rect(format));
    let x = rect.left + RADIO;
    let y = (rect.top + rect.bottom) / 2.0;
    let ring = if hovered(view, Region::Format(format)) {
        tones.text
    } else {
        tones.secondary
    };
    // SAFETY: inside BeginDraw/EndDraw.
    unsafe {
        if view.format == format {
            pen.circle(x, y, RADIO, Some(view.accent), None);
            pen.circle(x, y, RADIO * 0.4, Some(ink_on(view.accent, tones)), None);
        } else {
            pen.circle(x, y, RADIO, Some(tones.field), Some((ring, 1.0)));
        }
        let label = rect_f(
            rect.left + RADIO * 2.0 + 6.0,
            rect.top,
            rect.right,
            rect.bottom,
        );
        pen.text(format.label(), &pen.formats.value, label, tones.text);
    }
}

/// The value box's text, its selection, and the caret while focused. The
/// text scrolls left when the caret would leave the box.
unsafe fn field_text(pen: &Pen, view: &PickerView, area: D2D_RECT_F) {
    let text = view.field.text();
    let wide: Vec<u16> = text.encode_utf16().collect();
    let width = (area.right - area.left).max(1.0);
    let height = (area.bottom - area.top).max(1.0);
    let utf16 = |byte: usize| text.get(..byte).unwrap_or(text).encode_utf16().count() as u32;
    // SAFETY: inside BeginDraw/EndDraw; positions are within the text.
    unsafe {
        let Ok(layout) =
            pen.dwrite
                .CreateTextLayout(&wide, &pen.formats.value, f32::MAX / 4.0, height)
        else {
            return;
        };
        let x_of = |position: u32| {
            let (mut x, mut y) = (0.0f32, 0.0f32);
            let mut hit = DWRITE_HIT_TEST_METRICS::default();
            let _ = layout.HitTestTextPosition(position, false, &mut x, &mut y, &mut hit);
            (x, y, hit.height.max(1.0))
        };
        let caret = x_of(utf16(view.field.caret()));
        let scroll = (caret.0 - width + 2.0).max(0.0);
        let left = area.left - scroll;
        pen.target
            .PushAxisAlignedClip(&area, D2D1_ANTIALIAS_MODE_ALIASED);
        let (start, end) = view.field.selection();
        if view.field_focused && start != end {
            let (x0, y, line) = x_of(utf16(start));
            let (x1, _, _) = x_of(utf16(end));
            pen.fill(
                rect_f(left + x0, area.top + y, left + x1, area.top + y + line),
                0.0,
                view.accent.with_alpha(0x70),
            );
        }
        pen.brush.SetColor(&color_f(pen.tones.text));
        pen.target.DrawTextLayout(
            Vector2 {
                X: left,
                Y: area.top,
            },
            &layout,
            pen.brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
        );
        if view.field_focused && view.caret_visible {
            let (x, y, line) = caret;
            pen.fill(
                rect_f(left + x, area.top + y, left + x + 1.0, area.top + y + line),
                0.0,
                pen.tones.text,
            );
        }
        pen.target.PopAxisAlignedClip();
    }
}

unsafe fn paint_footer(pen: &Pen, view: &PickerView, layout: &PickerLayout) {
    let tones = pen.tones;
    let compare = to_rect_f(layout.compare);
    let (new, current) = layout.compare_halves();
    let (new, current) = (to_rect_f(new), to_rect_f(current));
    let color = view.color();
    // SAFETY: inside BeginDraw/EndDraw; each clip is popped before the next.
    unsafe {
        for (half, fill, label) in [(new, color, "New"), (current, view.initial, "Current")] {
            pen.target
                .PushAxisAlignedClip(&half, D2D1_ANTIALIAS_MODE_ALIASED);
            pen.fill_color(compare, BUTTON_RADIUS, fill);
            pen.target.PopAxisAlignedClip();
            pen.text(label, &pen.formats.button, half, ink_on(fill, tones));
        }
        if hovered(view, Region::Current) {
            pen.target
                .PushAxisAlignedClip(&current, D2D1_ANTIALIAS_MODE_ALIASED);
            pen.outline(compare, BUTTON_RADIUS, ink_on(view.initial, tones), 2.0);
            pen.target.PopAxisAlignedClip();
        }
        pen.outline(compare, BUTTON_RADIUS, tones.border, 1.0);

        let cancel = to_rect_f(layout.cancel);
        pen.button(
            cancel,
            BUTTON_RADIUS,
            tones.control,
            hovered(view, Region::Cancel),
        );
        pen.outline(cancel, BUTTON_RADIUS, tones.border, 1.0);
        pen.text("Cancel", &pen.formats.button, cancel, tones.text);

        let apply = to_rect_f(layout.apply);
        pen.fill(apply, BUTTON_RADIUS, view.accent);
        if hovered(view, Region::Apply) {
            pen.fill(apply, BUTTON_RADIUS, Color::rgba(255, 255, 255, 0x26));
        }
        pen.text(
            "Apply",
            &pen.formats.button,
            apply,
            ink_on(view.accent, tones),
        );
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::GENERIC_WRITE;
    use windows::Win32::Graphics::Imaging::{GUID_ContainerFormatPng, WICBitmapEncoderNoCache};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    use super::*;
    use crate::edit::TextEdit;
    use crate::render::color_picker::ValueFormat;

    /// Renders the picker, dark and light, to PNGs in the temp folder, so its
    /// look can be checked without a screen. Opaque, since an offscreen
    /// bitmap has no acrylic behind it.
    #[test]
    #[ignore = "writes preview images; run by hand"]
    fn render_preview() {
        // SAFETY: COM and WIC on this test thread; every object is local.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let graphics = Graphics::new().unwrap();
            let formats = Formats::new(&graphics).unwrap();
            let layout = PickerLayout::new();
            let scale = 1.5;
            let size = (
                (layout.size.0 as f32 * scale) as u32,
                (layout.size.1 as f32 * scale) as u32,
            );
            for dark in [true, false] {
                let initial = Color::rgb(59, 130, 246);
                let mut field = TextEdit::new("hsl(217, 91%, 6");
                field.select_range(15, 15);
                let view = PickerView {
                    hsv: initial.to_hsv(),
                    alpha: 200,
                    initial,
                    recent: vec![Color::rgb(255, 99, 71), Color::rgba(20, 20, 20, 120)],
                    format: ValueFormat::Hsl,
                    field,
                    field_focused: true,
                    caret_visible: true,
                    status: FieldStatus::Invalid("A hex code has 3, 4, 6, or 8 digits"),
                    hover: Some(Region::Swatch(Grid::Palette, 2)),
                    sampling: false,
                    dark,
                    acrylic: false,
                    accent: Color::rgb(0x4F, 0x6B, 0xED),
                };
                let bitmap = graphics
                    .wic
                    .CreateBitmap(
                        size.0,
                        size.1,
                        &GUID_WICPixelFormat32bppPBGRA,
                        WICBitmapCacheOnLoad,
                    )
                    .unwrap();
                let target = graphics
                    .d2d
                    .CreateWicBitmapRenderTarget(
                        &bitmap,
                        &D2D1_RENDER_TARGET_PROPERTIES {
                            pixelFormat: D2D1_PIXEL_FORMAT {
                                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                            },
                            dpiX: 96.0,
                            dpiY: 96.0,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                let brush = target
                    .CreateSolidColorBrush(&color_f(Color::TRANSPARENT), None)
                    .unwrap();
                target.BeginDraw();
                draw(&target, &brush, &graphics, &formats, &view, &layout, scale);
                target.EndDraw(None, None).unwrap();

                let name = if dark { "dark" } else { "light" };
                let path = std::env::temp_dir().join(format!("deskspace-color-picker-{name}.png"));
                let wide = crate::win32::system::wide(&path.to_string_lossy());
                let stream = graphics.wic.CreateStream().unwrap();
                stream
                    .InitializeFromFilename(PCWSTR(wide.as_ptr()), GENERIC_WRITE.0)
                    .unwrap();
                let encoder = graphics
                    .wic
                    .CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())
                    .unwrap();
                encoder
                    .Initialize(&stream, WICBitmapEncoderNoCache)
                    .unwrap();
                let mut frame = None;
                encoder
                    .CreateNewFrame(&mut frame, std::ptr::null_mut())
                    .unwrap();
                let frame = frame.unwrap();
                frame.Initialize(None).unwrap();
                frame.WriteSource(&bitmap, std::ptr::null()).unwrap();
                frame.Commit().unwrap();
                encoder.Commit().unwrap();
                println!("{}", path.display());
            }
        }
    }
}
