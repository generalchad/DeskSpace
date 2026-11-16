//! The item files the load worker reads again when no icon is waiting: one
//! batch per space, handed out a file at a time, so an icon request waits for
//! one read at most. Pure: the worker and the read are in `win32/`.

use std::collections::VecDeque;
use std::path::PathBuf;

use deskspace_core::{ItemFacts, ItemId, SpaceId};

/// What reading an item's file found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileState {
    /// The file is there, with these facts.
    Present(ItemFacts),
    /// The file is gone: its drive or share answers, and it is not there.
    Gone,
    /// Nothing can be said: the drive is unplugged or not ready, the share
    /// does not answer, or the read was refused. The file may well be there.
    Unreachable,
}

/// What reading one space's items again found. An item whose file could not
/// be reached is in neither list, so it keeps its place and the facts it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactBatch {
    pub space: SpaceId,
    /// The facts of each file that was read, as `Command::RefreshItemFacts`
    /// takes them.
    pub facts: Vec<(ItemId, ItemFacts)>,
    /// The items whose files are gone for certain.
    pub gone: Vec<ItemId>,
}

struct Job {
    waiting: VecDeque<(ItemId, PathBuf)>,
    found: FactBatch,
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
    /// Queues `space`'s `items`, each with the path to read.
    pub(crate) fn push(&mut self, space: SpaceId, items: Vec<(ItemId, PathBuf)>) {
        if items.is_empty() {
            return;
        }
        self.jobs.push_back(Job {
            waiting: items.into(),
            found: FactBatch {
                space,
                facts: Vec::new(),
                gone: Vec::new(),
            },
        });
    }

    /// The next file to read, from the oldest batch. `record` takes its
    /// answer before this is called again.
    pub(crate) fn next(&mut self) -> Option<PathBuf> {
        let (item, path) = self.jobs.front_mut()?.waiting.pop_front()?;
        self.reading = Some(item);
        Some(path)
    }

    /// Notes what reading the file `next` named found. Returns whether that
    /// finished a batch with something to hand over.
    pub(crate) fn record(&mut self, state: FileState) -> bool {
        let (Some(item), Some(job)) = (self.reading.take(), self.jobs.front_mut()) else {
            return false;
        };
        match state {
            FileState::Present(facts) => job.found.facts.push((item, facts)),
            FileState::Gone => job.found.gone.push(item),
            FileState::Unreachable => {}
        }
        if !job.waiting.is_empty() {
            return false;
        }
        let Some(job) = self.jobs.pop_front() else {
            return false;
        };
        if job.found.facts.is_empty() && job.found.gone.is_empty() {
            return false;
        }
        self.done.push(job.found);
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

    fn present(size: u64) -> FileState {
        FileState::Present(sized(size))
    }

    fn batch(space: SpaceId, facts: &[(u64, u64)], gone: &[u64]) -> FactBatch {
        FactBatch {
            space,
            facts: facts
                .iter()
                .map(|(id, size)| (ItemId(*id), sized(*size)))
                .collect(),
            gone: gone.iter().map(|id| ItemId(*id)).collect(),
        }
    }

    #[test]
    fn a_batch_is_handed_over_once_its_last_file_is_read() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "a"), (2, "b")]));
        assert_eq!(jobs.next(), Some(PathBuf::from("a")));
        assert!(!jobs.record(present(10)), "one file is still to read");
        assert!(jobs.take_done().is_empty());
        assert_eq!(jobs.next(), Some(PathBuf::from("b")));
        assert!(jobs.record(present(20)));
        assert_eq!(jobs.take_done(), [batch(WORK, &[(1, 10), (2, 20)], &[])]);
        assert_eq!(jobs.next(), None);
        assert!(jobs.take_done().is_empty(), "each batch is taken once");
    }

    #[test]
    fn batches_are_read_in_the_order_they_were_asked_for() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "a")]));
        jobs.push(PLAY, items(&[(2, "b")]));
        assert_eq!(jobs.next(), Some(PathBuf::from("a")));
        assert!(jobs.record(present(1)));
        assert_eq!(jobs.next(), Some(PathBuf::from("b")));
        assert!(jobs.record(present(2)));
        let spaces: Vec<SpaceId> = jobs.take_done().iter().map(|batch| batch.space).collect();
        assert_eq!(spaces, [WORK, PLAY]);
    }

    #[test]
    fn a_file_that_is_gone_is_named_and_one_out_of_reach_is_left_out() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "deleted"), (2, "unplugged"), (3, "c")]));
        jobs.next();
        assert!(!jobs.record(FileState::Gone));
        jobs.next();
        assert!(!jobs.record(FileState::Unreachable));
        jobs.next();
        assert!(jobs.record(present(30)));
        assert_eq!(jobs.take_done(), [batch(WORK, &[(3, 30)], &[1])]);
    }

    #[test]
    fn a_batch_of_gone_files_alone_is_handed_over() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "deleted")]));
        jobs.next();
        assert!(jobs.record(FileState::Gone));
        assert_eq!(jobs.take_done(), [batch(WORK, &[], &[1])]);
    }

    #[test]
    fn a_batch_with_nothing_reached_is_not_handed_over() {
        let mut jobs = FactJobs::default();
        jobs.push(WORK, items(&[(1, "unplugged")]));
        jobs.push(PLAY, items(&[(2, "b")]));
        jobs.next();
        assert!(!jobs.record(FileState::Unreachable));
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
        assert!(!jobs.record(present(1)));
        jobs.push(WORK, items(&[(1, "a")]));
        assert!(!jobs.record(present(1)), "no file was handed out yet");
        assert_eq!(jobs.next(), Some(PathBuf::from("a")));
    }
}
