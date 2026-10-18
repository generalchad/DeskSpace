//! Where the debug window's parts go: pure, so it can be tested.

use deskspace_core::geometry::Rect;

use crate::ui::DebugControlKind;

/// Width of the section list, at 96 DPI.
const RAIL_WIDTH: i32 = 168;
/// Space around and between the parts, at 96 DPI.
const EDGE: i32 = 12;
/// The smallest client the window allows, at 96 DPI.
pub(super) const MIN_WIDTH: i32 = 760;
pub(super) const MIN_HEIGHT: i32 = 560;
/// The console's least height, at 96 DPI.
const CONSOLE_MIN: i32 = 200;
/// Width of a control's label column, at 96 DPI.
const LABEL_WIDTH: i32 = 220;
/// Space around a section's rows inside the panel, at 96 DPI.
const ROW_PADDING: i32 = 4;
/// Space between a section's rows, at 96 DPI.
const ROW_GAP: i32 = 8;
const BUTTON_HEIGHT: i32 = 30;
const FIELD_HEIGHT: i32 = 24;
/// One line of a `Readout`, at 96 DPI.
const LINE_HEIGHT: i32 = 18;
/// A `Readout` grows to this many lines, then scrolls.
pub(super) const READOUT_MAX_LINES: usize = 8;
/// A `Table` shows this many rows, then scrolls.
const TABLE_ROWS: i32 = 8;
const TABLE_ROW_HEIGHT: i32 = 22;
const TABLE_HEADER_HEIGHT: i32 = 26;
/// Space between a label and its control, at 96 DPI.
const LABEL_GAP: i32 = 8;
/// A push button's least width, and the room around its caption, at 96 DPI.
const BUTTON_MIN_WIDTH: i32 = 120;
const BUTTON_TEXT_MARGIN: i32 = 32;
/// The drop-down list of a `Choice`, at 96 DPI: a combo box's window height
/// includes it.
const DROP_DOWN_HEIGHT: i32 = 200;
/// The console's toolbar row, at 96 DPI.
const TOOLBAR_HEIGHT: i32 = 28;
const TOOL_GAP: i32 = 8;
const LEVEL_WIDTH: i32 = 96;
const PAUSE_WIDTH: i32 = 72;
const AUTO_SCROLL_WIDTH: i32 = 104;
const TOOL_BUTTON_WIDTH: i32 = 88;

/// The debug window's three parts, in client pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DebugLayout {
    /// The section list, on the left of the top half.
    pub rail: Rect,
    /// The chosen section's controls, right of the rail.
    pub panel: Rect,
    /// The log console, across the bottom half.
    pub console: Rect,
}

/// The console's parts, in client pixels: a toolbar row above the log list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ConsoleLayout {
    pub level: Rect,
    /// The filter box takes the toolbar's spare width.
    pub filter: Rect,
    pub pause: Rect,
    pub auto_scroll: Rect,
    pub clear: Rect,
    pub copy: Rect,
    pub save: Rect,
    pub list: Rect,
}

/// Converts 96-DPI layout units to pixels at `scale`.
pub(super) fn s(value: i32, scale: f32) -> i32 {
    (value as f32 * scale).round() as i32
}

/// Lays the window out in a `width` × `height` client at `scale`.
pub(super) fn debug_layout(width: i32, height: i32, scale: f32) -> DebugLayout {
    let edge = s(EDGE, scale);
    let rail_width = s(RAIL_WIDTH, scale);
    let console_height = (height / 2).max(s(CONSOLE_MIN, scale)).min(height);
    let top_height = (height - console_height - 2 * edge).max(0);
    let inner_width = (width - 2 * edge).max(0);
    DebugLayout {
        rail: Rect::new(edge, edge, rail_width.min(inner_width), top_height),
        panel: Rect::new(
            2 * edge + rail_width,
            edge,
            (inner_width - rail_width - edge).max(0),
            top_height,
        ),
        console: Rect::new(
            edge,
            height - console_height,
            inner_width,
            (console_height - edge).max(0),
        ),
    }
}

