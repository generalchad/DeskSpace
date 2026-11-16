//! The item facts the load worker reads again when no icon is waiting: one
//! batch per space, handed out a file at a time, so an icon request waits for
//! one read at most. Pure: the worker and the read are in `win32/icons/`.

use std::collections::VecDeque;
use std::path::PathBuf;

use deskspace_core::{ItemFacts, ItemId, SpaceId};

/// The facts read for one space's items, as `Command::RefreshItemFacts`
/// takes them.
pub type FactBatch = (SpaceId, Vec<(ItemId, ItemFacts)>);

struct Job {
    space: SpaceId,
    waiting: VecDeque<(ItemId, PathBuf)>,
    read: Vec<(ItemId, ItemFacts)>,
}

/// The batches waiting to be read, oldest first, and the ones finished.
/// Unlike an icon request, a batch is never dropped: nothing asks for it
/// again.
#[derive(Default)]
pub(crate) struct FactJobs {
    jobs: VecDeque<Job>,
    /// The item of the oldest batch whose file is being read now.
    reading: Option<ItemId>,
    done: Vec<FactBatch>,
}

impl FactJobs {
    /// Queues the facts of `space`'s `items`, each with the path to read.
    pub(crate) fn push(&mut self, space: SpaceId, items: Vec<(ItemId, PathBuf)>) {
        if items.is_empty() {
            return;
        }
        self.jobs.push_back(Job {
            space,
            waiting: items.into(),
            read: Vec::new(),
        });
    }

    /// The next file to read, from the oldest batch. `record` takes its
    /// answer before this is called again.
    pub(crate) fn next(&mut self) -> Option<PathBuf> {
        let (item, path) = self.jobs.front_mut()?.waiting.pop_front()?;
        self.reading = Some(item);
        Some(path)
    }

    /// Notes what the file `next` named holds, leaving out one that could not
    /// be read (`None`) so a missing file keeps the facts it has. Returns
    /// whether that finished a batch with something to hand over.
    pub(crate) fn record(&mut self, facts: Option<ItemFacts>) -> bool {
        let (Some(item), Some(job)) = (self.reading.take(), self.jobs.front_mut()) else {
            return false;
        };
        if let Some(facts) = facts {
            job.read.push((item, facts));
        }
        if !job.waiting.is_empty() {
            return false;
        }
        let Some(job) = self.jobs.pop_front() else {
            return false;
        };
        if job.read.is_empty() {
            return false;
        }
        self.done.push((job.space, job.read));
        true
    }

    /// Takes the finished batches, in the order they were asked for.
    pub(crate) fn take_done(&mut self) -> Vec<FactBatch> {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: SpaceId = SpaceId(1);
    const PLAY: SpaceId = SpaceId(2);

    fn items(names: &[(u64, &str)]) -> Vec<(ItemId, PathBuf)> {
        names
            .iter()
            .map(|(id, name)| (ItemId(*id), PathBuf::from(name)))
            .collect()
    }

    fn sized(size: u64) -> ItemFacts {
        ItemFacts {
            size: Some(size),
            ..ItemFacts::default()
        }
    }

    #[test]
    fn a_batch_is_handed_over_once_its_last_file_is_read() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "a"), (2, "b")]));
        assert_eq!(jobs.next(), Some(PathBuf::from("a")));
        assert!(!jobs.record(Some(sized(10))), "one file is still to read");
        assert!(jobs.take_done().is_empty());
        assert_eq!(jobs.next(), Some(PathBuf::from("b")));
        assert!(jobs.record(Some(sized(20))));
        assert_eq!(
            jobs.take_done(),
            [(WORK, vec![(ItemId(1), sized(10)), (ItemId(2), sized(20))])]
        );
        assert_eq!(jobs.next(), None);
        assert!(jobs.take_done().is_empty(), "each batch is taken once");
    }

    #[test]
    fn batches_are_read_in_the_order_they_were_asked_for() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "a")]));
        jobs.push(PLAY, items(&[(2, "b")]));
        assert_eq!(jobs.next(), Some(PathBuf::from("a")));
        assert!(jobs.record(Some(sized(1))));
        assert_eq!(jobs.next(), Some(PathBuf::from("b")));
        assert!(jobs.record(Some(sized(2))));
        let spaces: Vec<SpaceId> = jobs.take_done().into_iter().map(|batch| batch.0).collect();
        assert_eq!(spaces, [WORK, PLAY]);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_left_out() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "gone"), (2, "b")]));
        jobs.next();
        assert!(!jobs.record(None));
        jobs.next();
        assert!(jobs.record(Some(sized(20))));
        assert_eq!(jobs.take_done(), [(WORK, vec![(ItemId(2), sized(20))])]);
    }

    #[test]
    fn a_batch_with_nothing_read_is_not_handed_over() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "gone")]));
        jobs.push(PLAY, items(&[(2, "b")]));
        jobs.next();
        assert!(!jobs.record(None));
        assert!(jobs.take_done().is_empty());
        assert_eq!(jobs.next(), Some(PathBuf::from("b")), "the next batch");
    }

    #[test]
    fn a_space_with_no_items_queues_nothing() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, Vec::new());
        assert_eq!(jobs.next(), None);
    }

    #[test]
    fn an_answer_nothing_asked_for_is_ignored() {
        let mut jobs = FactJobs::default();
        assert!(!jobs.record(Some(sized(1))));
        jobs.push(WORK, items(&[(1, "a")]));
        assert!(!jobs.record(Some(sized(1))), "no file was handed out yet");
        assert_eq!(jobs.next(), Some(PathBuf::from("a")));
    }
}
