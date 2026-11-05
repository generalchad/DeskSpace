//! Lays out the settings form: the Builder, page and field control
//! construction, the rail list boxes, and the fonts and scaling they use.

use super::cards::PageCards;
use super::commands::show_page;
use super::header::build_header;
use super::nav::{NavRow, entry_row, fill_list, nav_rows, rail_list, watch_title_clicks};
use super::scroll::set_bar;
use super::title::sync_title;
use super::update::apply_enabled;
use super::*;
use crate::page_scroll::{PageScroll, view_height};
use crate::settings_cards::{
    CARD_GAP, CARD_PADDING_X, CARD_PADDING_Y, DESCRIPTION_GAP, ROW_SEPARATOR_GAP, Role, card_plan,
};
use crate::win32::dialog::{self, ControlSpec};
use crate::win32::system::draw_text;
use deskspace_core::geometry::Rect;
use deskspace_core::typography::Role as FontRole;
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::SM_CXVSCROLL;

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
    let body = crate::win32::system::font_for(FontRole::Body, dpi);
    let strong = crate::win32::system::font_for(FontRole::Heading3, dpi);
    let heading = crate::win32::system::font_for(FontRole::Heading1, dpi);
    Fonts {
        body,
        heading,
        strong,
        glyph: crate::win32::system::glyph_font(dpi),
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
            draw_text(
                dc,
                text,
                &mut bounds,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
            );
            SelectObject(dc, previous);
            ReleaseDC(Some(parent), dc);
        }
        bounds.bottom - bounds.top
    }

    /// Creates the controls for one field at `y` inside `page` and returns the
    /// controls and the height used. `inset` moves the row in from the page's
    /// edge on both sides, as a card's padding does.
    unsafe fn field(
        &self,
        page: HWND,
        index: usize,
        field: &Field,
        y: i32,
        width: i32,
        inset: i32,
    ) -> (FieldControls, i32) {
        let x = self.s(EDGE + inset);
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

/// The pages and fields built for a form, and each page's height.
struct PageBuild {
    pages: Vec<HWND>,
    fields: Vec<(Field, FieldControls)>,
    /// Each field's card role, by field index.
    roles: Vec<Role>,
    /// Each page's cards and hairlines, by page index.
    cards: Vec<PageCards>,
    /// Each page's content height, by page index.
    heights: Vec<i32>,
}

/// The least height of the page area, at 96 DPI, however short the screen.
const MIN_PAGE_VIEW: i32 = 240;

/// The rail, scopes pane, and page area laid out beside each other.
struct Panes {
    client_width: i32,
    command_bar_top: i32,
    pages_x: i32,
    header_height: i32,
    list: Option<HWND>,
    scopes_list: Option<HWND>,
    dividers: Vec<(i32, i32, i32)>,
    nav_rows: Vec<NavRow>,
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

        let fonts = create_fonts(dpi);
        let builder = Builder {
            dpi,
            font: fonts.body,
            heading_font: fonts.heading,
        };
        let margin = builder.s(12);

        let PageBuild {
            pages,
            fields,
            roles,
            cards,
            heights,
        } = build_pages(&builder, hwnd, form, instance, page_width)?;
        // Every page area is as tall as the tallest page, unless the screen
        // is shorter; then the taller pages scroll.
        let tallest = heights.iter().copied().max().unwrap_or(0);
        let view = view_height(
            tallest,
            available_page_height(&builder, dpi, margin),
            builder.s(MIN_PAGE_VIEW),
        );
        let scrolls: Vec<PageScroll> = heights
            .iter()
            .map(|height| PageScroll::new(*height, view))
            .collect();
        // A page that scrolls is widened by its scroll bar, so its controls
        // keep their width, and the window by as much.
        let bar = if scrolls.iter().any(|scroll| scroll.overflows()) {
            GetSystemMetricsForDpi(SM_CXVSCROLL, dpi)
        } else {
            0
        };
        let mut panes = build_panes(&builder, hwnd, &form.nav, view, margin, page_width);
        panes.client_width += bar;
        let (header, header_layout) =
            build_header(&builder, hwnd, &form.nav, panes.pages_x, page_width, margin);

        for (page, scroll) in pages.iter().zip(&scrolls) {
            let width = page_width + if scroll.overflows() { bar } else { 0 };
            let _ = SetWindowPos(
                *page,
                Some(HWND_TOP),
                panes.pages_x,
                margin + panes.header_height,
                width,
                view,
                SWP_NOACTIVATE,
            );
            set_bar(*page, *scroll);
        }
        show_page(
            hwnd,
            &pages,
            form.nav.page_of(form.nav.selected).unwrap_or(0),
        );
        let scope_rows = form.nav.scopes.clone();
        with_state(hwnd, |state| {
            state.nav_rows = panes.nav_rows;
            state.scope_rows = scope_rows;
        });
        let (apply, client_height) = build_command_bar(
            &builder,
            hwnd,
            panes.client_width,
            panes.command_bar_top,
            margin,
            form.apply_enabled,
        );
        place_window(hwnd, dpi, panes.client_width, client_height);

        with_state(hwnd, |state| {
            state.fields = fields;
            state.roles = roles;
            state.cards = cards;
            state.pages = pages;
            state.scrolls = scrolls;
            state.list = panes.list;
            state.scopes_list = panes.scopes_list;
            state.header = header;
            state.header_layout = header_layout;
            state.dividers = panes.dividers;
            state.apply = apply;
            state.font = fonts.body;
            state.heading_font = fonts.heading;
            state.strong_font = fonts.strong;
            state.glyph_font = fonts.glyph;
            state.dpi = dpi;
            state.command_bar_top = panes.command_bar_top;
            state.updating = false;
        });
        sync_title(hwnd);
        Ok(())
    }
}

