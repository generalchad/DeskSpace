//! Transient per-space view state and cached groupings.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use deskspace_core::grouping::{Grouping, LocalDay};
use deskspace_core::{GroupBy, Item, SpaceId};

/// Per-space view state that is not persisted, keyed by `SpaceId`.
#[derive(Debug, Default)]
pub(crate) struct ViewCache {
    /// Scroll offset in pixels. May exceed the current maximum after a
    /// resize, so the app reads it through `App::scroll_of`, which clamps.
    scroll: HashMap<SpaceId, i32>,
    /// Sideways offset of a Details header in pixels. May exceed the current
    /// maximum after a resize, so the scene reads it through
    /// `details_max_scroll`, which clamps.
    scroll_x: HashMap<SpaceId, i32>,
    /// Each space's icon factor at its last re-flow: a fixed view's factor,
    /// or `App::icon_scale`'s, without DPI. A change to it can keep the
    /// space's size first (`App::keep_size_on_zoom`).
    icon_scales: HashMap<SpaceId, f32>,
    /// A grouped space's display order, until what it was made from changes.
    pub(super) groupings: HashMap<SpaceId, CachedGrouping>,
    /// The names of each space's folded groups, for the session (D18).
    pub(super) folded_groups: HashMap<SpaceId, HashSet<String>>,
}

/// A display order and a digest of what it was made from.
#[derive(Debug)]
pub(super) struct CachedGrouping {
    pub(super) stamp: u64,
    pub(super) grouping: Arc<Grouping>,
}

/// A word-at-a-time digest, because every repaint of a grouped space pays for
/// one and SipHash took about 5 ms on 50,000 items. Nothing here is keyed by
/// outside input, so it needs no defense against collisions on purpose.
/// Adapted from rustc-hash: https://github.com/rust-lang/rustc-hash
#[derive(Default)]
struct StampHasher(u64);

impl StampHasher {
    const ROTATE: u32 = 5;
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    fn word(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(Self::ROTATE) ^ word).wrapping_mul(Self::SEED);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        // The length keeps "ab", "c" apart from "a", "bc".
        self.word(bytes.len() as u64);
        let (words, rest) = bytes.as_chunks::<8>();
        for word in words {
            self.word(u64::from_le_bytes(*word));
        }
        let mut last = [0; 8];
        last[..rest.len()].copy_from_slice(rest);
        self.word(u64::from_le_bytes(last));
    }

    fn optional(&mut self, value: Option<u64>) {
        self.word(u64::from(value.is_some()));
        self.word(value.unwrap_or(0));
    }
}

/// A digest of everything a space's groups read: its items and their facts, the
/// grouping, today, and what is folded. Comparing it costs a walk of the items
/// but no sorting and no shell calls, and any change to a space reaches it
/// without each change having to say so.
pub(super) fn grouping_stamp(
    items: &[Item],
    group: GroupBy,
    today: LocalDay,
    folded: &HashSet<String>,
) -> u64 {
    let mut stamp = StampHasher::default();
    stamp.word(group.key as u64);
    stamp.word(u64::from(group.descending));
    stamp.word(today.midnight);
    let mut names: Vec<&String> = folded.iter().collect();
    names.sort();
    for name in names {
        stamp.bytes(name.as_bytes());
    }
    stamp.word(items.len() as u64);
    for item in items {
        stamp.word(item.id.0);
        stamp.bytes(item.path.as_os_str().as_encoded_bytes());
        stamp.bytes(item.label.as_bytes());
        stamp.optional(item.size);
        stamp.optional(item.modified);
        stamp.optional(item.created);
    }
    stamp.0
}

impl ViewCache {
    /// The display order of a grouped space, once `App::sync_grouping` has made
    /// one.
    pub(in crate::app) fn grouping(&self, id: SpaceId) -> Option<Arc<Grouping>> {
        self.groupings
            .get(&id)
            .map(|cached| Arc::clone(&cached.grouping))
    }

    /// Folds group `name` of space `id` if it is open, or opens it.
    pub(in crate::app) fn toggle_group(&mut self, id: SpaceId, name: &str) {
        let folded = self.folded_groups.entry(id).or_default();
        if !folded.remove(name) {
            folded.insert(name.to_owned());
        }
    }

    /// The stored scroll offset, unclamped; 0 when never scrolled.
    pub(in crate::app) fn scroll_of(&self, id: SpaceId) -> i32 {
        self.scroll.get(&id).copied().unwrap_or(0)
    }

    pub(in crate::app) fn set_scroll(&mut self, id: SpaceId, px: i32) {
        self.scroll.insert(id, px);
    }

    /// The stored sideways offset, unclamped; 0 when never scrolled.
    pub(in crate::app) fn scroll_x_of(&self, id: SpaceId) -> i32 {
        self.scroll_x.get(&id).copied().unwrap_or(0)
    }

    pub(in crate::app) fn set_scroll_x(&mut self, id: SpaceId, px: i32) {
        self.scroll_x.insert(id, px);
    }

    /// Records `id`'s icon factor and returns the one from its last re-flow.
    pub(super) fn record_icon_scale(&mut self, id: SpaceId, factor: f32) -> Option<f32> {
        self.icon_scales.insert(id, factor)
    }

