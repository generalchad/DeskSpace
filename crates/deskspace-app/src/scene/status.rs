//! Explorer's status bar along a space's bottom edge: item/selection counts and view buttons.

use deskspace_core::item::ItemKind;
use deskspace_core::space::DEFAULT_TITLE_HEIGHT;
use deskspace_core::{Item, Point, Rect, Space, ViewMode};
use deskspace_platform::{StatusBar, StatusButtonVisual, format_kb};

use crate::settings::{STATUS_BUTTONS, VIEW_MODES};

use super::ViewState;

/// Explorer's status-bar button size, gap, and edge inset at 96 DPI.
/// Provisional until the status bar's on-screen check measures Explorer's own.
const STATUS_BUTTON_SIZE: i32 = 22;
const STATUS_BUTTON_GAP: i32 = 2;
const STATUS_BAR_INSET: i32 = 4;

/// The status bar for `space`, or `None` while it is off or folded. Its
/// segments read the space's items and selection; an unavailable lens keeps
/// only the buttons, as Explorer does for a folder it cannot open.
pub(super) fn status_bar(space: &Space, view: &ViewState) -> Option<StatusBar> {
    let height = space.status_bar_height();
    if height <= 0 {
        return None;
    }
    let rect = Rect::new(0, space.rect().height - height, space.rect().width, height);
    let segments = if view.folder_unavailable {
        Vec::new()
    } else {
        let selected: Vec<&Item> = view
            .selected
            .iter()
            .filter_map(|id| space.items().iter().find(|item| item.id == *id))
            .collect();
        status_segments(
            space.items().len(),
            selected.len(),
            selected_bytes(&selected),
        )
    };
    Some(StatusBar {
        height,
        rect,
        segments,
        buttons: status_buttons(
            rect,
            space.title_height,
            space.view().mode,
            view.hovered_status,
        ),
    })
}

/// Explorer's status segments: the item count, then the selection, then the
/// selected files' size when every selected item is a file with a known size.
fn status_segments(count: usize, selected: usize, selected_bytes: Option<u64>) -> Vec<String> {
    let mut segments = vec![format!("{count} item{}", if count == 1 { "" } else { "s" })];
    if selected > 0 {
        segments.push(format!(
            "{selected} item{} selected",
            if selected == 1 { "" } else { "s" }
        ));
    }
    if let Some(bytes) = selected_bytes {
        segments.push(format_kb(bytes));
    }
    segments
}

/// The selected items' total size, or `None` unless every one is a file with
/// a known size (so a folder or an unreadable path hides it).
fn selected_bytes(selected: &[&Item]) -> Option<u64> {
    if selected.is_empty()
        || selected
            .iter()
            .any(|item| item.kind == ItemKind::Folder || item.size.is_none())
    {
        return None;
    }
    Some(selected.iter().filter_map(|item| item.size).sum())
}

/// The view buttons at the right of status bar `rect`, from `STATUS_BUTTONS`.
fn status_buttons(
    rect: Rect,
    title_height: i32,
    mode: ViewMode,
    hovered: Option<usize>,
) -> Vec<StatusButtonVisual> {
    let scale = (title_height as f32 / DEFAULT_TITLE_HEIGHT as f32).max(0.1);
    let scaled = |value: i32| ((value as f32 * scale).round() as i32).max(1);
    let size = scaled(STATUS_BUTTON_SIZE);
    let gap = scaled(STATUS_BUTTON_GAP);
    let inset = scaled(STATUS_BAR_INSET);
    let count = STATUS_BUTTONS.len() as i32;
    let mut x = rect.right() - inset - count * size - (count - 1) * gap;
    let y = rect.y + (rect.height - size) / 2;
    STATUS_BUTTONS
        .iter()
        .enumerate()
        .map(|(index, button)| {
            let visual = StatusButtonVisual {
                rect: Rect::new(x, y, size, size),
                mode: button.mode,
                glyph: button.glyph,
                checked: button.mode == mode,
                hovered: hovered == Some(index),
            };
            x += size + gap;
            visual
        })
        .collect()
}

/// The status button at `point` (desktop coordinates) on `space`, if any.
pub(crate) fn status_button_at(space: &Space, point: Point) -> Option<usize> {
    (0..STATUS_BUTTONS.len()).find(|index| {
        status_button_rect(space, *index).is_some_and(|rect| {
            rect.contains(Point::new(
                point.x - space.rect().x,
                point.y - space.rect().y,
            ))
        })
    })
}

/// The rect of status button `index` on `space`, in window coordinates.
pub(crate) fn status_button_rect(space: &Space, index: usize) -> Option<Rect> {
    let height = space.status_bar_height();
    if height <= 0 {
        return None;
    }
    let rect = Rect::new(0, space.rect().height - height, space.rect().width, height);
    status_buttons(rect, space.title_height, space.view().mode, None)
        .get(index)
        .map(|button| button.rect)
}

/// Explorer's tip for status button `index`, with its view's Ctrl+Shift
/// digit when it has one.
pub(crate) fn status_tip(index: usize) -> Option<String> {
    let button = STATUS_BUTTONS.get(index)?;
    let digit = VIEW_MODES
        .iter()
        .find(|(mode, ..)| *mode == button.mode)
        .and_then(|(.., digit)| *digit);
    Some(match digit {
        Some(digit) => format!("{} (Ctrl+Shift+{digit})", button.tip),
        None => button.tip.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use deskspace_core::{ItemFacts, ItemId};

    use super::*;

    #[test]
    fn status_segments_read_explorers_words() {
        assert_eq!(status_segments(0, 0, None), ["0 items"]);
        assert_eq!(status_segments(1, 0, None), ["1 item"]);
        assert_eq!(status_segments(12, 0, None), ["12 items"]);
        assert_eq!(
            status_segments(12, 1, None),
            ["12 items", "1 item selected"]
        );
        let sized = status_segments(12, 3, Some(2048));
        assert_eq!(sized.len(), 3);
        assert_eq!(sized[2], format_kb(2048));
    }

    #[test]
    fn the_selected_size_shows_only_for_files_with_sizes() {
        let file = Item::new(
            ItemId(1),
            "C:/missing/a.txt",
            ItemFacts {
                size: Some(1024),
                ..ItemFacts::default()
            },
        );
        let folder = Item::new(
            ItemId(2),
            "C:/missing/b",
            ItemFacts {
                is_folder: true,
                ..ItemFacts::default()
            },
        );
        let unknown = Item::new(ItemId(3), "C:/missing/c.txt", ItemFacts::default());
        assert_eq!(selected_bytes(&[&file]), Some(1024));
        assert_eq!(selected_bytes(&[&file, &folder]), None);
        assert_eq!(selected_bytes(&[&file, &unknown]), None);
        assert_eq!(selected_bytes(&[]), None);
    }
}