/// Creates the page containers and every field's controls, laying each page's
/// fields out on their cards.
unsafe fn build_pages(
    builder: &Builder,
    hwnd: HWND,
    form: &SettingsForm,
    instance: HINSTANCE,
    page_width: i32,
) -> Result<PageBuild, PlatformError> {
    // SAFETY: every control is a child of one of our live page windows.
    unsafe {
        let pages = create_pages(hwnd, instance, form.pages.len(), page_width)?;
        let mut fields = Vec::new();
        let mut roles = Vec::new();
        let mut page_cards = Vec::new();
        let mut heights = Vec::with_capacity(pages.len());
        for (&container, page) in pages.iter().zip(&form.pages) {
            let mut y = builder.s(16);
            let mut cards = PageCards::default();
            let mut card_top = 0;
            for (field, placement) in page.fields.iter().zip(card_plan(&page.fields)) {
                let on_card = placement.role != Role::Heading;
                if placement.opens_card {
                    y += builder.s(CARD_PADDING_Y);
                    card_top = y;
                } else if placement.role == Role::Row {
                    y += builder.s(ROW_SEPARATOR_GAP);
                    cards.hairlines.push(Rect::new(
                        builder.s(EDGE + CARD_PADDING_X),
                        y - builder.s(ROW_SEPARATOR_GAP) / 2,
                        page_width - 2 * builder.s(EDGE + CARD_PADDING_X),
                        1,
                    ));
                } else if placement.role == Role::Description {
                    y += builder.s(DESCRIPTION_GAP);
                }
                let inset = if on_card { CARD_PADDING_X } else { 0 };
                let index = fields.len();
                let (controls, height) =
                    builder.field(container, index, field, y, page_width, inset);
                apply_enabled(&controls, field.enabled);
                fields.push((field.clone(), controls));
                roles.push(placement.role);
                y += height;
                if placement.closes_card {
                    y += builder.s(CARD_PADDING_Y);
                    cards.cards.push(Rect::new(
                        builder.s(EDGE),
                        card_top,
                        page_width - 2 * builder.s(EDGE),
                        y - card_top,
                    ));
                    y += builder.s(CARD_GAP);
                }
            }
            page_cards.push(cards);
            heights.push(y + builder.s(8));
        }
        Ok(PageBuild {
            pages,
            fields,
            roles,
            cards: page_cards,
            heights,
        })
    }
}

