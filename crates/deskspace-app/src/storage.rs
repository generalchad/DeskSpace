//! The folder that files dropped into spaces are moved to (ROADMAP D3).
//!
//! Files are moved with a rename, which on the same volume is atomic and
//! copies nothing. Moving to another volume would mean copy-and-delete, so
//! those files are left where they are and the space refers to them instead.
//! Every space shares one folder, so moving an item between spaces never
//! touches the file. Names are never overwritten: a clash gets " (2)", " (3)",
//! and so on, like Explorer.

use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ItemStore {
    root: PathBuf,
}

/// Why a file was left in place instead of being moved.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{0} is on another drive; moving it would copy it")]
    OtherVolume(PathBuf),
    #[error("{0} cannot be moved into the space storage")]
    NotMovable(PathBuf),
    #[error("could not move {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl ItemStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether `path` was moved into the store (and so has to be moved back
    /// out when it leaves its space).
    pub fn contains(&self, path: &Path) -> bool {
        path.parent()
            .is_some_and(|parent| same_path(parent, &self.root))
    }

    /// Whether a lens may watch `folder`. Every space shares the store, and
    /// deleting a space moves its files out of it, so a lens on the store or
    /// inside it would list files that other spaces own. A folder that merely
    /// contains the store is fine.
    pub fn allows_lens(&self, folder: &Path) -> bool {
        let (folder, root) = match (
            std::fs::canonicalize(folder),
            std::fs::canonicalize(&self.root),
        ) {
            (Ok(folder), Ok(root)) => (folder, root),
            _ => (folder.to_path_buf(), self.root.clone()),
        };
        !is_inside(&folder, &root)
    }

    /// Moves `path` into the store and returns its new path. Items already
    /// stored are returned unchanged.
    pub fn adopt(&self, path: &Path) -> Result<PathBuf, StoreError> {
        if self.contains(path) {
            return Ok(path.to_path_buf());
        }
        // Drives, and folders containing the store, cannot move into it.
        if path.parent().is_none() || self.root.starts_with(path) {
            return Err(StoreError::NotMovable(path.to_path_buf()));
        }
        std::fs::create_dir_all(&self.root).map_err(|source| StoreError::Io {
            path: self.root.clone(),
            source,
        })?;
        move_into(path, &self.root)
    }

    /// Moves a stored `path` out to `folder` and returns its new path.
    pub fn release(&self, path: &Path, folder: &Path) -> Result<PathBuf, StoreError> {
        move_into(path, folder)
    }
}

/// Whether two paths name the same file, comparing `path_key`s. Exact equality
/// is the common case and avoids the filesystem.
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    a == b || path_key(a) == path_key(b)
}

/// The key two paths share when they name the same file.
///
/// Windows paths differ by case, by separator, and by 8.3 short names
/// (`TIMOTH~1` vs `TimothyWBrown`), and the shell can report any form, so a
/// path that exists is keyed by its canonical path and one that does not by
/// its text, without case and with `\` separators. Keying many paths once
/// (`App::item_index`) costs one filesystem call each, where comparing every
/// pair would cost two per pair.
pub(crate) fn path_key(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
        .into()
}

/// Whether `path` is `root` or lies below it, comparing whole components
/// case-insensitively as Windows paths do. Unlike a string-prefix test,
/// `...\DeskSpace.App` is not inside `...\DeskSpace`.
pub(crate) fn is_inside(path: &Path, root: &Path) -> bool {
    let mut path = path.components();
    root.components().all(|root| {
        path.next()
            .is_some_and(|part| part.as_os_str().eq_ignore_ascii_case(root.as_os_str()))
    })
}

