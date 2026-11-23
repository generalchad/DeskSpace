//! Creating shortcuts in spaces and lenses from shell items, files, and custom targets.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use deskspace_core::geometry::Point;
use deskspace_core::{Command, SpaceId};
use deskspace_platform::{
    CustomShortcut, ShellItem, Ui, create_custom_shortcut, create_shortcut, item_facts,
};

use super::super::App;
use super::super::undo::{FileChange, new_shortcuts_label};
use crate::storage::write_new;

impl App {
    /// Creates a shortcut in the store for each source and lists it in
    /// `target`, or, for a lens, writes it into the lens's folder, where its
    /// watch lists it: shell items dropped on a space (a Start-menu or taskbar app,
    /// or an Alt-drag; F-50), apps picked from the Applications folder, and
    /// custom shortcuts (F-53). Explorer writes a `.lnk` when a shell item is
    /// dropped on a folder, and the store is the space's folder, so the
    /// shortcut moves, opens, and deletes like any other item. A dropped
    /// shortcut file is copied instead, as Explorer does, so no shortcut
    /// points at a shortcut; the original is never moved or deleted. Every
    /// way of adding a shortcut ends here.
    pub(in crate::app) fn add_shortcuts(
        &mut self,
        ui: &mut Ui,
        target: SpaceId,
        at: Point,
        sources: Vec<ShortcutSource>,
    ) {
        if !self.create_shortcuts(target, at, sources) {
            return;
        }
        self.refresh_space(ui, target);
        self.refresh_tray(ui);
        self.save(ui);
        self.refresh_settings(ui);
    }

    pub(in crate::app) fn create_shortcuts(
        &mut self,
        target: SpaceId,
        at: Point,
        sources: Vec<ShortcutSource>,
    ) -> bool {
        let lens = self.lens_folder(target);
        let destination = match &lens {
            Some(folder) if !folder.is_dir() || !self.storage.allows_lens(folder) => {
                tracing::warn!(folder = %folder.display(), "lens folder is unavailable; no shortcut written");
                return false;
            }
            Some(folder) => folder.clone(),
            None => {
                let root = self.storage.root();
                if let Err(error) = std::fs::create_dir_all(root) {
                    tracing::warn!(%error, root = %root.display(), "cannot create the space storage folder");
                    return false;
                }
                root.to_path_buf()
            }
        };
        self.undo.begin(new_shortcuts_label(sources.len()));
        let mut count = 0usize;
        for source in sources {
            match write_shortcut(&destination, &source) {
                Ok(path) => {
                    tracing::info!(name = source.name(), path = %path.display(), "created a shortcut");
                    self.icons.invalidate(&path);
                    self.undo
                        .note_file(FileChange::Created { path: path.clone() });
                    if lens.is_some() {
                        continue;
                    }
                    // Not `commit`: one of several commands, refreshed once below.
                    if self
                        .dispatch(
                            Command::AddItem {
                                space: target,
                                facts: item_facts(&path),
                                path,
                            },
                            at,
                        )
                        .is_applied()
                    {
                        count += 1;
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, name = source.name(), "could not create a shortcut");
                }
            }
        }
        if count > 0 {
            self.undo.set_label(new_shortcuts_label(count));
        }
        self.undo.finish(&self.config.desktop);
        true
    }
}

/// Writes one shortcut into `folder` under a clash-free name and returns its
/// path: a copy of a dropped shortcut file, a new `.lnk` to a shell item, or
/// the `.lnk` or `.url` a custom shortcut describes. Each goes through
/// `write_new`, so it never replaces a file already in the folder, which in
/// a lens is the user's own.
fn write_shortcut(folder: &Path, source: &ShortcutSource) -> anyhow::Result<PathBuf> {
    match source {
        ShortcutSource::Shell(item) => {
            if let Some(link) = item.path.as_deref().filter(|path| is_link_file(path)) {
                let name = link_copy_name(link, &item.name);
                return write_new(folder, Path::new(&name), |temp| {
                    std::fs::copy(link, temp)?;
                    Ok(())
                });
            }
            let name = shortcut_file_name(&item.name, "lnk");
            write_new(folder, Path::new(&name), |temp| {
                Ok(create_shortcut(item, temp)?)
            })
        }
        ShortcutSource::Custom(shortcut) => {
            let name = shortcut_file_name(&shortcut.name, shortcut.target.extension());
            write_new(folder, Path::new(&name), |temp| {
                Ok(create_custom_shortcut(shortcut, temp)?)
            })
        }
    }
}

/// Whether `path` is a shortcut file (`.lnk` or `.url`, in any case), which a
/// drop copies rather than links to.
fn is_link_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("lnk") || extension.eq_ignore_ascii_case("url")
        })
}

/// The store name for a copied shortcut file: its own file name, which keeps
/// the `.lnk` or `.url` extension, or a name built from `display` when the
/// path has none.
fn link_copy_name(link: &Path, display: &str) -> String {
    link.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| shortcut_file_name(display, "lnk"))
}

/// A shortcut file name with `extension` (`lnk` or `url`) for a display name
/// (F-50, F-53). Characters that are illegal in a Windows file name become
/// `_`, trailing dots and spaces are trimmed (Windows ignores them), and an
/// empty result falls back to "Shortcut", as Explorer does.
fn shortcut_file_name(display: &str, extension: &str) -> String {
    let mut name: String = display
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    while name.ends_with([' ', '.']) {
        name.pop();
    }
    if name.is_empty() {
        name.push_str("Shortcut");
    }
    name.push('.');
    name.push_str(extension);
    name
}

