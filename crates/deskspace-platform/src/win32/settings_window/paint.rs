//! Owner-draw painting for the settings window: the body, command bar,
//! rail rows, scopes, and the destructive header button.

use super::build::scaled;
use super::commands::{colorref, window_text};
use super::nav::NavRow;
use super::*;
use deskspace_core::geometry::Rect;

/// Paints one row of the scopes pane like a rail entry: an accent bar when
/// selected, indented rows moved right, and dimmed rows (scopes that only
/// inherit) in disabled text unless selected. A row's kind glyph sits before
/// its label, in the same color.
pub(super) unsafe fn on_draw_scope(window: HWND, item: &DRAWITEMSTRUCT) {
    // SAFETY: `item.hDC` is a live DC owned by the list box during
    // WM_DRAWITEM; the borrowed data is cloned out before any GDI call.
    unsafe {
        let Some((colors, body, accent, font, glyph_font, dpi, row)) =
            with_state(window, |state| {
                (
                    state.colors,
                    state.body_brush,
                    state.accent_brush,
                    state.font,
                    state.glyph_font,
                    state.dpi,
                    state.scope_rows.get(item.itemID as usize).cloned(),
                )
            })
        else {
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
        let previous = SelectObject(item.hDC, HGDIOBJ(font.0));
        let mut left = item.rcItem.left
            + scaled(12, dpi)
            + if row.indent {
                scaled(SCOPE_INDENT, dpi)
            } else {
                0
            };
        let glyph = row
            .glyph
            .filter(|_| !glyph_font.is_invalid())
            .and_then(|glyph| u16::try_from(u32::from(glyph)).ok());
        if let Some(glyph) = glyph {
            let mut metrics = TEXTMETRICW::default();
            let _ = GetTextMetricsW(item.hDC, &mut metrics);
            let side = metrics.tmHeight.max(0);
            if side > 0 {
                let top = item.rcItem.top + (item.rcItem.bottom - item.rcItem.top - side) / 2;
                let square = RECT {
                    left,
                    top,
                    right: left + side,
                    bottom: top + side,
                };
                crate::win32::system::draw_glyph(item.hDC, glyph_font, glyph, square, text);
                left = square.right + scaled(SCOPE_GLYPH_GAP, dpi);
            }
        }
        SetTextColor(item.hDC, colorref(text));
        let mut label: Vec<u16> = row.label.encode_utf16().collect();
        let mut rect = item.rcItem;
        rect.left = left;
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
pub(super) unsafe fn on_draw_item(window: HWND, item: &DRAWITEMSTRUCT) {
    // SAFETY: `item.hDC` is a live DC owned by the list box during
    // WM_DRAWITEM; the borrowed data is cloned out before any GDI call.
    unsafe {
        let Some((colors, body, accent, font, strong, dpi, row)) = with_state(window, |state| {
            Some((
                state.colors,
                state.body_brush,
                state.accent_brush,
                state.font,
                state.strong_font,
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
        // A group title is semibold in the primary text color, and the
        // pages under it are indented, so it reads as a section header
        // rather than one more page.
        let (text, row_font, indent) = match &row {
            NavRow::Title(_) => (colors.text, strong, 0),
            NavRow::Entry { nested, .. } => (
                if selected {
                    colors.text
                } else {
                    colors.secondary_text
                },
                font,
                if *nested { scaled(12, dpi) } else { 0 },
            ),
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
        let previous = SelectObject(item.hDC, HGDIOBJ(row_font.0));
        let mut label: Vec<u16> = row.label().encode_utf16().collect();
        let mut rect = item.rcItem;
        rect.left += inset + indent;
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

pub(super) unsafe fn on_paint(window: HWND) {
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
pub(super) unsafe fn erase_body(settings: HWND, window: HWND, dc: HDC) -> bool {
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

/// Paints the header action button (a space's "Delete space") as an owner-draw
/// control: a slight red wash with a red edge, so the destructive action stands
/// out while the text stays readable in both color modes.
pub(super) unsafe fn on_draw_header_button(window: HWND, item: &DRAWITEMSTRUCT) {
    // SAFETY: `item.hDC` is a live DC owned by the button during WM_DRAWITEM;
    // the borrowed state is copied out before any GDI call.
    unsafe {
        let Some((colors, font, dpi)) =
            with_state(window, |state| (state.colors, state.font, state.dpi))
        else {
            return;
        };
        let disabled = item.itemState.0 & ODS_DISABLED.0 != 0;
        let pressed = item.itemState.0 & ODS_SELECTED.0 != 0;
        // A deeper wash while pressed; the base is the dialog body, so the tint
        // reads as a shade of red in both light and dark mode.
        let amount = if pressed { 0.40 } else { 0.20 };
        let fill = blend(colors.body, DELETE_RED, amount);
        let brush = CreateSolidBrush(colorref(fill));
        let pen = CreatePen(PS_SOLID, 1, colorref(DELETE_RED));
        let old_pen = SelectObject(item.hDC, HGDIOBJ(pen.0));
        let old_brush = SelectObject(item.hDC, HGDIOBJ(brush.0));
        let radius = scaled(8, dpi);
        let _ = RoundRect(
            item.hDC,
            item.rcItem.left,
            item.rcItem.top,
            item.rcItem.right,
            item.rcItem.bottom,
            radius,
            radius,
        );
        SelectObject(item.hDC, old_brush);
        SelectObject(item.hDC, old_pen);
        let _ = DeleteObject(HGDIOBJ(brush.0));
        let _ = DeleteObject(HGDIOBJ(pen.0));

        let label = window_text(item.hwndItem);
        SetBkMode(item.hDC, TRANSPARENT);
        let text = if disabled {
            colors.disabled_text
        } else {
            colors.text
        };
        SetTextColor(item.hDC, colorref(text));
        let previous = SelectObject(item.hDC, HGDIOBJ(font.0));
        let mut wide: Vec<u16> = label.encode_utf16().collect();
        let mut rect = item.rcItem;
        DrawTextW(
            item.hDC,
            &mut wide,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(item.hDC, previous);
        if item.itemState.0 & ODS_FOCUS.0 != 0 {
            let inset = scaled(3, dpi);
            let focus = RECT {
                left: item.rcItem.left + inset,
                top: item.rcItem.top + inset,
                right: item.rcItem.right - inset,
                bottom: item.rcItem.bottom - inset,
            };
            let _ = DrawFocusRect(item.hDC, &focus);
        }
    }
}
/// Paints a color field's control: the color as a rounded box with an
/// eyedropper glyph. It replaces the old flat swatch and "Choose…" button, so
/// clicking the swatch itself opens the picker.
pub(super) unsafe fn on_draw_swatch(window: HWND, item: &DRAWITEMSTRUCT) {
    // SAFETY: `item.hDC` is a live DC owned by the button during WM_DRAWITEM;
    // the borrowed state is copied out before any GDI call.
    unsafe {
        let id = item.CtlID as usize;
        if id < CONTROL_BASE {
            return;
        }
        let index = (id - CONTROL_BASE) / 4;
        let Some((color, colors, body, dpi, glyph_font)) = with_state(window, |state| {
            let (field, _) = state.fields.get(index)?;
            let color = match &field.kind {
                FieldKind::Color { color } => *color,
                _ => return None,
            };
            Some((
                color,
                state.colors,
                state.body_brush,
                state.dpi,
                state.glyph_font,
            ))
        })
        .flatten() else {
            return;
        };
        let disabled = item.itemState.0 & ODS_DISABLED.0 != 0;
        // The button is rectangular; clear it to the body so the rounded fill's
        // corners do not show the control's default background.
        FillRect(item.hDC, &item.rcItem, body);
        let rect = Rect::new(
            item.rcItem.left,
            item.rcItem.top,
            item.rcItem.right - item.rcItem.left,
            item.rcItem.bottom - item.rcItem.top,
        );
        let fill = if disabled {
            blend(color, colors.body, 0.5)
        } else {
            color
        };
        crate::win32::color_picker::draw_color_box(
            item.hDC,
            rect,
            fill,
            scaled(SWATCH_RADIUS, dpi),
            colors.divider,
        );
        let ink = if disabled {
            colors.disabled_text
        } else if fill.luminance() < 0.5 {
            Color::rgb(255, 255, 255)
        } else {
            Color::rgb(0, 0, 0)
        };
        crate::win32::system::draw_glyph(item.hDC, glyph_font, GLYPH_EYEDROPPER, item.rcItem, ink);
        if item.itemState.0 & ODS_FOCUS.0 != 0 {
            let inset = scaled(2, dpi);
            let focus = RECT {
                left: item.rcItem.left + inset,
                top: item.rcItem.top + inset,
                right: item.rcItem.right - inset,
                bottom: item.rcItem.bottom - inset,
            };
            let _ = DrawFocusRect(item.hDC, &focus);
        }
    }
}

/// Mixes `amount` of `over` into `base` (0 = base, 1 = over).
pub(super) fn blend(base: Color, over: Color, amount: f32) -> Color {
    let mix = |a: u8, b: u8| (a as f32 * (1.0 - amount) + b as f32 * amount).round() as u8;
    Color::rgb(
        mix(base.r, over.r),
        mix(base.g, over.g),
        mix(base.b, over.b),
    )
}
