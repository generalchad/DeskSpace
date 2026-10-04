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
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_HIT_TEST_METRICS, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmap, IWICImagingFactory,
    WICBitmapCacheOnLoad, WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, HICON};
use windows::core::{BOOL, HRESULT, PCWSTR, w};
use windows_numerics::Vector2;

use crate::edit::TextEdit;
use crate::error::PlatformError;
use crate::render::Icon;
use crate::render::scene::{ItemState, ItemVisual, SpaceScene};

const ICON_SIZE: f32 = super::ITEM_ICON_SIZE as f32;
const ICON_TOP: f32 = super::ITEM_ICON_TOP as f32;
const TITLE_INSET: f32 = 12.0;
const LABEL_LINE_HEIGHT: f32 = 16.0;
const SCROLL_THUMB_ALPHA: u8 = 110;
/// Opacity of an item's slot while the item itself is dragged.
const DRAGGED_ALPHA: u8 = 90;
/// Opacity of a cut item, shown ghosted until it is pasted, as Explorer does.
const CUT_ALPHA: u8 = 110;
/// Explorer's item check box, anchored at the icon's top-left corner.
const CHECK_BOX_SIZE: f32 = super::ITEM_CHECK_BOX_SIZE as f32;
const CHECK_BOX_MARGIN: f32 = super::ITEM_CHECK_BOX_MARGIN as f32;
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
    dwrite: IDWriteFactory,
    wic: IWICImagingFactory,
    title_format: IDWriteTextFormat,
    /// Like `title_format` but without ellipsis trimming, so the caret and
    /// selection positions match the text while renaming.
    edit_format: IDWriteTextFormat,
    label_format: IDWriteTextFormat,
    /// Label formats keyed by size percent, so scaling does not create a
    /// format on every paint.
    label_formats: HashMap<u8, IDWriteTextFormat>,
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

            let edit_format =
                make_format(&dwrite, text_family, DWRITE_FONT_WEIGHT_SEMI_BOLD, 13.0)?;
            edit_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            edit_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            edit_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;

            let label_format = make_format(&dwrite, text_family, DWRITE_FONT_WEIGHT_NORMAL, 12.0)?;
            label_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
            label_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            label_format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            let label_sign = dwrite.CreateEllipsisTrimmingSign(&label_format)?;
            label_format.SetTrimming(&character_trimming(), &label_sign)?;

            // One label format per size step; the painter rounds a configured
            // percent to the nearest step so it never formats during a paint.
            let mut label_formats = HashMap::new();
            for scale in (60..=160).step_by(10) {
                let format = make_format(
                    &dwrite,
                    text_family,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    12.0 * scale as f32 / 100.0,
                )?;
                format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
                format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
                let sign = dwrite.CreateEllipsisTrimmingSign(&format)?;
                format.SetTrimming(&character_trimming(), &sign)?;
                label_formats.insert(scale, format);
            }

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
                dwrite,
                wic,
                title_format,
                edit_format,
                label_format,
                label_formats,
                glyph_format,
            })
        }
    }

    /// The label format for a size percent, rounded to the nearest cached
    /// step (60..=160 by 10), falling back to the default size.
    fn label_format_for(&self, scale: u8) -> IDWriteTextFormat {
        let rounded = (scale.clamp(60, 160) + 5) / 10 * 10;
        self.label_formats
            .get(&rounded)
            .unwrap_or(&self.label_format)
            .clone()
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
        let (target, brush) = self.ensure_target(&graphics, hwnd)?;
        let scene = &prepared.scene;
        let width = self.size.0 as f32;
        let height = self.size.1 as f32;
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
            target.PushAxisAlignedClip(
                &rect_f(0.0, 0.0, width, title_height.min(height)),
                D2D1_ANTIALIAS_MODE_ALIASED,
            );
            fill_rounded(&target, &brush, whole, radius, scene.title_fill);
            target.PopAxisAlignedClip();
            self.draw_title(&graphics, &target, &brush, prepared, width, title_height);

            if !scene.rolled_up && height > title_height {
                target.PushAxisAlignedClip(
                    &rect_f(0.0, title_height, width, height),
                    D2D1_ANTIALIAS_MODE_ALIASED,
                );
                self.draw_items(&graphics, &target, &brush, prepared);
                if let Some(marquee) = scene.marquee {
                    draw_marquee(&target, &brush, marquee, scene.selected_fill);
                }
                target.PopAxisAlignedClip();
                if let Some(thumb) = scene.scroll_thumb {
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
        let text_right = (width - TITLE_INSET - lock_width).max(TITLE_INSET);
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            if scene.editing.is_some() {
                draw_edit_title(
                    graphics,
                    target,
                    brush,
                    prepared,
                    TITLE_INSET,
                    text_right,
                    title_height,
                );
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

    unsafe fn draw_items(
        &mut self,
        graphics: &Graphics,
        target: &ID2D1HwndRenderTarget,
        brush: &ID2D1SolidColorBrush,
        prepared: &PreparedScene,
    ) {
        let scene = &prepared.scene;
        for (item, label) in scene.items.iter().zip(&prepared.labels) {
            let bitmap = item.icon.as_ref().and_then(|icon| self.bitmap_for(icon));
            let edit = prepared
                .edit_item
                .as_ref()
                .filter(|(id, _)| *id == item.id)
                .map(|(_, buffer)| buffer.as_slice());
            // SAFETY: caller is inside BeginDraw/EndDraw.
            unsafe {
                draw_item(
                    graphics,
                    target,
                    brush,
                    scene,
                    item,
                    label,
                    bitmap.as_ref(),
                    edit,
                )
            };
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
        self.target = None;
        self.brush = None;
        self.icons.clear();
    }
}

/// Draws the rubber-band selection rectangle over empty space.
fn draw_marquee(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    marquee: Rect,
    fill: Color,
) {
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        let rect = to_rect_f(marquee);
        brush.SetColor(&color_f(fill.fade(70)));
        target.FillRectangle(&rect, brush);
        brush.SetColor(&color_f(fill.with_alpha(200)));
        target.DrawRectangle(&rect, brush, 1.0, None);
    }
}

/// Draws one item cell: highlight, icon, and label. Shared by the space
/// painter and `render_item`, so a drag image is exactly what the space shows.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_item(
    graphics: &Graphics,
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    scene: &SpaceScene,
    item: &ItemVisual,
    label: &[u16],
    icon: Option<&ID2D1Bitmap>,
    edit: Option<&[u16]>,
) {
    let cell = to_rect_f(item.rect);
    let highlight = match item.state {
        ItemState::Normal | ItemState::Dragged => None,
        ItemState::Hovered => Some(scene.hover_fill),
        ItemState::Selected => Some(scene.selected_fill),
    };
    let alpha = if item.state == ItemState::Dragged {
        DRAGGED_ALPHA
    } else if item.cut {
        CUT_ALPHA
    } else {
        255
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
        let icon_left = (center_x - ICON_SIZE / 2.0).round();
        if let Some(bitmap) = icon {
            let top = cell.top + ICON_TOP;
            target.DrawBitmap(
                bitmap,
                Some(&rect_f(
                    icon_left,
                    top,
                    icon_left + ICON_SIZE,
                    top + ICON_SIZE,
                )),
                alpha as f32 / 255.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
        // Explorer shows a check box while pointing at the item, and keeps it
        // while the item stays selected.
        if scene.show_check_boxes && (item.checked || item.hovered) {
            draw_check_box(target, brush, icon_left, cell.top + ICON_TOP, item.checked);
        }
        let label_top = cell.top + ICON_TOP + ICON_SIZE + 6.0;
        let scale = scene.label_size.clamp(60, 160) as f32 / 100.0;
        let lines = scene.label_lines.clamp(1, 2) as f32;
        let line_height = LABEL_LINE_HEIGHT * scale;
        let label_bottom = (label_top + lines * line_height).min(cell.bottom);
        if label_bottom > label_top {
            let label_rect = rect_f(cell.left + 4.0, label_top, cell.right - 4.0, label_bottom);
            let format = graphics.label_format_for(scene.label_size);
            if let (Some(buffer), Some((_, text_edit))) = (edit, &scene.editing_item) {
                draw_edit_buffer(
                    graphics,
                    target,
                    brush,
                    text_edit,
                    buffer,
                    &format,
                    label_rect,
                    scene.selected_fill,
                    scene.item_text.fade(alpha),
                    scene.caret_visible,
                );
            } else {
                draw_text(
                    target,
                    brush,
                    label,
                    &format,
                    label_rect,
                    scene.item_text.fade(alpha),
                    scene.text_shadow.map(|shadow| shadow.fade(alpha)),
                );
            }
        }
    }
}

/// Draws Explorer's item check box at the top-left of an icon. A light box
/// with a dark border stays visible on any backdrop; `checked` adds the tick.
unsafe fn draw_check_box(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    icon_left: f32,
    icon_top: f32,
    checked: bool,
) {
    let left = icon_left + CHECK_BOX_MARGIN;
    let top = icon_top + CHECK_BOX_MARGIN;
    let rect = rect_f(left, top, left + CHECK_BOX_SIZE, top + CHECK_BOX_SIZE);
    let rounded = D2D1_ROUNDED_RECT {
        rect,
        radiusX: 2.5,
        radiusY: 2.5,
    };
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        brush.SetColor(&color_f(Color::rgba(255, 255, 255, 210)));
        target.FillRoundedRectangle(&rounded, brush);
        brush.SetColor(&color_f(Color::rgba(0, 0, 0, 150)));
        target.DrawRoundedRectangle(&rounded, brush, 1.0, None);
        if checked {
            brush.SetColor(&color_f(Color::rgba(0, 0, 0, 255)));
            let s = CHECK_BOX_SIZE;
            let stroke = 1.4;
            target.DrawLine(
                Vector2::new(left + s * 0.22, top + s * 0.52),
                Vector2::new(left + s * 0.43, top + s * 0.74),
                brush,
                stroke,
                None,
            );
            target.DrawLine(
                Vector2::new(left + s * 0.43, top + s * 0.74),
                Vector2::new(left + s * 0.80, top + s * 0.26),
                brush,
                stroke,
                None,
            );
        }
    }
}

/// Renders item `id` of `scene` on its own, exactly as the space draws it
/// (state, icon, label, colors), into premultiplied pixels the size of its
/// cell. Used for drag images. Needs COM on this thread (for WIC).
pub(crate) fn render_item(scene: &SpaceScene, id: u64) -> Option<Icon> {
    let index = scene.items.iter().position(|item| item.id == id)?;
    let mut item = scene.items[index].clone();
    let width = item.rect.width.max(1) as u32;
    let height = item.rect.height.max(1) as u32;
    item.rect = Rect::new(0, 0, item.rect.width, item.rect.height);
    let label: Vec<u16> = item.label.encode_utf16().collect();
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
        let icon = item.icon.as_ref().and_then(|icon| {
            target
                .CreateBitmap(
                    D2D_SIZE_U {
                        width: icon.width(),
                        height: icon.height(),
                    },
                    Some(icon.pixels().as_ptr() as *const core::ffi::c_void),
                    icon.width() * 4,
                    &D2D1_BITMAP_PROPERTIES {
                        pixelFormat: properties.pixelFormat,
                        dpiX: 96.0,
                        dpiY: 96.0,
                    },
                )
                .ok()
        });
        target.BeginDraw();
        target.Clear(Some(&color_f(Color::TRANSPARENT)));
        draw_item(
            &graphics,
            &target,
            &brush,
            scene,
            &item,
            &label,
            icon.as_ref(),
            None,
        );
        target.EndDraw(None, None).ok()?;
        drop(target);
        to_icon(&graphics, &pixels)
    }
}

/// Copies an `HICON` into an owned premultiplied-BGRA `Icon`. The caller
/// keeps ownership of `hicon`. Needs COM on this thread (for WIC).
pub(crate) fn decode_icon(hicon: HICON) -> Option<Icon> {
    let graphics = graphics().ok()?;
    // SAFETY: `hicon` is a live icon owned by the caller for this call.
    let source = unsafe { graphics.wic.CreateBitmapFromHICON(hicon).ok()? };
    to_icon(&graphics, &source)
}

/// Converts any WIC source to top-down premultiplied BGRA pixels.
fn to_icon(graphics: &Graphics, source: &IWICBitmap) -> Option<Icon> {
    // SAFETY: the pixel buffer is sized from the converter's reported
    // dimensions; `source` is a live WIC bitmap.
    unsafe {
        let converter = graphics.wic.CreateFormatConverter().ok()?;
        converter
            .Initialize(
                source,
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

unsafe fn fill_rounded(
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: D2D_RECT_F,
    radius: f32,
    color: Color,
) {
    if color.a == 0 {
        return;
    }
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        brush.SetColor(&color_f(color));
        if radius > 0.0 {
            target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                brush,
            );
        } else {
            target.FillRectangle(&rect, brush);
        }
    }
}

/// Draws clipped text, first offset by one pixel in `shadow` when given.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_text(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    text: &[u16],
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    color: Color,
    shadow: Option<Color>,
) {
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        if let Some(shadow) = shadow {
            brush.SetColor(&color_f(shadow));
            target.DrawText(
                text,
                format,
                &rect_f(
                    rect.left + 1.0,
                    rect.top + 1.0,
                    rect.right + 1.0,
                    rect.bottom + 1.0,
                ),
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        brush.SetColor(&color_f(color));
        target.DrawText(
            text,
            format,
            &rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
}

/// Draws the title rename buffer in the title bar.
unsafe fn draw_edit_title(
    graphics: &Graphics,
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
    prepared: &PreparedScene,
    left: f32,
    right: f32,
    title_height: f32,
) {
    let scene = &prepared.scene;
    let Some(edit) = &scene.editing else {
        return;
    };
    let Some(buffer) = prepared.edit.as_ref() else {
        return;
    };
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        draw_edit_buffer(
            graphics,
            target,
            brush,
            edit,
            buffer,
            &graphics.edit_format,
            rect_f(left, 0.0, right, title_height),
            scene.selected_fill,
            scene.title_text,
            scene.caret_visible,
        );
    }
}

/// Draws an editable buffer with its selection highlight and caret. The text
/// layout is reused to place the caret and selection, so they match
/// DirectWrite's own shaping.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_edit_buffer(
    graphics: &Graphics,
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    edit: &TextEdit,
    buffer: &[u16],
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    fill: Color,
    text: Color,
    caret_visible: bool,
) {
    let width = (rect.right - rect.left).max(1.0);
    let height = (rect.bottom - rect.top).max(1.0);
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        let Ok(layout) = graphics
            .dwrite
            .CreateTextLayout(buffer, format, width, height)
        else {
            draw_text(target, brush, buffer, format, rect, text, None);
            return;
        };
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        let (start, end) = edit.selection();
        if start != end {
            let start16 = utf16_index(edit.text(), start);
            let end16 = utf16_index(edit.text(), end);
            let (mut x0, mut y0) = (0.0f32, 0.0f32);
            let (mut x1, mut y1) = (0.0f32, 0.0f32);
            let _ = layout.HitTestTextPosition(start16, false, &mut x0, &mut y0, &mut metrics);
            let line_height = metrics.height.max(1.0);
            let _ = layout.HitTestTextPosition(end16, false, &mut x1, &mut y1, &mut metrics);
            brush.SetColor(&color_f(fill));
            if y1 > y0 {
                // Wrapped selection: cover the first line to the end, the last
                // line from the start, and everything between.
                target.FillRectangle(
                    &rect_f(
                        rect.left + x0,
                        rect.top + y0,
                        rect.right,
                        rect.top + y0 + line_height,
                    ),
                    brush,
                );
                target.FillRectangle(
                    &rect_f(
                        rect.left,
                        rect.top + y1,
                        rect.left + x1,
                        rect.top + y1 + metrics.height,
                    ),
                    brush,
                );
                if y1 - y0 > line_height {
                    target.FillRectangle(
                        &rect_f(
                            rect.left,
                            rect.top + y0 + line_height,
                            rect.right,
                            rect.top + y1,
                        ),
                        brush,
                    );
                }
            } else {
                target.FillRectangle(
                    &rect_f(
                        rect.left + x0,
                        rect.top + y0,
                        rect.left + x1,
                        rect.top + y0 + line_height,
                    ),
                    brush,
                );
            }
        }
        draw_text(target, brush, buffer, format, rect, text, None);
        if caret_visible {
            let caret16 = utf16_index(edit.text(), edit.caret());
            let (mut cx, mut cy) = (0.0f32, 0.0f32);
            let _ = layout.HitTestTextPosition(caret16, false, &mut cx, &mut cy, &mut metrics);
            let line_height = metrics.height.max(1.0);
            target.FillRectangle(
                &rect_f(
                    rect.left + cx,
                    rect.top + cy,
                    rect.left + cx + 1.0,
                    rect.top + cy + line_height,
                ),
                brush,
            );
        }
    }
}

/// A byte offset in the text, counted in UTF-16 code units for DirectWrite.
/// Clamps to the text length if the offset is not on a boundary.
fn utf16_index(text: &str, byte: usize) -> u32 {
    text.get(..byte).unwrap_or(text).encode_utf16().count() as u32
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
