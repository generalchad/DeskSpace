//! Direct2D + DirectWrite painter for space windows.
//!
//! Each space window owns a `Painter` that draws with premultiplied alpha,
//! on an `ID2D1HwndRenderTarget` or on the composition drawing surface the
//! window shows (`canvas.rs`). Either way the window's pixels are composited
//! over the blur material: transparent pixels show the acrylic/blur material
//! and the tint is laid on top of it.
//! https://learn.microsoft.com/en-us/windows/win32/direct2d/supported-pixel-formats-and-alpha-modes

mod badge;
mod bitmap;
mod canvas;
mod color_picker;
mod drag_image;
mod edit;
mod effects;
mod geometry;
mod glyph;
mod graphics;
mod item;
mod prepared;
mod title;
mod zoom_bar;

pub(crate) use badge::debug_badge;
pub(crate) use bitmap::{decode_bitmap, decode_icon};
pub(crate) use color_picker::ColorPickerPainter;
pub(crate) use drag_image::render_items;
pub(crate) use glyph::rasterize;
pub use prepared::PreparedScene;
pub(crate) use zoom_bar::ZoomBarPainter;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use deskspace_core::geometry::Rect;
use deskspace_core::theme::{Color, EditColors, LABEL_SIZE_RANGE};
use windows::UI::Composition::{CompositionDrawingSurface, CompositionGraphicsDevice};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    D2D1_BITMAP_PROPERTIES, D2D1_DASH_STYLE_DOT, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
    D2D1_STROKE_STYLE_PROPERTIES, D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Bitmap, ID2D1Brush, ID2D1Device,
    ID2D1Factory, ID2D1HwndRenderTarget, ID2D1Layer, ID2D1RenderTarget, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_HIT_TEST_METRICS, DWRITE_LINE_METRICS, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TEXT_RANGE, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory,
    IDWriteFontCollection, IDWriteTextFormat, IDWriteTextLayout,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, HBITMAP, HPALETTE, InvalidateRect, PAINTSTRUCT,
};
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
use crate::render::scene::{ColumnAlign, ItemState, ItemVisual, SpaceScene};
use crate::render::transition::{Content, TransitionFrame};

use bitmap::to_icon;
use canvas::{Canvas, HwndCanvas, SurfaceCanvas};
use effects::Effects;
use geometry::{
    PIXEL_DPI, PREMULTIPLIED_BGRA, bitmap_properties, color_f, draw_text, fill_rounded, inset,
    rect_f, render_target_properties, to_rect_f, upload_icon,
};
use graphics::{GRAPHICS, Graphics, graphics};
use item::{draw_item, draw_marquee};

const LABEL_LINE_HEIGHT: f32 = 16.0;
const SCROLL_THUMB_ALPHA: u8 = 110;
/// The placeholder text's alpha factor: about 60 % of the label color.
const PLACEHOLDER_ALPHA: u8 = 153;
/// Space kept between the placeholder text and the item area's edges.
const PLACEHOLDER_INSET: f32 = 12.0;
/// Opacity of an item's slot while the item itself is dragged.
const DRAGGED_ALPHA: u8 = 90;
/// Opacity of a cut item, shown ghosted until it is pasted, as Explorer does.
const CUT_ALPHA: u8 = 110;
/// CRT's bright line at full glow: a cool white, slightly translucent so the
/// line still reads as light over the contents.
const CRT_GLOW: Color = Color::rgba(0xF0, 0xF6, 0xFF, 0xE6);
/// The outline marking the space selected in the settings window: its gap
/// inside the frame and its width.
const HIGHLIGHT_GAP: f32 = 1.5;
const HIGHLIGHT_WIDTH: f32 = 2.0;
/// Segoe Fluent Icons / Segoe MDL2 Assets "ChevronUp" and "ChevronDown", the
/// sorted Details header's arrow.
const SORT_UP_GLYPH: &[u16] = &[0xE70E];
const SORT_DOWN_GLYPH: &[u16] = &[0xE70D];

/// D2DERR_RECREATE_TARGET: the device was lost and every device-dependent
/// resource must be recreated.
/// https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-error-codes
const D2DERR_RECREATE_TARGET: HRESULT = HRESULT(0x8899_000C_u32 as i32);

/// A new Direct2D device for composition drawing surfaces (`Painter::on_surface`),
/// from the shared factory.
pub(crate) fn create_device() -> Result<ID2D1Device, PlatformError> {
    graphics()?.create_device()
}

