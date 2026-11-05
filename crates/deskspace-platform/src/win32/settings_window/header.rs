//! The line above the page: the space's icon slot, the title, the "Delete
//! space" button, and the "Customize this space" switch.

use deskspace_core::geometry::Rect;
use windows::Win32::Graphics::Gdi::{GetStockObject, NULL_BRUSH};
use windows::Win32::UI::Controls::{
    EM_SETSEL, TOOLTIPS_CLASSW, TTF_IDISHWND, TTF_SUBCLASS, TTM_ADDTOOLW, TTS_ALWAYSTIP,
    TTS_NOPREFIX, TTTOOLINFOW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    DLGC_WANTALLKEYS, GA_ROOTOWNER, GetAncestor, IDC_HAND, IsChild, LoadCursorW, SetCursor,
    WM_CHAR, WM_GETDLGCODE, WM_KEYDOWN, WM_KILLFOCUS, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::PWSTR;

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
/// reports its double-click.
/// https://learn.microsoft.com/en-us/windows/win32/controls/static-control-styles
const SLOT_STYLE: u32 = 0x000D | SS_NOTIFY;
/// The title: a label that reports its double-click.
const TITLE_STYLE: u32 = SS_NOPREFIX | SS_CENTERIMAGE | SS_NOTIFY;
const SS_NOTIFY: u32 = 0x0100;
/// STN_DBLCLK (WinUser.h): a static with SS_NOTIFY was double-clicked.
/// https://learn.microsoft.com/en-us/windows/win32/controls/stn-dblclk
const STN_DBLCLK: u32 = 1;
/// The name's edit, at 96 DPI: its height, centred on the title's line.
const EDIT_HEIGHT: i32 = 28;
/// The subclass's id on the name's edit (`SetWindowSubclass`).
const EDIT_SUBCLASS: usize = 1;
const ICON_TIP: &str = "Double-click to change the icon";

/// The header's controls.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct HeaderControls {
    pub(super) title: Option<HWND>,
    pub(super) switch: Option<HWND>,
    pub(super) button: Option<HWND>,
    pub(super) icon: Option<HWND>,
    /// The name's in-place edit, hidden until a double-click on the title.
    pub(super) edit: Option<HWND>,
    /// The icon slot's tooltip, owned by the settings window.
    pub(super) tooltip: Option<HWND>,
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
        let title = builder.control(
            hwnd,
            ControlSpec {
                class: WC_STATICW,
                text: header.map_or("", |header| header.title.as_str()),
                style: TITLE_STYLE,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: (
                    title_rect.x,
                    title_rect.y,
                    title_rect.width,
                    title_rect.height,
                ),
                id: ID_HEADER_TITLE,
            },
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
        let edit = builder.control(
            hwnd,
            ControlSpec {
                class: WC_EDITW,
                text: "",
                style: ES_AUTOHSCROLL as u32,
                ex_style: WS_EX_CLIENTEDGE,
                bounds: (
                    title_rect.x,
                    title_rect.y,
                    title_rect.width,
                    title_rect.height,
                ),
                id: ID_HEADER_EDIT,
            },
        );
        if let Some(edit) = edit {
            let _ = ShowWindow(edit, SW_HIDE);
            SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(builder.heading_font.0 as usize)),
                Some(LPARAM(1)),
            );
            let _ = SetWindowSubclass(edit, Some(edit_subclass), EDIT_SUBCLASS, 0);
        }
        let tooltip = icon.and_then(|icon| icon_tooltip(hwnd, icon));
        let controls = HeaderControls {
            title,
            switch,
            button,
            icon,
            edit,
            tooltip,
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

/// The icon slot's tooltip, owned by the settings window, so it closes with
/// it. The tool subclasses the slot, so it shows and hides by itself.
unsafe fn icon_tooltip(settings: HWND, icon: HWND) -> Option<HWND> {
    // SAFETY: creating a tooltip owned by our live window; TTM_ADDTOOLW
    // copies the tool and its text, which outlives the call.
    unsafe {
        let tooltip = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            TOOLTIPS_CLASSW,
            PCWSTR::null(),
            WINDOW_STYLE(WS_POPUP.0 | TTS_ALWAYSTIP | TTS_NOPREFIX),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            Some(settings),
            None,
            None,
            None,
        )
        .ok()?;
        let mut text = wide(ICON_TIP);
        let tool = TTTOOLINFOW {
            cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
            uFlags: TTF_IDISHWND | TTF_SUBCLASS,
            hwnd: settings,
            uId: icon.0 as usize,
            lpszText: PWSTR(text.as_mut_ptr()),
            ..Default::default()
        };
        SendMessageW(
            tooltip,
            TTM_ADDTOOLW,
            None,
            Some(LPARAM(&tool as *const TTTOOLINFOW as isize)),
        );
        Some(tooltip)
    }
}

/// Gives the icon slot's tooltip the dark or light style; it is not a child,
/// so `theme_children` does not reach it.
pub(super) fn theme_tooltip(controls: HeaderControls, dark: bool) {
    if let Some(tooltip) = controls.tooltip {
        crate::win32::dark_mode::theme_control(
            tooltip,
            crate::win32::dark_mode::ControlTheme::Explorer,
            dark,
        );
    }
}

