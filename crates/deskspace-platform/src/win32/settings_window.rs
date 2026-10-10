//! The settings window: a native, data-driven form. The app describes pages
//! and fields with `ui::SettingsForm`; this module lays them out with standard
//! Win32 controls (common controls v6 via the app's manifest) under a tab
//! control, and reports edits as `SettingsEvent`s.
//!
//! Each page is a container window (`PAGE_CLASS`) that forwards control
//! notifications to the settings window, so every control can be found by id
//! in one place. Window state lives behind a raw `*mut SettingsState` in
//! `GWLP_USERDATA` and is freed exactly once, in `WM_NCDESTROY`.

use std::cell::RefCell;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use deskspace_core::theme::Color;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontIndirectW, CreateSolidBrush, DT_CALCRECT, DT_CENTER, DT_END_ELLIPSIS,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteObject, DrawFocusRect, DrawTextW,
    Ellipse, EndPaint, FillRect, GetDC, HBRUSH, HDC, HFONT, HGDIOBJ, InvalidateRect, PAINTSTRUCT,
    RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RedrawWindow, ReleaseDC, SelectObject,
    SetBkColor, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, NMHDR, ODS_SELECTED, TBM_SETPOS, TBM_SETRANGEMIN, TBS_HORZ, TBS_NOTICKS,
    TCIF_TEXT, TCITEMW, TCM_ADJUSTRECT, TCM_GETCURSEL, TCM_GETITEMRECT, TCM_INSERTITEMW,
    TCN_SELCHANGE, TRACKBAR_CLASSW, WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_LISTBOXW, WC_STATICW,
    WC_TABCONTROLW,
};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SystemParametersInfoForDpi,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON,
    CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, CREATESTRUCTW,
    CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, EN_CHANGE, EN_KILLFOCUS, ES_AUTOHSCROLL,
    ES_NUMBER, ES_READONLY, ES_RIGHT, EnumChildWindows, GWLP_USERDATA, GetClassNameW,
    GetClientRect, GetCursorPos, GetParent, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, HMENU, HWND_TOP, ICON_BIG, ICON_SMALL, IsIconic, LB_ADDSTRING, LB_GETCURSEL,
    LB_RESETCONTENT, LB_SETCURSEL, LB_SETITEMHEIGHT, LBN_SELCHANGE, LBS_HASSTRINGS, LBS_NOTIFY,
    LBS_OWNERDRAWFIXED, NONCLIENTMETRICSW, RegisterClassW, SPI_GETNONCLIENTMETRICS, SW_HIDE,
    SW_RESTORE, SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC,
    WM_DRAWITEM, WM_ERASEBKGND, WM_HSCROLL, WM_KILLFOCUS, WM_NCCREATE, WM_NCDESTROY, WM_NOTIFY,
    WM_PAINT, WM_SETFOCUS, WM_SETFONT, WM_SETICON, WNDCLASSW, WS_CAPTION, WS_CHILD,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
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

const CLASS_NAME: PCWSTR = w!("DeskSpaceSettings");
const PAGE_CLASS: PCWSTR = w!("DeskSpaceSettingsPage");

const ID_OK: usize = 1;
const ID_CANCEL: usize = 2;
const ID_APPLY: usize = 3;
const ID_TAB: usize = 4;
const ID_LIST: usize = 5;
const ID_SCOPES: usize = 6;
const ID_HEADER_SWITCH: usize = 7;
const TAB_SUBCLASS_ID: usize = 1;
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

thread_local! {
    /// The 16 "custom colors" slots of the color picker, kept for the session.
    static CUSTOM_COLORS: RefCell<[COLORREF; 16]> =
        const { RefCell::new([COLORREF(0x00FF_FFFF); 16]) };
}

/// The controls created for one field.
struct FieldControls {
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

struct SettingsState {
    form: SettingsForm,
    fields: Vec<(Field, FieldControls)>,
    pages: Vec<HWND>,
    tab: Option<HWND>,
    /// The navigation rail, when the form supplies one (`SettingsForm::nav`).
    list: Option<HWND>,
    /// The rail's rows, rebuilt when the nav changes.
    nav_rows: Vec<NavRow>,
    /// The scopes pane, when the form had scopes at build time.
    scopes_list: Option<HWND>,
    /// The scopes pane's rows, rebuilt when they change.
    scope_rows: Vec<SettingsScope>,
    /// The header's title and switch, when the form has a rail.
    header_title: Option<HWND>,
    header_switch: Option<HWND>,
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
    fn labels(&self) -> Vec<HWND> {
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

/// One row of the navigation rail. Group titles are non-selectable.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NavRow {
    Title(String),
    Entry { label: String, marked: bool },
}

impl NavRow {
    fn label(&self) -> &str {
        match self {
            Self::Title(title) => title,
            Self::Entry { label, .. } => label,
        }
    }

