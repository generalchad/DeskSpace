//! Where a space's items come from: the space itself, or a folder it mirrors.

use std::path::{Path, PathBuf};

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

/// How a lens derives its `Space.title` from its folder (ROADMAP D15). The
/// default is `Custom`, so a lens saved before this setting keeps its title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensTitle {
    /// The folder's last part; a drive or share root keeps its whole path.
    FolderName,
    /// The path below the user's profile folder, or the profile's own name.
    ProfilePath,
    /// The folder's full path, with `\` separators.
    FullPath,
    /// The text the user typed.
    #[default]
    Custom,
}

/// A lens's bound folder and how its contents are shown. The view is saved
/// with the space; the items it produces are not (M4 phase 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderView {
    /// The folder the lens mirrors.
    pub path: PathBuf,
    /// How the space's title follows the folder.
    #[serde(default)]
    pub title: LensTitle,
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
            title: LensTitle::Custom,
            sort: SortKey::Name,
            descending: false,
            folders_first: false,
            show_hidden: false,
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

/// The title a lens shows for `view`, given its current title text and the
/// user's profile folder.
pub fn lens_title(view: &FolderView, current: &str, profile: &Path) -> String {
    match view.title {
        LensTitle::FolderName => folder_name(&view.path),
        LensTitle::ProfilePath => profile_path(&view.path, profile),
        LensTitle::FullPath => full_path(&view.path),
        LensTitle::Custom => current.to_owned(),
    }
}

/// The last part of `path`, or the whole path for a drive or share root.
fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| full_path(path))
}

/// The path below `profile`, the profile's own name for the profile itself,
/// or the whole path when `path` is outside it. Comparison is without case,
/// and `/` and `\` are alike.
fn profile_path(path: &Path, profile: &Path) -> String {
    let full = full_path(path);
    let profile_full = full_path(profile);
    let path_parts: Vec<&str> = full.trim_end_matches('\\').split('\\').collect();
    let profile_parts: Vec<&str> = profile_full.trim_end_matches('\\').split('\\').collect();
    if path_parts.len() < profile_parts.len()
        || !path_parts[..profile_parts.len()]
            .iter()
            .zip(&profile_parts)
            .all(|(a, b)| a.to_lowercase() == b.to_lowercase())
    {
        return full;
    }
    if path_parts.len() == profile_parts.len() {
        return folder_name(profile);
    }
    path_parts[profile_parts.len()..].join("\\")
}

/// `path` with `\` separators.
fn full_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
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

    fn view(path: &str, title: LensTitle) -> FolderView {
        FolderView {
            path: path.into(),
            title,
            ..FolderView::default()
        }
    }

    #[test]
    fn a_lens_title_follows_the_folder_name() {
        let profile = Path::new("C:/Users/me");
        assert_eq!(
            lens_title(
                &view("C:/Users/me/Downloads", LensTitle::FolderName),
                "",
                profile
            ),
            "Downloads"
        );
        assert_eq!(
            lens_title(&view(r"D:\", LensTitle::FolderName), "", profile),
            r"D:\"
        );
        assert_eq!(
            lens_title(&view(r"\\server\share", LensTitle::FolderName), "", profile),
            r"\\server\share"
        );
    }

    #[test]
    fn a_lens_title_can_follow_the_path_below_the_profile() {
        let profile = Path::new(r"C:\Users\me");
        assert_eq!(
            lens_title(
                &view(r"C:\Users\me\Documents\Work", LensTitle::ProfilePath),
                "",
                profile
            ),
            r"Documents\Work"
        );
        assert_eq!(
            lens_title(&view(r"C:\Users\me", LensTitle::ProfilePath), "", profile),
            "me"
        );
        assert_eq!(
            lens_title(&view(r"D:\Stuff", LensTitle::ProfilePath), "", profile),
            r"D:\Stuff"
        );
        assert_eq!(
            lens_title(
                &view("c:/users/ME/Documents/Work", LensTitle::ProfilePath),
                "",
                profile
            ),
            r"Documents\Work"
        );
    }

    #[test]
    fn a_lens_title_can_be_the_full_path_or_custom_text() {
        let profile = Path::new("C:/Users/me");
        assert_eq!(
            lens_title(
                &view("C:/Users/me/Documents", LensTitle::FullPath),
                "",
                profile
            ),
            r"C:\Users\me\Documents"
        );
        assert_eq!(
            lens_title(
                &view("C:/Users/me/Documents", LensTitle::Custom),
                "Mine",
                profile
            ),
            "Mine"
        );
    }

    #[test]
    fn a_lens_without_a_title_setting_loads_as_custom() {
        let view: FolderView = toml::from_str("path = 'C:/Temp'\n").unwrap();
        assert_eq!(view.title, LensTitle::Custom);
    }
}
