//! Control commands and events for the settings window: field edits, the
//! color picker, slider scrolling, and the color and text helpers.

use super::update::{Update, set_swatch};
use super::*;
use windows::Win32::Foundation::COLORREF;

pub(crate) fn colorref(color: Color) -> COLORREF {
    COLORREF(color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16))
}

pub(super) unsafe fn window_text(control: HWND) -> String {
    // SAFETY: the buffer is sized from GetWindowTextLengthW plus the NUL.
    unsafe {
        let length = GetWindowTextLengthW(control).max(0) as usize;
        let mut buffer = vec![0u16; length + 1];
        let copied = GetWindowTextW(control, &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..copied])
    }
}

/// Handles WM_COMMAND from a field control. Any dispatch is the last thing
/// done, because the handler may destroy this window.
pub(super) unsafe fn on_field_command(window: HWND, id: usize, code: u32, control: HWND) {
    let index = (id - CONTROL_BASE) / 4;
    let part = (id - CONTROL_BASE) % 4;
    // SAFETY (all calls): `window` is the live settings window and `control`
    // its child; state borrows end before any control message is sent.
    unsafe {
        let Some(Some((field, color))) = with_state(window, |state| {
            if state.updating {
                return None;
            }
            let (field, _) = state.fields.get(index)?;
            let color = match field.kind {
                FieldKind::Color { color } => color,
                _ => Color::TRANSPARENT,
            };
            Some((field.clone(), color))
        }) else {
            return;
        };
        // A slider's editable value box commits when it loses focus, so typing
        // is not clamped or rewritten under the caret.
        if part == PART_SECONDARY {
            if let FieldKind::Slider {
                min,
                max,
                value: stored,
                editable: true,
                ..
            } = &field.kind
                && code == EN_KILLFOCUS
            {
                let stored = *stored;
                let typed = window_text(control).trim().parse::<i32>().unwrap_or(stored);
                let value = typed.clamp(*min, *max);
                if value != stored {
                    with_state(window, |state| {
                        if let Some((field, _)) = state.fields.get_mut(index)
                            && let FieldKind::Slider { value: slot, .. } = &mut field.kind
                        {
                            *slot = value;
                        }
                    });
                    settings_event(SettingsEvent::Changed {
                        field: field.id,
                        value: FieldValue::Slider(value),
                    });
                } else {
                    // Reject invalid or out-of-range text.
                    with_state(window, |state| state.updating = true);
                    let text = wide(&value.to_string());
                    let _ = SetWindowTextW(control, PCWSTR(text.as_ptr()));
                    with_state(window, |state| state.updating = false);
                }
            }
            return;
        }
        if part != PART_MAIN {
            return;
        }
        let value = match (&field.kind, code) {
            (FieldKind::Choice { .. }, CBN_SELCHANGE) => {
                let selected = SendMessageW(control, CB_GETCURSEL, None, None).0;
                (selected >= 0).then_some(FieldValue::Choice(selected as usize))
            }
            (FieldKind::Toggle { .. }, BN_CLICKED) => {
                let checked = SendMessageW(control, BM_GETCHECK, None, None).0 == 1;
                Some(FieldValue::Toggle(checked))
            }
            (
                FieldKind::Text {
                    read_only: false, ..
                },
                EN_CHANGE,
            ) => Some(FieldValue::Text(window_text(control))),
            (FieldKind::Color { .. }, BN_CLICKED) => {
                let dark = with_state(window, |state| state.colors.dark).unwrap_or(false);
                crate::win32::color_picker::pick(window, color, dark).map(FieldValue::Color)
            }
            (FieldKind::Button { .. }, BN_CLICKED) => {
                settings_event(SettingsEvent::Button { field: field.id });
                return;
            }
            _ => None,
        };
        let Some(value) = value else {
            return;
        };
        if let FieldValue::Color(color) = value {
            let updates = with_state(window, |state| {
                let mut updates = Vec::new();
                if let Some((field, controls)) = state.fields.get_mut(index) {
                    field.kind = FieldKind::Color { color };
                    set_swatch(controls, color, &mut updates);
                }
                updates
            })
            .unwrap_or_default();
            for update in updates {
                match update {
                    Update::Repaint(hwnd) => {
                        let _ = InvalidateRect(Some(hwnd), None, true);
                    }
                    Update::Text(hwnd, text) => {
                        let text = wide(&text);
                        let _ = SetWindowTextW(hwnd, PCWSTR(text.as_ptr()));
                    }
                    _ => {}
                }
            }
        }
        settings_event(SettingsEvent::Changed {
            field: field.id,
            value,
        });
    }
}

pub(super) unsafe fn on_scroll(window: HWND, control: HWND) {
    // SAFETY (all calls): `control` is a trackbar child of this window.
    unsafe {
        let Some(Some((index, field_id, suffix, last, value_text, value_edit))) =
            with_state(window, |state| {
                if state.updating {
                    return None;
                }
                let index = state.field_index_for(control)?;
                let (field, controls) = &state.fields[index];
                match &field.kind {
                    FieldKind::Slider { value, suffix, .. } => Some((
                        index,
                        field.id,
                        suffix.clone(),
                        *value,
                        controls.value_text,
                        controls.value_edit,
                    )),
                    _ => None,
                }
            })
        else {
            return;
        };
        let value = SendMessageW(control, TBM_GETPOS, None, None).0 as i32;
        if value == last {
            return;
        }
        with_state(window, |state| {
            if let Some((field, _)) = state.fields.get_mut(index)
                && let FieldKind::Slider { value: stored, .. } = &mut field.kind
            {
                *stored = value;
            }
        });
        if let Some(edit) = value_edit {
            // Setting the text raises EN_CHANGE; ignore it.
            with_state(window, |state| state.updating = true);
            let text_value = wide(&value.to_string());
            let _ = SetWindowTextW(edit, PCWSTR(text_value.as_ptr()));
            with_state(window, |state| state.updating = false);
        } else if let Some(text) = value_text {
            let text_value = wide(&format!("{value}{suffix}"));
            let _ = SetWindowTextW(text, PCWSTR(text_value.as_ptr()));
        }
        settings_event(SettingsEvent::Changed {
            field: field_id,
            value: FieldValue::Slider(value),
        });
    }
}

/// Shows the page at `index` and hides the rest.
pub(super) unsafe fn show_page(pages: &[HWND], index: usize) {
    // SAFETY: the handles are live page windows of the settings window.
    unsafe {
        for (i, page) in pages.iter().enumerate() {
            let _ = ShowWindow(*page, if i == index { SW_SHOW } else { SW_HIDE });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_packs_bgr() {
        let color = Color::rgb(0x12, 0x34, 0x56);
        assert_eq!(colorref(color).0, 0x0056_3412);
    }
}
