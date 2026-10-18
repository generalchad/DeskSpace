//! The debug window's Config section: where the configuration and storage
//! live, and the configuration as it would be saved.

use deskspace_config::ConfigStore;
use deskspace_platform::Ui;

use crate::app::App;

impl App {
    /// The Configuration readout: the file and whether it is a scratch one.
    pub(super) fn config_path_text(&self) -> String {
        let real = self.debug.as_ref().is_some_and(|tools| tools.real_config);
        let kind = if real { "real" } else { "scratch" };
        format!("{} ({kind})", self.store.path().display())
    }

    pub(super) fn storage_path_text(&self) -> String {
        self.storage.root().display().to_string()
    }

    /// Writes the configuration to the log, one line per TOML line, exactly
    /// as a save would write it.
    pub(super) fn dump_config(&self) {
        match ConfigStore::serialize(&self.config) {
            Ok(text) => {
                tracing::info!(lines = text.lines().count(), "debug: configuration follows");
                for line in text.lines() {
                    tracing::info!("{line}");
                }
            }
            Err(error) => tracing::warn!(%error, "debug: could not write the configuration"),
        }
    }

    /// Opens the folder holding the configuration file in Explorer.
    pub(super) fn open_config_folder(&mut self, ui: &mut Ui) {
        if let Some(folder) = self.store.path().parent() {
            ui.open_path(folder);
            tracing::info!(folder = %folder.display(), "debug: opened the configuration folder");
        }
    }

    /// Opens the storage folder in Explorer, creating it first if no file
    /// was ever stored, as `add_shortcuts` does.
    pub(super) fn open_storage_folder(&mut self, ui: &mut Ui) {
        let root = self.storage.root().to_path_buf();
        if let Err(error) = std::fs::create_dir_all(&root) {
            tracing::warn!(%error, root = %root.display(), "debug: cannot create the storage folder");
            return;
        }
        ui.open_path(&root);
        tracing::info!(folder = %root.display(), "debug: opened the storage folder");
    }
}
