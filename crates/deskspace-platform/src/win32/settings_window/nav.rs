//! The settings window's navigation rail and scopes pane: rows, selection,
//! and the events they report.

use super::commands::show_page;
use super::*;

/// One row of the navigation rail. Group titles are non-selectable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NavRow {
    Title(String),
    /// `nested` entries sit under a group title and are indented below it.
    Entry {
        label: String,
        marked: bool,
        nested: bool,
    },
}

impl NavRow {
    pub(super) fn label(&self) -> &str {
        match self {
            Self::Title(title) => title,
            Self::Entry { label, .. } => label,
        }
    }

    pub(super) fn is_entry(&self) -> bool {
        matches!(self, Self::Entry { .. })
    }
}

/// The rail's rows: each group's title (untitled groups have none), then its
/// entries.
pub(super) fn nav_rows(nav: &SettingsNav) -> Vec<NavRow> {
    let mut rows = Vec::new();
    for group in &nav.groups {
        if !group.title.is_empty() {
            rows.push(NavRow::Title(group.title.clone()));
        }
        let nested = !group.title.is_empty();
        rows.extend(group.entries.iter().map(|entry| NavRow::Entry {
            label: entry.label.clone(),
            marked: entry.marked,
            nested,
        }));
    }
    rows
}

/// The flattened entry index of rail row `row`, or `None` for a title.
fn row_entry(rows: &[NavRow], row: usize) -> Option<usize> {
    if !rows.get(row)?.is_entry() {
        return None;
    }
    Some(rows[..row].iter().filter(|row| row.is_entry()).count())
}

/// The rail row showing flattened entry `entry`, or the first entry row.
pub(super) fn entry_row(rows: &[NavRow], entry: usize) -> usize {
    let mut seen = 0;
    for (index, row) in rows.iter().enumerate() {
        if row.is_entry() {
            if seen == entry {
                return index;
            }
            seen += 1;
        }
    }
    rows.iter().position(NavRow::is_entry).unwrap_or(0)
}

/// Handles a rail selection change. Group titles are not selectable: landing
/// on one moves the selection to the nearest entry. Shows that entry's page,
/// then tells the app last, because the handler may rebuild the form.
pub(super) unsafe fn on_nav_changed(window: HWND) {
    // SAFETY: the list and pages are live children of `window`; the state
    // borrow ends before the list messages and the dispatch.
    unsafe {
        let Some((Some(list), rows, pages, nav)) = with_state(window, |state| {
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
        if row_entry(&rows, selected).is_none() {
            let Some(target) = (selected + 1..rows.len())
                .chain((0..selected).rev())
                .find(|row| rows[*row].is_entry())
            else {
                return;
            };
            SendMessageW(list, LB_SETCURSEL, Some(WPARAM(target)), None);
            let _ = InvalidateRect(Some(list), None, true);
            selected = target;
        }
        let entry = row_entry(&rows, selected).expect("moved to an entry row");
        if let Some(page) = nav.page_of(entry) {
            show_page(&pages, page);
        }
        settings_event(SettingsEvent::NavChanged { entry });
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

    fn nav() -> SettingsNav {
        SettingsNav {
            groups: vec![
                SettingsNavGroup {
                    title: "Settings".into(),
                    entries: vec![
                        SettingsNavEntry {
                            label: "Appearance".into(),
                            page: 0,
                            marked: false,
                        },
                        SettingsNavEntry {
                            label: "General".into(),
                            page: 5,
                            marked: false,
                        },
                    ],
                },
                SettingsNavGroup {
                    title: "Spaces".into(),
                    entries: vec![SettingsNavEntry {
                        label: "Work".into(),
                        page: 7,
                        marked: true,
                    }],
                },
            ],
            selected: 1,
            ..Default::default()
        }
    }

    #[test]
    fn nav_rows_put_a_title_before_each_group() {
        let rows = nav_rows(&nav());
        let labels: Vec<&str> = rows.iter().map(NavRow::label).collect();
        assert_eq!(
            labels,
            ["Settings", "Appearance", "General", "Spaces", "Work"]
        );
        assert!(!rows[0].is_entry());
        assert!(rows[1].is_entry());
    }

    #[test]
    fn untitled_groups_have_no_title_row() {
        let mut nav = nav();
        nav.groups[0].title.clear();
        let rows = nav_rows(&nav);
        let labels: Vec<&str> = rows.iter().map(NavRow::label).collect();
        assert_eq!(labels, ["Appearance", "General", "Spaces", "Work"]);
        assert_eq!(row_entry(&rows, 0), Some(0));
        assert_eq!(entry_row(&rows, 2), 3);
    }

    #[test]
    fn nav_row_entries_map_to_flattened_indices() {
        let rows = nav_rows(&nav());
        assert_eq!(row_entry(&rows, 0), None);
        assert_eq!(row_entry(&rows, 1), Some(0));
        assert_eq!(row_entry(&rows, 2), Some(1));
        assert_eq!(row_entry(&rows, 3), None);
        assert_eq!(row_entry(&rows, 4), Some(2));
        // The row showing a flattened entry, and the fallback past the end.
        assert_eq!(entry_row(&rows, 2), 4);
        assert_eq!(entry_row(&rows, 9), 1);
    }

    #[test]
    fn only_entries_under_a_title_are_nested() {
        let mut nav = SettingsNav {
            groups: vec![
                SettingsNavGroup {
                    title: String::new(),
                    entries: vec![SettingsNavEntry {
                        label: "General".into(),
                        page: 0,
                        marked: false,
                    }],
                },
                SettingsNavGroup {
                    title: "Animations".into(),
                    entries: vec![SettingsNavEntry {
                        label: "Fold".into(),
                        page: 1,
                        marked: false,
                    }],
                },
            ],
            ..Default::default()
        };
        let nested = |rows: &[NavRow]| -> Vec<bool> {
            rows.iter()
                .filter_map(|row| match row {
                    NavRow::Entry { nested, .. } => Some(*nested),
                    NavRow::Title(_) => None,
                })
                .collect()
        };
        assert_eq!(nested(&nav_rows(&nav)), [false, true]);
        nav.groups[1].title.clear();
        assert_eq!(nested(&nav_rows(&nav)), [false, false]);
    }

    #[test]
    fn nav_row_helpers_tolerate_empty_groups() {
        let rows = nav_rows(&SettingsNav {
            groups: vec![SettingsNavGroup {
                title: "Spaces".into(),
                entries: vec![],
            }],
            selected: 0,
            ..Default::default()
        });
        assert_eq!(rows.len(), 1);
        assert_eq!(row_entry(&rows, 0), None);
        assert_eq!(entry_row(&rows, 0), 0);
        assert_eq!(row_entry(&rows, 1), None);
    }
}