fn move_into(path: &Path, folder: &Path) -> Result<PathBuf, StoreError> {
    let name = path
        .file_name()
        .ok_or_else(|| StoreError::NotMovable(path.to_path_buf()))?;
    let destination = unique_destination(folder, Path::new(name));
    match std::fs::rename(path, &destination) {
        Ok(()) => Ok(destination),
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            Err(StoreError::OtherVolume(path.to_path_buf()))
        }
        Err(source) => Err(StoreError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// `folder/name`, or `folder/stem (n).ext` with the smallest `n >= 2` that
/// does not exist yet.
pub(crate) fn unique_destination(folder: &Path, name: &Path) -> PathBuf {
    let first = folder.join(name);
    if !first.exists() {
        return first;
    }
    let stem = name.file_stem().unwrap_or_default().to_string_lossy();
    let extension = name
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| folder.join(format!("{stem} ({n}){extension}")))
        .find(|candidate| !candidate.exists())
        .expect("an unused name exists")
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_config::ConfigStore;

    /// A fresh directory under the system temp folder, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "deskspace-storage-test-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn same_path_tolerates_case_for_missing_files() {
        assert!(same_path(Path::new("C:/A/B.txt"), Path::new("c:/a/b.TXT")));
        assert!(!same_path(Path::new("C:/a"), Path::new("C:/b")));
    }

    #[test]
    fn path_key_folds_case_and_separators_for_missing_files() {
        let key = path_key(Path::new("C:/Nowhere/Report.TXT"));
        assert_eq!(key, path_key(Path::new(r"c:\nowhere\report.txt")));
        assert_ne!(key, path_key(Path::new(r"c:\nowhere\other.txt")));
    }

    #[test]
    fn two_spellings_of_an_existing_file_share_a_key() {
        let scratch = Scratch::new("path-key");
        let file = scratch.0.join("Report.txt");
        std::fs::write(&file, "x").unwrap();
        let other = scratch.0.join("report.TXT");
        let other = other.to_string_lossy().replace('\\', "/");
        assert_eq!(path_key(&file), path_key(Path::new(&other)));
    }

    #[test]
    fn contains_matches_the_store_folder() {
        let scratch = Scratch::new("contains");
        let store = ItemStore::new(scratch.0.join("storage"));
        std::fs::create_dir_all(store.root()).unwrap();
        let inside = store.root().join("a.txt");
        std::fs::write(&inside, "x").unwrap();
        assert!(store.contains(&inside));
        let outside = scratch.0.join("b.txt");
        std::fs::write(&outside, "x").unwrap();
        assert!(!store.contains(&outside));
    }

    #[test]
    fn adopt_moves_the_file_and_release_moves_it_back() {
        let scratch = Scratch::new("round-trip");
        let desktop = scratch.0.join("Desktop");
        std::fs::create_dir_all(&desktop).unwrap();
        let original = desktop.join("notes.txt");
        std::fs::write(&original, "hello").unwrap();
        let store = ItemStore::new(scratch.0.join("storage"));

        let stored = store.adopt(&original).unwrap();
        assert!(!original.exists());
        assert!(store.contains(&stored));
        assert_eq!(std::fs::read_to_string(&stored).unwrap(), "hello");
        assert_eq!(store.adopt(&stored).unwrap(), stored);

        let back = store.release(&stored, &desktop).unwrap();
        assert_eq!(back, original);
        assert!(!stored.exists());
    }

    #[test]
    fn unique_path_avoids_existing_files() {
        let scratch = Scratch::new("unique");
        let store = ItemStore::new(scratch.0.join("storage"));
        std::fs::create_dir_all(store.root()).unwrap();
        let unique = |name: &str| unique_destination(store.root(), Path::new(name));
        assert_eq!(unique("App.lnk"), store.root().join("App.lnk"));
        std::fs::write(store.root().join("App.lnk"), "x").unwrap();
        assert_eq!(unique("App.lnk"), store.root().join("App (2).lnk"));
    }

    #[test]
    fn clashing_names_get_a_number_instead_of_overwriting() {
        let scratch = Scratch::new("clash");
        let store = ItemStore::new(scratch.0.join("storage"));
        for folder in ["a", "b"] {
            let dir = scratch.0.join(folder);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("report.txt"), folder).unwrap();
        }
        let first = store.adopt(&scratch.0.join("a/report.txt")).unwrap();
        let second = store.adopt(&scratch.0.join("b/report.txt")).unwrap();
        assert_eq!(first.file_name().unwrap(), "report.txt");
        assert_eq!(second.file_name().unwrap(), "report (2).txt");
        assert_eq!(std::fs::read_to_string(second).unwrap(), "b");
    }

    #[test]
    fn folders_containing_the_store_are_not_moved() {
        let scratch = Scratch::new("ancestor");
        let store = ItemStore::new(scratch.0.join("storage"));
        assert!(matches!(
            store.adopt(&scratch.0),
            Err(StoreError::NotMovable(_))
        ));
        assert!(matches!(
            store.adopt(Path::new("C:\\")),
            Err(StoreError::NotMovable(_))
        ));
    }

    #[test]
    fn a_lens_may_not_watch_the_store_or_anything_inside_it() {
        let scratch = Scratch::new("lens");
        let root = scratch.0.join("storage");
        let store = ItemStore::new(&root);
        for dir in [
            root.join("child/grandchild"),
            scratch.0.join("storage-old"),
            scratch.0.join("elsewhere"),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        assert!(!store.allows_lens(&root));
        assert!(!store.allows_lens(&root.join("child")));
        assert!(!store.allows_lens(&root.join("child/grandchild")));
        let shouted = scratch.0.join("STORAGE");
        assert!(!store.allows_lens(&shouted), "same folder, other case");
        assert!(store.allows_lens(&scratch.0.join("storage-old")));
        assert!(store.allows_lens(&scratch.0.join("elsewhere")));
        assert!(store.allows_lens(&scratch.0), "the root's parent");
    }

    #[test]
    fn a_missing_store_still_refuses_its_own_path() {
        let store = ItemStore::new("C:/nowhere/storage");
        assert!(!store.allows_lens(Path::new("c:/NOWHERE/storage/sub")));
        assert!(store.allows_lens(Path::new("C:/nowhere")));
    }

    #[test]
    fn is_inside_compares_whole_components() {
        assert!(is_inside(Path::new(r"C:\A\B\x.txt"), Path::new(r"C:\a\b")));
        assert!(is_inside(Path::new(r"C:\A"), Path::new(r"C:\a")));
        assert!(!is_inside(
            Path::new(r"C:\DeskSpace.App"),
            Path::new(r"C:\DeskSpace")
        ));
        assert!(!is_inside(
            Path::new(r"C:\DeskSpace"),
            Path::new(r"C:\DeskSpace.App")
        ));
        assert!(!is_inside(Path::new(r"C:\Other"), Path::new(r"C:\A")));
    }

    /// ROADMAP D6: the installer's install root (`%LocalAppData%\DeskSpace.App`)
    /// must never contain the config or the stored files, or uninstalling would
    /// delete the user's data.
    #[test]
    fn user_data_is_outside_the_install_root() {
        let Ok(store) = ConfigStore::discover() else {
            return;
        };
        let Some(local) = std::env::var_os("LOCALAPPDATA") else {
            return;
        };
        let install_root = PathBuf::from(local).join("DeskSpace.App");
        assert!(!is_inside(store.path(), &install_root));
        assert!(!is_inside(store.storage_dir(), &install_root));
    }
}
