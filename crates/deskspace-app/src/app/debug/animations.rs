//! The debug window's Animations section: slow motion, ignoring Windows'
//! switch, and playing an effect on a space on demand.

use deskspace_core::SpaceId;
use deskspace_core::animation::Kind;
use deskspace_platform::{DebugControlKind, Ui, Way};

use crate::app::App;

/// The kinds a preview can play. Fold is checked from its own menu entry.
const PREVIEW_KINDS: [(Kind, &str); 2] = [(Kind::Startup, "Startup"), (Kind::Delete, "Delete")];

/// The slow-motion speeds offered: how many times slower, and the label.
const SPEEDS: [(u32, &str); 4] = [
    (1, "Normal"),
    (2, "\u{00BD}\u{00D7}"),
    (4, "\u{00BC}\u{00D7}"),
    (10, "\u{2152}\u{00D7}"),
];

impl App {
    pub(super) fn speed_choice(&self) -> DebugControlKind {
        let speed = self.animation_tweak.speed.max(1);
        DebugControlKind::Choice {
            options: SPEEDS.iter().map(|(_, label)| label.to_string()).collect(),
            selected: SPEEDS
                .iter()
                .position(|(each, _)| *each == speed)
                .unwrap_or(0),
        }
    }

    /// The spaces' titles in model order, with the preview's space chosen.
    pub(super) fn preview_space_choice(&self) -> DebugControlKind {
        let spaces = self.config.desktop.spaces();
        let ids: Vec<SpaceId> = spaces.iter().map(|space| space.id).collect();
        let chosen = self.debug.as_ref().and_then(|tools| tools.preview_space);
        DebugControlKind::Choice {
            options: spaces
                .iter()
                .map(|space| space.title().to_string())
                .collect(),
            selected: chosen_index(&ids, chosen),
        }
    }

    pub(super) fn preview_kind_choice(&self) -> DebugControlKind {
        DebugControlKind::Choice {
            options: PREVIEW_KINDS
                .iter()
                .map(|(_, label)| label.to_string())
                .collect(),
            selected: self.debug.as_ref().map_or(0, |tools| tools.preview_kind),
        }
    }

    pub(super) fn choose_preview_space(&mut self, index: usize) {
        let id = self
            .config
            .desktop
            .spaces()
            .get(index)
            .map(|space| space.id);
        if let Some(tools) = self.debug.as_mut() {
            tools.preview_space = id;
        }
    }

    /// Plays the chosen kind's Out and then its In on the chosen space, with
    /// the current settings and slow motion.
    pub(super) fn play_preview(&mut self, ui: &mut Ui) {
        let ids: Vec<SpaceId> = self
            .config
            .desktop
            .spaces()
            .iter()
            .map(|space| space.id)
            .collect();
        let Some(tools) = self.debug.as_ref() else {
            return;
        };
        let Some(id) = ids.get(chosen_index(&ids, tools.preview_space)).copied() else {
            return;
        };
        let (kind, label) = PREVIEW_KINDS[tools.preview_kind.min(PREVIEW_KINDS.len() - 1)];
        ui.preview_play(id.0, self.play(kind, Way::Out), self.play(kind, Way::In));
        tracing::info!(
            space = id.0,
            kind = label,
            "debug: played an effect preview"
        );
    }

    pub(super) fn set_speed(&mut self, index: usize) {
        if let Some((speed, label)) = SPEEDS.get(index) {
            self.animation_tweak.speed = *speed;
            tracing::info!(speed = label, "debug: animation speed");
        }
    }
}

/// Where `chosen` is in `ids`, or the first space when it is gone or was
/// never chosen.
fn chosen_index(ids: &[SpaceId], chosen: Option<SpaceId>) -> usize {
    chosen
        .and_then(|chosen| ids.iter().position(|id| *id == chosen))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chosen_space_is_found_and_a_deleted_one_falls_back_to_the_first() {
        let ids = [SpaceId(4), SpaceId(7), SpaceId(9)];
        assert_eq!(chosen_index(&ids, Some(SpaceId(7))), 1);
        assert_eq!(chosen_index(&ids, Some(SpaceId(5))), 0);
        assert_eq!(chosen_index(&ids, None), 0);
    }

    #[test]
    fn speeds_start_at_normal_and_slow_down() {
        assert_eq!(SPEEDS[0].0, 1);
        assert!(SPEEDS.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
}
