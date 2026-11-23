//! Pure menu entry list builders and choice mappers over plain data.

use deskspace_core::source::FolderView;
use deskspace_core::{Column, GroupBy, SortRule, Space, SpaceView, Theme};
use deskspace_platform::MenuEntry;

use super::super::{
    COLUMN_MENU_IDS, GROUP_MENU_IDS, ICON_SIZE_PRESETS, MenuId, NEW_SHORTCUT_ENTRIES, NewShortcut,
    OPACITY_PRESETS, SORT_MENU_IDS, VIEW_PRESETS,
};
use crate::settings::{DETAILS_COLUMNS, GROUP_NONE, SORT_KEYS, VIEW_MODES};

/// A menu entry for one of the app's menu ids.
pub(in crate::app) fn menu_item(id: MenuId, label: impl Into<String>) -> MenuEntry {
    MenuEntry::item(id as u32, label)
}

/// The "Appearance" submenu for a space drawn with `theme`: tint-opacity
/// presets, the icon size, custom tint and text colors, a way back to the
/// global appearance, and the space's settings page. Every edit customizes
/// the space.
pub(super) fn space_appearance_menu(space: &Space, theme: &Theme) -> MenuEntry {
    let mut entries: Vec<MenuEntry> = OPACITY_PRESETS
        .iter()
        .map(|(id, label, value)| menu_item(*id, *label).checked(theme.tint_opacity == *value))
        .collect();
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::submenu(
        "Icon size",
        ICON_SIZE_PRESETS
            .iter()
            .map(|(id, label, value)| menu_item(*id, *label).checked(theme.icon_size == *value))
            .collect(),
    ));
    entries.push(MenuEntry::Separator);
    entries.push(menu_item(MenuId::SpaceTint, "Custom tint\u{2026}").checked(theme.custom_colors));
    entries.push(menu_item(MenuId::SpaceText, "Custom text\u{2026}").checked(theme.custom_colors));
    entries
        .push(menu_item(MenuId::ResetAppearance, "Use global appearance").enabled(space.custom()));
    entries.push(MenuEntry::Separator);
    entries.push(menu_item(MenuId::SpaceSettings, "Space settings\u{2026}"));
    MenuEntry::submenu("Appearance", entries)
}

/// The "New shortcut" submenu (F-53, F-54), from `NEW_SHORTCUT_ENTRIES`.
/// `can_paste` enables "Paste shortcut" when the clipboard holds shell items.
pub(super) fn new_shortcut_menu(can_paste: bool) -> MenuEntry {
    MenuEntry::submenu("New shortcut", new_shortcut_entries(can_paste))
}

/// The New shortcut choices themselves, for the submenu and the settings
/// window's + tile.
pub(super) fn new_shortcut_entries(can_paste: bool) -> Vec<MenuEntry> {
    let mut entries = Vec::with_capacity(NEW_SHORTCUT_ENTRIES.len() + 1);
    for (id, label, kind) in NEW_SHORTCUT_ENTRIES {
        if kind == NewShortcut::Paste {
            entries.push(MenuEntry::Separator);
        }
        let enabled = kind != NewShortcut::Paste || can_paste;
        entries.push(menu_item(id, label).enabled(enabled));
    }
    entries
}

/// What a "New shortcut" menu id makes, if it is one.
pub(super) fn new_shortcut_choice(choice: MenuId) -> Option<NewShortcut> {
    NEW_SHORTCUT_ENTRIES
        .iter()
        .find(|(id, ..)| *id == choice)
        .map(|(.., kind)| *kind)
}