/// A notification from the title: a double-click opens the name's edit on a
/// space's page.
pub(super) unsafe fn on_title_command(window: HWND, code: u32) {
    if code != STN_DBLCLK {
        return;
    }
    // SAFETY: the borrow copies handles out; the edit is shown after it.
    unsafe {
        let Some((controls, layout, header, dpi)) = with_state(window, |state| {
            (
                state.header,
                state.header_layout,
                state.form.nav.header.clone(),
                state.dpi,
            )
        }) else {
            return;
        };
        let Some(header) = header.filter(|header| header.renamable) else {
            return;
        };
        let (Some(edit), Some(title)) = (controls.edit, controls.title) else {
            return;
        };
        let (_, title_rect) = layout.place(header.icon.is_some());
        let height = scaled(EDIT_HEIGHT, dpi).min(title_rect.height);
        let rect = Rect::new(
            title_rect.x,
            title_rect.y + (title_rect.height - height) / 2,
            title_rect.width,
            height,
        );
        with_state(window, |state| state.renaming = true);
        let text = wide(&header.title);
        let _ = SetWindowTextW(edit, PCWSTR(text.as_ptr()));
        place(edit, rect);
        let _ = ShowWindow(title, SW_HIDE);
        let _ = ShowWindow(edit, SW_SHOW);
        let _ = SetFocus(Some(edit));
        SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
    }
}

/// A notification from the icon slot: a double-click asks for the picker.
pub(super) unsafe fn on_icon_command(code: u32) {
    if code == STN_DBLCLK {
        settings_event(SettingsEvent::HeaderIconPressed);
    }
}

/// Ends the name's edit: shows the title again and, when `commit`, reports
/// the typed name last. Does nothing when no edit is open, so the focus
/// moving away while it closes cannot end it twice.
unsafe fn close_rename(window: HWND, commit: bool) {
    // SAFETY: the borrow only flips the flag and copies handles out.
    unsafe {
        let Some((controls, was_open)) = with_state(window, |state| {
            (state.header, std::mem::replace(&mut state.renaming, false))
        }) else {
            return;
        };
        if !was_open {
            return;
        }
        let (Some(edit), Some(title)) = (controls.edit, controls.title) else {
            return;
        };
        let typed = window_text(edit);
        // A hidden control keeps the focus, so hand it to the window first.
        if GetFocus() == edit {
            let _ = SetFocus(Some(window));
        }
        let _ = ShowWindow(edit, SW_HIDE);
        let _ = ShowWindow(title, SW_SHOW);
        if commit {
            settings_event(SettingsEvent::HeaderRenamed { title: typed });
        }
    }
}

/// While the name's edit is open, activating the window gives it the focus
/// back, as after the icon picker closes. Returns whether it did.
pub(super) unsafe fn refocus_rename(window: HWND) -> bool {
    // SAFETY: the borrow copies the handle out.
    unsafe {
        let edit =
            with_state(window, |state| state.header.edit.filter(|_| state.renaming)).flatten();
        match edit {
            Some(edit) => {
                let _ = SetFocus(Some(edit));
                true
            }
            None => false,
        }
    }
}

/// A hand over the icon slot, which a double-click opens the picker from.
/// Returns whether `over` was the slot.
pub(super) unsafe fn set_icon_cursor(window: HWND, over: HWND) -> bool {
    // SAFETY: the borrow copies the handle out; the cursor is a shared one.
    unsafe {
        if with_state(window, |state| state.header.icon) != Some(Some(over)) {
            return false;
        }
        if let Ok(hand) = LoadCursorW(None, IDC_HAND) {
            SetCursor(Some(hand));
        }
        true
    }
}

/// The name's edit: Enter keeps the name and Esc drops it, both kept from the
/// dialog manager, which would press OK or Cancel; the focus leaving keeps
/// it, except to the icon picker, which opens over an open edit.
unsafe extern "system" fn edit_subclass(
    edit: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: comctl32 calls this for the live edit, whose parent is the
    // settings window; each branch ends with the dispatch, if any.
    unsafe {
        let window = GetParent(edit).unwrap_or_default();
        match message {
            WM_GETDLGCODE => LRESULT(DLGC_WANTALLKEYS as isize),
            WM_KEYDOWN if wparam.0 == VK_RETURN.0 as usize => {
                close_rename(window, true);
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
                close_rename(window, false);
                LRESULT(0)
            }
            // The edit would beep at the Enter and Esc characters.
            WM_CHAR if wparam.0 == 0x0D || wparam.0 == 0x1B => LRESULT(0),
            WM_KILLFOCUS => {
                let result = DefSubclassProc(edit, message, wparam, lparam);
                let to = HWND(wparam.0 as *mut core::ffi::c_void);
                if !keeps_edit_open(window, to) {
                    close_rename(window, true);
                }
                result
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(edit, Some(edit_subclass), id);
                DefSubclassProc(edit, message, wparam, lparam)
            }
            _ => DefSubclassProc(edit, message, wparam, lparam),
        }
    }
}

/// Whether the focus moving to `to` leaves the name's edit open: it went to
/// a window the settings window owns but does not contain, the icon picker.
unsafe fn keeps_edit_open(window: HWND, to: HWND) -> bool {
    // SAFETY: both reads take any handle, live or not.
    unsafe {
        !to.is_invalid()
            && !IsChild(window, to).as_bool()
            && to != window
            && GetAncestor(to, GA_ROOTOWNER) == window
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
