//! The debug window's Animations section: slow motion, ignoring Windows'
//! switch, and playing an effect on a space on demand.

use deskspace_platform::DebugControlKind;

use crate::app::App;

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

    pub(super) fn set_speed(&mut self, index: usize) {
        if let Some((speed, label)) = SPEEDS.get(index) {
            self.animation_tweak.speed = *speed;
            tracing::info!(speed = label, "debug: animation speed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_start_at_normal_and_slow_down() {
        assert_eq!(SPEEDS[0].0, 1);
        assert!(SPEEDS.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
}
