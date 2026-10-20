//! Where a space's items come from: the space itself, or a folder it mirrors.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::id::ItemId;
use crate::item::{Item, ItemFacts, ItemKind, SortKey};

/// What fills a space. `Items` is a normal space whose items DeskSpace owns;
/// `Folder` is a lens, a live view of a real folder that DeskSpace never
/// moves, copies, or duplicates (ROADMAP D3, M4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceSource {
    #[default]
    Items,
    Folder(FolderView),
}

impl SpaceSource {
    pub fn is_items(&self) -> bool {
        matches!(self, Self::Items)
    }

    pub fn folder(&self) -> Option<&FolderView> {
        match self {
            Self::Items => None,
            Self::Folder(view) => Some(view),
        }
    }
}

/// A lens's bound folder and how its contents are shown. The view is saved
/// with the space; the items it produces are not (M4 phase 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderView {
    /// The folder the lens mirrors.
    pub path: PathBuf,
    /// The item field the lens is ordered by.
    pub sort: SortKey,
    pub descending: bool,
    /// Keep folders above files, whatever `sort` says.
    pub folders_first: bool,
    pub show_hidden: bool,
    /// Reserved for F-22; stored for forward compatibility but not applied
    /// by `Command::SyncLens` yet.
    pub include: Vec<String>,
    /// Reserved for F-22, as `include`.
    pub exclude: Vec<String>,
}

impl Default for FolderView {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            sort: SortKey::Name,
            descending: false,
            folders_first: false,
            show_hidden: false,
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

/// One entry the platform read from a lens's folder. Core does no IO, so the
/// app passes these to `Command::SyncLens`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderEntry {
    pub path: PathBuf,
    /// The shell's display name. Empty means the file stem.
    pub label: String,
    pub kind: ItemKind,
    /// Size in bytes; `None` for folders.
    pub size: Option<u64>,
    /// Last modification time in seconds since the Unix epoch.
    pub modified: Option<u64>,
    pub hidden: bool,
}

impl FolderEntry {
    /// The item this entry becomes when it is new to the lens.
    pub fn into_item(self, id: ItemId) -> Item {
        let facts = ItemFacts {
            is_folder: self.kind == ItemKind::Folder,
            size: self.size,
            modified: self.modified,
            ..ItemFacts::default()
        };
        let item = Item::new(id, self.path, facts).with_kind(self.kind);
        if self.label.is_empty() {
            item
        } else {
            item.with_label(self.label)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_space_starts_with_its_own_items() {
        assert!(SpaceSource::default().is_items());
        assert!(SpaceSource::default().folder().is_none());
    }

    #[test]
    fn a_folder_lens_reports_its_view() {
        let view = FolderView {
            path: "C:/Users/me/Documents".into(),
            ..FolderView::default()
        };
        let source = SpaceSource::Folder(view.clone());
        assert!(!source.is_items());
        assert_eq!(source.folder(), Some(&view));
    }

    #[test]
    fn an_entry_becomes_an_item_with_its_facts_and_label() {
        let entry = FolderEntry {
            path: "C:/Users/me/Documents/report.docx".into(),
            label: "Report".into(),
            kind: ItemKind::File,
            size: Some(42),
            modified: Some(7),
            hidden: false,
        };
        let item = entry.into_item(ItemId(3));
        assert_eq!(item.id, ItemId(3));
        assert_eq!(item.label, "Report");
        assert_eq!(item.kind, ItemKind::File);
        assert_eq!(item.size, Some(42));
        assert_eq!(item.modified, Some(7));
    }

    #[test]
    fn an_entry_without_a_label_uses_the_file_stem() {
        let entry = FolderEntry {
            path: "C:/Users/me/Documents/report.docx".into(),
            label: String::new(),
            kind: ItemKind::File,
            size: None,
            modified: None,
            hidden: false,
        };
        assert_eq!(entry.into_item(ItemId(1)).label, "report");
    }
}
