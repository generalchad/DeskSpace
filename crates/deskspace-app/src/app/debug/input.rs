//! The debug window's Input section: what the app is receiving and holding
//! now, and a marker to split the log between repro steps.

use deskspace_platform::{Event, SpaceEvent};

use crate::app::App;

impl App {
    /// Logs `event` when the Log events switch is on, leaving out the ones
    /// that arrive many times a second (`is_noisy`).
    pub(crate) fn trace_event(&self, event: &Event) {
        if self.debug.as_ref().is_some_and(|tools| tools.log_events) && !is_noisy(event) {
            tracing::info!(?event, "debug: event");
        }
    }

    pub(super) fn set_log_events(&mut self, on: bool) {
        if let Some(tools) = self.debug.as_mut() {
            tools.log_events = on;
        }
        tracing::info!(on, "debug: log events");
    }

    /// Writes a numbered marker line to the log, to tell repro steps apart.
    pub(super) fn mark_log(&mut self) {
        let Some(tools) = self.debug.as_mut() else {
            return;
        };
        tools.marks += 1;
        tracing::info!("debug: ---------- mark {} ----------", tools.marks);
    }

    /// The selection, the hover, any drag or marquee, and a rename.
    pub(super) fn input_text(&self) -> String {
        let selection = &self.selection;
        let pointer = &self.pointer;
        let or_none = |text: Option<String>| text.unwrap_or_else(|| "none".to_string());
        format!(
            "Current surface: {}\nSelected: {}\nFocused: {}\nCut: {}\nHovered: {}\nDragged: {}\nMarquee: {}\nRenaming: {}",
            or_none(selection.surface.map(|surface| format!("{surface:?}"))),
            ids(&selection.items),
            or_none(selection.focused.map(|item| item.0.to_string())),
            selection.cut.len(),
            or_none(
                pointer
                    .hovered
                    .map(|(surface, item)| format!("{} on {surface:?}", item.0))
            ),
            or_none(
                pointer
                    .dragged
                    .as_ref()
                    .map(|(surface, items)| format!("{} on {surface:?}", ids(items)))
            ),
            or_none(
                pointer
                    .marquee
                    .map(|(surface, rect)| format!("{rect:?} on {surface:?}"))
            ),
            if self.edit.is_active() { "yes" } else { "no" },
        )
    }
}

/// Item ids as `[3, 7, 9]`, or `none`.
fn ids(items: &[deskspace_core::ItemId]) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    let ids: Vec<String> = items.iter().map(|item| item.0.to_string()).collect();
    format!("[{}]", ids.join(", "))
}

/// Events that arrive many times a second: timers (the debug window's own
/// among them), worker wakes, and pointer moves over a space.
fn is_noisy(event: &Event) -> bool {
    matches!(
        event,
        Event::Timer { .. }
            | Event::Wake { .. }
            | Event::Space {
                event: SpaceEvent::MouseMove { .. },
                ..
            }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timers_and_pointer_moves_are_left_out_but_input_is_logged() {
        assert!(is_noisy(&Event::Timer { id: 11 }));
        assert!(is_noisy(&Event::Wake { id: 0 }));
        assert!(!is_noisy(&Event::SelectAll));
        assert!(!is_noisy(&Event::Started));
    }

    #[test]
    fn item_ids_read_as_a_list() {
        use deskspace_core::ItemId;
        assert_eq!(ids(&[]), "none");
        assert_eq!(ids(&[ItemId(3), ItemId(7)]), "[3, 7]");
    }
}
