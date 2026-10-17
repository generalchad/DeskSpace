//! A modern color picker popup (ROADMAP F-51), replacing the dated system
//! `ChooseColor` dialog. It opens on a palette grid inspired by the Windows
//! color dialog: a row of recent colors and a grid of standard colors, with the
//! full saturation/value picker one toggle away ("Custom colors"). It returns
//! the chosen color from a short modal loop.
//!
//! The window is opaque GDI (the settings window's model): it does not extend
//! the DWM frame into the client, because DWM would then read the client's
//! alpha channel, which GDI leaves at zero, and composite the whole popup as
//! glass. Rounded corners and the border come from DWM window attributes.
//!
//! `pick` owns the `PickerState` box; the window only points at it through
//! `GWLP_USERDATA` for the duration of the loop, which ends when the window is
//! destroyed. The loop reads only messages addressed to the picker, so the
//! owner (the settings window or the controller) is frozen while it is open.

use std::cell::RefCell;

use deskspace_core::geometry::{Point, Rect};
use deskspace_core::theme::{Color, Hsv};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_COLOR_DEFAULT, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection,
    CreateFontIndirectW, CreatePen, CreateSolidBrush, DIB_RGB_COLORS, DT_LEFT, DT_SINGLELINE,
    DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect, FrameRect, GdiFlush,
    GetStockObject, HBITMAP, HDC, HFONT, HGDIOBJ, InvalidateRect, NULL_BRUSH, PAINTSTRUCT,
    PS_SOLID, RoundRect, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
    GetMessageW, IsWindow, MSG, PostQuitMessage, SW_SHOW, SetForegroundWindow, ShowWindow,
    TranslateMessage, WM_CLOSE, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SYSKEYDOWN,
    WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::win32::dark_mode::{self, UiColors};
use crate::win32::settings_window::colorref;
use crate::win32::system::{draw_glyph, glyph_font, message_logfont, work_area_at};
use crate::win32::window_class::{self, WindowClass};
use crate::win32::window_state;

mod draw;
mod layout;
mod paint;

pub(crate) use draw::draw_color_box;
use layout::{Panel, PickerLayout, Region, fraction_x, saturation_value, swatch_color};
use paint::{Buffer, paint};

const CLASS_NAME: PCWSTR = w!("DeskSpaceColorPicker");

// Layout metrics at 96 DPI; `PickerLayout::compute` scales them per monitor.
const PAD: i32 = 16;
const SWATCH: i32 = 26;
const SWATCH_GAP: i32 = 4;
const COLS: usize = 8;
const ROWS: usize = 6;
const LABEL_HEIGHT: i32 = 18;
const SECTION_GAP: i32 = 14;
const FIELD_GAP: i32 = 10;
const ROW_GAP: i32 = 12;
const SQUARE_HEIGHT: i32 = 150;
const STRIP_HEIGHT: i32 = 16;
const PREVIEW_WIDTH: i32 = 44;
const PREVIEW_HEIGHT: i32 = 22;
const BUTTON_WIDTH: i32 = 70;
const BUTTON_HEIGHT: i32 = 30;
const BUTTON_GAP: i32 = 6;
const TOGGLE_WIDTH: i32 = 128;
const BUTTON_ROUND: i32 = 8;
/// Corner radius of a color swatch.
const SWATCH_RADIUS: i32 = 4;
/// Size of one checkerboard square behind the alpha strip and the preview.
const CHECKER: i32 = 8;
/// Radius of the marker on the saturation/value square.
const MARKER: i32 = 6;
/// Radius of the marker on the hue and alpha strips.
const STRIP_MARKER: i32 = 7;
/// Segoe Fluent Icons / Segoe MDL2 Assets "CheckMark" (`\u{E73E}`).
const GLYPH_CHECK: u16 = 0xE73E;

