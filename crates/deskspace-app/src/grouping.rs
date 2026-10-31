//! Which group each item of a grouped space is in, with Explorer's group names
//! (D18). Pure: the type names and today's date are passed in.

use std::collections::BTreeMap;

use deskspace_core::grouping::LocalDay;
use deskspace_core::{GroupBy, Item, ItemKind, SortKey};

const SECONDS_PER_DAY: i64 = 86_400;
const KB: u64 = 1024;
const MB: u64 = 1024 * KB;
const GB: u64 = 1024 * MB;

/// Explorer's Name groups, by the label's first character.
const NAME_GROUPS: [(char, char, &str); 4] = [
    ('0', '9', "0 - 9"),
    ('A', 'H', "A - H"),
    ('I', 'P', "I - P"),
    ('Q', 'Z', "Q - Z"),
];

/// Explorer's Size groups, smallest first, with the most bytes each holds.
const SIZE_GROUPS: [(u64, &str); 6] = [
    (0, "Empty (0 KB)"),
    (16 * KB, "Tiny (0 - 16 KB)"),
    (MB, "Small (16 KB - 1 MB)"),
    (128 * MB, "Medium (1 - 128 MB)"),
    (GB, "Large (128 MB - 1 GB)"),
    (4 * GB, "Huge (1 - 4 GB)"),
];
const GIGANTIC: &str = "Gigantic (> 4 GB)";
/// Explorer's group for an item with no value for the key.
const UNSPECIFIED: &str = "Unspecified";
const OTHER: &str = "Other";

/// The date groups, oldest first.
const DATE_GROUPS: [&str; 8] = [
    "A long time ago",
    "Earlier this year",
    "Last month",
    "Earlier this month",
    "Last week",
    "Earlier this week",
    "Yesterday",
    "Today",
];

/// What grouping needs that the items do not say.
pub(crate) struct GroupFacts<'a> {
    pub(crate) today: LocalDay,
    /// An item's type name, from the shell's cache.
    pub(crate) type_name: &'a mut dyn FnMut(&Item) -> String,
}

/// The groups of `items`, in display order, each with the stored indices of its
/// items. Items keep their stored order within a group: a lens already holds its
/// sort rule, and a normal space its own arrangement or its last sort.
pub(crate) fn groups(
    items: &[Item],
    group_by: GroupBy,
    facts: &mut GroupFacts<'_>,
) -> Vec<(String, Vec<u32>)> {
    let bounds = DateBounds::new(facts.today);
    let mut groups: BTreeMap<(u32, String), Vec<u32>> = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        let key = match group_by.key {
            SortKey::Name => name_group(&item.label),
            SortKey::Size => size_group(item),
            SortKey::Kind => (0, (facts.type_name)(item)),
            SortKey::Modified => date_group(item.modified, &bounds),
            SortKey::Created => date_group(item.created, &bounds),
        };
        groups.entry(key).or_default().push(index as u32);
    }
    let groups = groups.into_iter().map(|((_, name), items)| (name, items));
    if group_by.descending {
        groups.rev().collect()
    } else {
        groups.collect()
    }
}

fn name_group(label: &str) -> (u32, String) {
    let first = label.chars().next().map(|first| first.to_ascii_uppercase());
    let found = first.and_then(|first| {
        NAME_GROUPS
            .iter()
            .position(|(low, high, _)| (*low..=*high).contains(&first))
    });
    match found {
        Some(rank) => (rank as u32, NAME_GROUPS[rank].2.to_owned()),
        None => (NAME_GROUPS.len() as u32, OTHER.to_owned()),
    }
}

fn size_group(item: &Item) -> (u32, String) {
    let size = item.size.filter(|_| item.kind != ItemKind::Folder);
    let Some(size) = size else {
        return (0, UNSPECIFIED.to_owned());
    };
    match SIZE_GROUPS.iter().position(|(most, _)| size <= *most) {
        Some(rank) => (rank as u32 + 1, SIZE_GROUPS[rank].1.to_owned()),
        None => (SIZE_GROUPS.len() as u32 + 1, GIGANTIC.to_owned()),
    }
}

fn date_group(seconds: Option<u64>, bounds: &DateBounds) -> (u32, String) {
    let Some(seconds) = seconds else {
        return (0, UNSPECIFIED.to_owned());
    };
    let rank = bounds.date_rank(seconds);
    (rank as u32 + 1, DATE_GROUPS[rank].to_owned())
}

/// Where each date group begins, as Unix times, newest first.
struct DateBounds {
    /// Parallel to `DATE_GROUPS` reversed, without "A long time ago".
    starts: [i64; 7],
}

