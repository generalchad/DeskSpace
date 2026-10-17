use deskspace_core::geometry::Point;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, GetDpiForWindow,
    MDT_EFFECTIVE_DPI, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware;

pub fn enable_per_monitor_awareness() {
    // SAFETY: both calls only set the process DPI awareness mode and take no
    // pointers; the older API is the fallback when the newer one is absent.
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
            let _ = SetProcessDPIAware();
        }
    }
}

pub fn window_dpi(hwnd: HWND) -> u32 {
    // SAFETY: `hwnd` is a live window; the call only reads its DPI.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 96 } else { dpi }
}

/// The effective DPI of the monitor nearest `point` (screen coordinates), for
/// sizing a window before it is shown there.
/// https://learn.microsoft.com/en-us/windows/win32/api/shellscalingapi/nf-shellscalingapi-getdpiformonitor
pub fn monitor_dpi_at(point: Point) -> u32 {
    let (mut x, mut y) = (0, 0);
    // SAFETY: MonitorFromPoint always returns a monitor with
    // MONITOR_DEFAULTTONEAREST, and both out-pointers are valid locals.
    let result = unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: point.x,
                y: point.y,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y)
    };
    if result.is_err() || x == 0 { 96 } else { x }
}

pub fn scale_for(dpi: u32) -> f32 {
    dpi as f32 / 96.0
}
