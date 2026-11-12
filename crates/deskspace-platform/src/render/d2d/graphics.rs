//! Shared Direct2D, DirectWrite, and WIC factories and the text formats
//! built from them, cached per UI thread.

use windows::Win32::Foundation::{E_POINTER, HMODULE};
use windows::Win32::Graphics::Direct2D::ID2D1Factory1;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_WORD_WRAPPING_EMERGENCY_BREAK;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::Interface;

use super::*;
use crate::render::ItemLayout;
use crate::win32::system::wide;
use crate::win32::typography;
use deskspace_core::typography::Role;

/// Device-independent resources shared by every painter on the UI thread.
pub(super) struct Graphics {
    pub(super) d2d: ID2D1Factory,
    pub(super) dwrite: IDWriteFactory,
    pub(super) wic: IWICImagingFactory,
    pub(super) title_format: IDWriteTextFormat,
    /// Like `title_format` but without ellipsis trimming, so the caret and
    /// selection positions match the text while renaming.
    pub(super) edit_format: IDWriteTextFormat,
    pub(super) label_format: IDWriteTextFormat,
    /// Label formats keyed by size percent, so scaling does not create a
    /// format on every paint.
    pub(super) label_formats: HashMap<u8, IDWriteTextFormat>,
    /// The same sizes for an inline cell's single left-aligned line.
    pub(super) inline_label_format: IDWriteTextFormat,
    pub(super) inline_label_formats: HashMap<u8, IDWriteTextFormat>,
    /// The Details row and header text: one left-aligned and one
    /// right-aligned line, centered vertically.
    pub(super) details_format: IDWriteTextFormat,
    pub(super) details_right_format: IDWriteTextFormat,
    pub(super) glyph_format: Option<IDWriteTextFormat>,
    /// The zoom pop-up's percentage, and its button text (centered both ways).
    pub(super) zoom_percent_format: IDWriteTextFormat,
    pub(super) zoom_label_format: IDWriteTextFormat,
    /// A space's placeholder text: centered both ways, wrapping, and breaking
    /// a long path anywhere rather than running past the edge.
    pub(super) placeholder_format: IDWriteTextFormat,
    /// A dotted stroke for the keyboard focus rectangle.
    pub(super) focus_stroke: ID2D1StrokeStyle,
    /// The UI text family, and the icon font when one is installed, for
    /// painters that make their own formats.
    pub(super) text_family: PCWSTR,
    pub(super) glyph_family: Option<PCWSTR>,
}

thread_local! {
    // Process state: the Direct2D factories and brushes, shared by every
    // painter on this thread. Windows' window procedures get no context
    // pointer. `graphics` fills it and `release_graphics` drops it before OLE.
    pub(super) static GRAPHICS: RefCell<Option<Rc<Graphics>>> = const { RefCell::new(None) };
}

pub(super) fn graphics() -> Result<Rc<Graphics>, PlatformError> {
    if let Some(existing) = GRAPHICS.with(|cell| cell.borrow().clone()) {
        return Ok(existing);
    }
    let created = Rc::new(Graphics::new()?);
    GRAPHICS.with(|cell| *cell.borrow_mut() = Some(created.clone()));
    Ok(created)
}

/// Builds every text format again from the type scale in force, keeping this
/// thread's factories.
///
/// The factories must outlive a type-scale change. The composition device,
/// and each window's render target, were made from this thread's Direct2D
/// factory when they were first needed. A stroke style from a new factory
/// made every draw of a focused item fail with `D2DERR_WRONG_FACTORY`, and a
/// composition surface reports nothing, so the window went on showing an old
/// frame, another space's, or nothing. Every startup sets the type scale
/// after composition has made its device, so it happened on every run.
pub(crate) fn rebuild_text_formats() {
    GRAPHICS.with(|cell| {
        // DirectWrite sends no window messages, so nothing re-enters this
        // borrow.
        let mut kept = cell.borrow_mut();
        let Some(current) = kept.as_ref() else {
            return;
        };
        match current.with_formats_rebuilt() {
            Ok(rebuilt) => *kept = Some(Rc::new(rebuilt)),
            Err(error) => {
                tracing::warn!(%error, "could not rebuild the text formats; keeping the old ones");
            }
        }
    });
}

