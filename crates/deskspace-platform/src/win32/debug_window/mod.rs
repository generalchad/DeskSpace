//! The debug window (`--debug`, ROADMAP D14): a modeless, resizable window
//! the app describes as a `DebugPanel`. A section list on the left picks
//! which section's controls the panel shows; the log console fills the
//! bottom. Input comes back as `DebugEvent`s.
//!
//! The panel is a scrolled pane holding one page per section, all built at
//! once; only the chosen page is shown, and scrolling moves it up inside the
//! pane. The pane and pages forward control notifications to the window, so
//! every control is found by id in one place. The window owns its state
//! behind `GWLP_USERDATA` (`window_state`), freed in `WM_NCDESTROY`.

mod build;
mod console;
mod layout;
mod pages;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    FillRect, HDC, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RedrawWindow,
};
use windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BN_CLICKED, CB_GETCURSEL, CBN_SELCHANGE, CreateWindowExW, DefWindowProcW,
    DestroyWindow, EnumChildWindows, GA_ROOT, GetAncestor, GetClassNameW, GetClientRect, ICON_BIG,
    ICON_SMALL, LBN_SELCHANGE, MINMAXINFO, SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW,
    SetForegroundWindow, SetWindowPos, ShowWindow, WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN,
    WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DPICHANGED, WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WM_NOTIFY, WM_SETFONT, WM_SETICON,
    WM_SIZE, WM_VSCROLL, WS_CLIPCHILDREN, WS_EX_CONTROLPARENT, WS_OVERLAPPEDWINDOW,
};
use windows::core::{BOOL, PCWSTR, w};

use crate::debug_console::ConsoleModel;
use crate::error::PlatformError;
use crate::input::{DebugEvent, Event};
use crate::ui::{DebugControlKind, DebugPanel, LogLine};
use crate::win32::dark_mode::{self, ControlTheme};
use crate::win32::dialog::DialogChrome;
use crate::win32::dpi::{monitor_dpi_at, scale_for, window_dpi};
use crate::win32::system::{cursor_pos, loword, wide, work_area_at};
use crate::win32::window_class::{self, WindowClass};
use crate::win32::{runtime, settings_window, tray, window_state};

use build::{build_sections, color_table, create_frame_parts, set_value};
use console::ConsoleControls;
use layout::{min_client, s};
use pages::{layout_page, on_section_changed, on_vscroll, on_wheel, place, relayout};

const CLASS_NAME: PCWSTR = w!("DeskSpaceDebug");
/// The scrolled pane and the section pages inside it.
const PANE_CLASS: PCWSTR = w!("DeskSpaceDebugPane");
const ID_RAIL: usize = 5;
const ID_PANE: usize = 6;
/// A control's window id is `CONTROL_BASE + DebugControl::id`.
const CONTROL_BASE: usize = 1000;
/// The client size a new window asks for, at 96 DPI.
const INITIAL_WIDTH: i32 = 980;
const INITIAL_HEIGHT: i32 = 760;

/// The windows made for one control.
struct ControlHandles {
    /// The label left of the control; a button has none.
    label: Option<HWND>,
    main: Option<HWND>,
}

/// One section's page and its controls, in `DebugSection::controls` order.
struct SectionControls {
    page: HWND,
    controls: Vec<ControlHandles>,
}

struct DebugState {
    chrome: DialogChrome,
    /// What the window shows now.
    panel: DebugPanel,
    rail: Option<HWND>,
    pane: Option<HWND>,
    /// One per built section, in `panel.sections` order.
    sections: Vec<SectionControls>,
    /// The section the panel shows.
    current: usize,
    /// How far the current page is scrolled up, in pixels.
    scroll: i32,
    /// The log lines the console shows.
    console: ConsoleModel,
    console_controls: ConsoleControls,
    /// Whether new lines scroll the console to the bottom.
    auto_scroll: bool,
}

