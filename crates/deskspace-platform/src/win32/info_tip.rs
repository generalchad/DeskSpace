//! Explorer-style info tips: a native tracking tooltip shown while the pointer
//! rests on an item. The app decides when to show it (hover plus a short delay)
//! and supplies the desktop coordinates; this module builds Explorer's content
//! and drives the tooltip control.
//!
//! The tooltip is a `tooltips_class32` window used as a tracking tool
//! (`TTF_TRACK`), shown with `TTM_TRACKACTIVATE` at a position rather than
//! attached to a rectangle, because spaces never receive the activation a
//! normal tip would need.
//! https://learn.microsoft.com/en-us/windows/win32/controls/tooltip-control-reference

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{FILETIME, HWND, LPARAM, RECT, SYSTEMTIME, WPARAM};
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_NORMAL, FileTimeToLocalFileTime};
use windows::Win32::System::Time::FileTimeToSystemTime;
use windows::Win32::UI::Controls::{
    TOOLTIPS_CLASSW, TTF_ABSOLUTE, TTF_TRACK, TTM_ADDTOOLW, TTM_SETMAXTIPWIDTH, TTM_TRACKACTIVATE,
    TTM_TRACKPOSITION, TTM_UPDATETIPTEXTW, TTS_ALWAYSTIP, TTS_NOPREFIX, TTTOOLINFOW,
};
use windows::Win32::UI::Shell::{SHFILEINFOW, SHGFI_TYPENAME, SHGetFileInfoW};
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DestroyWindow, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SendMessageW, SetWindowPos, WINDOW_STYLE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};
use windows::core::{PCWSTR, PWSTR};

use crate::error::PlatformError;
use crate::win32::system::wide;

/// A tooltip window reused for every info tip.
pub(crate) struct InfoTip {
    tooltip: HWND,
    owner: HWND,
}

impl InfoTip {
    pub(crate) fn create(owner: HWND) -> Result<Self, PlatformError> {
        // SAFETY: creating a system tooltip window on the UI thread. The
        // window is destroyed in `Drop`, on the same thread.
        unsafe {
            let tooltip = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                TOOLTIPS_CLASSW,
                PCWSTR::null(),
                WINDOW_STYLE(WS_POPUP.0 | TTS_ALWAYSTIP | TTS_NOPREFIX),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                None,
                None,
            )?;
            // Wrap long names instead of running off the screen; a hidden tool
            // will not extend past this width.
            send(tooltip, TTM_SETMAXTIPWIDTH, 0, 400);
            let tip = Self { tooltip, owner };
            tip.add();
            Ok(tip)
        }
    }

    /// Shows `text` with its top-left near `at` (screen coordinates).
    pub(crate) fn show(&self, text: &str, at: windows::Win32::Foundation::POINT) {
        let mut text = wide(text);
        // SAFETY: the tool window is live; `text` outlives each call, and the
        // tooltip copies it before returning.
        unsafe {
            let tool = tool(self.owner, PWSTR(text.as_mut_ptr()));
            send(
                self.tooltip,
                TTM_UPDATETIPTEXTW,
                0,
                &tool as *const _ as isize,
            );
            send(self.tooltip, TTM_TRACKPOSITION, 0, pack_point(at.x, at.y));
            send(
                self.tooltip,
                TTM_TRACKACTIVATE,
                1,
                &tool as *const _ as isize,
            );
            let _ = SetWindowPos(
                self.tooltip,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    pub(crate) fn hide(&self) {
        // SAFETY: the tool window is live; a tool with `lpszText` null is
        // enough to address the entry added in `add`.
        unsafe {
            let tool = tool(self.owner, PWSTR::null());
            send(
                self.tooltip,
                TTM_TRACKACTIVATE,
                0,
                &tool as *const _ as isize,
            );
        }
    }

    /// Registers the tracking tool once; later shows update its text.
    unsafe fn add(&self) {
        // SAFETY: the tool window is live; `TTM_ADDTOOLW` copies the text.
        unsafe {
            let tool = tool(self.owner, PWSTR::null());
            send(self.tooltip, TTM_ADDTOOLW, 0, &tool as *const _ as isize);
        }
    }
}

/// Sends a tooltip message with its arguments wrapped, since `SendMessageW`
/// takes `Option`-wrapped words.
unsafe fn send(hwnd: HWND, message: u32, wparam: usize, lparam: isize) {
    // SAFETY: the caller guarantees `hwnd` is a live window.
    unsafe {
        SendMessageW(hwnd, message, Some(WPARAM(wparam)), Some(LPARAM(lparam)));
    }
}

impl Drop for InfoTip {
    fn drop(&mut self) {
        // SAFETY: the window was created by this thread and is destroyed here
        // exactly once.
        unsafe {
            let _ = DestroyWindow(self.tooltip);
        }
    }
}

fn tool(owner: HWND, text: PWSTR) -> TTTOOLINFOW {
    TTTOOLINFOW {
        cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
        uFlags: TTF_TRACK | TTF_ABSOLUTE,
        hwnd: owner,
        uId: 0,
        rect: RECT::default(),
        lpszText: text,
        ..Default::default()
    }
}

/// `TTM_TRACKPOSITION` packs x in the low word and y in the high word.
fn pack_point(x: i32, y: i32) -> isize {
    ((x as isize) & 0xFFFF) | (((y as isize) & 0xFFFF) << 16)
}

/// Explorer's info-tip text for `path`: the file name, its type, its size, and
/// its modification date, each on its own line.
pub(crate) fn describe(path: &Path) -> String {
    let mut lines = Vec::new();
    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        lines.push(name.to_string());
    }
    if let Some(kind) = type_name(path) {
        lines.push(kind);
    }
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.is_file() {
            lines.push(format_size(metadata.len()));
        }
        if let Ok(modified) = metadata.modified()
            && let Some(when) = format_modified(modified)
        {
            lines.push(format!("Date modified: {when}"));
        }
    }
    lines.join("\r\n")
}