impl Graphics {
    pub(super) fn new() -> Result<Self, PlatformError> {
        // SAFETY: plain factory creation; COM is initialised on this thread by
        // `runtime::run` before any window paints.
        let (d2d, dwrite, wic) = unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                Some(&D2D1_FACTORY_OPTIONS::default()),
            )?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let wic: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            (d2d, dwrite, wic)
        };
        Self::with_factories(d2d, dwrite, wic)
    }

    /// These factories, with every format made again from the type scale in
    /// force (`rebuild_text_formats`).
    fn with_formats_rebuilt(&self) -> Result<Self, PlatformError> {
        Self::with_factories(self.d2d.clone(), self.dwrite.clone(), self.wic.clone())
    }

    /// The formats and stroke styles made from `d2d` and `dwrite`, from the
    /// type scale in force.
    fn with_factories(
        d2d: ID2D1Factory,
        dwrite: IDWriteFactory,
        wic: IWICImagingFactory,
    ) -> Result<Self, PlatformError> {
        // SAFETY: plain DirectWrite and Direct2D resource creation on live
        // factories.
        unsafe {
            let mut fonts = None;
            dwrite.GetSystemFontCollection(&mut fonts, false)?;
            let fonts = fonts.ok_or(PlatformError::InvalidFormat("system font collection"))?;
            let (text_family, default_family) =
                if typography::is_installed(&fonts, w!("Segoe UI Variable Text")) {
                    (w!("Segoe UI Variable Text"), "Segoe UI Variable Text")
                } else {
                    (w!("Segoe UI"), "Segoe UI")
                };
            let glyph_family =
                first_installed(&fonts, &[w!("Segoe Fluent Icons"), w!("Segoe MDL2 Assets")]);

            let title_format = role_format(&dwrite, Role::SpaceTitle, 1.0, default_family)?;
            title_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            title_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            title_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let title_sign = dwrite.CreateEllipsisTrimmingSign(&title_format)?;
            title_format.SetTrimming(&character_trimming(), &title_sign)?;

            let edit_format = role_format(&dwrite, Role::SpaceTitle, 1.0, default_family)?;
            edit_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            edit_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            edit_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;

            let label_format = role_format(&dwrite, Role::ItemLabel, 1.0, default_family)?;
            label_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
            label_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            label_format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            let label_sign = dwrite.CreateEllipsisTrimmingSign(&label_format)?;
            label_format.SetTrimming(&character_trimming(), &label_sign)?;

            // One label format per size step; the painter rounds a configured
            // percent to the nearest step so it never formats during a paint.
            let mut label_formats = HashMap::new();
            for scale in (LABEL_SIZE_RANGE.0..=LABEL_SIZE_RANGE.1).step_by(10) {
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

            // An inline cell (Small icons) has one left-aligned line beside its
            // icon, ellipsized when it runs past the cell.
            let inline_label_format = role_format(&dwrite, Role::ItemLabel, 1.0, default_family)?;
            inline_label_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
            inline_label_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            inline_label_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let inline_sign = dwrite.CreateEllipsisTrimmingSign(&inline_label_format)?;
            inline_label_format.SetTrimming(&character_trimming(), &inline_sign)?;
            let mut inline_label_formats = HashMap::new();
            for scale in (LABEL_SIZE_RANGE.0..=LABEL_SIZE_RANGE.1).step_by(10) {
                let format = role_format(
                    &dwrite,
                    Role::ItemLabel,
                    scale as f32 / 100.0,
                    default_family,
                )?;
                format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
                format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                let sign = dwrite.CreateEllipsisTrimmingSign(&format)?;
                format.SetTrimming(&character_trimming(), &sign)?;
                inline_label_formats.insert(scale, format);
            }

            // A Details row and header are single, vertically centered lines;
            // values can be left or right aligned.
            let details_format = role_format(&dwrite, Role::Details, 1.0, default_family)?;

            details_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            details_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            details_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let details_sign = dwrite.CreateEllipsisTrimmingSign(&details_format)?;
            details_format.SetTrimming(&character_trimming(), &details_sign)?;
            let details_right_format = role_format(&dwrite, Role::Details, 1.0, default_family)?;

            details_right_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            details_right_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
            details_right_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let details_right_sign = dwrite.CreateEllipsisTrimmingSign(&details_right_format)?;
            details_right_format.SetTrimming(&character_trimming(), &details_right_sign)?;

            let glyph_format = match glyph_family {
                Some(family) => {
                    let format = make_format(&dwrite, family, DWRITE_FONT_WEIGHT_NORMAL, 12.0)?;
                    format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                    format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                    Some(format)
                }
                None => None,
            };

            let zoom_percent_format = centered_role_format(&dwrite, Role::Zoom, default_family)?;

            let zoom_label_format = centered_role_format(&dwrite, Role::Body, default_family)?;

            let placeholder_format = role_format(&dwrite, Role::Placeholder, 1.0, default_family)?;
            placeholder_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            placeholder_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            placeholder_format.SetWordWrapping(DWRITE_WORD_WRAPPING_EMERGENCY_BREAK)?;

            let focus_stroke = d2d.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    dashStyle: D2D1_DASH_STYLE_DOT,
                    ..Default::default()
                },
                None,
            )?;

            Ok(Self {
                d2d,
                dwrite,
                wic,
                title_format,
                edit_format,
                label_format,
                label_formats,
                inline_label_format,
                inline_label_formats,
                details_format,
                details_right_format,
                glyph_format,
                zoom_percent_format,
                zoom_label_format,
                placeholder_format,
                focus_stroke,
                text_family,
                glyph_family,
            })
        }
    }

    /// A Direct2D device over a new Direct3D 11 device, for composition
    /// drawing surfaces. It comes from this factory, as every resource the
    /// painters draw with must. The hardware device, else WARP (the
    /// software rasterizer), as when no GPU driver is installed.
    /// https://learn.microsoft.com/en-us/windows/win32/direct2d/devices-and-device-contexts
    pub(super) fn create_device(&self) -> Result<ID2D1Device, PlatformError> {
        let factory: ID2D1Factory1 = self.d2d.cast()?;
        let d3d = create_d3d_device(D3D_DRIVER_TYPE_HARDWARE)
            .or_else(|_| create_d3d_device(D3D_DRIVER_TYPE_WARP))?;
        let dxgi: IDXGIDevice = d3d.cast()?;
        // SAFETY: `dxgi` is a live device made for Direct2D (BGRA support).
        Ok(unsafe { factory.CreateDevice(&dxgi)? })
    }

    /// The label format for a size percent and item layout, rounded to the
    /// nearest cached step, falling back to the default size.
    pub(super) fn item_label_format_for(&self, scale: u8, layout: ItemLayout) -> IDWriteTextFormat {
        let rounded = (scale.clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1) + 5) / 10 * 10;
        let (formats, fallback) = match layout {
            ItemLayout::Grid => (&self.label_formats, &self.label_format),
            ItemLayout::Inline => (&self.inline_label_formats, &self.inline_label_format),
            ItemLayout::Details | ItemLayout::Tiles | ItemLayout::Content => {
                return self.details_format.clone();
            }
        };
        formats.get(&rounded).unwrap_or(fallback).clone()
    }
}

