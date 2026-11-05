//! The settings window's navigation rail and scopes pane: rows, selection,
//! and the events they report.

use super::build::Builder;
use super::commands::show_page;
use super::*;
use crate::win32::dialog::ControlSpec;
use crate::win32::system::{hiword, loword};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    LB_GETTOPINDEX, LB_ITEMFROMPOINT, LB_SETTOPINDEX, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_SETREDRAW,
};

/// One row of the navigation rail. A group title is never selected: a click
/// on it collapses or expands its group, and the keyboard steps over it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NavRow {
    Title {
        label: String,
        collapsed: bool,
    },
    /// `nested` entries sit under a group title and are indented below it;
    /// `entry` is the entry's index in `SettingsNav::entries`.
    Entry {
        label: String,
        marked: bool,
        nested: bool,
        entry: usize,
    },
}

impl NavRow {
    pub(super) fn label(&self) -> &str {
        match self {
            Self::Title { label, .. } | Self::Entry { label, .. } => label,
        }
    }

    pub(super) fn is_entry(&self) -> bool {
        matches!(self, Self::Entry { .. })
    }
}

/// The rail's rows: each group's title (untitled groups have none), then its
/// entries unless the title is in `collapsed`.
pub(super) fn nav_rows(nav: &SettingsNav, collapsed: &[String]) -> Vec<NavRow> {
    let mut rows = Vec::new();
    let mut entry = 0;
    for group in &nav.groups {
        let titled = !group.title.is_empty();
        let hidden = titled && collapsed.contains(&group.title);
        if titled {
            rows.push(NavRow::Title {
                label: group.title.clone(),
                collapsed: hidden,
            });
        }
        for each in &group.entries {
            if !hidden {
                rows.push(NavRow::Entry {
                    label: each.label.clone(),
                    marked: each.marked,
                    nested: titled,
                    entry,
                });
            }
            entry += 1;
        }
    }
    rows
}

/// The flattened entry index of rail row `row`, or `None` for a title.
fn row_entry(rows: &[NavRow], row: usize) -> Option<usize> {
    match rows.get(row)? {
        NavRow::Entry { entry, .. } => Some(*entry),
        NavRow::Title { .. } => None,
    }
}

/// The rail row showing flattened entry `entry`; `None` while its group is
/// collapsed, so the rail shows no selection.
pub(super) fn entry_row(rows: &[NavRow], entry: usize) -> Option<usize> {
    rows.iter()
        .position(|row| matches!(row, NavRow::Entry { entry: each, .. } if *each == entry))
}

/// `collapsed` with `title` flipped between collapsed and expanded.
fn toggled(collapsed: &[String], title: &str) -> Vec<String> {
    if collapsed.iter().any(|each| each == title) {
        collapsed
            .iter()
            .filter(|each| *each != title)
            .cloned()
            .collect()
    } else {
        let mut collapsed = collapsed.to_vec();
        collapsed.push(title.to_owned());
        collapsed
    }
}

/// Creates an owner-drawn list box for a rail pane (scopes or categories).
pub(super) unsafe fn rail_list(
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

/// Collapses or expands the group titled `title` and fills the rail again,
/// keeping its scroll and the selected entry (none while that entry's group
/// is collapsed). The shown page does not change. Returns the new rows.
unsafe fn toggle_group(window: HWND, title: &str) -> Option<Vec<NavRow>> {
    // SAFETY: the borrow only updates the rows; the list is filled after it.
    unsafe {
        let (list, rows, selected) = with_state(window, |state| {
            state.collapsed = toggled(&state.collapsed, title);
            let rows = nav_rows(&state.form.nav, &state.collapsed);
            state.nav_rows = rows.clone();
            let selected = entry_row(&rows, state.form.nav.selected);
            (state.list, rows, selected)
        })?;
        let list = list?;
        let top = SendMessageW(list, LB_GETTOPINDEX, None, None).0;
        SendMessageW(list, WM_SETREDRAW, Some(WPARAM(0)), None);
        fill_list(list, rows.iter().map(NavRow::label));
        // -1 (usize::MAX) clears the selection.
        SendMessageW(
            list,
            LB_SETCURSEL,
            Some(WPARAM(selected.unwrap_or(usize::MAX))),
            None,
        );
        SendMessageW(
            list,
            LB_SETTOPINDEX,
            Some(WPARAM(top.max(0) as usize)),
            None,
        );
        SendMessageW(list, WM_SETREDRAW, Some(WPARAM(1)), None);
        let _ = InvalidateRect(Some(list), None, true);
        Some(rows)
    }
}

/// The subclass's id on the rail (`SetWindowSubclass`).
const TITLE_CLICKS: usize = 1;

/// Makes a click on one of the rail's group titles collapse or expand it,
/// rather than reach the list box, which would select the title.
pub(super) fn watch_title_clicks(list: HWND) {
    // SAFETY: `list` is the live rail; the subclass removes itself in
    // WM_NCDESTROY.
    unsafe {
        let _ = SetWindowSubclass(list, Some(rail_subclass), TITLE_CLICKS, 0);
    }
}

unsafe extern "system" fn rail_subclass(
    list: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: comctl32 calls this for the live rail with its message's own
    // arguments; its parent is the settings window, which owns the rows.
    unsafe {
        match message {
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                // LB_ITEMFROMPOINT takes client coordinates, as the click
                // carries them, and sets the high word outside every row.
                // https://learn.microsoft.com/en-us/windows/win32/controls/lb-itemfrompoint
                let hit = SendMessageW(list, LB_ITEMFROMPOINT, None, Some(lparam)).0;
                let window = GetParent(list).unwrap_or_default();
                let title = (hiword(hit) == 0)
                    .then(|| {
                        with_state(window, |state| {
                            match state.nav_rows.get(loword(hit) as usize) {
                                Some(NavRow::Title { label, .. }) => Some(label.clone()),
                                _ => None,
                            }
                        })
                        .flatten()
                    })
                    .flatten();
                match title {
                    Some(title) => {
                        toggle_group(window, &title);
                        LRESULT(0)
                    }
                    None => DefSubclassProc(list, message, wparam, lparam),
                }
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(list, Some(rail_subclass), id);
                DefSubclassProc(list, message, wparam, lparam)
            }
            _ => DefSubclassProc(list, message, wparam, lparam),
        }
    }
}

