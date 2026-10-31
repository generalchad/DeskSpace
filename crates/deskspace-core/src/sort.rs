//! One item ordering for spaces and lenses (ROADMAP D17).
//!
//! A space keeps its rule and a lens always uses its own, so both kinds sort
//! through `SortRule::compare`: there is no separate lens comparator.

use std::cmp::Ordering;
use std::iter::Peekable;

use serde::{Deserialize, Serialize};

use crate::item::{Item, ItemKind, SortKey};

/// How a space orders its items: the key, its direction, and whether folders
/// stay above files. It lives in `SpaceView`, so every space has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SortRule {
    /// The item field the rule compares first.
    pub key: SortKey,
    /// Whether the key's order runs from high to low.
    pub descending: bool,
    /// Keep folders above files whatever the key and direction say.
    pub folders_first: bool,
}

impl SortRule {
    /// Whether this is the default rule, which the file leaves out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Sorts `items` in place by this rule.
    pub fn sort_items(&self, items: &mut [Item]) {
        items.sort_by(|a, b| self.compare(a, b));
    }

    /// Orders two items by the rule. Ties use natural name order, so `file2`
    /// comes before `file10`. "Folders first" holds in both directions, as
    /// Explorer's does: it is a group order, not part of the key.
    pub fn compare(&self, a: &Item, b: &Item) -> Ordering {
        if self.folders_first {
            let rank = |item: &Item| u8::from(item.kind != ItemKind::Folder);
            let order = rank(a).cmp(&rank(b));
            if order != Ordering::Equal {
                return order;
            }
        }
        let by_name = || compare_natural(&a.label, &b.label);
        let order = match self.key {
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
            SortKey::Created => a
                .created
                .unwrap_or(0)
                .cmp(&b.created.unwrap_or(0))
                .then_with(by_name),
        };
        if self.descending {
            order.reverse()
        } else {
            order
        }
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

    fn item(label: &str, kind: ItemKind, size: u64, modified: u64) -> Item {
        Item::new(
            ItemId(1),
            format!("C:/folder/{label}"),
            ItemFacts {
                is_folder: kind == ItemKind::Folder,
                size: Some(size),
                modified: Some(modified),
                ..ItemFacts::default()
            },
        )
        .with_label(label)
        .with_kind(kind)
    }

    fn rule(key: SortKey, descending: bool, folders_first: bool) -> SortRule {
        SortRule {
            key,
            descending,
            folders_first,
        }
    }

    #[test]
    fn the_default_rule_is_name_ascending() {
        assert_eq!(SortRule::default().key, SortKey::Name);
        assert!(!SortRule::default().descending);
        assert!(!SortRule::default().folders_first);
        assert!(SortRule::default().is_default());
    }

    #[test]
    fn each_key_orders_by_its_field_both_ways() {
        let small = item("apple", ItemKind::File, 10, 1);
        let large = item("banana", ItemKind::File, 300, 2);

        assert_eq!(
            rule(SortKey::Name, false, false).compare(&small, &large),
            Ordering::Less
        );
        assert_eq!(
            rule(SortKey::Name, true, false).compare(&small, &large),
            Ordering::Greater
        );
        assert_eq!(
            rule(SortKey::Size, false, false).compare(&small, &large),
            Ordering::Less
        );
        assert_eq!(
            rule(SortKey::Size, true, false).compare(&small, &large),
            Ordering::Greater
        );
        assert_eq!(
            rule(SortKey::Modified, false, false).compare(&small, &large),
            Ordering::Less
        );
        assert_eq!(
            rule(SortKey::Modified, true, false).compare(&small, &large),
            Ordering::Greater
        );
    }

    #[test]
    fn created_orders_by_creation_time_both_ways() {
        let created = |label: &str, created: u64, modified: u64| Item {
            created: Some(created),
            ..item(label, ItemKind::File, 10, modified)
        };
        let older = created("banana", 1, 9);
        let newer = created("apple", 2, 1);
        assert_eq!(
            rule(SortKey::Created, false, false).compare(&older, &newer),
            Ordering::Less,
            "the creation time decides, not the name or the modified time"
        );
        assert_eq!(
            rule(SortKey::Created, true, false).compare(&older, &newer),
            Ordering::Greater
        );
        let unknown = Item {
            created: None,
            ..older.clone()
        };
        assert_eq!(
            rule(SortKey::Created, false, false).compare(&unknown, &newer),
            Ordering::Less,
            "an unknown time sorts as the oldest, as Modified does"
        );
    }

    #[test]
    fn kind_orders_folders_then_files_both_ways() {
        let folder = item("z-folder", ItemKind::Folder, 0, 0);
        let file = item("a-file", ItemKind::File, 0, 0);
        assert_eq!(
            rule(SortKey::Kind, false, false).compare(&folder, &file),
            Ordering::Less
        );
        assert_eq!(
            rule(SortKey::Kind, true, false).compare(&folder, &file),
            Ordering::Greater
        );
    }

    #[test]
    fn folders_first_holds_in_both_directions() {
        let folder = item("z-folder", ItemKind::Folder, 0, 0);
        let file = item("a-file", ItemKind::File, 0, 0);
        for descending in [false, true] {
            assert_eq!(
                rule(SortKey::Name, descending, true).compare(&folder, &file),
                Ordering::Less,
                "folders stay first, descending {descending}"
            );
            assert_eq!(
                rule(SortKey::Name, descending, true).compare(&file, &folder),
                Ordering::Greater
            );
        }
    }

    #[test]
    fn ties_use_natural_name_order() {
        let file2 = item("file2", ItemKind::File, 10, 1);
        let file10 = item("file10", ItemKind::File, 10, 1);
        assert_eq!(
            rule(SortKey::Size, false, false).compare(&file2, &file10),
            Ordering::Less
        );
        assert_eq!(
            compare_natural("File2", "file10"),
            Ordering::Less,
            "natural order is case-insensitive"
        );
        assert_eq!(compare_natural("file02", "file2"), Ordering::Equal);
    }

    #[test]
    fn names_compare_without_case() {
        assert_eq!(compare_natural("apple", "Banana"), Ordering::Less);
        assert_eq!(compare_natural("ABC", "abc"), Ordering::Equal);
        assert_eq!(compare_natural("ab", "abc"), Ordering::Less);
    }
}