    /// Drops everything kept for a deleted space.
    pub(in crate::app) fn forget(&mut self, id: SpaceId) {
        self.groupings.remove(&id);
        self.folded_groups.remove(&id);
        self.scroll.remove(&id);
        self.scroll_x.remove(&id);
        self.icon_scales.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use deskspace_core::{ItemFacts, ItemId, SpaceView};
    use deskspace_platform::local_today;

    use super::*;

    /// Run by hand in a release build: `cargo test --release -p deskspace-app
    /// time_grouping -- --ignored --nocapture`. Every repaint of a grouped space
    /// pays this.
    #[test]
    #[ignore = "a timing, not a check"]
    fn time_grouping_stamp_of_fifty_thousand_items() {
        let items: Vec<Item> = (0..50_000)
            .map(|index| {
                Item::new(
                    ItemId(index),
                    format!("C:/Users/someone/Downloads/file{index}.txt"),
                    ItemFacts {
                        size: Some(index),
                        modified: Some(index),
                        ..ItemFacts::default()
                    },
                )
            })
            .collect();
        let group = GroupBy {
            key: deskspace_core::SortKey::Name,
            descending: false,
        };
        let today = local_today();
        let started = std::time::Instant::now();
        let stamp = grouping_stamp(&items, group, today, &HashSet::new());
        eprintln!("stamp: {:?} ({stamp:x})", started.elapsed());
    }

    #[test]
    fn the_stamp_notices_everything_a_grouping_reads() {
        let item = |label: &str, size| {
            Item::new(
                ItemId(1),
                format!("C:/a/{label}.txt"),
                ItemFacts {
                    size,
                    ..ItemFacts::default()
                },
            )
        };
        let group = GroupBy {
            key: deskspace_core::SortKey::Name,
            descending: false,
        };
        let today = LocalDay {
            year: 2026,
            month: 10,
            day: 31,
            weekday: 6,
            midnight: 1_000,
        };
        let none = HashSet::new();
        let base = grouping_stamp(&[item("apple", None)], group, today, &none);
        assert_eq!(
            base,
            grouping_stamp(&[item("apple", None)], group, today, &none)
        );
        let folded: HashSet<String> = ["A - H".to_owned()].into();
        for changed in [
            grouping_stamp(&[item("zpple", None)], group, today, &none),
            grouping_stamp(&[item("apple", Some(0))], group, today, &none),
            grouping_stamp(&[], group, today, &none),
            grouping_stamp(
                &[item("apple", None)],
                GroupBy {
                    descending: true,
                    ..group
                },
                today,
                &none,
            ),
            grouping_stamp(
                &[item("apple", None)],
                group,
                LocalDay {
                    midnight: 2_000,
                    ..today
                },
                &none,
            ),
            grouping_stamp(&[item("apple", None)], group, today, &folded),
        ] {
            assert_ne!(base, changed);
        }
    }

    #[test]
    fn view_cache_forgets_a_deleted_space() {
        let mut cache = ViewCache::default();
        let id = SpaceId(1);
        assert_eq!(cache.scroll_of(id), 0);
        assert_eq!(cache.scroll_x_of(id), 0);
        cache.set_scroll(id, 40);
        cache.set_scroll_x(id, 25);
        assert_eq!(cache.record_icon_scale(id, 1.0), None);
        assert_eq!(cache.record_icon_scale(id, 1.5), Some(1.0));
        assert_eq!(cache.scroll_of(id), 40);
        assert_eq!(cache.scroll_x_of(id), 25);

        cache.forget(id);
        assert_eq!(cache.scroll_of(id), 0);
        assert_eq!(cache.scroll_x_of(id), 0);
        assert_eq!(cache.record_icon_scale(id, 1.5), None, "no stale factor");
    }

    #[test]
    fn a_sideways_offset_clamps_and_grows_back_to_what_was_stored() {
        use deskspace_core::{Column, ColumnSpec};

        let view = SpaceView {
            columns: ColumnSpec::sanitize(vec![
                ColumnSpec {
                    id: Column::Name,
                    width: None,
                },
                ColumnSpec {
                    id: Column::Modified,
                    width: Some(400),
                },
            ]),
            ..SpaceView::default()
        }
        .with_name_held(Some(300));
        let mut cache = ViewCache::default();
        let id = SpaceId(1);
        cache.set_scroll_x(id, 300);

        // The columns span 700. In 500 the most they scroll is 200, so the
        // stored 300 is clamped there.
        let narrow = crate::scene::details_columns(&view, 500, 0, cache.scroll_x_of(id), 24, 1.0);
        assert_eq!(narrow.last().unwrap().rect.right(), 500);
        assert_eq!(cache.scroll_x_of(id), 300, "the stored value keeps");

        // Back in 200 the limit is 500, so the stored 300 shows again, and
        // the offset never grows past what was stored.
        let wide = crate::scene::details_columns(&view, 200, 0, cache.scroll_x_of(id), 24, 1.0);
        assert_eq!(wide.last().unwrap().rect.right(), 400);
    }
}
