//! Direct2D + DirectWrite painter for space windows.
//!
//! Each space window owns a `Painter` with an `ID2D1HwndRenderTarget` using
//! premultiplied alpha. Because the window's frame is extended over the whole
//! client area (`dwm::apply_style`), DWM composites those alpha values over the
//! blur material: transparent pixels show the acrylic/blur material and the
//! tint is laid on top of it.
//! https://learn.microsoft.com/en-us/windows/win32/direct2d/supported-pixel-formats-and-alpha-modes

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use deskspace_core::geometry::Rect;
use deskspace_core::theme::Color;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_OPTIONS, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, HICON};
use windows::core::{BOOL, HRESULT, PCWSTR, w};

use crate::error::PlatformError;
use crate::render::Icon;
use crate::render::scene::{spaceScene, ItemState};

const ICON_SIZE: f32 = 32.0;
const ICON_TOP: f32 = 10.0;
const TITLE_INSET: f32 = 12.0;
const LABEL_LINES: f32 = 2.0;
const LABEL_LINE_HEIGHT: f32 = 16.0;
const SCROLL_THUMB_ALPHA: u8 = 110;
/// Segoe Fluent Icons / Segoe MDL2 Assets "Lock" glyph.
/// https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
const LOCK_GLYPH: &[u16] = &[0xE72E];

/// D2DERR_RECREATE_TARGET: the device was lost and every device-dependent
/// resource must be recreated.
/// https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-error-codes
const D2DERR_RECREATE_TARGET: HRESULT = HRESULT(0x8899_000C_u32 as i32);

/// Device-independent resources shared by every painter on the UI thread.
struct Graphics {
    d2d: ID2D1Factory,
    wic: IWICImagingFactory,
    title_format: IDWriteTextFormat,
    label_format: IDWriteTextFormat,
    glyph_format: Option<IDWriteTextFormat>,
}

thread_local! {
    static GRAPHICS: RefCell<Option<Rc<Graphics>>> = const { RefCell::new(None) };
}

/// Releases the shared factories. Must run before `CoUninitialize`, because
/// thread-local destructors would otherwise release COM objects after COM is
/// gone.
pub(crate) fn release_graphics() {
    GRAPHICS.with(|cell| cell.borrow_mut().take());
}

fn graphics() -> Result<Rc<Graphics>, PlatformError> {
    if let Some(existing) = GRAPHICS.with(|cell| cell.borrow().clone()) {
        return Ok(existing);
    }
    let created = Rc::new(Graphics::new()?);
    GRAPHICS.with(|cell| *cell.borrow_mut() = Some(created.clone()));
    Ok(created)
}

impl Graphics {
    fn new() -> Result<Self, PlatformError> {
        // SAFETY: plain factory creation; COM is initialised on this thread by
        // `runtime::run` before any window paints.
        unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                Some(&D2D1_FACTORY_OPTIONS::default()),
            )?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let wic: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;

            let mut fonts = None;
            dwrite.GetSystemFontCollection(&mut fonts, false)?;
            let fonts = fonts.ok_or(PlatformError::InvalidFormat("system font collection"))?;
            let text_family =
                first_installed(&fonts, &[w!("Segoe UI Variable Text"), w!("Segoe UI")])
                    .unwrap_or(w!("Segoe UI"));
            let glyph_family =
                first_installed(&fonts, &[w!("Segoe Fluent Icons"), w!("Segoe MDL2 Assets")]);

            let title_format =
                make_format(&dwrite, text_family, DWRITE_FONT_WEIGHT_SEMI_BOLD, 13.0)?;
            title_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            title_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            title_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let title_sign = dwrite.CreateEllipsisTrimmingSign(&title_format)?;
            title_format.SetTrimming(&character_trimming(), &title_sign)?;

            let label_format = make_format(&dwrite, text_family, DWRITE_FONT_WEIGHT_NORMAL, 12.0)?;
            label_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
            label_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            label_format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            let label_sign = dwrite.CreateEllipsisTrimmingSign(&label_format)?;
            label_format.SetTrimming(&character_trimming(), &label_sign)?;

            let glyph_format = match glyph_family {
                Some(family) => {
                    let format = make_format(&dwrite, family, DWRITE_FONT_WEIGHT_NORMAL, 12.0)?;
                    format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                    format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                    Some(format)
                }
                None => None,
            };

            Ok(Self {
                d2d,
                wic,
                title_format,
                label_format,
                glyph_format,
            })
        }
    }
}

