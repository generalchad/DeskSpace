//! `App` menus: the tray, space, and item context menus.

mod entries;
mod new_space;

use std::path::PathBuf;

use deskspace_core::geometry::Point;
use deskspace_core::source::{FolderView, lens_title};
use deskspace_core::{
    Command, Extensions, ItemId, Palette, SortRule, Space, SpaceId, SpaceLook, SpaceView,
};
use deskspace_platform::{DropAction, ItemMenuChoice, MenuEntry, Ui, work_area_at};
use deskspace_update::InstallMode;

use super::files::ShortcutSource;
use super::{
    App, ICON_SIZE_PRESETS, MenuId, NewShortcut, OPACITY_PRESETS, SpaceColor, Surface,
    delete_needs_confirmation,
};
use entries::{
    column_choice, details_header_menu, group_after, keep_sorted_after, lens_folder_entries,
    lens_view_after, new_entries, new_shortcut_choice, new_shortcut_entries, new_shortcut_menu,
    sort_rule_after, space_appearance_menu, space_group_menu, space_sort_menu, space_view_menu,
    view_after,
};

pub(super) use entries::menu_item;

impl App {
    /// Opens the context menu for the focused item, or for the current space
    /// when no item is selected, as the Menu key or Shift+F10 does.
    pub(super) fn open_context_menu(&mut self, ui: &mut Ui) {
        let Some(surface) = self.selection.surface else {
            return;
        };
        let Some(at) = self.menu_anchor() else {
            return;
        };
        match self.selection.items.last().copied() {
            Some(item) => self.show_item_menu(ui, surface, item, at),
            None => self.show_surface_menu(ui, surface, at),
        }
    }

    /// The menu for an empty spot of `surface`.
    pub(super) fn show_surface_menu(&mut self, ui: &mut Ui, surface: Surface, at: Point) {
        match surface {
            Surface::Space(id) => self.show_space_menu(ui, id, at),
            Surface::Desktop => self.show_desktop_menu(ui, at),
        }
    }

    /// Where to anchor a keyboard-opened menu: the center of the focused item's
    /// cell, or the current surface's center when no item is selected.
    pub(super) fn menu_anchor(&self) -> Option<Point> {
        let surface = self.selection.surface?;
        let Some(item) = self.selection.items.last().copied() else {
            return Some(self.surface_window_rect(surface)?.center());
        };
        let entry = self.surface_item(surface, item)?;
        let cell = self.surface_cell(surface, entry);
        Some(Point::new(
            cell.x + cell.width / 2,
            cell.y + cell.height / 2,
        ))
    }

