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
    BeginPaint, COLOR_3DFACE, COLOR_WINDOW, CreateFontIndirectW, CreateSolidBrush, DT_CALCRECT,
    DT_NOPREFIX, DT_WORDBREAK, DeleteObject, DrawTextW, EndPaint, FillRect, GetDC, GetSysColor,
    GetSysColorBrush, HBRUSH, HDC, HFONT, HGDIOBJ, InvalidateRect, PAINTSTRUCT, ReleaseDC,
    SelectObject, SetBkColor,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::Controls::{
    NMHDR, TBM_SETPOS, TBM_SETRANGEMIN, TBS_HORZ, TBS_NOTICKS, TCIF_TEXT, TCITEMW, TCM_ADJUSTRECT,
    TCM_GETCURSEL, TCM_INSERTITEMW, TCN_SELCHANGE, TRACKBAR_CLASSW, WC_BUTTONW, WC_COMBOBOXW,
    WC_EDITW, WC_STATICW, WC_TABCONTROLW,
};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SystemParametersInfoForDpi,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON,
    CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, CREATESTRUCTW,
    CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, EN_CHANGE, ES_AUTOHSCROLL, ES_READONLY,
    GWLP_USERDATA, GetCursorPos, GetParent, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, HMENU, HWND_TOP, ICON_BIG, ICON_SMALL, IsIconic, NONCLIENTMETRICSW,
    RegisterClassW, SPI_GETNONCLIENTMETRICS, SW_HIDE, SW_RESTORE, SW_SHOW, SWP_NOACTIVATE,
    SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    SetWindowTextW, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_HSCROLL, WM_NCCREATE, WM_NCDESTROY,
    WM_NOTIFY, WM_PAINT, WM_SETFONT, WM_SETICON, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN,
    WS_CLIPSIBLINGS, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_MINIMIZEBOX, WS_OVERLAPPED,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, SettingsEvent};
use crate::ui::{Field, FieldKind, FieldValue, SettingsForm, SliderMark};
use crate::win32::system::{wide, work_area_at};
use crate::win32::{runtime, tray};

const CLASS_NAME: PCWSTR = w!("DeskSpaceSettings");
const PAGE_CLASS: PCWSTR = w!("DeskSpaceSettingsPage");

const ID_OK: usize = 1;
const ID_CANCEL: usize = 2;
const ID_APPLY: usize = 3;
const ID_TAB: usize = 4;
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
    apply: Option<HWND>,
    font: HFONT,
    heading_font: HFONT,
    command_bar_top: i32,
    /// Set while the window itself is changing control values, so the
    /// resulting notifications are not reported as user edits.
    updating: bool,
}

impl FieldControls {
    fn handles(&self) -> Vec<HWND> {
        [self.label, self.main, self.value_text, self.swatch]
            .into_iter()
            .flatten()
            .chain(self.marks.iter().copied())
            .collect()
    }
}

impl SettingsState {
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
        }
    }
}

pub(crate) fn create(form: &SettingsForm) -> Result<HWND, PlatformError> {
    // SAFETY: Win32 window creation on the UI thread. Ownership of the state
    // passes to the window in WM_NCCREATE; it is freed in WM_NCDESTROY.
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        ensure_classes(instance)?;
        let state = Box::into_raw(Box::new(SettingsState {
            form: form.clone(),
            fields: Vec::new(),
            pages: Vec::new(),
            tab: None,
            apply: None,
            font: HFONT::default(),
            heading_font: HFONT::default(),
            command_bar_top: 0,
            updating: true,
        }));
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

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        Ok(hwnd)
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
        // SAFETY: class structs are fully initialised; strings are static.
        unsafe {
            let background = GetSysColorBrush(COLOR_WINDOW);
            let main = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                hbrBackground: background,
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            let page = WNDCLASSW {
                lpfnWndProc: Some(page_wndproc),
                hInstance: instance,
                hbrBackground: background,
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
                    marks,
                } => {
                    controls.label = self.label(page, &field.label, label_bounds);
                    let value_width = self.s(48);
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
                    controls.value_text = self.label(
                        page,
                        &format!("{value}{suffix}"),
                        (
                            control_x + control_width - value_width + self.s(4),
                            y,
                            value_width,
                            row,
                        ),
                    );
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
                    | if page_index == 0 {
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
                apply_enabled(controls.handles(), field.enabled);
                fields.push((field.clone(), controls));
                y += height;
            }
            page_height = page_height.max(y + s(8));
            pages.push(container);
        }

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
            // Pages sit over the tab control's display area, so they must be
            // above it in z-order (children are created at the bottom).
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

        let client_width = (tab_rect.right - tab_rect.left) + 2 * margin;
        let command_bar_top = tab_rect.bottom + dy + margin;
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
            state.apply = apply;
            state.font = font;
            state.heading_font = heading_font;
            state.command_bar_top = command_bar_top;
            state.updating = false;
        });
    }
}

