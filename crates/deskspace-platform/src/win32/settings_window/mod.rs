//! The settings window: a native, data-driven form. The app describes pages
//! and fields with `ui::SettingsForm`; this module lays them out with standard
//! Win32 controls (common controls v6 via the app's manifest) in a scopes
//! pane, a categories pane, and a page, and reports edits as `SettingsEvent`s.
//!
//! Each page is a container window (`PAGE_CLASS`) that forwards control
//! notifications to the settings window, so every control can be found by id
//! in one place. Window state lives behind a raw `*mut SettingsState` in
//! `GWLP_USERDATA` and is freed exactly once, in `WM_NCDESTROY`.

mod build;
mod commands;
mod nav;
mod paint;
mod update;

pub(crate) use commands::colorref;
pub(crate) use update::update;

use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use deskspace_core::theme::Color;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontIndirectW, CreatePen, CreateSolidBrush, DT_CALCRECT, DT_CENTER,
    DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteObject,
    DrawFocusRect, DrawTextW, Ellipse, EndPaint, FillRect, GetDC, HBRUSH, HDC, HFONT, HGDIOBJ,
    InvalidateRect, PAINTSTRUCT, PS_SOLID, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE,
    RedrawWindow, ReleaseDC, RoundRect, SelectObject, SetBkColor, SetBkMode, SetTextColor,
    TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, ODS_DISABLED, ODS_FOCUS, ODS_SELECTED, TBM_SETPOS, TBM_SETRANGEMIN, TBS_HORZ,
    TBS_NOTICKS, TRACKBAR_CLASSW, WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_LISTBOXW, WC_STATICW,
};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SystemParametersInfoForDpi,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_OWNERDRAW,
    BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST,
    CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, EN_CHANGE, EN_KILLFOCUS,
    ES_AUTOHSCROLL, ES_NUMBER, ES_READONLY, ES_RIGHT, EnumChildWindows, GWLP_USERDATA,
    GetClassNameW, GetClientRect, GetCursorPos, GetParent, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, HMENU, HWND_TOP, ICON_BIG, ICON_SMALL, IsIconic, LB_ADDSTRING, LB_GETCURSEL,
    LB_RESETCONTENT, LB_SETCURSEL, LB_SETITEMHEIGHT, LBN_SELCHANGE, LBS_HASSTRINGS, LBS_NOTIFY,
    LBS_OWNERDRAWFIXED, NONCLIENTMETRICSW, RegisterClassW, SPI_GETNONCLIENTMETRICS, SW_HIDE,
    SW_RESTORE, SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC,
    WM_DRAWITEM, WM_ERASEBKGND, WM_HSCROLL, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SETFONT,
    WM_SETICON, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE,
    WS_EX_CONTROLPARENT, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    WS_VSCROLL,
};
use windows::core::{BOOL, PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, SettingsEvent};
use crate::ui::{
    Field, FieldKind, FieldValue, SettingsForm, SettingsHeader, SettingsNav, SettingsScope,
    SliderMark,
};
use crate::win32::dark_mode::{self, ControlTheme, UiColors};
use crate::win32::system::{wide, work_area_at};
use crate::win32::{runtime, tray};

use build::build;
use commands::{on_field_command, on_scroll};
use nav::{NavRow, on_header_switch, on_nav_changed, on_scope_changed};
use paint::{erase_body, on_draw_header_button, on_draw_item, on_draw_scope, on_paint};

const CLASS_NAME: PCWSTR = w!("DeskSpaceSettings");
const PAGE_CLASS: PCWSTR = w!("DeskSpaceSettingsPage");

