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
        Self {
            scene,
            title,
            edit,
            edit_item,
            labels,
        }
    }

    pub fn scene(&self) -> &SpaceScene {
        &self.scene
    }
}
