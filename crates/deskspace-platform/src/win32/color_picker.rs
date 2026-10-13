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
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

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
    DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect, FrameRect, GetStockObject,
    HDC, HFONT, HGDIOBJ, InvalidateRect, NULL_BRUSH, PAINTSTRUCT, PS_SOLID, Polyline, RoundRect,
    SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GWLP_USERDATA, GetCursorPos, GetMessageW, GetWindowLongPtrW, IsWindow, MSG, RegisterClassW,
    SW_SHOW, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, TranslateMessage, WM_CLOSE,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SYSKEYDOWN, WNDCLASSW, WS_CLIPCHILDREN,
    WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::win32::dark_mode::{self, UiColors};
use crate::win32::settings_window::colorref;
use crate::win32::system::{message_logfont, work_area_at};

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

static REGISTER_CLASS: Once = Once::new();
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

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

/// Which page of the picker is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Palette,
    Custom,
}

/// Which interactive part of the picker a point falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    /// A swatch in the palette, by index into the recent-then-standard list.
    Swatch(usize),
    Saturation,
    Hue,
    Alpha,
    Toggle,
    Ok,
    Cancel,
}

impl Region {
    fn is_value(self) -> bool {
        matches!(self, Region::Saturation | Region::Hue | Region::Alpha)
    }
}

/// The picker's rectangles, in window-local coordinates. Both panels share one
/// window size, so toggling does not resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PickerLayout {
    size: (i32, i32),
    recent_label: Rect,
    recent_grid: Rect,
    windows_label: Rect,
    windows_grid: Rect,
    saturation: Rect,
    hue: Rect,
    alpha: Rect,
    preview: Rect,
    hex: Rect,
    toggle: Rect,
    ok: Rect,
    cancel: Rect,
    swatch: i32,
    swatch_gap: i32,
    swatch_radius: i32,
}

impl PickerLayout {
    /// Lays the picker out at `scale` (1.0 at 96 DPI).
    fn compute(scale: f32) -> Self {
        let s = |value: i32| ((value as f32) * scale).round() as i32;
        let pad = s(PAD);
        let swatch = s(SWATCH);
        let swatch_gap = s(SWATCH_GAP);
        let label = s(LABEL_HEIGHT);
        let button_gap = s(BUTTON_GAP);
        let button_w = s(BUTTON_WIDTH);
        let toggle_w = s(TOGGLE_WIDTH);
        let grid_width = COLS as i32 * swatch + (COLS as i32 - 1) * swatch_gap;
        // The bar of three buttons is wider than the grid, so the window fits
        // both and the grid is centered in the extra width.
        let buttons_width = toggle_w + button_gap + button_w + button_gap + button_w;
        let content_width = grid_width.max(buttons_width);
        let width = content_width + pad * 2;
        let grid_x = pad + (content_width - grid_width) / 2;

        let recent_label = Rect::new(grid_x, pad, grid_width, label);
        let recent_grid = Rect::new(grid_x, recent_label.y + label + s(4), grid_width, swatch);
        let windows_label = Rect::new(
            grid_x,
            recent_grid.y + swatch + s(SECTION_GAP),
            grid_width,
            label,
        );
        let grid_height = ROWS as i32 * swatch + (ROWS as i32 - 1) * swatch_gap;
        let windows_grid = Rect::new(
            grid_x,
            windows_label.y + label + s(4),
            grid_width,
            grid_height,
        );

        let button_y = windows_grid.y + grid_height + s(16);
        let toggle = Rect::new(pad, button_y, toggle_w, s(BUTTON_HEIGHT));
        let ok = Rect::new(width - pad - button_w, button_y, button_w, s(BUTTON_HEIGHT));
        let cancel = Rect::new(
            ok.x - button_gap - button_w,
            button_y,
            button_w,
            s(BUTTON_HEIGHT),
        );
        let size = (width, button_y + s(BUTTON_HEIGHT) + pad);

        let saturation = Rect::new(pad, pad, content_width, s(SQUARE_HEIGHT));
        let hue = Rect::new(
            pad,
            saturation.y + saturation.height + s(FIELD_GAP),
            content_width,
            s(STRIP_HEIGHT),
        );
        let alpha = Rect::new(
            pad,
            hue.y + hue.height + s(FIELD_GAP),
            content_width,
            s(STRIP_HEIGHT),
        );
        let preview = Rect::new(
            pad,
            alpha.y + alpha.height + s(ROW_GAP),
            s(PREVIEW_WIDTH),
            s(PREVIEW_HEIGHT),
        );
        let hex_x = preview.x + preview.width + s(ROW_GAP);
        let hex = Rect::new(
            hex_x,
            preview.y,
            (content_width - (hex_x - pad)).max(0),
            preview.height,
        );

        Self {
            size,
            recent_label,
            recent_grid,
            windows_label,
            windows_grid,
            saturation,
            hue,
            alpha,
            preview,
            hex,
            toggle,
            ok,
            cancel,
            swatch,
            swatch_gap,
            swatch_radius: s(SWATCH_RADIUS),
        }
    }

