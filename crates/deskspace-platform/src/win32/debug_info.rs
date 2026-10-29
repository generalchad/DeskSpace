//! Facts the debug window shows about this process and Windows (ROADMAP D14).

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetCurrentProcessId, GetGuiResources,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GWL_EXSTYLE, GetClassNameW, GetForegroundWindow, GetWindowLongW,
    GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, MONITORINFOF_PRIMARY,
    WINDOW_EX_STYLE, WS_EX_TOPMOST,
};
use windows::core::BOOL;

use crate::win32::dialog::window_text;
use crate::win32::dpi::window_dpi;

/// What this process holds right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessStats {
    /// Physical memory in use, in bytes.
    pub working_set: u64,
    pub gdi_objects: u32,
    pub user_objects: u32,
}

/// This process's working set and GDI and USER object counts. A count that
/// cannot be read is 0.
/// https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getguiresources
pub fn process_stats() -> ProcessStats {
    // SAFETY: the pseudo handle for this process needs no closing, and the
    // counters struct is sized for the call.
    unsafe {
        let process = GetCurrentProcess();
        let mut memory = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        let working_set = match GetProcessMemoryInfo(process, &mut memory, memory.cb) {
            Ok(()) => memory.WorkingSetSize as u64,
            Err(_) => 0,
        };
        ProcessStats {
            working_set,
            gdi_objects: GetGuiResources(process, GR_GDIOBJECTS),
            user_objects: GetGuiResources(process, GR_USEROBJECTS),
        }
    }
}

/// The keyboard and mouse hooks' flags (`ENGINEERING.md` "Keys").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HookState {
    /// Capturing every editing key (inline rename).
    pub editing: bool,
    /// Watching the item keys while an item is selected.
    pub watch_selection: bool,
    /// Watching the space keys while a space is current.
    pub watch_paste: bool,
    /// Watching for clicks outside every space.
    pub outside_clicks: bool,
}

pub(crate) fn hook_state() -> HookState {
    let (editing, watch_selection, watch_paste) = crate::win32::keyboard::flags();
    HookState {
        editing,
        watch_selection,
        watch_paste,
        outside_clicks: crate::win32::outside_click::is_watching(),
    }
}

/// One of this process's windows, as the debug window's table lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRow {
    pub class: String,
    pub title: String,
    /// The space it draws, when it is a space window.
    pub space: Option<u64>,
    /// In screen coordinates.
    pub rect: Rect,
    pub dpi: u32,
    pub visible: bool,
}

/// One taskbar and where it stands against the desktop layer, which spans
/// the whole desktop: a taskbar under the layer cannot be clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskbarRow {
    /// `Shell_TrayWnd` (primary) or `Shell_SecondaryTrayWnd`.
    pub class: String,
    pub rect: Rect,
    pub topmost: bool,
    /// Whether it is above the layer in z-order; `None` without a layer.
    pub above_layer: Option<bool>,
}

impl TaskbarRow {
    /// Whether the layer covers it, so its clicks go to the layer.
    pub fn is_blocked(&self) -> bool {
        self.above_layer == Some(false)
    }
}

/// Every taskbar, primary first in z-order terms, against `layer`.
pub(crate) fn taskbars(layer: Option<HWND>) -> Vec<TaskbarRow> {
    let windows = enumerate(None);
    let layer_index = layer.and_then(|layer| windows.iter().position(|window| *window == layer));
    windows
        .iter()
        .enumerate()
        .filter_map(|(index, window)| {
            let row = window_row(*window, None);
            matches!(
                row.class.as_str(),
                "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            )
            .then(|| TaskbarRow {
                class: row.class,
                rect: row.rect,
                topmost: is_topmost(*window),
                above_layer: layer_index.map(|layer| index < layer),
            })
        })
        .collect()
}

/// The foreground window, of any process: what Explorer's full-screen
/// detection looks at.
pub fn foreground_window() -> Option<WindowRow> {
    // SAFETY: a plain query; the handle is only read from.
    let window = unsafe { GetForegroundWindow() };
    (!window.is_invalid()).then(|| window_row(window, None))
}

fn is_topmost(window: HWND) -> bool {
    // SAFETY: a plain style query on a window from the enumeration.
    let style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32;
    WINDOW_EX_STYLE(style).contains(WS_EX_TOPMOST)
}

/// One display, as Windows reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRow {
    pub rect: Rect,
    /// The rect less the taskbar and docked toolbars.
    pub work: Rect,
    pub dpi: u32,
    pub primary: bool,
}

