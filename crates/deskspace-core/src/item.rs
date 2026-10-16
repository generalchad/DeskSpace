use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::id::ItemId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ItemKind {
    File,
    Folder,
    Shortcut,
    Drive,
    #[default]
    Other,
}

impl ItemKind {
    /// The kind of `path`: a folder when the caller found one there, otherwise
    /// decided by the extension alone.
    pub fn classify(path: &Path, is_folder: bool) -> Self {
        if is_folder {
            return Self::Folder;
        }
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("lnk") | Some("url") => Self::Shortcut,
            Some(_) => Self::File,
            None => Self::Other,
        }
    }
}

/// What the file system knows about an item's path, read by the caller (core
/// does no IO). The default is what an unreadable path yields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemFacts {
    pub is_folder: bool,
    /// Size in bytes; `None` for folders.
    pub size: Option<u64>,
    /// Last modification time in seconds since the Unix epoch.
    pub modified: Option<u64>,
    pub compressed: bool,
    pub encrypted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub path: PathBuf,
    pub label: String,
    #[serde(default)]
    pub kind: ItemKind,
    /// Top-left of the item's grid cell, relative to its space's content area.
    #[serde(default)]
    pub position: Point,
    /// File size in bytes, from the `ItemFacts` the item was created with.
    /// `None` for folders and paths that cannot be read.
    #[serde(default)]
    pub size: Option<u64>,
    /// Last modification time in seconds since the Unix epoch, from the
    /// item's `ItemFacts`. `None` when unavailable.
    #[serde(default)]
    pub modified: Option<u64>,
    /// NTFS compressed attribute, for Explorer's compressed label color
    /// (`SSF_SHOWCOMPCOLOR`).
    #[serde(default)]
    pub compressed: bool,
    /// NTFS encrypted attribute, for Explorer's encrypted label color.
    #[serde(default)]
    pub encrypted: bool,
}

impl Item {
    pub fn new(id: ItemId, path: impl Into<PathBuf>, facts: ItemFacts) -> Self {
        let path = path.into();
        Self {
            id,
            kind: ItemKind::classify(&path, facts.is_folder),
            label: default_label(&path),
            path,
            position: Point::ZERO,
            size: facts.size,
            modified: facts.modified,
            compressed: facts.compressed,
            encrypted: facts.encrypted,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_kind(mut self, kind: ItemKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn file_name(&self) -> Option<&str> {
        self.path.file_name().and_then(|name| name.to_str())
    }
}

fn default_label(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_strips_extension() {
        let item = Item::new(
            ItemId(1),
            "C:/Users/me/Desktop/report.docx",
            ItemFacts::default(),
        );
        assert_eq!(item.label, "report");
    }

    #[test]
    fn shortcut_kind_detected_by_extension() {
        assert_eq!(
            ItemKind::classify(Path::new("thing.lnk"), false),
            ItemKind::Shortcut
        );
    }

    #[test]
    fn a_folder_is_a_folder_whatever_its_extension() {
        assert_eq!(
            ItemKind::classify(Path::new("archive.zip"), true),
            ItemKind::Folder
        );
    }

    #[test]
    fn new_item_takes_its_facts() {
        let facts = ItemFacts {
            size: Some(42),
            modified: Some(7),
            compressed: true,
            ..ItemFacts::default()
        };
        let item = Item::new(ItemId(1), "C:/a.txt", facts);
        assert_eq!(item.kind, ItemKind::File);
        assert_eq!((item.size, item.modified), (Some(42), Some(7)));
        assert!(item.compressed && !item.encrypted);
    }
}
