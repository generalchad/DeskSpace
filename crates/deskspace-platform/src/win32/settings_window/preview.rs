//! The space preview field (`FieldKind::SpacePreview`): a styled picture of a
//! space in the settings window with an X on each item and a trailing + tile
//! (ROADMAP F-53). It is a child window of its page, painted with GDI into a
//! back buffer, and reports clicks as `SettingsEvent`s.
//!
//! Its data lives with the field in the settings window's state (found from
//! the control id, like every field control), so a refreshed form only needs
//! a repaint. The window's own state is the view: the scroll offset, the hover,
//! and the pressed target, behind `GWLP_USERDATA` and freed in `WM_NCDESTROY`.

use deskspace_core::geometry::{Point, Rect, Size};
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, AlphaBlend, BLENDFUNCTION, BitBlt, ClientToScreen,
    CreateCompatibleBitmap, CreateCompatibleDC, DT_EDITCONTROL, DT_LEFT, DeleteDC, GetStockObject,
    IntersectClipRect, NULL_BRUSH, NULL_PEN, PS_DASH, RestoreDC, SRCCOPY, SaveDC,
};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetDlgCtrlID, IDC_ARROW, IDC_HAND, LoadCursorW, SetCursor,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_SETCURSOR,
};

use super::build::scaled;
use super::commands::colorref;
use super::paint::blend;
use super::*;
use crate::ui::SpacePreview;
use crate::win32::drag_image::dib;

/// Height of the preview field, at 96 DPI.
pub(super) const PREVIEW_HEIGHT: i32 = 330;
/// The preview's metrics, at 96 DPI.
const TITLE_HEIGHT: i32 = 30;
const PADDING: i32 = 10;
const TILE_WIDTH: i32 = 88;
const TILE_HEIGHT: i32 = 92;
const ICON_SIZE: i32 = 40;
const ICON_TOP: i32 = 10;
const LABEL_GAP: i32 = 4;
const REMOVE_SIZE: i32 = 18;
const REMOVE_INSET: i32 = 3;
const RADIUS: i32 = 8;
const SCROLL_THUMB_WIDTH: i32 = 4;
/// Half the length and the thickness of the + tile's bars.
const PLUS_ARM: i32 = 9;
const PLUS_THICKNESS: i32 = 2;

/// What a point in the preview is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreviewHit {
    Item(usize),
    /// The X on an item.
    Remove(usize),
    /// The trailing + tile.
    Add,
}

/// Where the preview's parts are, for a client size, DPI, and item count.
/// Pure, so painting and hit-testing share it and it can be tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreviewLayout {
    dpi: u32,
    title: Rect,
    content: Rect,
    columns: usize,
    /// The items plus the + tile.
    tiles: usize,
}

impl PreviewLayout {
    pub(super) fn new(client: Size, dpi: u32, items: usize) -> Self {
        let s = |value| scaled(value, dpi);
        let title = Rect::new(0, 0, client.width, s(TITLE_HEIGHT));
        let content = Rect::new(
            0,
            title.height,
            client.width,
            (client.height - title.height).max(0),
        );
        let usable = (content.width - 2 * s(PADDING)).max(0);
        Self {
            dpi,
            title,
            content,
            columns: (usable / s(TILE_WIDTH)).max(1) as usize,
            tiles: items + 1,
        }
    }

    fn s(&self, value: i32) -> i32 {
        scaled(value, self.dpi)
    }

    /// Tile `index` (the last one is the + tile) in client coordinates,
    /// scrolled up by `scroll`.
    pub(super) fn tile(&self, index: usize, scroll: i32) -> Rect {
        let column = (index % self.columns) as i32;
        let row = (index / self.columns) as i32;
        Rect::new(
            self.content.x + self.s(PADDING) + column * self.s(TILE_WIDTH),
            self.content.y + self.s(PADDING) + row * self.s(TILE_HEIGHT) - scroll,
            self.s(TILE_WIDTH),
            self.s(TILE_HEIGHT),
        )
    }

    /// The X in a tile's upper-right corner.
    pub(super) fn remove_button(&self, tile: Rect) -> Rect {
        let size = self.s(REMOVE_SIZE);
        let inset = self.s(REMOVE_INSET);
        Rect::new(
            tile.x + tile.width - size - inset,
            tile.y + inset,
            size,
            size,
        )
    }