/// Whether `code` means the device is gone, and every device resource with
/// it: Direct2D's own error, or the Direct3D device under a composition
/// surface removed or reset.
fn is_device_lost(code: HRESULT) -> bool {
    [
        D2DERR_RECREATE_TARGET,
        DXGI_ERROR_DEVICE_REMOVED,
        DXGI_ERROR_DEVICE_RESET,
    ]
    .contains(&code)
}

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

/// What one frame of a transition is drawn with, made before the window's
/// draw begins.
struct FrameResources {
    /// The whole space's size, which the window may crop.
    size: (f32, f32),
    /// The layer the contents go through, with the frame's opacity.
    layer: ID2D1Layer,
    /// The frame's mask, if it has one.
    mask: Option<ID2D1Brush>,
    /// Pixelate's small scene, and the part of it the scene covers.
    pixelated: Option<(ID2D1Bitmap, D2D_RECT_F)>,
    /// Burn's glow, char, and scorch, laid over the contents.
    burn_marks: Option<ID2D1Bitmap>,
}

/// Clears `target` for a new frame: to the opaque tint on a window with no
/// per-pixel alpha, else to transparent.
///
/// # Safety
/// Call between `BeginDraw` and `EndDraw` on `target`.
unsafe fn clear(target: &ID2D1RenderTarget, opaque: bool, tint: Color) {
    // SAFETY: the caller is inside BeginDraw/EndDraw on `target`.
    unsafe {
        if opaque {
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
            target.Clear(Some(&color_f(tint.with_alpha(255))));
        } else {
            // ClearType needs an opaque background to blend against; on a
            // translucent surface it leaves colored fringes.
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            target.Clear(Some(&color_f(Color::TRANSPARENT)));
        }
    }
}

#[derive(Default)]
pub struct Painter {
    canvas: Canvas,
    /// GPU copies of `Icon`s, keyed by `Icon::id`.
    icons: HashMap<u64, ID2D1Bitmap>,
    /// What an animation's frames are drawn with.
    effects: Effects,
    /// The last title cut from its start, so a repaint measures nothing.
    title_cut: Option<title::TitleCut>,
}

impl Painter {
    pub fn new() -> Self {
        Self::default()
    }

    /// A painter that draws into `surface`, the composition drawing surface
    /// its window shows (`win32::composition`), made on `device`, instead of
    /// the window, and an animation's frames into `blur_mask` too.
    pub(crate) fn on_surface(
        surface: &CompositionDrawingSurface,
        blur_mask: &CompositionDrawingSurface,
        device: &CompositionGraphicsDevice,
    ) -> Result<Self, PlatformError> {
        Ok(Self {
            canvas: Canvas::Surface(SurfaceCanvas::new(surface, blur_mask, device)?),
            ..Self::default()
        })
    }

    /// Paints `scene` in response to `WM_PAINT`, through `transition`'s
    /// frame while one is running (ROADMAP D12).
    pub(crate) fn paint(
        &mut self,
        hwnd: HWND,
        scene: Option<&PreparedScene>,
        frame: PaintFrame,
        transition: Option<&TransitionFrame>,
    ) -> Result<(), PlatformError> {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: BeginPaint/EndPaint bracket the paint for this window; the
        // render target draws to the same HWND in between.
        unsafe { BeginPaint(hwnd, &mut ps) };
        let result = match scene {
            Some(scene) => self.render(hwnd, scene, frame, transition),
            None => Ok(()),
        };
        // SAFETY: matches the BeginPaint above.
        unsafe {
            let _ = EndPaint(hwnd, &ps);
        }
        result
    }

    /// Renders, and starts over with new device resources when the device
    /// was lost.
    fn render(
        &mut self,
        hwnd: HWND,
        prepared: &PreparedScene,
        frame: PaintFrame,
        transition: Option<&TransitionFrame>,
    ) -> Result<(), PlatformError> {
        match self.draw(hwnd, prepared, frame, transition) {
            Err(PlatformError::Win32(error)) if is_device_lost(error.code()) => {
                tracing::debug!("direct2d device lost; recreating on next paint");
                self.release_device_resources(hwnd);
                Ok(())
            }
            result => result,
        }
    }