/// The "View" submenu: one radio item per icon view, checking the space's
/// current mode, then the status bar and file-extension checks. The
/// extension entry checks what the space shows now, which `Automatic`
/// decides with Explorer's own option.
pub(super) fn space_view_menu(space: &Space, shows_extensions: bool) -> MenuEntry {
    let mut entries: Vec<MenuEntry> = VIEW_PRESETS
        .iter()
        .map(|(id, mode)| {
            let label = VIEW_MODES
                .iter()
                .find(|(row, ..)| row == mode)
                .map_or("", |(_, label, _)| *label);
            menu_item(*id, label).checked(space.view().mode == *mode)
        })
        .collect();
    entries.extend([
        MenuEntry::Separator,
        menu_item(MenuId::CompactView, "Compact view").checked(space.view().compact),
        menu_item(MenuId::StatusBar, "Status bar").checked(space.view().status_bar),
        menu_item(MenuId::FileExtensions, "File name extensions").checked(shows_extensions),
    ]);
    MenuEntry::submenu("View", entries)
}

/// The Details header's menu: a check per column, on for the ones `view` shows.
/// Name is always shown, so its entry is on and grey (D18).
pub(super) fn details_header_menu(view: &SpaceView) -> Vec<MenuEntry> {
    COLUMN_MENU_IDS
        .iter()
        .map(|(id, column)| {
            let label = DETAILS_COLUMNS
                .iter()
                .find(|row| row.id == *column)
                .map_or("", |row| row.label);
            let shown = view.columns.iter().any(|spec| spec.id == *column);
            menu_item(*id, label)
                .checked(shown)
                .enabled(*column != Column::Name)
        })
        .collect()
}

/// The column a Details header menu choice names, or `None` for any other id.
pub(super) fn column_choice(choice: MenuId) -> Option<Column> {
    COLUMN_MENU_IDS
        .iter()
        .find(|(id, _)| *id == choice)
        .map(|(_, column)| *column)
}

/// The "Sort by" submenu (D17): Explorer's four keys as radio items, the
/// direction, then Folders first and Keep sorted. A lens also gets its
/// hidden-files toggle, which DeskSpace has no other menu for.
pub(super) fn space_sort_menu(view: &SpaceView, lens: Option<&FolderView>) -> MenuEntry {
    let rule = view.sort;
    let mut entries: Vec<MenuEntry> = SORT_MENU_IDS
        .iter()
        .map(|(id, key)| {
            let label = SORT_KEYS
                .iter()
                .find(|(row, _)| row == key)
                .map_or("", |(_, label)| *label);
            menu_item(*id, label).checked(rule.key == *key)
        })
        .collect();
    entries.extend([
        MenuEntry::Separator,
        menu_item(MenuId::SortAscending, "Ascending").checked(!rule.descending),
        menu_item(MenuId::SortDescending, "Descending").checked(rule.descending),
        MenuEntry::Separator,
        menu_item(MenuId::FoldersFirst, "Folders first").checked(rule.folders_first),
        menu_item(MenuId::KeepSorted, "Keep sorted")
            .checked(lens.is_some() || view.keep_sorted)
            .enabled(lens.is_none()),
    ]);
    if let Some(view) = lens {
        entries.push(menu_item(MenuId::ShowHidden, "Show hidden files").checked(view.show_hidden));
    }
    MenuEntry::submenu("Sort by", entries)
}

/// The "Group by" submenu, as Explorer's: (None), the keys, then the direction,
/// which means nothing until a key is chosen.
pub(super) fn space_group_menu(view: &SpaceView) -> MenuEntry {
    let group = view.group;
    let mut entries = vec![menu_item(MenuId::GroupNone, GROUP_NONE).checked(group.is_none())];
    entries.extend(GROUP_MENU_IDS.iter().map(|(id, key)| {
        let label = SORT_KEYS
            .iter()
            .find(|(row, _)| row == key)
            .map_or("", |(_, label)| *label);
        menu_item(*id, label).checked(group.is_some_and(|group| group.key == *key))
    }));
    entries.extend([
        MenuEntry::Separator,
        menu_item(MenuId::GroupAscending, "Ascending")
            .checked(group.is_some_and(|group| !group.descending))
            .enabled(group.is_some()),
        menu_item(MenuId::GroupDescending, "Descending")
            .checked(group.is_some_and(|group| group.descending))
            .enabled(group.is_some()),
    ]);
    MenuEntry::submenu("Group by", entries)
}