    /// The icon's square in a tile.
    fn icon(&self, tile: Rect) -> Rect {
        let size = self.s(ICON_SIZE);
        Rect::new(
            tile.x + (tile.width - size) / 2,
            tile.y + self.s(ICON_TOP),
            size,
            size,
        )
    }

    /// The label's box under the icon.
    fn label(&self, tile: Rect) -> Rect {
        let top = self.icon(tile).y + self.s(ICON_SIZE + LABEL_GAP);
        Rect::new(
            tile.x + self.s(4),
            top,
            tile.width - self.s(8),
            tile.y + tile.height - top - self.s(2),
        )
    }

    /// The height of every tile row plus the padding around them.
    fn content_height(&self) -> i32 {
        let rows = self.tiles.div_ceil(self.columns) as i32;
        rows * self.s(TILE_HEIGHT) + 2 * self.s(PADDING)
    }

    /// How far the tiles can scroll.
    pub(super) fn max_scroll(&self) -> i32 {
        (self.content_height() - self.content.height).max(0)
    }

    /// What `point` (client coordinates) is over. The title and the padding
    /// are over nothing.
    pub(super) fn hit(&self, point: Point, scroll: i32) -> Option<PreviewHit> {
        if !self.content.contains(point) {
            return None;
        }
        let last = self.tiles - 1;
        (0..self.tiles).find_map(|index| {
            let tile = self.tile(index, scroll);
            if !tile.contains(point) {
                return None;
            }
            Some(if index == last {
                PreviewHit::Add
            } else if self.remove_button(tile).contains(point) {
                PreviewHit::Remove(index)
            } else {
                PreviewHit::Item(index)
            })
        })
    }

    /// The tile a hit belongs to.
    fn tile_of(&self, hit: PreviewHit) -> usize {
        match hit {
            PreviewHit::Item(index) | PreviewHit::Remove(index) => index,
            PreviewHit::Add => self.tiles - 1,
        }
    }
}

/// The window's view state.
#[derive(Debug, Default)]
struct PreviewView {
    scroll: i32,
    hover: Option<PreviewHit>,
    pressed: Option<PreviewHit>,
    tracking: bool,
}

/// What painting and clicks need from the settings window's state.
struct FieldData {
    field: u32,
    enabled: bool,
    preview: SpacePreview,
    colors: UiColors,
    font: HFONT,
    dpi: u32,
}

/// The field this preview window shows, read from the settings window.
unsafe fn field_data(window: HWND) -> Option<FieldData> {
    // SAFETY: `window` is our live preview window; the settings window's
    // state is only read and cloned under the borrow.
    unsafe {
        let settings = GetAncestor(window, GA_ROOT);
        let index = (GetDlgCtrlID(window) as usize).checked_sub(CONTROL_BASE)? / 4;
        with_state(settings, |state| {
            let (field, _) = state.fields.get(index)?;
            let FieldKind::SpacePreview(preview) = &field.kind else {
                return None;
            };
            Some(FieldData {
                field: field.id,
                enabled: field.enabled,
                preview: preview.clone(),
                colors: state.colors,
                font: state.font,
                dpi: state.dpi,
            })
        })
        .flatten()
    }
}

unsafe fn view_of(window: HWND) -> Option<&'static mut PreviewView> {
    // SAFETY: a non-null slot is the live view (WM_NCDESTROY clears it before
    // freeing); callers keep the reference only within one message.
    unsafe { (GetWindowLongPtrW(window, GWLP_USERDATA) as *mut PreviewView).as_mut() }
}

unsafe fn layout_of(window: HWND, data: &FieldData) -> PreviewLayout {
    let mut client = RECT::default();
    // SAFETY: `window` is our live window.
    let _ = unsafe { GetClientRect(window, &mut client) };
    PreviewLayout::new(
        Size::new(client.right, client.bottom),
        data.dpi,
        data.preview.items.len(),
    )
}

