//! A scene with its strings pre-encoded for the painter.

use std::time::Instant;

use super::*;
use crate::render::ItemLayout;

/// A scene with its strings pre-encoded, so painting does not allocate.
pub struct PreparedScene {
    pub(super) scene: SpaceScene,
    pub(super) title: Vec<u16>,
    /// The rename buffer while `scene.editing` is set.
    pub(super) edit: Option<Vec<u16>>,
    /// The item id and rename buffer while `scene.editing_item` is set.
    pub(super) edit_item: Option<(u64, Vec<u16>)>,
    labels: Vec<Vec<u16>>,
    /// How many lines each label takes in an icon cell, so the highlight
    /// hugs it; worked out here, since painting lays no text out.
    label_lines: Vec<u8>,
    /// Each item's Details cells after Name, encoded once.
    cells: Vec<Vec<Vec<u16>>>,
    /// The Details header labels, encoded once.
    pub(super) header: Vec<Vec<u16>>,
    /// The group headers' labels, encoded once.
    pub(super) group_labels: Vec<Vec<u16>>,
    /// The status bar's segments joined with Explorer's gaps, encoded once.
    pub(super) status_segments: Vec<u16>,
    /// `scene.placeholder`, encoded once.
    pub(super) placeholder: Option<Vec<u16>>,
    /// `scene.notice`, encoded once.
    pub(super) notice: Option<Vec<u16>>,
}

impl PreparedScene {
    pub fn new(scene: SpaceScene) -> Self {
        let started = Instant::now();
        let title = scene.title.encode_utf16().collect();
        let edit = scene
            .editing
            .as_ref()
            .map(|edit| edit.text().encode_utf16().collect());
        let edit_item = scene
            .editing_item
            .as_ref()
            .map(|(id, edit)| (*id, edit.text().encode_utf16().collect()));
        let labels: Vec<Vec<u16>> = scene
            .items
            .iter()
            .map(|item| item.label.encode_utf16().collect())
            .collect();
        let label_lines = count_label_lines(&scene, &labels);
        let cells = scene
            .items
            .iter()
            .map(|item| {
                item.cells
                    .iter()
                    .map(|cell| cell.encode_utf16().collect())
                    .collect()
            })
            .collect();
        let header = scene
            .details
            .as_ref()
            .map(|details| {
                details
                    .columns
                    .iter()
                    .map(|column| column.label.encode_utf16().collect())
                    .collect()
            })
            .unwrap_or_default();
        let group_labels = scene
            .group_headers
            .iter()
            .map(|header| header.label.encode_utf16().collect())
            .collect();
        let status_segments = scene
            .status
            .as_ref()
            .map(|status| status.segments.join("   ").encode_utf16().collect())
            .unwrap_or_default();
        let placeholder = scene
            .placeholder
            .as_ref()
            .map(|text| text.encode_utf16().collect());
        let notice = scene
            .notice
            .as_ref()
            .map(|text| text.encode_utf16().collect());
        let elapsed = started.elapsed();
        let items = scene.items.len();
        if elapsed > crate::SCENE_STEP_BUDGET {
            tracing::debug!(items, elapsed_us = elapsed.as_micros(), "scene: prepared");
        } else {
            tracing::trace!(items, elapsed_us = elapsed.as_micros(), "scene: prepared");
        }
        Self {
            scene,
            title,
            edit,
            edit_item,
            labels,
            label_lines,
            cells,
            header,
            group_labels,
            status_segments,
            placeholder,
            notice,
        }
    }

    pub fn scene(&self) -> &SpaceScene {
        &self.scene
    }

    /// Each item's strings, in the scene's item order.
    pub(super) fn item_texts(&self) -> impl Iterator<Item = ItemText<'_>> {
        self.labels
            .iter()
            .zip(&self.label_lines)
            .zip(&self.cells)
            .map(|((label, lines), cells)| ItemText {
                label,
                lines: *lines,
                cells,
            })
    }
}

/// How many lines each of `scene`'s `labels` takes. Only an icon grid whose
/// look allows two lines has anything to measure; without the text formats
/// (no painter could draw either), every label keeps the lines allowed.
fn count_label_lines(scene: &SpaceScene, labels: &[Vec<u16>]) -> Vec<u8> {
    let allowed = scene.label_lines.clamp(1, 2);
    let measured = scene.item_layout == ItemLayout::Grid && allowed > 1;
    let graphics = measured.then(|| graphics().ok()).flatten();
    scene
        .items
        .iter()
        .zip(labels)
        .map(|(item, label)| match &graphics {
            Some(graphics) => graphics.item_label_lines(scene, item, label),
            None => allowed,
        })
        .collect()
}