/// This process's top-level windows in z-order, top first, then `host`'s
/// child windows that `space_of` names a space for (desktop-hosted spaces).
/// Follows `scripts/zorder.ps1`, which does the same from outside.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows
pub(crate) fn windows_of_this_process(
    host: Option<HWND>,
    space_of: impl Fn(HWND) -> Option<u64>,
) -> Vec<WindowRow> {
    // SAFETY: reading this process's id has no preconditions.
    let pid = unsafe { GetCurrentProcessId() };
    let mut rows: Vec<WindowRow> = enumerate(None)
        .into_iter()
        .filter(|window| {
            let mut owner = 0;
            // SAFETY: `window` came from the enumeration; `owner` is a local.
            unsafe { GetWindowThreadProcessId(*window, Some(&mut owner)) };
            owner == pid
        })
        .map(|window| window_row(window, space_of(window)))
        .collect();
    if let Some(host) = host {
        rows.extend(
            enumerate(Some(host))
                .into_iter()
                .filter_map(|window| space_of(window).map(|id| window_row(window, Some(id)))),
        );
    }
    rows
}

/// The top-level windows (`parent` `None`) or `parent`'s descendants, in
/// z-order, top first.
fn enumerate(parent: Option<HWND>) -> Vec<HWND> {
    unsafe extern "system" fn collect(window: HWND, list: LPARAM) -> BOOL {
        // SAFETY: `list` is the `Vec` below, alive for the whole enumeration.
        unsafe { (*(list.0 as *mut Vec<HWND>)).push(window) };
        BOOL::from(true)
    }
    let mut windows: Vec<HWND> = Vec::new();
    let list = LPARAM(&mut windows as *mut Vec<HWND> as isize);
    // SAFETY: the callback only pushes into `windows`, which outlives the call.
    unsafe {
        match parent {
            None => {
                let _ = EnumWindows(Some(collect), list);
            }
            Some(parent) => {
                let _ = EnumChildWindows(Some(parent), Some(collect), list);
            }
        }
    }
    windows
}

fn window_row(window: HWND, space: Option<u64>) -> WindowRow {
    let mut class = [0u16; 256];
    let mut rect = RECT::default();
    // SAFETY: reads of a live window into local buffers sized for the calls.
    unsafe {
        let length = GetClassNameW(window, &mut class).max(0) as usize;
        let _ = GetWindowRect(window, &mut rect);
        WindowRow {
            class: String::from_utf16_lossy(&class[..length]),
            title: window_text(window),
            space,
            rect: rect_of(rect),
            dpi: window_dpi(window),
            visible: IsWindowVisible(window).as_bool(),
        }
    }
}

fn rect_of(rect: RECT) -> Rect {
    Rect::new(
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    )
}

/// Every display, in the order Windows enumerates them.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaymonitors
pub fn monitors() -> Vec<MonitorRow> {
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        list: LPARAM,
    ) -> BOOL {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let (mut dpi, mut unused) = (0, 0);
        // SAFETY: `monitor` is live during the callback, `info` is sized for
        // the call, and `list` is the `Vec` below.
        unsafe {
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return BOOL::from(true);
            }
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut unused);
            (*(list.0 as *mut Vec<MonitorRow>)).push(MonitorRow {
                rect: rect_of(info.rcMonitor),
                work: rect_of(info.rcWork),
                dpi: if dpi == 0 { 96 } else { dpi },
                primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        BOOL::from(true)
    }
    let mut rows: Vec<MonitorRow> = Vec::new();
    // SAFETY: the callback only pushes into `rows`, which outlives the call.
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut rows as *mut Vec<MonitorRow> as isize),
        );
    }
    rows
}

#[cfg(test)]
mod tests {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
    };
    use windows::core::w;

    use super::*;

    #[test]
    fn nothing_is_hooked_or_watched_until_asked() {
        assert_eq!(hook_state(), HookState::default());
        assert_eq!(crate::win32::watchdog::is_running(), Some(false));
    }

    #[test]
    fn there_is_one_primary_monitor() {
        let monitors = monitors();
        assert_eq!(monitors.iter().filter(|monitor| monitor.primary).count(), 1);
        assert!(
            monitors
                .iter()
                .all(|monitor| monitor.dpi >= 96 && monitor.rect.width > 0)
        );
    }

    #[test]
    fn a_live_process_reports_memory_and_a_user_object() {
        // SAFETY: a message-only window of the system "Static" class, made
        // and destroyed on this thread.
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("Static"),
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .expect("a message-only window");
        let stats = process_stats();
        // SAFETY: the window was created above on this thread.
        let _ = unsafe { DestroyWindow(window) };
        assert!(stats.working_set > 0);
        assert!(stats.user_objects >= 1);
    }
}
