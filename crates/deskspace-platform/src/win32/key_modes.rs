//! What the keyboard hook is on for. The UI thread sets the modes and the
//! hook's thread reads them (`hook_thread`), so they are bits of one atomic
//! value and what they mean for a key is decided from one reading of it.

use std::sync::atomic::{AtomicU8, Ordering};

/// Inline editing consumes every key.
pub(crate) const EDITING: u8 = 1 << 0;
/// The selection keys are watched: an item is selected.
pub(crate) const WATCH_SELECTION: u8 = 1 << 1;
/// The keys that need a current space are watched.
pub(crate) const WATCH_PASTE: u8 = 1 << 2;
/// The keys that go back are watched: a lens is shown below its own folder.
pub(crate) const WATCH_BACK: u8 = 1 << 3;
/// A popup menu is being tracked, so every key passes through to it.
pub(crate) const MENU_OPEN: u8 = 1 << 4;
pub(crate) const WATCHES: u8 = WATCH_SELECTION | WATCH_PASTE | WATCH_BACK;

/// `mode` while `on`, else no bit: one mode's share of a `Modes::replace`.
pub(crate) fn mode_bits(mode: u8, on: bool) -> u8 {
    if on { mode } else { 0 }
}

/// The modes, as bits. They share one value so a key never sees half of a
/// change.
pub(crate) struct Modes(AtomicU8);

impl Modes {
    pub(crate) const fn new() -> Self {
        Self(AtomicU8::new(0))
    }

    pub(crate) fn load(&self) -> u8 {
        self.0.load(Ordering::SeqCst)
    }

    /// Sets the bits of `mask` to those of `bits` in one step and returns the
    /// new value.
    pub(crate) fn replace(&self, mask: u8, bits: u8) -> u8 {
        let replaced = |modes: u8| (modes & !mask) | (bits & mask);
        let before = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |modes| {
                Some(replaced(modes))
            })
            .unwrap_or_else(|modes| modes);
        replaced(before)
    }
}

/// Whether `modes` need the hook installed. The Back keys alone do not: a
/// lens that shows a subfolder is a current space.
pub(crate) fn wants_hook(modes: u8) -> bool {
    modes & (EDITING | WATCH_SELECTION | WATCH_PASTE) != 0
}

/// Which watch flags are set (the docs/ENGINEERING.md Keys table's columns).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Watch {
    /// An item is selected (`WATCH_SELECTION`).
    pub(crate) selection: bool,
    /// A space is current (`WATCH_PASTE`).
    pub(crate) paste: bool,
    /// The current space is a lens shown below its own folder
    /// (`WATCH_BACK`).
    pub(crate) back: bool,
}

impl Watch {
    pub(crate) fn any(self) -> bool {
        self.selection || self.paste
    }
}

/// What the hook does with a key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Handling {
    /// Leave it to whoever has the focus.
    Pass,
    /// Route it as an editing key of the inline rename.
    Edit,
    /// Route it if the watch names it.
    Watch(Watch),
}

/// What `modes` make of a key press. The hook is removed a moment after its
/// last mode clears, so a key can arrive with none set; it passes through.
pub(crate) fn handling(modes: u8) -> Handling {
    // An open menu owns every key (Esc to close, Enter to activate, arrows to
    // navigate), so pass them all through while one is tracked.
    if modes & MENU_OPEN != 0 {
        return Handling::Pass;
    }
    if modes & EDITING != 0 {
        return Handling::Edit;
    }
    let watch = Watch {
        selection: modes & WATCH_SELECTION != 0,
        paste: modes & WATCH_PASTE != 0,
        back: modes & WATCH_BACK != 0,
    };
    if watch.any() {
        Handling::Watch(watch)
    } else {
        Handling::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_modes_changes_only_the_masked_bits() {
        let modes = Modes::new();
        assert_eq!(modes.replace(MENU_OPEN, MENU_OPEN), MENU_OPEN);
        assert_eq!(
            modes.replace(WATCHES, WATCH_SELECTION | WATCH_BACK),
            MENU_OPEN | WATCH_SELECTION | WATCH_BACK
        );
        // A new set of watches replaces the old one whole.
        assert_eq!(modes.replace(WATCHES, WATCH_PASTE), MENU_OPEN | WATCH_PASTE);
        // Bits outside the mask are not written.
        assert_eq!(
            modes.replace(EDITING, EDITING | WATCH_BACK),
            MENU_OPEN | WATCH_PASTE | EDITING
        );
        // What stopping the hook clears leaves an open menu's bit to the
        // menu's owner.
        assert_eq!(modes.replace(EDITING | WATCHES, 0), MENU_OPEN);
        assert_eq!(modes.load(), MENU_OPEN);
    }

    #[test]
    fn a_mode_contributes_its_bit_only_while_on() {
        assert_eq!(mode_bits(WATCH_PASTE, true), WATCH_PASTE);
        assert_eq!(mode_bits(WATCH_PASTE, false), 0);
    }

    #[test]
    fn the_hook_is_wanted_for_a_rename_a_selection_or_a_current_space() {
        for modes in [EDITING, WATCH_SELECTION, WATCH_PASTE, WATCHES] {
            assert!(wants_hook(modes), "{modes:#07b}");
        }
        for modes in [0, WATCH_BACK, MENU_OPEN, MENU_OPEN | WATCH_BACK] {
            assert!(!wants_hook(modes), "{modes:#07b}");
        }
    }

    #[test]
    fn modes_decide_how_a_key_is_handled() {
        let watch = |selection, paste, back| {
            Handling::Watch(Watch {
                selection,
                paste,
                back,
            })
        };
        assert_eq!(handling(EDITING), Handling::Edit);
        assert_eq!(handling(WATCH_SELECTION), watch(true, false, false));
        assert_eq!(handling(WATCH_PASTE), watch(false, true, false));
        assert_eq!(handling(WATCH_PASTE | WATCH_BACK), watch(false, true, true));
        assert_eq!(handling(WATCHES), watch(true, true, true));
        // A rename takes every key, whatever is watched.
        assert_eq!(handling(EDITING | WATCHES), Handling::Edit);
        // An open menu owns every key.
        for modes in [EDITING, WATCHES, EDITING | WATCHES] {
            assert_eq!(handling(MENU_OPEN | modes), Handling::Pass);
        }
    }

    #[test]
    fn a_key_with_no_mode_set_passes_through() {
        // The hook outlives its last mode by a moment, and must not take a
        // key as an editing key then.
        assert_eq!(handling(0), Handling::Pass);
        assert_eq!(handling(WATCH_BACK), Handling::Pass);
    }
}