/// The standard color grid, eight per row, after the Windows 11 palette: vivid
/// hues, then pastels, then neutrals and earth tones.
const WINDOWS_COLORS: [Color; COLS * ROWS] = [
    Color::rgb(0xC4, 0x2B, 0x1C),
    Color::rgb(0xCA, 0x50, 0x10),
    Color::rgb(0xC1, 0x9C, 0x00),
    Color::rgb(0x10, 0x7C, 0x10),
    Color::rgb(0x03, 0x83, 0x87),
    Color::rgb(0x00, 0x63, 0xB1),
    Color::rgb(0x4F, 0x6B, 0xED),
    Color::rgb(0x74, 0x4D, 0xA9),
    Color::rgb(0xE7, 0x48, 0x56),
    Color::rgb(0xF7, 0x63, 0x0C),
    Color::rgb(0xFF, 0xB9, 0x00),
    Color::rgb(0x10, 0x7C, 0x10),
    Color::rgb(0x00, 0xB7, 0xC3),
    Color::rgb(0x00, 0x78, 0xD4),
    Color::rgb(0x6B, 0x69, 0xD6),
    Color::rgb(0x87, 0x64, 0xB8),
    Color::rgb(0xFF, 0x43, 0x43),
    Color::rgb(0xFF, 0x8C, 0x00),
    Color::rgb(0xFF, 0xD7, 0x00),
    Color::rgb(0x16, 0xC6, 0x0C),
    Color::rgb(0x00, 0xCC, 0xCC),
    Color::rgb(0x00, 0x99, 0xBC),
    Color::rgb(0x8A, 0x8A, 0xFF),
    Color::rgb(0xB1, 0x46, 0xC2),
    Color::rgb(0xFF, 0xB3, 0xB3),
    Color::rgb(0xFF, 0xCC, 0x99),
    Color::rgb(0xFF, 0xE6, 0x99),
    Color::rgb(0xB3, 0xE6, 0xB3),
    Color::rgb(0xB3, 0xEC, 0xEC),
    Color::rgb(0xB3, 0xD9, 0xF2),
    Color::rgb(0xC5, 0xC5, 0xF5),
    Color::rgb(0xE0, 0xB3, 0xE6),
    Color::rgb(0x00, 0x00, 0x00),
    Color::rgb(0x40, 0x40, 0x40),
    Color::rgb(0x60, 0x60, 0x60),
    Color::rgb(0x80, 0x80, 0x80),
    Color::rgb(0xA0, 0xA0, 0xA0),
    Color::rgb(0xC0, 0xC0, 0xC0),
    Color::rgb(0xE0, 0xE0, 0xE0),
    Color::rgb(0xFF, 0xFF, 0xFF),
    Color::rgb(0x5A, 0x3A, 0x1A),
    Color::rgb(0x8A, 0x6B, 0x3A),
    Color::rgb(0xB3, 0x9A, 0x6A),
    Color::rgb(0x4A, 0x5A, 0x3A),
    Color::rgb(0x6A, 0x7A, 0x5A),
    Color::rgb(0x3A, 0x4A, 0x5A),
    Color::rgb(0x5A, 0x6A, 0x7A),
    Color::rgb(0x8F, 0xA0, 0xB0),
];

/// How many recent colors are kept and shown (one row).
const RECENT_LIMIT: usize = COLS;

thread_local! {
    /// The colors picked recently, most recent first, kept for the session.
    static RECENT: RefCell<Vec<Color>> = const { RefCell::new(Vec::new()) };
}

fn remember(color: Color) {
    RECENT.with(|recent| {
        let mut recent = recent.borrow_mut();
        recent.retain(|existing| *existing != color);
        recent.insert(0, color);
        recent.truncate(RECENT_LIMIT);
    });
}

struct PickerState {
    panel: Panel,
    initial: Color,
    dark: bool,
    layout: PickerLayout,
    /// The UI font (the system message font at the owner's DPI), owned here.
    font: HFONT,
    /// The symbol font used for the selected-swatch check mark.
    glyph_font: HFONT,
    /// The back buffer, created on the first paint.
    buffer: Option<Buffer>,
    /// The palette's selected color, when one is picked or the initial matches.
    selected: Option<Color>,
    /// The recently used colors, read once at open.
    recent: Vec<Color>,
    // The custom panel's HSV state, kept so hue survives saturation and value
    // edits.
    hue: f32,
    saturation: f32,
    value: f32,
    alpha: u8,
    result: Option<Color>,
    drag: Option<Region>,
    hover: Option<Region>,
}

impl Drop for PickerState {
    fn drop(&mut self) {
        // SAFETY: both fonts are owned by this state; a null handle (tests) is
        // ignored.
        unsafe {
            for font in [self.font, self.glyph_font] {
                if !font.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(font.0));
                }
            }
        }
    }
}

impl PickerState {
    fn new(
        initial: Color,
        dark: bool,
        layout: PickerLayout,
        font: HFONT,
        glyph_font: HFONT,
    ) -> Self {
        let hsv = initial.to_hsv();
        let recent = RECENT.with(|recent| recent.borrow().clone());
        let selected = WINDOWS_COLORS
            .iter()
            .copied()
            .find(|color| *color == initial);
        Self {
            panel: Panel::Palette,
            initial,
            dark,
            layout,
            font,
            glyph_font,
            buffer: None,
            selected,
            recent,
            hue: hsv.h,
            saturation: hsv.s,
            value: hsv.v,
            alpha: initial.a,
            result: None,
            drag: None,
            hover: None,
        }
    }

