//! The line above the page: the space's icon slot, the title, the "Delete
//! space" button, and the "Customize this space" switch.

use deskspace_core::geometry::Rect;
use windows::Win32::Graphics::Gdi::{GetStockObject, NULL_BRUSH};

use super::build::{Builder, scaled};
use super::commands::window_text;
use super::paint::{blend, blit_icon};
use super::*;
use crate::ui::HeaderIcon;
use crate::win32::dialog::ControlSpec;

/// The icon slot's side and the gap after it, at 96 DPI.
const ICON_SIDE: i32 = 20;
const ICON_GAP: i32 = 8;
/// Segoe Fluent Icons / Segoe MDL2 Assets "Add": the placeholder's "+".
const GLYPH_ADD: u16 = 0xE710;
/// SS_OWNERDRAW | SS_NOTIFY (WinUser.h): the slot is drawn on WM_DRAWITEM and
/// reports clicks, which S12's double-click takes.
/// https://learn.microsoft.com/en-us/windows/win32/controls/static-control-styles
const SLOT_STYLE: u32 = 0x000D | 0x0100;

/// The header's controls.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct HeaderControls {
    pub(super) title: Option<HWND>,
    pub(super) switch: Option<HWND>,
    pub(super) button: Option<HWND>,
    pub(super) icon: Option<HWND>,
}

/// Where the icon slot and the title go, in client pixels: the line's left
/// edge, top, and height, where the title must end, and the slot's side and
/// the gap after it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct HeaderLayout {
    left: i32,
    top: i32,
    height: i32,
    right: i32,
    icon: i32,
    gap: i32,
}

impl HeaderLayout {
    /// The slot's square, centred on the title's line, and the title's rect,
    /// which starts after the slot when there is one.
    fn place(self, has_icon: bool) -> (Rect, Rect) {
        let slot = Rect::new(
            self.left,
            self.top + (self.height - self.icon) / 2,
            self.icon,
            self.icon,
        );
        let title_x = if has_icon {
            self.left + self.icon + self.gap
        } else {
            self.left
        };
        let width = (self.right - title_x).max(0);
        (slot, Rect::new(title_x, self.top, width, self.height))
    }
}