impl DateBounds {
    fn new(today: LocalDay) -> Self {
        let day = days_from_civil(today.year, today.month, today.day);
        let midnight = today.midnight as i64;
        let start = |date: i64| midnight - (day - date) * SECONDS_PER_DAY;
        let week = day - i64::from(today.weekday);
        let (last_year, last_month) = if today.month == 1 {
            (today.year - 1, 12)
        } else {
            (today.year, today.month - 1)
        };
        Self {
            starts: [
                midnight,
                start(day - 1),
                start(week),
                start(week - 7),
                start(days_from_civil(today.year, today.month, 1)),
                start(days_from_civil(last_year, last_month, 1)),
                start(days_from_civil(today.year, 1, 1)),
            ],
        }
    }

    /// The index in `DATE_GROUPS`. A time after today counts as today.
    fn date_rank(&self, seconds: u64) -> usize {
        let seconds = i64::try_from(seconds).unwrap_or(i64::MAX);
        self.starts
            .iter()
            .position(|start| seconds >= *start)
            .map_or(0, |newest_first| DATE_GROUPS.len() - 1 - newest_first)
    }
}

/// Days since the Unix epoch of a date in the proleptic Gregorian calendar.
/// https://howardhinnant.github.io/date_algorithms.html#days_from_civil
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use deskspace_core::{ItemFacts, ItemId};

    use super::*;

    /// Wednesday 2026-10-14, whose local midnight is the Unix time of its date.
    fn wednesday() -> LocalDay {
        day(2026, 10, 14, 3)
    }

    fn day(year: i32, month: u32, day: u32, weekday: u32) -> LocalDay {
        LocalDay {
            year,
            month,
            day,
            weekday,
            midnight: (days_from_civil(year, month, day) * SECONDS_PER_DAY) as u64,
        }
    }

    fn at(year: i32, month: u32, day: u32) -> u64 {
        (days_from_civil(year, month, day) * SECONDS_PER_DAY) as u64 + 3600
    }

    fn item(id: u64, name: &str, facts: ItemFacts) -> Item {
        Item::new(ItemId(id), name, facts)
    }

    fn file(id: u64, name: &str) -> Item {
        item(id, name, ItemFacts::default())
    }

    /// The groups' names and sizes under `key`.
    fn named(items: &[Item], key: SortKey, descending: bool) -> Vec<(String, Vec<u32>)> {
        let mut type_name = |item: &Item| {
            item.path
                .extension()
                .map_or("File", |_| "Text Document")
                .to_owned()
        };
        groups(
            items,
            GroupBy { key, descending },
            &mut GroupFacts {
                today: wednesday(),
                type_name: &mut type_name,
            },
        )
    }

    fn names(items: &[Item], key: SortKey, descending: bool) -> Vec<String> {
        named(items, key, descending)
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    fn date_name(today: LocalDay, seconds: u64) -> &'static str {
        DATE_GROUPS[DateBounds::new(today).date_rank(seconds)]
    }

    #[test]
    fn an_empty_space_has_no_groups() {
        for key in [
            SortKey::Name,
            SortKey::Kind,
            SortKey::Size,
            SortKey::Modified,
            SortKey::Created,
        ] {
            assert!(named(&[], key, false).is_empty(), "{key:?}");
        }
    }

    #[test]
    fn name_groups_are_explorers_ranges_in_both_directions() {
        let items = [
            file(1, "zebra"),
            file(2, "apple"),
            file(3, "9 lives"),
            file(4, "Mango"),
            file(5, "ñu"),
            file(6, "banana"),
        ];
        assert_eq!(
            names(&items, SortKey::Name, false),
            ["0 - 9", "A - H", "I - P", "Q - Z", "Other"]
        );
        assert_eq!(
            names(&items, SortKey::Name, true),
            ["Other", "Q - Z", "I - P", "A - H", "0 - 9"]
        );
    }

    #[test]
    fn a_groups_items_stay_together_in_their_stored_order() {
        let items = [
            file(1, "banana"),
            file(2, "zebra"),
            file(3, "apple"),
            file(4, "yak"),
        ];
        assert_eq!(
            named(&items, SortKey::Name, false),
            [
                ("A - H".to_owned(), vec![0, 2]),
                ("Q - Z".to_owned(), vec![1, 3])
            ]
        );
    }

    #[test]
    fn type_groups_are_the_shells_names_in_order() {
        let items = [file(1, "a.txt"), file(2, "plain"), file(3, "b.txt")];
        assert_eq!(
            named(&items, SortKey::Kind, false),
            [
                ("File".to_owned(), vec![1]),
                ("Text Document".to_owned(), vec![0, 2])
            ]
        );
    }

    #[test]
    fn size_groups_run_from_empty_to_gigantic() {
        let sized = |id, size| {
            item(
                id,
                "a.bin",
                ItemFacts {
                    size: Some(size),
                    ..ItemFacts::default()
                },
            )
        };
        let items = [
            sized(1, 5 * GB),
            sized(2, 0),
            sized(3, 16 * KB),
            sized(4, 16 * KB + 1),
            sized(5, 128 * MB),
            sized(6, GB),
            sized(7, 4 * GB),
            file(8, "unknown"),
        ];
        assert_eq!(
            names(&items, SortKey::Size, false),
            [
                "Unspecified",
                "Empty (0 KB)",
                "Tiny (0 - 16 KB)",
                "Small (16 KB - 1 MB)",
                "Medium (1 - 128 MB)",
                "Large (128 MB - 1 GB)",
                "Huge (1 - 4 GB)",
                "Gigantic (> 4 GB)",
            ]
        );
    }

    #[test]
    fn a_folder_has_no_size_group() {
        let folder = item(
            1,
            "folder",
            ItemFacts {
                is_folder: true,
                size: Some(0),
                ..ItemFacts::default()
            },
        );
        assert_eq!(names(&[folder], SortKey::Size, false), ["Unspecified"]);
    }

    #[test]
    fn date_groups_cover_a_week_a_month_and_a_year() {
        let today = wednesday();
        for (seconds, name) in [
            (at(2026, 10, 14), "Today"),
            (at(2026, 10, 13), "Yesterday"),
            (at(2026, 10, 12), "Earlier this week"),
            (at(2026, 10, 11), "Earlier this week"),
            (at(2026, 10, 10), "Last week"),
            (at(2026, 10, 4), "Last week"),
            (at(2026, 10, 3), "Earlier this month"),
            (at(2026, 10, 1), "Earlier this month"),
            (at(2026, 9, 30), "Last month"),
            (at(2026, 9, 1), "Last month"),
            (at(2026, 8, 31), "Earlier this year"),
            (at(2026, 1, 1), "Earlier this year"),
            (at(2025, 12, 31), "A long time ago"),
            (0, "A long time ago"),
        ] {
            assert_eq!(date_name(today, seconds), name, "{seconds}");
        }
    }

    #[test]
    fn a_date_after_today_counts_as_today() {
        assert_eq!(date_name(wednesday(), at(2030, 1, 1)), "Today");
        assert_eq!(date_name(wednesday(), u64::MAX), "Today");
    }

    #[test]
    fn date_groups_cross_a_new_year() {
        // Friday 2027-01-01: December is last month, and last year's rest is
        // a long time ago.
        let today = day(2027, 1, 1, 5);
        assert_eq!(date_name(today, at(2026, 12, 31)), "Yesterday");
        assert_eq!(date_name(today, at(2026, 12, 28)), "Earlier this week");
        assert_eq!(date_name(today, at(2026, 12, 20)), "Last week");
        assert_eq!(date_name(today, at(2026, 12, 2)), "Last month");
        assert_eq!(date_name(today, at(2026, 11, 30)), "A long time ago");
    }

    #[test]
    fn a_week_begun_last_month_stays_this_week() {
        // Friday 2026-05-01's week began in April.
        let today = day(2026, 5, 1, 5);
        assert_eq!(date_name(today, at(2026, 4, 27)), "Earlier this week");
        assert_eq!(date_name(today, at(2026, 4, 20)), "Last week");
        assert_eq!(date_name(today, at(2026, 4, 2)), "Last month");
    }

    #[test]
    fn an_unknown_date_is_unspecified_and_sorts_first() {
        let dated = item(
            1,
            "a",
            ItemFacts {
                modified: Some(at(2026, 10, 14)),
                ..ItemFacts::default()
            },
        );
        assert_eq!(
            names(&[dated, file(2, "b")], SortKey::Modified, false),
            ["Unspecified", "Today"]
        );
    }

    #[test]
    fn a_date_groups_order_follows_the_direction() {
        let dated = |id, year| {
            item(
                id,
                "a",
                ItemFacts {
                    created: Some(at(year, 10, 14)),
                    ..ItemFacts::default()
                },
            )
        };
        let items = [dated(1, 2026), dated(2, 2020)];
        assert_eq!(
            names(&items, SortKey::Created, false),
            ["A long time ago", "Today"]
        );
        assert_eq!(
            names(&items, SortKey::Created, true),
            ["Today", "A long time ago"]
        );
    }

    /// Run by hand in a release build: `cargo test --release -p deskspace-app
    /// time_grouping -- --ignored --nocapture`.
    #[test]
    #[ignore = "a timing, not a check"]
    fn time_grouping_fifty_thousand_items() {
        let items: Vec<Item> = (0..50_000)
            .map(|index| {
                item(
                    index,
                    &format!("file{index}.ext{}", index % 40),
                    ItemFacts {
                        size: Some(index * 1000),
                        modified: Some(1_700_000_000 + index * 600),
                        ..ItemFacts::default()
                    },
                )
            })
            .collect();
        for key in [
            SortKey::Name,
            SortKey::Kind,
            SortKey::Size,
            SortKey::Modified,
        ] {
            let started = std::time::Instant::now();
            let result = named(&items, key, false);
            eprintln!("{key:?}: {:?}, {} groups", started.elapsed(), result.len());
        }
    }
}
