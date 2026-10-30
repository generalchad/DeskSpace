//! `App` inline rename: the edit session, caret, and commit (R1), plus the
//! `EditState` they share (R2).

use std::path::Path;

use deskspace_core::{Command, ItemId, ItemKind, LensTitle, Space, SpaceId, lens_title};
use deskspace_platform::{EditKey, EditOutcome, TextEdit, Ui};

use crate::scene::item_label;

use super::undo::FileChange;
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
    pub(super) text: TextEdit,
}

/// How often to look for a rename Explorer started in its hidden desktop
/// view after a command from the desktop menu, and for how many looks: New ›
/// Folder starts one only once the shell has made the folder and its view
/// has caught up.
const EXPLORER_RENAME_POLL_MS: u32 = 100;
const EXPLORER_RENAME_POLLS: u8 = 20;

/// The inline-edit state: the active rename session, if any, whether the
/// caret is currently shown (toggled by `Timer::Caret`), and how many more
/// times to look for a rename Explorer started (`Timer::ExplorerRename`).
#[derive(Debug, Default)]
pub(super) struct EditState {
    session: Option<EditSession>,
    caret_visible: bool,
    explorer_polls: u8,
}

impl EditState {
    /// Starts looking for a rename Explorer starts.
    fn watch_explorer(&mut self) {
        self.explorer_polls = EXPLORER_RENAME_POLLS;
    }

    /// Uses up one look; returns whether any are left after it.
    fn next_explorer_poll(&mut self) -> bool {
        self.explorer_polls = self.explorer_polls.saturating_sub(1);
        self.explorer_polls > 0
    }

