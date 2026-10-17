//! `App` inline rename: the edit session, caret, and commit (R1), plus the
//! `EditState` they share (R2).

use std::path::Path;

use deskspace_core::geometry::Point;
use deskspace_core::{Command, ItemId, ItemKind, SpaceId};
use deskspace_platform::{EditKey, EditOutcome, TextEdit, Ui};

use crate::scene::item_label;

use super::{App, Surface, Timer};

/// Caret blink half-period, matching Windows' default.
const CARET_BLINK_MS: u32 = 530;

/// What an inline edit is renaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditTarget {
    /// The space's title.
    Title,
    /// An item's file name. `shows_extension` is true when the edited label
    /// includes the file extension, so the typed text is the whole name.
    Item { id: ItemId, shows_extension: bool },
}

/// An in-progress inline rename. The platform captures keys while it lives.
#[derive(Debug)]
pub(super) struct EditSession {
    pub(super) surface: Surface,
    pub(super) target: EditTarget,
    pub(super) original: String,
    pub(super) text: TextEdit,
}

/// The inline-edit state: the active rename session, if any, and whether the
/// caret is currently shown (toggled by `Timer::Caret`).
#[derive(Debug, Default)]
pub(super) struct EditState {
    session: Option<EditSession>,
    caret_visible: bool,
}

impl EditState {
    /// Whether a rename is in progress.
    pub(super) fn is_active(&self) -> bool {
        self.session.is_some()
    }

    /// The surface being renamed, when one is.
    pub(super) fn surface(&self) -> Option<Surface> {
        self.session.as_ref().map(|session| session.surface)
    }

    /// The active session, for reading its target and text.
    pub(super) fn session(&self) -> Option<&EditSession> {
        self.session.as_ref()
    }

    /// The active session, for applying a key to its text.
    pub(super) fn session_mut(&mut self) -> Option<&mut EditSession> {
        self.session.as_mut()
    }

    /// Starts a rename with the caret shown.
    pub(super) fn begin(&mut self, session: EditSession) {
        self.session = Some(session);
        self.caret_visible = true;
    }

    /// Ends the rename and hands back the session.
    pub(super) fn take(&mut self) -> Option<EditSession> {
        self.session.take()
    }

    /// Shows the caret, restarting its blink after a keystroke.
    pub(super) fn show_caret(&mut self) {
        self.caret_visible = true;
    }

    /// Flips the caret for a blink.
    pub(super) fn toggle_caret(&mut self) {
        self.caret_visible = !self.caret_visible;
    }

    /// Whether the caret is currently shown.
    pub(super) fn caret_visible(&self) -> bool {
        self.caret_visible
    }
}

impl App {
    /// Starts inline editing of a space's title, capturing keys until commit
    /// or cancel.
    pub(super) fn begin_rename(&mut self, ui: &mut Ui, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let original = space.title().to_owned();
        self.edit.begin(EditSession {
            surface: Surface::Space(id),
            target: EditTarget::Title,
            original: original.clone(),
            text: TextEdit::new(original),
        });
        Self::set_timer(ui, Timer::Caret, CARET_BLINK_MS);
        ui.capture_text_input(true);
        self.refresh_space(ui, id);
    }

    /// Handles F2: renames the focused (last-selected) item, or the current
    /// space's title when no item is selected, as F2 does in Explorer.
    pub(super) fn begin_f2_rename(&mut self, ui: &mut Ui) {
        // Only a space renames: its title, or an item's file. A loose desktop
        // item's file rename would need `Command::RenameItem` to reach the
        // layer.
        let Some(Surface::Space(space)) = self.selection.surface else {
            return;
        };
        match self.selection.items.last().copied() {
            Some(item) => self.begin_item_rename(ui, space, item),
            None => self.begin_rename(ui, space),
        }
    }

    pub(super) fn begin_item_rename(&mut self, ui: &mut Ui, space: SpaceId, item: ItemId) {
        let show_extensions = self.match_explorer(space) && self.shell_display.show_extensions;
        let Some(entry) = self.config.desktop.item(item) else {
            return;
        };
        let original = item_label(entry, show_extensions);
        let shows_extension = show_extensions && entry.kind == ItemKind::File;
        let mut text = TextEdit::new(original.clone());
        if shows_extension {
            // Select the base name and leave the extension unselected, as
            // Explorer does when file extensions are shown.
            if let Some(dot) = original.rfind('.') {
                text.select_range(0, dot);
            }
        }
        self.edit.begin(EditSession {
            surface: Surface::Space(space),
            target: EditTarget::Item {
                id: item,
                shows_extension,
            },
            original,
            text,
        });
        Self::set_timer(ui, Timer::Caret, CARET_BLINK_MS);
        ui.capture_text_input(true);
        self.refresh_space(ui, space);
    }