/// The shell's type name for a path (for example "Text Document"), which
/// matches what Explorer shows in an info tip.
fn type_name(path: &Path) -> Option<String> {
    let wide = wide(&path.to_string_lossy());
    let mut info = SHFILEINFOW::default();
    // SAFETY: `wide` is NUL-terminated and `info` is a valid out-structure of
    // the size passed.
    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_TYPENAME,
        )
    };
    if result == 0 {
        return None;
    }
    let end = info
        .szTypeName
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(info.szTypeName.len());
    (end > 0).then(|| String::from_utf16_lossy(&info.szTypeName[..end]))
}

/// Formats `modified` as local time, using the same `YYYY-MM-DD HH:MM` shape on
/// every machine so the tip is unambiguous.
fn format_modified(modified: SystemTime) -> Option<String> {
    let seconds = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    // 100-nanosecond intervals between 1601-01-01 and 1970-01-01.
    const EPOCH_OFFSET: u64 = 11_644_473_600;
    let filetime = (seconds + EPOCH_OFFSET) * 10_000_000;
    let utc = FILETIME {
        dwLowDateTime: filetime as u32,
        dwHighDateTime: (filetime >> 32) as u32,
    };
    let mut local = FILETIME::default();
    // SAFETY: both `FILETIME`s are valid; the calls only fill the output.
    unsafe {
        FileTimeToLocalFileTime(&utc, &mut local).ok()?;
    }
    let mut system = SYSTEMTIME::default();
    // SAFETY: as above.
    unsafe {
        FileTimeToSystemTime(&local, &mut system).ok()?;
    }
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        system.wYear, system.wMonth, system.wDay, system.wHour, system.wMinute
    ))
}

/// Formats a byte count the way Explorer's info tip does.
fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let size = bytes as f64;
    if size < KB {
        format!("{bytes} bytes")
    } else if size < KB * KB {
        format!("{:.1} KB", size / KB)
    } else if size < KB * KB * KB {
        format!("{:.1} MB", size / (KB * KB))
    } else {
        format!("{:.1} GB", size / (KB * KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_explorers_units() {
        assert_eq!(format_size(0), "0 bytes");
        assert_eq!(format_size(512), "512 bytes");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn point_packs_low_and_high_words() {
        assert_eq!(pack_point(10, 20), 10 | (20 << 16));
        // Negative coordinates (monitors left of the primary) keep their low
        // 16 bits, as MAKELPARAM truncates.
        assert_eq!(pack_point(-1, -1), 0xFFFF | (0xFFFF << 16));
    }

    #[test]
    fn a_missing_path_still_describes_its_name() {
        let text = describe(Path::new("C:/definitely/missing/report.txt"));
        assert_eq!(text.lines().next(), Some("report.txt"));
    }
}