    fn draw(
        &mut self,
        hwnd: HWND,
        prepared: &PreparedScene,
        frame: PaintFrame,
        transition: Option<&TransitionFrame>,
    ) -> Result<(), PlatformError> {
        let graphics = graphics()?;
        let (resources, brush) = self.canvas.ensure(&graphics, hwnd)?;
        let ctx = DrawContext {
            graphics: &graphics,
            target: &resources,
            brush: &brush,
        };
        // The space is drawn whole even while the window around it is smaller
        // (a roll crops it, a fly clips it at the screen's edge).
        let size = match transition {
            Some(transition) => (
                transition.space.width as f32,
                transition.space.height as f32,
            ),
            None => {
                let (width, height) = self.canvas.size();
                (width as f32, height as f32)
            }
        };
        let animated = match transition {
            Some(transition) => {
                let resources = self.frame_resources(ctx, prepared, frame, transition, size)?;
                Some((transition, resources))
            }
            None => None,
        };

        let (target, origin) = self.canvas.begin()?;
        let ctx = DrawContext {
            target: &target,
            ..ctx
        };
        // SAFETY: all calls are on live COM objects owned by this painter,
        // between the canvas's begin and end on the UI thread.
        unsafe {
            target.SetTransform(&Matrix3x2::translation(origin.X, origin.Y));
            clear(&target, frame.opaque, prepared.scene.tint);
            match &animated {
                Some((transition, resources)) => {
                    self.draw_transition(ctx, prepared, frame, transition, resources, origin)
                }
                None => self.draw_scene(ctx, prepared, frame, size),
            }
        }
        self.canvas.end()?;
        if let Some((transition, resources)) = animated
            && !transition.covers_window()
        {
            self.draw_blur_mask(transition, resources, &brush)?;
        }
        Ok(())
    }

    /// What `transition`'s frame is drawn with, updated for this frame.
    /// Pixelate draws the scene small on its own target here, since that
    /// must finish before the window's target begins.
    fn frame_resources(
        &mut self,
        window: DrawContext,
        prepared: &PreparedScene,
        frame: PaintFrame,
        transition: &TransitionFrame,
        size: (f32, f32),
    ) -> Result<FrameResources, PlatformError> {
        let pixelated = match transition.content {
            Content::Pixelated { block } => {
                Some(self.draw_small(window, prepared, frame, size, block)?)
            }
            Content::Whole | Content::Lit { .. } => None,
        };
        Ok(FrameResources {
            size,
            layer: self.effects.layer(window.target)?,
            mask: self
                .effects
                .mask_brush(window.target, transition.mask, size)?,
            pixelated,
            burn_marks: self.effects.burn_marks(transition.mask),
        })
    }

