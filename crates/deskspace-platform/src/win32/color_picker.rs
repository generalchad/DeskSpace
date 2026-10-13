//! A modern color picker popup (ROADMAP F-51), replacing the dated system
//! `ChooseColor` dialog. It draws a saturation/value square, hue and alpha
//! strips, a preview, and OK/Cancel in DeskSpace's own style, and returns the
//! chosen color from a short modal loop.
//!
//! `pick` owns the `PickerState` box; the window only points at it through
//! `GWLP_USERDATA` for the duration of the loop, which ends when the window is
//! destroyed. The loop reads only messages addressed to the picker, so the
//! owner (the settings window or the controller) is frozen while it is open.

use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use deskspace_core::geometry::{Point, Rect};
use deskspace_core::theme::{Backdrop, Color, Hsv};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection,
    CreatePen, CreateSolidBrush, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteDC,
    DeleteObject, DrawTextW, Ellipse, EndPaint, FillRect, GetStockObject, HDC, HGDIOBJ,
    InvalidateRect, NULL_BRUSH, PAINTSTRUCT, PS_SOLID, RoundRect, SRCCOPY, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA,
    GetCursorPos, GetMessageW, GetWindowLongPtrW, IsWindow, MSG, RegisterClassW, SW_SHOW,
    SetForegroundWindow, SetWindowLongPtrW, ShowWindow, TranslateMessage, WM_CLOSE, WM_ERASEBKGND,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WM_PAINT,
    WM_SYSKEYDOWN, WNDCLASSW, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::render::SpaceStyle;
use crate::win32::dark_mode::UiColors;
use crate::win32::settings_window::colorref;
use crate::win32::system::work_area_at;

const CLASS_NAME: PCWSTR = w!("DeskSpaceColorPicker");

// Layout metrics at 96 DPI; `PickerLayout::compute` scales them per monitor.
const PAD: i32 = 14;
const SQUARE_WIDTH: i32 = 252;
const SQUARE_HEIGHT: i32 = 150;
const STRIP_HEIGHT: i32 = 16;
const GAP: i32 = 10;
const ROW_GAP: i32 = 12;
const PREVIEW_WIDTH: i32 = 44;
const PREVIEW_HEIGHT: i32 = 22;
const BUTTON_WIDTH: i32 = 72;
const BUTTON_HEIGHT: i32 = 28;
/// Size of one checkerboard square behind the alpha strip and the preview.
const CHECKER: i32 = 8;
/// Radius of the marker on the saturation/value square.
const MARKER: i32 = 6;
/// Radius of the marker on the hue and alpha strips.
const STRIP_MARKER: i32 = 7;
/// Corner radius of the buttons.
const BUTTON_ROUND: i32 = 8;

static REGISTER_CLASS: Once = Once::new();
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Which interactive part of the picker a point falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Saturation,
    Hue,
    Alpha,
    Ok,
    Cancel,
}

impl Region {
    /// Whether the region is one of the color controls (as opposed to a button).
    fn is_value(self) -> bool {
        matches!(self, Region::Saturation | Region::Hue | Region::Alpha)
    }
}

/// The picker's rectangles, in window-local coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PickerLayout {
    size: (i32, i32),
    saturation: Rect,
    hue: Rect,
    alpha: Rect,
    preview: Rect,
    hex: Rect,
    ok: Rect,
    cancel: Rect,
}

impl PickerLayout {
    /// Lays the picker out at `scale` (1.0 at 96 DPI).
    fn compute(scale: f32) -> Self {
        let s = |value: i32| ((value as f32) * scale).round() as i32;
        let pad = s(PAD);
        let square_w = s(SQUARE_WIDTH);
        let square_h = s(SQUARE_HEIGHT);
        let strip_h = s(STRIP_HEIGHT);
        let gap = s(GAP);
        let row_gap = s(ROW_GAP);
        let preview_w = s(PREVIEW_WIDTH);
        let preview_h = s(PREVIEW_HEIGHT);
        let button_w = s(BUTTON_WIDTH);
        let button_h = s(BUTTON_HEIGHT);
        let width = square_w + pad * 2;

        let saturation = Rect::new(pad, pad, square_w, square_h);
        let hue = Rect::new(
            pad,
            saturation.y + saturation.height + gap,
            square_w,
            strip_h,
        );
        let alpha = Rect::new(pad, hue.y + hue.height + gap, square_w, strip_h);
        let preview = Rect::new(pad, alpha.y + alpha.height + row_gap, preview_w, preview_h);
        let hex_x = preview.x + preview.width + row_gap;
        let hex = Rect::new(hex_x, preview.y, (width - pad - hex_x).max(0), preview_h);
        let buttons_y = preview.y + preview.height + row_gap;
        let ok = Rect::new(width - pad - button_w, buttons_y, button_w, button_h);
        let cancel = Rect::new(ok.x - row_gap - button_w, buttons_y, button_w, button_h);
        Self {
            size: (width, buttons_y + button_h + pad),
            saturation,
            hue,
            alpha,
            preview,
            hex,
            ok,
            cancel,
        }
    }