/// Handles a rail selection change. A click never lands on a group title
/// (`rail_subclass` takes it), so a title selected here came from the
/// keyboard: a collapsed group opens, and the selection moves on to the
/// nearest entry in the direction it was going. Shows that entry's page,
/// then tells the app last, because the handler may rebuild the form.
pub(super) unsafe fn on_nav_changed(window: HWND) {
    // SAFETY: the list and pages are live children of `window`; the state
    // borrow ends before the list messages and the dispatch.
    unsafe {
        let Some((Some(list), mut rows, pages, nav)) = with_state(window, |state| {
            (
                state.list,
                state.nav_rows.clone(),
                state.pages.clone(),
                state.form.nav.clone(),
            )
        }) else {
            return;
        };
        let current = SendMessageW(list, LB_GETCURSEL, None, None).0;
        if current < 0 {
            return;
        }
        let mut selected = current as usize;
        let upward = entry_row(&rows, nav.selected).is_some_and(|before| selected < before);
        if row_entry(&rows, selected).is_none() {
            let opened = title_to_open(&rows, selected, upward).map(str::to_owned);
            if let Some(title) = &opened
                && let Some(new_rows) = toggle_group(window, title)
            {
                rows = new_rows;
            }
            let Some(target) = keyboard_target(&rows, selected, upward, opened.as_deref()) else {
                return;
            };
            SendMessageW(list, LB_SETCURSEL, Some(WPARAM(target)), None);
            let _ = InvalidateRect(Some(list), None, true);
            selected = target;
        }
        let entry = row_entry(&rows, selected).expect("moved to an entry row");
        if let Some(page) = nav.page_of(entry) {
            show_page(window, &pages, page);
        }
        settings_event(SettingsEvent::NavChanged { entry });
    }
}

/// The collapsed group the keyboard opens on landing on title row `row`:
/// moving down, that title's own group; moving up, which lands on the title
/// of the group just left, the group above it when that one is collapsed.
fn title_to_open(rows: &[NavRow], row: usize, upward: bool) -> Option<&str> {
    let candidate = if upward { row.checked_sub(1)? } else { row };
    match rows.get(candidate)? {
        NavRow::Title {
            label,
            collapsed: true,
        } => Some(label),
        _ => None,
    }
}

/// Where the keyboard's selection goes from title row `row`. Moving down it
/// is the next entry, moving up the one before; a group the move just opened
/// (`opened`) is entered at its first entry going down and its last going
/// up. At either end of the rail it turns back.
fn keyboard_target(
    rows: &[NavRow],
    row: usize,
    upward: bool,
    opened: Option<&str>,
) -> Option<usize> {
    let is_entry = |row: &usize| rows[*row].is_entry();
    if let Some(title) = opened {
        let at = rows
            .iter()
            .position(|each| matches!(each, NavRow::Title { label, .. } if label == title))?;
        let group = (at + 1..rows.len()).take_while(is_entry);
        return if upward {
            group.last()
        } else {
            group.into_iter().next()
        };
    }
    let mut below = row + 1..rows.len();
    let mut above = (0..row).rev();
    if upward {
        above.find(is_entry).or_else(|| below.find(is_entry))
    } else {
        below.find(is_entry).or_else(|| above.find(is_entry))
    }
}

/// Handles a scopes-pane selection change. The app answers with a form for
/// that scope, so this only reports it, as the last thing done.
pub(super) unsafe fn on_scope_changed(window: HWND) {
    // SAFETY: the list is a live child of `window`; the borrow ends first.
    unsafe {
        let Some(Some(list)) = with_state(window, |state| state.scopes_list) else {
            return;
        };
        let current = SendMessageW(list, LB_GETCURSEL, None, None).0;
        if current >= 0 {
            settings_event(SettingsEvent::ScopeChanged {
                scope: current as usize,
            });
        }
    }
}

