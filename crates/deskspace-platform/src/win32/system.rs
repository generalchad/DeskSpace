use std::sync::OnceLock;
use std::time::Duration;

use deskspace_core::Color;
use deskspace_core::geometry::{Point, Rect, Size};
use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::Foundation::{ERROR_SUCCESS, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, GetDC, GetMonitorInfoW, GetTextFaceW, HDC, HFONT, HGDIOBJ, LOGFONTW,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, ReleaseDC, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, NONCLIENTMETRICSW, SM_CXDRAG, SM_CXVIRTUALSCREEN, SM_CYDRAG,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SPI_GETCLIENTAREAANIMATION,
    SPI_GETNONCLIENTMETRICS, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::{BOOL, PCWSTR, w};

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

/// Where Windows keeps the personalization switches DeskSpace follows.
const PERSONALIZE_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

/// A DWORD value under `HKEY_CURRENT_USER`, or `None` when it is missing or
/// not a DWORD.
fn user_dword(key: PCWSTR, name: PCWSTR) -> Option<u32> {
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value` and `size` are valid out-pointers sized for a DWORD.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS).then_some(value)
}

/// Whether Windows is set to dark mode for apps. There is no documented Win32
/// API for this; the Settings app writes `AppsUseLightTheme` (0 = dark) and
/// broadcasts `WM_SETTINGCHANGE` with "ImmersiveColorSet" when it changes.
pub fn system_uses_dark_mode() -> bool {
    user_dword(PERSONALIZE_KEY, w!("AppsUseLightTheme")) == Some(0)
}

/// Windows' "Transparency effects" switch (Settings > Personalization >
/// Colors): the `EnableTransparency` DWORD, or `None` when it is unset.
pub fn transparency_enabled() -> Option<bool> {
    user_dword(PERSONALIZE_KEY, w!("EnableTransparency")).map(|value| value != 0)
}

/// Windows' accent in the shade its own UI draws on a dark (Light 2) or light
/// (Dark 1) background, where it keeps its contrast; the raw accent can be
/// too dark or too pale for one of them. `None` when WinRT cannot say, for
/// example before COM is initialized on this thread.
/// <https://learn.microsoft.com/uwp/api/windows.ui.viewmanagement.uicolortype>
pub(crate) fn accent_shade(dark: bool) -> Option<Color> {
    thread_local! {
        // Process state: this thread's `UISettings`, kept because the settings
        // window asks on every appearance edit. The first call creates it; it
        // is never cleared.
        static SETTINGS: std::cell::RefCell<Option<UISettings>> =
            const { std::cell::RefCell::new(None) };
    }
    let kind = if dark {
        UIColorType::AccentLight2
    } else {
        UIColorType::AccentDark1
    };
    SETTINGS.with_borrow_mut(|settings| {
        if settings.is_none() {
            *settings = UISettings::new().ok();
        }
        let shade = settings.as_ref()?.GetColorValue(kind).ok()?;
        Some(Color::rgb(shade.R, shade.G, shade.B))
    })
}

/// Windows' accent color, read from the DWM registry value. The value is a
/// DWORD laid out `0xAABBGGRR`, and its alpha is often zero, so callers should
/// treat only the RGB as meaningful (`Palette::with_accent` keeps its own
/// alpha). Returns `None` when the value is missing or unreadable.
pub fn system_accent_color() -> Option<Color> {
    user_dword(w!(r"Software\Microsoft\Windows\DWM"), w!("AccentColor")).map(color_from_abgr)
}

/// Whether Windows' "Animation effects" switch is on (Settings >
/// Accessibility > Visual effects; on Windows 10, Ease of Access > Display),
/// which DeskSpace follows for every animation (ROADMAP D12). On when it
/// cannot be read, Windows' own default.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow
pub fn system_animations() -> bool {
    let mut on = BOOL::from(true);
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through `pvParam`.
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut on as *mut BOOL as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    result.is_err() || on.as_bool()
}

/// The first Windows 11 build.
pub(crate) const WINDOWS_11: u32 = 22000;

/// Whether this is Windows 11 or later, read once.
pub(crate) fn is_windows_11() -> bool {
    // Process state: whether this is Windows 11, read once because the build
    // cannot change under the process. First use fills it; never cleared.
    static WINDOWS_11_OR_LATER: OnceLock<bool> = OnceLock::new();
    *WINDOWS_11_OR_LATER.get_or_init(|| windows_build().is_some_and(|build| build >= WINDOWS_11))
}

/// The Windows build number (22000 and up is Windows 11), or `None` when it
/// cannot be read. It comes from the registry because `GetVersionExW`
/// reports the version the executable's manifest declares, not the real one.
pub fn windows_build() -> Option<u32> {
    let mut buffer = [0u16; 16];
    let mut size = std::mem::size_of_val(&buffer) as u32;
    // SAFETY: `buffer` and `size` describe a writable buffer of `size` bytes.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
            w!("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let length = buffer.iter().position(|&unit| unit == 0)?;
    String::from_utf16(&buffer[..length]).ok()?.parse().ok()
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

/// How long since the user last pressed a key or moved the mouse, in this
/// session.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo
pub fn idle_time() -> Duration {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is a valid out-pointer with `cbSize` set, as required.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return Duration::ZERO;
    }
    // SAFETY: a plain tick query. Both values are 32-bit tick counts that wrap
    // after 49.7 days, so the wrapping difference is still the idle time.
    let now = unsafe { GetTickCount() };
    Duration::from_millis(u64::from(now.wrapping_sub(info.dwTime)))
}

/// Whether the user is in a full-screen app, a presentation, or otherwise
/// "busy" by the shell's own notification rules, when DeskSpace should not
/// restart itself even if no input arrives.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shqueryusernotificationstate
pub fn user_is_presenting() -> bool {
    // SAFETY: a plain shell query with no arguments.
    let Ok(state) = (unsafe { SHQueryUserNotificationState() }) else {
        return false;
    };
    [
        QUNS_BUSY,
        QUNS_RUNNING_D3D_FULL_SCREEN,
        QUNS_PRESENTATION_MODE,
    ]
    .contains(&state)
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
/// the eyedropper. Returns a null font when neither is installed. Draw only
/// code points that both faces have; not every symbol is in both.
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
    // SAFETY: `dc` is the valid screen DC above and `font` a live font; the
    // previous object is selected back before the DC is released.
    let previous = unsafe { SelectObject(dc, HGDIOBJ(font.0)) };
    let mut name = [0u16; 32];
    // SAFETY: `name` is a writable buffer whose length the slice carries.
    let length = unsafe { GetTextFaceW(dc, Some(&mut name[..])) };
    // SAFETY: restores the DC's original font and releases the DC we got.
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