    fn region_at(&self, point: Point) -> Option<Region> {
        if self.saturation.contains(point) {
            Some(Region::Saturation)
        } else if self.hue.contains(point) {
            Some(Region::Hue)
        } else if self.alpha.contains(point) {
            Some(Region::Alpha)
        } else if self.ok.contains(point) {
            Some(Region::Ok)
        } else if self.cancel.contains(point) {
            Some(Region::Cancel)
        } else {
            None
        }
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

/// Blends `over` onto `base` by `alpha` (`0.0..=1.0`), used to draw a
/// translucent color over the checkerboard.
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
    hue: f32,
    saturation: f32,
    value: f32,
    alpha: u8,
    dark: bool,
    layout: PickerLayout,
    result: Option<Color>,
    /// The value control the pointer is dragging, if any.
    drag: Option<Region>,
    /// The button the pointer is over, for the hover highlight.
    hover: Option<Region>,
}

impl PickerState {
    fn new(initial: Color, dark: bool, layout: PickerLayout) -> Self {
        let hsv = initial.to_hsv();
        Self {
            hue: hsv.h,
            saturation: hsv.s,
            value: hsv.v,
            alpha: initial.a,
            dark,
            layout,
            result: None,
            drag: None,
            hover: None,
        }
    }

    fn color(&self) -> Color {
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
            Region::Ok | Region::Cancel => {}
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
        if !ensure_class(instance) {
            return None;
        }
        let layout = PickerLayout::compute(dpi_scale(owner));
        let mut state = Box::new(PickerState::new(initial, dark, layout));
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

        crate::win32::dwm::apply_style(hwnd, picker_style(dark));
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

/// The DPI scale of the owner's monitor (`1.0` at 96 DPI).
fn dpi_scale(owner: HWND) -> f32 {
    // SAFETY: `owner` is a live window.
    let dpi = unsafe { GetDpiForWindow(owner) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// A solid, rounded popup with a DWM border and the app's dark/light mode.
fn picker_style(dark: bool) -> SpaceStyle {
    SpaceStyle {
        backdrop: Backdrop::Solid,
        corner_radius: 8,
        dark,
        border: true,
        chrome: true,
    }
}

fn ensure_class(instance: windows::Win32::Foundation::HINSTANCE) -> bool {
    REGISTER_CLASS.call_once(|| {
        let class = WNDCLASSW {
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
                    if let Some(region) = state.drag {
                        state.set_from_point(region, point);
                        redraw = true;
                    }
                    let hover = state
                        .layout
                        .region_at(point)
                        .filter(|region| !region.is_value());
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
                with_state(window, |state| {
                    if let Some(region) = state.layout.region_at(point)
                        && region.is_value()
                    {
                        state.drag = Some(region);
                        state.set_from_point(region, point);
                    }
                });
                SetCapture(window);
                let _ = InvalidateRect(Some(window), None, false);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let point = mouse_point(lparam);
                let finish = with_state(window, |state| {
                    state.drag = None;
                    match state.layout.region_at(point) {
                        Some(Region::Ok) => {
                            state.result = Some(state.color());
                            true
                        }
                        Some(Region::Cancel) => {
                            state.result = None;
                            true
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
                    with_state(window, |state| state.result = Some(state.color()));
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
    let current = state.color();
    let whole = Rect::new(0, 0, state.layout.size.0, state.layout.size.1);
    // SAFETY: `hdc` is a live paint DC; every GDI object created here is
    // selected out and deleted before this function returns.
    unsafe {
        fill_solid(hdc, whole, colors.body);

        // Saturation falls to the right, value rises upward.
        let square = state.layout.saturation;
        let hue = state.hue;
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
        );
        let strip = scaled_marker(state.layout, STRIP_MARKER);
        let hue_cx = hue_rect.x + (state.hue / 360.0 * (hue_rect.width - 1) as f32).round() as i32;
        draw_marker(hdc, hue_cx, hue_rect.y + hue_rect.height / 2, strip);
        let alpha_cx = alpha_rect.x
            + (state.alpha as f32 / 255.0 * (alpha_rect.width - 1) as f32).round() as i32;
        draw_marker(hdc, alpha_cx, alpha_rect.y + alpha_rect.height / 2, strip);

        draw_hex(hdc, state.layout.hex, current, colors);
        draw_button(
            hdc,
            state.layout.ok,
            "OK",
            colors,
            true,
            state.hover == Some(Region::Ok),
        );
        draw_button(
            hdc,
            state.layout.cancel,
            "Cancel",
            colors,
            false,
            state.hover == Some(Region::Cancel),
        );
    }
}

/// The marker radius scaled with the rest of the layout.
fn scaled_marker(layout: PickerLayout, base: i32) -> i32 {
    (base as f32 * layout.size.0 as f32 / (SQUARE_WIDTH + PAD * 2) as f32).round() as i32
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

unsafe fn frame(hdc: HDC, rect: Rect, color: Color) {
    // SAFETY: a one-pixel rectangle outline drawn with a temporary frame
    // brush; it is deleted before returning.
    unsafe {
        let brush = CreateSolidBrush(colorref(color));
        let win = rect_to_win(rect);
        let _ = windows::Win32::Graphics::Gdi::FrameRect(hdc, &win, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

unsafe fn draw_marker(hdc: HDC, cx: i32, cy: i32, radius: i32) {
    // SAFETY: a hollow ellipse outline drawn with stock brushes and temporary
    // pens, all restored or deleted before returning.
    unsafe {
        let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        let black = CreatePen(PS_SOLID, 2, colorref(Color::rgb(0, 0, 0)));
        let white = CreatePen(PS_SOLID, 2, colorref(Color::rgb(255, 255, 255)));
        let old_pen = SelectObject(hdc, HGDIOBJ(black.0));
        let _ = Ellipse(
            hdc,
            cx - radius - 1,
            cy - radius - 1,
            cx + radius + 1,
            cy + radius + 1,
        );
        let _ = SelectObject(hdc, HGDIOBJ(white.0));
        let _ = Ellipse(hdc, cx - radius, cy - radius, cx + radius, cy + radius);
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(black.0));
        let _ = DeleteObject(HGDIOBJ(white.0));
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
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_places_every_part_inside_the_window() {
        let layout = PickerLayout::compute(1.0);
        assert_eq!(layout.size, (280, 304));
        for rect in [
            layout.saturation,
            layout.hue,
            layout.alpha,
            layout.preview,
            layout.hex,
            layout.ok,
            layout.cancel,
        ] {
            assert!(rect.x >= 0 && rect.y >= 0);
            assert!(rect.x + rect.width <= layout.size.0);
            assert!(rect.y + rect.height <= layout.size.1);
        }
        assert!(layout.saturation.y < layout.hue.y);
        assert!(layout.hue.y < layout.alpha.y);
        assert!(layout.alpha.y < layout.preview.y);
        assert!(layout.preview.y < layout.ok.y);
    }

    #[test]
    fn layout_scales_with_dpi() {
        let one = PickerLayout::compute(1.0);
        let two = PickerLayout::compute(2.0);
        assert_eq!(two.size, (one.size.0 * 2, one.size.1 * 2));
        assert_eq!(two.ok.width, one.ok.width * 2);
    }

    #[test]
    fn region_at_maps_points_to_parts() {
        let layout = PickerLayout::compute(1.0);
        let center = |rect: Rect| Point::new(rect.x + rect.width / 2, rect.y + rect.height / 2);
        assert_eq!(
            layout.region_at(center(layout.saturation)),
            Some(Region::Saturation)
        );
        assert_eq!(layout.region_at(center(layout.hue)), Some(Region::Hue));
        assert_eq!(layout.region_at(center(layout.alpha)), Some(Region::Alpha));
        assert_eq!(layout.region_at(center(layout.ok)), Some(Region::Ok));
        assert_eq!(
            layout.region_at(center(layout.cancel)),
            Some(Region::Cancel)
        );
        assert_eq!(layout.region_at(Point::new(0, 0)), None);
    }

    #[test]
    fn saturation_value_maps_the_square_corners() {
        let rect = Rect::new(10, 20, 201, 101);
        assert_eq!(saturation_value(rect, Point::new(10, 20)), (0.0, 1.0));
        assert_eq!(saturation_value(rect, Point::new(210, 120)), (1.0, 0.0));
        // Out-of-bounds points clamp.
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
            let state = PickerState::new(color, true, PickerLayout::compute(1.0));
            assert_eq!(state.color(), color);
        }
    }
}