    /// Stops looking.
    fn stop_watching_explorer(&mut self) {
        self.explorer_polls = 0;
    }

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
        let title = space.title().to_owned();
        // The pointer may rest on this title with its tip already shown
        // (W27); the edit takes its place.
        self.dismiss_info_tip(ui);
        self.edit.begin(EditSession {
            surface: Surface::Space(id),
            target: EditTarget::Title,
            text: TextEdit::new(title),
        });
        Self::set_timer(ui, Timer::Caret, CARET_BLINK_MS);
        ui.capture_text_input(true);
        self.refresh_space(ui, id);
    }

    /// Handles F2: renames the focused (last-selected) item, or the current
    /// space's title when no item is selected, as F2 does in Explorer.
    pub(super) fn begin_f2_rename(&mut self, ui: &mut Ui) {
        let Some(surface) = self.selection.surface else {
            return;
        };
        match (surface, self.selection.items.last().copied()) {
            (_, Some(item)) => self.begin_item_rename(ui, surface, item),
            (Surface::Space(space), None) => self.begin_rename(ui, space),
            // The desktop has no title.
            (Surface::Desktop, None) => {}
        }
    }

    pub(super) fn begin_item_rename(&mut self, ui: &mut Ui, surface: Surface, item: ItemId) {
        let show_extensions = self.shows_extensions(surface);
        let Some(entry) = self.config.desktop.item(item) else {
            return;
        };
        let original = item_label(entry, show_extensions);
        let shows_extension = show_extensions && entry.kind == ItemKind::File;
        // Select the base name and leave the extension unselected, as
        // Explorer does when file extensions are shown.
        let base_end = original.rfind('.').filter(|_| shows_extension);
        let mut text = TextEdit::new(original);
        if let Some(dot) = base_end {
            text.select_range(0, dot);
        }
        self.edit.begin(EditSession {
            surface,
            target: EditTarget::Item {
                id: item,
                shows_extension,
            },
            text,
        });
        Self::set_timer(ui, Timer::Caret, CARET_BLINK_MS);
        ui.capture_text_input(true);
        self.refresh_surface(ui, surface);
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

    /// Ends the rename; on commit, `title_commit` decides what the typed
    /// title does.
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
                let default = self.config.window.new_lens.title;
                if commit
                    && let Some(command) = self.config.desktop.space(id).and_then(|space| {
                        title_commit(space, session.text.text(), default, &self.profile)
                    })
                {
                    let effects = self.commit_title(id, command);
                    self.run(ui, effects);
                    self.resnapshot_settings(ui, id);
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

    /// Records a title change as one "Rename space" undo entry.
    pub(super) fn commit_title(
        &mut self,
        id: SpaceId,
        command: Command,
    ) -> Vec<super::effect::Effect> {
        let near = self.space_center(id);
        self.record_effects("Rename space", command, near, Vec::new())
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
        if self.config.desktop.space_containing(id).is_some() {
            if self.rename_space_item(id, shows_extension, typed, |path, new_name| {
                ui.rename_path(path, new_name)
            }) {
                self.refresh_tray(ui);
                self.save(ui);
            }
            return;
        }
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
        // A loose item mirrors the Desktop folder, which now holds the
        // new name; it keeps the selection under its new id.
        self.resync_desktop_layer(ui);
        let renamed = self
            .config
            .desktop
            .layer()
            .items()
            .iter()
            .find(|item| item.path == new_path);
        if let Some(renamed) = renamed.map(|item| item.id) {
            self.selection.select_only(Surface::Desktop, renamed);
        }
    }

    /// Renames space item `id` with `rename_on_disk` and records one
    /// `"Rename item"` undo entry. Returns whether the rename succeeded.
    pub(super) fn rename_space_item(
        &mut self,
        id: ItemId,
        shows_extension: bool,
        typed: &str,
        rename_on_disk: impl FnOnce(&Path, &str) -> bool,
    ) -> bool {
        let typed = typed.trim();
        let Some(space) = self.config.desktop.space_containing(id) else {
            return false;
        };
        let Some(item) = self.config.desktop.item(id) else {
            return false;
        };
        let path = item.path.clone();
        let kind = item.kind;
        let new_name = renamed_file_name(&path, typed, shows_extension || kind == ItemKind::Folder);
        let Some(parent) = path.parent() else {
            return false;
        };
        let new_path = parent.join(&new_name);
        if new_name.is_empty() || new_path == path {
            return false;
        }
        if !rename_on_disk(&path, &new_name) || !new_path.exists() {
            return false;
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
        let near = self.space_center(space);
        self.undo.begin("Rename item");
        self.undo.note_file(FileChange::Renamed {
            from: path,
            to: new_path.clone(),
        });
        // Not `commit`: the file is already renamed on disk, so the caller
        // saves at once.
        self.dispatch(
            Command::RenameItem {
                id,
                path: new_path,
                label,
            },
            near,
        );
        self.undo.finish(&self.config.desktop);
        true
    }

    pub(super) fn cancel_edit(&mut self, ui: &mut Ui) {
        if self.edit.is_active() {
            self.finish_rename(ui, false);
        }
    }

    /// After a shell command from the desktop menu, watches for a rename
    /// Explorer starts in its view (New › Folder, any New entry), which is
    /// invisible while takeover hides its icons. Without takeover Explorer's
    /// own box shows, so nothing is watched.
    pub(super) fn watch_explorer_rename(&mut self, ui: &mut Ui) {
        if !self.desktop_takeover() {
            return;
        }
        self.edit.watch_explorer();
        Self::set_timer(ui, Timer::ExplorerRename, EXPLORER_RENAME_POLL_MS);
    }

    /// `Timer::ExplorerRename`: takes over a rename Explorer started, showing
    /// DeskSpace's own on the same item, or gives up after the last look.
    pub(super) fn poll_explorer_rename(&mut self, ui: &mut Ui) {
        let label = deskspace_platform::take_explorer_rename();
        if label.is_none() && self.edit.next_explorer_poll() {
            return;
        }
        self.edit.stop_watching_explorer();
        Self::clear_timer(ui, Timer::ExplorerRename);
        let Some(label) = label else {
            return;
        };
        // The new item may not be on the layer yet.
        self.resync_desktop_layer(ui);
        let item = self
            .config
            .desktop
            .layer()
            .items()
            .iter()
            .find(|item| {
                item.label == label
                    || item
                        .path
                        .file_name()
                        .is_some_and(|name| name == label.as_str())
            })
            .map(|item| item.id);
        let Some(item) = item else {
            tracing::debug!(%label, "Explorer renamed an item the layer does not show");
            return;
        };
        self.cancel_edit(ui);
        self.selection.select_only(Surface::Desktop, item);
        self.begin_item_rename(ui, Surface::Desktop, item);
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
/// What a committed title edit on `space` does. A typed title renames it,
/// which makes a lens's title custom. An empty one on a custom lens makes the
/// title follow the folder again, as `default` (the Lenses page's "Show as")
/// says, or by the folder's name when that default is itself custom. An empty
/// title anywhere else, or an unchanged one, does nothing, so Enter without
/// editing never changes or clears a name.
fn title_commit(space: &Space, typed: &str, default: LensTitle, profile: &Path) -> Option<Command> {
    let typed = typed.trim();
    if !typed.is_empty() {
        return (typed != space.title()).then(|| Command::RenameSpace {
            id: space.id,
            title: typed.to_owned(),
        });
    }
    let view = space.folder_view()?;
    if view.title != LensTitle::Custom {
        return None;
    }
    let title = match default {
        LensTitle::Custom => LensTitle::FolderName,
        follows => follows,
    };
    let mut next = view.clone();
    next.title = title;
    Some(Command::SetLensTitle {
        id: space.id,
        title,
        text: lens_title(&next, space.title(), profile),
    })
}

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
    use deskspace_core::{Desktop, Rect};

    use super::*;
    use crate::fixtures;

    const PROFILE: &str = r"C:\Users\me";

    /// A desktop with a lens on `Documents\Work` titled "Nickname" by hand.
    fn custom_lens() -> (Desktop, SpaceId) {
        let mut desktop = Desktop::default();
        let rect = Rect::new(0, 0, 300, 200);
        let id = fixtures::create_lens(&mut desktop, "Work", rect, r"C:\Users\me\Documents\Work");
        fixtures::apply(
            &mut desktop,
            Command::RenameSpace {
                id,
                title: "Nickname".to_owned(),
            },
        );
        (desktop, id)
    }

    fn commit(desktop: &Desktop, id: SpaceId, typed: &str, default: LensTitle) -> Option<Command> {
        let space = desktop.space(id).expect("the space exists");
        title_commit(space, typed, default, Path::new(PROFILE))
    }

    #[test]
    fn an_empty_title_makes_a_custom_lens_follow_its_folder_again() {
        let (desktop, id) = custom_lens();
        for typed in ["", "   "] {
            assert_eq!(
                commit(&desktop, id, typed, LensTitle::ProfilePath),
                Some(Command::SetLensTitle {
                    id,
                    title: LensTitle::ProfilePath,
                    text: r"Documents\Work".to_owned(),
                }),
                "typed {typed:?}",
            );
        }
    }

    #[test]
    fn a_custom_default_falls_back_to_the_folder_name() {
        let (desktop, id) = custom_lens();
        assert_eq!(
            commit(&desktop, id, "", LensTitle::Custom),
            Some(Command::SetLensTitle {
                id,
                title: LensTitle::FolderName,
                text: "Work".to_owned(),
            }),
        );
    }

    #[test]
    fn a_typed_title_renames_and_an_unchanged_one_does_nothing() {
        let (desktop, id) = custom_lens();
        assert_eq!(
            commit(&desktop, id, "  Projects ", LensTitle::FolderName),
            Some(Command::RenameSpace {
                id,
                title: "Projects".to_owned(),
            }),
        );
        assert_eq!(
            commit(&desktop, id, "Nickname", LensTitle::FolderName),
            None
        );
    }

    #[test]
    fn an_empty_title_changes_nothing_on_a_space_or_a_following_lens() {
        let mut desktop = Desktop::default();
        let rect = Rect::new(0, 0, 300, 200);
        let space = fixtures::create_space(&mut desktop, "Games", rect);
        let lens = fixtures::create_lens(&mut desktop, "Work", rect, r"C:\Users\me\Work");
        assert_eq!(commit(&desktop, space, "", LensTitle::FolderName), None);
        assert_eq!(commit(&desktop, lens, "", LensTitle::FullPath), None);
    }

    #[test]
    fn the_explorer_rename_watch_gives_up_after_its_last_look() {
        let mut edit = EditState::default();
        assert!(!edit.next_explorer_poll(), "nothing is watched at first");
        edit.watch_explorer();
        let looks = std::iter::from_fn(|| edit.next_explorer_poll().then_some(())).count();
        assert_eq!(looks + 1, usize::from(EXPLORER_RENAME_POLLS));
        edit.watch_explorer();
        edit.stop_watching_explorer();
        assert!(!edit.next_explorer_poll());
    }

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