/// The view a Group by choice leaves, or `None` when `choice` is not one. A key
/// keeps the direction, and a direction needs a key.
pub(super) fn group_after(view: &SpaceView, choice: MenuId) -> Option<SpaceView> {
    let with_group = |group| SpaceView {
        group,
        ..view.clone()
    };
    if let Some((_, key)) = GROUP_MENU_IDS.iter().find(|(id, _)| *id == choice) {
        let descending = view.group.is_some_and(|group| group.descending);
        return Some(with_group(Some(GroupBy {
            key: *key,
            descending,
        })));
    }
    let direction = |descending| {
        view.group.map(|group| {
            with_group(Some(GroupBy {
                descending,
                ..group
            }))
        })
    };
    match choice {
        MenuId::GroupNone => Some(with_group(None)),
        MenuId::GroupAscending => direction(false),
        MenuId::GroupDescending => direction(true),
        _ => None,
    }
}

/// The view a View ▸ choice leaves, or `None` when `choice` is not a view.
pub(super) fn view_after(view: &SpaceView, choice: MenuId) -> Option<SpaceView> {
    VIEW_PRESETS
        .iter()
        .find(|(preset, _)| *preset == choice)
        .map(|(_, mode)| view.with_mode(*mode))
}

/// The view a Keep sorted choice leaves, or `None` when it is not that
/// choice. A lens is always kept sorted, so its entry is greyed and changes
/// nothing (D17).
pub(super) fn keep_sorted_after(view: &SpaceView, choice: MenuId, lens: bool) -> Option<SpaceView> {
    if choice != MenuId::KeepSorted || lens {
        return None;
    }
    Some(SpaceView {
        keep_sorted: !view.keep_sorted,
        ..view.clone()
    })
}

/// The rule a Sort by choice leaves, or `None` when `choice` is not a Sort by
/// entry. A key keeps the direction and folders-first, a direction keeps the
/// key, and Folders first toggles.
pub(super) fn sort_rule_after(rule: SortRule, choice: MenuId) -> Option<SortRule> {
    if let Some((_, key)) = SORT_MENU_IDS.iter().find(|(id, _)| *id == choice) {
        return Some(SortRule { key: *key, ..rule });
    }
    match choice {
        MenuId::SortAscending => Some(SortRule {
            descending: false,
            ..rule
        }),
        MenuId::SortDescending => Some(SortRule {
            descending: true,
            ..rule
        }),
        MenuId::FoldersFirst => Some(SortRule {
            folders_first: !rule.folders_first,
            ..rule
        }),
        _ => None,
    }
}

/// "New space" and "New lens…", in that order, for every menu that creates
/// spaces: the space menu, the tray, and the desktop layer's menu.
pub(super) fn new_entries() -> [MenuEntry; 2] {
    [
        menu_item(MenuId::NewSpace, "New space"),
        menu_item(MenuId::NewLens, "New lens\u{2026}"),
    ]
}

/// A lens's own entries in its space menu.
pub(super) fn lens_folder_entries() -> [MenuEntry; 2] {
    [
        menu_item(MenuId::OpenFolder, "Open folder"),
        menu_item(MenuId::ChangeFolder, "Change folder\u{2026}"),
    ]
}

/// The view a lens's menu `choice` leaves it with, or `None` when the choice
/// does not change its folder view.
pub(super) fn lens_view_after(view: &FolderView, choice: MenuId) -> Option<FolderView> {
    let mut next = view.clone();
    match choice {
        MenuId::ShowHidden => next.show_hidden = !view.show_hidden,
        _ => return None,
    }
    Some(next)
}

#[cfg(test)]
mod tests {
    use deskspace_core::{Rect, SortKey};

    use super::*;