pub(super) unsafe extern "system" fn preview_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`. The view
    // is only touched within one message, and the event dispatch that may
    // destroy the window is the last thing a branch does.
    unsafe {
        match message {
            WM_NCCREATE => {
                let view = Box::into_raw(Box::new(PreviewView::default()));
                SetWindowLongPtrW(window, GWLP_USERDATA, view as isize);
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                paint(window);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                on_mouse_move(window, mouse_point(lparam));
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                if let Some(view) = view_of(window) {
                    view.tracking = false;
                    view.pressed = None;
                    if view.hover.take().is_some() {
                        let _ = InvalidateRect(Some(window), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let hit = hit_at(window, mouse_point(lparam));
                if let Some(view) = view_of(window) {
                    view.pressed = hit;
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                on_click(window, mouse_point(lparam));
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
                scroll_by(window, delta);
                LRESULT(0)
            }
            WM_SETCURSOR => {
                let hand = view_of(window).is_some_and(|view| {
                    matches!(view.hover, Some(PreviewHit::Remove(_) | PreviewHit::Add))
                });
                let shape = if hand { IDC_HAND } else { IDC_ARROW };
                if let Ok(cursor) = LoadCursorW(None, shape) {
                    SetCursor(Some(cursor));
                }
                LRESULT(1)
            }
            WM_NCDESTROY => {
                let view = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut PreviewView;
                if !view.is_null() {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                    drop(Box::from_raw(view));
                }
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

/// The client point in a mouse message's `lparam` (signed, because a
/// captured pointer can be left of or above the window).
fn mouse_point(lparam: LPARAM) -> Point {
    Point::new(
        (lparam.0 & 0xFFFF) as u16 as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32,
    )
}

unsafe fn hit_at(window: HWND, point: Point) -> Option<PreviewHit> {
    // SAFETY: reads our own window and the settings state.
    unsafe {
        let data = field_data(window)?;
        if !data.enabled {
            return None;
        }
        let scroll = view_of(window).map_or(0, |view| view.scroll);
        layout_of(window, &data).hit(point, scroll)
    }
}

unsafe fn on_mouse_move(window: HWND, point: Point) {
    // SAFETY: our own window; TrackMouseEvent only arms WM_MOUSELEAVE.
    unsafe {
        let hit = hit_at(window, point);
        let Some(view) = view_of(window) else {
            return;
        };
        if !view.tracking {
            // Ask for WM_MOUSELEAVE so the hover clears when the pointer
            // leaves.
            // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-trackmouseevent
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: window,
                dwHoverTime: 0,
            };
            view.tracking = TrackMouseEvent(&mut track).is_ok();
        }
        if view.hover != hit {
            view.hover = hit;
            let _ = InvalidateRect(Some(window), None, false);
        }
    }
}

unsafe fn scroll_by(window: HWND, delta: i32) {
    // SAFETY: our own window and view.
    unsafe {
        let Some(data) = field_data(window) else {
            return;
        };
        let layout = layout_of(window, &data);
        let Some(view) = view_of(window) else {
            return;
        };
        // One wheel notch (WHEEL_DELTA, 120) scrolls half a tile row.
        let step = scaled(TILE_HEIGHT, data.dpi) / 2;
        let scroll = (view.scroll - delta * step / 120).clamp(0, layout.max_scroll());
        if scroll != view.scroll {
            view.scroll = scroll;
            view.hover = None;
            let _ = InvalidateRect(Some(window), None, false);
        }
    }
}

/// A release over the same X or + it was pressed on reports it.
unsafe fn on_click(window: HWND, point: Point) {
    // SAFETY: our own window; the dispatch is the last call, because the
    // handler may rebuild or destroy the settings window.
    unsafe {
        let hit = hit_at(window, point);
        let pressed = view_of(window).and_then(|view| view.pressed.take());
        if hit.is_none() || hit != pressed {
            return;
        }
        let Some(data) = field_data(window) else {
            return;
        };
        let field = data.field;
        match hit {
            Some(PreviewHit::Remove(index)) => {
                settings_event(SettingsEvent::PreviewRemove { field, index });
            }
            Some(PreviewHit::Add) => {
                let layout = layout_of(window, &data);
                let scroll = view_of(window).map_or(0, |view| view.scroll);
                let tile = layout.tile(layout.tile_of(PreviewHit::Add), scroll);
                let mut anchor = POINT {
                    x: tile.x,
                    y: tile.y + tile.height,
                };
                let _ = ClientToScreen(window, &mut anchor);
                settings_event(SettingsEvent::PreviewAdd {
                    field,
                    x: anchor.x,
                    y: anchor.y,
                });
            }
            _ => {}
        }
    }
}

/// Paints the preview into a back buffer, then copies it to the window.
unsafe fn paint(window: HWND) {
    // SAFETY: standard BeginPaint/EndPaint on our window; every GDI object
    // created here is selected out and deleted before returning.
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut ps);
        let mut client = RECT::default();
        let _ = GetClientRect(window, &mut client);
        let (width, height) = (client.right, client.bottom);
        if let Some(data) = field_data(window)
            && width > 0
            && height > 0
        {
            let buffer = CreateCompatibleDC(Some(dc));
            let bitmap = CreateCompatibleBitmap(dc, width, height);
            let previous = SelectObject(buffer, HGDIOBJ(bitmap.0));
            let layout = layout_of(window, &data);
            let view = view_of(window);
            let scroll = view
                .as_ref()
                .map_or(0, |view| view.scroll.min(layout.max_scroll()));
            let hover = view.and_then(|view| view.hover);
            draw(buffer, &data, &layout, scroll, hover, client);
            let _ = BitBlt(dc, 0, 0, width, height, Some(buffer), 0, 0, SRCCOPY);
            SelectObject(buffer, previous);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(buffer);
        }
        let _ = EndPaint(window, &ps);
    }
}

unsafe fn draw(
    dc: HDC,
    data: &FieldData,
    layout: &PreviewLayout,
    scroll: i32,
    hover: Option<PreviewHit>,
    client: RECT,
) {
    let s = |value| scaled(value, data.dpi);
    let preview = &data.preview;
    let dim = |color: Color| {
        if data.enabled {
            color
        } else {
            blend(color, data.colors.body, 0.5)
        }
    };
    let radius = s(RADIUS);
    // SAFETY: `dc` is the back buffer; objects are restored and deleted.
    unsafe {
        fill_rect(dc, client, data.colors.body);
        // The space: its body, then the title bar clipped to its height, so
        // both share the rounded outline.
        round_rect(dc, client, radius, Some(dim(preview.fill)), None);
        let saved = SaveDC(dc);
        let title = layout.title;
        IntersectClipRect(
            dc,
            title.x,
            title.y,
            title.x + title.width,
            title.y + title.height,
        );
        round_rect(dc, client, radius, Some(dim(preview.title_fill)), None);
        let _ = RestoreDC(dc, saved);
        SetBkMode(dc, TRANSPARENT);
        draw_text(
            dc,
            data.font,
            &preview.title,
            RECT {
                left: s(12),
                top: 0,
                right: title.width - s(12),
                bottom: title.height,
            },
            dim(preview.text),
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );

        let content = layout.content;
        let saved = SaveDC(dc);
        IntersectClipRect(
            dc,
            content.x,
            content.y,
            content.x + content.width,
            content.y + content.height - s(2),
        );
        let hovered_tile = hover.map(|hit| layout.tile_of(hit));
        for index in 0..=preview.items.len() {
            let tile = layout.tile(index, scroll);
            if !tile.intersects(content) {
                continue;
            }
            if hovered_tile == Some(index) {
                let wash = blend(preview.fill, preview.text, 0.10);
                round_rect(dc, rect_of(tile), s(6), Some(wash), None);
            }
            match preview.items.get(index) {
                Some(item) => {
                    draw_icon(dc, item.icon.as_ref(), layout.icon(tile), preview, data);
                    draw_text(
                        dc,
                        data.font,
                        &item.label,
                        rect_of(layout.label(tile)),
                        dim(preview.text),
                        DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS,
                    );
                    let remove_hovered = hover == Some(PreviewHit::Remove(index));
                    draw_remove_button(dc, layout.remove_button(tile), remove_hovered, data);
                }
                None => draw_add_tile(dc, layout, tile, data),
            }
        }
        let _ = RestoreDC(dc, saved);

        let max_scroll = layout.max_scroll();
        if max_scroll > 0 {
            let track = content.height - s(8);
            let total = track + max_scroll;
            let thumb = (track * track / total).max(s(24));
            let top = content.y + s(4) + (track - thumb) * scroll / max_scroll;
            let width = s(SCROLL_THUMB_WIDTH);
            let bar = RECT {
                left: content.width - width - s(3),
                top,
                right: content.width - s(3),
                bottom: top + thumb,
            };
            round_rect(
                dc,
                bar,
                width,
                Some(blend(preview.fill, preview.text, 0.35)),
                None,
            );
        }
        // The outline last, over everything.
        round_rect(dc, client, radius, None, Some(data.colors.divider));
    }
}

unsafe fn draw_icon(
    dc: HDC,
    icon: Option<&crate::render::Icon>,
    at: Rect,
    preview: &SpacePreview,
    data: &FieldData,
) {
    // SAFETY: the DIB and the source DC are created and deleted here.
    unsafe {
        let Some(bitmap) = icon.and_then(|icon| dib(icon, true).map(|bitmap| (icon, bitmap)))
        else {
            let placeholder = blend(preview.fill, preview.text, 0.2);
            round_rect(
                dc,
                rect_of(at),
                scaled(6, data.dpi),
                Some(placeholder),
                None,
            );
            return;
        };
        let (icon, bitmap) = bitmap;
        let source = CreateCompatibleDC(Some(dc));
        let previous = SelectObject(source, HGDIOBJ(bitmap.0));
        // Icons are premultiplied BGRA, which AlphaBlend's AC_SRC_ALPHA expects.
        // https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-alphablend
        let blend_function = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: if data.enabled { 255 } else { 128 },
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let _ = AlphaBlend(
            dc,
            at.x,
            at.y,
            at.width,
            at.height,
            source,
            0,
            0,
            icon.width() as i32,
            icon.height() as i32,
            blend_function,
        );
        SelectObject(source, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(source);
    }
}

/// The X: a small circle, red while hovered, with a cross.
unsafe fn draw_remove_button(dc: HDC, at: Rect, hovered: bool, data: &FieldData) {
    let preview = &data.preview;
    let (fill, ink) = if hovered {
        (DELETE_RED, Color::rgb(0xFF, 0xFF, 0xFF))
    } else {
        (blend(preview.fill, preview.text, 0.25), preview.text)
    };
    // SAFETY: drawing into the back buffer.
    unsafe {
        round_rect(dc, rect_of(at), at.width, Some(fill), None);
        draw_text(
            dc,
            data.font,
            "\u{2715}",
            rect_of(at),
            ink,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

/// The + tile: a dashed outline around a large plus, and a caption.
unsafe fn draw_add_tile(dc: HDC, layout: &PreviewLayout, tile: Rect, data: &FieldData) {
    let preview = &data.preview;
    let s = |value| scaled(value, data.dpi);
    let ink = blend(preview.fill, preview.text, 0.7);
    let icon = layout.icon(tile);
    // SAFETY: drawing into the back buffer; the pen is deleted below.
    unsafe {
        let pen = CreatePen(PS_DASH, 1, colorref(ink));
        let old_pen = SelectObject(dc, HGDIOBJ(pen.0));
        let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        let _ = RoundRect(
            dc,
            icon.x,
            icon.y,
            icon.x + icon.width,
            icon.y + icon.height,
            s(8),
            s(8),
        );
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        let _ = DeleteObject(HGDIOBJ(pen.0));
        // The plus as two bars, so it is crisp and in scale with the tile at
        // any DPI (a text glyph at the UI font size looked lost in it).
        let center = icon.center();
        let (half, thick) = (s(PLUS_ARM), s(PLUS_THICKNESS).max(2));
        for bar in [
            Rect::new(center.x - half, center.y - thick / 2, 2 * half, thick),
            Rect::new(center.x - thick / 2, center.y - half, thick, 2 * half),
        ] {
            fill_rect(dc, rect_of(bar), ink);
        }
        draw_text(
            dc,
            data.font,
            "New shortcut",
            rect_of(layout.label(tile)),
            ink,
            DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS,
        );
    }
}

fn rect_of(rect: Rect) -> RECT {
    RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    }
}

unsafe fn fill_rect(dc: HDC, rect: RECT, color: Color) {
    // SAFETY: the brush is deleted after the fill.
    unsafe {
        let brush = CreateSolidBrush(colorref(color));
        FillRect(dc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// A rounded rectangle filled with `fill` and outlined with `edge`; `None`
/// leaves that part out.
unsafe fn round_rect(dc: HDC, rect: RECT, radius: i32, fill: Option<Color>, edge: Option<Color>) {
    // SAFETY: created objects are selected out and deleted; stock objects
    // need no deletion.
    unsafe {
        let brush = fill.map(|color| CreateSolidBrush(colorref(color)));
        let pen = edge.map(|color| CreatePen(PS_SOLID, 1, colorref(color)));
        let old_brush = SelectObject(
            dc,
            brush.map_or(GetStockObject(NULL_BRUSH), |brush| HGDIOBJ(brush.0)),
        );
        let old_pen = SelectObject(
            dc,
            pen.map_or(GetStockObject(NULL_PEN), |pen| HGDIOBJ(pen.0)),
        );
        // A null pen draws nothing, and GDI then stops one pixel short on the
        // right and bottom, so grow the shape to cover the whole rectangle.
        let grow = i32::from(edge.is_none());
        let _ = RoundRect(
            dc,
            rect.left,
            rect.top,
            rect.right + grow,
            rect.bottom + grow,
            radius,
            radius,
        );
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        if let Some(brush) = brush {
            let _ = DeleteObject(HGDIOBJ(brush.0));
        }
        if let Some(pen) = pen {
            let _ = DeleteObject(HGDIOBJ(pen.0));
        }
    }
}

unsafe fn draw_text(
    dc: HDC,
    font: HFONT,
    text: &str,
    mut rect: RECT,
    color: Color,
    format: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    // SAFETY: the font is selected back out after drawing.
    unsafe {
        SetTextColor(dc, colorref(color));
        let previous = SelectObject(dc, HGDIOBJ(font.0));
        DrawTextW(dc, &mut wide, &mut rect, format | DT_NOPREFIX);
        SelectObject(dc, previous);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(items: usize) -> PreviewLayout {
        PreviewLayout::new(Size::new(508, PREVIEW_HEIGHT), 96, items)
    }

    #[test]
    fn tiles_fill_rows_and_end_with_the_add_tile() {
        let layout = layout(6);
        assert_eq!(layout.columns, 5, "(508 - 20) / 88");
        let first = layout.tile(0, 0);
        assert_eq!(
            (first.x, first.y),
            (PADDING, TITLE_HEIGHT + PADDING),
            "below the title, inside the padding"
        );
        let wrapped = layout.tile(5, 0);
        assert_eq!((wrapped.x, wrapped.y), (PADDING, first.y + TILE_HEIGHT));
        // Scrolling moves every tile up.
        assert_eq!(layout.tile(0, 30).y, first.y - 30);
    }

    #[test]
    fn hits_tell_items_their_x_and_the_add_tile_apart() {
        let layout = layout(2);
        let item = layout.tile(1, 0);
        assert_eq!(
            layout.hit(item.center(), 0),
            Some(PreviewHit::Item(1)),
            "the middle of a tile"
        );
        let remove = layout.remove_button(item);
        assert_eq!(layout.hit(remove.center(), 0), Some(PreviewHit::Remove(1)));
        let add = layout.tile(2, 0);
        assert_eq!(layout.hit(add.center(), 0), Some(PreviewHit::Add));
        // The title bar and empty space are over nothing.
        assert_eq!(layout.hit(Point::new(200, 5), 0), None);
        assert_eq!(layout.hit(Point::new(500, 300), 0), None);
    }

    #[test]
    fn only_overflowing_previews_scroll() {
        assert_eq!(layout(3).max_scroll(), 0);
        // 30 items + the add tile make 7 rows of 5: 7 * 92 + 20 = 664 tall,
        // in a 300-pixel content area.
        assert_eq!(
            layout(30).max_scroll(),
            664 - (PREVIEW_HEIGHT - TITLE_HEIGHT)
        );
    }
}