/// Creates and shows the debug window for `panel`.
pub(crate) fn create(panel: &DebugPanel, dark: bool) -> Result<HWND, PlatformError> {
    // SAFETY: Win32 window creation on the UI thread. Ownership of the state
    // passes to the window in WM_NCCREATE; it is freed in WM_NCDESTROY.
    unsafe {
        let instance = window_class::instance()?;
        ensure_classes()?;
        let cursor = cursor_pos();
        let dpi = monitor_dpi_at(cursor);
        let state = Box::into_raw(Box::new(DebugState {
            chrome: DialogChrome::new(dpi, dark),
            panel: panel.clone(),
            rail: None,
            pane: None,
            sections: Vec::new(),
            current: 0,
            scroll: 0,
            console: ConsoleModel::default(),
            console_controls: ConsoleControls::default(),
            auto_scroll: true,
        }));
        let (x, y, width, height) = initial_frame(cursor, dpi);
        let title = wide("DeskSpace Debug");
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            CLASS_NAME,
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance),
            Some(state as *const core::ffi::c_void),
        )?;
        let Some((font, colors)) =
            with_state(hwnd, |state| (state.chrome.font, state.chrome.colors))
        else {
            let _ = DestroyWindow(hwnd);
            return Err(PlatformError::Win32(windows::core::Error::from_thread()));
        };
        let (rail, pane) = create_frame_parts(hwnd, font);
        let console_controls = console::create(hwnd, font, colors, panel.console_level);
        with_state(hwnd, |state| {
            state.rail = rail;
            state.pane = pane;
            state.console_controls = console_controls;
        });
        build_panel(hwnd, panel);
        let icon = tray::shared_icon();
        for size in [ICON_BIG, ICON_SMALL] {
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(size as usize)),
                Some(LPARAM(icon.0 as isize)),
            );
        }
        dark_mode::set_caption_dark(hwnd, dark);
        theme_controls(hwnd, dark);
        relayout(hwnd);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        Ok(hwnd)
    }
}

/// The window's frame for the initial client size, centered on the work
/// area under `cursor`: x, y, width, height.
fn initial_frame(cursor: deskspace_core::geometry::Point, dpi: u32) -> (i32, i32, i32, i32) {
    let scale = scale_for(dpi);
    let work = work_area_at(cursor);
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: s(INITIAL_WIDTH, scale),
        bottom: s(INITIAL_HEIGHT, scale),
    };
    // SAFETY: a pure size computation on a local rect.
    unsafe {
        let _ = AdjustWindowRectExForDpi(
            &mut frame,
            WS_OVERLAPPEDWINDOW,
            false,
            WS_EX_CONTROLPARENT,
            dpi,
        );
    }
    let width = (frame.right - frame.left).min(work.width);
    let height = (frame.bottom - frame.top).min(work.height);
    (
        work.x + (work.width - width) / 2,
        work.y + (work.height - height) / 2,
        width,
        height,
    )
}

/// Shows `panel` in the open window: only the changed values when it has
/// the same shape, otherwise a rebuild of every section.
pub(crate) fn update(hwnd: HWND, panel: &DebugPanel) {
    // SAFETY: `hwnd` is the live debug window; the borrow ends before any
    // control is written.
    unsafe {
        let Some(same) = with_state(hwnd, |state| state.panel.same_shape(panel)) else {
            return;
        };
        if !same {
            tracing::warn!("the debug panel changed shape; rebuilding the window");
            rebuild(hwnd, panel);
            return;
        }
        let Some(old) = with_state(hwnd, |state| {
            std::mem::replace(&mut state.panel, panel.clone())
        }) else {
            return;
        };
        if old == *panel {
            return;
        }
        if old.console_level != panel.console_level
            && let Some(controls) = with_state(hwnd, |state| state.console_controls)
        {
            console::set_level(controls, panel.console_level);
        }
        let writes = with_state(hwnd, |state| {
            let mut writes = Vec::new();
            for ((before, after), built) in old
                .sections
                .iter()
                .zip(&panel.sections)
                .zip(&state.sections)
            {
                for ((x, y), handles) in before
                    .controls
                    .iter()
                    .zip(&after.controls)
                    .zip(&built.controls)
                {
                    if let Some(main) = handles.main.filter(|_| x.kind != y.kind) {
                        writes.push((main, x.kind.clone(), y.kind.clone()));
                    }
                }
            }
            writes
        })
        .unwrap_or_default();
        for (main, before, after) in writes {
            set_value(main, Some(&before), &after);
        }
        // A readout's height follows its line count.
        layout_page(hwnd);
    }
}

