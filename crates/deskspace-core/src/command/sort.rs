//! Item ordering for `Command::SortSpace` and a lens's `FolderView`.

use std::cmp::Ordering;
use std::iter::Peekable;

use super::SortKey;
use crate::item::{Item, ItemKind};
use crate::source::FolderView;

/// Orders two items by `key`, ascending. Every key but `Name` breaks ties by
/// name, so the order is stable for the user.
pub(super) fn compare_items(key: SortKey, a: &Item, b: &Item) -> Ordering {
    let by_name = || compare_names(&a.label, &b.label);
    match key {
        SortKey::Name => by_name(),
        SortKey::Kind => kind_rank(a.kind).cmp(&kind_rank(b.kind)).then_with(by_name),
        SortKey::Size => a
            .size
            .unwrap_or(0)
            .cmp(&b.size.unwrap_or(0))
            .then_with(by_name),
        SortKey::Modified => a
            .modified
            .unwrap_or(0)
            .cmp(&b.modified.unwrap_or(0))
            .then_with(by_name),
    }
}

/// Orders two lens items by the view's settings. Folders stay first even when
/// the sort is descending, as Explorer's "Folders first" does. Names use
/// natural order, so `file2` comes before `file10`.
pub(super) fn compare_lens_items(view: &FolderView, a: &Item, b: &Item) -> Ordering {
    if view.folders_first {
        let rank = |item: &Item| u8::from(item.kind != ItemKind::Folder);
        let order = rank(a).cmp(&rank(b));
        if order != Ordering::Equal {
            return order;
        }
    }
    let by_name = || compare_natural(&a.label, &b.label);
    let order = match view.sort {
        SortKey::Name => by_name(),
        SortKey::Kind => kind_rank(a.kind).cmp(&kind_rank(b.kind)).then_with(by_name),
        SortKey::Size => a
            .size
            .unwrap_or(0)
            .cmp(&b.size.unwrap_or(0))
            .then_with(by_name),
        SortKey::Modified => a
            .modified
            .unwrap_or(0)
            .cmp(&b.modified.unwrap_or(0))
            .then_with(by_name),
    };
    if view.descending {
        order.reverse()
    } else {
        order
    }
}

/// Case-insensitive natural order: runs of digits compare by value, so `2`
/// comes before `10`, while the rest compares as case-folded characters.
fn compare_natural(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().flat_map(char::to_lowercase).peekable();
    let mut right = b.chars().flat_map(char::to_lowercase).peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let order = compare_numbers(&take_number(&mut left), &take_number(&mut right));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(_), Some(_)) => {
                let order = left.next().cmp(&right.next());
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

/// Takes the next run of ASCII digits. The caller has peeked a digit.
fn take_number(chars: &mut Peekable<impl Iterator<Item = char>>) -> String {
    let mut digits = String::new();
    while let Some(&digit) = chars.peek() {
        if !digit.is_ascii_digit() {
            break;
        }
        digits.push(digit);
        chars.next();
    }
    digits
}

/// Compares two digit runs by value without parsing: leading zeros are
/// ignored, then the longer run is larger, then the bytes decide.
fn compare_numbers(a: &str, b: &str) -> Ordering {
    let a = a.trim_start_matches('0');
    let b = b.trim_start_matches('0');
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// Case-insensitive label order, comparing lowered characters without
/// allocating.
fn compare_names(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().flat_map(char::to_lowercase);
    let mut right = b.chars().flat_map(char::to_lowercase);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => match x.cmp(&y) {
                Ordering::Equal => continue,
                other => return other,
            },
        }
    }
}

/// Group order for `SortKey::Kind`: folders, files, shortcuts, drives, other.
fn kind_rank(kind: ItemKind) -> u8 {
    match kind {
        ItemKind::Folder => 0,
        ItemKind::File => 1,
        ItemKind::Shortcut => 2,
        ItemKind::Drive => 3,
        ItemKind::Other => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::ItemId;
    use crate::item::ItemFacts;

    fn item(label: &str, kind: ItemKind) -> Item {
        Item::new(
            ItemId(1),
            format!("C:/folder/{label}"),
            ItemFacts::default(),
        )
        .with_label(label)
        .with_kind(kind)
    }

    #[test]
    fn names_compare_case_insensitively_and_by_length() {
        assert_eq!(compare_names("apple", "Banana"), Ordering::Less);
        assert_eq!(compare_names("ABC", "abc"), Ordering::Equal);
        assert_eq!(compare_names("ab", "abc"), Ordering::Less);
    }

    #[test]
    fn natural_order_puts_numbers_in_value_order() {
        assert_eq!(compare_natural("file2", "file10"), Ordering::Less);
        assert_eq!(compare_natural("File2", "file10"), Ordering::Less);
        assert_eq!(compare_natural("file02", "file2"), Ordering::Equal);
        assert_eq!(compare_natural("a", "b"), Ordering::Less);
    }

    #[test]
    fn lens_sort_keeps_folders_first_both_ways() {
        let view = FolderView {
            folders_first: true,
            ..FolderView::default()
        };
        let folder = item("z-folder", ItemKind::Folder);
        let file = item("a-file", ItemKind::File);
        assert_eq!(compare_lens_items(&view, &folder, &file), Ordering::Less);
        let descending = FolderView {
            descending: true,
            ..view.clone()
        };
        assert_eq!(
            compare_lens_items(&descending, &folder, &file),
            Ordering::Less
        );
    }

    #[test]
    fn lens_sort_descends_within_the_group() {
        let view = FolderView {
            descending: true,
            ..FolderView::default()
        };
        assert_eq!(
            compare_lens_items(
                &view,
                &item("a", ItemKind::File),
                &item("b", ItemKind::File)
            ),
            Ordering::Greater
        );
    }
}