    /// The color OK would return right now.
    fn color(&self) -> Color {
        match self.panel {
            Panel::Palette => self.selected.unwrap_or(self.initial),
            Panel::Custom => self.custom_color(),
        }
    }

    fn custom_color(&self) -> Color {
        Hsv {
            h: self.hue,
            s: self.saturation,
            v: self.value,
        }
        .to_color(self.alpha)
    }

    fn set_from_point(&mut self, region: Region, point: Point) {
        match region {
            Region::Saturation => {
                let (saturation, value) = saturation_value(self.layout.saturation, point);
                self.saturation = saturation;
                self.value = value;
            }
            Region::Hue => self.hue = fraction_x(self.layout.hue, point) * 360.0,
            Region::Alpha => {
                self.alpha = (fraction_x(self.layout.alpha, point) * 255.0).round() as u8;
            }
            _ => {}
        }
    }

    /// Switches panels, carrying the current color across.
    fn toggle_panel(&mut self) {
        match self.panel {
            Panel::Palette => {
                let hsv = self.color().to_hsv();
                self.hue = hsv.h;
                self.saturation = hsv.s;
                self.value = hsv.v;
                self.alpha = self.color().a;
                self.panel = Panel::Custom;
            }
            Panel::Custom => {
                let current = self.color();
                self.selected = WINDOWS_COLORS
                    .iter()
                    .copied()
                    .find(|color| *color == current);
                self.panel = Panel::Palette;
            }
        }
    }

    fn select_swatch(&mut self, index: usize) {
        self.selected = swatch_color(&self.recent, index);
    }

    fn region_at(&self, point: Point) -> Option<Region> {
        self.layout.region_at(self.panel, self.recent.len(), point)
    }

    /// Drags a value control and tracks button hover; returns whether to
    /// redraw.
    fn pointer_moved(&mut self, point: Point) -> bool {
        let mut redraw = false;
        if let Some(region) = self.drag
            && region.is_value()
        {
            self.set_from_point(region, point);
            redraw = true;
        }
        let hover = self
            .region_at(point)
            .filter(|region| matches!(region, Region::Ok | Region::Cancel | Region::Toggle));
        if hover != self.hover {
            self.hover = hover;
            redraw = true;
        }
        redraw
    }

    /// Selects a swatch or starts dragging a value control; returns whether to
    /// redraw.
    fn pressed(&mut self, point: Point) -> bool {
        match self.region_at(point) {
            Some(Region::Swatch(index)) => {
                self.select_swatch(index);
                true
            }
            Some(region) if region.is_value() => {
                self.drag = Some(region);
                self.set_from_point(region, point);
                true
            }
            _ => false,
        }
    }

    /// A double-click on a swatch picks it; returns whether the picker is done.
    fn double_clicked(&mut self, point: Point) -> bool {
        let Some(Region::Swatch(index)) = self.region_at(point) else {
            return false;
        };
        self.select_swatch(index);
        confirm(self);
        true
    }

    /// Ends a drag and presses the button under `point`; returns whether the
    /// picker is done.
    fn released(&mut self, point: Point) -> bool {
        self.drag = None;
        match self.region_at(point) {
            Some(Region::Ok) => {
                confirm(self);
                true
            }
            Some(Region::Cancel) => true,
            Some(Region::Toggle) => {
                self.toggle_panel();
                false
            }
            _ => false,
        }
    }
}