/// Adds `lines` to the open window's console.
pub(crate) fn append_log(hwnd: HWND, lines: &[LogLine]) {
    // SAFETY: `hwnd` is the live debug window.
    unsafe { console::append(hwnd, lines) }
}

/// Switches the open window between dark and light.
pub(crate) fn set_dark(hwnd: HWND, dark: bool) {
    let dpi = window_dpi(hwnd);
    // SAFETY: `hwnd` is the live debug window.
    unsafe {
        if with_state(hwnd, |state| state.chrome.colors.dark) == Some(dark) {
            return;
        }
        restyle(hwnd, dpi, dark);
    }
}

pub(crate) use settings_window::bring_to_front;

/// Builds every section into the pane and shows the first.
unsafe fn build_panel(hwnd: HWND, panel: &DebugPanel) {
    // SAFETY: handles are copied out under a short borrow; the controls are
    // created after it.
    unsafe {
        let Some((Some(rail), Some(pane), font, colors)) = with_state(hwnd, |state| {
            (
                state.rail,
                state.pane,
                state.chrome.font,
                state.chrome.colors,
            )
        }) else {
            tracing::warn!("the debug window has no section list or pane");
            return;
        };
        let sections = build_sections(rail, pane, panel, font, colors);
        with_state(hwnd, |state| {
            state.sections = sections;
            state.current = 0;
            state.scroll = 0;
        });
    }
}

/// Replaces every section with `panel`'s (its shape changed).
unsafe fn rebuild(hwnd: HWND, panel: &DebugPanel) {
    // SAFETY: the old pages are taken out under a short borrow and destroyed
    // after it; destroying a page destroys its controls.
    unsafe {
        let old = with_state(hwnd, |state| {
            state.panel = panel.clone();
            std::mem::take(&mut state.sections)
        })
        .unwrap_or_default();
        for section in old {
            let _ = DestroyWindow(section.page);
        }
        build_panel(hwnd, panel);
        let dark = with_state(hwnd, |state| state.chrome.colors.dark).unwrap_or(false);
        theme_controls(hwnd, dark);
        layout_page(hwnd);
    }
}

/// A control notification, routed by id.
unsafe fn on_command(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    let id = wparam.0 & 0xFFFF;
    let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
    let control = HWND(lparam.0 as *mut core::ffi::c_void);
    // SAFETY: `control` is the live control that sent the notification; the
    // borrow ends before the event is dispatched.
    unsafe {
        if console::on_command(hwnd, id, code, control) {
            return;
        }
        if id == ID_RAIL {
            if code == LBN_SELCHANGE {
                on_section_changed(hwnd, control);
            }
            return;
        }
        let Some(debug_id) = id
            .checked_sub(CONTROL_BASE)
            .and_then(|id| u32::try_from(id).ok())
        else {
            return;
        };
        let Some(kind) = with_state(hwnd, |state| {
            state
                .panel
                .sections
                .iter()
                .flat_map(|section| &section.controls)
                .find(|control| control.id == debug_id)
                .map(|control| std::mem::discriminant(&control.kind))
        })
        .flatten() else {
            return;
        };
        let event = if kind == std::mem::discriminant(&DebugControlKind::Button { enabled: true })
            && code == BN_CLICKED
        {
            DebugEvent::Pressed { id: debug_id }
        } else if kind == std::mem::discriminant(&DebugControlKind::Toggle { on: true })
            && code == BN_CLICKED
        {
            let on = SendMessageW(control, BM_GETCHECK, None, None).0 != 0;
            DebugEvent::Toggled { id: debug_id, on }
        } else if code == CBN_SELCHANGE {
            let index = SendMessageW(control, CB_GETCURSEL, None, None).0;
            let Ok(index) = usize::try_from(index) else {
                return;
            };
            DebugEvent::Chose {
                id: debug_id,
                index,
            }
        } else {
            return;
        };
        runtime::dispatch(Event::Debug(event));
    }
}

