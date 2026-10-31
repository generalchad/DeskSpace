//! A grouped space's display order (D18).
//!
//! A space's stored item order stays as it is, so ungrouping restores a hand
//! arrangement exactly. The app works out which group each item is in, because
//! type names and today's date are not core's to know, and this holds the
//! result as plain numbers for `ViewLayout`.

/// A day as the user's clock reads it, which the grouping by date is relative
/// to. The caller reads the clock, so what it decides can be tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalDay {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31.
    pub day: u32,
    /// Days since the week began, 0 on Sunday.
    pub weekday: u32,
    /// The Unix time of that day's local midnight.
    pub midnight: u64,
}

/// One group in a space's display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    /// How many items it holds, folded or not.
    pub len: usize,
    pub folded: bool,
    /// The display position of its first item; for a folded group, of the next
    /// item shown.
    first: usize,
}

impl Group {
    /// How many of its items are shown.
    pub fn shown(&self) -> usize {
        if self.folded { 0 } else { self.len }
    }

    pub fn first(&self) -> usize {
        self.first
    }
}

/// Which items a grouped space shows, in what order, under which groups. A
/// position counts only shown items, so a folded group's items have none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grouping {
    /// The stored index of the item at each position.
    order: Vec<u32>,
    /// The position of each stored index, or `NOT_SHOWN`.
    positions: Vec<u32>,
    groups: Vec<Group>,
}

const NOT_SHOWN: u32 = u32::MAX;

impl Grouping {
    /// `groups` lists each group's name and stored item indices, in display
    /// order. `folded` says which names are folded.
    pub fn new(
        groups: Vec<(String, Vec<u32>)>,
        item_count: usize,
        folded: impl Fn(&str) -> bool,
    ) -> Self {
        let mut order = Vec::with_capacity(item_count);
        let mut positions = vec![NOT_SHOWN; item_count];
        let groups = groups
            .into_iter()
            .map(|(name, items)| {
                let folded = folded(&name);
                let first = order.len();
                if !folded {
                    for item in &items {
                        if let Some(position) = positions.get_mut(*item as usize) {
                            *position = order.len() as u32;
                            order.push(*item);
                        }
                    }
                }
                Group {
                    name,
                    len: items.len(),
                    folded,
                    first,
                }
            })
            .collect();
        Self {
            order,
            positions,
            groups,
        }
    }

    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// How many items are shown.
    pub fn shown(&self) -> usize {
        self.order.len()
    }

    /// The stored index of the item at `position`.
    pub fn item_at(&self, position: usize) -> Option<usize> {
        self.order.get(position).map(|item| *item as usize)
    }

    /// The position of the item stored at `item`, or `None` while it is folded.
    pub fn position_of(&self, item: usize) -> Option<usize> {
        self.positions
            .get(item)
            .filter(|position| **position != NOT_SHOWN)
            .map(|position| *position as usize)
    }

    /// The group holding `position`.
    pub fn group_at(&self, position: usize) -> Option<usize> {
        self.groups
            .partition_point(|group| group.first <= position)
            .checked_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grouping(folded: &[&str]) -> Grouping {
        Grouping::new(
            vec![
                ("A".into(), vec![3, 1]),
                ("B".into(), vec![0]),
                ("C".into(), vec![4, 2]),
            ],
            5,
            |name| folded.contains(&name),
        )
    }

    #[test]
    fn positions_follow_the_groups_not_the_stored_order() {
        let grouping = grouping(&[]);
        let order: Vec<usize> = (0..5).filter_map(|at| grouping.item_at(at)).collect();
        assert_eq!(order, [3, 1, 0, 4, 2]);
        assert_eq!(grouping.position_of(3), Some(0));
        assert_eq!(grouping.position_of(2), Some(4));
        assert_eq!(grouping.group_at(1), Some(0));
        assert_eq!(grouping.group_at(2), Some(1));
        assert_eq!(grouping.group_at(4), Some(2));
    }

    #[test]
    fn a_folded_group_keeps_its_count_and_gives_up_its_positions() {
        let grouping = grouping(&["A"]);
        assert_eq!(grouping.shown(), 3);
        assert_eq!(grouping.groups()[0].len, 2, "the count stays");
        assert_eq!(grouping.groups()[0].shown(), 0);
        assert_eq!(grouping.position_of(3), None);
        assert_eq!(grouping.position_of(0), Some(0));
        assert_eq!(grouping.group_at(0), Some(1), "past the folded group");
    }

    #[test]
    fn every_group_folded_shows_nothing() {
        let grouping = grouping(&["A", "B", "C"]);
        assert_eq!(grouping.shown(), 0);
        assert_eq!(grouping.item_at(0), None);
        assert_eq!(grouping.groups().len(), 3, "the headers stay");
    }

    #[test]
    fn an_item_past_the_store_is_ignored() {
        let grouping = Grouping::new(vec![("A".into(), vec![0, 9])], 1, |_| false);
        assert_eq!(grouping.shown(), 1);
        assert_eq!(grouping.position_of(9), None);
    }
}