/// Something a shortcut is made from; see `App::add_shortcuts`.
#[derive(Debug, Clone)]
pub(in crate::app) enum ShortcutSource {
    /// A dropped or picked shell item.
    Shell(ShellItem),
    /// A shortcut described in the "Custom shortcut" dialog.
    Custom(CustomShortcut),
}

impl ShortcutSource {
    /// The name the shortcut is listed under, for logging.
    fn name(&self) -> &str {
        match self {
            Self::Shell(item) => &item.name,
            Self::Custom(shortcut) => &shortcut.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_shortcut_is_copied_into_the_folder_under_a_free_name() {
        let root = std::env::temp_dir().join(format!(
            "deskspace-write-shortcut-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let (from, to) = (root.join("from"), root.join("to"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        let link = from.join("App.lnk");
        std::fs::write(&link, "x").unwrap();
        let source = ShortcutSource::Shell(ShellItem {
            name: "App".to_owned(),
            path: Some(link),
            id_list: Vec::new(),
        });
        let first = write_shortcut(&to, &source).unwrap();
        let second = write_shortcut(&to, &source).unwrap();
        let written = std::fs::read_dir(&to).unwrap().count();
        let copied = std::fs::read_to_string(&second);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(first, to.join("App.lnk"));
        assert_eq!(second, to.join("App (2).lnk"));
        assert_eq!(written, 2, "no temporary file is left beside them");
        assert_eq!(copied.unwrap(), "x");
    }

    #[test]
    fn shortcut_names_make_illegal_characters_safe() {
        assert_eq!(shortcut_file_name("Minesweeper", "lnk"), "Minesweeper.lnk");
        assert_eq!(shortcut_file_name("A/B:C?D", "lnk"), "A_B_C_D.lnk");
    }

    #[test]
    fn shortcut_names_trim_trailing_dots_and_fall_back_when_empty() {
        assert_eq!(shortcut_file_name("Report...", "lnk"), "Report.lnk");
        assert_eq!(shortcut_file_name("   ", "lnk"), "Shortcut.lnk");
        assert_eq!(shortcut_file_name("", "lnk"), "Shortcut.lnk");
    }

    #[test]
    fn a_web_shortcut_name_takes_the_url_extension() {
        assert_eq!(shortcut_file_name("example.com", "url"), "example.com.url");
    }

    #[test]
    fn shortcut_files_are_recognized_in_any_case() {
        assert!(is_link_file(Path::new(r"C:\Start\Notepad.lnk")));
        assert!(is_link_file(Path::new(r"C:\Links\Docs.URL")));
        assert!(!is_link_file(Path::new(r"C:\Desktop\notes.txt")));
        assert!(!is_link_file(Path::new(r"C:\Desktop\lnk")));
    }

    #[test]
    fn a_copied_shortcut_keeps_its_file_name() {
        assert_eq!(
            link_copy_name(Path::new(r"C:\Start\Notepad.lnk"), "Notepad"),
            "Notepad.lnk"
        );
        assert_eq!(link_copy_name(Path::new(""), "Docs"), "Docs.lnk");
    }

    #[test]
    fn creating_or_adding_or_removing_an_item_invalidates_its_cached_icon() {
        let root = std::env::temp_dir().join(format!(
            "deskspace-icon-invalidate-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let source_dir = root.join("src");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_lnk = source_dir.join("App.lnk");
        std::fs::write(&source_lnk, "x").unwrap();

        let store = deskspace_config::ConfigStore::with_path(root.join("deskspace.toml"));
        let mut app = App::new(deskspace_config::Config::new(), store, false, false);
        let space = crate::fixtures::create_space(
            &mut app.config.desktop,
            "S",
            deskspace_core::Rect::new(0, 0, 300, 200),
        );

        let stored_lnk = app.storage.root().join("App.lnk");
        assert!(app.icons.icon_now(&stored_lnk, 32, 80).is_none());
        assert_eq!(app.icons.len(), 1);
        assert!(app.create_shortcuts(
            space,
            Point::ZERO,
            vec![ShortcutSource::Shell(ShellItem {
                name: "App".to_owned(),
                path: Some(source_lnk),
                id_list: Vec::new(),
            })],
        ));
        assert!(app.icons.is_empty());

        let extra = root.join("extra.txt");
        assert!(app.icons.icon_now(&extra, 32, 80).is_none());
        assert_eq!(app.icons.len(), 1);
        assert!(
            app.dispatch(
                Command::AddItem {
                    space,
                    path: extra.clone(),
                    facts: Default::default(),
                },
                Point::ZERO,
            )
            .is_applied()
        );
        assert!(app.icons.is_empty());

        let extra_id = app
            .config
            .desktop
            .space(space)
            .unwrap()
            .items()
            .iter()
            .find(|item| item.path == extra)
            .unwrap()
            .id;
        assert!(app.icons.icon_now(&extra, 32, 80).is_none());
        assert_eq!(app.icons.len(), 1);
        assert!(
            app.dispatch(Command::RemoveItem { id: extra_id }, Point::ZERO)
                .is_applied()
        );
        assert!(app.icons.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }
}
