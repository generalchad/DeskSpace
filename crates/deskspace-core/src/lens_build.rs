//! Turns a folder's listing into a lens's finished item list, off the UI thread.
//!
//! Filtering, sorting, and matching ids on a big folder takes far longer than
//! a frame, so the watch worker does it with a `LensBuilder`, and the UI thread
//! only swaps the result in (`Command::SyncLens`). Pure, so all of it is tested
//! without a worker.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::id::ItemId;
use crate::item::{Item, ItemKind};
use crate::sort::SortRule;
use crate::source::FolderEntry;

/// How many item ids one watch reserves. Far more new paths than one watch can
/// meet, so a worker never has to ask the UI thread for more; ids are `u64`, so
/// a session cannot run out of blocks either.
pub const ITEM_ID_BLOCK: u64 = 1 << 32;

/// Item ids reserved for one watch's worker (`Desktop::reserve_item_ids`), so
/// it can name new items without the model, and no id is ever reused (D11).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemIdBlock {
    pub(crate) next: u64,
    pub(crate) end: u64,
}

impl ItemIdBlock {
    /// The next id, or `None` once the block is used up.
    fn take(&mut self) -> Option<ItemId> {
        (self.next < self.end).then(|| {
            let id = ItemId(self.next);
            self.next += 1;
            id
        })
    }
}

/// Everything about a lens that decides which entries it lists and in what
/// order. A list carries the rules it was built under, so one built under
/// older rules is never swapped in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LensRules {
    pub show_hidden: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub sort: SortRule,
}

impl LensRules {
    /// Whether an entry is listed under these rules.
    fn shows(&self, entry: &FolderEntry) -> bool {
        if entry.hidden && !self.show_hidden {
            return false;
        }
        let name = entry.path.file_name().unwrap_or_default().to_string_lossy();
        let any = |patterns: &[String]| {
            patterns
                .iter()
                .any(|pattern| crate::wildcard::matches(pattern, &name))
        };
        // "Hide" drops any match. "Show only" keeps only the files that match
        // one, so folders stay browsable (D18). Names only, never full paths.
        if any(&self.exclude) {
            return false;
        }
        entry.kind == ItemKind::Folder || self.include.is_empty() || any(&self.include)
    }
}

/// A lens's finished list, ready to swap in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LensList {
    /// The items to show, filtered and in order. Their positions are left for
    /// the swap, which knows the space's grid.
    pub items: Vec<Item>,
    /// The rules the list was built under.
    pub rules: LensRules,
    /// Listed paths whose modified time changed, so their icons went stale.
    pub rewritten: Vec<PathBuf>,
    /// Whether the folder held anything before the rules were applied, so a
    /// lens that shows nothing can say its filters are why.
    pub listed_any: bool,
}

/// What the builder knows about a path that keeps its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Known {
    id: ItemId,
    modified: Option<u64>,
}

/// One watch's memory of its folder: the last listing, and the id of each path
/// in it, so a path that survives a refresh keeps its id and a rule change
/// needs no new read of the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LensBuilder {
    rules: LensRules,
    ids: ItemIdBlock,
    known: HashMap<PathBuf, Known>,
    /// The last listing, until the folder goes away.
    entries: Option<Vec<FolderEntry>>,
}

impl LensBuilder {
    /// A builder for a lens that holds `items` now, so they keep their ids.
    pub fn new(rules: LensRules, ids: ItemIdBlock, items: &[Item]) -> Self {
        let mut builder = Self {
            rules,
            ids,
            known: HashMap::with_capacity(items.len()),
            entries: None,
        };
        for item in items {
            builder.note_item(item.path.clone(), item.id, item.modified);
        }
        builder
    }

    /// The rules the next list is built under.
    pub fn rules(&self) -> &LensRules {
        &self.rules
    }

    /// Remembers that `path` has `id`, for an item the model re-pointed, such
    /// as a rename, so the next listing finds it under its new path.
    pub fn note_item(&mut self, path: PathBuf, id: ItemId, modified: Option<u64>) {
        self.known.insert(path, Known { id, modified });
    }

