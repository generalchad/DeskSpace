use deskspace_core::Color;
use deskspace_core::geometry::{Point, Rect, Size};
use windows::Win32::Foundation::{ERROR_SUCCESS, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, GetDC, GetMonitorInfoW, GetTextFaceW, HDC, HFONT, HGDIOBJ, LOGFONTW,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, ReleaseDC, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, NONCLIENTMETRICSW, SM_CXDRAG, SM_CXVIRTUALSCREEN, SM_CYDRAG,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SPI_GETNONCLIENTMETRICS,
};
use windows::core::{PCWSTR, w};

/// The work area (screen minus taskbar and docked bars) of the monitor
/// nearest `point`.
pub fn work_area_at(point: Point) -> Rect {
    let fallback = Rect::new(0, 0, 1920, 1040);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid MONITORINFO with cbSize set.
    let found = unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: point.x,
                y: point.y,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        GetMonitorInfoW(monitor, &mut info).as_bool()
    };
    if !found {
        return fallback;
    }
    let work = info.rcWork;
    Rect::new(
        work.left,
        work.top,
        work.right - work.left,
        work.bottom - work.top,
    )
}

/// The pointer's position in virtual-desktop coordinates. Falls back to the
/// origin when the pointer cannot be read.
pub fn cursor_pos() -> Point {
    let mut point = POINT::default();
    // SAFETY: `point` is a valid out-pointer for GetCursorPos to fill.
    if unsafe { GetCursorPos(&mut point) }.is_err() {
        return Point::ZERO;
    }
    Point::new(point.x, point.y)
}

