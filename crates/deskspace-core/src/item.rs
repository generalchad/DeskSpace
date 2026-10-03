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
    pub fn classify(path: &Path) -> Self {
        if path.is_dir() {
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
    /// File size in bytes, read when the item is created. `None` for folders
    /// and paths that cannot be read.
    #[serde(default)]
    pub size: Option<u64>,
    /// Last modification time in seconds since the Unix epoch, read when the
    /// item is created. `None` when unavailable.
    #[serde(default)]
    pub modified: Option<u64>,
}

impl Item {
    pub fn new(id: ItemId, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let kind = ItemKind::classify(&path);
        let label = default_label(&path);
        let (size, modified) = read_metadata(&path);
        Self {
            id,
            path,
            label,
            kind,
            position: Point::ZERO,
            size,
            modified,
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

/// The size (files only) and last-modified time of `path`, when it can be
/// read. Like `ItemKind::classify`, this inspects the filesystem once at
/// creation; there is no watcher yet, so the values can go stale.
fn read_metadata(path: &Path) -> (Option<u64>, Option<u64>) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return (None, None);
    };
    let size = metadata.is_file().then_some(metadata.len());
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_secs());
    (size, modified)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_strips_extension() {
        let item = Item::new(ItemId(1), "C:/Users/me/Desktop/report.docx");
        assert_eq!(item.label, "report");
    }

    #[test]
    fn shortcut_kind_detected_by_extension() {
        assert_eq!(
            ItemKind::classify(Path::new("thing.lnk")),
            ItemKind::Shortcut
        );
    }
}
