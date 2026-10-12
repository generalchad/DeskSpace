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

/// Whether two paths name the same file. Windows paths differ by case and by
/// 8.3 short names (`TIMOTH~1` vs `TimothyWBrown`), and the shell can report
/// either form, so compare canonicalized paths when the file exists and fall
/// back to a case-insensitive string compare otherwise. Exact equality is the
/// common case and avoids the filesystem.
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
    }
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
fn unique_destination(folder: &Path, name: &Path) -> PathBuf {
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

    /// Whether `path` is `root` or lies below it, comparing whole components
    /// case-insensitively as Windows paths do. Unlike a string-prefix test,
    /// `...\DeskSpace.App` is not inside `...\DeskSpace`.
    fn is_inside(path: &Path, root: &Path) -> bool {
        let mut path = path.components();
        root.components().all(|root| {
            path.next()
                .is_some_and(|part| part.as_os_str().eq_ignore_ascii_case(root.as_os_str()))
        })
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
