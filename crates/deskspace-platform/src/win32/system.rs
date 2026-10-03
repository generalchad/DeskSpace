use deskspace_core::geometry::{Point, Rect, Size};
use windows::Win32::Foundation::{ERROR_SUCCESS, POINT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXDRAG, SM_CYDRAG};
use windows::core::w;

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

/// How far the mouse must move with a button down before it counts as a
/// drag rather than a click (the user's SM_CXDRAG / SM_CYDRAG setting).
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getsystemmetrics
pub fn drag_threshold() -> Size {
    // SAFETY: plain metric queries.
    let (x, y) = unsafe { (GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG)) };
    Size::new(x.max(1), y.max(1))
}

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
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
}
