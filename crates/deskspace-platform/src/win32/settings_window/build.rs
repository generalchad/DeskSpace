//! Lays out the settings form: the Builder, page and field control
//! construction, the rail list boxes, and the fonts and scaling they use.

use super::commands::show_page;
use super::nav::{NavRow, entry_row, nav_rows};
use super::update::{apply_enabled, apply_header};
use super::*;
use crate::win32::dialog::{self, ControlSpec};
use crate::win32::system::message_logfont;
use windows::Win32::Graphics::Gdi::CreateFontIndirectW;

/// Converts 96-DPI layout units to pixels at `dpi`.
pub(super) fn scaled(value: i32, dpi: u32) -> i32 {
    (value as i64 * dpi as i64 / 96) as i32
}

/// Field layout units, at 96 DPI, scaled through `Builder::s`.
pub(super) const EDGE: i32 = 16;
pub(super) const LABEL_WIDTH: i32 = 184;
pub(super) const LABEL_GAP: i32 = 12;
pub(super) const ROW_HEIGHT: i32 = 28;
pub(super) const HEADING_GAP: i32 = 10;
pub(super) const HEADING_HEIGHT: i32 = 28;
pub(super) const HEADING_ADVANCE: i32 = 34;
pub(super) const NOTE_GAP: i32 = 10;
pub(super) const COMBO_HEIGHT: i32 = 240;
pub(super) const CHOICE_ADVANCE: i32 = 38;
pub(super) const TOGGLE_ADVANCE: i32 = 34;
pub(super) const VALUE_WIDTH: i32 = 52;
pub(super) const SLIDER_INSET: i32 = 6;
pub(super) const VALUE_GAP: i32 = 4;
pub(super) const VALUE_EDIT_TOP: i32 = 2;
pub(super) const VALUE_EDIT_HEIGHT: i32 = 24;
pub(super) const SLIDER_ADVANCE: i32 = 38;
pub(super) const MARKS_GAP: i32 = 14;
pub(super) const SWATCH_SIZE: i32 = 26;
pub(super) const SWATCH_GAP: i32 = 12;
pub(super) const COLOR_VALUE_WIDTH: i32 = 96;
pub(super) const COLOR_ADVANCE: i32 = 38;
pub(super) const TEXT_EDIT_TOP: i32 = 2;
pub(super) const TEXT_EDIT_HEIGHT: i32 = 24;
pub(super) const TEXT_ADVANCE: i32 = 38;
pub(super) const BUTTON_WIDTH: i32 = 180;
pub(super) const BUTTON_HEIGHT: i32 = 30;
pub(super) const BUTTON_ADVANCE: i32 = 40;
pub(super) const PREVIEW_GAP: i32 = 10;

/// The window's fonts at `dpi`.
struct Fonts {
    /// The UI font (Segoe UI 9pt on Windows 11).
    body: HFONT,
    /// A larger semibold variant for section headings.
    heading: HFONT,
    /// The UI font in semibold, for the rail's group titles.
    strong: HFONT,
    /// The symbol font for glyphs such as the color eyedropper.
    glyph: HFONT,
}

unsafe fn create_fonts(dpi: u32) -> Fonts {
    let body = message_logfont(dpi);
    let mut strong = body;
    strong.lfWeight = 600;
    let mut heading = strong;
    heading.lfHeight = body.lfHeight * 4 / 3;
    // SAFETY: the LOGFONTs are fully initialised locals.
    unsafe {
        Fonts {
            body: CreateFontIndirectW(&body),
            heading: CreateFontIndirectW(&heading),
            strong: CreateFontIndirectW(&strong),
            glyph: crate::win32::system::glyph_font(dpi),
        }
    }
}

/// One field row's shared geometry, ids, and controls.
pub(super) struct FieldRow<'a> {
    pub(super) page: HWND,
    pub(super) field: &'a Field,
    pub(super) y: i32,
    pub(super) x: i32,
    pub(super) full: i32,
    pub(super) label_width: i32,
    pub(super) control_x: i32,
    pub(super) control_width: i32,
    pub(super) row_height: i32,
    pub(super) id_main: usize,
    pub(super) id_secondary: usize,
    pub(super) tab: u32,
    pub(super) controls: FieldControls,
}

pub(super) struct Builder {
    dpi: u32,
    font: HFONT,
    pub(super) heading_font: HFONT,
}

impl Builder {
    pub(super) fn s(&self, value: i32) -> i32 {
        scaled(value, self.dpi)
    }

    pub(super) unsafe fn control(&self, parent: HWND, spec: ControlSpec<'_>) -> Option<HWND> {
        // SAFETY: creating a child control of our own window.
        unsafe { dialog::control(self.font, parent, spec) }
    }

    pub(super) unsafe fn label(
        &self,
        parent: HWND,
        text: &str,
        bounds: (i32, i32, i32, i32),
    ) -> Option<HWND> {
        // SAFETY: see `control`.
        unsafe {
            self.control(
                parent,
                ControlSpec {
                    class: WC_STATICW,
                    text,
                    style: SS_NOPREFIX | SS_CENTERIMAGE,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds,
                    id: 0,
                },
            )
        }
    }