/// Lays the scopes pane, the rail, and the page area beside each other.
unsafe fn build_panes(
    builder: &Builder,
    hwnd: HWND,
    nav: &SettingsNav,
    page_height: i32,
    margin: i32,
    page_width: i32,
) -> Panes {
    // SAFETY: every list is a child of our live window.
    unsafe {
        let header_height = builder.s(HEADER_HEIGHT);
        let panes_height = header_height + page_height;
        let rail = builder.s(NAV_WIDTH);
        let mut scopes_list = None;
        let mut dividers = Vec::new();
        let mut x = margin;
        if !nav.scopes.is_empty() {
            let width = builder.s(SCOPE_WIDTH);
            scopes_list = rail_list(builder, hwnd, (x, margin, width, panes_height), ID_SCOPES);
            if let Some(created) = scopes_list {
                fill_list(created, nav.scopes.iter().map(|scope| scope.label.as_str()));
                SendMessageW(
                    created,
                    LB_SETCURSEL,
                    Some(WPARAM(nav.selected_scope)),
                    None,
                );
            }
            x += width + builder.s(6);
            dividers.push((x - builder.s(3), margin, margin + panes_height));
        }
        // A new window opens with every group expanded.
        let nav_rows = nav_rows(nav, &[]);
        let list = rail_list(builder, hwnd, (x, margin, rail, panes_height), ID_LIST);
        if let Some(created) = list {
            watch_title_clicks(created);
            fill_list(created, nav_rows.iter().map(NavRow::label));
            if let Some(selected) = entry_row(&nav_rows, nav.selected) {
                SendMessageW(created, LB_SETCURSEL, Some(WPARAM(selected)), None);
            }
        }
        x += rail + builder.s(6);
        dividers.push((x - builder.s(3), margin, margin + panes_height));
        let pages_x = x + builder.s(6);
        Panes {
            client_width: pages_x + page_width + margin,
            command_bar_top: margin + panes_height + margin,
            pages_x,
            header_height,
            list,
            scopes_list,
            dividers,
            nav_rows,
        }
    }
}

/// Creates OK, Cancel, and Apply along the bottom, and returns Apply and the
/// client height.
unsafe fn build_command_bar(
    builder: &Builder,
    hwnd: HWND,
    client_width: i32,
    command_bar_top: i32,
    margin: i32,
    enabled: bool,
) -> (Option<HWND>, i32) {
    // SAFETY: every control is a child of our live window.
    unsafe {
        let button_height = builder.s(30);
        let client_height = command_bar_top + command_bar_height(builder);
        let button_width = builder.s(96);
        let gap = builder.s(8);
        let button_y = command_bar_top + builder.s(14);
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
            let _ = EnableWindow(apply, enabled);
        }
        (apply, client_height)
    }
}

/// The command bar's height: its buttons and the room above and below them.
fn command_bar_height(builder: &Builder) -> i32 {
    builder.s(30) + 2 * builder.s(14)
}

/// The height the page area can take on the pointer's monitor, where
/// `place_window` puts the window: its work area less the window frame, the
/// header, the margins, and the command bar, so the OK, Cancel, and Apply bar
/// stays on screen.
unsafe fn available_page_height(builder: &Builder, dpi: u32, margin: i32) -> i32 {
    let mut frame = RECT::default();
    let mut cursor = Default::default();
    // SAFETY: both only fill in the local out-parameters.
    unsafe {
        let _ =
            AdjustWindowRectExForDpi(&mut frame, window_style(), false, WS_EX_CONTROLPARENT, dpi);
        let _ = GetCursorPos(&mut cursor);
    }
    let work = work_area_at(deskspace_core::geometry::Point::new(cursor.x, cursor.y));
    work.height
        - (frame.bottom - frame.top)
        - 2 * margin
        - builder.s(HEADER_HEIGHT)
        - command_bar_height(builder)
}

/// Sizes the frame for its client area and centers it on the pointer's monitor.
unsafe fn place_window(hwnd: HWND, dpi: u32, client_width: i32, client_height: i32) {
    // SAFETY: `hwnd` is our live window; the calls only read metrics or move it.
    unsafe {
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
                    WS_CHILD | WS_CLIPCHILDREN | WS_VSCROLL,
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
