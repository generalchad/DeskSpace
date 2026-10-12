//! `App` inline rename: the edit session, caret, and commit (R1).

use std::path::Path;

use deskspace_core::geometry::Point;
use deskspace_core::{Command, ItemId, ItemKind, SpaceId};
use deskspace_platform::{EditKey, EditOutcome, TextEdit, Ui};

use crate::scene::item_label;

use super::{App, CARET_BLINK_MS, CARET_TIMER, EditSession, EditTarget, Surface};

impl App {
    /// Starts inline editing of a space's title, capturing keys until commit
    /// or cancel.
    pub(super) fn begin_rename(&mut self, ui: &mut Ui, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let original = space.title.clone();
        self.edit = Some(EditSession {
            surface: Surface::Space(id),
            target: EditTarget::Title,
            original: original.clone(),
            text: TextEdit::new(original),
        });
        self.caret_visible = true;
        ui.set_timer(CARET_TIMER, CARET_BLINK_MS);
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
        self.edit = Some(EditSession {
            surface: Surface::Space(space),
            target: EditTarget::Item {
                id: item,
                shows_extension,
            },
            original,
            text,
        });
        self.caret_visible = true;
        ui.set_timer(CARET_TIMER, CARET_BLINK_MS);
        ui.capture_text_input(true);
        self.refresh_space(ui, space);
    }

    pub(super) fn apply_edit_key(&mut self, ui: &mut Ui, key: EditKey) {
        let Some(session) = self.edit.as_mut() else {
            return;
        };
        let space = session.surface;
        let outcome = session.text.apply(key);
        match outcome {
            EditOutcome::Changed => {
                // Typing restarts the blink with the caret shown.
                self.caret_visible = true;
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
        ui.clear_timer(CARET_TIMER);
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
                        if self.dispatch(Command::RenameSpace { id, title }, near) {
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
        if self.edit.is_some() {
            self.finish_rename(ui, false);
        }
    }

    /// Toggles the edit caret so it blinks, as Windows does.
    pub(super) fn blink_caret(&mut self, ui: &mut Ui) {
        let Some(surface) = self.edit.as_ref().map(|session| session.surface) else {
            ui.clear_timer(CARET_TIMER);
            return;
        };
        self.caret_visible = !self.caret_visible;
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
