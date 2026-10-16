//! Item ordering for `Command::SortSpace`.

use std::cmp::Ordering;

use super::SortKey;
use crate::item::{Item, ItemKind};

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

    #[test]
    fn names_compare_case_insensitively_and_by_length() {
        assert_eq!(compare_names("apple", "Banana"), Ordering::Less);
        assert_eq!(compare_names("ABC", "abc"), Ordering::Equal);
        assert_eq!(compare_names("ab", "abc"), Ordering::Less);
    }
}