/// Opens the picker `dark`-themed over `owner` and returns the chosen color, or
/// `None` if it was cancelled or the window could not be created. Blocks in a
/// modal loop that services only the picker, so the owner waits.
pub(crate) unsafe fn pick(owner: HWND, initial: Color, dark: bool) -> Option<Color> {
    // SAFETY: Win32 window creation on the UI thread. `state` is owned here and
    // dropped after the loop; the window points at it only while it is alive.
    unsafe {
        let instance = match GetModuleHandleW(None) {
            Ok(module) => windows::Win32::Foundation::HINSTANCE(module.0),
            Err(_) => return None,
        };
        ensure_class().ok()?;
        let dpi = dpi_of(owner);
        let layout = PickerLayout::compute(dpi as f32 / 96.0);
        let font = CreateFontIndirectW(&message_logfont(dpi));
        let glyph = glyph_font(dpi);
        let mut state = Box::new(PickerState::new(initial, dark, layout, font, glyph));
        let state_pointer = &mut *state as *mut PickerState;

        let (x, y) = picker_position(layout.size);
        let hwnd = match CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS_NAME,
            w!(""),
            WS_POPUP | WS_CLIPCHILDREN,
            x,
            y,
            layout.size.0,
            layout.size.1,
            Some(owner),
            None,
            Some(instance),
            Some(state_pointer as *const core::ffi::c_void),
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                tracing::warn!(%error, "could not create the color picker window");
                return None;
            }
        };

        style_window(hwnd, dark);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = EnableWindow(owner, false);

        let mut message = MSG::default();
        while IsWindow(Some(hwnd)).as_bool() {
            // Pump every message, not just the picker's: the owner is disabled,
            // so it cannot be driven, but a thread that stops retrieving its
            // own windows' messages is marked "not responding" by Windows.
            let status = GetMessageW(&mut message, None, 0, 0);
            if status.0 == 0 {
                // Another window asked to quit; hand it back to the outer loop.
                PostQuitMessage(message.wParam.0 as i32);
                break;
            }
            if status.0 == -1 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = EnableWindow(owner, true);
        state.result
    }
}

/// Places the picker at the cursor, kept inside that monitor's work area.
fn picker_position(size: (i32, i32)) -> (i32, i32) {
    let mut cursor = POINT::default();
    // SAFETY: `cursor` is a valid out-pointer.
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let work = work_area_at(Point::new(cursor.x, cursor.y));
    (
        cursor
            .x
            .clamp(work.x, (work.x + work.width - size.0).max(work.x)),
        cursor
            .y
            .clamp(work.y, (work.y + work.height - size.1).max(work.y)),
    )
}

/// The DPI of the owner's monitor (96 when it cannot be read).
fn dpi_of(owner: HWND) -> u32 {
    // SAFETY: `owner` is a live window.
    let dpi = unsafe { GetDpiForWindow(owner) };
    if dpi == 0 { 96 } else { dpi }
}

/// Rounds the popup and gives it a captionless dark/light frame. The frame is
/// deliberately not extended over the client: that would make DWM read the
/// client's alpha, which GDI leaves at zero, and the popup would be invisible.
fn style_window(hwnd: HWND, dark: bool) {
    dark_mode::set_caption_dark(hwnd, dark);
    let corner = DWMWCP_ROUND;
    let border = DWMWA_COLOR_DEFAULT;
    // SAFETY: `hwnd` is our live window; both values outlive the calls.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&corner) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&border) as u32,
        );
    }
}

fn ensure_class() -> Result<(), PlatformError> {
    window_class::register(WindowClass::new(CLASS_NAME, Some(wndproc)).style(CS_DBLCLKS))
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut PickerState) -> R) -> Option<R> {
    // SAFETY: picker windows hold the `PickerState` `pick` owns for as long
    // as the window exists; the caller rules out re-entry.
    unsafe { window_state::with_state(window, f) }
}

/// The client point from a mouse message's `lparam`.
fn mouse_point(lparam: LPARAM) -> Point {
    Point::new(
        (lparam.0 & 0xFFFF) as u16 as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32,
    )
}

