//! The log console under the panel: a toolbar (Level, Filter, Pause,
//! Auto-scroll, Clear, Copy, Save log…) above a virtual list view that reads
//! its rows from the window's `ConsoleModel`.
//!
//! Only the Level box reaches the app (`DebugEvent::ConsoleLevel`); the
//! filter, pause, auto-scroll, and clear are view state, like scrolling.
//! Nothing here logs: a log line would feed the console itself.

use deskspace_core::theme::Color;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{HFONT, InvalidateRect};
use windows::Win32::UI::Controls::{
    CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDRF_DODEFAULT, CDRF_NEWFONT, CDRF_NOTIFYITEMDRAW,
    EM_SETCUEBANNER, LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW, LVCOLUMNW_MASK, LVIF_TEXT,
    LVM_ENSUREVISIBLE, LVM_INSERTCOLUMNW, LVM_SETCOLUMNWIDTH, LVM_SETEXTENDEDLISTVIEWSTYLE,
    LVM_SETITEMCOUNT, LVN_GETDISPINFOW, LVS_EX_DOUBLEBUFFER, LVS_EX_FULLROWSELECT, LVS_OWNERDATA,
    LVS_REPORT, LVS_SHOWSELALWAYS, LVSCW_AUTOSIZE_USEHEADER, LVSICF_NOSCROLL, NM_CUSTOMDRAW, NMHDR,
    NMLVCUSTOMDRAW, NMLVDISPINFOW, WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_LISTVIEWW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_PUSHBUTTON, CB_ADDSTRING,
    CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, EN_CHANGE, ES_AUTOHSCROLL,
    MB_ICONWARNING, MB_OK, MessageBoxW, SendMessageW, WINDOW_EX_STYLE, WS_EX_CLIENTEDGE,
    WS_TABSTOP, WS_VSCROLL,
};
use windows::core::PCWSTR;

use super::build::{color_table, control};
use super::layout::{ConsoleLayout, s, with_drop_down};
use super::{place, with_state};
use crate::debug_console::{format_line, format_time};
use crate::input::{DebugEvent, Event};
use crate::ui::{LogLevel, LogLine};
use crate::win32::dark_mode::UiColors;
use crate::win32::dialog::{self, window_text};
use crate::win32::settings_window::colorref;
use crate::win32::system::wide;
use crate::win32::{clipboard, runtime};

const ID_LEVEL: usize = 10;
const ID_FILTER: usize = 11;
const ID_PAUSE: usize = 12;
const ID_AUTO_SCROLL: usize = 13;
const ID_CLEAR: usize = 14;
const ID_COPY: usize = 15;
const ID_SAVE: usize = 16;
const ID_LOG: usize = 17;
/// The log list's columns and their widths at 96 DPI; the last column
/// takes the rest of the width.
const COLUMNS: [(&str, i32); 4] = [("Time", 84), ("Level", 56), ("Target", 220), ("Message", 0)];
/// Text colors for the levels that stand out; Debug and Trace use the
/// dimmed text color, Info the normal one.
const ERROR_RED: Color = Color::rgb(0xE0, 0x48, 0x48);
const WARN_AMBER: Color = Color::rgb(0xD0, 0x8C, 0x00);

/// The console's controls.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ConsoleControls {
    level: Option<HWND>,
    filter: Option<HWND>,
    pause: Option<HWND>,
    auto_scroll: Option<HWND>,
    clear: Option<HWND>,
    copy: Option<HWND>,
    save: Option<HWND>,
    pub list: Option<HWND>,
}

