//! The debug window's Windows section: on-top, and tables of this process's
//! windows and of the monitors.

use deskspace_core::geometry::Rect;
use deskspace_platform::{DebugControlKind, Ui, monitors};

use crate::app::App;

impl App {
    /// This process's windows, top first.
    pub(super) fn window_table(&self, ui: &Ui) -> DebugControlKind {
        let rows = ui
            .debug_windows()
            .into_iter()
            .map(|window| {
                vec![
                    window.class,
                    window.title,
                    window.space.map(|id| id.to_string()).unwrap_or_default(),
                    rect_text(window.rect),
                    window.dpi.to_string(),
                    yes_no(window.visible).to_string(),
                ]
            })
            .collect();
        table(&["Class", "Title", "Space", "Rect", "DPI", "Visible"], rows)
    }

    /// Every display Windows reports.
    pub(super) fn monitor_table(&self) -> DebugControlKind {
        let rows = monitors()
            .into_iter()
            .map(|monitor| {
                vec![
                    rect_text(monitor.rect),
                    rect_text(monitor.work),
                    monitor.dpi.to_string(),
                    yes_no(monitor.primary).to_string(),
                ]
            })
            .collect();
        table(&["Rect", "Work area", "DPI", "Primary"], rows)
    }

    /// Keeps the spaces above other windows, or back at the bottom, for this
    /// session only, as `--on-top` does.
    pub(super) fn set_on_top(&mut self, ui: &mut Ui, on: bool) {
        ui.set_on_top(on);
        tracing::info!(on_top = on, "debug: spaces kept on top or not");
    }
}

fn table(columns: &[&str], rows: Vec<Vec<String>>) -> DebugControlKind {
    DebugControlKind::Table {
        columns: columns.iter().map(|column| column.to_string()).collect(),
        rows,
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// `rect` as `x, y  w × h`.
fn rect_text(rect: Rect) -> String {
    format!(
        "{}, {}  {} \u{00D7} {}",
        rect.x, rect.y, rect.width, rect.height
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_read_as_origin_then_size() {
        assert_eq!(
            rect_text(Rect::new(-1920, 0, 1920, 1080)),
            "-1920, 0  1920 \u{00D7} 1080"
        );
    }
}
