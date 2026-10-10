use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
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

pub fn scale_for(dpi: u32) -> f32 {
    dpi as f32 / 96.0
}