    pub(super) fn apply_edit_key(&mut self, ui: &mut Ui, key: EditKey) {
        let Some(session) = self.edit.session_mut() else {
            return;
        };
        let space = session.surface;
        let outcome = session.text.apply(key);
        match outcome {
            EditOutcome::Changed => {
                // Typing restarts the blink with the caret shown.
                self.edit.show_caret();
                self.refresh_surface(ui, space);
            }
            EditOutcome::Commit => self.finish_rename(ui, true),
            EditOutcome::Cancel => self.finish_rename(ui, false),
            EditOutcome::Ignored => {}
        }
    }

    /// Ends the rename. An empty or unchanged title is dropped on commit, so
    /// pressing Enter without editing never changes or clears the name.
    pub(super) fn finish_rename(&mut self, ui: &mut Ui, commit: bool) {
        let Some(session) = self.edit.take() else {
            return;
        };
        ui.capture_text_input(false);
        Self::clear_timer(ui, Timer::Caret);
        let surface = session.surface;
        match session.target {
            EditTarget::Title => {
                let Surface::Space(id) = surface else {
                    return;
                };
                if commit {
                    let title = session.text.text().trim().to_string();
                    if !title.is_empty() && title != session.original {
                        let near = self.space_center(id);
                        if self
                            .dispatch(Command::RenameSpace { id, title }, near)
                            .is_applied()
                        {
                            self.schedule_save(ui);
                        }
                    }
                }
            }
            EditTarget::Item {
                id: item,
                shows_extension,
            } => {
                if commit {
                    self.commit_item_rename(ui, item, shows_extension, session.text.text());
                }
            }
        }
        self.refresh_surface(ui, surface);
    }

    /// Renames an item's file to the edited text and updates the model. The
    /// original extension is kept when the label did not show it, as Explorer
    /// does. A cancelled or skipped shell rename leaves the item unchanged.
    pub(super) fn commit_item_rename(
        &mut self,
        ui: &mut Ui,
        id: ItemId,
        shows_extension: bool,
        typed: &str,
    ) {
        let typed = typed.trim();
        let Some(item) = self.config.desktop.item(id) else {
            return;
        };
        let path = item.path.clone();
        let kind = item.kind;
        let new_name = renamed_file_name(&path, typed, shows_extension || kind == ItemKind::Folder);
        let Some(parent) = path.parent() else {
            return;
        };
        let new_path = parent.join(&new_name);
        if new_name.is_empty() || new_path == path {
            return;
        }
        if !ui.rename_path(&path, &new_name) || !new_path.exists() {
            return;
        }
        self.icons.invalidate(&path);
        let label = if kind == ItemKind::Folder {
            new_name.clone()
        } else {
            Path::new(&new_name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(new_name.as_str())
                .to_string()
        };
        let near = self
            .config
            .desktop
            .space_containing(id)
            .map_or(Point::ZERO, |space| self.space_center(space));
        self.dispatch(
            Command::RenameItem {
                id,
                path: new_path,
                label,
            },
            near,
        );
        self.save();
    }

    pub(super) fn cancel_edit(&mut self, ui: &mut Ui) {
        if self.edit.is_active() {
            self.finish_rename(ui, false);
        }
    }

    /// Toggles the edit caret so it blinks, as Windows does.
    pub(super) fn blink_caret(&mut self, ui: &mut Ui) {
        let Some(surface) = self.edit.surface() else {
            Self::clear_timer(ui, Timer::Caret);
            return;
        };
        self.edit.toggle_caret();
        self.refresh_surface(ui, surface);
    }
}

/// The file name to rename to, preserving the original extension when the
/// edited label did not show it, as Explorer does.
fn renamed_file_name(original: &Path, typed: &str, shows_extension: bool) -> String {
    if shows_extension {
        return typed.to_string();
    }
    let Some(extension) = original.extension().and_then(|ext| ext.to_str()) else {
        return typed.to_string();
    };
    let suffix = format!(".{extension}");
    if typed.to_lowercase().ends_with(&suffix.to_lowercase()) {
        typed.to_string()
    } else {
        format!("{typed}{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_preserves_hidden_extensions() {
        let file = Path::new("C:/x/report.txt");
        assert_eq!(renamed_file_name(file, "notes", false), "notes.txt");
        // Typing the extension does not double it.
        assert_eq!(renamed_file_name(file, "notes.TXT", false), "notes.TXT");
        // When the extension is shown (or it is a folder), the name is as typed.
        assert_eq!(renamed_file_name(file, "notes.md", true), "notes.md");
        assert_eq!(
            renamed_file_name(Path::new("C:/x/my.folder"), "other", true),
            "other"
        );
        // A name without an extension is used as typed.
        assert_eq!(
            renamed_file_name(Path::new("C:/x/README"), "NOTES", false),
            "NOTES"
        );
    }
}