/// Reports the header switch's new state, unless the window set it itself.
pub(super) unsafe fn on_header_switch(window: HWND, control: HWND) {
    // SAFETY: `control` is the live header checkbox of `window`.
    unsafe {
        if with_state(window, |state| state.updating) != Some(false) {
            return;
        }
        let on = SendMessageW(control, BM_GETCHECK, None, None).0 == 1;
        settings_event(SettingsEvent::HeaderSwitch { on });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{SettingsNavEntry, SettingsNavGroup};

    fn group(title: &str, labels: &[&str]) -> SettingsNavGroup {
        SettingsNavGroup {
            title: title.into(),
            entries: labels
                .iter()
                .map(|label| SettingsNavEntry {
                    label: (*label).into(),
                    page: 0,
                    marked: false,
                })
                .collect(),
        }
    }

    fn nav() -> SettingsNav {
        SettingsNav {
            groups: vec![
                group("Look", &["Appearance", "Labels"]),
                group("Fonts", &["Spaces", "Windows"]),
                group("DeskSpace", &["General"]),
            ],
            selected: 1,
            ..Default::default()
        }
    }

    fn labels(rows: &[NavRow]) -> Vec<&str> {
        rows.iter().map(NavRow::label).collect()
    }

    fn collapsed(titles: &[&str]) -> Vec<String> {
        titles.iter().map(|title| (*title).to_owned()).collect()
    }

    #[test]
    fn nav_rows_put_a_title_before_each_group() {
        let rows = nav_rows(&nav(), &[]);
        assert_eq!(
            labels(&rows),
            [
                "Look",
                "Appearance",
                "Labels",
                "Fonts",
                "Spaces",
                "Windows",
                "DeskSpace",
                "General"
            ]
        );
        assert!(!rows[0].is_entry());
        assert!(rows[1].is_entry());
    }

    #[test]
    fn untitled_groups_have_no_title_row_and_are_not_nested() {
        let mut nav = nav();
        nav.groups[0].title.clear();
        let rows = nav_rows(&nav, &[]);
        assert_eq!(labels(&rows)[..3], ["Appearance", "Labels", "Fonts"]);
        assert_eq!(row_entry(&rows, 0), Some(0));
        assert!(matches!(rows[0], NavRow::Entry { nested: false, .. }));
        assert!(matches!(rows[3], NavRow::Entry { nested: true, .. }));
    }

    #[test]
    fn a_collapsed_group_shows_only_its_title_and_keeps_the_entry_numbers() {
        let rows = nav_rows(&nav(), &collapsed(&["Fonts"]));
        assert_eq!(
            labels(&rows),
            [
                "Look",
                "Appearance",
                "Labels",
                "Fonts",
                "DeskSpace",
                "General"
            ]
        );
        assert!(matches!(
            rows[3],
            NavRow::Title {
                collapsed: true,
                ..
            }
        ));
        assert!(matches!(
            rows[0],
            NavRow::Title {
                collapsed: false,
                ..
            }
        ));
        assert_eq!(row_entry(&rows, 5), Some(4), "General is still entry 4");
        assert_eq!(entry_row(&rows, 4), Some(5));
        assert_eq!(entry_row(&rows, 2), None, "Spaces is hidden");
        // An untitled group cannot collapse.
        let mut untitled = nav();
        untitled.groups[1].title.clear();
        assert_eq!(nav_rows(&untitled, &collapsed(&[""])).len(), 7);
    }

    #[test]
    fn toggling_a_title_flips_it_alone() {
        let once = toggled(&collapsed(&["Look"]), "Fonts");
        assert_eq!(once, ["Look", "Fonts"]);
        assert_eq!(toggled(&once, "Look"), ["Fonts"]);
    }

    #[test]
    fn the_keyboard_steps_over_an_open_title() {
        let rows = nav_rows(&nav(), &[]);
        // Down from Labels onto Fonts: Spaces. Up from Spaces onto Fonts:
        // Labels.
        assert_eq!(title_to_open(&rows, 3, false), None);
        assert_eq!(keyboard_target(&rows, 3, false, None), Some(4));
        assert_eq!(keyboard_target(&rows, 3, true, None), Some(2));
        // Up from Appearance onto the first title turns back.
        assert_eq!(keyboard_target(&rows, 0, true, None), Some(1));
    }

    #[test]
    fn the_keyboard_opens_a_collapsed_group_it_moves_into() {
        let rows = nav_rows(&nav(), &collapsed(&["Fonts"]));
        // Down from Labels onto collapsed Fonts opens it at Spaces.
        assert_eq!(title_to_open(&rows, 3, false), Some("Fonts"));
        let opened = nav_rows(&nav(), &[]);
        assert_eq!(keyboard_target(&opened, 3, false, Some("Fonts")), Some(4));
        // Up from General onto DeskSpace, under collapsed Fonts, opens Fonts
        // at its last entry, Windows.
        assert_eq!(title_to_open(&rows, 4, true), Some("Fonts"));
        assert_eq!(keyboard_target(&opened, 6, true, Some("Fonts")), Some(5));
    }
}
