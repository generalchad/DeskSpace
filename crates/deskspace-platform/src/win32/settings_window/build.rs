//! Lays out the settings form: the Builder, page and field control
//! construction, the rail list boxes, and the fonts and scaling they use.

use super::commands::show_page;
use super::nav::{NavRow, entry_row, nav_rows};
use super::update::{apply_enabled, apply_header};
use super::*;
use crate::win32::system::message_logfont;
use windows::Win32::Graphics::Gdi::CreateFontIndirectW;

/// Converts 96-DPI layout units to pixels at `dpi`.
pub(super) fn scaled(value: i32, dpi: u32) -> i32 {
    (value as i64 * dpi as i64 / 96) as i32
}

/// The UI font (Segoe UI 9pt on Windows 11) at `dpi`, a larger semibold
/// variant for section headings, and the symbol font for glyphs such as the
/// color eyedropper.
unsafe fn create_fonts(dpi: u32) -> (HFONT, HFONT, HFONT) {
    let body = message_logfont(dpi);
    let mut heading = body;
    heading.lfWeight = 600;
    heading.lfHeight = body.lfHeight * 4 / 3;
    // SAFETY: both LOGFONTs are fully initialised locals.
    let (body_font, heading_font) =
        unsafe { (CreateFontIndirectW(&body), CreateFontIndirectW(&heading)) };
    let glyph_font = crate::win32::system::glyph_font(dpi);
    (body_font, heading_font, glyph_font)
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
                    let swatch_size = self.s(26);
                    // The swatch is the button: an owner-draw control that
                    // draws the color box and an eyedropper glyph.
                    let swatch = self.control(
                        page,
                        WC_BUTTONW,
                        "",
                        BS_OWNERDRAW as u32 | tab,
                        WINDOW_EX_STYLE(0),
                        (
                            control_x,
                            y + (row - swatch_size) / 2,
                            swatch_size,
                            swatch_size,
                        ),
                        id_main,
                    );
                    controls.main = swatch;
                    controls.swatch = swatch;
                    controls.value_text = self.label(
                        page,
                        &color.to_rgb_hex(),
                        (control_x + swatch_size + self.s(12), y, self.s(96), row),
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

pub(super) unsafe fn build(hwnd: HWND, form: &SettingsForm) {
    // SAFETY: every call creates or positions children of `hwnd`; no state
    // borrow is held while controls are created.
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let (font, heading_font, glyph_font) = create_fonts(dpi);
        let builder = Builder {
            instance: HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0),
            dpi,
            font,
            heading_font,
        };
        let s = |value| builder.s(value);
        let margin = s(12);
        let page_width = s(540);

        let nav = &form.nav;

        let mut pages = Vec::new();
        let mut fields = Vec::new();
        let mut page_height = 0;
        for page in &form.pages {
            let container = CreateWindowExW(
                WS_EX_CONTROLPARENT,
                PAGE_CLASS,
                w!(""),
                WS_CHILD | WS_CLIPCHILDREN,
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

        // Lay the pages beside the rail and compute the client size.
        let (client_width, command_bar_top, list);
        let mut scopes_list = None;
        let mut dividers = Vec::new();
        let header_height = s(HEADER_HEIGHT);
        let panes_height = header_height + page_height;
        let rail = s(NAV_WIDTH);
        let mut x = margin;
        if !nav.scopes.is_empty() {
            let width = s(SCOPE_WIDTH);
            scopes_list = rail_list(&builder, hwnd, (x, margin, width, panes_height), ID_SCOPES);
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
        let button_width = s(HEADER_BUTTON_WIDTH);
        let gap = s(8);
        let switch_x = pages_x + page_width - s(16) - switch_width;
        let button_x = switch_x - gap - button_width;
        let title_x = pages_x + s(16);
        let header = nav.header.as_ref();
        let header_title = builder.label(
            hwnd,
            header.map_or("", |header| header.title.as_str()),
            (
                title_x,
                margin,
                (button_x - gap - title_x).max(s(40)),
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
        let header_button = builder.control(
            hwnd,
            WC_BUTTONW,
            "",
            BS_OWNERDRAW as u32 | WS_TABSTOP.0,
            WINDOW_EX_STYLE(0),
            (button_x, margin, button_width, header_height),
            ID_HEADER_BUTTON,
        );
        let header_switch = builder.control(
            hwnd,
            WC_BUTTONW,
            "",
            BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0,
            WINDOW_EX_STYLE(0),
            (switch_x, margin, switch_width, header_height),
            ID_HEADER_SWITCH,
        );
        apply_header(header_title, header_switch, header_button, header);

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
            state.list = list;
            state.scopes_list = scopes_list;
            state.header_title = header_title;
            state.header_switch = header_switch;
            state.header_button = header_button;
            state.dividers = dividers;
            state.apply = apply;
            state.font = font;
            state.heading_font = heading_font;
            state.glyph_font = glyph_font;
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
pub(super) unsafe fn fill_list<'a>(list: HWND, labels: impl Iterator<Item = &'a str>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_scales_with_dpi() {
        assert_eq!(scaled(100, 96), 100);
        assert_eq!(scaled(100, 144), 150);
        // The rail width and row height scale like every other layout unit.
        assert_eq!(scaled(NAV_WIDTH, 96), NAV_WIDTH);
        assert_eq!(scaled(NAV_WIDTH, 144), NAV_WIDTH * 3 / 2);
        assert_eq!(scaled(NAV_ROW_HEIGHT, 192), NAV_ROW_HEIGHT * 2);
    }
}