fn character_trimming() -> DWRITE_TRIMMING {
    DWRITE_TRIMMING {
        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
        delimiter: 0,
        delimiterCount: 0,
    }
}

unsafe fn make_format(
    dwrite: &IDWriteFactory,
    family: PCWSTR,
    weight: DWRITE_FONT_WEIGHT,
    size: f32,
) -> Result<IDWriteTextFormat, PlatformError> {
    // SAFETY: `family` is a static wide string; the locale is a literal.
    unsafe {
        Ok(dwrite.CreateTextFormat(
            family,
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!(""),
        )?)
    }
}

unsafe fn first_installed(fonts: &IDWriteFontCollection, families: &[PCWSTR]) -> Option<PCWSTR> {
    families.iter().copied().find(|family| {
        let mut index = 0;
        let mut exists = BOOL::default();
        // SAFETY: out-pointers are valid locals.
        unsafe { fonts.FindFamilyName(*family, &mut index, &mut exists) }.is_ok()
            && exists.as_bool()
    })
}

/// A scene with its strings pre-encoded, so painting does not allocate.
pub struct PreparedScene {
    scene: spaceScene,
    title: Vec<u16>,
    labels: Vec<Vec<u16>>,
}

impl PreparedScene {
    pub fn new(scene: spaceScene) -> Self {
        let title = scene.title.encode_utf16().collect();
        let labels = scene
            .items
            .iter()
            .map(|item| item.label.encode_utf16().collect())
            .collect();
        Self {
            scene,
            title,
            labels,
        }
    }

    pub fn scene(&self) -> &spaceScene {
        &self.scene
    }
}

#[derive(Default)]
pub struct Painter {
    target: Option<ID2D1HwndRenderTarget>,
    brush: Option<ID2D1SolidColorBrush>,
    /// GPU copies of `Icon`s, keyed by `Icon::id`.
    icons: HashMap<u64, ID2D1Bitmap>,
    size: (u32, u32),
}

