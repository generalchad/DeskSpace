//! Refreshes an open settings window in place from a new SettingsForm.

use super::build::fill_list;
use super::commands::{show_page, window_text};
use super::nav::{NavRow, entry_row, nav_rows};
use super::*;

/// Selects `row` in a rail list. Edits arrive on every slider tick; touching
/// the list only when it changed keeps it from repainting each time.
unsafe fn sync_selection(list: HWND, rebuilt: bool, row: usize) {
    // SAFETY: `list` is a live list box of the settings window.
    unsafe {
        let current = SendMessageW(list, LB_GETCURSEL, None, None).0;
        if rebuilt || current != row as isize {
            SendMessageW(list, LB_SETCURSEL, Some(WPARAM(row)), None);
            let _ = InvalidateRect(Some(list), None, false);
        }
    }
}

/// Shows `header` in the header controls: the title, the button, and the
/// switch, each only when the header supplies one.
pub(super) unsafe fn apply_header(
    title: Option<HWND>,
    switch: Option<HWND>,
    button: Option<HWND>,
    header: Option<&SettingsHeader>,
) {
    // SAFETY: the handles are live controls of the settings window. Neither
    // SetWindowText on a static/checkbox/button nor BM_SETCHECK reports an edit.
    unsafe {
        if let Some(title) = title {
            let text = header.map_or("", |header| header.title.as_str());
            if window_text(title) != text {
                let text = wide(text);
                let _ = SetWindowTextW(title, PCWSTR(text.as_ptr()));
            }
        }
        if let Some(control) = button {
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
        if let Some(control) = switch {
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

/// Enables or disables a field's input control and repaints its labels,
/// which take their color from the field's enabled state.
pub(super) unsafe fn apply_enabled(controls: &FieldControls, enabled: bool) {
    // SAFETY: the handles are live controls of the settings window.
    unsafe {
        if let Some(main) = controls.main {
            let _ = EnableWindow(main, enabled);
        }
        if let Some(edit) = controls.value_edit {
            let _ = EnableWindow(edit, enabled);
        }
        for label in controls.labels() {
            let _ = InvalidateRect(Some(label), None, true);
        }
    }
}

/// An edit the window must make to a control, collected under the state
/// borrow and applied after it, because control messages can re-enter
/// `wndproc` (for example SetWindowText raises EN_CHANGE).
pub(super) enum Update {
    /// The field's main control, an optional value edit, and its labels (see
    /// `apply_enabled`).
    Enable(Option<HWND>, Option<HWND>, Vec<HWND>, bool),
    Text(HWND, String),
    Choice(HWND, usize),
    Check(HWND, bool),
    Slider(HWND, i32),
    Repaint(HWND),
}

/// The header controls and the header they should show.
struct HeaderUpdate {
    title: Option<HWND>,
    switch: Option<HWND>,
    button: Option<HWND>,
    header: Option<SettingsHeader>,
}

/// The controls to update after a plan is made, collected so no state borrow
/// is held while control messages run.
struct UpdatePlan {
    updates: Vec<Update>,
    apply: Option<HWND>,
    rail: Option<(HWND, Vec<NavRow>, bool, usize)>,
    page: Option<usize>,
    pages: Vec<HWND>,
    scopes: Option<(HWND, Vec<SettingsScope>, bool, usize)>,
    header: Option<HeaderUpdate>,
}

/// Refreshes control values, labels, and enabled state from `form`. Returns
/// false when `form` has a different shape and the window must be rebuilt.
/// The panes, the header, and the shown page are touched only when they
/// change, because a slider drag sends an update per tick.
pub(crate) fn update(hwnd: HWND, form: &SettingsForm) -> bool {
    // SAFETY: `plan_update` only borrows the window's state and handles.
    let Some(plan) = (unsafe { plan_update(hwnd, form) }) else {
        return false;
    };
    // SAFETY: every handle in the plan is a live control of this window.
    unsafe {
        update_pages(plan.updates, plan.apply, form);
        update_panes(plan.rail, plan.page, plan.pages, plan.scopes);
        update_header(plan.header);
        with_state(hwnd, |state| state.updating = false);
    }
    true
}

/// Collects the changes `form` makes, or `None` when the window must be
/// rebuilt because its shape changed.
unsafe fn plan_update(hwnd: HWND, form: &SettingsForm) -> Option<UpdatePlan> {
    // SAFETY: the borrow only reads/writes state and creates brushes; control
    // messages are sent after it ends.
    unsafe {
        with_state(hwnd, |state| {
            if !state.form.same_shape(form) {
                return None;
            }
            let mut updates = Vec::new();
            for ((old, controls), new) in state
                .fields
                .iter_mut()
                .zip(form.pages.iter().flat_map(|page| page.fields.iter()))
            {
                plan_field(old, controls, new, &mut updates);
                *old = new.clone();
            }
            let new_rows = nav_rows(&form.nav);
            let rebuild = state.nav_rows != new_rows;
            let selected_row = entry_row(&new_rows, form.nav.selected);
            let page = form.nav.page_of(form.nav.selected);
            let old_page = state.form.nav.page_of(state.form.nav.selected);
            // Re-showing the same page on every edit would hide and show the
            // pages for nothing.
            let page = page.filter(|page| rebuild || Some(*page) != old_page);
            let new_scopes = form.nav.scopes.clone();
            let scopes = state.scopes_list.map(|list| {
                let rebuild = state.scope_rows != new_scopes;
                let selected = form.nav.selected_scope;
                (list, new_scopes.clone(), rebuild, selected)
            });
            let new_header = form.nav.header.clone();
            let old_header = state.form.nav.header.as_ref();
            let header = (old_header != new_header.as_ref()).then_some(HeaderUpdate {
                title: state.header_title,
                switch: state.header_switch,
                button: state.header_button,
                header: new_header,
            });
            state.nav_rows = new_rows.clone();
            state.scope_rows = new_scopes;
            state.form = form.clone();
            state.updating = true;
            let rail = state
                .list
                .map(|list| (list, new_rows, rebuild, selected_row));
            Some(UpdatePlan {
                updates,
                apply: state.apply,
                rail,
                page,
                pages: state.pages.clone(),
                scopes,
                header,
            })
        })
        .flatten()
    }
}

/// Applies the planned field updates and the Apply button's state.
unsafe fn update_pages(updates: Vec<Update>, apply: Option<HWND>, form: &SettingsForm) {
    // SAFETY: every handle is a live control of this window.
    unsafe {
        for update in updates {
            match update {
                Update::Enable(main, edit, labels, enabled) => {
                    if let Some(main) = main {
                        let _ = EnableWindow(main, enabled);
                    }
                    if let Some(edit) = edit {
                        let _ = EnableWindow(edit, enabled);
                    }
                    for label in labels {
                        let _ = InvalidateRect(Some(label), None, true);
                    }
                }
                Update::Text(control, text) => {
                    if window_text(control) != text {
                        let text = wide(&text);
                        let _ = SetWindowTextW(control, PCWSTR(text.as_ptr()));
                    }
                }
                Update::Choice(control, index) => {
                    SendMessageW(control, CB_SETCURSEL, Some(WPARAM(index)), None);
                }
                Update::Check(control, on) => {
                    SendMessageW(control, BM_SETCHECK, Some(WPARAM(on as usize)), None);
                }
                Update::Slider(control, value) => {
                    SendMessageW(
                        control,
                        TBM_SETPOS,
                        Some(WPARAM(1)),
                        Some(LPARAM(value as isize)),
                    );
                }
                Update::Repaint(control) => {
                    let _ = InvalidateRect(Some(control), None, true);
                }
            }
        }
        if let Some(apply) = apply {
            let _ = EnableWindow(apply, form.apply_enabled);
        }
    }
}

/// Rebuilds or selects the rail and scopes pane, and shows the named page.
unsafe fn update_panes(
    rail: Option<(HWND, Vec<NavRow>, bool, usize)>,
    page: Option<usize>,
    pages: Vec<HWND>,
    scopes: Option<(HWND, Vec<SettingsScope>, bool, usize)>,
) {
    // SAFETY: every handle is a live control of this window.
    unsafe {
        if let Some((list, rows, rebuild, selected_row)) = rail {
            if rebuild {
                fill_list(list, rows.iter().map(NavRow::label));
            }
            sync_selection(list, rebuild, selected_row);
            if let Some(page) = page {
                show_page(&pages, page);
            }
        }
        if let Some((list, scopes, rebuild, selected)) = scopes {
            if rebuild {
                fill_list(list, scopes.iter().map(|scope| scope.label.as_str()));
            }
            sync_selection(list, rebuild, selected);
        }
    }
}

/// Shows the header's title, button, and switch.
unsafe fn update_header(header: Option<HeaderUpdate>) {
    if let Some(header) = header {
        // SAFETY: every handle is a live control of this window.
        unsafe {
            apply_header(
                header.title,
                header.switch,
                header.button,
                header.header.as_ref(),
            );
        }
    }
}

fn plan_field(old: &Field, controls: &mut FieldControls, new: &Field, updates: &mut Vec<Update>) {
    if old.enabled != new.enabled {
        updates.push(Update::Enable(
            controls.main,
            controls.value_edit,
            controls.labels(),
            new.enabled,
        ));
    }
    if old.label != new.label {
        let target = match new.kind {
            FieldKind::Toggle { .. } => controls.main,
            _ => controls.label,
        };
        if let Some(target) = target {
            updates.push(Update::Text(target, new.label.clone()));
        }
    }
    let Some(main) = controls.main else {
        return;
    };
    match (&old.kind, &new.kind) {
        (FieldKind::Choice { selected: a, .. }, FieldKind::Choice { selected: b, .. })
            if a != b =>
        {
            updates.push(Update::Choice(main, *b));
        }
        (FieldKind::Toggle { on: a }, FieldKind::Toggle { on: b }) if a != b => {
            updates.push(Update::Check(main, *b));
        }
        (
            FieldKind::Slider { value: a, .. },
            FieldKind::Slider {
                value: b, suffix, ..
            },
        ) if a != b => {
            updates.push(Update::Slider(main, *b));
            if let Some(edit) = controls.value_edit {
                updates.push(Update::Text(edit, b.to_string()));
            } else if let Some(text) = controls.value_text {
                updates.push(Update::Text(text, format!("{b}{suffix}")));
            }
        }
        (FieldKind::Color { color: a }, FieldKind::Color { color: b }) if a != b => {
            set_swatch(controls, *b, updates);
        }
        (FieldKind::Text { value: a, .. }, FieldKind::Text { value: b, .. }) if a != b => {
            updates.push(Update::Text(main, b.clone()));
        }
        (FieldKind::Button { caption: a }, FieldKind::Button { caption: b }) if a != b => {
            updates.push(Update::Text(main, b.clone()));
        }
        // The preview paints from the field, which the caller replaces with
        // the new one, so a repaint shows the change.
        (FieldKind::SpacePreview(a), FieldKind::SpacePreview(b)) if a != b => {
            updates.push(Update::Repaint(main));
        }
        _ => {}
    }
}

pub(super) fn set_swatch(controls: &mut FieldControls, color: Color, updates: &mut Vec<Update>) {
    if let Some(swatch) = controls.swatch {
        updates.push(Update::Repaint(swatch));
    }
    if let Some(text) = controls.value_text {
        updates.push(Update::Text(text, color.to_rgb_hex()));
    }
}