    /// The rectangle of swatch `index` in `grid`.
    fn swatch_rect(&self, grid: Rect, index: usize) -> Rect {
        let col = (index % COLS) as i32;
        let row = (index / COLS) as i32;
        Rect::new(
            grid.x + col * (self.swatch + self.swatch_gap),
            grid.y + row * (self.swatch + self.swatch_gap),
            self.swatch,
            self.swatch,
        )
    }

    fn region_at(&self, panel: Panel, recent_len: usize, point: Point) -> Option<Region> {
        if self.ok.contains(point) {
            return Some(Region::Ok);
        }
        if self.cancel.contains(point) {
            return Some(Region::Cancel);
        }
        if self.toggle.contains(point) {
            return Some(Region::Toggle);
        }
        match panel {
            Panel::Palette => {
                for index in 0..recent_len.min(COLS) {
                    if self.swatch_rect(self.recent_grid, index).contains(point) {
                        return Some(Region::Swatch(index));
                    }
                }
                for index in 0..WINDOWS_COLORS.len() {
                    if self.swatch_rect(self.windows_grid, index).contains(point) {
                        return Some(Region::Swatch(recent_len + index));
                    }
                }
                None
            }
            Panel::Custom => {
                if self.saturation.contains(point) {
                    Some(Region::Saturation)
                } else if self.hue.contains(point) {
                    Some(Region::Hue)
                } else if self.alpha.contains(point) {
                    Some(Region::Alpha)
                } else {
                    None
                }
            }
        }
    }
}

/// The color of the swatch with `index` in the recent-then-standard list.
fn swatch_color(recent: &[Color], index: usize) -> Option<Color> {
    if index < recent.len() {
        recent.get(index).copied()
    } else {
        WINDOWS_COLORS.get(index - recent.len()).copied()
    }
}

/// The saturation and value (`0.0..=1.0`) a point maps to in the square. The
/// top edge is value 1 and the left edge is saturation 0.
fn saturation_value(rect: Rect, point: Point) -> (f32, f32) {
    let width = (rect.width - 1).max(1) as f32;
    let height = (rect.height - 1).max(1) as f32;
    (
        ((point.x - rect.x) as f32 / width).clamp(0.0, 1.0),
        1.0 - ((point.y - rect.y) as f32 / height).clamp(0.0, 1.0),
    )
}

/// The horizontal fraction (`0.0..=1.0`) a point maps to within `rect`.
fn fraction_x(rect: Rect, point: Point) -> f32 {
    let width = (rect.width - 1).max(1) as f32;
    ((point.x - rect.x) as f32 / width).clamp(0.0, 1.0)
}

/// Blends `over` onto `base` by `alpha` (`0.0..=1.0`), to draw a translucent
/// color over the checkerboard.
fn blend(base: Color, over: Color, alpha: f32) -> Color {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |b: u8, o: u8| ((b as f32 * (1.0 - a)) + (o as f32 * a)).round() as u8;
    Color::rgb(
        mix(base.r, over.r),
        mix(base.g, over.g),
        mix(base.b, over.b),
    )
}