/// Lays the console out in `console` (from `debug_layout`): the Level box
/// and the filter on the left, the check boxes and buttons on the right.
pub(super) fn console_layout(console: Rect, scale: f32) -> ConsoleLayout {
    let gap = s(TOOL_GAP, scale);
    let height = s(TOOLBAR_HEIGHT, scale);
    let tool = |x: i32, width: i32| Rect::new(x, console.y, width, height);
    let button = s(TOOL_BUTTON_WIDTH, scale);
    let save = tool(console.right() - button, button);
    let copy = tool(save.x - gap - button, button);
    let clear = tool(copy.x - gap - button, button);
    let auto_scroll_width = s(AUTO_SCROLL_WIDTH, scale);
    let auto_scroll = tool(clear.x - gap - auto_scroll_width, auto_scroll_width);
    let pause_width = s(PAUSE_WIDTH, scale);
    let pause = tool(auto_scroll.x - gap - pause_width, pause_width);
    let level = tool(console.x, s(LEVEL_WIDTH, scale));
    let filter_x = level.right() + gap;
    let filter = tool(filter_x, (pause.x - gap - filter_x).max(0));
    let list_top = console.y + height + gap;
    ConsoleLayout {
        level,
        filter,
        pause,
        auto_scroll,
        clear,
        copy,
        save,
        list: Rect::new(
            console.x,
            list_top,
            console.width,
            (console.bottom() - list_top).max(0),
        ),
    }
}

/// A combo box's window rect: `field` with its drop-down list below.
pub(super) fn with_drop_down(field: Rect, scale: f32) -> Rect {
    Rect::new(
        field.x,
        field.y,
        field.width,
        field.height + s(DROP_DOWN_HEIGHT, scale),
    )
}

/// The smallest client size, in pixels at `scale`.
pub(super) fn min_client(scale: f32) -> (i32, i32) {
    (s(MIN_WIDTH, scale), s(MIN_HEIGHT, scale))
}

/// How many lines a `Readout` box shows for `text`: all of them, up to
/// `READOUT_MAX_LINES`.
fn readout_lines(text: &str) -> usize {
    text.lines().count().clamp(1, READOUT_MAX_LINES)
}

/// The height of one control's row at `scale`.
pub(super) fn row_height(kind: &DebugControlKind, scale: f32) -> i32 {
    let units = match kind {
        DebugControlKind::Button { .. } => BUTTON_HEIGHT,
        DebugControlKind::Toggle { .. } | DebugControlKind::Choice { .. } => FIELD_HEIGHT,
        DebugControlKind::Readout { text } => {
            readout_lines(text) as i32 * LINE_HEIGHT + 2 * ROW_PADDING
        }
        DebugControlKind::Table { .. } => TABLE_HEADER_HEIGHT + TABLE_ROWS * TABLE_ROW_HEIGHT,
    };
    s(units, scale)
}

/// The top of each control's row in a section, and the section's whole
/// height, padding included.
pub(super) fn section_rows<'a>(
    kinds: impl IntoIterator<Item = &'a DebugControlKind>,
    scale: f32,
) -> (Vec<i32>, i32) {
    let padding = s(ROW_PADDING, scale);
    let gap = s(ROW_GAP, scale);
    let mut tops = Vec::new();
    let mut y = padding;
    for kind in kinds {
        tops.push(y);
        y += row_height(kind, scale) + gap;
    }
    let height = if tops.is_empty() {
        2 * padding
    } else {
        y - gap + padding
    };
    (tops, height)
}