    #[test]
    fn the_new_shortcut_menu_maps_each_entry_back() {
        let MenuEntry::Submenu { label, entries, .. } = new_shortcut_menu(false) else {
            panic!("expected a submenu");
        };
        assert_eq!(label, "New shortcut");
        let ids: Vec<Option<MenuId>> = entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { id, .. } => Some(MenuId::from_id(*id)),
                _ => None,
            })
            .collect();
        assert_eq!(
            ids,
            [
                Some(MenuId::NewShortcutApps),
                Some(MenuId::NewShortcutCustom),
                Some(MenuId::PasteShortcut)
            ]
        );
        assert_eq!(
            new_shortcut_choice(MenuId::NewShortcutApps),
            Some(NewShortcut::Apps)
        );
        assert_eq!(
            new_shortcut_choice(MenuId::NewShortcutCustom),
            Some(NewShortcut::Custom)
        );
        assert_eq!(
            new_shortcut_choice(MenuId::PasteShortcut),
            Some(NewShortcut::Paste)
        );
        assert_eq!(new_shortcut_choice(MenuId::Fold), None);
    }

    /// The ids and check marks of `entries`' items, in order.
    fn item_checks(entries: &[MenuEntry]) -> Vec<(Option<MenuId>, bool)> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { id, checked, .. } => Some((MenuId::from_id(*id), *checked)),
                _ => None,
            })
            .collect()
    }

    fn sort_entries(view: &SpaceView, lens: Option<&FolderView>) -> Vec<MenuEntry> {
        match space_sort_menu(view, lens) {
            MenuEntry::Submenu { entries, .. } => entries,
            _ => panic!("expected a submenu"),
        }
    }

    fn view_entries(space: &Space, shows_extensions: bool) -> Vec<MenuEntry> {
        match space_view_menu(space, shows_extensions) {
            MenuEntry::Submenu { entries, .. } => entries,
            _ => panic!("expected a submenu"),
        }
    }

    #[test]
    fn the_view_menu_checks_the_space_mode_and_uses_the_table_labels() {
        use crate::fixtures::{apply, create_space};
        use deskspace_core::{Command, Desktop, ViewMode};

        let mut desktop = Desktop::new();
        let id = create_space(&mut desktop, "A", Rect::new(0, 0, 100, 100));
        apply(
            &mut desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView::default().with_mode(ViewMode::LargeIcons),
            },
        );
        let entries = view_entries(desktop.space(id).unwrap(), false);
        let checks = item_checks(&entries);
        assert_eq!(
            checks.len(),
            VIEW_PRESETS.len() + 3,
            "the views and the three checks"
        );
        assert_eq!(
            checks.iter().filter(|(_, checked)| *checked).count(),
            1,
            "one radio entry is checked"
        );
        assert_eq!(
            checks
                .iter()
                .find(|(_, checked)| *checked)
                .and_then(|(id, _)| *id),
            Some(MenuId::ViewLargeIcons)
        );
        let labels: Vec<&str> = entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { label, .. } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        let mut expected: Vec<&str> = VIEW_MODES.iter().map(|(_, label, _)| *label).collect();
        expected.push("Compact view");
        expected.push("Status bar");
        expected.push("File name extensions");
        assert_eq!(labels, expected);
    }

    #[test]
    fn the_view_menu_extension_check_follows_what_the_space_shows() {
        use crate::fixtures::create_space;
        use deskspace_core::Desktop;

        let mut desktop = Desktop::new();
        let id = create_space(&mut desktop, "A", Rect::new(0, 0, 100, 100));
        let space = desktop.space(id).unwrap();
        for (shows, checked) in [(true, true), (false, false)] {
            let entries = view_entries(space, shows);
            assert_eq!(
                item_checks(&entries)
                    .into_iter()
                    .find(|(id, _)| *id == Some(MenuId::FileExtensions))
                    .map(|(_, checked)| checked),
                Some(checked),
                "the check follows what the space shows now ({shows})"
            );
        }
    }

    #[test]
    fn every_view_has_one_menu_preset_and_one_mode_row() {
        assert_eq!(VIEW_PRESETS.len(), VIEW_MODES.len());
        for (mode, ..) in VIEW_MODES {
            assert_eq!(
                VIEW_PRESETS
                    .iter()
                    .filter(|(_, preset)| *preset == mode)
                    .count(),
                1,
                "{mode:?}"
            );
        }
        for (_, mode) in VIEW_PRESETS {
            assert_eq!(
                VIEW_MODES.iter().filter(|(row, ..)| *row == mode).count(),
                1,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn the_sort_menu_checks_the_rule_and_uses_explorer_order_and_words() {
        let rule = SortRule {
            key: SortKey::Modified,
            descending: true,
            folders_first: true,
        };
        let entries = sort_entries(
            &SpaceView {
                sort: rule,
                ..SpaceView::default()
            },
            None,
        );
        let labels: Vec<&str> = entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { label, .. } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        let mut expected: Vec<&str> = SORT_KEYS.iter().map(|(_, label)| *label).collect();
        expected.extend(["Ascending", "Descending", "Folders first", "Keep sorted"]);
        assert_eq!(labels, expected);

        let checked = |entries: &[MenuEntry]| -> Vec<MenuId> {
            item_checks(entries)
                .into_iter()
                .filter(|(_, checked)| *checked)
                .filter_map(|(id, _)| id)
                .collect()
        };
        assert_eq!(
            checked(&entries),
            [
                MenuId::SortByModified,
                MenuId::SortDescending,
                MenuId::FoldersFirst
            ]
        );
        assert_eq!(
            checked(&sort_entries(&SpaceView::default(), None)),
            [MenuId::SortByName, MenuId::SortAscending]
        );
    }

    #[test]
    fn a_lens_sort_menu_also_offers_show_hidden() {
        let view = FolderView {
            show_hidden: true,
            ..FolderView::default()
        };
        let checks = item_checks(&sort_entries(&SpaceView::default(), Some(&view)));
        assert!(checks.contains(&(Some(MenuId::ShowHidden), true)));
        let normal = item_checks(&sort_entries(&SpaceView::default(), None));
        assert!(
            !normal.iter().any(|(id, _)| *id == Some(MenuId::ShowHidden)),
            "a normal space has no hidden-files choice"
        );
    }

    /// Whether the item with `id` is enabled in `entries`.
    fn item_enabled(entries: &[MenuEntry], id: MenuId) -> Option<bool> {
        entries.iter().find_map(|entry| match entry {
            MenuEntry::Item {
                id: row, enabled, ..
            } if MenuId::from_id(*row) == Some(id) => Some(*enabled),
            _ => None,
        })
    }

    #[test]
    fn the_keep_sorted_entry_follows_the_view_and_greys_on_a_lens() {
        let kept = SpaceView {
            keep_sorted: true,
            ..SpaceView::default()
        };
        let checks = item_checks(&sort_entries(&kept, None));
        assert!(checks.contains(&(Some(MenuId::KeepSorted), true)));
        let normal = sort_entries(&SpaceView::default(), None);
        assert_eq!(item_enabled(&normal, MenuId::KeepSorted), Some(true));
        assert!(item_checks(&normal).contains(&(Some(MenuId::KeepSorted), false)));

        let lens = sort_entries(&kept, Some(&FolderView::default()));
        assert_eq!(item_enabled(&lens, MenuId::KeepSorted), Some(false));
        assert!(
            item_checks(&lens).contains(&(Some(MenuId::KeepSorted), true)),
            "a lens is always kept sorted"
        );
    }

    #[test]
    fn a_view_choice_keeps_the_sort_rule_and_the_status_bar() {
        let view = SpaceView {
            sort: SortRule {
                key: SortKey::Modified,
                descending: true,
                folders_first: false,
            },
            keep_sorted: false,
            status_bar: true,
            ..SpaceView::default()
        };
        let next = view_after(&view, MenuId::ViewDetails).unwrap();
        assert_eq!(next.mode, deskspace_core::ViewMode::Details);
        assert_eq!(next.sort, view.sort);
        assert!(!next.keep_sorted);
        assert!(next.status_bar);
        assert_eq!(view_after(&view, MenuId::KeepSorted), None);
    }

    #[test]
    fn the_group_menu_checks_the_current_grouping() {
        let entries = |view: &SpaceView| match space_group_menu(view) {
            MenuEntry::Submenu { entries, .. } => entries,
            _ => panic!("expected a submenu"),
        };
        let view = SpaceView::default();
        let checked = |entries: &[MenuEntry]| -> Vec<MenuId> {
            item_checks(entries)
                .into_iter()
                .filter(|(_, checked)| *checked)
                .filter_map(|(id, _)| id)
                .collect()
        };
        assert_eq!(checked(&entries(&view)), [MenuId::GroupNone]);
        assert_eq!(
            item_enabled(&entries(&view), MenuId::GroupAscending),
            Some(false),
            "no direction without a key"
        );

        let grouped = group_after(&view, MenuId::GroupBySize).unwrap();
        assert_eq!(
            checked(&entries(&grouped)),
            [MenuId::GroupBySize, MenuId::GroupAscending]
        );
        assert_eq!(
            item_enabled(&entries(&grouped), MenuId::GroupDescending),
            Some(true)
        );
    }

    #[test]
    fn a_group_choice_keeps_the_rest_of_the_view() {
        use deskspace_core::SortKey;

        let view = SpaceView {
            status_bar: true,
            ..SpaceView::default()
        };
        assert_eq!(group_after(&view, MenuId::Fold), None);
        assert_eq!(
            group_after(&view, MenuId::GroupDescending),
            None,
            "a direction needs a key"
        );
        let by_size = group_after(&view, MenuId::GroupBySize).unwrap();
        assert!(by_size.status_bar);
        let down = group_after(&by_size, MenuId::GroupDescending).unwrap();
        assert_eq!(
            down.group,
            Some(GroupBy {
                key: SortKey::Size,
                descending: true
            })
        );
        let by_name = group_after(&down, MenuId::GroupByName).unwrap();
        assert_eq!(
            by_name.group,
            Some(GroupBy {
                key: SortKey::Name,
                descending: true
            }),
            "a key keeps the direction"
        );
        assert_eq!(
            group_after(&by_name, MenuId::GroupNone).unwrap().group,
            None
        );
    }

    #[test]
    fn every_sort_key_can_group() {
        for (key, _) in SORT_KEYS {
            assert!(GROUP_MENU_IDS.iter().any(|(_, row)| *row == key), "{key:?}");
        }
    }

    #[test]
    fn the_header_menu_checks_the_columns_shown_and_greys_name() {
        let view = SpaceView::default();
        let entries = details_header_menu(&view);
        assert_eq!(
            item_checks(&entries),
            [
                (Some(MenuId::ColumnName), true),
                (Some(MenuId::ColumnModified), true),
                (Some(MenuId::ColumnKind), true),
                (Some(MenuId::ColumnSize), true),
                (Some(MenuId::ColumnCreated), false),
            ]
        );
        assert_eq!(item_enabled(&entries, MenuId::ColumnName), Some(false));
        assert_eq!(item_enabled(&entries, MenuId::ColumnSize), Some(true));

        let shown = view.with_column_toggled(Column::Created);
        let hidden = shown.with_column_toggled(Column::Size);
        assert_eq!(
            item_checks(&details_header_menu(&hidden)),
            [
                (Some(MenuId::ColumnName), true),
                (Some(MenuId::ColumnModified), true),
                (Some(MenuId::ColumnKind), true),
                (Some(MenuId::ColumnSize), false),
                (Some(MenuId::ColumnCreated), true),
            ]
        );
    }

    #[test]
    fn a_header_menu_choice_names_its_column() {
        assert_eq!(column_choice(MenuId::ColumnSize), Some(Column::Size));
        assert_eq!(column_choice(MenuId::ColumnCreated), Some(Column::Created));
        assert_eq!(column_choice(MenuId::Fold), None);
    }

    #[test]
    fn every_column_has_a_header_menu_entry() {
        for column in Column::ALL {
            assert!(
                COLUMN_MENU_IDS.iter().any(|(_, row)| *row == column),
                "{column:?}"
            );
        }
    }

    #[test]
    fn keep_sorted_toggles_a_normal_space_only() {
        let view = SpaceView::default();
        let on = keep_sorted_after(&view, MenuId::KeepSorted, false).unwrap();
        assert!(on.keep_sorted);
        assert_eq!(on.sort, view.sort, "the rule is kept");
        let off = keep_sorted_after(&on, MenuId::KeepSorted, false).unwrap();
        assert!(!off.keep_sorted);
        assert_eq!(keep_sorted_after(&view, MenuId::KeepSorted, true), None);
        assert_eq!(keep_sorted_after(&view, MenuId::FoldersFirst, false), None);
    }

    #[test]
    fn every_sort_key_has_one_menu_entry() {
        assert_eq!(SORT_KEYS.len(), SORT_MENU_IDS.len());
        let menu_keys: Vec<SortKey> = SORT_MENU_IDS.iter().map(|(_, key)| *key).collect();
        let table_keys: Vec<SortKey> = SORT_KEYS.iter().map(|(key, _)| *key).collect();
        assert_eq!(menu_keys, table_keys, "the menu follows SORT_KEYS order");
        for (key, _) in SORT_KEYS {
            assert_eq!(
                SORT_MENU_IDS.iter().filter(|(_, row)| *row == key).count(),
                1,
                "{key:?}"
            );
        }
        for (_, key) in SORT_MENU_IDS {
            assert_eq!(
                SORT_KEYS.iter().filter(|(row, _)| *row == key).count(),
                1,
                "{key:?}"
            );
        }
    }

    #[test]
    fn a_sort_choice_changes_only_its_part_of_the_rule() {
        let rule = SortRule {
            key: SortKey::Size,
            descending: true,
            folders_first: true,
        };
        assert_eq!(
            sort_rule_after(rule, MenuId::SortByName),
            Some(SortRule {
                key: SortKey::Name,
                ..rule
            })
        );
        let ascending = sort_rule_after(rule, MenuId::SortAscending).unwrap();
        assert_eq!(
            (ascending.key, ascending.descending),
            (SortKey::Size, false)
        );
        assert!(ascending.folders_first, "the direction keeps folders first");
        let descending = sort_rule_after(rule, MenuId::SortDescending).unwrap();
        assert!(descending.descending);
        let folders = sort_rule_after(rule, MenuId::FoldersFirst).unwrap();
        assert!(!folders.folders_first);
        assert_eq!((folders.key, folders.descending), (SortKey::Size, true));
        assert_eq!(sort_rule_after(rule, MenuId::Fold), None);
    }

    #[test]
    fn a_lenss_hidden_choice_changes_only_the_hidden_flag() {
        let view = FolderView {
            path: "C:/Users/me/Downloads".into(),
            ..FolderView::default()
        };
        let hidden = lens_view_after(&view, MenuId::ShowHidden).unwrap();
        assert!(hidden.show_hidden);
        assert_eq!(hidden.path, view.path);
        assert_eq!(lens_view_after(&view, MenuId::FoldersFirst), None);
        assert_eq!(lens_view_after(&view, MenuId::SortByName), None);
    }

    #[test]
    fn new_lens_follows_new_space() {
        let ids: Vec<Option<MenuId>> = item_checks(&new_entries())
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, [Some(MenuId::NewSpace), Some(MenuId::NewLens)]);
    }

    #[test]
    fn a_lens_menu_offers_its_folder() {
        let ids: Vec<Option<MenuId>> = item_checks(&lens_folder_entries())
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, [Some(MenuId::OpenFolder), Some(MenuId::ChangeFolder)]);
    }

    #[test]
    fn paste_shortcut_follows_a_separator_and_the_clipboard() {
        let empty = new_shortcut_entries(false);
        assert_eq!(empty[2], MenuEntry::Separator);
        assert_eq!(
            empty[3],
            menu_item(MenuId::PasteShortcut, "Paste shortcut").enabled(false)
        );
        let filled = new_shortcut_entries(true);
        assert_eq!(
            filled[3],
            menu_item(MenuId::PasteShortcut, "Paste shortcut").enabled(true)
        );
    }
}
