//! Creates the debug window's section list, pages, and controls, and writes
//! each control's value.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::HFONT;
use windows::Win32::UI::Controls::{
    LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW, LVCOLUMNW_MASK, LVIF_TEXT, LVITEMW, LVM_DELETEALLITEMS,
    LVM_DELETECOLUMN, LVM_INSERTCOLUMNW, LVM_INSERTITEMW, LVM_SETBKCOLOR, LVM_SETCOLUMNWIDTH,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMTEXTW, LVM_SETTEXTBKCOLOR, LVM_SETTEXTCOLOR,
    LVS_EX_DOUBLEBUFFER, LVS_EX_FULLROWSELECT, LVS_REPORT, LVS_SHOWSELALWAYS, LVS_SINGLESEL,
    LVSCW_AUTOSIZE_USEHEADER, ShowScrollBar, WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_LISTBOXW,
    WC_LISTVIEWW, WC_STATICW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_SETCHECK, BS_AUTOCHECKBOX, BS_PUSHBUTTON, CB_ADDSTRING, CB_RESETCONTENT, CB_SETCURSEL,
    CBS_DROPDOWNLIST, CreateWindowExW, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, HMENU,
    LB_ADDSTRING, LB_RESETCONTENT, LB_SETCURSEL, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, SB_VERT,
    SendMessageW, SetWindowTextW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_SETFONT, WS_CHILD,
    WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::PCWSTR;

use super::layout::READOUT_MAX_LINES;
use super::{CONTROL_BASE, ControlHandles, ID_PANE, ID_RAIL, PANE_CLASS, SectionControls};
use crate::ui::{DebugControl, DebugControlKind, DebugPanel};
use crate::win32::dark_mode::UiColors;
use crate::win32::dialog;
use crate::win32::settings_window::colorref;
use crate::win32::system::wide;

/// SS_NOPREFIX | SS_CENTERIMAGE for labels (WinUser.h).
/// https://learn.microsoft.com/en-us/windows/win32/controls/static-control-styles
const LABEL_STYLE: u32 = 0x0080 | 0x0200;

/// Creates a child control of `parent` with `font`. Unlike
/// `dialog::control`, it takes the font alone, so no borrow of the window's
/// state is held while the control is created (creation sends messages).
#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn control(
    parent: HWND,
    class: PCWSTR,
    text: &str,
    style: u32,
    ex_style: WINDOW_EX_STYLE,
    id: usize,
    font: HFONT,
) -> Option<HWND> {
    let text = wide(text);
    // SAFETY: creating a child of our own window; `text` outlives the call.
    unsafe {
        let hwnd = CreateWindowExW(
            ex_style,
            class,
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
            0,
            0,
            0,
            0,
            Some(parent),
            Some(HMENU(id as *mut core::ffi::c_void)),
            dialog::instance(),
            None,
        )
        .ok()?;
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        Some(hwnd)
    }
}