unsafe fn apply_enabled(handles: Vec<HWND>, enabled: bool) {
    for hwnd in handles {
        // SAFETY: `hwnd` is a live control of the settings window.
        unsafe {
            let _ = EnableWindow(hwnd, enabled);
        }
    }
}

/// An edit the window must make to a control, collected under the state
/// borrow and applied after it, because control messages can re-enter
/// `wndproc` (for example SetWindowText raises EN_CHANGE).
enum Update {
    Enable(Vec<HWND>, bool),
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
            state.form = form.clone();
            state.updating = true;
            Some((updates, state.apply))
        })
        .flatten()
    };
    let Some((updates, apply)) = plan else {
        return false;
    };

    // SAFETY: every handle is a live control of this window.
    unsafe {
        for update in updates {
            match update {
                Update::Enable(handles, enabled) => apply_enabled(handles, enabled),
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
        with_state(hwnd, |state| state.updating = false);
    }
    true
}

fn plan_field(old: &Field, controls: &mut FieldControls, new: &Field, updates: &mut Vec<Update>) {
    if old.enabled != new.enabled {
        updates.push(Update::Enable(controls.handles(), new.enabled));
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
            if let Some(text) = controls.value_text {
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
unsafe fn pick_color(owner: HWND, initial: Color) -> Option<Color> {
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
        let Some(Some((index, field_id, suffix, last, value_text))) = with_state(window, |state| {
            if state.updating {
                return None;
            }
            let index = state.field_index_for(control)?;
            let (field, controls) = &state.fields[index];
            match &field.kind {
                FieldKind::Slider { value, suffix, .. } => {
                    Some((index, field.id, suffix.clone(), *value, controls.value_text))
                }
                _ => None,
            }
        }) else {
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
        if let Some(text) = value_text {
            let text_value = wide(&format!("{value}{suffix}"));
            let _ = SetWindowTextW(text, PCWSTR(text_value.as_ptr()));
        }
        settings_event(SettingsEvent::Changed {
            field: field_id,
            value: FieldValue::Slider(value),
        });
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
        for (index, page) in pages.iter().enumerate() {
            let command = if index as isize == selected {
                SW_SHOW
            } else {
                SW_HIDE
            };
            let _ = ShowWindow(*page, command);
        }
    }
}

unsafe fn on_paint(window: HWND) {
    // SAFETY: standard BeginPaint/EndPaint on our own window.
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut ps);
        let top = with_state(window, |state| state.command_bar_top).unwrap_or(0);
        let mut client = RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(window, &mut client);
        // Windows 11 dialogs put their buttons on a slightly darker command bar.
        let bar = RECT { top, ..client };
        FillRect(dc, &bar, GetSysColorBrush(COLOR_3DFACE));
        let _ = EndPaint(window, &ps);
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
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT => {
                let control = HWND(lparam.0 as *mut core::ffi::c_void);
                let swatch = with_state(window, |state| {
                    state
                        .fields
                        .iter()
                        .find(|(_, controls)| controls.swatch == Some(control))
                        .and_then(|(_, controls)| controls.swatch_brush)
                })
                .flatten();
                if let Some(brush) = swatch {
                    return LRESULT(brush.0 as isize);
                }
                if message == WM_CTLCOLOREDIT {
                    return DefWindowProcW(window, message, wparam, lparam);
                }
                // Labels, checkboxes, and trackbars default to the dialog
                // face color; match the white page instead.
                let dc = HDC(wparam.0 as *mut core::ffi::c_void);
                SetBkColor(dc, COLORREF(GetSysColor(COLOR_WINDOW)));
                LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize)
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
            WM_COMMAND | WM_HSCROLL | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT => {
                match GetParent(window) {
                    Ok(parent) => SendMessageW(parent, message, Some(wparam), Some(lparam)),
                    Err(_) => DefWindowProcW(window, message, wparam, lparam),
                }
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
