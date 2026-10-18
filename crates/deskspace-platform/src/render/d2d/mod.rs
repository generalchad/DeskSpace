//! Direct2D + DirectWrite painter for space windows.
//!
//! Each space window owns a `Painter` with an `ID2D1HwndRenderTarget` using
//! premultiplied alpha. Because the window's frame is extended over the whole
//! client area (`dwm::apply_style`), DWM composites those alpha values over the
//! blur material: transparent pixels show the acrylic/blur material and the
//! tint is laid on top of it.
//! https://learn.microsoft.com/en-us/windows/win32/direct2d/supported-pixel-formats-and-alpha-modes

mod bitmap;
mod canvas;
mod edit;
mod geometry;
mod graphics;
mod item;
mod zoom_bar;

pub(crate) use bitmap::{decode_bitmap, decode_icon};
pub(crate) use zoom_bar::ZoomBarPainter;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use deskspace_core::geometry::Rect;
use deskspace_core::theme::{Color, EditColors, LABEL_SIZE_RANGE};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES, D2D1_DASH_STYLE_DOT,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
    D2D1_STROKE_STYLE_PROPERTIES, D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory,
    ID2D1HwndRenderTarget, ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_HIT_TEST_METRICS, DWRITE_LINE_METRICS, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_RANGE,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    IDWriteTextFormat, IDWriteTextLayout,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, HBITMAP, HPALETTE, PAINTSTRUCT};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmap, IWICImagingFactory,
    WICBitmapCacheOnLoad, WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut,
    WICBitmapUseAlpha,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::{BOOL, HRESULT, PCWSTR, w};
use windows_numerics::{Matrix3x2, Vector2};

use crate::edit::TextEdit;
use crate::error::PlatformError;
use crate::render::Icon;
use crate::render::scene::{ItemState, ItemVisual, SpaceScene};

use bitmap::to_icon;
use canvas::HwndCanvas;
use edit::draw_edit_title;
use geometry::{color_f, draw_text, fill_rounded, inset, rect_f, to_rect_f};
use graphics::{GRAPHICS, Graphics, graphics};
use item::{draw_item, draw_marquee};

const TITLE_INSET: f32 = 12.0;
const LABEL_LINE_HEIGHT: f32 = 16.0;
const SCROLL_THUMB_ALPHA: u8 = 110;
/// Opacity of an item's slot while the item itself is dragged.
const DRAGGED_ALPHA: u8 = 90;
/// Opacity of a cut item, shown ghosted until it is pasted, as Explorer does.
const CUT_ALPHA: u8 = 110;
/// Segoe Fluent Icons / Segoe MDL2 Assets "Lock" glyph.
/// https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
const LOCK_GLYPH: &[u16] = &[0xE72E];

/// D2DERR_RECREATE_TARGET: the device was lost and every device-dependent
/// resource must be recreated.
/// https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-error-codes
const D2DERR_RECREATE_TARGET: HRESULT = HRESULT(0x8899_000C_u32 as i32);

/// Releases the shared factories. Must run before `CoUninitialize`, because
/// thread-local destructors would otherwise release COM objects after COM is
/// gone.
pub(crate) fn release_graphics() {
    GRAPHICS.with(|cell| cell.borrow_mut().take());
}

/// The render target, brush, and shared factories every draw helper uses,
/// gathered once per paint (and once in `render_items`).
#[derive(Clone, Copy)]
pub(super) struct DrawContext<'a> {
    graphics: &'a Graphics,
    target: &'a ID2D1RenderTarget,
    brush: &'a ID2D1SolidColorBrush,
}

/// A scene with its strings pre-encoded, so painting does not allocate.
pub struct PreparedScene {
    scene: SpaceScene,
    title: Vec<u16>,
    /// The rename buffer while `scene.editing` is set.
    edit: Option<Vec<u16>>,
    /// The item id and rename buffer while `scene.editing_item` is set.
    edit_item: Option<(u64, Vec<u16>)>,
    labels: Vec<Vec<u16>>,
}

impl PreparedScene {
    pub fn new(scene: SpaceScene) -> Self {
        let title = scene.title.encode_utf16().collect();
        let edit = scene
            .editing
            .as_ref()
            .map(|edit| edit.text().encode_utf16().collect());
        let edit_item = scene
            .editing_item
            .as_ref()
            .map(|(id, edit)| (*id, edit.text().encode_utf16().collect()));
        let labels = scene
            .items
            .iter()
            .map(|item| item.label.encode_utf16().collect())
            .collect();
        Self {
            scene,
            title,
            edit,
            edit_item,
            labels,
        }
    }

    pub fn scene(&self) -> &SpaceScene {
        &self.scene
    }
}

#[derive(Default)]
pub struct Painter {
    canvas: HwndCanvas,
    /// GPU copies of `Icon`s, keyed by `Icon::id`.
    icons: HashMap<u64, ID2D1Bitmap>,
}