/// Direct2D draws only on a Direct3D device made with BGRA support.
fn create_d3d_device(driver: D3D_DRIVER_TYPE) -> windows::core::Result<ID3D11Device> {
    let mut device = None;
    // SAFETY: the out-pointer is a valid local; no adapter or module.
    unsafe {
        D3D11CreateDevice(
            None,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;
    }
    device.ok_or_else(|| E_POINTER.into())
}

fn character_trimming() -> DWRITE_TRIMMING {
    DWRITE_TRIMMING {
        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
        delimiter: 0,
        delimiterCount: 0,
    }
}

/// `role`'s format at `scale` times its configured size. The family is the
/// role's own, or `default_family` when it names none.
fn role_format(
    dwrite: &IDWriteFactory,
    role: Role,
    scale: f32,
    default_family: &str,
) -> Result<IDWriteTextFormat, PlatformError> {
    let typography = typography::current();
    let font = typography.font(role);
    let family = wide(if font.family.is_empty() {
        default_family
    } else {
        font.family.as_str()
    });
    // SAFETY: `family` outlives the call, which copies what it needs.
    unsafe {
        make_format(
            dwrite,
            PCWSTR(family.as_ptr()),
            DWRITE_FONT_WEIGHT(i32::from(font.weight)),
            font.size * scale,
        )
    }
}

pub(super) unsafe fn make_format(
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

/// A one-line format centered horizontally and vertically.
unsafe fn centered_format(
    dwrite: &IDWriteFactory,
    family: PCWSTR,
    weight: DWRITE_FONT_WEIGHT,
    size: f32,
) -> Result<IDWriteTextFormat, PlatformError> {
    // SAFETY: as in `make_format`.
    unsafe {
        let format = make_format(dwrite, family, weight, size)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        Ok(format)
    }
}

/// `role`'s format, centered both ways.
fn centered_role_format(
    dwrite: &IDWriteFactory,
    role: Role,
    default_family: &str,
) -> Result<IDWriteTextFormat, PlatformError> {
    let typography = typography::current();
    let font = typography.font(role);
    let family = wide(if font.family.is_empty() {
        default_family
    } else {
        font.family.as_str()
    });
    // SAFETY: `family` outlives the call.
    unsafe {
        centered_format(
            dwrite,
            PCWSTR(family.as_ptr()),
            DWRITE_FONT_WEIGHT(i32::from(font.weight)),
            font.size,
        )
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

#[cfg(test)]
mod tests {
    use windows::Win32::Graphics::Direct2D::{
        D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
        ID2D1Image,
    };
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    use super::*;
    use crate::render::d2d::geometry::{PIXEL_DPI, PREMULTIPLIED_BGRA};

    #[test]
    fn a_type_scale_change_keeps_drawing_on_a_device_made_before_it() {
        // SAFETY: initialises COM on the test thread; a repeat call is
        // harmless.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        // Made first, as composition makes its device at startup.
        let device = create_device().expect("a device");
        let before = graphics().expect("graphics");

        rebuild_text_formats();
        let after = graphics().expect("graphics");
        assert!(!Rc::ptr_eq(&before, &after), "the formats are new");

        // SAFETY: drawing on a live context and bitmap made here, between
        // its own BeginDraw and EndDraw.
        let drawn = unsafe {
            let context = device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
                .expect("a context");
            let target = context
                .CreateBitmap(
                    D2D_SIZE_U {
                        width: 16,
                        height: 16,
                    },
                    None,
                    0,
                    &D2D1_BITMAP_PROPERTIES1 {
                        pixelFormat: PREMULTIPLIED_BGRA,
                        dpiX: PIXEL_DPI,
                        dpiY: PIXEL_DPI,
                        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                        ..Default::default()
                    },
                )
                .expect("a target");
            context.SetTarget(Some(&target.cast::<ID2D1Image>().expect("an image")));
            let brush = context
                .CreateSolidColorBrush(&color_f(Color::rgb(255, 0, 0)), None)
                .expect("a brush");
            context.BeginDraw();
            // The focus rectangle, the one draw that needs the factory.
            context.DrawRectangle(
                &rect_f(1.0, 1.0, 15.0, 15.0),
                &brush,
                1.0,
                Some(&after.focus_stroke),
            );
            let drawn = context.EndDraw(None, None);
            context.SetTarget(None::<&ID2D1Image>);
            drawn
        };
        release_graphics();
        assert_eq!(drawn, Ok(()), "the focus stroke draws on the old device");
    }
}
