//! A scene with its strings pre-encoded for the painter.

use super::*;

/// A scene with its strings pre-encoded, so painting does not allocate.
pub struct PreparedScene {
    pub(super) scene: SpaceScene,
    pub(super) title: Vec<u16>,
    /// The rename buffer while `scene.editing` is set.
    pub(super) edit: Option<Vec<u16>>,
    /// The item id and rename buffer while `scene.editing_item` is set.
    pub(super) edit_item: Option<(u64, Vec<u16>)>,
    pub(super) labels: Vec<Vec<u16>>,
    /// Each item's Details cells after Name, encoded once.
    pub(super) cells: Vec<Vec<Vec<u16>>>,
    /// The Details header labels, encoded once.
    pub(super) header: Vec<Vec<u16>>,
    /// The status bar's segments joined with Explorer's gaps, encoded once.
    pub(super) status_segments: Vec<u16>,
    /// `scene.placeholder`, encoded once.
    pub(super) placeholder: Option<Vec<u16>>,
}

impl PreparedScene {
    pub fn new(scene: SpaceScene) -> Self {
        let title = scene.title.encode_utf16().collect();
        let edit = scene
            .editing
            .as_ref()
            .map(|edit| edit.text().encode_utf16().collect());
        let edit_item = scene
            .editing_item
            .as_ref()
            .map(|(id, edit)| (*id, edit.text().encode_utf16().collect()));
        let labels = scene
            .items
            .iter()
            .map(|item| item.label.encode_utf16().collect())
            .collect();
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
        let status_segments = scene
            .status
            .as_ref()
            .map(|status| status.segments.join("   ").encode_utf16().collect())
            .unwrap_or_default();
        let placeholder = scene
            .placeholder
            .as_ref()
            .map(|text| text.encode_utf16().collect());
        Self {
            scene,
            title,
            edit,
            edit_item,
            labels,
            cells,
            header,
            status_segments,
            placeholder,
        }
    }

    pub fn scene(&self) -> &SpaceScene {
        &self.scene
    }
}