/// The checkerboard cell color under a point.
fn checker_color(x: i32, y: i32, colors: UiColors) -> Color {
    let (light, dark) = if colors.dark {
        (Color::rgb(0x60, 0x60, 0x60), Color::rgb(0x38, 0x38, 0x38))
    } else {
        (Color::rgb(0xFF, 0xFF, 0xFF), Color::rgb(0xC8, 0xC8, 0xC8))
    };
    if ((x / CHECKER) + (y / CHECKER)) % 2 == 0 {
        light
    } else {
        dark
    }
}

struct PickerState {
    panel: Panel,
    initial: Color,
    dark: bool,
    layout: PickerLayout,
    /// The UI font (the system message font at the owner's DPI), owned here.
    font: HFONT,
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
        // SAFETY: the font is owned by this state; a null handle (tests) is
        // ignored.
        unsafe {
            if !self.font.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(self.font.0));
            }
        }
    }
}

impl PickerState {
    fn new(initial: Color, dark: bool, layout: PickerLayout, font: HFONT) -> Self {
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
        if !ensure_class(instance) {
            return None;
        }
        let dpi = dpi_of(owner);
        let layout = PickerLayout::compute(dpi as f32 / 96.0);
        let font = CreateFontIndirectW(&message_logfont(dpi));
        let mut state = Box::new(PickerState::new(initial, dark, layout, font));
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
            let status = GetMessageW(&mut message, Some(hwnd), 0, 0);
            if status.0 <= 0 {
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

fn ensure_class(instance: windows::Win32::Foundation::HINSTANCE) -> bool {
    REGISTER_CLASS.call_once(|| {
        let class = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` is fully initialised and its class name is a static
        // string that outlives the process.
        CLASS_REGISTERED.store(unsafe { RegisterClassW(&class) } != 0, Ordering::SeqCst);
    });
    CLASS_REGISTERED.load(Ordering::SeqCst)
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
unsafe fn with_state<R>(window: HWND, f: impl FnOnce(&mut PickerState) -> R) -> Option<R> {
    // SAFETY: the pointer is the live state `pick` owns for as long as the
    // window exists, and `f` cannot re-enter to create a second borrow.
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut PickerState;
    unsafe { pointer.as_mut() }.map(f)
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
                let create = lparam.0 as *const CREATESTRUCTW;
                if !create.is_null() {
                    let state = (*create).lpCreateParams as *mut PickerState;
                    if !state.is_null() {
                        SetWindowLongPtrW(window, GWLP_USERDATA, state as isize);
                    }
                }
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
                let redraw = with_state(window, |state| {
                    let mut redraw = false;
                    if let Some(region) = state.drag
                        && region.is_value()
                    {
                        state.set_from_point(region, point);
                        redraw = true;
                    }
                    let hover = state
                        .layout
                        .region_at(state.panel, state.recent.len(), point)
                        .filter(|region| {
                            matches!(region, Region::Ok | Region::Cancel | Region::Toggle)
                        });
                    if hover != state.hover {
                        state.hover = hover;
                        redraw = true;
                    }
                    redraw
                })
                .unwrap_or(false);
                if redraw {
                    let _ = InvalidateRect(Some(window), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let point = mouse_point(lparam);
                let redraw = with_state(window, |state| {
                    match state
                        .layout
                        .region_at(state.panel, state.recent.len(), point)
                    {
                        Some(Region::Swatch(index)) => {
                            state.select_swatch(index);
                            true
                        }
                        Some(region) if region.is_value() => {
                            state.drag = Some(region);
                            state.set_from_point(region, point);
                            true
                        }
                        _ => false,
                    }
                })
                .unwrap_or(false);
                SetCapture(window);
                if redraw {
                    let _ = InvalidateRect(Some(window), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                let point = mouse_point(lparam);
                let finish = with_state(window, |state| {
                    match state
                        .layout
                        .region_at(state.panel, state.recent.len(), point)
                    {
                        Some(Region::Swatch(index)) => {
                            state.select_swatch(index);
                            confirm(state);
                            true
                        }
                        _ => false,
                    }
                })
                .unwrap_or(false);
                let _ = ReleaseCapture();
                if finish {
                    let _ = DestroyWindow(window);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let point = mouse_point(lparam);
                let finish = with_state(window, |state| {
                    state.drag = None;
                    match state
                        .layout
                        .region_at(state.panel, state.recent.len(), point)
                    {
                        Some(Region::Ok) => {
                            confirm(state);
                            true
                        }
                        Some(Region::Cancel) => true,
                        Some(Region::Toggle) => {
                            state.toggle_panel();
                            false
                        }
                        _ => false,
                    }
                })
                .unwrap_or(false);
                let _ = ReleaseCapture();
                if finish {
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
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

unsafe fn paint(window: HWND) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: BeginPaint/EndPaint bracket the paint; GDI drawing does not send
    // messages, so the state borrow inside is safe.
    unsafe {
        let hdc = BeginPaint(window, &mut ps);
        let _ = with_state(window, |state| paint_content(hdc, state));
        let _ = EndPaint(window, &ps);
    }
}

unsafe fn paint_content(hdc: HDC, state: &PickerState) {
    let colors = UiColors::new(state.dark);
    let whole = Rect::new(0, 0, state.layout.size.0, state.layout.size.1);
    // SAFETY: `hdc` is a live paint DC; every GDI object created here is
    // selected out and deleted before this function returns.
    unsafe {
        fill_solid(hdc, whole, colors.body);
        let previous_font = SelectObject(hdc, HGDIOBJ(state.font.0));
        match state.panel {
            Panel::Palette => paint_palette(hdc, state, colors),
            Panel::Custom => paint_custom(hdc, state, colors),
        }
        let toggle = match state.panel {
            Panel::Palette => "Custom colors\u{2026}",
            Panel::Custom => "Colors",
        };
        draw_button(
            hdc,
            state.layout.toggle,
            toggle,
            colors,
            false,
            state.hover == Some(Region::Toggle),
        );
        draw_button(
            hdc,
            state.layout.cancel,
            "Cancel",
            colors,
            false,
            state.hover == Some(Region::Cancel),
        );
        draw_button(
            hdc,
            state.layout.ok,
            "OK",
            colors,
            true,
            state.hover == Some(Region::Ok),
        );
        SelectObject(hdc, previous_font);
    }
}

unsafe fn paint_palette(hdc: HDC, state: &PickerState, colors: UiColors) {
    // SAFETY: caller holds a live paint DC.
    unsafe {
        draw_label(hdc, state.layout.recent_label, "Recent colors", colors);
        for index in 0..state.recent.len().min(COLS) {
            let rect = state.layout.swatch_rect(state.layout.recent_grid, index);
            let selected = state.selected == Some(state.recent[index]);
            draw_swatch(
                hdc,
                rect,
                state.recent[index],
                selected,
                colors,
                state.layout,
            );
        }
        draw_label(hdc, state.layout.windows_label, "Windows colors", colors);
        for (index, color) in WINDOWS_COLORS.iter().enumerate() {
            let rect = state.layout.swatch_rect(state.layout.windows_grid, index);
            draw_swatch(
                hdc,
                rect,
                *color,
                state.selected == Some(*color),
                colors,
                state.layout,
            );
        }
    }
}

unsafe fn paint_custom(hdc: HDC, state: &PickerState, colors: UiColors) {
    let current = state.custom_color();
    let square = state.layout.saturation;
    let hue = state.hue;
    // SAFETY: caller holds a live paint DC.
    unsafe {
        fill_image(hdc, square, |x, y| {
            let s = x as f32 / (square.width - 1).max(1) as f32;
            let v = 1.0 - y as f32 / (square.height - 1).max(1) as f32;
            Hsv { h: hue, s, v }.to_color(255)
        });

        let hue_rect = state.layout.hue;
        fill_image(hdc, hue_rect, |x, _| {
            let h = x as f32 / (hue_rect.width - 1).max(1) as f32 * 360.0;
            Hsv { h, s: 1.0, v: 1.0 }.to_color(255)
        });

        let alpha_rect = state.layout.alpha;
        fill_image(hdc, alpha_rect, |x, y| {
            let a = x as f32 / (alpha_rect.width - 1).max(1) as f32;
            blend(checker_color(x, y, colors), current, a)
        });

        let preview = state.layout.preview;
        let preview_alpha = current.a as f32 / 255.0;
        fill_image(hdc, preview, |x, y| {
            blend(checker_color(x, y, colors), current, preview_alpha)
        });
        frame(hdc, preview, colors.divider);

        let marker = scaled_marker(state.layout, MARKER);
        draw_marker(
            hdc,
            square.x + (state.saturation * (square.width - 1) as f32).round() as i32,
            square.y + ((1.0 - state.value) * (square.height - 1) as f32).round() as i32,
            marker,
            true,
        );
        let strip = scaled_marker(state.layout, STRIP_MARKER);
        let hue_cx = hue_rect.x + (state.hue / 360.0 * (hue_rect.width - 1) as f32).round() as i32;
        draw_marker(hdc, hue_cx, hue_rect.y + hue_rect.height / 2, strip, false);
        let alpha_cx = alpha_rect.x
            + (state.alpha as f32 / 255.0 * (alpha_rect.width - 1) as f32).round() as i32;
        draw_marker(
            hdc,
            alpha_cx,
            alpha_rect.y + alpha_rect.height / 2,
            strip,
            false,
        );

        draw_hex(hdc, state.layout.hex, current, colors);
    }
}

/// The marker radius scaled with the rest of the layout.
fn scaled_marker(layout: PickerLayout, base: i32) -> i32 {
    let reference = COLS as i32 * SWATCH + (COLS as i32 - 1) * SWATCH_GAP + PAD * 2;
    (base as f32 * layout.size.0 as f32 / reference as f32).round() as i32
}

fn rect_to_win(rect: Rect) -> RECT {
    RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    }
}

unsafe fn fill_solid(hdc: HDC, rect: Rect, color: Color) {
    // SAFETY: the brush is deleted before returning; FillRect leaves it
    // selected in no DC.
    unsafe {
        let brush = CreateSolidBrush(colorref(color));
        let win = rect_to_win(rect);
        let _ = FillRect(hdc, &win, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

unsafe fn frame(hdc: HDC, rect: Rect, color: Color) {
    // SAFETY: a one-pixel outline drawn with a temporary brush, deleted before
    // returning.
    unsafe {
        let brush = CreateSolidBrush(colorref(color));
        let win = rect_to_win(rect);
        let _ = FrameRect(hdc, &win, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// Draws a color preview: a slightly rounded fill with a one-pixel border.
/// Shared by the picker's swatches and the settings window's color fields.
pub(crate) unsafe fn draw_color_box(
    hdc: HDC,
    rect: Rect,
    color: Color,
    radius: i32,
    border: Color,
) {
    // SAFETY: the brush and pen are selected out and deleted before returning.
    unsafe {
        let brush = CreateSolidBrush(colorref(color));
        let pen = CreatePen(PS_SOLID, 1, colorref(border));
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = RoundRect(
            hdc,
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height,
            radius * 2,
            radius * 2,
        );
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// Draws a hollow rounded outline of `width` just inside `rect`.
unsafe fn draw_ring(hdc: HDC, rect: Rect, color: Color, width: i32, radius: i32) {
    // SAFETY: the pen is selected out and deleted; the stock null brush fills
    // nothing.
    unsafe {
        let pen = CreatePen(PS_SOLID, width, colorref(color));
        let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = RoundRect(
            hdc,
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height,
            radius * 2,
            radius * 2,
        );
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

/// Draws one palette swatch: its color in a slightly rounded box, a defining
/// outline, and, when selected, an accent ring and a check mark.
unsafe fn draw_swatch(
    hdc: HDC,
    rect: Rect,
    color: Color,
    selected: bool,
    colors: UiColors,
    layout: PickerLayout,
) {
    // SAFETY: caller holds a live paint DC.
    unsafe {
        draw_color_box(hdc, rect, color, layout.swatch_radius, colors.divider);
        if selected {
            draw_ring(hdc, rect, colors.accent, 2, layout.swatch_radius);
            draw_check(hdc, rect, color, layout.swatch);
        }
    }
}

/// Draws a check mark scaled to a `swatch`-sized box, in black or white to
/// contrast with `color`.
unsafe fn draw_check(hdc: HDC, rect: Rect, color: Color, swatch: i32) {
    let unit = swatch as f32 / SWATCH as f32;
    let at = |x: f32, y: f32| POINT {
        x: rect.x + (x * unit).round() as i32,
        y: rect.y + (y * unit).round() as i32,
    };
    let points = [at(6.0, 13.0), at(11.0, 18.0), at(20.0, 7.0)];
    let ink = if color.luminance() < 0.5 {
        Color::rgb(255, 255, 255)
    } else {
        Color::rgb(0, 0, 0)
    };
    // SAFETY: the pen is selected out and deleted before returning.
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, colorref(ink));
        let previous = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = Polyline(hdc, &points);
        SelectObject(hdc, previous);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

unsafe fn draw_marker(hdc: HDC, cx: i32, cy: i32, radius: i32, filled: bool) {
    // SAFETY: a hollow ellipse outline drawn with stock brushes and temporary
    // pens, all restored or deleted before returning.
    unsafe {
        let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        let black = CreatePen(PS_SOLID, 2, colorref(Color::rgb(0, 0, 0)));
        let white = CreatePen(PS_SOLID, 2, colorref(Color::rgb(255, 255, 255)));
        let old_pen = SelectObject(hdc, HGDIOBJ(black.0));
        let _ = windows::Win32::Graphics::Gdi::Ellipse(
            hdc,
            cx - radius - 1,
            cy - radius - 1,
            cx + radius + 1,
            cy + radius + 1,
        );
        let _ = SelectObject(hdc, HGDIOBJ(white.0));
        let _ = windows::Win32::Graphics::Gdi::Ellipse(
            hdc,
            cx - radius,
            cy - radius,
            cx + radius,
            cy + radius,
        );
        if filled {
            // A small dot at the center makes the point easier to read on a
            // saturated square.
            let dot = CreateSolidBrush(colorref(Color::rgb(255, 255, 255)));
            let old = SelectObject(hdc, HGDIOBJ(dot.0));
            let _ = windows::Win32::Graphics::Gdi::Ellipse(hdc, cx - 2, cy - 2, cx + 2, cy + 2);
            SelectObject(hdc, old);
            let _ = DeleteObject(HGDIOBJ(dot.0));
        }
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(black.0));
        let _ = DeleteObject(HGDIOBJ(white.0));
    }
}

unsafe fn draw_label(hdc: HDC, rect: Rect, text: &str, colors: UiColors) {
    // SAFETY: the text buffer and rectangle outlive the call.
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, colorref(colors.text));
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let mut text_rect = rect_to_win(rect);
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut text_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

unsafe fn draw_hex(hdc: HDC, rect: Rect, color: Color, colors: UiColors) {
    // SAFETY: the text buffer and rectangle outlive the call.
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, colorref(colors.text));
        let text = format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.r, color.g, color.b, color.a
        );
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let mut text_rect = rect_to_win(rect);
        text_rect.left += 2;
        let _ = DrawTextW(hdc, &mut wide, &mut text_rect, DT_VCENTER | DT_SINGLELINE);
    }
}

unsafe fn draw_button(
    hdc: HDC,
    rect: Rect,
    label: &str,
    colors: UiColors,
    accent: bool,
    hovered: bool,
) {
    let (fill, text) = if accent {
        (colors.accent, Color::rgb(255, 255, 255))
    } else if hovered {
        (colors.divider, colors.text)
    } else {
        (colors.command_bar, colors.text)
    };
    // SAFETY: the brush and pen are selected out and deleted before returning;
    // the text buffer lives for the call.
    unsafe {
        let brush = CreateSolidBrush(colorref(fill));
        let pen = CreatePen(PS_SOLID, 1, colorref(fill));
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = RoundRect(
            hdc,
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height,
            BUTTON_ROUND * 2,
            BUTTON_ROUND * 2,
        );
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
        let _ = DeleteObject(HGDIOBJ(brush.0));

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, colorref(text));
        let mut wide: Vec<u16> = label.encode_utf16().collect();
        let mut text_rect = rect_to_win(rect);
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut text_rect,
            windows::Win32::Graphics::Gdi::DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

/// Fills `dest` pixel by pixel from `color_at`, drawn through a top-down 32bpp
/// DIB so a smooth gradient costs one `BitBlt` rather than per-pixel drawing.
unsafe fn fill_image(hdc: HDC, dest: Rect, color_at: impl Fn(i32, i32) -> Color) {
    let width = dest.width.max(1);
    let height = dest.height.max(1);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised; `bits` receives the DIB's pixels.
    unsafe {
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let memory = CreateCompatibleDC(Some(hdc));
        let Ok(bitmap) = CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(memory);
            return;
        };
        if !bits.is_null() {
            let pixels = bits as *mut u32;
            for y in 0..height {
                for x in 0..width {
                    let color = color_at(x, y);
                    // BI_RGB stores bytes B, G, R, A, so little-endian u32 is
                    // A << 24 | R << 16 | G << 8 | B.
                    *pixels.add((y * width + x) as usize) = (color.a as u32) << 24
                        | (color.r as u32) << 16
                        | (color.g as u32) << 8
                        | color.b as u32;
                }
            }
        }
        let previous = SelectObject(memory, HGDIOBJ(bitmap.0));
        let _ = BitBlt(
            hdc,
            dest.x,
            dest.y,
            width,
            height,
            Some(memory),
            0,
            0,
            SRCCOPY,
        );
        SelectObject(memory, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory);
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
    fn layout_places_every_part_inside_the_window() {
        let layout = PickerLayout::compute(1.0);
        for rect in [
            layout.recent_label,
            layout.recent_grid,
            layout.windows_label,
            layout.windows_grid,
            layout.saturation,
            layout.hue,
            layout.alpha,
            layout.preview,
            layout.hex,
            layout.toggle,
            layout.ok,
            layout.cancel,
        ] {
            assert!(rect.x >= 0 && rect.y >= 0, "{rect:?}");
            assert!(rect.x + rect.width <= layout.size.0, "{rect:?}");
            assert!(rect.y + rect.height <= layout.size.1, "{rect:?}");
        }
        assert!(layout.recent_grid.y < layout.windows_grid.y);
        assert!(layout.windows_grid.y < layout.ok.y);
    }

    #[test]
    fn layout_scales_with_dpi() {
        let one = PickerLayout::compute(1.0);
        let two = PickerLayout::compute(2.0);
        assert_eq!(two.size, (one.size.0 * 2, one.size.1 * 2));
        assert_eq!(two.swatch, one.swatch * 2);
    }

    #[test]
    fn the_button_bar_fits_without_overlap() {
        let layout = PickerLayout::compute(1.0);
        assert!(
            layout.toggle.x + layout.toggle.width <= layout.cancel.x,
            "the toggle must not overlap Cancel"
        );
        assert!(layout.cancel.x + layout.cancel.width <= layout.ok.x);
        assert!(layout.ok.x + layout.ok.width <= layout.size.0);
        // The narrow grid is centered in the wider button bar.
        let left = layout.windows_grid.x;
        let right = layout.size.0 - (layout.windows_grid.x + layout.windows_grid.width);
        assert!((left - right).abs() <= 1);
    }

    #[test]
    fn swatch_rects_tile_the_grid() {
        let layout = PickerLayout::compute(1.0);
        let first = layout.swatch_rect(layout.windows_grid, 0);
        let next = layout.swatch_rect(layout.windows_grid, 1);
        assert_eq!(next.x, first.x + layout.swatch + layout.swatch_gap);
        assert_eq!(next.y, first.y);
        let second_row = layout.swatch_rect(layout.windows_grid, COLS);
        assert_eq!(second_row.x, first.x);
        assert_eq!(second_row.y, first.y + layout.swatch + layout.swatch_gap);
    }

    #[test]
    fn region_at_maps_the_palette_and_buttons() {
        let layout = PickerLayout::compute(1.0);
        let center = |rect: Rect| Point::new(rect.x + rect.width / 2, rect.y + rect.height / 2);
        let recent = 2;
        assert_eq!(
            layout.region_at(
                Panel::Palette,
                recent,
                center(layout.swatch_rect(layout.recent_grid, 0))
            ),
            Some(Region::Swatch(0))
        );
        assert_eq!(
            layout.region_at(
                Panel::Palette,
                recent,
                center(layout.swatch_rect(layout.windows_grid, 3))
            ),
            Some(Region::Swatch(recent + 3))
        );
        assert_eq!(
            layout.region_at(Panel::Palette, recent, center(layout.ok)),
            Some(Region::Ok)
        );
        assert_eq!(
            layout.region_at(Panel::Palette, recent, center(layout.cancel)),
            Some(Region::Cancel)
        );
        assert_eq!(
            layout.region_at(Panel::Palette, recent, center(layout.toggle)),
            Some(Region::Toggle)
        );
        assert_eq!(
            layout.region_at(Panel::Palette, recent, Point::new(0, 0)),
            None
        );
    }

    #[test]
    fn region_at_maps_the_custom_controls() {
        let layout = PickerLayout::compute(1.0);
        let center = |rect: Rect| Point::new(rect.x + rect.width / 2, rect.y + rect.height / 2);
        assert_eq!(
            layout.region_at(Panel::Custom, 0, center(layout.saturation)),
            Some(Region::Saturation)
        );
        assert_eq!(
            layout.region_at(Panel::Custom, 0, center(layout.hue)),
            Some(Region::Hue)
        );
        assert_eq!(
            layout.region_at(Panel::Custom, 0, center(layout.alpha)),
            Some(Region::Alpha)
        );
    }

    #[test]
    fn swatch_color_reads_recent_then_standard() {
        let recent = vec![Color::rgb(1, 2, 3), Color::rgb(4, 5, 6)];
        assert_eq!(swatch_color(&recent, 0), Some(Color::rgb(1, 2, 3)));
        assert_eq!(swatch_color(&recent, 1), Some(Color::rgb(4, 5, 6)));
        assert_eq!(swatch_color(&recent, 2), Some(WINDOWS_COLORS[0]));
        assert_eq!(swatch_color(&recent, 999), None);
    }

    #[test]
    fn saturation_value_maps_the_square_corners() {
        let rect = Rect::new(10, 20, 201, 101);
        assert_eq!(saturation_value(rect, Point::new(10, 20)), (0.0, 1.0));
        assert_eq!(saturation_value(rect, Point::new(210, 120)), (1.0, 0.0));
        assert_eq!(saturation_value(rect, Point::new(-50, -50)), (0.0, 1.0));
        assert_eq!(saturation_value(rect, Point::new(9999, 9999)), (1.0, 0.0));
    }

    #[test]
    fn fraction_x_maps_and_clamps() {
        let rect = Rect::new(10, 0, 101, 16);
        assert_eq!(fraction_x(rect, Point::new(10, 5)), 0.0);
        assert_eq!(fraction_x(rect, Point::new(110, 5)), 1.0);
        assert_eq!(fraction_x(rect, Point::new(60, 5)), 0.5);
        assert_eq!(fraction_x(rect, Point::new(-1, 5)), 0.0);
    }

    #[test]
    fn blend_hits_both_ends() {
        let base = Color::rgb(0, 0, 0);
        let over = Color::rgb(255, 255, 255);
        assert_eq!(blend(base, over, 0.0), base);
        assert_eq!(blend(base, over, 1.0), over);
    }

    #[test]
    fn a_picked_color_round_trips_through_the_state() {
        for color in [
            Color::rgb(0x12, 0x34, 0x56),
            Color::rgba(0xAB, 0xCD, 0xEF, 128),
            Color::rgb(255, 0, 0),
        ] {
            let mut state =
                PickerState::new(color, true, PickerLayout::compute(1.0), HFONT::default());
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
        );
        let custom = state.color();
        state.toggle_panel();
        assert_eq!(state.panel, Panel::Custom);
        assert_eq!(state.custom_color(), custom);
        state.toggle_panel();
        assert_eq!(state.panel, Panel::Palette);
        assert_eq!(state.selected, Some(Color::rgb(0x10, 0x7C, 0x10)));
    }
}