    /// Takes a fresh listing of the folder and builds the list from it.
    pub fn listed(&mut self, entries: Vec<FolderEntry>) -> LensList {
        let rewritten = entries
            .iter()
            .filter(|entry| {
                self.known
                    .get(&entry.path)
                    .is_some_and(|known| known.modified != entry.modified)
            })
            .map(|entry| entry.path.clone())
            .collect();
        let mut seen = HashSet::with_capacity(entries.len());
        let mut known = HashMap::with_capacity(entries.len());
        for entry in &entries {
            if !seen.insert(entry.path.as_path()) {
                continue;
            }
            let id = self.known.get(&entry.path).map(|known| known.id);
            // A used-up block leaves the path out rather than repeat an id.
            let Some(id) = id.or_else(|| self.ids.take()) else {
                continue;
            };
            known.insert(
                entry.path.clone(),
                Known {
                    id,
                    modified: entry.modified,
                },
            );
        }
        self.known = known;
        self.entries = Some(entries);
        LensList {
            rewritten,
            ..self.build()
        }
    }

    /// Takes new rules, and builds the list again from the last listing if the
    /// folder has one; without one, the next listing uses them.
    pub fn set_rules(&mut self, rules: LensRules) -> Option<LensList> {
        self.rules = rules;
        self.entries.is_some().then(|| self.build())
    }

    /// Forgets the listing and its ids, because the folder went away. A path
    /// that comes back is a new item.
    pub fn folder_gone(&mut self) {
        self.known.clear();
        self.entries = None;
    }