    /// A context-menu appearance edit. It changes one value of the space's own
    /// look, starting from what the space shows now (so a space that followed
    /// Global changes only that value), and customizes the space. It commits
    /// at once.
    fn edit_space_look(
        &mut self,
        ui: &mut Ui,
        id: SpaceId,
        at: Point,
        edit: impl FnOnce(&mut SpaceLook, &Palette),
    ) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let mut look = SpaceLook::from_global(
            space.theme(&self.config.theme),
            &space.grid(self.config.desktop.layout),
        );
        edit(&mut look, &self.palette_for(id));
        let command = Command::SetSpaceLook {
            id,
            custom: true,
            look: Some(look),
        };
        // Not `commit`: a new look needs a restyle and re-flow, not a repaint.
        if self.dispatch(command, at).is_applied() {
            self.space_look_changed(ui, id);
        }
    }

    fn set_space_tint_opacity(&mut self, ui: &mut Ui, id: SpaceId, percent: u8, at: Point) {
        self.edit_space_look(ui, id, at, |look, _| look.theme.tint_opacity = percent);
    }

    fn set_space_icon_size(&mut self, ui: &mut Ui, id: SpaceId, percent: u8, at: Point) {
        self.edit_space_look(ui, id, at, |look, _| look.theme.icon_size = percent);
    }

    /// Picks a custom tint or text color. Custom colors replace both, so
    /// turning them on keeps the other color as it is shown now.
    fn pick_space_color(&mut self, ui: &mut Ui, id: SpaceId, at: Point, which: SpaceColor) {
        let palette = self.palette_for(id);
        let current = match which {
            SpaceColor::Tint => palette.tint.with_alpha(255),
            SpaceColor::Text => palette.title_text,
        };
        let Some(color) = ui.pick_color(current) else {
            return;
        };
        self.edit_space_look(ui, id, at, |look, palette| {
            let theme = &mut look.theme;
            if !theme.custom_colors {
                theme.custom_colors = true;
                theme.tint = palette.tint.with_alpha(255);
                theme.text = palette.title_text;
            }
            match which {
                SpaceColor::Tint => theme.tint = color.with_alpha(255),
                SpaceColor::Text => theme.text = color.with_alpha(255),
            }
        });
    }

    /// The menu's "Use global appearance": the space follows Global again.
    /// Its look is kept, so customizing it later restores it.
    fn use_global_appearance(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let command = Command::CustomizeSpace {
            id,
            on: false,
            seed: SpaceLook::from_global(&self.config.theme, &self.config.desktop.layout),
        };
        // Not `commit`: a new look needs a restyle and re-flow, not a repaint.
        if self.dispatch(command, at).is_applied() {
            self.space_look_changed(ui, id);
        }
    }

    /// Shows a space's changed look, saves it, and keeps an open settings
    /// window in step: the change is taken into its Cancel snapshot (a menu
    /// edit commits at once, so Cancel must not undo it) and shown.
    fn space_look_changed(&mut self, ui: &mut Ui, id: SpaceId) {
        self.restyle_space(ui, id);
        self.reflow_space(ui, id);
        self.schedule_save(ui);
        self.resnapshot_settings(ui, id);
    }

    /// Applies a Sort by choice (D17). A normal space sorts once and keeps the
    /// rule; a lens keeps the rule and its worker builds its list again, so the
    /// new order applies now rather than at the next change.
    pub(super) fn sort_space(&mut self, ui: &mut Ui, id: SpaceId, rule: SortRule, at: Point) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        if space.is_lens() {
            let mut view = space.view().clone();
            view.sort = rule;
            if self.set_space_view_labelled(ui, id, view, at, "Sort lens") {
                self.sync_watches(ui);
            }
        } else {
            let effects = self.sort_space_items(id, rule, at);
            self.run(ui, effects);
        }
    }

    pub(super) fn sort_space_items(
        &mut self,
        id: SpaceId,
        rule: SortRule,
        at: Point,
    ) -> Vec<super::effect::Effect> {
        self.record_effects(
            "Sort space",
            Command::SortSpace { id, rule },
            at,
            Vec::new(),
        )
    }

    pub(super) fn fit_space_to_contents(
        &mut self,
        id: SpaceId,
        at: Point,
    ) -> Vec<super::effect::Effect> {
        let effects = self.record_effects("Fit to contents", self.fit_command(id), at, Vec::new());
        if !effects.is_empty() {
            self.view_cache.set_scroll(id, 0);
            self.view_cache.set_scroll_x(id, 0);
        }
        effects
    }

    pub(super) fn toggle_space_lock(
        &mut self,
        id: SpaceId,
        at: Point,
    ) -> Vec<super::effect::Effect> {
        let label = match self.config.desktop.space(id) {
            Some(space) if space.locked() => "Unlock space",
            _ => "Lock space",
        };
        self.record_effects(label, Command::ToggleLock { id }, at, Vec::new())
    }

    /// Opens the folder lens `id` shows in Explorer. The menu and the Lens
    /// page share this.
    pub(super) fn open_lens_folder(&mut self, ui: &mut Ui, id: SpaceId) {
        if let Some(folder) = self.lens_folder(id) {
            ui.open_path(&folder);
        }
    }

    /// Asks for a new folder for lens `id` and points the lens at it. A folder
    /// DeskSpace keeps its own files in is refused with a message, as the
    /// startup guard refuses it.
    pub(super) fn change_lens_folder(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let Some((view, current)) = self.config.desktop.space(id).and_then(|space| {
            space
                .folder_view()
                .cloned()
                .map(|view| (view, space.title().to_owned()))
        }) else {
            return;
        };
        let Some(folder) =
            ui.pick_folder(&format!("Choose a folder for \u{201C}{current}\u{201D}"))
        else {
            return;
        };
        if !self.storage.allows_lens(&folder) {
            ui.alert("Change folder", &self.storage_refusal());
            return;
        }
        let same_folder = folder == view.path;
        let browsed = self.is_browsed(id);
        let mut next = view;
        next.path = folder.clone();
        let title = lens_title(&next, &current, &self.profile);
        let (applied, effects) = self.set_lens_folder(id, folder.clone(), title, at);
        self.run(ui, effects);
        if applied {
            self.unavailable_lenses.remove(&id);
            if same_folder && browsed {
                // The items are the subfolder's, so Change folder… to the
                // lens's own folder returns home as browsing up does.
                self.show_subfolder(ui, id, folder, PathBuf::new());
            } else {
                // Change folder… always returns to the lens's own folder.
                self.forget_browse(id);
                self.sync_watches(ui);
            }
            self.resnapshot_settings(ui, id);
        }
    }

    /// The undo-recording half of Change folder…, without a `Ui`.
    pub(super) fn set_lens_folder(
        &mut self,
        id: SpaceId,
        path: PathBuf,
        title: String,
        at: Point,
    ) -> (bool, Vec<super::effect::Effect>) {
        self.record_outcome(
            "Change folder",
            Command::SetLensFolder { id, path, title },
            at,
            Vec::new(),
        )
    }

    /// Replaces lens `id`'s view as one undo entry and has its worker build its
    /// list again, so the hidden-files choice applies now rather than at the
    /// next change.
    fn set_lens_view(&mut self, ui: &mut Ui, id: SpaceId, view: FolderView, at: Point) {
        let (applied, effects) = self.change_lens_view(id, view, at);
        self.run(ui, effects);
        if applied {
            self.sync_watches(ui);
            self.resnapshot_settings(ui, id);
        }
    }

    /// The undo-recording half of a lens view change, without a `Ui`.
    pub(super) fn change_lens_view(
        &mut self,
        id: SpaceId,
        view: FolderView,
        at: Point,
    ) -> (bool, Vec<super::effect::Effect>) {
        self.record_outcome(
            "Change lens view",
            Command::SetLensView { id, view },
            at,
            Vec::new(),
        )
    }

    pub(super) fn show_item_menu(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        item: ItemId,
        at: Point,
    ) {
        let Some(path) = self.config.desktop.item(item).map(|item| item.path.clone()) else {
            return;
        };
        // Removing an item is Explorer's own Delete in the same menu (to the
        // Recycle Bin, as the Delete key does), and taking one back to the
        // desktop is a drag, so DeskSpace adds its inline Rename, which
        // Explorer leaves out of a menu without a view to rename in, and Fit
        // to contents for the space that holds it (D23).
        let mut entries = vec![menu_item(MenuId::RenameItem, "Rename")];
        if let Surface::Space(id) = surface {
            let can_fit = self.config.desktop.space(id).is_some_and(Space::can_fit);
            entries.push(menu_item(MenuId::Fit, "Fit to contents").enabled(can_fit));
        }
        match ui.item_menu(&path, &entries, at) {
            Some(ItemMenuChoice::App(choice)) => match MenuId::from_id(choice) {
                Some(MenuId::RenameItem) => self.begin_item_rename(ui, surface, item),
                Some(MenuId::Fit) => {
                    if let Surface::Space(id) = surface
                        && self.config.desktop.space(id).is_some()
                    {
                        let effects = self.fit_space_to_contents(id, at);
                        self.run(ui, effects);
                    }
                }
                _ => {}
            },
            Some(ItemMenuChoice::Shell) => {
                self.icons.invalidate(&path);
                match surface {
                    Surface::Space(id) => {
                        self.prune_missing(ui, id);
                        self.refresh_space(ui, id);
                    }
                    Surface::Desktop => self.resync_desktop_layer(ui),
                }
            }
            None => {}
        }
    }

    /// The menu for an empty spot of the desktop layer: DeskSpace's own
    /// entries, then Explorer's desktop background menu, which takeover would
    /// otherwise leave unreachable under the layer.
    fn show_desktop_menu(&mut self, ui: &mut Ui, at: Point) {
        let [new_space, new_lens] = new_entries();
        let entries = [
            new_space,
            new_lens,
            menu_item(MenuId::Settings, "DeskSpace settings\u{2026}"),
            self.visibility_entry(),
        ];
        match ui.desktop_menu(&entries, at) {
            Some(ItemMenuChoice::App(choice)) => {
                if let Some(choice) = MenuId::from_id(choice) {
                    self.run_common_menu(ui, choice, at);
                }
            }
            Some(ItemMenuChoice::Shell) => {
                self.resync_desktop_layer(ui);
                self.watch_explorer_rename(ui);
            }
            None => {}
        }
    }

    /// Explorer's menu for a Details header: a check per column, which shows
    /// or hides it. The menu pumps messages, so the space is looked up again
    /// after it.
    pub(super) fn show_details_header_menu(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let entries = details_header_menu(space.view());
        let Some(column) = ui
            .popup_menu(&entries, at)
            .and_then(MenuId::from_id)
            .and_then(column_choice)
        else {
            return;
        };
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let view = space.view().with_column_toggled(column);
        self.set_space_view_labelled(ui, id, view, at, "Choose columns");
    }

    pub(super) fn show_space_menu(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let [undo, redo] = self.undo_entries();
        let fold = if space.folded() { "Unfold" } else { "Fold" };
        let mut entries = vec![
            undo,
            redo,
            MenuEntry::Separator,
            menu_item(MenuId::Fold, fold),
            menu_item(MenuId::Rename, "Rename"),
            menu_item(MenuId::Lock, "Lock position").checked(space.locked()),
        ];
        if space.can_fit() {
            entries.push(menu_item(MenuId::Fit, "Fit to contents"));
        }
        entries.push(MenuEntry::Separator);
        if space.is_lens() {
            entries.extend(lens_folder_entries());
            entries.push(MenuEntry::Separator);
        }
        // Locking fixes a space's position, not its contents, so a locked
        // space still takes new shortcuts.
        let can_paste_shortcut = !ui.clipboard_shell_items().is_empty();
        entries.push(new_shortcut_menu(can_paste_shortcut));
        entries.push(menu_item(MenuId::AddFiles, "Add files\u{2026}"));
        entries.push(space_appearance_menu(
            space,
            space.theme(&self.config.theme),
        ));
        entries.push(space_view_menu(
            space,
            self.shows_extensions(Surface::Space(space.id)),
        ));
        entries.push(space_sort_menu(space.view(), space.folder_view()));
        entries.push(space_group_menu(space.view()));
        entries.push(MenuEntry::Separator);
        // The ellipsis promises a dialog, which only non-empty spaces show.
        let delete = if delete_needs_confirmation(space) {
            "Delete space\u{2026}"
        } else {
            "Delete space"
        };
        entries.extend([menu_item(MenuId::Delete, delete), MenuEntry::Separator]);
        entries.extend(new_entries());
        entries.extend([
            MenuEntry::Separator,
            menu_item(MenuId::Settings, "Settings\u{2026}"),
            self.visibility_entry(),
            menu_item(MenuId::Exit, "Exit DeskSpace"),
        ]);
        let lens_view = space.folder_view().cloned();
        let view = space.view().clone();
        let rule = view.sort;
        match ui.popup_menu(&entries, at).and_then(MenuId::from_id) {
            Some(MenuId::OpenFolder) => self.open_lens_folder(ui, id),
            Some(MenuId::ChangeFolder) => self.change_lens_folder(ui, id, at),
            Some(MenuId::Fold) => self.toggle_fold(ui, id),
            Some(MenuId::Rename) => self.begin_rename(ui, id),
            Some(MenuId::Delete) => self.delete_space(ui, id),
            Some(MenuId::Fit) => {
                let effects = self.fit_space_to_contents(id, at);
                self.run(ui, effects);
            }
            Some(MenuId::Lock) => {
                let effects = self.toggle_space_lock(id, at);
                self.run(ui, effects);
            }
            Some(MenuId::SpaceTint) => self.pick_space_color(ui, id, at, SpaceColor::Tint),
            Some(MenuId::SpaceText) => self.pick_space_color(ui, id, at, SpaceColor::Text),
            Some(MenuId::ResetAppearance) => self.use_global_appearance(ui, id, at),
            Some(MenuId::SpaceSettings) => self.open_settings_with(ui, Some(id)),
            Some(MenuId::AddFiles) => self.add_files(ui, id, at),
            Some(choice) => {
                if choice == MenuId::StatusBar {
                    let next = SpaceView {
                        status_bar: !view.status_bar,
                        ..view
                    };
                    self.set_space_view(ui, id, next, at);
                } else if choice == MenuId::CompactView {
                    let next = SpaceView {
                        compact: !view.compact,
                        ..view
                    };
                    self.set_space_view(ui, id, next, at);
                } else if choice == MenuId::FileExtensions {
                    // The entry checks what the space shows now, so choosing
                    // it flips to the opposite; `Automatic` is the View
                    // page's to choose.
                    let extensions = if self.shows_extensions(Surface::Space(id)) {
                        Extensions::Hide
                    } else {
                        Extensions::Show
                    };
                    let next = SpaceView { extensions, ..view };
                    self.set_space_view(ui, id, next, at);
                } else if let Some(next) = group_after(&view, choice) {
                    self.set_space_view(ui, id, next, at);
                } else if let Some(next) = view_after(&view, choice) {
                    self.set_space_view(ui, id, next, at);
                } else if let Some(view) = lens_view
                    .as_ref()
                    .and_then(|view| lens_view_after(view, choice))
                {
                    // A lens keeps its hidden-files choice in its folder view.
                    self.set_lens_view(ui, id, view, at);
                } else if let Some(next) = sort_rule_after(rule, choice) {
                    self.sort_space(ui, id, next, at);
                } else if let Some(next) = keep_sorted_after(&view, choice, lens_view.is_some()) {
                    self.set_space_view(ui, id, next, at);
                } else if let Some((_, _, percent)) = OPACITY_PRESETS
                    .iter()
                    .find(|(preset, ..)| *preset == choice)
                {
                    self.set_space_tint_opacity(ui, id, *percent, at);
                } else if let Some((_, _, icon_size)) = ICON_SIZE_PRESETS
                    .iter()
                    .find(|(preset, ..)| *preset == choice)
                {
                    self.set_space_icon_size(ui, id, *icon_size, at);
                } else if let Some(kind) = new_shortcut_choice(choice) {
                    self.new_shortcut(ui, id, kind, at);
                } else {
                    self.run_common_menu(ui, choice, at);
                }
            }
            None => {}
        }
    }

    /// Makes new shortcuts in space `id` the way `kind` says: apps checked in
    /// the installed-apps picker, one described in the "Custom shortcut"
    /// dialog, or the items on the clipboard. Cancelling a dialog adds
    /// nothing. The shortcuts are listed at the end of the space; `at` only
    /// picks the monitor whose work area bounds the space.
    pub(super) fn new_shortcut(&mut self, ui: &mut Ui, id: SpaceId, kind: NewShortcut, at: Point) {
        let Some(title) = self
            .config
            .desktop
            .space(id)
            .map(|space| space.title().to_owned())
        else {
            return;
        };
        let sources: Vec<ShortcutSource> = match kind {
            NewShortcut::Apps => ui
                .pick_apps(&format!("Add apps to \u{201C}{title}\u{201D}"))
                .into_iter()
                .map(ShortcutSource::Shell)
                .collect(),
            NewShortcut::Custom => ui
                .custom_shortcut(&format!("New shortcut in \u{201C}{title}\u{201D}"))
                .map(ShortcutSource::Custom)
                .into_iter()
                .collect(),
            NewShortcut::Paste => ui
                .clipboard_shell_items()
                .into_iter()
                .map(ShortcutSource::Shell)
                .collect(),
        };
        if !sources.is_empty() {
            self.add_shortcuts(ui, id, at, sources);
        }
    }

    /// "Add files…" (F-55): picks files in the standard Open dialog and moves
    /// them into space `id`, exactly as dropping them would (ROADMAP D3).
    /// Cancelling adds nothing.
    pub(super) fn add_files(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let Some(title) = self
            .config
            .desktop
            .space(id)
            .map(|space| space.title().to_owned())
        else {
            return;
        };
        let paths = ui.pick_files(&format!("Add files to \u{201C}{title}\u{201D}"), &[]);
        if !paths.is_empty() {
            self.add_dropped(ui, id, at, paths, DropAction::Move);
        }
    }

    /// Shows the "New shortcut" choices at `at` (the settings window's +
    /// tile) and makes the chosen kind of shortcut in space `id`.
    pub(super) fn offer_new_shortcut(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let can_paste = !ui.clipboard_shell_items().is_empty();
        let choice = ui
            .popup_menu(&new_shortcut_entries(can_paste), at)
            .and_then(MenuId::from_id)
            .and_then(new_shortcut_choice);
        if let Some(kind) = choice {
            let near = self.space_center(id);
            self.new_shortcut(ui, id, kind, near);
        }
    }

    pub(super) fn show_tray_menu(&mut self, ui: &mut Ui, at: Point) {
        let [undo, redo] = self.undo_entries();
        let mut entries = vec![
            undo,
            redo,
            MenuEntry::Separator,
            menu_item(MenuId::Settings, "Settings\u{2026}"),
        ];
        if self.debug.is_some() {
            entries.push(menu_item(MenuId::Debug, "Debug\u{2026}"));
        }
        entries.push(self.visibility_entry());
        entries.extend(new_entries());
        entries.extend([
            MenuEntry::Separator,
            self.update_entry(),
            MenuEntry::Separator,
            menu_item(MenuId::Exit, "Exit DeskSpace"),
        ]);
        if let Some(choice) = ui.popup_menu(&entries, at).and_then(MenuId::from_id) {
            match choice {
                MenuId::Debug => return self.open_debug(ui),
                MenuId::CheckUpdates => return self.start_update_check(ui, true),
                MenuId::InstallUpdate => {
                    return self.install_ready_update(ui, InstallMode::Passive);
                }
                _ => {}
            }
            let work = work_area_at(at);
            self.run_common_menu(ui, choice, work.center());
        }
    }

    /// The tray's update entry: install a downloaded update, or check.
    fn update_entry(&self) -> MenuEntry {
        match self.update.ready_version() {
            Some(version) => menu_item(
                MenuId::InstallUpdate,
                format!("Restart to update to {version}"),
            ),
            None => menu_item(MenuId::CheckUpdates, "Check for updates\u{2026}")
                .enabled(deskspace_update::is_configured()),
        }
    }

    pub(super) fn undo_entries(&self) -> [MenuEntry; 2] {
        let undo = match self.undo.undo_label() {
            Some(label) => menu_item(MenuId::Undo, format!("Undo {label}")),
            None => menu_item(MenuId::Undo, "Undo").enabled(false),
        };
        let redo = match self.undo.redo_label() {
            Some(label) => menu_item(MenuId::Redo, format!("Redo {label}")),
            None => menu_item(MenuId::Redo, "Redo").enabled(false),
        };
        [undo, redo]
    }

    pub(super) fn run_common_menu(&mut self, ui: &mut Ui, choice: MenuId, at: Point) {
        match choice {
            MenuId::Undo => self.undo(ui),
            MenuId::Redo => self.redo(ui),
            MenuId::NewSpace => {
                self.create_space(ui, at);
            }
            MenuId::NewLens => {
                self.create_lens(ui, at);
            }
            MenuId::Settings => self.open_settings(ui),
            MenuId::ToggleVisible => self.set_visible(ui, !self.visible),
            MenuId::Exit => self.exit(ui),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::Rect;

    #[test]
    fn keyboard_menu_anchor_follows_the_focus() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id =
            fixtures::create_space(&mut app.config.desktop, "S", Rect::new(100, 100, 240, 200));
        let item = fixtures::add_item(&mut app.config.desktop, id, "C:/missing/a.txt");
        app.selection.surface = Some(Surface::Space(id));

        let (rect, content, position) = {
            let space = app.config.desktop.space(id).unwrap();
            let position = space.item(item).unwrap().position;
            (space.rect(), space.content_rect(), position)
        };
        // The cell the space draws the item in, not the stored base grid.
        let layout = app.layout_for(id);

        app.selection.items.clear();
        assert_eq!(app.menu_anchor(), Some(rect.center()));

        app.selection.items = vec![item];
        assert_eq!(
            app.menu_anchor(),
            Some(Point::new(
                content.x + position.x + layout.cell_width / 2,
                content.y + position.y + layout.cell_height / 2,
            ))
        );
    }

    #[test]
    fn undo_menu_entries_name_the_top_action_and_grey_when_empty() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id =
            fixtures::create_space(&mut app.config.desktop, "S", Rect::new(100, 100, 240, 200));
        assert_eq!(
            app.undo_entries(),
            [
                menu_item(MenuId::Undo, "Undo").enabled(false),
                menu_item(MenuId::Redo, "Redo").enabled(false),
            ]
        );

        app.commit_title(
            id,
            Command::RenameSpace {
                id,
                title: "Work".to_string(),
            },
        );
        assert_eq!(
            app.undo_entries(),
            [
                menu_item(MenuId::Undo, "Undo Rename space"),
                menu_item(MenuId::Redo, "Redo").enabled(false),
            ]
        );

        app.apply_undo();
        assert_eq!(
            app.undo_entries(),
            [
                menu_item(MenuId::Undo, "Undo").enabled(false),
                menu_item(MenuId::Redo, "Redo Rename space"),
            ]
        );
    }
}