/// Records the current color as the result and remembers it as recent.
fn confirm(state: &mut PickerState) {
    let color = state.color();
    state.result = Some(color);
    remember(color);
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`; every state
    // access is a short `with_state` borrow that does not call back into here.
    unsafe {
        match message {
            WM_NCCREATE => {
                window_state::attach_create_param::<PickerState>(window, lparam);
                DefWindowProcW(window, message, wparam, lparam)
            }
            // The whole client is painted in `WM_PAINT`.
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                paint(window);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let point = mouse_point(lparam);
                if with_state(window, |state| state.pointer_moved(point)) == Some(true) {
                    let _ = InvalidateRect(Some(window), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let point = mouse_point(lparam);
                let redraw = with_state(window, |state| state.pressed(point));
                SetCapture(window);
                if redraw == Some(true) {
                    let _ = InvalidateRect(Some(window), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                let point = mouse_point(lparam);
                let done = with_state(window, |state| state.double_clicked(point));
                let _ = ReleaseCapture();
                if done == Some(true) {
                    let _ = DestroyWindow(window);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let point = mouse_point(lparam);
                let done = with_state(window, |state| state.released(point));
                let _ = ReleaseCapture();
                if done == Some(true) {
                    let _ = DestroyWindow(window);
                } else {
                    let _ = InvalidateRect(Some(window), None, false);
                }
                LRESULT(0)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let key = wparam.0 as u16;
                if key == VK_ESCAPE.0 {
                    let _ = DestroyWindow(window);
                } else if key == VK_RETURN.0 {
                    with_state(window, confirm);
                    let _ = DestroyWindow(window);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(window);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                window_state::detach(window);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_has_one_swatch_per_cell() {
        assert_eq!(WINDOWS_COLORS.len(), COLS * ROWS);
    }

    #[test]
    fn a_picked_color_round_trips_through_the_state() {
        for color in [
            Color::rgb(0x12, 0x34, 0x56),
            Color::rgba(0xAB, 0xCD, 0xEF, 128),
            Color::rgb(255, 0, 0),
        ] {
            let mut state = PickerState::new(
                color,
                true,
                PickerLayout::compute(1.0),
                HFONT::default(),
                HFONT::default(),
            );
            state.panel = Panel::Custom;
            assert_eq!(state.color(), color);
        }
    }

    #[test]
    fn selecting_a_swatch_changes_the_result() {
        let mut state = PickerState::new(
            Color::rgb(0, 0, 0),
            true,
            PickerLayout::compute(1.0),
            HFONT::default(),
            HFONT::default(),
        );
        assert_eq!(state.color(), Color::rgb(0, 0, 0));
        state.select_swatch(0);
        assert_eq!(state.color(), WINDOWS_COLORS[0]);
    }

    #[test]
    fn toggling_panels_carries_the_color_across() {
        let mut state = PickerState::new(
            Color::rgb(0x10, 0x7C, 0x10),
            true,
            PickerLayout::compute(1.0),
            HFONT::default(),
            HFONT::default(),
        );
        let custom = state.color();
        state.toggle_panel();
        assert_eq!(state.panel, Panel::Custom);
        assert_eq!(state.custom_color(), custom);
        state.toggle_panel();
        assert_eq!(state.panel, Panel::Palette);
        assert_eq!(state.selected, Some(Color::rgb(0x10, 0x7C, 0x10)));
    }

    fn state_at(color: Color) -> PickerState {
        PickerState::new(
            color,
            true,
            PickerLayout::compute(1.0),
            HFONT::default(),
            HFONT::default(),
        )
    }

    fn center(rect: Rect) -> Point {
        Point::new(rect.x + rect.width / 2, rect.y + rect.height / 2)
    }

    #[test]
    fn releasing_on_ok_or_cancel_finishes_and_only_ok_picks() {
        let mut state = state_at(Color::rgb(1, 2, 3));
        assert!(state.released(center(state.layout.cancel)));
        assert_eq!(state.result, None);
        assert!(state.released(center(state.layout.ok)));
        assert_eq!(state.result, Some(Color::rgb(1, 2, 3)));
    }

    #[test]
    fn releasing_on_toggle_switches_panels_without_finishing() {
        let mut state = state_at(Color::rgb(1, 2, 3));
        assert!(!state.released(center(state.layout.toggle)));
        assert_eq!(state.panel, Panel::Custom);
    }

    #[test]
    fn pressing_a_value_control_drags_it_until_release() {
        let mut state = state_at(Color::rgb(1, 2, 3));
        state.toggle_panel();
        let hue = state.layout.hue;
        assert!(state.pressed(Point::new(hue.x, hue.y + 1)));
        assert_eq!(state.drag, Some(Region::Hue));
        assert!(state.pointer_moved(Point::new(hue.x + hue.width, hue.y + 1)));
        assert_eq!(state.hue, 360.0);
        state.released(Point::new(0, 0));
        assert_eq!(state.drag, None);
    }

    #[test]
    fn double_clicking_a_swatch_picks_it() {
        let mut state = state_at(Color::rgb(1, 2, 3));
        let swatch = state.layout.swatch_rect(state.layout.windows_grid, 0);
        assert!(state.double_clicked(center(swatch)));
        assert_eq!(state.result, Some(WINDOWS_COLORS[0]));
        assert!(!state.double_clicked(Point::new(0, 0)));
    }

    #[test]
    fn hovering_a_button_redraws_once() {
        let mut state = state_at(Color::rgb(1, 2, 3));
        let ok = center(state.layout.ok);
        assert!(state.pointer_moved(ok));
        assert_eq!(state.hover, Some(Region::Ok));
        assert!(!state.pointer_moved(ok));
    }
}