/// The section list and the scrolled pane that holds the pages.
pub(super) unsafe fn create_frame_parts(window: HWND, font: HFONT) -> (Option<HWND>, Option<HWND>) {
    // SAFETY: both are children of our live window.
    unsafe {
        let rail = control(
            window,
            WC_LISTBOXW,
            "",
            (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            WS_EX_CLIENTEDGE,
            ID_RAIL,
            font,
        );
        let pane = control(
            window,
            PANE_CLASS,
            "",
            WS_VSCROLL.0 | WS_CLIPCHILDREN.0,
            WS_EX_CONTROLPARENT,
            ID_PANE,
            font,
        );
        (rail, pane)
    }
}

/// Fills the section list and builds one hidden page per section inside
/// `pane`, each holding its section's controls with their values.
pub(super) unsafe fn build_sections(
    rail: HWND,
    pane: HWND,
    panel: &DebugPanel,
    font: HFONT,
    colors: UiColors,
) -> Vec<SectionControls> {
    // SAFETY: the rail and pane are live children of the debug window; every
    // page and control is created as their descendant.
    unsafe {
        SendMessageW(rail, LB_RESETCONTENT, None, None);
        let mut sections = Vec::with_capacity(panel.sections.len());
        for section in &panel.sections {
            let title = wide(&section.title);
            SendMessageW(
                rail,
                LB_ADDSTRING,
                None,
                Some(LPARAM(title.as_ptr() as isize)),
            );
            let Some(page) = control(
                pane,
                PANE_CLASS,
                "",
                WS_CLIPCHILDREN.0,
                WS_EX_CONTROLPARENT,
                0,
                font,
            ) else {
                tracing::warn!(section = %section.title, "could not create a debug page");
                continue;
            };
            let mut controls = Vec::with_capacity(section.controls.len());
            let mut group_titles = Vec::new();
            let mut current: Option<&str> = None;
            for debug in &section.controls {
                let group = debug.group.as_deref();
                if group != current {
                    group_titles.extend(group.map(|title| group_title(page, title, font)));
                    current = group;
                }
                controls.push(build_control(page, debug, font, colors));
            }
            sections.push(SectionControls {
                page,
                controls,
                group_titles,
                group_frames: Vec::new(),
            });
        }
        SendMessageW(rail, LB_SETCURSEL, Some(WPARAM(0)), None);
        sections
    }
}

/// Creates the label that titles a group's box; the page draws the box.
unsafe fn group_title(page: HWND, title: &str, font: HFONT) -> Option<HWND> {
    // SAFETY: a child of the live page.
    unsafe {
        control(
            page,
            WC_STATICW,
            title,
            LABEL_STYLE,
            WINDOW_EX_STYLE(0),
            0,
            font,
        )
    }
}

/// Creates one control's label and main control, and writes its value.
unsafe fn build_control(
    page: HWND,
    debug: &DebugControl,
    font: HFONT,
    colors: UiColors,
) -> ControlHandles {
    let id = CONTROL_BASE + debug.id as usize;
    let tab = WS_TABSTOP.0;
    // SAFETY: every control is a child of the live page.
    unsafe {
        let label = match debug.kind {
            DebugControlKind::Button { .. } => None,
            _ => control(
                page,
                WC_STATICW,
                &debug.label,
                LABEL_STYLE,
                WINDOW_EX_STYLE(0),
                0,
                font,
            ),
        };
        let main = match &debug.kind {
            DebugControlKind::Button { .. } => control(
                page,
                WC_BUTTONW,
                &debug.label,
                BS_PUSHBUTTON as u32 | tab,
                WINDOW_EX_STYLE(0),
                id,
                font,
            ),
            DebugControlKind::Toggle { .. } => control(
                page,
                WC_BUTTONW,
                "",
                BS_AUTOCHECKBOX as u32 | tab,
                WINDOW_EX_STYLE(0),
                id,
                font,
            ),
            DebugControlKind::Choice { .. } => control(
                page,
                WC_COMBOBOXW,
                "",
                CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | tab,
                WINDOW_EX_STYLE(0),
                id,
                font,
            ),
            DebugControlKind::Readout { .. } => control(
                page,
                WC_EDITW,
                "",
                (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0 | tab,
                WS_EX_CLIENTEDGE,
                id,
                font,
            ),
            DebugControlKind::Table { .. } => {
                let table = control(
                    page,
                    WC_LISTVIEWW,
                    "",
                    LVS_REPORT | LVS_SHOWSELALWAYS | LVS_SINGLESEL | tab,
                    WS_EX_CLIENTEDGE,
                    id,
                    font,
                );
                if let Some(table) = table {
                    let style = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER;
                    SendMessageW(
                        table,
                        LVM_SETEXTENDEDLISTVIEWSTYLE,
                        Some(WPARAM(style as usize)),
                        Some(LPARAM(style as isize)),
                    );
                    color_table(table, colors);
                }
                table
            }
        };
        if let Some(main) = main {
            set_value(main, None, &debug.kind);
        }
        ControlHandles { label, main }
    }
}

/// Writes `new` into the control, touching only what differs from `old`
/// (`None` writes everything).
pub(super) unsafe fn set_value(main: HWND, old: Option<&DebugControlKind>, new: &DebugControlKind) {
    if old == Some(new) {
        return;
    }
    // SAFETY: `main` is a live control of the kind `new` describes (the
    // window rebuilds instead when a kind changes).
    unsafe {
        match new {
            DebugControlKind::Button { enabled } => {
                let _ = EnableWindow(main, *enabled);
            }
            DebugControlKind::Toggle { on } => {
                SendMessageW(main, BM_SETCHECK, Some(WPARAM(*on as usize)), None);
            }
            DebugControlKind::Choice {
                options,
                selected,
                enabled,
            } => {
                let _ = EnableWindow(main, *enabled);
                let same_options = matches!(
                    old,
                    Some(DebugControlKind::Choice { options: before, .. }) if before == options
                );
                if !same_options {
                    SendMessageW(main, CB_RESETCONTENT, None, None);
                    for option in options {
                        let text = wide(option);
                        SendMessageW(
                            main,
                            CB_ADDSTRING,
                            None,
                            Some(LPARAM(text.as_ptr() as isize)),
                        );
                    }
                }
                SendMessageW(main, CB_SETCURSEL, Some(WPARAM(*selected)), None);
            }
            DebugControlKind::Readout { text } => {
                // A multi-line edit breaks lines only at CR LF.
                let shown = wide(&text.lines().collect::<Vec<_>>().join("\r\n"));
                let _ = SetWindowTextW(main, PCWSTR(shown.as_ptr()));
                let overflows = text.lines().count() > READOUT_MAX_LINES;
                let _ = ShowScrollBar(main, SB_VERT, overflows);
            }
            DebugControlKind::Table { columns, rows } => {
                let same_columns = matches!(
                    old,
                    Some(DebugControlKind::Table { columns: before, .. }) if before == columns
                );
                if !same_columns {
                    set_columns(main, columns);
                }
                set_rows(main, rows);
                for column in 0..columns.len() {
                    SendMessageW(
                        main,
                        LVM_SETCOLUMNWIDTH,
                        Some(WPARAM(column)),
                        Some(LPARAM(LVSCW_AUTOSIZE_USEHEADER as isize)),
                    );
                }
            }
        }
    }
}

/// Gives a table the window's colors.
pub(super) unsafe fn color_table(table: HWND, colors: UiColors) {
    let background = if colors.dark {
        colors.command_bar
    } else {
        colors.body
    };
    // SAFETY: `table` is a live list view of ours.
    unsafe {
        for message in [LVM_SETBKCOLOR, LVM_SETTEXTBKCOLOR] {
            SendMessageW(
                table,
                message,
                None,
                Some(LPARAM(colorref(background).0 as isize)),
            );
        }
        SendMessageW(
            table,
            LVM_SETTEXTCOLOR,
            None,
            Some(LPARAM(colorref(colors.text).0 as isize)),
        );
    }
}

unsafe fn set_columns(table: HWND, columns: &[String]) {
    // SAFETY: `table` is a live list view; each column struct and its text
    // outlive the message that copies them.
    unsafe {
        while SendMessageW(table, LVM_DELETECOLUMN, Some(WPARAM(0)), None).0 != 0 {}
        for (index, title) in columns.iter().enumerate() {
            let mut text = wide(title);
            let column = LVCOLUMNW {
                mask: LVCOLUMNW_MASK(LVCF_TEXT.0 | LVCF_WIDTH.0),
                cx: 100,
                pszText: windows::core::PWSTR(text.as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(
                table,
                LVM_INSERTCOLUMNW,
                Some(WPARAM(index)),
                Some(LPARAM(&column as *const LVCOLUMNW as isize)),
            );
        }
    }
}

unsafe fn set_rows(table: HWND, rows: &[Vec<String>]) {
    // SAFETY: `table` is a live list view; each item struct and its text
    // outlive the message that copies them.
    unsafe {
        SendMessageW(table, LVM_DELETEALLITEMS, None, None);
        for (index, row) in rows.iter().enumerate() {
            for (column, cell) in row.iter().enumerate() {
                let mut text = wide(cell);
                let item = LVITEMW {
                    mask: LVIF_TEXT,
                    iItem: index as i32,
                    iSubItem: column as i32,
                    pszText: windows::core::PWSTR(text.as_mut_ptr()),
                    ..Default::default()
                };
                let message = if column == 0 {
                    LVM_INSERTITEMW
                } else {
                    LVM_SETITEMTEXTW
                };
                SendMessageW(
                    table,
                    message,
                    Some(WPARAM(index)),
                    Some(LPARAM(&item as *const LVITEMW as isize)),
                );
            }
        }
    }
}
