use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{FALSE, HWND, LPARAM, TRUE, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, SMTO_NORMAL, SendMessageTimeoutW,
};
use windows::core::{BOOL, PCWSTR, w};

/// Undocumented `Progman` message that makes Explorer create the wallpaper
/// `WorkerW` window, which hosted spaces use as a parent. Sent with a bounded
/// timeout so a future Explorer that stops answering it cannot hang us.
const SPAWN_WORKER_MESSAGE: u32 = 0x052C;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    #[default]
    TopLevel,
    DesktopBackground,
    DesktopForeground,
}

impl Placement {
    pub fn is_desktop_hosted(self) -> bool {
        !matches!(self, Self::TopLevel)
    }
}

static FOUND_WORKER: AtomicIsize = AtomicIsize::new(0);

pub fn find_host(placement: Placement) -> Option<HWND> {
    match placement {
        Placement::TopLevel => None,
        Placement::DesktopForeground => progman(),
        Placement::DesktopBackground => wallpaper_worker(),
    }
}

/// Whether `window` is the desktop shell (`Progman` or the wallpaper
/// `WorkerW`), i.e. a press there is on the desktop background or its icons.
pub(crate) fn is_desktop_window(window: HWND) -> bool {
    let mut class = [0u16; 32];
    // SAFETY: `class` is a valid buffer; the call copies at most its length.
    let length = unsafe { GetClassNameW(window, &mut class) } as usize;
    let class = String::from_utf16_lossy(&class[..length]);
    class == "Progman" || class == "WorkerW"
}

fn progman() -> Option<HWND> {
    // SAFETY: the found window is checked for validity before use, and the
    // message is sent with a bounded timeout so a hung Explorer cannot block us.
    unsafe {
        let hwnd = FindWindowW(w!("Progman"), PCWSTR::null()).ok()?;
        if hwnd.is_invalid() {
            return None;
        }
        let mut result = 0usize;
        let _ = SendMessageTimeoutW(
            hwnd,
            SPAWN_WORKER_MESSAGE,
            WPARAM(0),
            LPARAM(0),
            SMTO_NORMAL,
            1000,
            Some(&mut result),
        );
        Some(hwnd)
    }
}

fn wallpaper_worker() -> Option<HWND> {
    let _ = progman()?;
    FOUND_WORKER.store(0, Ordering::SeqCst);
    // SAFETY: `find_worker` matches the callback signature and only reads the
    // window handles EnumWindows passes it.
    unsafe {
        let _ = EnumWindows(Some(find_worker), LPARAM(0));
    }
    let raw = FOUND_WORKER.load(Ordering::SeqCst);
    if raw == 0 {
        None
    } else {
        Some(HWND(raw as *mut core::ffi::c_void))
    }
}

unsafe extern "system" fn find_worker(window: HWND, _param: LPARAM) -> BOOL {
    // SAFETY: `window` is a live handle supplied by EnumWindows; the child
    // lookups only read it and store the result atomically.
    unsafe {
        let def_view = FindWindowExW(Some(window), None, w!("SHELLDLL_DefView"), PCWSTR::null());
        if let Ok(def_view) = def_view {
            if !def_view.is_invalid() {
                if let Ok(worker) = FindWindowExW(None, Some(window), w!("WorkerW"), PCWSTR::null())
                {
                    if !worker.is_invalid() {
                        FOUND_WORKER.store(worker.0 as isize, Ordering::SeqCst);
                        return FALSE;
                    }
                }
            }
        }
    }
    TRUE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_has_no_host() {
        assert!(!Placement::TopLevel.is_desktop_hosted());
        assert!(Placement::DesktopBackground.is_desktop_hosted());
    }
}
