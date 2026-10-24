//! `App` menus: the tray, space, and item context menus (R1).

use deskspace_config::NEW_SPACE_SIZE_RANGE;
use deskspace_core::geometry::{Point, Rect, Size};
use deskspace_core::placement::{DEFAULT_SPACE_GAP, drop_space_origin, new_space_origin};
use deskspace_core::source::{FolderView, LensTitle, lens_title};
use deskspace_core::{
    Command, ItemId, Palette, SortRule, Space, SpaceId, SpaceLook, SpaceView, Theme,
};
use deskspace_platform::{
    DropAction, ItemMenuChoice, MenuEntry, Ui, WatchKey, monitor_dpi_at, scale_for, work_area_at,
};
use deskspace_update::InstallMode;

use super::files::ShortcutSource;
use super::{
    App, ICON_SIZE_PRESETS, MenuId, NEW_SHORTCUT_ENTRIES, NewShortcut, OPACITY_PRESETS,
    SORT_MENU_IDS, SpaceColor, Surface, VIEW_PRESETS, delete_needs_confirmation,
};
use crate::settings::{SORT_KEYS, VIEW_MODES};

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

    /// Applies a Sort by choice (D15). A normal space sorts once and keeps the
    /// rule; a lens keeps the rule and lists its folder again, so the new
    /// order applies now rather than at the next change.
    pub(super) fn sort_space(&mut self, ui: &mut Ui, id: SpaceId, rule: SortRule, at: Point) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        if space.is_lens() {
            let mut view = space.view();
            view.sort = rule;
            self.set_space_view(ui, id, view, at);
            self.restart_lens_watch(ui, id);
        } else {
            self.commit(ui, Command::SortSpace { id, rule }, at);
        }
    }

    /// Opens lens `id`'s folder in Explorer. The menu and the Lens page share
    /// this.
    pub(super) fn open_lens_folder(&mut self, ui: &mut Ui, id: SpaceId) {
        if let Some(view) = self.config.desktop.space(id).and_then(Space::folder_view) {
            ui.open_path(&view.path);
        }
    }

    /// Asks for a new folder for lens `id` and points the lens at it. A folder
    /// DeskSpace keeps its own files in is refused with a message, as W3a's
    /// guard refuses it at startup.
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
        let mut next = view;
        next.path = folder.clone();
        let title = lens_title(&next, &current, &self.profile);
        if self
            .commit(
                ui,
                Command::SetLensFolder {
                    id,
                    path: folder,
                    title,
                },
                at,
            )
            .is_applied()
        {
            self.unavailable_lenses.remove(&id);
            self.sync_watches(ui);
            self.resnapshot_settings(ui, id);
        }
    }

    /// Replaces lens `id`'s view and lists its folder again, so the new order
    /// and the hidden-files choice apply now rather than at the next change.
    fn set_lens_view(&mut self, ui: &mut Ui, id: SpaceId, view: FolderView, at: Point) {
        if self
            .commit(ui, Command::SetLensView { id, view }, at)
            .is_applied()
        {
            self.restart_lens_watch(ui, id);
            self.resnapshot_settings(ui, id);
        }
    }

    /// Lists a lens's folder again by starting its watch over: a watch lists
    /// its folder when it starts, so this is a fresh listing through the same
    /// path as every other.
    fn restart_lens_watch(&mut self, ui: &mut Ui, id: SpaceId) {
        ui.unwatch_folder(WatchKey::Space(id));
        self.sync_watches(ui);
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
        // desktop is a drag, so DeskSpace adds only its inline Rename. A loose
        // desktop item gets Explorer's menu alone.
        let entries = match surface {
            Surface::Space(_) => vec![menu_item(MenuId::RenameItem, "Rename")],
            Surface::Desktop => Vec::new(),
        };
        match ui.item_menu(&path, &entries, at) {
            Some(ItemMenuChoice::App(choice))
                if MenuId::from_id(choice) == Some(MenuId::RenameItem) =>
            {
                if let Surface::Space(id) = surface {
                    self.begin_item_rename(ui, id, item);
                }
            }
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
            _ => {}
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
            Some(ItemMenuChoice::Shell) => self.resync_desktop_layer(ui),
            None => {}
        }
    }

    pub(super) fn show_space_menu(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let fold = if space.folded() { "Unfold" } else { "Fold" };
        let mut entries = vec![
            menu_item(MenuId::Fold, fold),
            menu_item(MenuId::Rename, "Rename"),
            menu_item(MenuId::Lock, "Lock position").checked(space.locked()),
        ];
        if space.user_sized() && !space.locked() {
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
        entries.push(space_view_menu(space));
        entries.push(space_sort_menu(space.view(), space.folder_view()));
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
        let view = space.view();
        let rule = view.sort;
        match ui.popup_menu(&entries, at).and_then(MenuId::from_id) {
            Some(MenuId::OpenFolder) => self.open_lens_folder(ui, id),
            Some(MenuId::ChangeFolder) => self.change_lens_folder(ui, id, at),
            Some(MenuId::Fold) => self.toggle_fold(ui, id),
            Some(MenuId::Rename) => self.begin_rename(ui, id),
            Some(MenuId::Delete) => self.delete_space(ui, id),
            Some(MenuId::Fit) => {
                if self
                    .commit(ui, Command::FitToContents { id }, at)
                    .is_applied()
                {
                    self.view_cache.set_scroll(id, 0);
                }
            }
            Some(MenuId::Lock) => {
                self.commit(ui, Command::ToggleLock { id }, at);
            }
            Some(MenuId::SpaceTint) => self.pick_space_color(ui, id, at, SpaceColor::Tint),
            Some(MenuId::SpaceText) => self.pick_space_color(ui, id, at, SpaceColor::Text),
            Some(MenuId::ResetAppearance) => self.use_global_appearance(ui, id, at),
            Some(MenuId::SpaceSettings) => self.open_settings_with(ui, Some(id)),
            Some(MenuId::AddFiles) => self.add_files(ui, id, at),
            Some(choice) => {
                // A view choice changes only this space's layout.
                if let Some((_, mode)) = VIEW_PRESETS.iter().find(|(preset, _)| *preset == choice) {
                    self.set_space_view(ui, id, SpaceView::with_mode(*mode), at);
                } else if let Some(view) = lens_view
                    .as_ref()
                    .and_then(|view| lens_view_after(view, choice))
                {
                    // A lens keeps its hidden-files choice in its folder view.
                    self.set_lens_view(ui, id, view, at);
                } else if let Some(next) = sort_rule_after(rule, choice) {
                    self.sort_space(ui, id, next, at);
                } else if let Some(next) = keep_sorted_after(view, choice, lens_view.is_some()) {
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
        let paths = ui.pick_files(&format!("Add files to \u{201C}{title}\u{201D}"));
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
        let mut entries = vec![menu_item(MenuId::Settings, "Settings\u{2026}")];
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

    pub(super) fn run_common_menu(&mut self, ui: &mut Ui, choice: MenuId, at: Point) {
        match choice {
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

    /// New space (menus, the hotkey, and the debug window's test spaces): in
    /// line with the top row of spaces on the monitor at `at` (ROADMAP D10).
    /// Returns its id.
    pub(super) fn create_space(&mut self, ui: &mut Ui, at: Point) -> Option<SpaceId> {
        let (origin, size) = self.new_space_spot(at);
        self.open_new_space(ui, origin, size)
    }

    /// New lens…: asks for a folder, then opens a lens on it where New space
    /// would put a space, titled with the folder's name. DeskSpace's storage
    /// folder is refused with a message (W3a). Returns its id.
    pub(super) fn create_lens(&mut self, ui: &mut Ui, at: Point) -> Option<SpaceId> {
        let folder = ui.pick_folder("Choose a folder for the new lens")?;
        if !self.storage.allows_lens(&folder) {
            ui.alert("New lens", &self.storage_refusal());
            return None;
        }
        let (origin, size) = self.new_space_spot(at);
        let view = FolderView {
            path: folder.clone(),
            title: LensTitle::FolderName,
            ..FolderView::default()
        };
        let title = lens_title(&view, "", &self.profile);
        let command = Command::CreateLens {
            title,
            at: origin,
            size,
            path: folder,
            locked: self.config.window.new_space_locked,
        };
        let id = self.show_new_space(ui, command, origin)?;
        self.sync_watches(ui);
        Some(id)
    }

    /// Why a lens cannot show DeskSpace's storage folder.
    fn storage_refusal(&self) -> String {
        format!(
            "DeskSpace keeps its own files in \u{201C}{}\u{201D}, so a lens cannot show that \
             folder or one inside it.",
            self.storage.root().display()
        )
    }

    /// Where New space puts a new space for the monitor at `at`, and its
    /// size: in line with the top row of spaces there (ROADMAP D10).
    fn new_space_spot(&self, at: Point) -> (Point, Size) {
        let size = self.new_space_size();
        let spaces = self.space_rects();
        let origin = new_space_origin(&spaces, size, work_area_at(at), gap_at(at)).unwrap_or(at);
        (origin, size)
    }

    /// A space for items dropped on the desktop layer (ROADMAP D7): as near
    /// the drop point `at` as it fits with a gutter around it, else where
    /// New space would put it. Returns its id.
    pub(super) fn create_space_for_drop(&mut self, ui: &mut Ui, at: Point) -> Option<SpaceId> {
        let size = self.new_space_size();
        let spaces = self.space_rects();
        let (bounds, gap) = (work_area_at(at), gap_at(at));
        let origin = drop_space_origin(at, &spaces, size, bounds, gap)
            .or_else(|| new_space_origin(&spaces, size, bounds, gap))
            .unwrap_or(at);
        self.open_new_space(ui, origin, size)
    }

    /// Creates a space at `origin` with the configured title and lock state,
    /// shows it, and returns its id.
    fn open_new_space(&mut self, ui: &mut Ui, origin: Point, size: Size) -> Option<SpaceId> {
        let window = &self.config.window;
        let title = window.new_space_title.trim();
        let title = if title.is_empty() {
            "New space".to_string()
        } else {
            title.to_string()
        };
        let command = Command::CreateSpace {
            title,
            at: origin,
            size,
            locked: window.new_space_locked,
        };
        self.show_new_space(ui, command, origin)
    }

    /// Applies `command`, which creates a space or a lens at `origin`, gives it
    /// a window, and returns its id. Spaces are shown first if hidden.
    fn show_new_space(&mut self, ui: &mut Ui, command: Command, origin: Point) -> Option<SpaceId> {
        let id = self.config.desktop.next_space_id();
        // Not `commit`: a new space needs a window (`sync_windows`), not a
        // repaint.
        if !self.dispatch(command, origin).is_applied() {
            return None;
        }
        if !self.visible {
            self.set_visible(ui, true);
        }
        self.sync_windows(ui);
        self.schedule_save(ui);
        Some(id)
    }

    /// The configured size of a new space, clamped to its range.
    fn new_space_size(&self) -> Size {
        let window = &self.config.window;
        Size::new(
            window
                .new_space_width
                .clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1),
            window
                .new_space_height
                .clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1),
        )
    }

    fn space_rects(&self) -> Vec<Rect> {
        self.config
            .desktop
            .spaces()
            .iter()
            .map(|space| space.rect())
            .collect()
    }
}

/// A menu entry for one of the app's menu ids.
pub(super) fn menu_item(id: MenuId, label: impl Into<String>) -> MenuEntry {
    MenuEntry::item(id as u32, label)
}

/// The default gutter around a new space, scaled for the monitor at `at`.
fn gap_at(at: Point) -> i32 {
    let scale = scale_for(monitor_dpi_at(at));
    (DEFAULT_SPACE_GAP as f32 * scale).round() as i32
}

/// The "Appearance" submenu for a space drawn with `theme`: tint-opacity
/// presets, the icon size, custom tint and text colors, a way back to the
/// global appearance, and the space's settings page. Every edit customizes
/// the space.
fn space_appearance_menu(space: &Space, theme: &Theme) -> MenuEntry {
    let mut entries: Vec<MenuEntry> = OPACITY_PRESETS
        .iter()
        .map(|(id, label, value)| menu_item(*id, *label).checked(theme.tint_opacity == *value))
        .collect();
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::submenu(
        "Icon size",
        ICON_SIZE_PRESETS
            .iter()
            .map(|(id, label, value)| menu_item(*id, *label).checked(theme.icon_size == *value))
            .collect(),
    ));
    entries.push(MenuEntry::Separator);
    entries.push(menu_item(MenuId::SpaceTint, "Custom tint\u{2026}").checked(theme.custom_colors));
    entries.push(menu_item(MenuId::SpaceText, "Custom text\u{2026}").checked(theme.custom_colors));
    entries
        .push(menu_item(MenuId::ResetAppearance, "Use global appearance").enabled(space.custom()));
    entries.push(MenuEntry::Separator);
    entries.push(menu_item(MenuId::SpaceSettings, "Space settings\u{2026}"));
    MenuEntry::submenu("Appearance", entries)
}

/// The "New shortcut" submenu (F-53, F-54), from `NEW_SHORTCUT_ENTRIES`.
/// `can_paste` enables "Paste shortcut" when the clipboard holds shell items.
fn new_shortcut_menu(can_paste: bool) -> MenuEntry {
    MenuEntry::submenu("New shortcut", new_shortcut_entries(can_paste))
}

/// The New shortcut choices themselves, for the submenu and the settings
/// window's + tile.
fn new_shortcut_entries(can_paste: bool) -> Vec<MenuEntry> {
    let mut entries = Vec::with_capacity(NEW_SHORTCUT_ENTRIES.len() + 1);
    for (id, label, kind) in NEW_SHORTCUT_ENTRIES {
        if kind == NewShortcut::Paste {
            entries.push(MenuEntry::Separator);
        }
        let enabled = kind != NewShortcut::Paste || can_paste;
        entries.push(menu_item(id, label).enabled(enabled));
    }
    entries
}

/// What a "New shortcut" menu id makes, if it is one.
fn new_shortcut_choice(choice: MenuId) -> Option<NewShortcut> {
    NEW_SHORTCUT_ENTRIES
        .iter()
        .find(|(id, ..)| *id == choice)
        .map(|(.., kind)| *kind)
}

/// The "View" submenu: one radio item per icon view, checking the space's
/// current mode.
fn space_view_menu(space: &Space) -> MenuEntry {
    let entries = VIEW_PRESETS
        .iter()
        .map(|(id, mode)| {
            let label = VIEW_MODES
                .iter()
                .find(|(row, ..)| row == mode)
                .map_or("", |(_, label, _)| *label);
            menu_item(*id, label).checked(space.view().mode == *mode)
        })
        .collect();
    MenuEntry::submenu("View", entries)
}

/// The "Sort by" submenu (D15): Explorer's four keys as radio items, the
/// direction, then Folders first and Keep sorted. A lens also gets its
/// hidden-files toggle, which DeskSpace has no other menu for.
fn space_sort_menu(view: SpaceView, lens: Option<&FolderView>) -> MenuEntry {
    let rule = view.sort;
    let mut entries: Vec<MenuEntry> = SORT_MENU_IDS
        .iter()
        .map(|(id, key)| {
            let label = SORT_KEYS
                .iter()
                .find(|(row, _)| row == key)
                .map_or("", |(_, label)| *label);
            menu_item(*id, label).checked(rule.key == *key)
        })
        .collect();
    entries.extend([
        MenuEntry::Separator,
        menu_item(MenuId::SortAscending, "Ascending").checked(!rule.descending),
        menu_item(MenuId::SortDescending, "Descending").checked(rule.descending),
        MenuEntry::Separator,
        menu_item(MenuId::FoldersFirst, "Folders first").checked(rule.folders_first),
        menu_item(MenuId::KeepSorted, "Keep sorted")
            .checked(lens.is_some() || view.keep_sorted)
            .enabled(lens.is_none()),
    ]);
    if let Some(view) = lens {
        entries.push(menu_item(MenuId::ShowHidden, "Show hidden files").checked(view.show_hidden));
    }
    MenuEntry::submenu("Sort by", entries)
}

/// The view a Keep sorted choice leaves, or `None` when it is not that
/// choice. A lens is always kept sorted, so its entry is greyed and changes
/// nothing (D15).
fn keep_sorted_after(view: SpaceView, choice: MenuId, lens: bool) -> Option<SpaceView> {
    if choice != MenuId::KeepSorted || lens {
        return None;
    }
    Some(SpaceView {
        keep_sorted: !view.keep_sorted,
        ..view
    })
}

/// The rule a Sort by choice leaves, or `None` when `choice` is not a Sort by
/// entry. A key keeps the direction and folders-first, a direction keeps the
/// key, and Folders first toggles.
fn sort_rule_after(rule: SortRule, choice: MenuId) -> Option<SortRule> {
    if let Some((_, key)) = SORT_MENU_IDS.iter().find(|(id, _)| *id == choice) {
        return Some(SortRule { key: *key, ..rule });
    }
    match choice {
        MenuId::SortAscending => Some(SortRule {
            descending: false,
            ..rule
        }),
        MenuId::SortDescending => Some(SortRule {
            descending: true,
            ..rule
        }),
        MenuId::FoldersFirst => Some(SortRule {
            folders_first: !rule.folders_first,
            ..rule
        }),
        _ => None,
    }
}

/// "New space" and "New lens…", in that order, for every menu that creates
/// spaces: the space menu, the tray, and the desktop layer's menu.
fn new_entries() -> [MenuEntry; 2] {
    [
        menu_item(MenuId::NewSpace, "New space"),
        menu_item(MenuId::NewLens, "New lens\u{2026}"),
    ]
}

/// A lens's own entries in its space menu.
fn lens_folder_entries() -> [MenuEntry; 2] {
    [
        menu_item(MenuId::OpenFolder, "Open folder"),
        menu_item(MenuId::ChangeFolder, "Change folder\u{2026}"),
    ]
}

/// The view a lens's menu `choice` leaves it with, or `None` when the choice
/// does not change its folder view.
fn lens_view_after(view: &FolderView, choice: MenuId) -> Option<FolderView> {
    let mut next = view.clone();
    match choice {
        MenuId::ShowHidden => next.show_hidden = !view.show_hidden,
        _ => return None,
    }
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::{Rect, SortKey};

    #[test]
    fn the_new_shortcut_menu_maps_each_entry_back() {
        let MenuEntry::Submenu { label, entries, .. } = new_shortcut_menu(false) else {
            panic!("expected a submenu");
        };
        assert_eq!(label, "New shortcut");
        let ids: Vec<Option<MenuId>> = entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { id, .. } => Some(MenuId::from_id(*id)),
                _ => None,
            })
            .collect();
        assert_eq!(
            ids,
            [
                Some(MenuId::NewShortcutApps),
                Some(MenuId::NewShortcutCustom),
                Some(MenuId::PasteShortcut)
            ]
        );
        assert_eq!(
            new_shortcut_choice(MenuId::NewShortcutApps),
            Some(NewShortcut::Apps)
        );
        assert_eq!(
            new_shortcut_choice(MenuId::NewShortcutCustom),
            Some(NewShortcut::Custom)
        );
        assert_eq!(
            new_shortcut_choice(MenuId::PasteShortcut),
            Some(NewShortcut::Paste)
        );
        assert_eq!(new_shortcut_choice(MenuId::Fold), None);
    }

    /// The ids and check marks of `entries`' items, in order.
    fn item_checks(entries: &[MenuEntry]) -> Vec<(Option<MenuId>, bool)> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { id, checked, .. } => Some((MenuId::from_id(*id), *checked)),
                _ => None,
            })
            .collect()
    }

    fn sort_entries(view: SpaceView, lens: Option<&FolderView>) -> Vec<MenuEntry> {
        match space_sort_menu(view, lens) {
            MenuEntry::Submenu { entries, .. } => entries,
            _ => panic!("expected a submenu"),
        }
    }

    fn view_entries(space: &Space) -> Vec<MenuEntry> {
        match space_view_menu(space) {
            MenuEntry::Submenu { entries, .. } => entries,
            _ => panic!("expected a submenu"),
        }
    }

    #[test]
    fn the_view_menu_checks_the_space_mode_and_uses_the_table_labels() {
        use crate::fixtures::{apply, create_space};
        use deskspace_core::{Desktop, ViewMode};

        let mut desktop = Desktop::new();
        let id = create_space(&mut desktop, "A", Rect::new(0, 0, 100, 100));
        apply(
            &mut desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView::with_mode(ViewMode::LargeIcons),
            },
        );
        let entries = view_entries(desktop.space(id).unwrap());
        let checks = item_checks(&entries);
        assert_eq!(checks.len(), VIEW_PRESETS.len());
        assert_eq!(
            checks.iter().filter(|(_, checked)| *checked).count(),
            1,
            "one radio entry is checked"
        );
        assert_eq!(
            checks
                .iter()
                .find(|(_, checked)| *checked)
                .and_then(|(id, _)| *id),
            Some(MenuId::ViewLargeIcons)
        );
        let labels: Vec<&str> = entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { label, .. } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            VIEW_MODES
                .iter()
                .map(|(_, label, _)| *label)
                .collect::<Vec<&str>>()
        );
    }

    #[test]
    fn every_view_has_one_menu_preset_and_one_mode_row() {
        assert_eq!(VIEW_PRESETS.len(), VIEW_MODES.len());
        for (mode, ..) in VIEW_MODES {
            assert_eq!(
                VIEW_PRESETS
                    .iter()
                    .filter(|(_, preset)| *preset == mode)
                    .count(),
                1,
                "{mode:?}"
            );
        }
        for (_, mode) in VIEW_PRESETS {
            assert_eq!(
                VIEW_MODES.iter().filter(|(row, ..)| *row == mode).count(),
                1,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn the_sort_menu_checks_the_rule_and_uses_explorer_order_and_words() {
        let rule = SortRule {
            key: SortKey::Modified,
            descending: true,
            folders_first: true,
        };
        let entries = sort_entries(
            SpaceView {
                sort: rule,
                ..SpaceView::default()
            },
            None,
        );
        let labels: Vec<&str> = entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item { label, .. } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        let mut expected: Vec<&str> = SORT_KEYS.iter().map(|(_, label)| *label).collect();
        expected.extend(["Ascending", "Descending", "Folders first", "Keep sorted"]);
        assert_eq!(labels, expected);

        let checked = |entries: &[MenuEntry]| -> Vec<MenuId> {
            item_checks(entries)
                .into_iter()
                .filter(|(_, checked)| *checked)
                .filter_map(|(id, _)| id)
                .collect()
        };
        assert_eq!(
            checked(&entries),
            [
                MenuId::SortByModified,
                MenuId::SortDescending,
                MenuId::FoldersFirst
            ]
        );
        assert_eq!(
            checked(&sort_entries(SpaceView::default(), None)),
            [MenuId::SortByName, MenuId::SortAscending]
        );
    }

    #[test]
    fn a_lens_sort_menu_also_offers_show_hidden() {
        let view = FolderView {
            show_hidden: true,
            ..FolderView::default()
        };
        let checks = item_checks(&sort_entries(SpaceView::default(), Some(&view)));
        assert!(checks.contains(&(Some(MenuId::ShowHidden), true)));
        let normal = item_checks(&sort_entries(SpaceView::default(), None));
        assert!(
            !normal.iter().any(|(id, _)| *id == Some(MenuId::ShowHidden)),
            "a normal space has no hidden-files choice"
        );
    }

    /// Whether the item with `id` is enabled in `entries`.
    fn item_enabled(entries: &[MenuEntry], id: MenuId) -> Option<bool> {
        entries.iter().find_map(|entry| match entry {
            MenuEntry::Item {
                id: row, enabled, ..
            } if MenuId::from_id(*row) == Some(id) => Some(*enabled),
            _ => None,
        })
    }

    #[test]
    fn the_keep_sorted_entry_follows_the_view_and_greys_on_a_lens() {
        let kept = SpaceView {
            keep_sorted: true,
            ..SpaceView::default()
        };
        let checks = item_checks(&sort_entries(kept, None));
        assert!(checks.contains(&(Some(MenuId::KeepSorted), true)));
        let normal = sort_entries(SpaceView::default(), None);
        assert_eq!(item_enabled(&normal, MenuId::KeepSorted), Some(true));
        assert!(item_checks(&normal).contains(&(Some(MenuId::KeepSorted), false)));

        let lens = sort_entries(kept, Some(&FolderView::default()));
        assert_eq!(item_enabled(&lens, MenuId::KeepSorted), Some(false));
        assert!(
            item_checks(&lens).contains(&(Some(MenuId::KeepSorted), true)),
            "a lens is always kept sorted"
        );
    }

    #[test]
    fn keep_sorted_toggles_a_normal_space_only() {
        let view = SpaceView::default();
        let on = keep_sorted_after(view, MenuId::KeepSorted, false).unwrap();
        assert!(on.keep_sorted);
        assert_eq!(on.sort, view.sort, "the rule is kept");
        let off = keep_sorted_after(on, MenuId::KeepSorted, false).unwrap();
        assert!(!off.keep_sorted);
        assert_eq!(keep_sorted_after(view, MenuId::KeepSorted, true), None);
        assert_eq!(keep_sorted_after(view, MenuId::FoldersFirst, false), None);
    }

    #[test]
    fn every_sort_key_has_one_menu_entry() {
        assert_eq!(SORT_KEYS.len(), SORT_MENU_IDS.len());
        let menu_keys: Vec<SortKey> = SORT_MENU_IDS.iter().map(|(_, key)| *key).collect();
        let table_keys: Vec<SortKey> = SORT_KEYS.iter().map(|(key, _)| *key).collect();
        assert_eq!(menu_keys, table_keys, "the menu follows SORT_KEYS order");
        for (key, _) in SORT_KEYS {
            assert_eq!(
                SORT_MENU_IDS.iter().filter(|(_, row)| *row == key).count(),
                1,
                "{key:?}"
            );
        }
        for (_, key) in SORT_MENU_IDS {
            assert_eq!(
                SORT_KEYS.iter().filter(|(row, _)| *row == key).count(),
                1,
                "{key:?}"
            );
        }
    }

    #[test]
    fn a_sort_choice_changes_only_its_part_of_the_rule() {
        let rule = SortRule {
            key: SortKey::Size,
            descending: true,
            folders_first: true,
        };
        assert_eq!(
            sort_rule_after(rule, MenuId::SortByName),
            Some(SortRule {
                key: SortKey::Name,
                ..rule
            })
        );
        let ascending = sort_rule_after(rule, MenuId::SortAscending).unwrap();
        assert_eq!(
            (ascending.key, ascending.descending),
            (SortKey::Size, false)
        );
        assert!(ascending.folders_first, "the direction keeps folders first");
        let descending = sort_rule_after(rule, MenuId::SortDescending).unwrap();
        assert!(descending.descending);
        let folders = sort_rule_after(rule, MenuId::FoldersFirst).unwrap();
        assert!(!folders.folders_first);
        assert_eq!((folders.key, folders.descending), (SortKey::Size, true));
        assert_eq!(sort_rule_after(rule, MenuId::Fold), None);
    }

    #[test]
    fn a_lenss_hidden_choice_changes_only_the_hidden_flag() {
        let view = FolderView {
            path: "C:/Users/me/Downloads".into(),
            ..FolderView::default()
        };
        let hidden = lens_view_after(&view, MenuId::ShowHidden).unwrap();
        assert!(hidden.show_hidden);
        assert_eq!(hidden.path, view.path);
        assert_eq!(lens_view_after(&view, MenuId::FoldersFirst), None);
        assert_eq!(lens_view_after(&view, MenuId::SortByName), None);
    }

    #[test]
    fn new_lens_follows_new_space() {
        let ids: Vec<Option<MenuId>> = item_checks(&new_entries())
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, [Some(MenuId::NewSpace), Some(MenuId::NewLens)]);
    }

    #[test]
    fn a_lens_menu_offers_its_folder() {
        let ids: Vec<Option<MenuId>> = item_checks(&lens_folder_entries())
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, [Some(MenuId::OpenFolder), Some(MenuId::ChangeFolder)]);
    }

    #[test]
    fn paste_shortcut_follows_a_separator_and_the_clipboard() {
        let empty = new_shortcut_entries(false);
        assert_eq!(empty[2], MenuEntry::Separator);
        assert_eq!(
            empty[3],
            menu_item(MenuId::PasteShortcut, "Paste shortcut").enabled(false)
        );
        let filled = new_shortcut_entries(true);
        assert_eq!(
            filled[3],
            menu_item(MenuId::PasteShortcut, "Paste shortcut").enabled(true)
        );
    }

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
        let layout = app.config.desktop.layout;

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
}
