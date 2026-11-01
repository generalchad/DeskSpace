//! `App` visibility: showing and hiding spaces, the desktop entry, and
//! start-with-Windows (R1).

use deskspace_core::animation::Kind;
use deskspace_platform::{
    DesktopVerb, MenuEntry, Ui, Way, set_desktop_verbs, set_start_with_windows,
};

use super::menus::menu_item;
use super::{App, MenuId};

impl App {
    pub(super) fn set_visible(&mut self, ui: &mut Ui, visible: bool) {
        self.dismiss_info_tip(ui);
        if !visible {
            self.clear_selection(ui);
        }
        self.visible = visible;
        let way = if visible { Way::In } else { Way::Out };
        ui.set_spaces_visible(visible, self.play(Kind::Startup, way));
        self.sync_desktop_verbs();
        tracing::info!(visible, "toggled space visibility");
    }

    /// Writes the desktop background verbs: New space and DeskSpace Settings
    /// always, and Show DeskSpace spaces only while the spaces are hidden, so
    /// each entry is there exactly when it can do something.
    pub(super) fn sync_desktop_verbs(&self) {
        // Tests must not touch the real per-user registry.
        if cfg!(test) {
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
        let verbs = desktop_verbs(&exe, !self.visible);
        if let Err(error) = set_desktop_verbs(&verbs) {
            tracing::warn!(%error, "could not update the desktop menu entries");
        }
    }

    /// Removes every desktop background verb, so a clean exit does not leave
    /// menu items that can only start a fresh instance.
    pub(super) fn remove_desktop_verbs(&self) {
        if cfg!(test) {
            return;
        }
        if let Err(error) = set_desktop_verbs(&[]) {
            tracing::warn!(%error, "could not remove the desktop menu entries");
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

/// The desktop verbs for the executable at `exe`: New space and DeskSpace
/// Settings always, and Show DeskSpace spaces only while `hidden`. Pure, so
/// the set is tested without the registry.
fn desktop_verbs(exe: &str, hidden: bool) -> Vec<DesktopVerb> {
    let icon = format!("\"{exe}\",0");
    let command = |flag: &str| format!("\"{exe}\" {flag}");
    let mut verbs = vec![
        DesktopVerb {
            key: "NewSpace",
            label: "New space",
            command: command("--new-space"),
            icon: icon.clone(),
        },
        DesktopVerb {
            key: "Settings",
            label: "DeskSpace Settings",
            command: command("--settings"),
            icon: icon.clone(),
        },
    ];
    if hidden {
        verbs.push(DesktopVerb {
            key: "ShowSpaces",
            label: "Show DeskSpace spaces",
            command: command("--show"),
            icon,
        });
    }
    verbs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_verbs_offer_new_space_settings_and_show_only_when_hidden() {
        let shown = desktop_verbs(r"C:\Apps\deskspace.exe", false);
        assert_eq!(
            shown.iter().map(|verb| verb.key).collect::<Vec<_>>(),
            ["NewSpace", "Settings"]
        );
        assert_eq!(shown[0].label, "New space");
        assert_eq!(shown[0].command, r#""C:\Apps\deskspace.exe" --new-space"#);
        assert_eq!(shown[0].icon, r#""C:\Apps\deskspace.exe",0"#);
        assert_eq!(shown[1].command, r#""C:\Apps\deskspace.exe" --settings"#);

        let hidden = desktop_verbs(r"C:\Apps\deskspace.exe", true);
        assert_eq!(hidden.last().map(|verb| verb.key), Some("ShowSpaces"));
        assert_eq!(
            hidden.last().unwrap().command,
            r#""C:\Apps\deskspace.exe" --show"#
        );
    }
}
