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
    /// The same sizes for an inline cell's single left-aligned line (W16).
    pub(super) inline_label_format: IDWriteTextFormat,
    pub(super) inline_label_formats: HashMap<u8, IDWriteTextFormat>,
    pub(super) glyph_format: Option<IDWriteTextFormat>,
    /// The zoom pop-up's percentage, and its button text (centered both ways).
    pub(super) zoom_percent_format: IDWriteTextFormat,
    pub(super) zoom_label_format: IDWriteTextFormat,
    /// A space's placeholder text: centered both ways, wrapping, and breaking
    /// a long path anywhere rather than running past the edge.
    pub(super) placeholder_format: IDWriteTextFormat,
    /// A dotted stroke for the keyboard focus rectangle.
    pub(super) focus_stroke: ID2D1StrokeStyle,
}

thread_local! {
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

impl Graphics {
    pub(super) fn new() -> Result<Self, PlatformError> {
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
            // icon, ellipsized when it runs past the cell (W16).
            let inline_label_format =
                make_format(&dwrite, text_family, DWRITE_FONT_WEIGHT_NORMAL, 12.0)?;
            inline_label_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
            inline_label_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            inline_label_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let inline_sign = dwrite.CreateEllipsisTrimmingSign(&inline_label_format)?;
            inline_label_format.SetTrimming(&character_trimming(), &inline_sign)?;
            let mut inline_label_formats = HashMap::new();
            for scale in (LABEL_SIZE_RANGE.0..=LABEL_SIZE_RANGE.1).step_by(10) {
                let format = make_format(
                    &dwrite,
                    text_family,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    12.0 * scale as f32 / 100.0,
                )?;
                format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
                format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                let sign = dwrite.CreateEllipsisTrimmingSign(&format)?;
                format.SetTrimming(&character_trimming(), &sign)?;
                inline_label_formats.insert(scale, format);
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

            let zoom_percent_format =
                centered_format(&dwrite, text_family, DWRITE_FONT_WEIGHT_SEMI_BOLD, 14.0)?;
            let zoom_label_format =
                centered_format(&dwrite, text_family, DWRITE_FONT_WEIGHT_NORMAL, 12.0)?;

            let placeholder_format =
                make_format(&dwrite, text_family, DWRITE_FONT_WEIGHT_NORMAL, 12.0)?;
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
                glyph_format,
                zoom_percent_format,
                zoom_label_format,
                placeholder_format,
                focus_stroke,
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

unsafe fn first_installed(fonts: &IDWriteFontCollection, families: &[PCWSTR]) -> Option<PCWSTR> {
    families.iter().copied().find(|family| {
        let mut index = 0;
        let mut exists = BOOL::default();
        // SAFETY: out-pointers are valid locals.
        unsafe { fonts.FindFamilyName(*family, &mut index, &mut exists) }.is_ok()
            && exists.as_bool()
    })
}