    /// Adds tick marks to `slider` and a label under each. Each label is
    /// centered on the thumb position for its value, which the trackbar
    /// reports exactly (TBM_GETTHUMBRECT) rather than this code guessing its
    /// channel insets, and is only as wide as the space to its neighbors so
    /// labels never overlap. Labels at the ends of the range are aligned
    /// inward so they stay inside the page.
    pub(super) unsafe fn slider_marks(
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
                    ControlSpec {
                        class: WC_STATICW,
                        text: &mark.label,
                        style: SS_NOPREFIX | align,
                        ex_style: WINDOW_EX_STYLE(0),
                        bounds: (x, top, width, self.s(MARK_LABEL_HEIGHT)),
                        id: 0,
                    },
                )
            });
        }
        labels
    }
    pub(super) unsafe fn text_height(&self, parent: HWND, text: &str, width: i32) -> i32 {
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
        let x = self.s(EDGE);
        let label_width = self.s(LABEL_WIDTH);
        let control_x = x + label_width + self.s(LABEL_GAP);
        let mut row = FieldRow {
            page,
            field,
            y,
            x,
            full: width - 2 * x,
            label_width,
            control_x,
            control_width: width - control_x - x,
            row_height: self.s(ROW_HEIGHT),
            id_main: CONTROL_BASE + index * 4 + PART_MAIN,
            id_secondary: CONTROL_BASE + index * 4 + PART_SECONDARY,
            tab: WS_TABSTOP.0,
            controls: FieldControls::default(),
        };

        // SAFETY: all controls are children of our page window.
        let height = unsafe {
            match &field.kind {
                FieldKind::Heading => self.heading_field(&mut row),
                FieldKind::Note => self.note_field(&mut row),
                FieldKind::Choice { .. } => self.choice_field(&mut row),
                FieldKind::Toggle { .. } => self.toggle_field(&mut row),
                FieldKind::Slider { .. } => self.slider_field(&mut row),
                FieldKind::Color { .. } => self.color_field(&mut row),
                FieldKind::Text { .. } => self.text_field(&mut row),
                FieldKind::Button { .. } => self.button_field(&mut row),
                FieldKind::SpacePreview(_) => self.space_preview_field(&mut row),
            }
        };
        (row.controls, height)
    }
}

/// Creates every control of the settings window. Fails, leaving the caller to
/// destroy `hwnd`, if a page container cannot be created: the navigation and
/// the field diff in `update` index pages and fields by their position in
/// `form`, so a skipped page would shift every page after it.
pub(super) unsafe fn build(hwnd: HWND, form: &SettingsForm) -> Result<(), PlatformError> {
    // SAFETY: every call creates or positions children of `hwnd`; no state
    // borrow is held while controls are created.
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let dpi = GetDpiForWindow(hwnd).max(96);
        let page_width = scaled(540, dpi);
        let pages = create_pages(hwnd, instance, form.pages.len(), page_width)?;

        let fonts = create_fonts(dpi);
        let builder = Builder {
            dpi,
            font: fonts.body,
            heading_font: fonts.heading,
        };
        let s = |value| builder.s(value);
        let margin = s(12);

        let nav = &form.nav;

        let mut fields = Vec::new();
        let mut page_height = 0;
        for (&container, page) in pages.iter().zip(&form.pages) {
            let mut y = s(16);
            for field in &page.fields {
                let index = fields.len();
                let (controls, height) = builder.field(container, index, field, y, page_width);
                apply_enabled(&controls, field.enabled);
                fields.push((field.clone(), controls));
                y += height;
            }
            page_height = page_height.max(y + s(8));
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
            ControlSpec {
                class: WC_BUTTONW,
                text: "",
                style: BS_OWNERDRAW as u32 | WS_TABSTOP.0,
                ex_style: WINDOW_EX_STYLE(0),
                bounds: (button_x, margin, button_width, header_height),
                id: ID_HEADER_BUTTON,
            },
        );
        let header_switch = builder.control(
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
                ControlSpec {
                    class: WC_BUTTONW,
                    text: caption,
                    style,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (x, button_y, button_width, button_height),
                    id,
                },
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
            state.font = fonts.body;
            state.heading_font = fonts.heading;
            state.strong_font = fonts.strong;
            state.glyph_font = fonts.glyph;
            state.dpi = dpi;
            state.command_bar_top = command_bar_top;
            state.updating = false;
        });
        Ok(())
    }
}

/// Creates one hidden page container per form page, in form order. They are
/// sized and positioned once the tallest page is known.
unsafe fn create_pages(
    parent: HWND,
    instance: HINSTANCE,
    count: usize,
    page_width: i32,
) -> Result<Vec<HWND>, PlatformError> {
    (0..count)
        .map(|_| {
            // SAFETY: creating a child window of our own live window.
            let page = unsafe {
                CreateWindowExW(
                    WS_EX_CONTROLPARENT,
                    PAGE_CLASS,
                    w!(""),
                    WS_CHILD | WS_CLIPCHILDREN,
                    0,
                    0,
                    page_width,
                    0,
                    Some(parent),
                    None,
                    Some(instance),
                    None,
                )
            };
            Ok(page?)
        })
        .collect()
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
            ControlSpec {
                class: WC_LISTBOXW,
                text: "",
                style: (LBS_OWNERDRAWFIXED | LBS_NOTIFY | LBS_HASSTRINGS) as u32
                    | WS_VSCROLL.0
                    | WS_TABSTOP.0,
                ex_style: WINDOW_EX_STYLE(0),
                bounds,
                id,
            },
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