    fn build(&self) -> LensList {
        let entries = self.entries.as_deref().unwrap_or_default();
        let mut seen = HashSet::with_capacity(entries.len());
        let mut items: Vec<Item> = entries
            .iter()
            .filter(|entry| self.rules.shows(entry) && seen.insert(entry.path.as_path()))
            .filter_map(|entry| {
                let known = self.known.get(&entry.path)?;
                Some(entry.clone().into_item(known.id))
            })
            .collect();
        self.rules.sort.sort_items(&mut items);
        LensList {
            items,
            rules: self.rules.clone(),
            rewritten: Vec::new(),
            listed_any: !entries.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::SortKey;

    fn ids() -> ItemIdBlock {
        ItemIdBlock {
            next: 100,
            end: 100 + ITEM_ID_BLOCK,
        }
    }

    fn entry(name: &str) -> FolderEntry {
        FolderEntry {
            path: PathBuf::from(format!("C:/folder/{name}")),
            label: name.trim_end_matches(".txt").into(),
            kind: ItemKind::File,
            size: Some(1),
            modified: Some(2),
            created: Some(1),
            hidden: false,
        }
    }

    fn builder() -> LensBuilder {
        LensBuilder::new(LensRules::default(), ids(), &[])
    }

    fn labels(list: &LensList) -> Vec<&str> {
        list.items.iter().map(|item| item.label.as_str()).collect()
    }

    #[test]
    fn a_path_that_survives_keeps_its_id_and_a_new_one_gets_a_fresh_id() {
        let mut builder = builder();
        let first = builder.listed(vec![entry("b.txt"), entry("a.txt")]);
        assert_eq!(labels(&first), ["a", "b"], "in order");
        let (a, b) = (first.items[0].id, first.items[1].id);

        let second = builder.listed(vec![entry("a.txt"), entry("c.txt")]);
        assert_eq!(labels(&second), ["a", "c"]);
        assert_eq!(second.items[0].id, a);
        assert!(second.items.iter().all(|item| item.id != b), "b is gone");
        assert!(second.items[1].id > b, "ids are never reused");
    }

    #[test]
    fn a_path_that_leaves_and_comes_back_is_a_new_item() {
        let mut builder = builder();
        let first = builder.listed(vec![entry("a.txt")]).items[0].id;
        builder.listed(Vec::new());
        assert_ne!(builder.listed(vec![entry("a.txt")]).items[0].id, first);
    }

    #[test]
    fn a_seeded_builder_keeps_the_ids_of_the_items_it_was_given() {
        let mut builder = builder();
        let list = builder.listed(vec![entry("a.txt")]);
        let mut seeded = LensBuilder::new(LensRules::default(), ids(), &list.items);
        assert_eq!(
            seeded.listed(vec![entry("a.txt")]).items[0].id,
            list.items[0].id
        );
    }

    #[test]
    fn a_noted_rename_keeps_the_items_id_under_its_new_path() {
        let mut builder = builder();
        let id = builder.listed(vec![entry("a.txt")]).items[0].id;
        builder.note_item(PathBuf::from("C:/folder/z.txt"), id, Some(2));
        let list = builder.listed(vec![entry("z.txt")]);
        assert_eq!(list.items[0].id, id);
        assert!(list.rewritten.is_empty(), "a rename rewrites nothing");
    }

    #[test]
    fn only_a_changed_modified_time_counts_as_rewritten() {
        let mut builder = builder();
        assert!(
            builder.listed(vec![entry("a.txt")]).rewritten.is_empty(),
            "a new path has no stale icon"
        );
        assert!(builder.listed(vec![entry("a.txt")]).rewritten.is_empty());
        let touched = FolderEntry {
            modified: Some(9),
            ..entry("a.txt")
        };
        assert_eq!(
            builder.listed(vec![touched]).rewritten,
            [PathBuf::from("C:/folder/a.txt")]
        );
        let lost_its_time = FolderEntry {
            modified: None,
            ..entry("a.txt")
        };
        assert_eq!(
            builder.listed(vec![lost_its_time]).rewritten,
            [PathBuf::from("C:/folder/a.txt")]
        );
    }

    #[test]
    fn a_rule_change_rebuilds_from_the_last_listing_and_keeps_ids() {
        let mut builder = builder();
        let first = builder.listed(vec![entry("a.txt"), entry("b.pdf")]);
        let id = first.items[1].id;
        assert_eq!(labels(&first), ["a", "b.pdf"]);

        let rules = LensRules {
            include: vec!["*.pdf".into()],
            ..LensRules::default()
        };
        let list = builder.set_rules(rules.clone()).unwrap();
        assert_eq!(labels(&list), ["b.pdf"]);
        assert_eq!(list.items[0].id, id);
        assert_eq!(list.rules, rules, "the list names what it was built from");
        assert!(list.rewritten.is_empty());

        let all = builder.set_rules(LensRules::default()).unwrap();
        assert_eq!(all.items[0].id, first.items[0].id, "a filtered id survives");
    }

    #[test]
    fn rules_with_no_listing_yet_wait_for_one() {
        let mut builder = builder();
        let rules = LensRules {
            exclude: vec!["*.txt".into()],
            ..LensRules::default()
        };
        assert_eq!(builder.set_rules(rules), None);
        assert!(builder.listed(vec![entry("a.txt")]).items.is_empty());
    }

    #[test]
    fn a_folder_that_went_away_forgets_its_listing_and_ids() {
        let mut builder = builder();
        let id = builder.listed(vec![entry("a.txt")]).items[0].id;
        builder.folder_gone();
        assert_eq!(builder.set_rules(LensRules::default()), None);
        assert_ne!(builder.listed(vec![entry("a.txt")]).items[0].id, id);
    }

    #[test]
    fn listed_any_says_whether_the_folder_held_anything() {
        let mut builder = builder();
        assert!(!builder.listed(Vec::new()).listed_any);
        let rules = LensRules {
            exclude: vec!["*".into()],
            ..LensRules::default()
        };
        builder.set_rules(rules);
        let list = builder.listed(vec![entry("a.txt")]);
        assert!(list.items.is_empty());
        assert!(list.listed_any, "the filters hid it");
    }

    #[test]
    fn hidden_entries_show_only_when_asked() {
        let hidden = FolderEntry {
            hidden: true,
            ..entry("h.txt")
        };
        let mut builder = builder();
        let list = builder.listed(vec![entry("a.txt"), hidden]);
        assert_eq!(labels(&list), ["a"]);
        let rules = LensRules {
            show_hidden: true,
            ..LensRules::default()
        };
        assert_eq!(labels(&builder.set_rules(rules).unwrap()), ["a", "h"]);
    }

    #[test]
    fn the_list_follows_the_rules_order() {
        let sized = |name: &str, size| FolderEntry {
            size: Some(size),
            ..entry(name)
        };
        let rules = LensRules {
            sort: SortRule {
                key: SortKey::Size,
                descending: true,
                folders_first: false,
            },
            ..LensRules::default()
        };
        let mut builder = LensBuilder::new(rules, ids(), &[]);
        let list = builder.listed(vec![sized("small.txt", 10), sized("big.txt", 300)]);
        assert_eq!(labels(&list), ["big", "small"]);
    }

    #[test]
    fn a_duplicate_path_lists_once() {
        let mut builder = builder();
        let list = builder.listed(vec![entry("a.txt"), entry("a.txt")]);
        assert_eq!(list.items.len(), 1);
    }

    #[test]
    fn a_used_up_block_leaves_new_paths_out_and_never_repeats_an_id() {
        let block = ItemIdBlock { next: 1, end: 2 };
        let mut builder = LensBuilder::new(LensRules::default(), block, &[]);
        let list = builder.listed(vec![entry("a.txt"), entry("b.txt")]);
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].id, ItemId(1));
    }
}
