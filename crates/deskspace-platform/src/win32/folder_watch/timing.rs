//! Pure debounce timers for folder-watch bursts.

use std::collections::HashMap;
use std::time::Instant;

use windows::Win32::System::Threading::INFINITE;

use super::DEBOUNCE_MS;

/// Milliseconds since the worker started.
pub(super) fn millis(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

/// The debounce clock of every watched folder, by slot. Pure, so the timing is
/// tested without real folders or waits.
#[derive(Default)]
pub(super) struct Bursts {
    by_slot: HashMap<usize, Coalescer>,
}

impl Bursts {
    pub(super) fn add(&mut self, slot: usize) {
        self.by_slot.insert(slot, Coalescer::default());
    }

    pub(super) fn remove(&mut self, slot: usize) {
        self.by_slot.remove(&slot);
    }

    pub(super) fn mark(&mut self, slot: usize, now_ms: u64) {
        if let Some(coalescer) = self.by_slot.get_mut(&slot) {
            coalescer.mark(now_ms);
        }
    }

    /// The shortest wait any folder needs, or `INFINITE` when none does.
    pub(super) fn wait_ms(&self, now_ms: u64) -> u32 {
        self.by_slot
            .values()
            .map(|coalescer| coalescer.wait_ms(now_ms))
            .min()
            .unwrap_or(INFINITE)
    }

    /// Takes the slots whose burst is due.
    pub(super) fn take_due(&mut self, now_ms: u64) -> Vec<usize> {
        self.by_slot
            .iter_mut()
            .filter_map(|(slot, coalescer)| coalescer.take_if_due(now_ms).then_some(*slot))
            .collect()
    }

    pub(super) fn clear(&mut self) {
        self.by_slot.clear();
    }
}

/// Debounces a burst of changes into one post: the first change opens a quiet
/// window, and a change within it extends the window; when the window passes
/// with no further change the burst is posted once. Pure, so the timing is
/// tested without real waits.
#[derive(Default)]
struct Coalescer {
    /// When the last change of the pending burst arrived, in milliseconds since
    /// the worker started; `None` while no burst is pending.
    pending: Option<u64>,
}

impl Coalescer {
    /// Folds a change at `now_ms` into the pending burst.
    fn mark(&mut self, now_ms: u64) {
        self.pending = Some(now_ms);
    }

    /// How long to wait for the next change: `INFINITE` while no burst is
    /// pending, else the time left in the quiet window (never negative).
    fn wait_ms(&self, now_ms: u64) -> u32 {
        match self.pending {
            None => INFINITE,
            Some(last) => DEBOUNCE_MS.saturating_sub(now_ms.saturating_sub(last)) as u32,
        }
    }

    /// Clears the pending burst and returns true to post, once `now_ms` is at
    /// least `DEBOUNCE_MS` past its last change.
    fn take_if_due(&mut self, now_ms: u64) -> bool {
        match self.pending {
            Some(last) if now_ms.saturating_sub(last) >= DEBOUNCE_MS => {
                self.pending = None;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_watch_waits_forever() {
        assert_eq!(Coalescer::default().wait_ms(10_000), INFINITE);
    }

    #[test]
    fn a_burst_of_changes_posts_once() {
        let mut coalescer = Coalescer::default();
        coalescer.mark(0);
        assert_eq!(coalescer.wait_ms(0), DEBOUNCE_MS as u32);
        // A change inside the window extends it instead of posting early.
        coalescer.mark(100);
        assert_eq!(coalescer.wait_ms(100), DEBOUNCE_MS as u32);
        assert!(!coalescer.take_if_due(200));
        assert!(coalescer.take_if_due(250));
        assert_eq!(coalescer.wait_ms(250), INFINITE);
    }

    #[test]
    fn changes_separated_by_the_window_post_apart() {
        let mut coalescer = Coalescer::default();
        coalescer.mark(0);
        assert!(coalescer.take_if_due(150));
        coalescer.mark(500);
        assert!(!coalescer.take_if_due(600));
        assert!(coalescer.take_if_due(650));
    }

    #[test]
    fn two_folders_debounce_independently() {
        let mut bursts = Bursts::default();
        bursts.add(1);
        bursts.add(2);
        bursts.mark(1, 0);
        // Folder 1 is due at 150 ms; folder 2 has no burst at all.
        assert!(!bursts.take_due(100).contains(&1));
        assert_eq!(bursts.take_due(150), vec![1]);
        assert_eq!(bursts.wait_ms(150), INFINITE);
        // A later change to folder 2 opens its own window.
        bursts.mark(2, 200);
        assert_eq!(bursts.wait_ms(250), 100);
        assert_eq!(bursts.take_due(350), vec![2]);
    }

    #[test]
    fn the_soonest_timeout_wins() {
        let mut bursts = Bursts::default();
        bursts.add(1);
        bursts.add(2);
        bursts.mark(1, 0);
        bursts.mark(2, 100);
        // At 100 ms folder 1 has 50 ms left and folder 2 has the full window.
        assert_eq!(bursts.wait_ms(100), 50);
        assert_eq!(bursts.take_due(150), vec![1]);
        assert_eq!(bursts.wait_ms(150), 100);
    }

    #[test]
    fn removing_an_unknown_slot_changes_nothing() {
        let mut bursts = Bursts::default();
        bursts.add(1);
        bursts.mark(1, 0);
        bursts.remove(2);
        assert_eq!(bursts.wait_ms(100), 50);
        assert_eq!(bursts.take_due(150), vec![1]);
    }
}