impl Painter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Paints `scene` in response to `WM_PAINT`. `opaque` is for spaces hosted
    /// inside the desktop window, where per-pixel alpha is not composited.
    pub fn paint(
        &mut self,
        hwnd: HWND,
        scene: Option<&PreparedScene>,
        opaque: bool,
    ) -> Result<(), PlatformError> {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: BeginPaint/EndPaint bracket the paint for this window; the
        // render target draws to the same HWND in between.
        unsafe { BeginPaint(hwnd, &mut ps) };
        let result = match scene {
            Some(scene) => self.render(hwnd, scene, opaque),
            None => Ok(()),
        };
        // SAFETY: matches the BeginPaint above.
        unsafe {
            let _ = EndPaint(hwnd, &ps);
        }
        result
    }

    fn render(
        &mut self,
        hwnd: HWND,
        prepared: &PreparedScene,
        opaque: bool,
    ) -> Result<(), PlatformError> {
        let graphics = graphics()?;
        let (target, brush) = self.ensure_target(&graphics, hwnd)?;
        let scene = &prepared.scene;
        let width = self.size.0 as f32;
        let height = self.size.1 as f32;

        // SAFETY: all calls are on live COM objects owned by this painter,
        // between BeginDraw and EndDraw on the UI thread.
        let end = unsafe {
            target.BeginDraw();
            if opaque {
                target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
                target.Clear(Some(&color_f(scene.tint.with_alpha(255))));
            } else {
                // ClearType needs an opaque background to blend against; on a
                // translucent surface it leaves colored fringes.
                target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
                target.Clear(Some(&color_f(Color::TRANSPARENT)));
                fill(&target, &brush, rect_f(0.0, 0.0, width, height), scene.tint);
            }

            let title_height = scene.title_height as f32;
            fill(
                &target,
                &brush,
                rect_f(0.0, 0.0, width, title_height.min(height)),
                scene.title_fill,
            );
            self.draw_title(&graphics, &target, &brush, prepared, width, title_height);

            if !scene.rolled_up && height > title_height {
                target.PushAxisAlignedClip(
                    &rect_f(0.0, title_height, width, height),
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
                self.draw_items(&graphics, &target, &brush, prepared);
                target.PopAxisAlignedClip();
                if let Some(thumb) = scene.scroll_thumb {
                    brush.SetColor(&color_f(scene.item_text.fade(SCROLL_THUMB_ALPHA)));
                    let thumb = to_rect_f(thumb);
                    let radius = (thumb.right - thumb.left) / 2.0;
                    target.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: thumb,
                            radiusX: radius,
                            radiusY: radius,
                        },
                        &brush,
                    );
                }
            }

            if opaque && scene.border.a > 0 {
                brush.SetColor(&color_f(scene.border));
                let radius = scene.corner_radius as f32;
                target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: rect_f(0.5, 0.5, width - 0.5, height - 0.5),
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
                self.release_device_resources();
                return Ok(());
            }
            return Err(error.into());
        }
        Ok(())
    }

    unsafe fn draw_title(
        &self,
        graphics: &Graphics,
        target: &ID2D1HwndRenderTarget,
        brush: &ID2D1SolidColorBrush,
        prepared: &PreparedScene,
        width: f32,
        title_height: f32,
    ) {
        let scene = &prepared.scene;
        let lock_width = if scene.locked { title_height } else { 0.0 };
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            brush.SetColor(&color_f(scene.title_text));
            target.DrawText(
                &prepared.title,
                &graphics.title_format,
                &rect_f(
                    TITLE_INSET,
                    0.0,
                    (width - TITLE_INSET - lock_width).max(TITLE_INSET),
                    title_height,
                ),
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            if let (true, Some(format)) = (scene.locked, &graphics.glyph_format) {
                brush.SetColor(&color_f(scene.title_text.fade(170)));
                target.DrawText(
                    LOCK_GLYPH,
                    format,
                    &rect_f(width - lock_width - 4.0, 0.0, width - 4.0, title_height),
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }

    unsafe fn draw_items(
        &mut self,
        graphics: &Graphics,
        target: &ID2D1HwndRenderTarget,
        brush: &ID2D1SolidColorBrush,
        prepared: &PreparedScene,
    ) {
        let scene = &prepared.scene;
        for (item, label) in scene.items.iter().zip(&prepared.labels) {
            let cell = to_rect_f(item.rect);
            let highlight = match item.state {
                ItemState::Normal => None,
                ItemState::Hovered => Some(scene.hover_fill),
                ItemState::Selected => Some(scene.selected_fill),
            };
            // SAFETY: caller is inside BeginDraw/EndDraw.
            unsafe {
                if let Some(color) = highlight {
                    brush.SetColor(&color_f(color));
                    target.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: inset(cell, 2.0),
                            radiusX: 4.0,
                            radiusY: 4.0,
                        },
                        brush,
                    );
                }

                let center_x = (cell.left + cell.right) / 2.0;
                if let Some(bitmap) = item.icon.as_ref().and_then(|icon| self.bitmap_for(icon)) {
                    let left = (center_x - ICON_SIZE / 2.0).round();
                    let top = cell.top + ICON_TOP;
                    target.DrawBitmap(
                        &bitmap,
                        Some(&rect_f(left, top, left + ICON_SIZE, top + ICON_SIZE)),
                        1.0,
                        D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                        None,
                    );
                }

                let label_top = cell.top + ICON_TOP + ICON_SIZE + 6.0;
                let label_bottom = (label_top + LABEL_LINES * LABEL_LINE_HEIGHT).min(cell.bottom);
                if label_bottom > label_top {
                    brush.SetColor(&color_f(scene.item_text));
                    target.DrawText(
                        label,
                        &graphics.label_format,
                        &rect_f(cell.left + 4.0, label_top, cell.right - 4.0, label_bottom),
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            }
        }
    }

    fn ensure_target(
        &mut self,
        graphics: &Graphics,
        hwnd: HWND,
    ) -> Result<(ID2D1HwndRenderTarget, ID2D1SolidColorBrush), PlatformError> {
        let mut client = RECT::default();
        // SAFETY: `hwnd` is the window being painted.
        unsafe { GetClientRect(hwnd, &mut client)? };
        let size = (
            (client.right - client.left).max(1) as u32,
            (client.bottom - client.top).max(1) as u32,
        );

        if let (Some(target), Some(brush)) = (&self.target, &self.brush) {
            if size != self.size {
                // SAFETY: resizing a live render target.
                unsafe {
                    target.Resize(&D2D_SIZE_U {
                        width: size.0,
                        height: size.1,
                    })?
                };
                self.size = size;
            }
            return Ok((target.clone(), brush.clone()));
        }

        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            // 96 DPI makes one DIP one physical pixel, matching the model's
            // pixel coordinates.
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: size.0,
                height: size.1,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        // SAFETY: property structs are fully initialised locals.
        let (target, brush) = unsafe {
            let target = graphics
                .d2d
                .CreateHwndRenderTarget(&properties, &hwnd_properties)?;
            target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            let brush = target.CreateSolidColorBrush(&color_f(Color::TRANSPARENT), None)?;
            (target, brush)
        };
        self.target = Some(target.clone());
        self.brush = Some(brush.clone());
        self.size = size;
        Ok((target, brush))
    }

    fn bitmap_for(&mut self, icon: &Icon) -> Option<ID2D1Bitmap> {
        if let Some(bitmap) = self.icons.get(&icon.id()) {
            return Some(bitmap.clone());
        }
        let target = self.target.as_ref()?;
        let properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };
        // SAFETY: `Icon::new` guarantees `pixels` holds width * height BGRA
        // pixels, so `width * 4` is the row pitch; D2D copies the data.
        let bitmap = unsafe {
            target
                .CreateBitmap(
                    D2D_SIZE_U {
                        width: icon.width(),
                        height: icon.height(),
                    },
                    Some(icon.pixels().as_ptr() as *const core::ffi::c_void),
                    icon.width() * 4,
                    &properties,
                )
                .ok()?
        };
        self.icons.insert(icon.id(), bitmap.clone());
        Some(bitmap)
    }

    /// Drops cached bitmaps for icons `scene` no longer shows, so the cache
    /// is bounded by what is on screen rather than by everything ever shown.
    pub fn retain_icons(&mut self, scene: &spaceScene) {
        if self.icons.is_empty() {
            return;
        }
        self.icons.retain(|id, _| {
            scene
                .items
                .iter()
                .any(|item| item.icon.as_ref().is_some_and(|icon| icon.id() == *id))
        });
    }

    fn release_device_resources(&mut self) {
        self.target = None;
        self.brush = None;
        self.icons.clear();
    }
}

/// Copies an `HICON` into an owned premultiplied-BGRA `Icon`. The caller
/// keeps ownership of `hicon`. Needs COM on this thread (for WIC).
pub(crate) fn decode_icon(hicon: HICON) -> Option<Icon> {
    let graphics = graphics().ok()?;
    // SAFETY: `hicon` is a live icon owned by the caller for this call; the
    // pixel buffer is sized from the converter's reported dimensions.
    unsafe {
        let source = graphics.wic.CreateBitmapFromHICON(hicon).ok()?;
        let converter = graphics.wic.CreateFormatConverter().ok()?;
        converter
            .Initialize(
                &source,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
            .ok()?;
        let (mut width, mut height) = (0u32, 0u32);
        converter.GetSize(&mut width, &mut height).ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        let mut pixels = vec![0u8; width as usize * height as usize * 4].into_boxed_slice();
        converter
            .CopyPixels(std::ptr::null(), width * 4, &mut pixels)
            .ok()?;
        Some(Icon::new(width, height, pixels))
    }
}

unsafe fn fill(
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: D2D_RECT_F,
    color: Color,
) {
    if color.a == 0 {
        return;
    }
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        brush.SetColor(&color_f(color));
        target.FillRectangle(&rect, brush);
    }
}

fn color_f(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r as f32 / 255.0,
        g: color.g as f32 / 255.0,
        b: color.b as f32 / 255.0,
        a: color.a as f32 / 255.0,
    }
}

fn rect_f(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

fn to_rect_f(rect: Rect) -> D2D_RECT_F {
    rect_f(
        rect.x as f32,
        rect.y as f32,
        rect.right() as f32,
        rect.bottom() as f32,
    )
}

fn inset(rect: D2D_RECT_F, amount: f32) -> D2D_RECT_F {
    rect_f(
        rect.left + amount,
        rect.top + amount,
        rect.right - amount,
        rect.bottom - amount,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_conversion_is_straight_alpha() {
        let converted = color_f(Color::rgba(255, 0, 51, 128));
        assert_eq!(converted.r, 1.0);
        assert_eq!(converted.g, 0.0);
        assert!((converted.b - 0.2).abs() < 1e-6);
        assert!((converted.a - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn inset_shrinks_every_edge() {
        let shrunk = inset(rect_f(0.0, 0.0, 10.0, 10.0), 2.0);
        assert_eq!(
            (shrunk.left, shrunk.top, shrunk.right, shrunk.bottom),
            (2.0, 2.0, 8.0, 8.0)
        );
    }
}
