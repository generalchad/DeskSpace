//! `App` visibility: showing and hiding spaces, the desktop entry, and
//! start-with-Windows (R1).

use deskspace_platform::{MenuEntry, Ui, set_start_with_windows};

use super::menus::menu_item;
use super::{App, MenuId};

impl App {
    pub(super) fn set_visible(&mut self, ui: &mut Ui, visible: bool) {
        self.dismiss_info_tip(ui);
        if !visible {
            self.clear_selection(ui);
        }
        self.visible = visible;
        ui.set_spaces_visible(visible);
        self.sync_desktop_show_entry();
        tracing::info!(visible, "toggled space visibility");
    }

    /// Adds the desktop right-click "Show spaces" entry while the spaces are
    /// hidden and removes it once they are shown, so the entry is there exactly
    /// when it can do something.
    pub(super) fn sync_desktop_show_entry(&self) {
        // Tests must not touch the real per-user registry.
        if cfg!(test) {
            return;
        }
        if self.visible {
            if let Err(error) = deskspace_platform::set_desktop_show_entry(false, "", "", "") {
                tracing::warn!(%error, "could not remove the desktop Show spaces entry");
            }
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let exe = exe.to_string_lossy();
        // Older versions wrote a .ico beside the storage folder for this entry;
        // the executable carries its own icon resource now, so clean it up.
        let leftover = self.storage.root().with_file_name("deskspace.ico");
        let _ = std::fs::remove_file(&leftover);
        // The verb's icon is the executable's first embedded icon.
        let command = format!("\"{exe}\" --show");
        let icon = format!("\"{exe}\",0");
        if let Err(error) = deskspace_platform::set_desktop_show_entry(
            true,
            "Show DeskSpace spaces",
            &command,
            &icon,
        ) {
            tracing::warn!(%error, "could not add the desktop Show spaces entry");
        }
    }

    /// Removes the desktop "Show spaces" entry, so a clean exit does not leave
    /// a menu item that can only start a fresh instance.
    pub(super) fn remove_desktop_show_entry(&self) {
        if cfg!(test) {
            return;
        }
        if let Err(error) = deskspace_platform::set_desktop_show_entry(false, "", "", "") {
            tracing::warn!(%error, "could not remove the desktop Show spaces entry");
        }
    }

    /// Writes or clears the per-user `Run` value that starts DeskSpace at
    /// sign-in. Rewriting it on every start keeps the stored path current.
    pub(super) fn sync_start_with_windows(&self, enabled: bool) {
        // Tests must not touch the real per-user registry.
        if cfg!(test) {
            return;
        }
        if let Err(error) = set_start_with_windows(enabled) {
            tracing::warn!(%error, "could not update the start-with-Windows entry");
        }
    }

    pub(super) fn visibility_entry(&self) -> MenuEntry {
        let label = if self.visible {
            "Hide spaces"
        } else {
            "Show spaces"
        };
        menu_item(MenuId::ToggleVisible, label)
    }
}
