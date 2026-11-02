//! Builds the controls for each settings field kind.

use crate::win32::dialog::ControlSpec;

use super::build::{
    BUTTON_ADVANCE, BUTTON_HEIGHT, BUTTON_WIDTH, Builder, CHOICE_ADVANCE, COLOR_ADVANCE,
    COLOR_VALUE_WIDTH, COMBO_HEIGHT, EDGE, FieldRow, HEADING_ADVANCE, HEADING_GAP, HEADING_HEIGHT,
    MARKS_GAP, NOTE_GAP, PREVIEW_GAP, SLIDER_ADVANCE, SLIDER_INSET, SWATCH_GAP, SWATCH_SIZE,
    TEXT_ADVANCE, TEXT_EDIT_HEIGHT, TEXT_EDIT_TOP, TOGGLE_ADVANCE, VALUE_EDIT_HEIGHT,
    VALUE_EDIT_TOP, VALUE_GAP, VALUE_WIDTH,
};
use super::*;

impl Builder {
    pub(super) unsafe fn heading_field(&self, row: &mut FieldRow<'_>) -> i32 {
        // SAFETY: the control is a child of the row's page.
        unsafe {
            let top = if row.y > self.s(EDGE) {
                self.s(HEADING_GAP)
            } else {
                0
            };
            let label = self.label(
                row.page,
                &row.field.label,
                (row.x, row.y + top, row.full, self.s(HEADING_HEIGHT)),
            );
            if let Some(label) = label {
                SendMessageW(
                    label,
                    WM_SETFONT,
                    Some(WPARAM(self.heading_font.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
            row.controls.label = label;
            top + self.s(HEADING_ADVANCE)
        }
    }

    pub(super) unsafe fn note_field(&self, row: &mut FieldRow<'_>) -> i32 {
        // SAFETY: the control is a child of the row's page.
        unsafe {
            let text_height = self.text_height(row.page, &row.field.label, row.full);
            row.controls.label = self.control(
                row.page,
                ControlSpec {
                    class: WC_STATICW,
                    text: &row.field.label,
                    style: SS_NOPREFIX,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (row.x, row.y, row.full, text_height),
                    id: 0,
                },
            );
            text_height + self.s(NOTE_GAP)
        }
    }

    pub(super) unsafe fn choice_field(&self, row: &mut FieldRow<'_>) -> i32 {
        let FieldKind::Choice { options, selected } = &row.field.kind else {
            return 0;
        };
        // SAFETY: the controls are children of the row's page.
        unsafe {
            row.controls.label = self.label(
                row.page,
                &row.field.label,
                (row.x, row.y, row.label_width, row.row_height),
            );
            // A drop-down list's height includes the open list.
            let combo = self.control(
                row.page,
                ControlSpec {
                    class: WC_COMBOBOXW,
                    text: "",
                    style: CBS_DROPDOWNLIST as u32 | row.tab,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (
                        row.control_x,
                        row.y,
                        row.control_width,
                        self.s(COMBO_HEIGHT),
                    ),
                    id: row.id_main,
                },
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
            row.controls.main = combo;
            self.s(CHOICE_ADVANCE)
        }
    }

    pub(super) unsafe fn toggle_field(&self, row: &mut FieldRow<'_>) -> i32 {
        let FieldKind::Toggle { on } = &row.field.kind else {
            return 0;
        };
        // SAFETY: the control is a child of the row's page.
        unsafe {
            let check = self.control(
                row.page,
                ControlSpec {
                    class: WC_BUTTONW,
                    text: &row.field.label,
                    style: BS_AUTOCHECKBOX as u32 | row.tab,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (row.x, row.y, row.full, row.row_height),
                    id: row.id_main,
                },
            );
            if let Some(check) = check {
                SendMessageW(check, BM_SETCHECK, Some(WPARAM(*on as usize)), None);
            }
            row.controls.main = check;
            self.s(TOGGLE_ADVANCE)
        }
    }

    pub(super) unsafe fn slider_field(&self, row: &mut FieldRow<'_>) -> i32 {
        let FieldKind::Slider {
            min,
            max,
            value,
            editable,
            marks,
            ..
        } = &row.field.kind
        else {
            return 0;
        };
        let (min, max, value) = (*min, *max, *value);
        let (editable, has_marks) = (*editable, !marks.is_empty());
        // SAFETY: the controls are children of the row's page.
        unsafe {
            row.controls.label = self.label(
                row.page,
                &row.field.label,
                (row.x, row.y, row.label_width, row.row_height),
            );
            self.slider_track(row, min, max, value);
            self.slider_value(row, value, editable);
            if has_marks {
                row.row_height + self.s(MARK_LABEL_HEIGHT + MARKS_GAP)
            } else {
                self.s(SLIDER_ADVANCE)
            }
        }
    }

    pub(super) unsafe fn slider_track(
        &self,
        row: &mut FieldRow<'_>,
        min: i32,
        max: i32,
        value: i32,
    ) {
        let FieldKind::Slider { marks, .. } = &row.field.kind else {
            return;
        };
        // SAFETY: the control is a child of the row's page.
        unsafe {
            let ticks = if marks.is_empty() { TBS_NOTICKS } else { 0 };
            let value_width = self.s(VALUE_WIDTH);
            let slider_x = row.control_x - self.s(SLIDER_INSET);
            let slider = self.control(
                row.page,
                ControlSpec {
                    class: TRACKBAR_CLASSW,
                    text: "",
                    style: TBS_HORZ | ticks | row.tab,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (
                        slider_x,
                        row.y,
                        row.control_width - value_width,
                        row.row_height,
                    ),
                    id: row.id_main,
                },
            );
            if let Some(slider) = slider {
                SendMessageW(
                    slider,
                    TBM_SETRANGEMIN,
                    Some(WPARAM(0)),
                    Some(LPARAM(min as isize)),
                );
                SendMessageW(
                    slider,
                    TBM_SETRANGEMAX,
                    Some(WPARAM(0)),
                    Some(LPARAM(max as isize)),
                );
                if !marks.is_empty() {
                    row.controls.marks = self.slider_marks(
                        row.page,
                        slider,
                        slider_x,
                        row.y + row.row_height,
                        (min, max),
                        marks,
                    );
                }
                SendMessageW(
                    slider,
                    TBM_SETPOS,
                    Some(WPARAM(1)),
                    Some(LPARAM(value as isize)),
                );
            }
            row.controls.main = slider;
        }
    }

    pub(super) unsafe fn slider_value(&self, row: &mut FieldRow<'_>, value: i32, editable: bool) {
        let FieldKind::Slider { suffix, .. } = &row.field.kind else {
            return;
        };
        let value_width = self.s(VALUE_WIDTH);
        let value_bounds = (
            row.control_x + row.control_width - value_width + self.s(VALUE_GAP),
            row.y,
            value_width,
            row.row_height,
        );
        // SAFETY: the control is a child of the row's page.
        unsafe {
            if editable {
                row.controls.value_edit = self.control(
                    row.page,
                    ControlSpec {
                        class: WC_EDITW,
                        text: &value.to_string(),
                        style: ES_RIGHT as u32 | ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | row.tab,
                        ex_style: WS_EX_CLIENTEDGE,
                        bounds: (
                            value_bounds.0,
                            row.y + self.s(VALUE_EDIT_TOP),
                            value_bounds.2,
                            self.s(VALUE_EDIT_HEIGHT),
                        ),
                        id: row.id_secondary,
                    },
                );
            } else {
                row.controls.value_text =
                    self.label(row.page, &format!("{value}{suffix}"), value_bounds);
            }
        }
    }

    pub(super) unsafe fn color_field(&self, row: &mut FieldRow<'_>) -> i32 {
        let FieldKind::Color { color } = &row.field.kind else {
            return 0;
        };
        // SAFETY: the controls are children of the row's page.
        unsafe {
            row.controls.label = self.label(
                row.page,
                &row.field.label,
                (row.x, row.y, row.label_width, row.row_height),
            );
            let swatch_size = self.s(SWATCH_SIZE);
            // The swatch is the button: an owner-draw control that draws the
            // color box and an eyedropper glyph.
            let swatch = self.control(
                row.page,
                ControlSpec {
                    class: WC_BUTTONW,
                    text: "",
                    style: BS_OWNERDRAW as u32 | row.tab,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (
                        row.control_x,
                        row.y + (row.row_height - swatch_size) / 2,
                        swatch_size,
                        swatch_size,
                    ),
                    id: row.id_main,
                },
            );
            row.controls.main = swatch;
            row.controls.swatch = swatch;
            row.controls.value_text = self.label(
                row.page,
                &color.to_rgb_hex(),
                (
                    row.control_x + swatch_size + self.s(SWATCH_GAP),
                    row.y,
                    self.s(COLOR_VALUE_WIDTH),
                    row.row_height,
                ),
            );
            self.s(COLOR_ADVANCE)
        }
    }

    pub(super) unsafe fn text_field(&self, row: &mut FieldRow<'_>) -> i32 {
        let FieldKind::Text { value, read_only } = &row.field.kind else {
            return 0;
        };
        // SAFETY: the control is a child of the row's page.
        unsafe {
            row.controls.label = self.label(
                row.page,
                &row.field.label,
                (row.x, row.y, row.label_width, row.row_height),
            );
            let mut style = ES_AUTOHSCROLL as u32 | row.tab;
            if *read_only {
                style |= ES_READONLY as u32;
            }
            row.controls.main = self.control(
                row.page,
                ControlSpec {
                    class: WC_EDITW,
                    text: value,
                    style,
                    ex_style: WS_EX_CLIENTEDGE,
                    bounds: (
                        row.control_x,
                        row.y + self.s(TEXT_EDIT_TOP),
                        row.control_width,
                        self.s(TEXT_EDIT_HEIGHT),
                    ),
                    id: row.id_main,
                },
            );
            self.s(TEXT_ADVANCE)
        }
    }

    pub(super) unsafe fn button_field(&self, row: &mut FieldRow<'_>) -> i32 {
        let FieldKind::Button { caption } = &row.field.kind else {
            return 0;
        };
        // SAFETY: the control is a child of the row's page.
        unsafe {
            let left = if row.field.label.is_empty() {
                row.x
            } else {
                row.controls.label = self.label(
                    row.page,
                    &row.field.label,
                    (row.x, row.y, row.label_width, row.row_height),
                );
                row.control_x
            };
            row.controls.main = self.control(
                row.page,
                ControlSpec {
                    class: WC_BUTTONW,
                    text: caption,
                    style: BS_PUSHBUTTON as u32 | row.tab,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (left, row.y, self.s(BUTTON_WIDTH), self.s(BUTTON_HEIGHT)),
                    id: row.id_main,
                },
            );
            self.s(BUTTON_ADVANCE)
        }
    }

    pub(super) unsafe fn space_preview_field(&self, row: &mut FieldRow<'_>) -> i32 {
        // SAFETY: the control is a child of the row's page.
        unsafe {
            // The preview reads its data from this field in the window's
            // state, found through the control id.
            row.controls.main = self.control(
                row.page,
                ControlSpec {
                    class: PREVIEW_CLASS,
                    text: "",
                    style: 0,
                    ex_style: WINDOW_EX_STYLE(0),
                    bounds: (
                        row.x,
                        row.y,
                        row.full,
                        self.s(super::preview::PREVIEW_HEIGHT),
                    ),
                    id: row.id_main,
                },
            );
            self.s(super::preview::PREVIEW_HEIGHT + PREVIEW_GAP)
        }
    }
}