const ID_OK: usize = 1;
const ID_CANCEL: usize = 2;
const ID_APPLY: usize = 3;
const ID_LIST: usize = 5;
const ID_SCOPES: usize = 6;
const ID_HEADER_SWITCH: usize = 7;
const ID_HEADER_BUTTON: usize = 8;
/// A muted Windows critical red: the wash and edge of the destructive header
/// button (a space's "Delete space").
const DELETE_RED: Color = Color::rgb(0xC4, 0x2B, 0x1C);
/// Height of one navigation-rail row, at 96 DPI.
const NAV_ROW_HEIGHT: i32 = 30;
/// Width of the navigation rail, at 96 DPI.
const NAV_WIDTH: i32 = 168;
/// Width of the scopes pane, at 96 DPI.
const SCOPE_WIDTH: i32 = 176;
/// How far an indented scope row is moved right, at 96 DPI.
const SCOPE_INDENT: i32 = 16;
/// Height of the header line above the page, at 96 DPI.
const HEADER_HEIGHT: i32 = 44;
/// Width of the header switch, at 96 DPI.
const HEADER_SWITCH_WIDTH: i32 = 200;
/// Width of the header action button (for example "Delete space"), at 96 DPI.
const HEADER_BUTTON_WIDTH: i32 = 132;
/// Field controls get `CONTROL_BASE + index * 4 + part`, so the field index
/// and which of its controls fired can be recovered from the id alone.
const CONTROL_BASE: usize = 1000;
const PART_MAIN: usize = 0;
const PART_SECONDARY: usize = 1;

/// Static and trackbar styles that `windows` exposes under different types
/// than `WINDOW_STYLE`; the values are from WinUser.h / CommCtrl.h.
/// https://learn.microsoft.com/en-us/windows/win32/controls/static-control-styles
const SS_LEFT: u32 = 0x0000;
const SS_CENTER: u32 = 0x0001;
const SS_RIGHT: u32 = 0x0002;
const SS_NOPREFIX: u32 = 0x0080;
const SS_CENTERIMAGE: u32 = 0x0200;
/// Trackbar messages are WM_USER-based: TBM_GETPOS is WM_USER, TBM_SETTIC
/// WM_USER + 4, TBM_GETTHUMBRECT WM_USER + 25; TBM_SETRANGEMAX follows
/// TBM_SETRANGEMIN.
/// https://learn.microsoft.com/en-us/windows/win32/controls/bumper-trackbar-control-reference-messages
const TBM_GETPOS: u32 = 0x0400;
const TBM_SETTIC: u32 = 0x0404;
const TBM_GETTHUMBRECT: u32 = 0x0419;
const TBM_SETRANGEMAX: u32 = TBM_SETRANGEMIN + 1;
/// Height of the labels under a slider's tick marks, at 96 DPI.
const MARK_LABEL_HEIGHT: i32 = 16;

static REGISTER_CLASSES: Once = Once::new();
static CLASSES_REGISTERED: AtomicBool = AtomicBool::new(false);

/// The controls created for one field.
pub(super) struct FieldControls {
    label: Option<HWND>,
    main: Option<HWND>,
    /// Slider value text, or the hex text beside a color swatch.
    value_text: Option<HWND>,
    /// An editable slider value box, in place of `value_text`.
    value_edit: Option<HWND>,
    swatch: Option<HWND>,
    swatch_brush: Option<HBRUSH>,
    /// Labels under a slider's tick marks.
    marks: Vec<HWND>,
}

pub(super) struct SettingsState {
    form: SettingsForm,
    fields: Vec<(Field, FieldControls)>,
    pages: Vec<HWND>,
    /// The navigation rail's categories pane.
    list: Option<HWND>,
    /// The rail's rows, rebuilt when the nav changes.
    nav_rows: Vec<NavRow>,
    /// The scopes pane, when the form had scopes at build time.
    scopes_list: Option<HWND>,
    /// The scopes pane's rows, rebuilt when they change.
    scope_rows: Vec<SettingsScope>,
    /// The header's title, switch, and action button, when the form has a rail.
    header_title: Option<HWND>,
    header_switch: Option<HWND>,
    header_button: Option<HWND>,
    /// Vertical lines between the panes: x, top, bottom.
    dividers: Vec<(i32, i32, i32)>,
    apply: Option<HWND>,
    font: HFONT,
    heading_font: HFONT,
    /// The layout DPI, for scaling the rail's paint metrics.
    dpi: u32,
    command_bar_top: i32,
    /// Set while the window itself is changing control values, so the
    /// resulting notifications are not reported as user edits.
    updating: bool,
    colors: UiColors,
    body_brush: HBRUSH,
    command_bar_brush: HBRUSH,
    /// The accent fill for the rail's selected bar and the marked dot.
    accent_brush: HBRUSH,
}