    /// Draws the space through `transition`'s frame: the contents (or
    /// Pixelate's blocks) at the frame's offset through its opacity and
    /// mask, then Burn's marks and CRT's glow over them.
    ///
    /// # Safety
    /// Call between `BeginDraw` and `EndDraw` on `ctx.target`.
    unsafe fn draw_transition(
        &mut self,
        ctx: DrawContext,
        prepared: &PreparedScene,
        frame: PaintFrame,
        transition: &TransitionFrame,
        resources: &FrameResources,
        origin: Vector2,
    ) {
        let DrawContext { target, brush, .. } = ctx;
        let size = resources.size;
        let whole = rect_f(0.0, 0.0, size.0, size.1);
        let offset = transition.content_offset;
        // SAFETY: the caller is inside BeginDraw/EndDraw on `target`.
        unsafe {
            target.SetTransform(&Matrix3x2::translation(
                origin.X + offset.x as f32,
                origin.Y + offset.y as f32,
            ));
            let mask = resources.mask.clone();
            effects::with_layer(target, &resources.layer, transition.opacity, mask, || {
                match &resources.pixelated {
                    Some((bitmap, source)) => target.DrawBitmap(
                        bitmap,
                        Some(&whole),
                        1.0,
                        D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                        Some(source),
                    ),
                    None => self.draw_scene(ctx, prepared, frame, size),
                }
            });
            if let Some(marks) = &resources.burn_marks {
                target.DrawBitmap(
                    marks,
                    Some(&whole),
                    transition.opacity,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
            if let Content::Lit { glow } = transition.content {
                let alpha = (glow * f32::from(CRT_GLOW.a)).round() as u8;
                brush.SetColor(&color_f(CRT_GLOW.with_alpha(alpha)));
                target.FillRectangle(&whole, brush);
            }
        }
    }

    /// Draws how much of the blur `transition`'s frame leaves (T13): the
    /// space's whole rect through the same opacity and mask as its contents.
    /// Pixelate's blocks are not drawn; the blur only fades with them.
    fn draw_blur_mask(
        &mut self,
        transition: &TransitionFrame,
        resources: FrameResources,
        brush: &ID2D1SolidColorBrush,
    ) -> windows::core::Result<()> {
        let offset = transition.content_offset;
        let FrameResources {
            size, layer, mask, ..
        } = resources;
        self.canvas.draw_blur_mask(|target, origin| {
            // SAFETY: inside the mask surface's draw, on live objects made on
            // its device.
            unsafe {
                target.SetTransform(&Matrix3x2::translation(
                    origin.X + offset.x as f32,
                    origin.Y + offset.y as f32,
                ));
                effects::with_layer(target, &layer, transition.opacity, mask, || {
                    brush.SetColor(&color_f(Color::rgba(255, 255, 255, 255)));
                    target.FillRectangle(&rect_f(0.0, 0.0, size.0, size.1), brush);
                });
            }
        })
    }

    /// Draws the scene at 1 / `block` of `size` on Pixelate's offscreen
    /// target, made from `window`'s, and returns that bitmap with the part of
    /// it the scene covers.
    fn draw_small(
        &mut self,
        window: DrawContext,
        prepared: &PreparedScene,
        frame: PaintFrame,
        size: (f32, f32),
        block: u32,
    ) -> Result<(ID2D1Bitmap, D2D_RECT_F), PlatformError> {
        let offscreen = self.effects.pixel_target(window.target, size)?;
        let (scale, source) = effects::pixelated(size, block);
        let ctx = DrawContext {
            target: &offscreen,
            ..window
        };
        // SAFETY: drawing on a live offscreen target between its own
        // BeginDraw and EndDraw, before the window's target begins.
        unsafe {
            offscreen.BeginDraw();
            offscreen.Clear(Some(&color_f(Color::TRANSPARENT)));
            offscreen.SetTransform(&Matrix3x2::scale(scale.0, scale.1));
            self.draw_scene(ctx, prepared, frame, size);
            offscreen.SetTransform(&Matrix3x2::identity());
            offscreen.EndDraw(None, None)?;
            Ok((offscreen.GetBitmap()?, source))
        }
    }

    /// Draws the whole space, `size` large, on `ctx`'s target under its
    /// current transform: the tint (unless the window is opaque, which its
    /// clear already filled), the title bar, the items, the border, and the
    /// highlight.
    ///
    /// # Safety
    /// Call between `BeginDraw` and `EndDraw` on `ctx.target`.
    unsafe fn draw_scene(
        &mut self,
        ctx: DrawContext,
        prepared: &PreparedScene,
        frame: PaintFrame,
        (width, height): (f32, f32),
    ) {
        let DrawContext { target, brush, .. } = ctx;
        let scene = &prepared.scene;
        let whole = rect_f(0.0, 0.0, width, height);
        let radius = frame.radius as f32;

        // SAFETY: the caller is inside BeginDraw/EndDraw on `target`.
        unsafe {
            if !frame.opaque {
                fill_rounded(target, brush, whole, radius, scene.tint);
            }

            // The title fill is the body's rounded shape cut at the title's
            // bottom edge, so its top corners follow the space's corners.
            let title_height = scene.title_height as f32;
            if scene.chrome {
                target.PushAxisAlignedClip(
                    &rect_f(0.0, 0.0, width, title_height.min(height)),
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
                fill_rounded(target, brush, whole, radius, scene.title_fill);
                target.PopAxisAlignedClip();
                title::draw_title(self, ctx, prepared, width, title_height);
            }

            // A chrome-less layer (the desktop layer) draws its items from the
            // top; a space leaves room for the title bar. Details leaves the
            // fixed header row above the items (W18).
            let content_top = if scene.chrome { title_height } else { 0.0 };
            let header_height = scene
                .details
                .as_ref()
                .map_or(0.0, |details| details.header_height as f32);
            let status_height = scene
                .status
                .as_ref()
                .map_or(0.0, |status| status.height as f32);
            let items_top = content_top + header_height;
            let content_bottom = (height - status_height).max(items_top);
            if !scene.folded && content_bottom > items_top {
                target.PushAxisAlignedClip(
                    &rect_f(0.0, items_top, width, content_bottom),
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
                self.draw_items(ctx, prepared);
                if let Some(text) = &prepared.placeholder {
                    draw_text(
                        target,
                        brush,
                        text,
                        &ctx.graphics.placeholder_format,
                        inset(
                            rect_f(0.0, items_top, width, content_bottom),
                            PLACEHOLDER_INSET,
                        ),
                        scene.item_text.fade(PLACEHOLDER_ALPHA),
                        scene
                            .text_shadow
                            .map(|shadow| shadow.fade(PLACEHOLDER_ALPHA)),
                    );
                }
                if let Some(marquee) = scene.marquee {
                    draw_marquee(
                        target,
                        brush,
                        marquee,
                        scene.marquee_color,
                        scene.show_marquee_border,
                    );
                }
                target.PopAxisAlignedClip();
                if scene.details.is_some() {
                    self.draw_details_header(ctx, prepared, width, content_top);
                }
                if scene.chrome
                    && let Some(thumb) = scene.scroll_thumb
                {
                    let thumb = to_rect_f(thumb);
                    let thumb_radius = (thumb.right - thumb.left) / 2.0;
                    fill_rounded(
                        target,
                        brush,
                        thumb,
                        thumb_radius,
                        scene.item_text.fade(SCROLL_THUMB_ALPHA),
                    );
                }
            }

            if let Some(status) = &scene.status {
                self.draw_status(ctx, prepared, status);
            }

            if let Some(border) = frame.border
                && scene.border.a > 0
            {
                // The stroke is centred on its rect, so a half-width inset
                // and radius keep its outer edge on the window's.
                let half = border.width / 2.0;
                let radius = (frame.outline_radius - half).max(0.0);
                brush.SetColor(&color_f(scene.border));
                target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: inset(whole, half),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    brush,
                    border.width,
                    None,
                );
            }

            // Drawn last, so the settings window's highlight sits over the
            // border.
            if scene.highlight {
                let radius = (frame.outline_radius - HIGHLIGHT_GAP).max(0.0);
                brush.SetColor(&color_f(scene.selected_fill.with_alpha(255)));
                target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: inset(whole, HIGHLIGHT_GAP),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    brush,
                    HIGHLIGHT_WIDTH,
                    None,
                );
            }
        }
    }

    /// Draws the Details header row at `top`: each column's label, Explorer's
    /// sort arrow on the sorted column, and a rule along its bottom (W18).
    unsafe fn draw_details_header(
        &self,
        ctx: DrawContext,
        prepared: &PreparedScene,
        width: f32,
        top: f32,
    ) {
        let Some(details) = prepared.scene.details.as_ref() else {
            return;
        };
        let DrawContext {
            graphics,
            target,
            brush,
        } = ctx;
        let scene = &prepared.scene;
        let height = details.header_height as f32;
        let pad = details.cell_pad as f32;
        let bottom = top + height;
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            brush.SetColor(&color_f(scene.item_text.fade(80)));
            target.DrawLine(
                Vector2::new(0.0, bottom - 0.5),
                Vector2::new(width, bottom - 0.5),
                brush,
                1.0,
                None,
            );
            for (index, (column, label)) in details.columns.iter().zip(&prepared.header).enumerate()
            {
                let mut right = column.rect.right() as f32 - pad;
                if let Some((_, up)) = details.sorted.filter(|(sorted, _)| *sorted == index)
                    && let Some(format) = &graphics.glyph_format
                {
                    let glyph_width = height * 0.6;
                    brush.SetColor(&color_f(scene.item_text.fade(200)));
                    target.DrawText(
                        if up { SORT_UP_GLYPH } else { SORT_DOWN_GLYPH },
                        format,
                        &rect_f(right - glyph_width, top, right, bottom),
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                    right -= glyph_width;
                }
                draw_text(
                    target,
                    brush,
                    label,
                    &graphics.details_format,
                    rect_f(column.rect.x as f32 + pad, top, right, bottom),
                    scene.item_text.fade(190),
                    scene.text_shadow,
                );
            }
        }
    }

    /// Draws Explorer's status bar: the counts on the left and the view
    /// buttons on the right, with the space's current view marked (W19).
    unsafe fn draw_status(
        &self,
        ctx: DrawContext,
        prepared: &PreparedScene,
        status: &crate::render::scene::StatusBar,
    ) {
        let DrawContext {
            graphics,
            target,
            brush,
        } = ctx;
        let scene = &prepared.scene;
        let rect = to_rect_f(status.rect);
        let height = rect.bottom - rect.top;
        let pad = (height * 0.25).round();
        let text_right = status
            .buttons
            .first()
            .map_or(rect.right - pad, |button| button.rect.x as f32 - pad);
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            brush.SetColor(&color_f(scene.item_text.fade(60)));
            target.DrawLine(
                Vector2::new(rect.left, rect.top + 0.5),
                Vector2::new(rect.right, rect.top + 0.5),
                brush,
                1.0,
                None,
            );
            draw_text(
                target,
                brush,
                &prepared.status_segments,
                &graphics.details_format,
                rect_f(rect.left + pad, rect.top, text_right, rect.bottom),
                scene.item_text.fade(200),
                scene.text_shadow,
            );
            for button in &status.buttons {
                let button_rect = to_rect_f(button.rect);
                if button.checked || button.hovered {
                    let radius = (button_rect.bottom - button_rect.top) * 0.2;
                    fill_rounded(
                        target,
                        brush,
                        button_rect,
                        radius,
                        if button.hovered {
                            scene.hover_fill
                        } else {
                            scene.selected_fill
                        },
                    );
                }
                if let Some(format) = &graphics.glyph_format {
                    brush.SetColor(&color_f(scene.item_text.fade(if button.checked {
                        255
                    } else {
                        200
                    })));
                    target.DrawText(
                        button.glyph,
                        format,
                        &button_rect,
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            }
        }
    }

    unsafe fn draw_items(&mut self, ctx: DrawContext, prepared: &PreparedScene) {
        let scene = &prepared.scene;
        let editing = prepared.edit_item.as_ref().map(|(id, _)| *id);
        for ((item, label), cells) in scene
            .items
            .iter()
            .zip(&prepared.labels)
            .zip(&prepared.cells)
        {
            if Some(item.id) == editing {
                continue;
            }
            let bitmap = item
                .icon
                .as_ref()
                .and_then(|icon| self.bitmap_for(ctx.target, icon));
            // SAFETY: caller is inside BeginDraw/EndDraw.
            unsafe { draw_item(ctx, scene, item, label, cells, bitmap.as_ref(), None) };
        }
        // The rename box grows over the items below it, as Explorer's does, so
        // draw the item being renamed last.
        let Some((id, buffer)) = &prepared.edit_item else {
            return;
        };
        let Some(((item, label), cells)) = scene
            .items
            .iter()
            .zip(&prepared.labels)
            .zip(&prepared.cells)
            .find(|((item, _), _)| item.id == *id)
        else {
            return;
        };
        let bitmap = item
            .icon
            .as_ref()
            .and_then(|icon| self.bitmap_for(ctx.target, icon));
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            draw_item(
                ctx,
                scene,
                item,
                label,
                cells,
                bitmap.as_ref(),
                Some(buffer.as_slice()),
            )
        };
    }

    /// `icon` on the GPU, uploaded through `target` the first time.
    pub(super) fn bitmap_for(
        &mut self,
        target: &ID2D1RenderTarget,
        icon: &Icon,
    ) -> Option<ID2D1Bitmap> {
        if let Some(bitmap) = self.icons.get(&icon.id()) {
            return Some(bitmap.clone());
        }
        let bitmap = upload_icon(target, icon)?;
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
                .title_icon
                .as_ref()
                .and_then(|title| title.image.as_ref())
                .is_some_and(|icon| icon.id() == *id)
                || scene
                    .items
                    .iter()
                    .any(|item| item.icon.as_ref().is_some_and(|icon| icon.id() == *id))
        });
    }

    fn release_device_resources(&mut self, hwnd: HWND) {
        self.canvas.release(hwnd);
        self.icons.clear();
        self.effects.release();
    }
}

/// How a space window's edge is drawn; decided by the window from its style
/// and placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PaintFrame {
    /// Desktop-hosted windows get no per-pixel alpha; paint them opaque.
    pub opaque: bool,
    /// Corner radius the painter rounds the body to; 0 when DWM rounds the
    /// window or the corners are square.
    pub radius: i32,
    /// The radius of the window's visible corners, in window pixels, which
    /// the border and the highlight follow. It can differ from `radius`:
    /// where DWM rounds the window, the body is not rounded but the outlines
    /// follow DWM's corners.
    pub outline_radius: f32,
    pub border: Option<BorderStroke>,
}

/// The border a painter draws, in window pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BorderStroke {
    pub width: f32,
}