/// Replaces the font and colors for `dpi` and `dark`, then lays out again.
unsafe fn restyle(hwnd: HWND, dpi: u32, dark: bool) {
    let chrome = DialogChrome::new(dpi, dark);
    let font = chrome.font;
    let colors = chrome.colors;
    // SAFETY: the new font is set on every control before the old chrome,
    // which owns the old font, is dropped (outside the borrow).
    unsafe {
        unsafe extern "system" fn set_font(control: HWND, font: LPARAM) -> BOOL {
            // SAFETY: `control` is a live descendant being enumerated.
            unsafe {
                SendMessageW(
                    control,
                    WM_SETFONT,
                    Some(WPARAM(font.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
            BOOL::from(true)
        }
        let _ = EnumChildWindows(Some(hwnd), Some(set_font), LPARAM(font.0 as isize));
        let old = with_state(hwnd, |state| std::mem::replace(&mut state.chrome, chrome));
        drop(old);
        dark_mode::set_caption_dark(hwnd, dark);
        theme_controls(hwnd, dark);
        let tables = with_state(hwnd, |state| {
            state
                .panel
                .sections
                .iter()
                .zip(&state.sections)
                .flat_map(|(section, built)| section.controls.iter().zip(&built.controls))
                .filter(|(control, _)| matches!(control.kind, DebugControlKind::Table { .. }))
                .filter_map(|(_, handles)| handles.main)
                .chain(state.console_controls.list)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
        for table in tables {
            color_table(table, colors);
        }
        relayout(hwnd);
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_FRAME,
        );
    }
}

/// Gives every control the dark or light visual style for its class.
unsafe fn theme_controls(hwnd: HWND, dark: bool) {
    unsafe extern "system" fn visit(control: HWND, dark: LPARAM) -> BOOL {
        let mut class = [0u16; 32];
        // SAFETY: `control` is a live descendant being enumerated; the
        // buffer is sized for the call.
        let length = unsafe { GetClassNameW(control, &mut class) } as usize;
        let theme = match String::from_utf16_lossy(&class[..length]).as_str() {
            "Button" | "ListBox" | "SysListView32" => Some(ControlTheme::Explorer),
            "ComboBox" | "Edit" => Some(ControlTheme::FileDialog),
            _ => None,
        };
        if let Some(theme) = theme {
            dark_mode::theme_control(control, theme, dark.0 != 0);
        }
        BOOL::from(true)
    }
    // SAFETY: the callback only themes the enumerated controls.
    unsafe {
        let _ = EnumChildWindows(Some(hwnd), Some(visit), LPARAM(dark as isize));
    }
}

/// Fills `dc` with the body color. `hwnd` is the debug window; `target` is
/// the window being erased.
unsafe fn erase(hwnd: HWND, target: HWND, dc: HDC) -> bool {
    // SAFETY: `dc` is `target`'s DC for this message; the borrow is short.
    unsafe {
        let Some(brush) = with_state(hwnd, |state| state.chrome.body_brush) else {
            return false;
        };
        let mut client = RECT::default();
        let _ = GetClientRect(target, &mut client);
        FillRect(dc, &client, brush);
        true
    }
}

/// The smallest window for the minimum client at the window's DPI.
unsafe fn set_min_size(hwnd: HWND, lparam: LPARAM) {
    let info = lparam.0 as *mut MINMAXINFO;
    let dpi = window_dpi(hwnd);
    let (width, height) = min_client(scale_for(dpi));
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    // SAFETY: WM_GETMINMAXINFO passes a live MINMAXINFO for this message.
    unsafe {
        let _ = AdjustWindowRectExForDpi(
            &mut frame,
            WS_OVERLAPPEDWINDOW,
            false,
            WS_EX_CONTROLPARENT,
            dpi,
        );
        if let Some(info) = info.as_mut() {
            info.ptMinTrackSize.x = frame.right - frame.left;
            info.ptMinTrackSize.y = frame.bottom - frame.top;
        }
    }
}

/// Moves to the rect Windows suggests for the new DPI, with fonts and
/// metrics for it.
unsafe fn on_dpi_changed(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    let dpi = loword(wparam.0 as isize) as u32;
    // SAFETY: WM_DPICHANGED passes the suggested window rect in `lparam`.
    unsafe {
        let dark = with_state(hwnd, |state| state.chrome.colors.dark).unwrap_or(false);
        restyle(hwnd, dpi, dark);
        if let Some(rect) = (lparam.0 as *const RECT).as_ref() {
            let _ = SetWindowPos(
                hwnd,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`; state
    // access follows the rules on `with_state`.
    unsafe {
        match message {
            WM_NCCREATE => {
                window_state::attach_create_param::<DebugState>(window, lparam);
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_COMMAND => {
                on_command(window, wparam, lparam);
                LRESULT(0)
            }
            WM_NOTIFY => console::on_notify(window, lparam)
                .unwrap_or_else(|| DefWindowProcW(window, message, wparam, lparam)),
            WM_VSCROLL if lparam.0 == 0 => {
                on_vscroll(window, wparam);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                on_wheel(window, wparam);
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                with_state(window, |state| state.chrome.control_color(message, wparam))
                    .flatten()
                    .unwrap_or_else(|| DefWindowProcW(window, message, wparam, lparam))
            }
            WM_ERASEBKGND => {
                if erase(window, window, HDC(wparam.0 as *mut core::ffi::c_void)) {
                    LRESULT(1)
                } else {
                    DefWindowProcW(window, message, wparam, lparam)
                }
            }
            WM_SIZE => {
                relayout(window);
                LRESULT(0)
            }
            WM_GETMINMAXINFO => {
                set_min_size(window, lparam);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                on_dpi_changed(window, wparam, lparam);
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(window);
                runtime::dispatch(Event::Debug(DebugEvent::Closed));
                LRESULT(0)
            }
            WM_NCDESTROY => {
                runtime::debug_closed(window);
                window_state::free::<DebugState>(window);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

/// The pane and the pages pass control notifications, scrolling, and
/// erasing up to the debug window, which holds the state.
unsafe extern "system" fn pane_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: forwarding a message with its original arguments to our own
    // top-level window.
    unsafe {
        let root = GetAncestor(window, GA_ROOT);
        match message {
            WM_COMMAND | WM_NOTIFY | WM_VSCROLL | WM_MOUSEWHEEL | WM_CTLCOLORSTATIC
            | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                SendMessageW(root, message, Some(wparam), Some(lparam))
            }
            WM_ERASEBKGND => {
                if erase(root, window, HDC(wparam.0 as *mut core::ffi::c_void)) {
                    LRESULT(1)
                } else {
                    DefWindowProcW(window, message, wparam, lparam)
                }
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

/// Registers the window and pane classes. Both erase with the body color
/// (WM_ERASEBKGND), so the classes have no background brush.
fn ensure_classes() -> Result<(), PlatformError> {
    window_class::register(WindowClass::new(CLASS_NAME, Some(wndproc)).arrow_cursor())?;
    window_class::register(WindowClass::new(PANE_CLASS, Some(pane_wndproc)).arrow_cursor())
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut DebugState) -> R) -> Option<R> {
    // SAFETY: the debug window always holds a `DebugState`; the caller rules
    // out re-entry.
    unsafe { window_state::with_state(window, f) }
}