impl SettingsState {
    /// Switches to `colors`, replacing the background brushes.
    fn set_colors(&mut self, colors: UiColors) {
        // SAFETY: the old brushes are only selected while painting, which is
        // not happening during this short borrow.
        unsafe {
            for old in [self.body_brush, self.command_bar_brush, self.accent_brush] {
                if !old.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(old.0));
                }
            }
            self.body_brush = CreateSolidBrush(colorref(colors.body));
            self.command_bar_brush = CreateSolidBrush(colorref(colors.command_bar));
            self.accent_brush = CreateSolidBrush(colorref(colors.accent));
        }
        self.colors = colors;
    }
}

impl FieldControls {
    /// Static text and the color swatch. These are never disabled: Windows
    /// draws disabled static text embossed and ignores WM_CTLCOLORSTATIC,
    /// which is unreadable on the dark body. They are greyed through
    /// WM_CTLCOLORSTATIC instead (`SettingsState::is_disabled_label`).
    pub(super) fn labels(&self) -> Vec<HWND> {
        [self.label, self.value_text, self.swatch]
            .into_iter()
            .flatten()
            .chain(self.marks.iter().copied())
            .collect()
    }

    fn owns_label(&self, control: HWND) -> bool {
        [self.label, self.value_text].contains(&Some(control)) || self.marks.contains(&control)
    }
}

impl SettingsState {
    fn is_disabled_label(&self, control: HWND) -> bool {
        self.fields
            .iter()
            .any(|(field, controls)| !field.enabled && controls.owns_label(control))
    }

    fn field_index_for(&self, control: HWND) -> Option<usize> {
        self.fields.iter().position(|(_, controls)| {
            controls.main == Some(control) || controls.swatch == Some(control)
        })
    }
}

impl Drop for SettingsState {
    fn drop(&mut self) {
        // SAFETY: the fonts and brushes are owned by this state and the
        // controls using them are already destroyed (children are destroyed
        // before the parent's WM_NCDESTROY).
        unsafe {
            for (_, controls) in &self.fields {
                if let Some(brush) = controls.swatch_brush {
                    let _ = DeleteObject(HGDIOBJ(brush.0));
                }
            }
            let _ = DeleteObject(HGDIOBJ(self.font.0));
            let _ = DeleteObject(HGDIOBJ(self.heading_font.0));
            let _ = DeleteObject(HGDIOBJ(self.body_brush.0));
            let _ = DeleteObject(HGDIOBJ(self.command_bar_brush.0));
            let _ = DeleteObject(HGDIOBJ(self.accent_brush.0));
        }
    }
}

pub(crate) fn create(form: &SettingsForm, dark: bool) -> Result<HWND, PlatformError> {
    // SAFETY: Win32 window creation on the UI thread. Ownership of the state
    // passes to the window in WM_NCCREATE; it is freed in WM_NCDESTROY.
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        ensure_classes(instance)?;
        let mut state = SettingsState {
            form: form.clone(),
            fields: Vec::new(),
            pages: Vec::new(),
            list: None,
            nav_rows: Vec::new(),
            scopes_list: None,
            scope_rows: Vec::new(),
            header_title: None,
            header_switch: None,
            header_button: None,
            dividers: Vec::new(),
            apply: None,
            font: HFONT::default(),
            heading_font: HFONT::default(),
            dpi: 0,
            command_bar_top: 0,
            updating: true,
            colors: UiColors::new(dark),
            body_brush: HBRUSH::default(),
            command_bar_brush: HBRUSH::default(),
            accent_brush: HBRUSH::default(),
        };
        state.set_colors(UiColors::new(dark));
        let state = Box::into_raw(Box::new(state));
        let title = wide(&form.title);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            CLASS_NAME,
            PCWSTR(title.as_ptr()),
            window_style(),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            100,
            100,
            None,
            None,
            Some(instance),
            Some(state as *const core::ffi::c_void),
        )?;

        build(hwnd, form);

        let icon = tray::shared_icon();
        SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(icon.0 as isize)),
        );
        SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(icon.0 as isize)),
        );
        // Mica in the caption, like other Windows 11 settings windows. The
        // client area stays opaque because classic controls do not support
        // a translucent background. Best effort: ignored before 22H2.
        let mica = DWMSBT_MAINWINDOW;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &mica as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&mica) as u32,
        );
        dark_mode::set_caption_dark(hwnd, dark);
        theme_controls(hwnd, dark);

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        Ok(hwnd)
    }
}

