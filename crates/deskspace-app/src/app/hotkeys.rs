//! The registered global hotkeys, one slot per `settings::HOTKEY_BINDINGS`
//! row. Start, Apply, and close share these bodies.

use deskspace_config::WindowSettings;
use deskspace_platform::{Hotkey, Ui, hotkey_id, parse_hotkey};

use crate::settings::HOTKEY_BINDINGS;

const HOTKEY_COUNT: usize = HOTKEY_BINDINGS.len();

/// The hotkey registered for each binding slot, if any.
#[derive(Debug, Default)]
pub(super) struct Hotkeys([Option<Hotkey>; HOTKEY_COUNT]);

impl Hotkeys {
    /// Registers every binding from the saved settings. A binding that cannot
    /// be registered is logged and left unbound.
    pub(super) fn register_all(&mut self, ui: &mut Ui, window: &WindowSettings) {
        for (slot, binding) in HOTKEY_BINDINGS.iter().enumerate() {
            if let Err(message) = self.register(ui, slot, binding.text(window)) {
                tracing::warn!(message, "hotkey unavailable");
            }
        }
    }

    /// Unregisters every hotkey, as DeskSpace closes.
    pub(super) fn unregister_all(&mut self, ui: &mut Ui) {
        for hotkey in self.0.iter_mut().filter_map(Option::take) {
            ui.unregister_hotkey(hotkey);
        }
    }

    /// Registers the binding in `slot` to `text`, replacing any previous one.
    /// An empty `text` leaves the action unbound.
    pub(super) fn register(&mut self, ui: &mut Ui, slot: usize, text: &str) -> Result<(), String> {
        if let Some(previous) = self.0[slot].take() {
            ui.unregister_hotkey(previous);
        }
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        let spec =
            parse_hotkey(text).ok_or_else(|| format!("\"{text}\" is not a valid hotkey."))?;
        match ui.register_hotkey(hotkey_index(slot), spec) {
            Ok(hotkey) => {
                self.0[slot] = Some(hotkey);
                Ok(())
            }
            Err(error) => {
                tracing::warn!(%error, hotkey = text, slot, "could not register hotkey");
                Err(format!("{text} is already in use by another app."))
            }
        }
    }

    pub(super) fn is_registered(&self, slot: usize) -> bool {
        self.0[slot].is_some()
    }

    /// The binding slot whose hotkey has the platform id `id`.
    pub(super) fn slot_for(id: i32) -> Option<usize> {
        (0..HOTKEY_COUNT).find(|slot| hotkey_id(hotkey_index(*slot)) == id)
    }
}

/// The platform registration index of the binding in `slot`.
fn hotkey_index(slot: usize) -> i32 {
    i32::try_from(slot).expect("a handful of hotkey bindings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_for_finds_each_binding_by_its_platform_id() {
        for slot in 0..HOTKEY_COUNT {
            assert_eq!(Hotkeys::slot_for(hotkey_id(hotkey_index(slot))), Some(slot));
        }
        assert_eq!(
            Hotkeys::slot_for(hotkey_id(hotkey_index(HOTKEY_COUNT))),
            None
        );
    }
}
