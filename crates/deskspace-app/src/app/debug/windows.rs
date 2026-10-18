//! The debug window's Windows section: on-top, and tables of this process's
//! windows and of the monitors.

use deskspace_core::geometry::Rect;
use deskspace_platform::{DebugControlKind, TaskbarRow, Ui, monitors};

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

/// Each taskbar: topmost or not, and whether it is above the desktop layer
/// (under it, its clicks go to the layer).
pub(super) fn taskbars_text(taskbars: &[TaskbarRow]) -> String {
    if taskbars.is_empty() {
        return "none found".to_string();
    }
    taskbars
        .iter()
        .map(|taskbar| {
            let layer = match taskbar.above_layer {
                None => "no layer",
                Some(true) => "above the layer",
                Some(false) => "UNDER THE LAYER (clicks go to the layer)",
            };
            format!(
                "{}: topmost {}, {layer}",
                taskbar.class,
                yes_no(taskbar.topmost)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
pub(super) fn rect_text(rect: Rect) -> String {
    format!(
        "{}, {}  {} \u{00D7} {}",
        rect.x, rect.y, rect.width, rect.height
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taskbar_under_the_layer_stands_out() {
        let row = |above_layer| TaskbarRow {
            class: "Shell_TrayWnd".into(),
            rect: Rect::new(0, 1032, 2560, 48),
            topmost: false,
            above_layer,
        };
        assert!(row(Some(false)).is_blocked());
        assert!(!row(None).is_blocked());
        let text = taskbars_text(&[row(Some(false))]);
        assert_eq!(
            text,
            "Shell_TrayWnd: topmost no, UNDER THE LAYER (clicks go to the layer)"
        );
    }

    #[test]
    fn rects_read_as_origin_then_size() {
        assert_eq!(
            rect_text(Rect::new(-1920, 0, 1920, 1080)),
            "-1920, 0  1920 \u{00D7} 1080"
        );
    }
}