/// Creates the console's controls, with Auto-scroll on and the Level box
/// showing `level`.
pub(super) unsafe fn create(
    window: HWND,
    font: HFONT,
    colors: UiColors,
    level: LogLevel,
) -> ConsoleControls {
    let tab = WS_TABSTOP.0;
    let none = WINDOW_EX_STYLE(0);
    // SAFETY: every control is a child of the live debug window.
    unsafe {
        let button = |text: &str, id| {
            control(
                window,
                WC_BUTTONW,
                text,
                BS_PUSHBUTTON as u32 | tab,
                none,
                id,
                font,
            )
        };
        let check = |text: &str, id| {
            control(
                window,
                WC_BUTTONW,
                text,
                BS_AUTOCHECKBOX as u32 | tab,
                none,
                id,
                font,
            )
        };
        let controls = ConsoleControls {
            level: control(
                window,
                WC_COMBOBOXW,
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | tab,
                none,
                ID_LEVEL,
                font,
            ),
            filter: control(
                window,
                WC_EDITW,
                "",
                ES_AUTOHSCROLL as u32 | tab,
                WS_EX_CLIENTEDGE,
                ID_FILTER,
                font,
            ),
            pause: check("Pause", ID_PAUSE),
            auto_scroll: check("Auto-scroll", ID_AUTO_SCROLL),
            clear: button("Clear", ID_CLEAR),
            copy: button("Copy", ID_COPY),
            save: button("Save log\u{2026}", ID_SAVE),
            list: control(
                window,
                WC_LISTVIEWW,
                "",
                LVS_REPORT | LVS_OWNERDATA | LVS_SHOWSELALWAYS | tab,
                WS_EX_CLIENTEDGE,
                ID_LOG,
                font,
            ),
        };
        if let Some(combo) = controls.level {
            for level in LogLevel::ALL {
                let text = wide(level.label());
                SendMessageW(
                    combo,
                    CB_ADDSTRING,
                    None,
                    Some(LPARAM(text.as_ptr() as isize)),
                );
            }
        }
        set_level(controls, level);
        if let Some(filter) = controls.filter {
            let cue = wide("Filter");
            SendMessageW(
                filter,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(cue.as_ptr() as isize)),
            );
        }
        if let Some(auto_scroll) = controls.auto_scroll {
            SendMessageW(auto_scroll, BM_SETCHECK, Some(WPARAM(1)), None);
        }
        if let Some(list) = controls.list {
            configure_list(list, colors);
        }
        controls
    }
}