impl Painter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Paints `scene` in response to `WM_PAINT`.
    pub(crate) fn paint(
        &mut self,
        hwnd: HWND,
        scene: Option<&PreparedScene>,
        frame: PaintFrame,
    ) -> Result<(), PlatformError> {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: BeginPaint/EndPaint bracket the paint for this window; the
        // render target draws to the same HWND in between.
        unsafe { BeginPaint(hwnd, &mut ps) };
        let result = match scene {
            Some(scene) => self.render(hwnd, scene, frame),
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
        frame: PaintFrame,
    ) -> Result<(), PlatformError> {
        let graphics = graphics()?;
        let (target, brush) = self.canvas.ensure(&graphics, hwnd)?;
        let ctx = DrawContext {
            graphics: &graphics,
            target: &target,
            brush: &brush,
        };
        let scene = &prepared.scene;
        let (width, height) = self.canvas.size();
        let (width, height) = (width as f32, height as f32);
        let whole = rect_f(0.0, 0.0, width, height);
        let radius = frame.radius as f32;

        // SAFETY: all calls are on live COM objects owned by this painter,
        // between BeginDraw and EndDraw on the UI thread.
        let end = unsafe {
            target.BeginDraw();
            if frame.opaque {
                target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
                target.Clear(Some(&color_f(scene.tint.with_alpha(255))));
            } else {
                // ClearType needs an opaque background to blend against; on a
                // translucent surface it leaves colored fringes.
                target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
                target.Clear(Some(&color_f(Color::TRANSPARENT)));
                fill_rounded(&target, &brush, whole, radius, scene.tint);
            }

            // The title fill is the body's rounded shape cut at the title's
            // bottom edge, so its top corners follow the space's corners.
            let title_height = scene.title_height as f32;
            if scene.chrome {
                target.PushAxisAlignedClip(
                    &rect_f(0.0, 0.0, width, title_height.min(height)),
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
                fill_rounded(&target, &brush, whole, radius, scene.title_fill);
                target.PopAxisAlignedClip();
                self.draw_title(ctx, prepared, width, title_height);
            }

            // A chrome-less layer (the desktop layer) draws its items from the
            // top; a space leaves room for the title bar.
            let content_top = if scene.chrome { title_height } else { 0.0 };
            if !scene.folded && height > content_top {
                target.PushAxisAlignedClip(
                    &rect_f(0.0, content_top, width, height),
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
                self.draw_items(ctx, prepared);
                if let Some(marquee) = scene.marquee {
                    draw_marquee(
                        &target,
                        &brush,
                        marquee,
                        scene.marquee_color,
                        scene.show_marquee_border,
                    );
                }
                target.PopAxisAlignedClip();
                if scene.chrome
                    && let Some(thumb) = scene.scroll_thumb
                {
                    let thumb = to_rect_f(thumb);
                    let thumb_radius = (thumb.right - thumb.left) / 2.0;
                    fill_rounded(
                        &target,
                        &brush,
                        thumb,
                        thumb_radius,
                        scene.item_text.fade(SCROLL_THUMB_ALPHA),
                    );
                }
            }

            if frame.border && scene.border.a > 0 {
                brush.SetColor(&color_f(scene.border));
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

            // A 2 px outline just inside the frame marks the space selected in
            // the settings window; drawn last so it sits over the border.
            if scene.highlight {
                let gap = 1.5;
                brush.SetColor(&color_f(scene.selected_fill.with_alpha(255)));
                target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: inset(whole, gap),
                        radiusX: (radius - gap).max(0.0),
                        radiusY: (radius - gap).max(0.0),
                    },
                    &brush,
                    2.0,
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
        ctx: DrawContext,
        prepared: &PreparedScene,
        width: f32,
        title_height: f32,
    ) {
        let DrawContext {
            graphics,
            target,
            brush,
        } = ctx;
        let scene = &prepared.scene;
        let lock_width = if scene.locked { title_height } else { 0.0 };
        let text_right = (width - TITLE_INSET - lock_width).max(TITLE_INSET);
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            if scene.editing.is_some() {
                draw_edit_title(ctx, prepared, TITLE_INSET, text_right, title_height);
            } else {
                draw_text(
                    target,
                    brush,
                    &prepared.title,
                    &graphics.title_format,
                    rect_f(TITLE_INSET, 0.0, text_right, title_height),
                    scene.title_text,
                    scene.text_shadow,
                );
            }
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

    unsafe fn draw_items(&mut self, ctx: DrawContext, prepared: &PreparedScene) {
        let scene = &prepared.scene;
        let editing = prepared.edit_item.as_ref().map(|(id, _)| *id);
        for (item, label) in scene.items.iter().zip(&prepared.labels) {
            if Some(item.id) == editing {
                continue;
            }
            let bitmap = item.icon.as_ref().and_then(|icon| self.bitmap_for(icon));
            // SAFETY: caller is inside BeginDraw/EndDraw.
            unsafe { draw_item(ctx, scene, item, label, bitmap.as_ref(), None) };
        }
        // The rename box grows over the items below it, as Explorer's does, so
        // draw the item being renamed last.
        let Some((id, buffer)) = &prepared.edit_item else {
            return;
        };
        let Some((item, label)) = scene
            .items
            .iter()
            .zip(&prepared.labels)
            .find(|(item, _)| item.id == *id)
        else {
            return;
        };
        let bitmap = item.icon.as_ref().and_then(|icon| self.bitmap_for(icon));
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            draw_item(
                ctx,
                scene,
                item,
                label,
                bitmap.as_ref(),
                Some(buffer.as_slice()),
            )
        };
    }

    fn bitmap_for(&mut self, icon: &Icon) -> Option<ID2D1Bitmap> {
        if let Some(bitmap) = self.icons.get(&icon.id()) {
            return Some(bitmap.clone());
        }
        let target = self.canvas.target()?;
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
    pub fn retain_icons(&mut self, scene: &SpaceScene) {
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
        self.canvas.release();
        self.icons.clear();
    }
}

/// Renders the items `ids` of `scene` together into one premultiplied bitmap,
/// keeping their relative positions and scaling the whole group by `scale`
/// about `origin` (scene coordinates). Used for drag images, so a group drag
/// shows every selected item laid out as Explorer shows it. Needs COM on this
/// thread (for WIC).
pub(crate) fn render_items(
    scene: &SpaceScene,
    ids: &[u64],
    origin: Rect,
    scale: f32,
) -> Option<Icon> {
    let width = ((origin.width as f32 * scale).ceil() as u32).max(1);
    let height = ((origin.height as f32 * scale).ceil() as u32).max(1);
    let graphics = graphics().ok()?;
    let properties = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    // SAFETY: the WIC bitmap and its render target live for this call only;
    // drawing happens between BeginDraw and EndDraw, and the pixels are copied
    // out afterwards.
    unsafe {
        let pixels = graphics
            .wic
            .CreateBitmap(
                width,
                height,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )
            .ok()?;
        let target = graphics
            .d2d
            .CreateWicBitmapRenderTarget(&pixels, &properties)
            .ok()?;
        target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        // Transparent background: ClearType needs an opaque one.
        target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        let brush = target
            .CreateSolidColorBrush(&color_f(Color::TRANSPARENT), None)
            .ok()?;
        let bitmap_properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: properties.pixelFormat,
            dpiX: 96.0,
            dpiY: 96.0,
        };

        let ctx = DrawContext {
            graphics: &graphics,
            target: &target,
            brush: &brush,
        };
        let mut scene = scene.clone();
        scene.show_check_boxes = false;

        target.BeginDraw();
        target.Clear(Some(&color_f(Color::TRANSPARENT)));
        // Map scene coordinates into the canvas: p' = (p - origin) * scale.
        // https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-transforms-overview
        target.SetTransform(&Matrix3x2 {
            M11: scale,
            M12: 0.0,
            M21: 0.0,
            M22: scale,
            M31: -(origin.x as f32) * scale,
            M32: -(origin.y as f32) * scale,
        });
        for id in ids {
            let Some(index) = scene.items.iter().position(|item| item.id == *id) else {
                continue;
            };
            let mut item = scene.items[index].clone();
            // The drag image is the item itself, without the hover highlight,
            // cut ghosting, or a check box that only appears while pointing.
            item.state = ItemState::Normal;
            item.hovered = false;
            item.checked = false;
            item.cut = false;
            let label: Vec<u16> = item.label.encode_utf16().collect();
            let icon = item.icon.as_ref().and_then(|icon| {
                target
                    .CreateBitmap(
                        D2D_SIZE_U {
                            width: icon.width(),
                            height: icon.height(),
                        },
                        Some(icon.pixels().as_ptr() as *const core::ffi::c_void),
                        icon.width() * 4,
                        &bitmap_properties,
                    )
                    .ok()
            });
            draw_item(ctx, &scene, &item, &label, icon.as_ref(), None);
        }
        target.EndDraw(None, None).ok()?;
        drop(target);
        to_icon(&graphics, &pixels)
    }
}

/// How a space window's edge is drawn; decided by the window from its style
/// and placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PaintFrame {
    /// Desktop-hosted windows get no per-pixel alpha; paint them opaque.
    pub opaque: bool,
    /// Corner radius the painter rounds the body to; 0 when DWM rounds the
    /// window or the corners are square.
    pub radius: i32,
    /// Draw the border here because DWM does not.
    pub border: bool,
}