    fn is_entry(&self) -> bool {
        matches!(self, Self::Entry { .. })
    }
}

/// The rail's rows: each group's title (untitled groups have none), then its
/// entries.
fn nav_rows(nav: &SettingsNav) -> Vec<NavRow> {
    let mut rows = Vec::new();
    for group in &nav.groups {
        if !group.title.is_empty() {
            rows.push(NavRow::Title(group.title.clone()));
        }
        rows.extend(group.entries.iter().map(|entry| NavRow::Entry {
            label: entry.label.clone(),
            marked: entry.marked,
        }));
    }
    rows
}

/// The flattened entry index of rail row `row`, or `None` for a title.
fn row_entry(rows: &[NavRow], row: usize) -> Option<usize> {
    if !rows.get(row)?.is_entry() {
        return None;
    }
    Some(rows[..row].iter().filter(|row| row.is_entry()).count())
}

/// The rail row showing flattened entry `entry`, or the first entry row.
fn entry_row(rows: &[NavRow], entry: usize) -> usize {
    let mut seen = 0;
    for (index, row) in rows.iter().enumerate() {
        if row.is_entry() {
            if seen == entry {
                return index;
            }
            seen += 1;
        }
    }
    rows.iter().position(NavRow::is_entry).unwrap_or(0)
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
            tab: None,
            list: None,
            nav_rows: Vec::new(),
            scopes_list: None,
            scope_rows: Vec::new(),
            header_title: None,
            header_switch: None,
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

/// Gives every control the dark or light visual style for its class. The
/// tab control is painted by `paint_tab` instead, because it has no dark
/// style.
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

/// Converts 96-DPI layout units to pixels at `dpi`.
fn scaled(value: i32, dpi: u32) -> i32 {
    (value as i64 * dpi as i64 / 96) as i32
}

/// The UI font (Segoe UI 9pt on Windows 11) at `dpi`, plus a larger semibold
/// variant for section headings.
unsafe fn create_fonts(dpi: u32) -> (HFONT, HFONT) {
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    // SAFETY: `metrics` is sized for SPI_GETNONCLIENTMETRICS.
    unsafe {
        let _ = SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some(&mut metrics as *mut _ as *mut core::ffi::c_void),
            0,
            dpi,
        );
        let body = metrics.lfMessageFont;
        let mut heading = body;
        heading.lfWeight = 600;
        heading.lfHeight = body.lfHeight * 4 / 3;
        (CreateFontIndirectW(&body), CreateFontIndirectW(&heading))
    }
}

struct Builder {
    instance: HINSTANCE,
    dpi: u32,
    font: HFONT,
    heading_font: HFONT,
}

impl Builder {
    fn s(&self, value: i32) -> i32 {
        scaled(value, self.dpi)
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn control(
        &self,
        parent: HWND,
        class: PCWSTR,
        text: &str,
        style: u32,
        ex_style: WINDOW_EX_STYLE,
        bounds: (i32, i32, i32, i32),
        id: usize,
    ) -> Option<HWND> {
        let text = wide(text);
        // SAFETY: creating a child control of our own window.
        unsafe {
            let hwnd = CreateWindowExW(
                ex_style,
                class,
                PCWSTR(text.as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
                bounds.0,
                bounds.1,
                bounds.2,
                bounds.3,
                Some(parent),
                Some(HMENU(id as *mut core::ffi::c_void)),
                Some(self.instance),
                None,
            )
            .ok()?;
            SendMessageW(
                hwnd,
                WM_SETFONT,
                Some(WPARAM(self.font.0 as usize)),
                Some(LPARAM(1)),
            );
            Some(hwnd)
        }
    }

    unsafe fn label(&self, parent: HWND, text: &str, bounds: (i32, i32, i32, i32)) -> Option<HWND> {
        // SAFETY: see `control`.
        unsafe {
            self.control(
                parent,
                WC_STATICW,
                text,
                SS_NOPREFIX | SS_CENTERIMAGE,
                WINDOW_EX_STYLE(0),
                bounds,
                0,
            )
        }
    }

    /// Adds tick marks to `slider` and a label under each. Each label is
    /// centered on the thumb position for its value, which the trackbar
    /// reports exactly (TBM_GETTHUMBRECT) rather than this code guessing its
    /// channel insets, and is only as wide as the space to its neighbors so
    /// labels never overlap. Labels at the ends of the range are aligned
    /// inward so they stay inside the page.
    unsafe fn slider_marks(
        &self,
        page: HWND,
        slider: HWND,
        slider_x: i32,
        top: i32,
        (min, max): (i32, i32),
        marks: &[SliderMark],
    ) -> Vec<HWND> {
        let mut centers = Vec::with_capacity(marks.len());
        let (mut first_left, mut last_right) = (slider_x, slider_x);
        // SAFETY: `slider` is a live trackbar of this window; the thumb rect
        // is a valid out-parameter for TBM_GETTHUMBRECT.
        unsafe {
            for mark in marks {
                SendMessageW(slider, TBM_SETTIC, None, Some(LPARAM(mark.value as isize)));
                // The trackbar only recomputes its thumb rect when the
                // position change is redrawn (wParam TRUE).
                SendMessageW(
                    slider,
                    TBM_SETPOS,
                    Some(WPARAM(1)),
                    Some(LPARAM(mark.value as isize)),
                );
                let mut thumb = RECT::default();
                SendMessageW(
                    slider,
                    TBM_GETTHUMBRECT,
                    None,
                    Some(LPARAM(&mut thumb as *mut RECT as isize)),
                );
                if mark.value <= min {
                    first_left = slider_x + thumb.left;
                }
                if mark.value >= max {
                    last_right = slider_x + thumb.right;
                }
                centers.push(slider_x + (thumb.left + thumb.right) / 2);
            }
        }
        let widest = self.s(80);
        let mut labels = Vec::with_capacity(marks.len());
        for (index, mark) in marks.iter().enumerate() {
            let center = centers[index];
            let gap_before = index.checked_sub(1).map(|i| center - centers[i]);
            let gap_after = centers.get(index + 1).map(|next| next - center);
            let (x, width, align) = if mark.value <= min {
                let width = gap_after.map_or(widest, |gap| gap - gap / 2).min(widest);
                (first_left, width, SS_LEFT)
            } else if mark.value >= max {
                let width = gap_before.map_or(widest, |gap| gap - gap / 2).min(widest);
                (last_right - width, width, SS_RIGHT)
            } else {
                let width = gap_before
                    .into_iter()
                    .chain(gap_after)
                    .min()
                    .unwrap_or(widest)
                    .min(widest);
                (center - width / 2, width, SS_CENTER)
            };
            // SAFETY: creating a child control of our own page.
            labels.extend(unsafe {
                self.control(
                    page,
                    WC_STATICW,
                    &mark.label,
                    SS_NOPREFIX | align,
                    WINDOW_EX_STYLE(0),
                    (x, top, width, self.s(MARK_LABEL_HEIGHT)),
                    0,
                )
            });
        }
        labels
    }
    unsafe fn text_height(&self, parent: HWND, text: &str, width: i32) -> i32 {
        let mut buffer: Vec<u16> = text.encode_utf16().collect();
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: 0,
        };
        // SAFETY: the DC is released below; the font outlives the call.
        unsafe {
            let dc: HDC = GetDC(Some(parent));
            let previous = SelectObject(dc, HGDIOBJ(self.font.0));
            DrawTextW(
                dc,
                &mut buffer,
                &mut bounds,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
            );
            SelectObject(dc, previous);
            ReleaseDC(Some(parent), dc);
        }
        bounds.bottom - bounds.top
    }

    /// Creates the controls for one field at `y` inside `page` and returns the
    /// controls and the height used.
    unsafe fn field(
        &self,
        page: HWND,
        index: usize,
        field: &Field,
        y: i32,
        width: i32,
    ) -> (FieldControls, i32) {
        let x = self.s(16);
        let full = width - 2 * x;
        let label_width = self.s(184);
        let control_x = x + label_width + self.s(12);
        let control_width = width - control_x - x;
        let id_main = CONTROL_BASE + index * 4 + PART_MAIN;
        let id_secondary = CONTROL_BASE + index * 4 + PART_SECONDARY;
        let tab = WS_TABSTOP.0;
        let mut controls = FieldControls {
            label: None,
            main: None,
            value_text: None,
            value_edit: None,
            swatch: None,
            swatch_brush: None,
            marks: Vec::new(),
        };
        let row = self.s(28);
        let label_bounds = (x, y, label_width, row);

        // SAFETY: all controls are children of our page window.
        let height = unsafe {
            match &field.kind {
                FieldKind::Heading => {
                    let top = if y > self.s(16) { self.s(10) } else { 0 };
                    let label = self.label(page, &field.label, (x, y + top, full, self.s(28)));
                    if let Some(label) = label {
                        SendMessageW(
                            label,
                            WM_SETFONT,
                            Some(WPARAM(self.heading_font.0 as usize)),
                            Some(LPARAM(1)),
                        );
                    }
                    controls.label = label;
                    top + self.s(34)
                }
                FieldKind::Note => {
                    let text_height = self.text_height(page, &field.label, full);
                    controls.label = self.control(
                        page,
                        WC_STATICW,
                        &field.label,
                        SS_NOPREFIX,
                        WINDOW_EX_STYLE(0),
                        (x, y, full, text_height),
                        0,
                    );
                    text_height + self.s(10)
                }
                FieldKind::Choice { options, selected } => {
                    controls.label = self.label(page, &field.label, label_bounds);
                    // A drop-down list's height includes the open list.
                    let combo = self.control(
                        page,
                        WC_COMBOBOXW,
                        "",
                        CBS_DROPDOWNLIST as u32 | tab,
                        WINDOW_EX_STYLE(0),
                        (control_x, y, control_width, self.s(240)),
                        id_main,
                    );
                    if let Some(combo) = combo {
                        for option in options {
                            let text = wide(option);
                            SendMessageW(
                                combo,
                                CB_ADDSTRING,
                                None,
                                Some(LPARAM(text.as_ptr() as isize)),
                            );
                        }
                        SendMessageW(combo, CB_SETCURSEL, Some(WPARAM(*selected)), None);
                    }
                    controls.main = combo;
                    self.s(38)
                }
                FieldKind::Toggle { on } => {
                    let check = self.control(
                        page,
                        WC_BUTTONW,
                        &field.label,
                        BS_AUTOCHECKBOX as u32 | tab,
                        WINDOW_EX_STYLE(0),
                        (x, y, full, row),
                        id_main,
                    );
                    if let Some(check) = check {
                        SendMessageW(check, BM_SETCHECK, Some(WPARAM(*on as usize)), None);
                    }
                    controls.main = check;
                    self.s(34)
                }
                FieldKind::Slider {
                    min,
                    max,
                    value,
                    suffix,
                    editable,
                    marks,
                } => {
                    controls.label = self.label(page, &field.label, label_bounds);
                    let value_width = self.s(52);
                    let ticks = if marks.is_empty() { TBS_NOTICKS } else { 0 };
                    let slider_x = control_x - self.s(6);
                    let slider = self.control(
                        page,
                        TRACKBAR_CLASSW,
                        "",
                        TBS_HORZ | ticks | tab,
                        WINDOW_EX_STYLE(0),
                        (slider_x, y, control_width - value_width, row),
                        id_main,
                    );
                    if let Some(slider) = slider {
                        SendMessageW(
                            slider,
                            TBM_SETRANGEMIN,
                            Some(WPARAM(0)),
                            Some(LPARAM(*min as isize)),
                        );
                        SendMessageW(
                            slider,
                            TBM_SETRANGEMAX,
                            Some(WPARAM(0)),
                            Some(LPARAM(*max as isize)),
                        );
                        if !marks.is_empty() {
                            controls.marks = self.slider_marks(
                                page,
                                slider,
                                slider_x,
                                y + row,
                                (*min, *max),
                                marks,
                            );
                        }
                        SendMessageW(
                            slider,
                            TBM_SETPOS,
                            Some(WPARAM(1)),
                            Some(LPARAM(*value as isize)),
                        );
                    }
                    controls.main = slider;
                    let value_bounds = (
                        control_x + control_width - value_width + self.s(4),
                        y,
                        value_width,
                        row,
                    );
                    if *editable {
                        controls.value_edit = self.control(
                            page,
                            WC_EDITW,
                            &value.to_string(),
                            ES_RIGHT as u32 | ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | tab,
                            WS_EX_CLIENTEDGE,
                            (value_bounds.0, y + self.s(2), value_bounds.2, self.s(24)),
                            id_secondary,
                        );
                    } else {
                        controls.value_text =
                            self.label(page, &format!("{value}{suffix}"), value_bounds);
                    }
                    if marks.is_empty() {
                        self.s(38)
                    } else {
                        row + self.s(MARK_LABEL_HEIGHT + 14)
                    }
                }
                FieldKind::Color { color } => {
                    controls.label = self.label(page, &field.label, label_bounds);
                    let swatch_size = self.s(24);
                    controls.swatch = self.control(
                        page,
                        WC_STATICW,
                        "",
                        0,
                        WS_EX_CLIENTEDGE,
                        (
                            control_x,
                            y + (row - swatch_size) / 2,
                            swatch_size,
                            swatch_size,
                        ),
                        id_secondary,
                    );
                    controls.swatch_brush = Some(CreateSolidBrush(colorref(*color)));
                    controls.value_text = self.label(
                        page,
                        &color.to_rgb_hex(),
                        (control_x + swatch_size + self.s(10), y, self.s(76), row),
                    );
                    controls.main = self.control(
                        page,
                        WC_BUTTONW,
                        "Choose\u{2026}",
                        BS_PUSHBUTTON as u32 | tab,
                        WINDOW_EX_STYLE(0),
                        (control_x + swatch_size + self.s(92), y, self.s(100), row),
                        id_main,
                    );
                    self.s(38)
                }
                FieldKind::Text { value, read_only } => {
                    controls.label = self.label(page, &field.label, label_bounds);
                    let mut style = ES_AUTOHSCROLL as u32 | tab;
                    if *read_only {
                        style |= ES_READONLY as u32;
                    }
                    controls.main = self.control(
                        page,
                        WC_EDITW,
                        value,
                        style,
                        WS_EX_CLIENTEDGE,
                        (control_x, y + self.s(2), control_width, self.s(24)),
                        id_main,
                    );
                    self.s(38)
                }
                FieldKind::Button { caption } => {
                    let left = if field.label.is_empty() {
                        x
                    } else {
                        controls.label = self.label(page, &field.label, label_bounds);
                        control_x
                    };
                    controls.main = self.control(
                        page,
                        WC_BUTTONW,
                        caption,
                        BS_PUSHBUTTON as u32 | tab,
                        WINDOW_EX_STYLE(0),
                        (left, y, self.s(180), self.s(30)),
                        id_main,
                    );
                    self.s(40)
                }
            }
        };
        (controls, height)
    }
}

unsafe fn build(hwnd: HWND, form: &SettingsForm) {
    // SAFETY: every call creates or positions children of `hwnd`; no state
    // borrow is held while controls are created.
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let (font, heading_font) = create_fonts(dpi);
        let builder = Builder {
            instance: HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
            dpi,
            font,
            heading_font,
        };
        let s = |value| builder.s(value);
        let margin = s(12);
        let page_width = s(540);

        let nav = form.nav.as_ref();
        let tab = if nav.is_none() {
            let tab = builder.control(
                hwnd,
                WC_TABCONTROLW,
                "",
                (WS_CLIPSIBLINGS | WS_TABSTOP).0,
                WINDOW_EX_STYLE(0),
                (margin, margin, page_width, s(100)),
                ID_TAB,
            );
            if let Some(tab) = tab {
                let _ = SetWindowSubclass(tab, Some(tab_proc), TAB_SUBCLASS_ID, 0);
                for (index, page) in form.pages.iter().enumerate() {
                    let mut text = wide(&page.title);
                    let item = TCITEMW {
                        mask: TCIF_TEXT,
                        pszText: windows::core::PWSTR(text.as_mut_ptr()),
                        ..Default::default()
                    };
                    SendMessageW(
                        tab,
                        TCM_INSERTITEMW,
                        Some(WPARAM(index)),
                        Some(LPARAM(&item as *const _ as isize)),
                    );
                }
            }
            tab
        } else {
            None
        };

        let mut pages = Vec::new();
        let mut fields = Vec::new();
        let mut page_height = 0;
        for (page_index, page) in form.pages.iter().enumerate() {
            let container = CreateWindowExW(
                WS_EX_CONTROLPARENT,
                PAGE_CLASS,
                w!(""),
                WS_CHILD
                    | WS_CLIPCHILDREN
                    | if nav.is_none() && page_index == 0 {
                        WS_VISIBLE
                    } else {
                        WINDOW_STYLE(0)
                    },
                0,
                0,
                page_width,
                s(100),
                Some(hwnd),
                None,
                Some(builder.instance),
                None,
            );
            let Ok(container) = container else {
                continue;
            };
            let mut y = s(16);
            for field in &page.fields {
                let index = fields.len();
                let (controls, height) = builder.field(container, index, field, y, page_width);
                apply_enabled(&controls, field.enabled);
                fields.push((field.clone(), controls));
                y += height;
            }
            page_height = page_height.max(y + s(8));
            pages.push(container);
        }

        // Lay the pages over the tab's display area (tabs) or beside the rail
        // (nav), and compute the client size.
        let (client_width, command_bar_top, list);
        let mut scopes_list = None;
        let (mut header_title, mut header_switch) = (None, None);
        let mut dividers = Vec::new();
        if let Some(nav) = nav {
            let header_height = s(HEADER_HEIGHT);
            let panes_height = header_height + page_height;
            let rail = s(NAV_WIDTH);
            let mut x = margin;
            if !nav.scopes.is_empty() {
                let width = s(SCOPE_WIDTH);
                scopes_list =
                    rail_list(&builder, hwnd, (x, margin, width, panes_height), ID_SCOPES);
                if let Some(created) = scopes_list {
                    fill_list(created, nav.scopes.iter().map(|scope| scope.label.as_str()));
                    SendMessageW(
                        created,
                        LB_SETCURSEL,
                        Some(WPARAM(nav.selected_scope)),
                        None,
                    );
                }
                x += width + s(6);
                dividers.push((x - s(3), margin, margin + panes_height));
            }
            let rows = nav_rows(nav);
            list = rail_list(&builder, hwnd, (x, margin, rail, panes_height), ID_LIST);
            if let Some(created) = list {
                fill_list(created, rows.iter().map(NavRow::label));
                let selected = entry_row(&rows, nav.selected);
                SendMessageW(created, LB_SETCURSEL, Some(WPARAM(selected)), None);
            }
            x += rail + s(6);
            dividers.push((x - s(3), margin, margin + panes_height));
            let pages_x = x + s(6);
            client_width = pages_x + page_width + margin;
            command_bar_top = margin + panes_height + margin;

            // The header lines up with the fields, which sit s(16) into a page.
            let switch_width = s(HEADER_SWITCH_WIDTH);
            let title_x = pages_x + s(16);
            let header = nav.header.as_ref();
            header_title = builder.label(
                hwnd,
                header.map_or("", |header| header.title.as_str()),
                (
                    title_x,
                    margin,
                    page_width - s(32) - switch_width,
                    header_height,
                ),
            );
            if let Some(title) = header_title {
                SendMessageW(
                    title,
                    WM_SETFONT,
                    Some(WPARAM(builder.heading_font.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
            header_switch = builder.control(
                hwnd,
                WC_BUTTONW,
                "",
                BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0,
                WINDOW_EX_STYLE(0),
                (
                    pages_x + page_width - s(16) - switch_width,
                    margin,
                    switch_width,
                    header_height,
                ),
                ID_HEADER_SWITCH,
            );
            apply_header(header_title, header_switch, header);

            for page in &pages {
                let _ = SetWindowPos(
                    *page,
                    Some(HWND_TOP),
                    pages_x,
                    margin + header_height,
                    page_width,
                    page_height,
                    SWP_NOACTIVATE,
                );
            }
            show_page(&pages, nav.page_of(nav.selected).unwrap_or(0));
            let scope_rows = nav.scopes.clone();
            with_state(hwnd, |state| {
                state.nav_rows = rows;
                state.scope_rows = scope_rows;
            });
        } else {
            // Size the tab so its display area is exactly the page area; the
            // header height depends on the font, so ask the control.
            let mut tab_rect = RECT {
                left: margin,
                top: margin,
                right: margin + page_width,
                bottom: margin + page_height,
            };
            if let Some(tab) = tab {
                SendMessageW(
                    tab,
                    TCM_ADJUSTRECT,
                    Some(WPARAM(1)),
                    Some(LPARAM(&mut tab_rect as *mut _ as isize)),
                );
            }
            let dx = margin - tab_rect.left;
            let dy = margin - tab_rect.top;
            if let Some(tab) = tab {
                let _ = SetWindowPos(
                    tab,
                    None,
                    tab_rect.left + dx,
                    tab_rect.top + dy,
                    tab_rect.right - tab_rect.left,
                    tab_rect.bottom - tab_rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            for page in &pages {
                // Pages sit over the tab control's display area, so they must
                // be above it in z-order (children are created at the bottom).
                let _ = SetWindowPos(
                    *page,
                    Some(HWND_TOP),
                    margin + dx,
                    margin + dy,
                    page_width,
                    page_height,
                    SWP_NOACTIVATE,
                );
            }
            client_width = (tab_rect.right - tab_rect.left) + 2 * margin;
            command_bar_top = tab_rect.bottom + dy + margin;
            list = None;
        }
        let button_height = s(30);
        let command_bar_height = button_height + 2 * s(14);
        let client_height = command_bar_top + command_bar_height;

        let button_width = s(96);
        let gap = s(8);
        let button_y = command_bar_top + s(14);
        let mut x = client_width - margin - button_width * 3 - gap * 2;
        let mut button = |caption: &str, id: usize, default: bool| {
            let style = if default {
                BS_DEFPUSHBUTTON
            } else {
                BS_PUSHBUTTON
            } as u32
                | WS_TABSTOP.0;
            let hwnd = builder.control(
                hwnd,
                WC_BUTTONW,
                caption,
                style,
                WINDOW_EX_STYLE(0),
                (x, button_y, button_width, button_height),
                id,
            );
            x += button_width + gap;
            hwnd
        };
        button("OK", ID_OK, true);
        button("Cancel", ID_CANCEL, false);
        let apply = button("Apply", ID_APPLY, false);
        if let Some(apply) = apply {
            let _ = EnableWindow(apply, form.apply_enabled);
        }

        let mut frame = RECT {
            left: 0,
            top: 0,
            right: client_width,
            bottom: client_height,
        };
        let _ =
            AdjustWindowRectExForDpi(&mut frame, window_style(), false, WS_EX_CONTROLPARENT, dpi);
        let width = frame.right - frame.left;
        let height = frame.bottom - frame.top;
        let mut cursor = Default::default();
        let _ = GetCursorPos(&mut cursor);
        let work = work_area_at(deskspace_core::geometry::Point::new(cursor.x, cursor.y));
        let _ = SetWindowPos(
            hwnd,
            None,
            work.x + (work.width - width) / 2,
            work.y + (work.height - height) / 2,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );

        with_state(hwnd, |state| {
            state.fields = fields;
            state.pages = pages;
            state.tab = tab;
            state.list = list;
            state.scopes_list = scopes_list;
            state.header_title = header_title;
            state.header_switch = header_switch;
            state.dividers = dividers;
            state.apply = apply;
            state.font = font;
            state.heading_font = heading_font;
            state.dpi = dpi;
            state.command_bar_top = command_bar_top;
            state.updating = false;
        });
    }
}

/// Creates an owner-drawn list box for a rail pane (scopes or categories).
unsafe fn rail_list(
    builder: &Builder,
    parent: HWND,
    bounds: (i32, i32, i32, i32),
    id: usize,
) -> Option<HWND> {
    // SAFETY: creating and configuring a child list box of our own window.
    unsafe {
        let list = builder.control(
            parent,
            WC_LISTBOXW,
            "",
            (LBS_OWNERDRAWFIXED | LBS_NOTIFY | LBS_HASSTRINGS) as u32 | WS_VSCROLL.0 | WS_TABSTOP.0,
            WINDOW_EX_STYLE(0),
            bounds,
            id,
        )?;
        SendMessageW(
            list,
            LB_SETITEMHEIGHT,
            Some(WPARAM(0)),
            Some(LPARAM(builder.s(NAV_ROW_HEIGHT) as isize)),
        );
        Some(list)
    }
}

/// Replaces a rail list's rows with `labels`.
unsafe fn fill_list<'a>(list: HWND, labels: impl Iterator<Item = &'a str>) {
    // SAFETY: `list` is a live list box; each string outlives its message.
    unsafe {
        SendMessageW(list, LB_RESETCONTENT, None, None);
        for label in labels {
            let text = wide(label);
            SendMessageW(
                list,
                LB_ADDSTRING,
                None,
                Some(LPARAM(text.as_ptr() as isize)),
            );
        }
    }
}

/// Selects `row` in a rail list. Edits arrive on every slider tick; touching
/// the list only when it changed keeps it from repainting each time.
unsafe fn sync_selection(list: HWND, rebuilt: bool, row: usize) {
    // SAFETY: `list` is a live list box of the settings window.
    unsafe {
        let current = SendMessageW(list, LB_GETCURSEL, None, None).0;
        if rebuilt || current != row as isize {
            SendMessageW(list, LB_SETCURSEL, Some(WPARAM(row)), None);
            let _ = InvalidateRect(Some(list), None, false);
        }
    }
}

/// Shows `header` in the header controls: the title, and the switch only
/// when the header has one.
unsafe fn apply_header(title: Option<HWND>, switch: Option<HWND>, header: Option<&SettingsHeader>) {
    // SAFETY: the handles are live controls of the settings window. Neither
    // SetWindowText on a static/checkbox nor BM_SETCHECK reports an edit.
    unsafe {
        if let Some(title) = title {
            let text = header.map_or("", |header| header.title.as_str());
            if window_text(title) != text {
                let text = wide(text);
                let _ = SetWindowTextW(title, PCWSTR(text.as_ptr()));
            }
        }
        if let Some(control) = switch {
            match header.and_then(|header| header.switch.as_ref()) {
                Some(state) => {
                    if window_text(control) != state.label {
                        let text = wide(&state.label);
                        let _ = SetWindowTextW(control, PCWSTR(text.as_ptr()));
                    }
                    SendMessageW(control, BM_SETCHECK, Some(WPARAM(state.on as usize)), None);
                    let _ = ShowWindow(control, SW_SHOW);
                }
                None => {
                    let _ = ShowWindow(control, SW_HIDE);
                }
            }
        }
    }
}

/// Enables or disables a field's input control and repaints its labels,
/// which take their color from the field's enabled state.
unsafe fn apply_enabled(controls: &FieldControls, enabled: bool) {
    // SAFETY: the handles are live controls of the settings window.
    unsafe {
        if let Some(main) = controls.main {
            let _ = EnableWindow(main, enabled);
        }
        if let Some(edit) = controls.value_edit {
            let _ = EnableWindow(edit, enabled);
        }
        for label in controls.labels() {
            let _ = InvalidateRect(Some(label), None, true);
        }
    }
}

/// An edit the window must make to a control, collected under the state
/// borrow and applied after it, because control messages can re-enter
/// `wndproc` (for example SetWindowText raises EN_CHANGE).
enum Update {
    /// The field's main control, an optional value edit, and its labels (see
    /// `apply_enabled`).
    Enable(Option<HWND>, Option<HWND>, Vec<HWND>, bool),
    Text(HWND, String),
    Choice(HWND, usize),
    Check(HWND, bool),
    Slider(HWND, i32),
    Repaint(HWND),
}

/// Refreshes control values, labels, and enabled state from `form`. Returns
/// false when `form` has a different shape and the window must be rebuilt.
pub(crate) fn update(hwnd: HWND, form: &SettingsForm) -> bool {
    // SAFETY: the borrow below only reads/writes state and creates brushes;
    // control messages are sent after it ends.
    let plan = unsafe {
        with_state(hwnd, |state| {
            if !state.form.same_shape(form) {
                return None;
            }
            let mut updates = Vec::new();
            for ((old, controls), new) in state
                .fields
                .iter_mut()
                .zip(form.pages.iter().flat_map(|page| page.fields.iter()))
            {
                plan_field(old, controls, new, &mut updates);
                *old = new.clone();
            }
            let new_rows = form.nav.as_ref().map(nav_rows).unwrap_or_default();
            let rebuild = state.nav_rows != new_rows;
            let selected_row = form
                .nav
                .as_ref()
                .map(|nav| entry_row(&new_rows, nav.selected))
                .unwrap_or(0);
            let page = form.nav.as_ref().and_then(|nav| nav.page_of(nav.selected));
            let old_page = state
                .form
                .nav
                .as_ref()
                .and_then(|nav| nav.page_of(nav.selected));
            // Re-showing the same page on every edit would hide and show the
            // pages for nothing.
            let page = page.filter(|page| rebuild || Some(*page) != old_page);
            let new_scopes = form
                .nav
                .as_ref()
                .map(|nav| nav.scopes.clone())
                .unwrap_or_default();
            let scopes = state.scopes_list.map(|list| {
                let rebuild = state.scope_rows != new_scopes;
                let selected = form.nav.as_ref().map_or(0, |nav| nav.selected_scope);
                (list, new_scopes.clone(), rebuild, selected)
            });
            let new_header = form.nav.as_ref().and_then(|nav| nav.header.clone());
            let old_header = state.form.nav.as_ref().and_then(|nav| nav.header.as_ref());
            let header = (old_header != new_header.as_ref()).then_some((
                state.header_title,
                state.header_switch,
                new_header,
            ));
            state.nav_rows = new_rows.clone();
            state.scope_rows = new_scopes;
            state.form = form.clone();
            state.updating = true;
            let rail = state
                .list
                .map(|list| (list, new_rows, rebuild, selected_row));
            Some((
                updates,
                state.apply,
                rail,
                page,
                state.pages.clone(),
                scopes,
                header,
            ))
        })
        .flatten()
    };
    let Some((updates, apply, rail, page, pages, scopes, header)) = plan else {
        return false;
    };

    // SAFETY: every handle is a live control of this window.
    unsafe {
        for update in updates {
            match update {
                Update::Enable(main, edit, labels, enabled) => {
                    if let Some(main) = main {
                        let _ = EnableWindow(main, enabled);
                    }
                    if let Some(edit) = edit {
                        let _ = EnableWindow(edit, enabled);
                    }
                    for label in labels {
                        let _ = InvalidateRect(Some(label), None, true);
                    }
                }
                Update::Text(control, text) => {
                    if window_text(control) != text {
                        let text = wide(&text);
                        let _ = SetWindowTextW(control, PCWSTR(text.as_ptr()));
                    }
                }
                Update::Choice(control, index) => {
                    SendMessageW(control, CB_SETCURSEL, Some(WPARAM(index)), None);
                }
                Update::Check(control, on) => {
                    SendMessageW(control, BM_SETCHECK, Some(WPARAM(on as usize)), None);
                }
                Update::Slider(control, value) => {
                    SendMessageW(
                        control,
                        TBM_SETPOS,
                        Some(WPARAM(1)),
                        Some(LPARAM(value as isize)),
                    );
                }
                Update::Repaint(control) => {
                    let _ = InvalidateRect(Some(control), None, true);
                }
            }
        }
        if let Some(apply) = apply {
            let _ = EnableWindow(apply, form.apply_enabled);
        }
        if let Some((list, rows, rebuild, selected_row)) = rail {
            if rebuild {
                fill_list(list, rows.iter().map(NavRow::label));
            }
            sync_selection(list, rebuild, selected_row);
            if let Some(page) = page {
                show_page(&pages, page);
            }
        }
        if let Some((list, scopes, rebuild, selected)) = scopes {
            if rebuild {
                fill_list(list, scopes.iter().map(|scope| scope.label.as_str()));
            }
            sync_selection(list, rebuild, selected);
        }
        if let Some((title, switch, header)) = header {
            apply_header(title, switch, header.as_ref());
        }
        with_state(hwnd, |state| state.updating = false);
    }
    true
}

fn plan_field(old: &Field, controls: &mut FieldControls, new: &Field, updates: &mut Vec<Update>) {
    if old.enabled != new.enabled {
        updates.push(Update::Enable(
            controls.main,
            controls.value_edit,
            controls.labels(),
            new.enabled,
        ));
    }
    if old.label != new.label {
        let target = match new.kind {
            FieldKind::Toggle { .. } => controls.main,
            _ => controls.label,
        };
        if let Some(target) = target {
            updates.push(Update::Text(target, new.label.clone()));
        }
    }
    let Some(main) = controls.main else {
        return;
    };
    match (&old.kind, &new.kind) {
        (FieldKind::Choice { selected: a, .. }, FieldKind::Choice { selected: b, .. })
            if a != b =>
        {
            updates.push(Update::Choice(main, *b));
        }
        (FieldKind::Toggle { on: a }, FieldKind::Toggle { on: b }) if a != b => {
            updates.push(Update::Check(main, *b));
        }
        (
            FieldKind::Slider { value: a, .. },
            FieldKind::Slider {
                value: b, suffix, ..
            },
        ) if a != b => {
            updates.push(Update::Slider(main, *b));
            if let Some(edit) = controls.value_edit {
                updates.push(Update::Text(edit, b.to_string()));
            } else if let Some(text) = controls.value_text {
                updates.push(Update::Text(text, format!("{b}{suffix}")));
            }
        }
        (FieldKind::Color { color: a }, FieldKind::Color { color: b }) if a != b => {
            set_swatch(controls, *b, updates);
        }
        (FieldKind::Text { value: a, .. }, FieldKind::Text { value: b, .. }) if a != b => {
            updates.push(Update::Text(main, b.clone()));
        }
        (FieldKind::Button { caption: a }, FieldKind::Button { caption: b }) if a != b => {
            updates.push(Update::Text(main, b.clone()));
        }
        _ => {}
    }
}

fn set_swatch(controls: &mut FieldControls, color: Color, updates: &mut Vec<Update>) {
    // SAFETY: brush creation/deletion does not send messages; the old brush is
    // no longer selected anywhere once the swatch repaints with the new one.
    unsafe {
        if let Some(old) = controls
            .swatch_brush
            .replace(CreateSolidBrush(colorref(color)))
        {
            let _ = DeleteObject(HGDIOBJ(old.0));
        }
    }
    if let Some(swatch) = controls.swatch {
        updates.push(Update::Repaint(swatch));
    }
    if let Some(text) = controls.value_text {
        updates.push(Update::Text(text, color.to_rgb_hex()));
    }
}

fn colorref(color: Color) -> COLORREF {
    COLORREF(color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16))
}

fn from_colorref(value: COLORREF) -> Color {
    Color::rgb(
        (value.0 & 0xFF) as u8,
        ((value.0 >> 8) & 0xFF) as u8,
        ((value.0 >> 16) & 0xFF) as u8,
    )
}

unsafe fn window_text(control: HWND) -> String {
    // SAFETY: the buffer is sized from GetWindowTextLengthW plus the NUL.
    unsafe {
        let length = GetWindowTextLengthW(control).max(0) as usize;
        let mut buffer = vec![0u16; length + 1];
        let copied = GetWindowTextW(control, &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..copied])
    }
}

unsafe fn state_of(window: HWND) -> *mut SettingsState {
    // SAFETY: reading the user-data slot of a window of our class.
    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut SettingsState }
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut SettingsState) -> R) -> Option<R> {
    // SAFETY: a non-null pointer is the live state (WM_NCDESTROY clears the
    // slot before freeing), and `f` cannot re-enter to create a second borrow.
    unsafe { state_of(window).as_mut() }.map(f)
}

fn settings_event(event: SettingsEvent) {
    runtime::dispatch(Event::Settings(event));
}

/// Handles WM_COMMAND from a field control. Any dispatch is the last thing
/// done, because the handler may destroy this window.
unsafe fn on_field_command(window: HWND, id: usize, code: u32, control: HWND) {
    let index = (id - CONTROL_BASE) / 4;
    let part = (id - CONTROL_BASE) % 4;
    // SAFETY (all calls): `window` is the live settings window and `control`
    // its child; state borrows end before any control message is sent.
    unsafe {
        let Some(Some((field, color))) = with_state(window, |state| {
            if state.updating {
                return None;
            }
            let (field, _) = state.fields.get(index)?;
            let color = match field.kind {
                FieldKind::Color { color } => color,
                _ => Color::TRANSPARENT,
            };
            Some((field.clone(), color))
        }) else {
            return;
        };
        // A slider's editable value box commits when it loses focus, so typing
        // is not clamped or rewritten under the caret.
        if part == PART_SECONDARY {
            if let FieldKind::Slider {
                min,
                max,
                value: stored,
                editable: true,
                ..
            } = &field.kind
            {
                if code == EN_KILLFOCUS {
                    let stored = *stored;
                    let typed = window_text(control).trim().parse::<i32>().unwrap_or(stored);
                    let value = typed.clamp(*min, *max);
                    if value != stored {
                        with_state(window, |state| {
                            if let Some((field, _)) = state.fields.get_mut(index) {
                                if let FieldKind::Slider { value: slot, .. } = &mut field.kind {
                                    *slot = value;
                                }
                            }
                        });
                        settings_event(SettingsEvent::Changed {
                            field: field.id,
                            value: FieldValue::Slider(value),
                        });
                    } else {
                        // Reject invalid or out-of-range text.
                        with_state(window, |state| state.updating = true);
                        let text = wide(&value.to_string());
                        let _ = SetWindowTextW(control, PCWSTR(text.as_ptr()));
                        with_state(window, |state| state.updating = false);
                    }
                }
            }
            return;
        }
        if part != PART_MAIN {
            return;
        }
        let value = match (&field.kind, code) {
            (FieldKind::Choice { .. }, CBN_SELCHANGE) => {
                let selected = SendMessageW(control, CB_GETCURSEL, None, None).0;
                (selected >= 0).then_some(FieldValue::Choice(selected as usize))
            }
            (FieldKind::Toggle { .. }, BN_CLICKED) => {
                let checked = SendMessageW(control, BM_GETCHECK, None, None).0 == 1;
                Some(FieldValue::Toggle(checked))
            }
            (
                FieldKind::Text {
                    read_only: false, ..
                },
                EN_CHANGE,
            ) => Some(FieldValue::Text(window_text(control))),
            (FieldKind::Color { .. }, BN_CLICKED) => {
                pick_color(window, color).map(FieldValue::Color)
            }
            (FieldKind::Button { .. }, BN_CLICKED) => {
                settings_event(SettingsEvent::Button { field: field.id });
                return;
            }
            _ => None,
        };
        let Some(value) = value else {
            return;
        };
        if let FieldValue::Color(color) = value {
            let updates = with_state(window, |state| {
                let mut updates = Vec::new();
                if let Some((field, controls)) = state.fields.get_mut(index) {
                    field.kind = FieldKind::Color { color };
                    set_swatch(controls, color, &mut updates);
                }
                updates
            })
            .unwrap_or_default();
            for update in updates {
                match update {
                    Update::Repaint(hwnd) => {
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                    Update::Text(hwnd, text) => {
                        let text = wide(&text);
                        let _ = SetWindowTextW(hwnd, PCWSTR(text.as_ptr()));
                    }
                    _ => {}
                }
            }
        }
        settings_event(SettingsEvent::Changed {
            field: field.id,
            value,
        });
    }
}

/// Opens the system color picker. Modal: messages for other windows keep
/// flowing while it is open.
pub(crate) unsafe fn pick_color(owner: HWND, initial: Color) -> Option<Color> {
    let mut custom = CUSTOM_COLORS.with(|colors| *colors.borrow());
    let mut choose = CHOOSECOLORW {
        lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: owner,
        rgbResult: colorref(initial),
        lpCustColors: custom.as_mut_ptr(),
        Flags: CC_RGBINIT | CC_FULLOPEN,
        ..Default::default()
    };
    // SAFETY: `choose` and `custom` outlive the modal call.
    let picked = unsafe { ChooseColorW(&mut choose) }.as_bool();
    CUSTOM_COLORS.with(|colors| *colors.borrow_mut() = custom);
    picked.then(|| from_colorref(choose.rgbResult))
}

unsafe fn on_scroll(window: HWND, control: HWND) {
    // SAFETY (all calls): `control` is a trackbar child of this window.
    unsafe {
        let Some(Some((index, field_id, suffix, last, value_text, value_edit))) =
            with_state(window, |state| {
                if state.updating {
                    return None;
                }
                let index = state.field_index_for(control)?;
                let (field, controls) = &state.fields[index];
                match &field.kind {
                    FieldKind::Slider { value, suffix, .. } => Some((
                        index,
                        field.id,
                        suffix.clone(),
                        *value,
                        controls.value_text,
                        controls.value_edit,
                    )),
                    _ => None,
                }
            })
        else {
            return;
        };
        let value = SendMessageW(control, TBM_GETPOS, None, None).0 as i32;
        if value == last {
            return;
        }
        with_state(window, |state| {
            if let Some((field, _)) = state.fields.get_mut(index) {
                if let FieldKind::Slider { value: stored, .. } = &mut field.kind {
                    *stored = value;
                }
            }
        });
        if let Some(edit) = value_edit {
            // Setting the text raises EN_CHANGE; ignore it.
            with_state(window, |state| state.updating = true);
            let text_value = wide(&value.to_string());
            let _ = SetWindowTextW(edit, PCWSTR(text_value.as_ptr()));
            with_state(window, |state| state.updating = false);
        } else if let Some(text) = value_text {
            let text_value = wide(&format!("{value}{suffix}"));
            let _ = SetWindowTextW(text, PCWSTR(text_value.as_ptr()));
        }
        settings_event(SettingsEvent::Changed {
            field: field_id,
            value: FieldValue::Slider(value),
        });
    }
}

/// Shows the page at `index` and hides the rest.
unsafe fn show_page(pages: &[HWND], index: usize) {
    // SAFETY: the handles are live page windows of the settings window.
    unsafe {
        for (i, page) in pages.iter().enumerate() {
            let _ = ShowWindow(*page, if i == index { SW_SHOW } else { SW_HIDE });
        }
    }
}

unsafe fn on_tab_changed(window: HWND) {
    // SAFETY: tab and pages are live children of this window.
    unsafe {
        let Some((Some(tab), pages)) = with_state(window, |state| (state.tab, state.pages.clone()))
        else {
            return;
        };
        let selected = SendMessageW(tab, TCM_GETCURSEL, None, None).0;
        if selected >= 0 {
            show_page(&pages, selected as usize);
        }
    }
}

/// Handles a rail selection change. Group titles are not selectable: landing
/// on one moves the selection to the nearest entry. Shows that entry's page,
/// then tells the app last, because the handler may rebuild the form.
unsafe fn on_nav_changed(window: HWND) {
    // SAFETY: the list and pages are live children of `window`; the state
    // borrow ends before the list messages and the dispatch.
    unsafe {
        let Some((Some(list), rows, pages, nav)) = with_state(window, |state| {
            (
                state.list,
                state.nav_rows.clone(),
                state.pages.clone(),
                state.form.nav.clone(),
            )
        }) else {
            return;
        };
        let current = SendMessageW(list, LB_GETCURSEL, None, None).0;
        if current < 0 {
            return;
        }
        let mut selected = current as usize;
        if row_entry(&rows, selected).is_none() {
            let Some(target) = (selected + 1..rows.len())
                .chain((0..selected).rev())
                .find(|row| rows[*row].is_entry())
            else {
                return;
            };
            SendMessageW(list, LB_SETCURSEL, Some(WPARAM(target)), None);
            let _ = InvalidateRect(Some(list), None, true);
            selected = target;
        }
        let entry = row_entry(&rows, selected).expect("moved to an entry row");
        if let Some(page) = nav.and_then(|nav| nav.page_of(entry)) {
            show_page(&pages, page);
        }
        settings_event(SettingsEvent::NavChanged { entry });
    }
}

/// Handles a scopes-pane selection change. The app answers with a form for
/// that scope, so this only reports it, as the last thing done.
unsafe fn on_scope_changed(window: HWND) {
    // SAFETY: the list is a live child of `window`; the borrow ends first.
    unsafe {
        let Some(Some(list)) = with_state(window, |state| state.scopes_list) else {
            return;
        };
        let current = SendMessageW(list, LB_GETCURSEL, None, None).0;
        if current >= 0 {
            settings_event(SettingsEvent::ScopeChanged {
                scope: current as usize,
            });
        }
    }
}

/// Reports the header switch's new state, unless the window set it itself.
unsafe fn on_header_switch(window: HWND, control: HWND) {
    // SAFETY: `control` is the live header checkbox of `window`.
    unsafe {
        if with_state(window, |state| state.updating) != Some(false) {
            return;
        }
        let on = SendMessageW(control, BM_GETCHECK, None, None).0 == 1;
        settings_event(SettingsEvent::HeaderSwitch { on });
    }
}

/// Paints one row of the scopes pane like a rail entry: an accent bar when
/// selected, indented rows moved right, and dimmed rows (scopes that only
/// inherit) in disabled text unless selected.
unsafe fn on_draw_scope(window: HWND, item: &DRAWITEMSTRUCT) {
    // SAFETY: `item.hDC` is a live DC owned by the list box during
    // WM_DRAWITEM; the borrowed data is cloned out before any GDI call.
    unsafe {
        let Some((colors, body, accent, font, dpi, row)) = with_state(window, |state| {
            (
                state.colors,
                state.body_brush,
                state.accent_brush,
                state.font,
                state.dpi,
                state.scope_rows.get(item.itemID as usize).cloned(),
            )
        }) else {
            return;
        };
        FillRect(item.hDC, &item.rcItem, body);
        let Some(row) = row else {
            return;
        };
        let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
        let text = if selected {
            colors.text
        } else if row.dimmed {
            colors.disabled_text
        } else {
            colors.secondary_text
        };
        if selected {
            let bar = RECT {
                left: item.rcItem.left,
                right: item.rcItem.left + scaled(3, dpi),
                ..item.rcItem
            };
            FillRect(item.hDC, &bar, accent);
        }
        SetBkMode(item.hDC, TRANSPARENT);
        SetTextColor(item.hDC, colorref(text));
        let previous = SelectObject(item.hDC, HGDIOBJ(font.0));
        let mut label: Vec<u16> = row.label.encode_utf16().collect();
        let mut rect = item.rcItem;
        rect.left += scaled(12, dpi)
            + if row.indent {
                scaled(SCOPE_INDENT, dpi)
            } else {
                0
            };
        rect.right -= scaled(12, dpi);
        DrawTextW(
            item.hDC,
            &mut label,
            &mut rect,
            DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(item.hDC, previous);
    }
}

/// Paints one row of the navigation rail: group titles in secondary text,
/// entries in normal text, an accent bar on the selected entry, and an accent
/// dot on a marked one.
unsafe fn on_draw_item(window: HWND, item: &DRAWITEMSTRUCT) {
    // SAFETY: `item.hDC` is a live DC owned by the list box during
    // WM_DRAWITEM; the borrowed data is cloned out before any GDI call.
    unsafe {
        let Some((colors, body, accent, font, dpi, row)) = with_state(window, |state| {
            Some((
                state.colors,
                state.body_brush,
                state.accent_brush,
                state.font,
                state.dpi,
                state.nav_rows.get(item.itemID as usize).cloned(),
            ))
        })
        .flatten() else {
            return;
        };
        FillRect(item.hDC, &item.rcItem, body);
        let Some(row) = row else {
            return;
        };
        let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
        let text = match &row {
            NavRow::Title(_) => colors.secondary_text,
            NavRow::Entry { .. } if selected => colors.text,
            NavRow::Entry { .. } => colors.secondary_text,
        };
        let inset = scaled(12, dpi);
        if selected && row.is_entry() {
            let bar = RECT {
                left: item.rcItem.left,
                right: item.rcItem.left + scaled(3, dpi),
                ..item.rcItem
            };
            FillRect(item.hDC, &bar, accent);
        }
        SetBkMode(item.hDC, TRANSPARENT);
        SetTextColor(item.hDC, colorref(text));
        let previous = SelectObject(item.hDC, HGDIOBJ(font.0));
        let mut label: Vec<u16> = row.label().encode_utf16().collect();
        let mut rect = item.rcItem;
        rect.left += inset;
        rect.right -= inset;
        DrawTextW(
            item.hDC,
            &mut label,
            &mut rect,
            DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        if matches!(row, NavRow::Entry { marked: true, .. }) {
            let radius = scaled(3, dpi);
            let (cx, cy) = (rect.right, (rect.top + rect.bottom) / 2);
            let previous_brush = SelectObject(item.hDC, HGDIOBJ(accent.0));
            let _ = Ellipse(item.hDC, cx - radius, cy - radius, cx + radius, cy + radius);
            SelectObject(item.hDC, previous_brush);
        }
        SelectObject(item.hDC, previous);
    }
}

unsafe fn on_paint(window: HWND) {
    // SAFETY: standard BeginPaint/EndPaint on our own window.
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut ps);
        if let Some((top, brush, dividers, divider)) = with_state(window, |state| {
            (
                state.command_bar_top,
                state.command_bar_brush,
                state.dividers.clone(),
                state.colors.divider,
            )
        }) {
            let mut client = RECT::default();
            let _ = GetClientRect(window, &mut client);
            // Windows 11 dialogs put their buttons on a slightly darker
            // command bar.
            let bar = RECT { top, ..client };
            FillRect(dc, &bar, brush);
            // Hairlines separate the rail panes from each other and from the
            // page.
            if !dividers.is_empty() {
                let line_brush = CreateSolidBrush(colorref(divider));
                for (x, top, bottom) in dividers {
                    let line = RECT {
                        left: x,
                        top,
                        right: x + 1,
                        bottom,
                    };
                    FillRect(dc, &line, line_brush);
                }
                let _ = DeleteObject(HGDIOBJ(line_brush.0));
            }
        }
        let _ = EndPaint(window, &ps);
    }
}

/// Fills `window`'s client area with the settings body color. `settings`
/// is the settings window (the window itself or a page's parent).
unsafe fn erase_body(settings: HWND, window: HWND, dc: HDC) -> bool {
    // SAFETY: `dc` is the erase DC for `window`, a live window of ours.
    unsafe {
        let Some(brush) = with_state(settings, |state| state.body_brush) else {
            return false;
        };
        let mut client = RECT::default();
        let _ = GetClientRect(window, &mut client);
        FillRect(dc, &client, brush);
        true
    }
}

/// Paints the tab strip like Windows 11 navigation: tab titles on the body
/// color, the selected one in full-strength text with an accent underline,
/// and a divider below. The common tab control has no dark visual style, so
/// it is painted here in both modes for a consistent look.
unsafe fn paint_tab(tab: HWND) {
    // SAFETY: BeginPaint/EndPaint on the live tab control; the GDI objects
    // created here are deleted before returning, and the font is restored.
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(tab, &mut ps);
        let settings = GetParent(tab).unwrap_or_default();
        let Some((colors, body, font, titles)) = with_state(settings, |state| {
            let titles: Vec<Vec<u16>> = state
                .form
                .pages
                .iter()
                .map(|page| page.title.encode_utf16().collect())
                .collect();
            (state.colors, state.body_brush, state.font, titles)
        }) else {
            let _ = EndPaint(tab, &ps);
            return;
        };
        let mut client = RECT::default();
        let _ = GetClientRect(tab, &mut client);
        FillRect(dc, &client, body);

        let selected = SendMessageW(tab, TCM_GETCURSEL, None, None).0;
        let focused = GetFocus() == tab;
        let previous_font = SelectObject(dc, HGDIOBJ(font.0));
        SetBkMode(dc, TRANSPARENT);
        let accent = CreateSolidBrush(colorref(colors.accent));
        let mut header_bottom = 0;
        for (index, title) in titles.iter().enumerate() {
            let mut item = RECT::default();
            SendMessageW(
                tab,
                TCM_GETITEMRECT,
                Some(WPARAM(index)),
                Some(LPARAM(&mut item as *mut RECT as isize)),
            );
            header_bottom = header_bottom.max(item.bottom);
            let is_selected = index as isize == selected;
            let color = if is_selected {
                colors.text
            } else {
                colors.secondary_text
            };
            SetTextColor(dc, colorref(color));
            let mut text = title.clone();
            DrawTextW(
                dc,
                &mut text,
                &mut item.clone(),
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
            if is_selected {
                let inset = (item.right - item.left) / 4;
                let underline = RECT {
                    left: item.left + inset,
                    right: item.right - inset,
                    top: item.bottom - 3,
                    bottom: item.bottom,
                };
                FillRect(dc, &underline, accent);
                if focused {
                    let _ = DrawFocusRect(dc, &item);
                }
            }
        }
        let _ = DeleteObject(HGDIOBJ(accent.0));
        let divider = CreateSolidBrush(colorref(colors.divider));
        let line = RECT {
            top: header_bottom,
            bottom: header_bottom + 1,
            ..client
        };
        FillRect(dc, &line, divider);
        let _ = DeleteObject(HGDIOBJ(divider.0));
        SelectObject(dc, previous_font);
        let _ = EndPaint(tab, &ps);
    }
}

/// Subclass procedure of the tab control: takes over painting.
/// https://learn.microsoft.com/en-us/windows/win32/controls/subclassing-overview
unsafe extern "system" fn tab_proc(
    tab: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: called by comctl32 for the subclassed tab control.
    unsafe {
        match message {
            WM_PAINT => {
                paint_tab(tab);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_SETFOCUS | WM_KILLFOCUS => {
                let _ = InvalidateRect(Some(tab), None, false);
                DefSubclassProc(tab, message, wparam, lparam)
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(tab, Some(tab_proc), TAB_SUBCLASS_ID);
                DefSubclassProc(tab, message, wparam, lparam)
            }
            _ => DefSubclassProc(tab, message, wparam, lparam),
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
            WM_NOTIFY => {
                let header = lparam.0 as *const NMHDR;
                if !header.is_null()
                    && (*header).idFrom == ID_TAB
                    && (*header).code == TCN_SELCHANGE
                {
                    on_tab_changed(window);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{SettingsNavEntry, SettingsNavGroup};

    #[test]
    fn colorref_round_trips() {
        let color = Color::rgb(0x12, 0x34, 0x56);
        assert_eq!(colorref(color).0, 0x0056_3412);
        assert_eq!(from_colorref(colorref(color)), color);
    }

    #[test]
    fn layout_scales_with_dpi() {
        assert_eq!(scaled(100, 96), 100);
        assert_eq!(scaled(100, 144), 150);
        // The rail width and row height scale like every other layout unit.
        assert_eq!(scaled(NAV_WIDTH, 96), NAV_WIDTH);
        assert_eq!(scaled(NAV_WIDTH, 144), NAV_WIDTH * 3 / 2);
        assert_eq!(scaled(NAV_ROW_HEIGHT, 192), NAV_ROW_HEIGHT * 2);
    }

    fn nav() -> SettingsNav {
        SettingsNav {
            groups: vec![
                SettingsNavGroup {
                    title: "Settings".into(),
                    entries: vec![
                        SettingsNavEntry {
                            label: "Appearance".into(),
                            page: 0,
                            marked: false,
                        },
                        SettingsNavEntry {
                            label: "General".into(),
                            page: 5,
                            marked: false,
                        },
                    ],
                },
                SettingsNavGroup {
                    title: "Spaces".into(),
                    entries: vec![SettingsNavEntry {
                        label: "Work".into(),
                        page: 7,
                        marked: true,
                    }],
                },
            ],
            selected: 1,
            ..Default::default()
        }
    }

    #[test]
    fn nav_rows_put_a_title_before_each_group() {
        let rows = nav_rows(&nav());
        let labels: Vec<&str> = rows.iter().map(NavRow::label).collect();
        assert_eq!(
            labels,
            ["Settings", "Appearance", "General", "Spaces", "Work"]
        );
        assert!(!rows[0].is_entry());
        assert!(rows[1].is_entry());
    }

    #[test]
    fn untitled_groups_have_no_title_row() {
        let mut nav = nav();
        nav.groups[0].title.clear();
        let rows = nav_rows(&nav);
        let labels: Vec<&str> = rows.iter().map(NavRow::label).collect();
        assert_eq!(labels, ["Appearance", "General", "Spaces", "Work"]);
        assert_eq!(row_entry(&rows, 0), Some(0));
        assert_eq!(entry_row(&rows, 2), 3);
    }

    #[test]
    fn nav_row_entries_map_to_flattened_indices() {
        let rows = nav_rows(&nav());
        assert_eq!(row_entry(&rows, 0), None);
        assert_eq!(row_entry(&rows, 1), Some(0));
        assert_eq!(row_entry(&rows, 2), Some(1));
        assert_eq!(row_entry(&rows, 3), None);
        assert_eq!(row_entry(&rows, 4), Some(2));
        // The row showing a flattened entry, and the fallback past the end.
        assert_eq!(entry_row(&rows, 2), 4);
        assert_eq!(entry_row(&rows, 9), 1);
    }

    #[test]
    fn nav_row_helpers_tolerate_empty_groups() {
        let rows = nav_rows(&SettingsNav {
            groups: vec![SettingsNavGroup {
                title: "Spaces".into(),
                entries: vec![],
            }],
            selected: 0,
            ..Default::default()
        });
        assert_eq!(rows.len(), 1);
        assert_eq!(row_entry(&rows, 0), None);
        assert_eq!(entry_row(&rows, 0), 0);
        assert_eq!(row_entry(&rows, 1), None);
    }
}
