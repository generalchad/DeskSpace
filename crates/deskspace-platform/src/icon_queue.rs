//! Which icons the load worker should fetch next, and who is waiting for them.
//! Pure: the worker and the Win32 calls are in `win32/icons.rs`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hash;

struct Waiting<U> {
    stamp: u64,
    users: BTreeSet<U>,
}

struct InFlight<U> {
    users: BTreeSet<U>,
    stale: bool,
}

/// An icon the worker finished: who to tell, and whether its file changed
/// while it was loading, so its pixels may be out of date.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Finished<U> {
    pub(crate) users: BTreeSet<U>,
    pub(crate) stale: bool,
}

/// The requests waiting for the worker, one per key, each naming the users
/// that still want it. A user states everything it wants at once, so a key no
/// user wants any more (it scrolled out of view) goes before the worker
/// reaches it.
pub(crate) struct IconQueue<K, U> {
    clock: u64,
    waiting: HashMap<K, Waiting<U>>,
    by_age: BTreeMap<u64, K>,
    in_flight: HashMap<K, InFlight<U>>,
}

impl<K: Clone + Eq + Hash, U: Clone + Ord> IconQueue<K, U> {
    pub(crate) fn new() -> Self {
        Self {
            clock: 0,
            waiting: HashMap::new(),
            by_age: BTreeMap::new(),
            in_flight: HashMap::new(),
        }
    }

    /// Replaces what `user` wants with `keys`, given in the order they should
    /// load. Each is now the newest request, so what is on screen goes ahead of
    /// what was asked for earlier, and a key already loading is not asked for
    /// twice.
    pub(crate) fn set_wanted(&mut self, user: &U, keys: &[K]) {
        let by_age = &mut self.by_age;
        self.waiting.retain(|_, waiting| {
            waiting.users.remove(user);
            let wanted = !waiting.users.is_empty();
            if !wanted {
                by_age.remove(&waiting.stamp);
            }
            wanted
        });
        for key in keys.iter().rev() {
            if let Some(in_flight) = self.in_flight.get_mut(key) {
                in_flight.users.insert(user.clone());
                continue;
            }
            self.clock += 1;
            let waiting = self.waiting.entry(key.clone()).or_insert_with(|| Waiting {
                stamp: 0,
                users: BTreeSet::new(),
            });
            self.by_age.remove(&waiting.stamp);
            waiting.stamp = self.clock;
            waiting.users.insert(user.clone());
            self.by_age.insert(self.clock, key.clone());
        }
    }

    /// Takes the newest request for the worker.
    pub(crate) fn pop(&mut self) -> Option<K> {
        let (_, key) = self.by_age.pop_last()?;
        let waiting = self.waiting.remove(&key)?;
        self.in_flight.insert(
            key.clone(),
            InFlight {
                users: waiting.users,
                stale: false,
            },
        );
        Some(key)
    }

    /// Whether a request is waiting for the worker.
    pub(crate) fn is_empty(&self) -> bool {
        self.by_age.is_empty()
    }

    /// Notes that the file behind every loading key `matches` accepts changed.
    pub(crate) fn mark_stale(&mut self, matches: impl Fn(&K) -> bool) {
        for (_, in_flight) in self.in_flight.iter_mut().filter(|(key, _)| matches(key)) {
            in_flight.stale = true;
        }
    }

    /// Closes a key the worker finished.
    pub(crate) fn finish(&mut self, key: &K) -> Option<Finished<U>> {
        let in_flight = self.in_flight.remove(key)?;
        Some(Finished {
            users: in_flight.users,
            stale: in_flight.stale,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(queue: &mut IconQueue<&'static str, u8>) -> Vec<&'static str> {
        std::iter::from_fn(|| queue.pop()).collect()
    }

    #[test]
    fn a_key_two_users_want_loads_once() {
        let mut queue = IconQueue::new();
        queue.set_wanted(&1, &["a"]);
        queue.set_wanted(&2, &["a"]);
        assert_eq!(queue.pop(), Some("a"));
        assert_eq!(queue.pop(), None);
        let finished = queue.finish(&"a").expect("in flight");
        assert_eq!(finished.users, BTreeSet::from([1, 2]));
    }

    #[test]
    fn one_scene_loads_in_the_order_it_asked() {
        let mut queue = IconQueue::new();
        queue.set_wanted(&1, &["top", "middle", "bottom"]);
        assert_eq!(drain(&mut queue), ["top", "middle", "bottom"]);
    }

    #[test]
    fn the_newest_request_goes_first() {
        let mut queue = IconQueue::new();
        queue.set_wanted(&1, &["a", "b"]);
        queue.set_wanted(&2, &["c", "a"]);
        assert_eq!(drain(&mut queue), ["c", "a", "b"]);
    }

    #[test]
    fn a_key_scrolled_out_of_view_is_dropped() {
        let mut queue = IconQueue::new();
        queue.set_wanted(&1, &["gone", "kept"]);
        queue.set_wanted(&1, &["kept", "new"]);
        assert_eq!(drain(&mut queue), ["kept", "new"]);
    }

    #[test]
    fn a_key_stays_while_another_user_still_wants_it() {
        let mut queue = IconQueue::new();
        queue.set_wanted(&1, &["shared"]);
        queue.set_wanted(&2, &["shared"]);
        queue.set_wanted(&1, &[]);
        assert_eq!(queue.pop(), Some("shared"));
        let finished = queue.finish(&"shared").expect("in flight");
        assert_eq!(finished.users, BTreeSet::from([2]));
    }

    #[test]
    fn a_key_no_user_wants_empties_the_queue() {
        let mut queue = IconQueue::new();
        queue.set_wanted(&1, &["a"]);
        assert!(!queue.is_empty());
        queue.set_wanted(&1, &[]);
        assert!(queue.is_empty());
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn a_key_already_loading_is_not_asked_for_twice() {
        let mut queue = IconQueue::new();
        queue.set_wanted(&1, &["a"]);
        assert_eq!(queue.pop(), Some("a"));
        queue.set_wanted(&2, &["a"]);
        assert_eq!(queue.pop(), None);
        let finished = queue.finish(&"a").expect("in flight");
        assert_eq!(finished.users, BTreeSet::from([1, 2]));
    }

    #[test]
    fn a_file_that_changes_while_loading_finishes_stale() {
        let mut queue = IconQueue::new();
        queue.set_wanted(&1, &["changed", "same"]);
        assert_eq!(queue.pop(), Some("changed"));
        assert_eq!(queue.pop(), Some("same"));
        queue.mark_stale(|key| *key == "changed");
        assert!(queue.finish(&"changed").expect("in flight").stale);
        assert!(!queue.finish(&"same").expect("in flight").stale);
    }

    #[test]
    fn a_key_that_is_not_loading_finishes_nothing() {
        let mut queue: IconQueue<&str, u8> = IconQueue::new();
        assert_eq!(queue.finish(&"a"), None);
    }
}