/// Columns, full-row selection, and the window's colors.
unsafe fn configure_list(list: HWND, colors: UiColors) {
    let style = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER;
    // SAFETY: `list` is our live list view; each column struct and its text
    // outlive the message that copies them.
    unsafe {
        SendMessageW(
            list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            Some(WPARAM(style as usize)),
            Some(LPARAM(style as isize)),
        );
        color_table(list, colors);
        for (index, (title, width)) in COLUMNS.iter().enumerate() {
            let mut text = wide(title);
            let column = LVCOLUMNW {
                mask: LVCOLUMNW_MASK(LVCF_TEXT.0 | LVCF_WIDTH.0),
                cx: *width,
                pszText: windows::core::PWSTR(text.as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(
                list,
                LVM_INSERTCOLUMNW,
                Some(WPARAM(index)),
                Some(LPARAM(&column as *const LVCOLUMNW as isize)),
            );
        }
    }
}

/// Shows `level` in the Level box.
pub(super) unsafe fn set_level(controls: ConsoleControls, level: LogLevel) {
    let Some(combo) = controls.level else {
        return;
    };
    let index = LogLevel::ALL
        .iter()
        .position(|each| *each == level)
        .unwrap_or(0);
    // SAFETY: `combo` is our live combo box.
    unsafe {
        SendMessageW(combo, CB_SETCURSEL, Some(WPARAM(index)), None);
    }
}

/// Places the controls in `layout`, scaling the fixed columns.
pub(super) unsafe fn place_all(controls: ConsoleControls, layout: ConsoleLayout, scale: f32) {
    let parts = [
        (controls.level, with_drop_down(layout.level, scale)),
        (controls.filter, layout.filter),
        (controls.pause, layout.pause),
        (controls.auto_scroll, layout.auto_scroll),
        (controls.clear, layout.clear),
        (controls.copy, layout.copy),
        (controls.save, layout.save),
        (controls.list, layout.list),
    ];
    for (control, rect) in parts {
        if let Some(control) = control {
            place(control, rect);
        }
    }
    let Some(list) = controls.list else {
        return;
    };
    // SAFETY: `list` is our live list view.
    unsafe {
        for (index, (_, width)) in COLUMNS.iter().enumerate() {
            let width = if index + 1 == COLUMNS.len() {
                LVSCW_AUTOSIZE_USEHEADER as isize
            } else {
                s(*width, scale) as isize
            };
            SendMessageW(
                list,
                LVM_SETCOLUMNWIDTH,
                Some(WPARAM(index)),
                Some(LPARAM(width)),
            );
        }
    }
}

/// Adds `lines` to the console (or holds them while paused).
pub(super) unsafe fn append(window: HWND, lines: &[LogLine]) {
    // SAFETY: the model is written under a short borrow; the list after it.
    unsafe {
        with_state(window, |state| state.console.push(lines));
        refresh_list(window);
    }
}

/// Tells the list how many rows the model shows now, repaints them, and,
/// with Auto-scroll on, shows the last.
unsafe fn refresh_list(window: HWND) {
    // SAFETY: the counts are copied out under a short borrow; the list is
    // written after it.
    unsafe {
        let Some((Some(list), count, auto_scroll)) = with_state(window, |state| {
            (
                state.console_controls.list,
                state.console.shown_len(),
                state.auto_scroll,
            )
        }) else {
            return;
        };
        SendMessageW(
            list,
            LVM_SETITEMCOUNT,
            Some(WPARAM(count)),
            Some(LPARAM(LVSICF_NOSCROLL as isize)),
        );
        // Dropped lines shift every row up without changing the count.
        let _ = InvalidateRect(Some(list), None, false);
        if auto_scroll && count > 0 {
            SendMessageW(
                list,
                LVM_ENSUREVISIBLE,
                Some(WPARAM(count - 1)),
                Some(LPARAM(0)),
            );
        }
    }
}

/// Handles a console control's notification; returns whether `id` was one.
pub(super) unsafe fn on_command(window: HWND, id: usize, code: u32, control: HWND) -> bool {
    // SAFETY: `control` is the live control that sent the notification; each
    // borrow is short, and the event is dispatched last.
    unsafe {
        let checked = || SendMessageW(control, BM_GETCHECK, None, None).0 != 0;
        match (id, code) {
            (ID_LEVEL, CBN_SELCHANGE) => {
                let index = SendMessageW(control, CB_GETCURSEL, None, None).0;
                if let Some(level) = usize::try_from(index)
                    .ok()
                    .and_then(|i| LogLevel::ALL.get(i))
                {
                    runtime::dispatch(Event::Debug(DebugEvent::ConsoleLevel { level: *level }));
                }
            }
            (ID_FILTER, EN_CHANGE) => {
                let filter = window_text(control);
                with_state(window, |state| state.console.set_filter(&filter));
                refresh_list(window);
            }
            (ID_PAUSE, BN_CLICKED) => {
                let paused = checked();
                with_state(window, |state| state.console.set_paused(paused));
                refresh_list(window);
            }
            (ID_AUTO_SCROLL, BN_CLICKED) => {
                let on = checked();
                with_state(window, |state| state.auto_scroll = on);
                refresh_list(window);
            }
            (ID_CLEAR, BN_CLICKED) => {
                with_state(window, |state| state.console.clear());
                refresh_list(window);
            }
            (ID_COPY, BN_CLICKED) => {
                // A failed copy leaves the clipboard as it was; logging it
                // would feed the console.
                let _ = clipboard::set_text(window, &shown_text(window));
            }
            (ID_SAVE, BN_CLICKED) => save_log(window),
            (
                ID_LEVEL | ID_FILTER | ID_PAUSE | ID_AUTO_SCROLL | ID_CLEAR | ID_COPY | ID_SAVE,
                _,
            ) => {}
            _ => return false,
        }
        true
    }
}

/// Every shown line as text, one per line.
unsafe fn shown_text(window: HWND) -> String {
    // SAFETY: a short borrow that only formats.
    unsafe {
        with_state(window, |state| {
            state
                .console
                .shown_lines()
                .map(format_line)
                .collect::<Vec<_>>()
                .join("\r\n")
        })
        .unwrap_or_default()
    }
}

/// "Save log…": asks for a path and writes the shown lines as UTF-8.
unsafe fn save_log(window: HWND) {
    // SAFETY: the Save dialog and message box are modal over our window; no
    // borrow is held across them.
    unsafe {
        let Some(path) = dialog::pick_save_path(
            window,
            "Save the DeskSpace log",
            "deskspace-log.txt",
            &[("Text files", "*.txt"), ("All files", "*.*")],
        ) else {
            return;
        };
        let text = shown_text(window);
        if let Err(error) = std::fs::write(&path, text) {
            let message = wide(&format!(
                "DeskSpace could not save the log to {}.\n\n{error}",
                path.display()
            ));
            let title = wide("DeskSpace Debug");
            MessageBoxW(
                Some(window),
                PCWSTR(message.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONWARNING,
            );
        }
    }
}

/// Handles the log list's notifications: row text on demand and the text
/// color by level. `None` leaves the message to the default.
pub(super) unsafe fn on_notify(window: HWND, lparam: LPARAM) -> Option<LRESULT> {
    let header = lparam.0 as *const NMHDR;
    // SAFETY: WM_NOTIFY passes a live NMHDR, and the structure it heads is
    // the one its code names, for the duration of the message.
    unsafe {
        let header = header.as_ref()?;
        if header.idFrom != ID_LOG {
            return None;
        }
        match header.code {
            LVN_GETDISPINFOW => {
                fill_cell(window, &mut *(lparam.0 as *mut NMLVDISPINFOW));
                Some(LRESULT(0))
            }
            NM_CUSTOMDRAW => Some(LRESULT(on_custom_draw(
                window,
                &mut *(lparam.0 as *mut NMLVCUSTOMDRAW),
            ) as isize)),
            _ => None,
        }
    }
}

/// Writes the text of the asked-for cell into the list's buffer.
unsafe fn fill_cell(window: HWND, info: &mut NMLVDISPINFOW) {
    let item = &mut info.item;
    if item.mask & LVIF_TEXT != LVIF_TEXT || item.pszText.is_null() || item.cchTextMax <= 0 {
        return;
    }
    let (row, column) = (item.iItem, item.iSubItem);
    // SAFETY: a short borrow that only formats.
    let text = unsafe {
        with_state(window, |state| {
            let row = usize::try_from(row)
                .ok()
                .filter(|row| *row < state.console.shown_len())?;
            Some(cell(state.console.shown(row), column))
        })
        .flatten()
    }
    .unwrap_or_default();
    let units = wide(&text);
    let room = item.cchTextMax as usize - 1;
    let length = (units.len() - 1).min(room);
    // SAFETY: the list's buffer holds `cchTextMax` units; at most `room` plus
    // the NUL are written.
    unsafe {
        std::ptr::copy_nonoverlapping(units.as_ptr(), item.pszText.0, length);
        *item.pszText.0.add(length) = 0;
    }
}

/// One cell's text. A message's line breaks become spaces, since a row
/// shows one line.
fn cell(line: &LogLine, column: i32) -> String {
    match column {
        0 => format_time(line.millis),
        1 => line.level.label().to_string(),
        2 => line.target.clone(),
        _ => line.message.replace(['\r', '\n'], " "),
    }
}

/// Colors each row's text by its level.
unsafe fn on_custom_draw(window: HWND, draw: &mut NMLVCUSTOMDRAW) -> u32 {
    if draw.nmcd.dwDrawStage == CDDS_PREPAINT {
        return CDRF_NOTIFYITEMDRAW;
    }
    if draw.nmcd.dwDrawStage != CDDS_ITEMPREPAINT {
        return CDRF_DODEFAULT;
    }
    let row = draw.nmcd.dwItemSpec;
    // SAFETY: a short borrow that only reads.
    let color = unsafe {
        with_state(window, |state| {
            (row < state.console.shown_len())
                .then(|| level_color(state.console.shown(row).level, state.chrome.colors))
        })
        .flatten()
    };
    let Some(color) = color else {
        return CDRF_DODEFAULT;
    };
    draw.clrText = colorref(color);
    CDRF_NEWFONT
}

fn level_color(level: LogLevel, colors: UiColors) -> Color {
    match level {
        LogLevel::Error => ERROR_RED,
        LogLevel::Warn => WARN_AMBER,
        LogLevel::Info => colors.text,
        LogLevel::Debug | LogLevel::Trace => colors.secondary_text,
    }
}