/// Creates the icon slot, title, button, and switch above the page.
pub(super) unsafe fn build_header(
    builder: &Builder,
    hwnd: HWND,
    nav: &SettingsNav,
    pages_x: i32,
    page_width: i32,
    margin: i32,
) -> (HeaderControls, HeaderLayout) {
    // SAFETY: every control is a child of our live window.
    unsafe {
        let header_height = builder.s(HEADER_HEIGHT);
        let switch_width = builder.s(HEADER_SWITCH_WIDTH);
        let button_width = builder.s(HEADER_BUTTON_WIDTH);
        let gap = builder.s(8);
        let switch_x = pages_x + page_width - builder.s(16) - switch_width;
        let button_x = switch_x - gap - button_width;
        let title_x = pages_x + builder.s(16);
        let layout = HeaderLayout {
            left: title_x,
            top: margin,
            height: header_height,
            right: title_x + (button_x - gap - title_x).max(builder.s(40)),
            icon: builder.s(ICON_SIDE),
            gap: builder.s(ICON_GAP),
        };
        let header = nav.header.as_ref();
        let (slot, title_rect) = layout.place(header.is_some_and(|header| header.icon.is_some()));
        let title = builder.label(
            hwnd,
            header.map_or("", |header| header.title.as_str()),
            (
                title_rect.x,
                title_rect.y,
                title_rect.width,
                title_rect.height,
            ),
        );
        if let Some(title) = title {
            SendMessageW(
                title,
                WM_SETFONT,
                Some(WPARAM(builder.heading_font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        let icon = builder.control(
            hwnd,
            ControlSpec {
                class: WC_STATICW,
                text: "",
                style: SLOT_STYLE,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: (slot.x, slot.y, slot.width, slot.height),
                id: ID_HEADER_ICON,
            },
        );
        let button = builder.control(
            hwnd,
            ControlSpec {
                class: WC_BUTTONW,
                text: "",
                style: BS_OWNERDRAW as u32 | WS_TABSTOP.0,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: (button_x, margin, button_width, header_height),
                id: ID_HEADER_BUTTON,
            },
        );
        let switch = builder.control(
            hwnd,
            ControlSpec {
                class: WC_BUTTONW,
                text: "",
                style: BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: (switch_x, margin, switch_width, header_height),
                id: ID_HEADER_SWITCH,
            },
        );
        let controls = HeaderControls {
            title,
            switch,
            button,
            icon,
        };
        apply_header(controls, layout, header);
        (controls, layout)
    }
}

/// Shows `header` in the header controls: the icon slot and where the title
/// starts, the title, the button, and the switch, each only when the header
/// supplies one.
pub(super) unsafe fn apply_header(
    controls: HeaderControls,
    layout: HeaderLayout,
    header: Option<&SettingsHeader>,
) {
    // SAFETY: the handles are live controls of the settings window. Neither
    // SetWindowText on a static/checkbox/button nor BM_SETCHECK reports an edit.
    unsafe {
        let has_icon = header.is_some_and(|header| header.icon.is_some());
        let (slot, title_rect) = layout.place(has_icon);
        if let Some(icon) = controls.icon {
            if has_icon {
                place(icon, slot);
                let _ = ShowWindow(icon, SW_SHOW);
                let _ = InvalidateRect(Some(icon), None, false);
            } else {
                let _ = ShowWindow(icon, SW_HIDE);
            }
        }
        if let Some(title) = controls.title {
            place(title, title_rect);
            let text = header.map_or("", |header| header.title.as_str());
            if window_text(title) != text {
                let text = wide(text);
                let _ = SetWindowTextW(title, PCWSTR(text.as_ptr()));
            }
        }
        if let Some(control) = controls.button {
            match header.and_then(|header| header.button.as_ref()) {
                Some(state) => {
                    if window_text(control) != state.label {
                        let text = wide(&state.label);
                        let _ = SetWindowTextW(control, PCWSTR(text.as_ptr()));
                    }
                    let _ = ShowWindow(control, SW_SHOW);
                }
                None => {
                    let _ = ShowWindow(control, SW_HIDE);
                }
            }
        }
        if let Some(control) = controls.switch {
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

fn place(window: HWND, rect: Rect) {
    // SAFETY: `window` is a live child of the settings window.
    unsafe {
        let _ = SetWindowPos(
            window,
            None,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Draws the icon slot: the space's icon in the text color, or for a space
/// without one an outlined "+", in the text color blended halfway to the
/// body, as the desktop's placeholder is.
pub(super) unsafe fn on_draw_header_icon(window: HWND, item: &DRAWITEMSTRUCT) {
    // SAFETY: `item.hDC` is the slot's live DC during WM_DRAWITEM. The glyph
    // is drawn into the cache inside the borrow, which sends no messages;
    // the GDI drawing happens after it.
    unsafe {
        let rect = item.rcItem;
        let side = rect.right - rect.left;
        let Some((colors, body, glyph_font, dpi, shown)) = with_state(window, |state| {
            let colors = state.colors;
            let shown = match state.form.nav.header.as_ref()?.icon.as_ref()? {
                HeaderIcon::Glyph(glyph) => Some(state.glyphs.icon(
                    &glyph.key,
                    side,
                    colors.text,
                    || Some(glyph.clone()),
                )),
                HeaderIcon::Placeholder => None,
            };
            Some((colors, state.body_brush, state.glyph_font, state.dpi, shown))
        })
        .flatten() else {
            return;
        };
        FillRect(item.hDC, &rect, body);
        match shown {
            Some(Some(icon)) => blit_icon(item.hDC, &icon, rect, 255),
            // A glyph that could not be drawn leaves the slot empty.
            Some(None) => {}
            None => {
                let ink = blend(colors.body, colors.text, 0.5);
                let pen = CreatePen(PS_SOLID, 1, colorref(ink));
                let old_pen = SelectObject(item.hDC, HGDIOBJ(pen.0));
                let old_brush = SelectObject(item.hDC, GetStockObject(NULL_BRUSH));
                let radius = scaled(4, dpi);
                let _ = RoundRect(
                    item.hDC,
                    rect.left,
                    rect.top,
                    rect.right,
                    rect.bottom,
                    radius,
                    radius,
                );
                SelectObject(item.hDC, old_brush);
                SelectObject(item.hDC, old_pen);
                let _ = DeleteObject(HGDIOBJ(pen.0));
                crate::win32::system::draw_glyph(item.hDC, glyph_font, GLYPH_ADD, rect, ink);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> HeaderLayout {
        HeaderLayout {
            left: 100,
            top: 12,
            height: 44,
            right: 400,
            icon: 20,
            gap: 8,
        }
    }

    #[test]
    fn a_space_title_starts_after_its_icon_slot() {
        let (slot, title) = layout().place(true);
        assert_eq!(slot, Rect::new(100, 24, 20, 20), "centred on the line");
        assert_eq!(title, Rect::new(128, 12, 272, 44));
    }

    #[test]
    fn global_keeps_its_title_at_the_edge() {
        let (_, title) = layout().place(false);
        assert_eq!(title, Rect::new(100, 12, 300, 44));
    }
}
