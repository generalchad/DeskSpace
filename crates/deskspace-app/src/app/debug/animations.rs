//! The debug window's Animations section: slow motion, ignoring Windows'
//! switch, and playing an effect on one space or every space on demand.

use std::num::NonZeroU32;

use deskspace_core::SpaceId;
use deskspace_core::animation::{KINDS, Kind};
use deskspace_platform::{DebugControlKind, Ui, Way};

use crate::app::App;
use crate::settings::AnimationPage;

/// The slow-motion speeds offered: how many times slower, and the label.
const SPEEDS: [(NonZeroU32, &str); 4] = [
    (NonZeroU32::MIN, "Normal"),
    (times(2), "\u{00BD}\u{00D7}"),
    (times(4), "\u{00BC}\u{00D7}"),
    (times(10), "\u{2152}\u{00D7}"),
];

const fn times(slower: u32) -> NonZeroU32 {
    NonZeroU32::new(slower).expect("a speed is at least 1")
}

impl App {
    pub(super) fn speed_choice(&self) -> DebugControlKind {
        let speed = self.animation_tweak.speed;
        DebugControlKind::Choice {
            options: SPEEDS.iter().map(|(_, label)| label.to_string()).collect(),
            selected: SPEEDS
                .iter()
                .position(|(each, _)| *each == speed)
                .unwrap_or(0),
            enabled: true,
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
            enabled: true,
        }
    }

    pub(super) fn preview_kind_choice(&self) -> DebugControlKind {
        DebugControlKind::Choice {
            options: (0..KINDS.len())
                .map(|index| preview_kind(index).1.to_string())
                .collect(),
            selected: self.debug.as_ref().map_or(0, |tools| tools.preview_kind),
            enabled: true,
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
        let (kind, label) = preview_kind(tools.preview_kind);
        ui.preview_play(id.0, self.play(kind, Way::Out), self.play(kind, Way::In));
        tracing::info!(
            space = id.0,
            kind = label,
            "debug: played an effect preview"
        );
    }

    /// Plays the chosen kind's Out and then its In on every space at once.
    pub(super) fn play_preview_all(&mut self, ui: &mut Ui) {
        let Some(tools) = self.debug.as_ref() else {
            return;
        };
        let (kind, label) = preview_kind(tools.preview_kind);
        let (out, back) = (self.play(kind, Way::Out), self.play(kind, Way::In));
        let spaces = self.config.desktop.spaces();
        for space in spaces {
            ui.preview_play(space.id.0, out, back);
        }
        tracing::info!(
            spaces = spaces.len(),
            kind = label,
            "debug: played an effect preview on every space"
        );
    }

    /// The kind chosen in the Kind box.
    pub(super) fn chosen_kind(&self) -> Kind {
        preview_kind(self.debug.as_ref().map_or(0, |tools| tools.preview_kind)).0
    }

    pub(super) fn set_speed(&mut self, index: usize) {
        if let Some((speed, label)) = SPEEDS.get(index) {
            self.animation_tweak.speed = *speed;
            tracing::info!(speed = label, "debug: animation speed");
        }
    }
}

/// The Kind box's kind at `index` and its name, or the last one when
/// `index` is out of range. The box offers every kind in `KINDS` order,
/// named as the settings name it: the kind a preview plays, and the one the
/// Animation settings box shows. A preview plays Fold's effect out and back
/// like the others, since it never touches the model.
fn preview_kind(index: usize) -> (Kind, &'static str) {
    let kind = KINDS[index.min(KINDS.len() - 1)].kind;
    (kind, AnimationPage::of(kind).title)
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
        assert_eq!(SPEEDS[0].0, NonZeroU32::MIN);
        assert!(SPEEDS.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn the_kind_box_offers_every_kind_by_its_settings_name() {
        for (index, spec) in KINDS.iter().enumerate() {
            assert_eq!(
                preview_kind(index),
                (spec.kind, AnimationPage::of(spec.kind).title)
            );
        }
        assert_eq!(preview_kind(99).0, KINDS[KINDS.len() - 1].kind);
    }
}