/// Where one control's label and main control go, in page pixels, for a
/// row at `top` in a page `width` wide. `caption_width` is the measured
/// width of a button's caption; other kinds ignore it.
pub(super) fn control_rects(
    kind: &DebugControlKind,
    top: i32,
    width: i32,
    caption_width: i32,
    scale: f32,
) -> (Option<Rect>, Rect) {
    let padding = s(ROW_PADDING, scale);
    let height = row_height(kind, scale);
    if let DebugControlKind::Button { .. } = kind {
        let button = (caption_width + s(BUTTON_TEXT_MARGIN, scale))
            .max(s(BUTTON_MIN_WIDTH, scale))
            .min((width - 2 * padding).max(0));
        return (None, Rect::new(padding, top, button, height));
    }
    let label_width = s(LABEL_WIDTH, scale);
    let label = Rect::new(
        padding,
        top,
        label_width,
        height.min(s(FIELD_HEIGHT, scale)),
    );
    let x = padding + label_width + s(LABEL_GAP, scale);
    let main = match kind {
        DebugControlKind::Toggle { .. } => Rect::new(x, top, s(FIELD_HEIGHT, scale), height),
        DebugControlKind::Choice { .. } => with_drop_down(
            Rect::new(x, top, (width - x - padding).max(0), height),
            scale,
        ),
        _ => Rect::new(x, top, (width - x - padding).max(0), height),
    };
    (Some(label), main)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(layout: DebugLayout) -> [Rect; 3] {
        [layout.rail, layout.panel, layout.console]
    }

    #[test]
    fn no_part_overlaps_another() {
        for (width, height, scale) in [(760, 560, 1.0), (1400, 1000, 1.0), (1900, 1400, 1.5)] {
            let [rail, panel, console] = parts(debug_layout(width, height, scale));
            assert!(!rail.intersects(panel), "{width}x{height}");
            assert!(!rail.intersects(console), "{width}x{height}");
            assert!(!panel.intersects(console), "{width}x{height}");
        }
    }

    #[test]
    fn every_part_fits_the_client() {
        for (width, height, scale) in [(760, 560, 1.0), (1400, 1000, 1.0), (1140, 840, 1.5)] {
            let client = Rect::new(0, 0, width, height);
            for part in parts(debug_layout(width, height, scale)) {
                assert!(part.width > 0 && part.height > 0, "{part:?} is empty");
                assert_eq!(part.union(client), client, "{part:?} leaves the client");
            }
        }
    }

    #[test]
    fn the_console_grows_with_the_height() {
        let short = debug_layout(800, 600, 1.0).console;
        let tall = debug_layout(800, 1000, 1.0).console;
        assert!(tall.height > short.height);
        assert_eq!(tall.bottom(), 1000 - EDGE);
    }

    #[test]
    fn the_console_keeps_its_minimum() {
        let console = debug_layout(800, 300, 1.0).console;
        assert_eq!(console.height, CONSOLE_MIN - EDGE);
    }

    #[test]
    fn the_console_parts_fit_and_do_not_overlap_at_the_minimum_size() {
        for scale in [1.0, 1.5] {
            let (width, height) = min_client(scale);
            let console = debug_layout(width, height, scale).console;
            let parts = console_layout(console, scale);
            let all = [
                parts.level,
                parts.filter,
                parts.pause,
                parts.auto_scroll,
                parts.clear,
                parts.copy,
                parts.save,
                parts.list,
            ];
            for (index, part) in all.iter().enumerate() {
                assert!(part.width > 0 && part.height > 0, "{part:?} is empty");
                assert_eq!(part.union(console), console, "{part:?} leaves the console");
                for other in &all[index + 1..] {
                    assert!(!part.intersects(*other), "{part:?} overlaps {other:?}");
                }
            }
        }
    }

    #[test]
    fn rows_stack_with_gaps_and_padding() {
        let kinds = [
            DebugControlKind::Button { enabled: true },
            DebugControlKind::Toggle { on: false },
        ];
        let (tops, height) = section_rows(&kinds, 1.0);
        assert_eq!(
            tops,
            vec![ROW_PADDING, ROW_PADDING + BUTTON_HEIGHT + ROW_GAP]
        );
        assert_eq!(height, tops[1] + FIELD_HEIGHT + ROW_PADDING);
    }

    #[test]
    fn labels_sit_left_of_their_controls_and_buttons_stand_alone() {
        let toggle = DebugControlKind::Toggle { on: true };
        let (label, main) = control_rects(&toggle, 10, 600, 0, 1.0);
        let label = label.expect("a toggle has a label");
        assert!(label.right() <= main.left());
        assert_eq!((label.y, main.y), (10, 10));
        let button = DebugControlKind::Button { enabled: true };
        let (label, main) = control_rects(&button, 10, 600, 300, 1.0);
        assert!(label.is_none());
        assert_eq!(main.width, 300 + BUTTON_TEXT_MARGIN);
        let (_, narrow) = control_rects(&button, 10, 200, 300, 1.0);
        assert_eq!(narrow.right(), 200 - ROW_PADDING);
    }

    #[test]
    fn a_readout_grows_to_its_line_limit() {
        let readout = |lines: usize| DebugControlKind::Readout {
            text: vec!["line"; lines].join("\n"),
        };
        assert!(row_height(&readout(3), 1.0) > row_height(&readout(1), 1.0));
        assert_eq!(
            row_height(&readout(READOUT_MAX_LINES + 5), 1.0),
            row_height(&readout(READOUT_MAX_LINES), 1.0)
        );
        assert_eq!(readout_lines(""), 1);
    }
}