/// Whether Windows is set to dark mode for apps. There is no documented Win32
/// API for this; the Settings app writes `AppsUseLightTheme` (0 = dark) and
/// broadcasts `WM_SETTINGCHANGE` with "ImmersiveColorSet" when it changes.
pub fn system_uses_dark_mode() -> bool {
    let mut value: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value` and `size` are valid out-pointers sized for a DWORD.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    status == ERROR_SUCCESS && value == 0
}

/// Windows' accent color, read from the DWM registry value. The value is a
/// DWORD laid out `0xAABBGGRR`, and its alpha is often zero, so callers should
/// treat only the RGB as meaningful (`Palette::with_accent` keeps its own
/// alpha). Returns `None` when the value is missing or unreadable.
pub fn system_accent_color() -> Option<Color> {
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value` and `size` are valid out-pointers sized for a DWORD.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\DWM"),
            w!("AccentColor"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS {
        Some(color_from_abgr(value))
    } else {
        None
    }
}

/// Splits a DWM accent DWORD (`0xAABBGGRR`) into a `Color`.
fn color_from_abgr(value: u32) -> Color {
    Color::rgba(
        (value & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
        ((value >> 24) & 0xFF) as u8,
    )
}

/// How far the mouse must move with a button down before it counts as a
/// drag rather than a click (the user's SM_CXDRAG / SM_CYDRAG setting).
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getsystemmetrics
pub fn drag_threshold() -> Size {
    // SAFETY: plain metric queries.
    let (x, y) = unsafe { (GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG)) };
    Size::new(x.max(1), y.max(1))
}

/// The bounding rectangle of every monitor in virtual-desktop coordinates,
/// whose origin can be negative when a monitor is left of or above the
/// primary. The desktop layer spans it.
pub fn virtual_desktop_rect() -> Rect {
    // SAFETY: plain metric queries.
    let (x, y, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    Rect::new(x, y, width.max(1), height.max(1))
}

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The system message font (`Segoe UI 9pt` on Windows 11) at `dpi`, from
/// `SPI_GETNONCLIENTMETRICS`. The settings window and the color picker build
/// their fonts from it, so both match native windows.
pub(crate) fn message_logfont(dpi: u32) -> LOGFONTW {
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    // SAFETY: `metrics` is sized for SPI_GETNONCLIENTMETRICS.
    unsafe {
        let _ = SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some(&mut metrics as *mut _ as *mut core::ffi::c_void),
            0,
            dpi,
        );
    }
    metrics.lfMessageFont
}

/// A symbol font (`Segoe Fluent Icons`, or `Segoe MDL2 Assets` on older
/// Windows) at the message font's size, for glyphs such as the check mark and
/// the eyedropper. Returns a null font when neither is installed. Callers
/// should still check a glyph exists (`glyph_present`) before drawing it,
/// because not every symbol is in both fonts.
pub(crate) fn glyph_font(dpi: u32) -> HFONT {
    let height = message_logfont(dpi).lfHeight;
    for face in [w!("Segoe Fluent Icons"), w!("Segoe MDL2 Assets")] {
        // SAFETY: creating a font from an initialised LOGFONT.
        let font = unsafe { create_symbol_font(face, height) };
        if font.is_invalid() {
            continue;
        }
        // SAFETY: `font` is a live font of this thread.
        if unsafe { face_is(font, face) } {
            return font;
        }
        // SAFETY: the substituted font is ours to delete.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
    HFONT::default()
}

unsafe fn create_symbol_font(face: PCWSTR, height: i32) -> HFONT {
    let mut logfont = LOGFONTW {
        lfHeight: height,
        ..Default::default()
    };
    // SAFETY: `face` is a static, NUL-terminated wide string.
    let name = unsafe { face.as_wide() };
    for (slot, ch) in logfont.lfFaceName.iter_mut().zip(name) {
        *slot = *ch;
    }
    // SAFETY: `logfont` is fully initialised and lives for the call.
    unsafe { CreateFontIndirectW(&logfont) }
}

/// Whether `font` actually resolved to `expected` (GDI substitutes a default
/// face for a missing family instead of failing).
unsafe fn face_is(font: HFONT, expected: PCWSTR) -> bool {
    // SAFETY: a screen DC is used only to query the selected face.
    let dc = unsafe { GetDC(None) };
    if dc.is_invalid() {
        return false;
    }
    let previous = unsafe { SelectObject(dc, HGDIOBJ(font.0)) };
    let mut name = [0u16; 32];
    let length = unsafe { GetTextFaceW(dc, Some(&mut name[..])) };
    unsafe {
        SelectObject(dc, previous);
        ReleaseDC(None, dc);
    }
    let length = (length.max(0) as usize).saturating_sub(1);
    // SAFETY: `expected` is a static, NUL-terminated wide string.
    let expected = unsafe { expected.as_wide() };
    &name[..length.min(name.len())] == expected
}

/// Draws symbol `glyph` from `font` centered in `rect`, in `color`. The font
/// rendering is anti-aliased, so this is used for the picker's check mark and
/// the settings window's eyedropper. Does nothing if `font` is null.
pub(crate) unsafe fn draw_glyph(hdc: HDC, font: HFONT, glyph: u16, rect: RECT, color: Color) {
    if font.is_invalid() {
        return;
    }
    // SAFETY: `hdc` is a live DC; the state is restored before returning.
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        let cref = windows::Win32::Foundation::COLORREF(
            color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16),
        );
        SetTextColor(hdc, cref);
        let previous = SelectObject(hdc, HGDIOBJ(font.0));
        let mut text = [glyph];
        let mut rect = rect;
        let _ = DrawTextW(
            hdc,
            &mut text,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(hdc, previous);
    }
}

/// Equivalent of GET_X_LPARAM / GET_Y_LPARAM: the `as i16` sign-extends, so
/// coordinates left of or above the origin (possible while the mouse is
/// captured, and on multi-monitor setups) stay negative. LOWORD/HIWORD would
/// turn them into large positive values.
/// https://learn.microsoft.com/en-us/windows/win32/api/windowsx/nf-windowsx-get_x_lparam
pub(crate) fn loword(value: isize) -> i32 {
    (value & 0xFFFF) as i16 as i32
}

pub(crate) fn hiword(value: isize) -> i32 {
    ((value >> 16) & 0xFFFF) as i16 as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_extraction_sign_extends() {
        let packed = ((-5i32 as u16 as isize) << 16) | (-20i32 as u16 as isize);
        assert_eq!(loword(packed), -20);
        assert_eq!(hiword(packed), -5);
    }

    #[test]
    fn wide_strings_are_nul_terminated() {
        assert_eq!(wide("ab"), vec![b'a' as u16, b'b' as u16, 0]);
    }

    #[test]
    fn accent_dword_is_abgr() {
        assert_eq!(
            color_from_abgr(0xFF_00_80_FF),
            Color::rgba(0xFF, 0x80, 0x00, 0xFF)
        );
        assert_eq!(
            color_from_abgr(0x00_11_22_33),
            Color::rgba(0x33, 0x22, 0x11, 0x00)
        );
    }
}