/// Switches an open settings window between dark and light.
pub(crate) fn set_dark(hwnd: HWND, dark: bool) {
    // SAFETY: `hwnd` is the live settings window; the state borrow ends
    // before any call that sends messages.
    unsafe {
        let changed = with_state(hwnd, |state| {
            let changed = state.colors.dark != dark;
            if changed {
                state.set_colors(UiColors::new(dark));
            }
            changed
        });
        if changed != Some(true) {
            return;
        }
        dark_mode::set_caption_dark(hwnd, dark);
        theme_controls(hwnd, dark);
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_FRAME,
        );
    }
}

/// Gives every control the dark or light visual style for its class.
unsafe fn theme_controls(window: HWND, dark: bool) {
    unsafe extern "system" fn visit(control: HWND, dark: LPARAM) -> BOOL {
        let mut class = [0u16; 32];
        // SAFETY: `control` is a live descendant being enumerated; the
        // buffer is sized for the call.
        let length = unsafe { GetClassNameW(control, &mut class) } as usize;
        let theme = match String::from_utf16_lossy(&class[..length]).as_str() {
            "Button" | "msctls_trackbar32" => Some(ControlTheme::Explorer),
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
        let _ = EnumChildWindows(Some(window), Some(visit), LPARAM(dark as isize));
    }
}

pub(crate) fn bring_to_front(hwnd: HWND) {
    // SAFETY: `hwnd` is the live settings window.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

fn window_style() -> WINDOW_STYLE {
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN
}

fn ensure_classes(instance: HINSTANCE) -> Result<(), PlatformError> {
    REGISTER_CLASSES.call_once(|| {
        // No class background brushes: both window procedures erase with
        // the current light or dark body color (WM_ERASEBKGND).
        // SAFETY: class structs are fully initialised; strings are static.
        unsafe {
            let main = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            let page = WNDCLASSW {
                lpfnWndProc: Some(page_wndproc),
                hInstance: instance,
                lpszClassName: PAGE_CLASS,
                ..Default::default()
            };
            let registered = RegisterClassW(&main) != 0 && RegisterClassW(&page) != 0;
            CLASSES_REGISTERED.store(registered, Ordering::SeqCst);
        }
    });
    if CLASSES_REGISTERED.load(Ordering::SeqCst) {
        Ok(())
    } else {
        Err(PlatformError::ClassRegistration)
    }
}

unsafe fn state_of(window: HWND) -> *mut SettingsState {
    // SAFETY: reading the user-data slot of a window of our class.
    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut SettingsState }
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
pub(super) unsafe fn with_state<R>(
    window: HWND,
    f: impl FnOnce(&mut SettingsState) -> R,
) -> Option<R> {
    // SAFETY: a non-null pointer is the live state (WM_NCDESTROY clears the
    // slot before freeing), and `f` cannot re-enter to create a second borrow.
    unsafe { state_of(window).as_mut() }.map(f)
}

pub(super) fn settings_event(event: SettingsEvent) {
    runtime::dispatch(Event::Settings(event));
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
                let create = lparam.0 as *const CREATESTRUCTW;
                if !create.is_null() {
                    let state = (*create).lpCreateParams as *mut SettingsState;
                    if !state.is_null() {
                        SetWindowLongPtrW(window, GWLP_USERDATA, state as isize);
                    }
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xFFFF;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                let control = HWND(lparam.0 as *mut core::ffi::c_void);
                match id {
                    ID_OK => settings_event(SettingsEvent::Ok),
                    ID_CANCEL => settings_event(SettingsEvent::Cancel),
                    ID_APPLY => settings_event(SettingsEvent::Apply),
                    ID_LIST if code == LBN_SELCHANGE => on_nav_changed(window),
                    ID_SCOPES if code == LBN_SELCHANGE => on_scope_changed(window),
                    ID_HEADER_SWITCH if code == BN_CLICKED => on_header_switch(window, control),
                    ID_HEADER_BUTTON if code == BN_CLICKED => {
                        settings_event(SettingsEvent::HeaderButton)
                    }
                    id if id >= CONTROL_BASE => on_field_command(window, id, code, control),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_HSCROLL => {
                if lparam.0 != 0 {
                    on_scroll(window, HWND(lparam.0 as *mut core::ffi::c_void));
                }
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                let control = HWND(lparam.0 as *mut core::ffi::c_void);
                let Some((swatch, colors, body, field, disabled, rail)) =
                    with_state(window, |state| {
                        let swatch = state
                            .fields
                            .iter()
                            .find(|(_, controls)| controls.swatch == Some(control))
                            .and_then(|(_, controls)| controls.swatch_brush);
                        (
                            swatch,
                            state.colors,
                            state.body_brush,
                            state.command_bar_brush,
                            state.is_disabled_label(control),
                            [state.list, state.scopes_list].contains(&Some(control)),
                        )
                    })
                else {
                    return DefWindowProcW(window, message, wparam, lparam);
                };
                if let Some(brush) = swatch {
                    return LRESULT(brush.0 as isize);
                }
                let dc = HDC(wparam.0 as *mut core::ffi::c_void);
                // The rail panes are part of the window body, not input
                // fields: below their rows they show the body color too.
                if rail {
                    SetBkColor(dc, colorref(colors.body));
                    return LRESULT(body.0 as isize);
                }
                let editable = matches!(message, WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX);
                if editable && !colors.dark {
                    return DefWindowProcW(window, message, wparam, lparam);
                }
                // Labels, checkboxes, trackbars, and read-only edits sit on
                // the page body; in dark mode, editable fields and drop-down
                // lists use the slightly darker field color.
                let (background, brush) = if editable {
                    (colors.command_bar, field)
                } else {
                    (colors.body, body)
                };
                let text = if disabled {
                    colors.disabled_text
                } else {
                    colors.text
                };
                SetTextColor(dc, colorref(text));
                SetBkColor(dc, colorref(background));
                LRESULT(brush.0 as isize)
            }
            WM_DRAWITEM => {
                let item = lparam.0 as *const DRAWITEMSTRUCT;
                // SAFETY: Windows passes a live DRAWITEMSTRUCT for an
                // owner-draw control of this window.
                if item.is_null() {
                    return DefWindowProcW(window, message, wparam, lparam);
                }
                match (*item).CtlID as usize {
                    ID_LIST => on_draw_item(window, &*item),
                    ID_SCOPES => on_draw_scope(window, &*item),
                    ID_HEADER_BUTTON => on_draw_header_button(window, &*item),
                    _ => return DefWindowProcW(window, message, wparam, lparam),
                }
                LRESULT(1)
            }
            WM_ERASEBKGND => {
                let dc = HDC(wparam.0 as *mut core::ffi::c_void);
                if erase_body(window, window, dc) {
                    LRESULT(1)
                } else {
                    DefWindowProcW(window, message, wparam, lparam)
                }
            }
            WM_PAINT => {
                on_paint(window);
                LRESULT(0)
            }
            WM_CLOSE => {
                settings_event(SettingsEvent::Cancel);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                runtime::settings_closed(window);
                let state = state_of(window);
                if !state.is_null() {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                    drop(Box::from_raw(state));
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

/// Page containers forward control notifications to the settings window.
unsafe extern "system" fn page_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: forwarding a message with its original arguments to our parent.
    unsafe {
        match message {
            WM_COMMAND | WM_HSCROLL | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT
            | WM_CTLCOLORLISTBOX => match GetParent(window) {
                Ok(parent) => SendMessageW(parent, message, Some(wparam), Some(lparam)),
                Err(_) => DefWindowProcW(window, message, wparam, lparam),
            },
            WM_ERASEBKGND => {
                let dc = HDC(wparam.0 as *mut core::ffi::c_void);
                let settings = GetParent(window).unwrap_or_default();
                if erase_body(settings, window, dc) {
                    LRESULT(1)
                } else {
                    DefWindowProcW(window, message, wparam, lparam)
                }
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}
