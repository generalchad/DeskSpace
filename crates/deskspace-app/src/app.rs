use deskspace_config::{Config, ConfigStore};
use deskspace_core::geometry::Point;
use deskspace_core::{Command, Desktop, SortKey, Space, SpaceId};
use deskspace_platform::{
    ClipboardAction, Event, EventHandler, IconCache, SelectionKey, TrayEvent, Ui, ZoomAction,
    cursor_pos, desktop_display_since,
};

use crate::settings::{self, HotkeyAction};
use crate::storage::ItemStore;

mod clipboard;
mod drag;
mod files;
mod hit;
mod hotkeys;
mod info_tip;
mod menus;
mod persist;
mod pointer;
mod rename;
mod selection;
mod settings_session;
mod update;
mod view;
mod visibility;
mod zoom;

use hotkeys::Hotkeys;
use pointer::PointerState;
use rename::EditState;
use selection::Selection;
use settings_session::SettingsUi;
use update::UpdateState;
pub(crate) use view::{LayerState, SystemLook, ViewCache};

/// How many global hotkeys can be registered at once, one per binding.
/// Width of the band along a space's edges that starts a resize.
const RESIZE_BORDER: i32 = 6;
/// `WHEEL_DELTA`: one notch of a standard mouse wheel.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
const WHEEL_DELTA: i32 = 120;
/// Icon-size percent changed by one Ctrl+wheel notch.
const ICON_SIZE_WHEEL_STEP: i32 = 10;
/// Wake ids passed to `Ui::waker` by the update worker threads: the check
/// result, then the download result.
const UPDATE_WAKE: usize = 0;
const UPDATE_DOWNLOADED_WAKE: usize = 1;
/// The app's timers, delivered back as `Event::Timer` with the variant's
/// value as the id. Never renumber one; a new timer takes the highest value
/// plus one and joins `ALL`, and `App::on_timer`'s exhaustive match asks for
/// its handler. Each delay constant lives in the module that starts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
enum Timer {
    /// The debounced configuration save.
    Autosave = 1,
    /// The blinking edit caret.
    Caret = 2,
    /// The delayed item info tip.
    InfoTip = 3,
    /// The periodic update check (`app/update.rs`).
    Update = 4,
    /// The settings-window highlight on a space.
    Highlight = 5,
    /// The deferred settings preview.
    Preview = 6,
    /// The type-to-select prefix buffer.
    Type = 7,
    /// The desktop-takeover health check.
    Health = 8,
    /// The quiet-moment poll of a downloaded automatic update
    /// (`app/update.rs`).
    UpdateIdle = 9,
    /// Hiding the zoom pop-up (`app/zoom.rs`).
    ZoomBar = 10,
}

impl Timer {
    const ALL: [Self; 10] = [
        Self::Autosave,
        Self::Caret,
        Self::InfoTip,
        Self::Update,
        Self::Highlight,
        Self::Preview,
        Self::Type,
        Self::Health,
        Self::UpdateIdle,
        Self::ZoomBar,
    ];

    fn from_id(id: usize) -> Option<Self> {
        Self::ALL.into_iter().find(|timer| *timer as usize == id)
    }
}

/// The app's popup-menu ids. Never renumber one; a new entry takes the
/// highest value plus one, below `0x8000`, where the shell's ids start
/// (`SHELL_FIRST_ID`), and joins `menu_ids_are_unique`.
mod menu {
    pub const ROLL: u32 = 1;
    pub const LOCK: u32 = 2;
    pub const NEW_SPACE: u32 = 3;
    pub const SETTINGS: u32 = 4;
    pub const TOGGLE_VISIBLE: u32 = 5;
    pub const EXIT: u32 = 6;
    pub const DELETE: u32 = 7;
    pub const FIT: u32 = 8;
    // 9 was the item menu's "Remove from space"; it stays unused.
    pub const RENAME: u32 = 10;
    pub const RENAME_ITEM: u32 = 11;
    pub const CHECK_UPDATES: u32 = 12;
    pub const INSTALL_UPDATE: u32 = 13;
    pub const OPACITY_100: u32 = 20;
    pub const OPACITY_80: u32 = 21;
    pub const OPACITY_60: u32 = 22;
    pub const OPACITY_40: u32 = 23;
    pub const OPACITY_20: u32 = 24;
    pub const SPACE_TINT: u32 = 25;
    pub const SPACE_TEXT: u32 = 26;
    pub const RESET_APPEARANCE: u32 = 27;
    pub const SPACE_SETTINGS: u32 = 28;
    pub const ICON_SIZE_SMALL: u32 = 41;
    pub const ICON_SIZE_MEDIUM: u32 = 42;
    pub const ICON_SIZE_LARGE: u32 = 43;
    pub const ICON_SIZE_XLARGE: u32 = 44;
    pub const SORT_NAME_ASC: u32 = 30;
    pub const SORT_NAME_DESC: u32 = 31;
    pub const SORT_KIND: u32 = 32;
    pub const SORT_SIZE_ASC: u32 = 33;
    pub const SORT_SIZE_DESC: u32 = 34;
    pub const SORT_DATE_ASC: u32 = 35;
    pub const SORT_DATE_DESC: u32 = 36;
    pub const NEW_SHORTCUT_APPS: u32 = 45;
    pub const NEW_SHORTCUT_CUSTOM: u32 = 46;
    pub const PASTE_SHORTCUT: u32 = 47;
    pub const ADD_FILES: u32 = 48;
}

/// A way to make a new shortcut in a space (F-53, F-54).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewShortcut {
    /// Pick apps from the Applications folder.
    Apps,
    /// Describe the target by hand.
    Custom,
    /// Shortcut to the items on the clipboard.
    Paste,
}

/// The "New shortcut" submenu: menu id, label, and what it makes. Shared by
/// the space menu and the settings window's Shortcuts page.
const NEW_SHORTCUT_ENTRIES: [(u32, &str, NewShortcut); 3] = [
    (
        menu::NEW_SHORTCUT_APPS,
        "From installed apps\u{2026}",
        NewShortcut::Apps,
    ),
    (
        menu::NEW_SHORTCUT_CUSTOM,
        "Custom shortcut\u{2026}",
        NewShortcut::Custom,
    ),
    (menu::PASTE_SHORTCUT, "Paste shortcut", NewShortcut::Paste),
];

/// The tint-opacity presets the Appearance submenu offers: menu id, label, and
/// the space's tint opacity in percent. Drives both the submenu and how a
/// choice is handled.
const OPACITY_PRESETS: [(u32, &str, u8); 5] = [
    (menu::OPACITY_100, "Opacity 100%", 100),
    (menu::OPACITY_80, "Opacity 80%", 80),
    (menu::OPACITY_60, "Opacity 60%", 60),
    (menu::OPACITY_40, "Opacity 40%", 40),
    (menu::OPACITY_20, "Opacity 20%", 20),
];

/// The icon-size choices the "Icon size" submenu offers: menu id, label, and
/// the space's icon size in percent.
const ICON_SIZE_PRESETS: [(u32, &str, u8); 4] = [
    (menu::ICON_SIZE_SMALL, "Small", 75),
    (menu::ICON_SIZE_MEDIUM, "Medium", 100),
    (menu::ICON_SIZE_LARGE, "Large", 150),
    (menu::ICON_SIZE_XLARGE, "Extra large", 200),
];

/// The sort choices the Sort by submenu offers: menu id, label, sort key, and
/// whether the order descends. Drives both the submenu and the choice handling.
const SORT_PRESETS: [(u32, &str, SortKey, bool); 7] = [
    (
        menu::SORT_NAME_ASC,
        "Name (A\u{2013}Z)",
        SortKey::Name,
        false,
    ),
    (
        menu::SORT_NAME_DESC,
        "Name (Z\u{2013}A)",
        SortKey::Name,
        true,
    ),
    (menu::SORT_KIND, "Type", SortKey::Kind, false),
    (menu::SORT_SIZE_ASC, "Size (smallest)", SortKey::Size, false),
    (menu::SORT_SIZE_DESC, "Size (largest)", SortKey::Size, true),
    (
        menu::SORT_DATE_ASC,
        "Date (oldest)",
        SortKey::Modified,
        false,
    ),
    (
        menu::SORT_DATE_DESC,
        "Date (newest)",
        SortKey::Modified,
        true,
    ),
];

/// The event id the platform gives the chrome-less desktop layer window
/// (`Ui::create_desktop_layer`). Real spaces get ids from `Desktop::next_space_id`
/// starting at 1, so 0 is free to mean the loose-desktop surface.
const DESKTOP_LAYER_ID: SpaceId = SpaceId(0);

/// Where item interaction happens: a space's window, or the desktop layer that
/// holds the loose desktop items (ROADMAP D7). Hit-testing, selection, hover,
/// drag, and drop go through this so one item path serves both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    Space(SpaceId),
    Desktop,
}

impl Surface {
    /// The surface an event id belongs to: 0 is the desktop layer, anything
    /// else a real space.
    fn of(id: SpaceId) -> Self {
        if id == DESKTOP_LAYER_ID {
            Self::Desktop
        } else {
            Self::Space(id)
        }
    }
}

/// Which per-space color an inline color picker edits.
#[derive(Debug, Clone, Copy)]
enum SpaceColor {
    Tint,
    Text,
}

/// A key that scrolls the current space.
#[derive(Debug, Clone, Copy)]
enum ScrollKey {
    Home,
    End,
    PageUp,
    PageDown,
}

/// The space refreshes a settings preview still owes: a restyle re-applies a
/// window style and repaints, a re-flow re-positions items. Global edits owe
/// them for every space, a space's own edits only for that space.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Preview {
    restyle_all: bool,
    relayout_all: bool,
    restyle: Vec<SpaceId>,
    relayout: Vec<SpaceId>,
}

impl Preview {
    fn restyle_all() -> Self {
        Self {
            restyle_all: true,
            ..Self::default()
        }
    }

    fn relayout_all() -> Self {
        Self {
            relayout_all: true,
            ..Self::default()
        }
    }

    fn restyle(id: SpaceId) -> Self {
        Self {
            restyle: vec![id],
            ..Self::default()
        }
    }

    fn relayout(id: SpaceId) -> Self {
        Self {
            relayout: vec![id],
            ..Self::default()
        }
    }

    fn is_pending(&self) -> bool {
        self.restyle_all
            || self.relayout_all
            || !self.restyle.is_empty()
            || !self.relayout.is_empty()
    }

    fn merge(&mut self, other: Preview) {
        self.restyle_all |= other.restyle_all;
        self.relayout_all |= other.relayout_all;
        for id in other.restyle {
            if !self.restyle.contains(&id) {
                self.restyle.push(id);
            }
        }
        for id in other.relayout {
            if !self.relayout.contains(&id) {
                self.relayout.push(id);
            }
        }
    }
}

/// Coalesces configuration writes: every change marks it dirty, and one save
/// pays for all of them once the user pauses.
#[derive(Debug, Default)]
struct Autosave {
    dirty: bool,
}

impl Autosave {
    fn mark(&mut self) {
        self.dirty = true;
    }

    /// Reports whether a save is due, clearing the flag.
    fn take(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
}

pub struct App {
    config: Config,
    pub(crate) store: ConfigStore,
    pub(crate) storage: ItemStore,
    icons: IconCache,
    /// What Windows reports: dark mode, the accent, Explorer's display.
    system: SystemLook,
    /// The item selection: the current surface, its items, the Shift anchor,
    /// the keyboard focus, and the cut and check-box marks.
    selection: Selection,
    /// The transient pointer state: the in-progress drag, the hover, the
    /// rubber-band rectangle, and the drag image.
    pointer: PointerState,
    /// The in-progress inline rename, of a space title or an item label.
    edit: EditState,
    /// Pending configuration write, debounced by `Timer::Autosave`.
    autosave: Autosave,
    /// Per-space view state that is not persisted: scroll and icon factor.
    view_cache: ViewCache,
    visible: bool,
    /// The registered global hotkeys, indexed by `hotkey_id`'s index.
    hotkeys: Hotkeys,
    settings_ui: SettingsUi,
    open_settings_on_start: bool,
    /// The desktop layer's flags and the shell entries it draws.
    pub(crate) layer: LayerState,
    /// Update checking and installation state (`app/update.rs`).
    update: UpdateState,
}

impl App {
    pub fn new(config: Config, store: ConfigStore, hidden: bool, open_settings: bool) -> Self {
        Self {
            config,
            storage: ItemStore::new(store.storage_dir()),
            store,
            icons: IconCache::new(),
            system: SystemLook::read(),
            // Read for real in `Started`, once OLE is initialized.
            selection: Selection::default(),
            pointer: PointerState::default(),
            edit: EditState::default(),
            autosave: Autosave::default(),
            view_cache: ViewCache::default(),
            visible: !hidden,
            hotkeys: Hotkeys::default(),
            settings_ui: SettingsUi::default(),
            open_settings_on_start: open_settings,
            layer: LayerState::default(),
            update: UpdateState::default(),
        }
    }

    /// Handles Alt+Enter: shows the focused (last-selected) item's shell
    /// Properties dialog, or DeskSpace Settings when no item is selected.
    fn show_properties(&mut self, ui: &mut Ui) {
        if let Some(entry) = self
            .selection
            .items
            .last()
            .and_then(|item| self.config.desktop.item(*item))
        {
            ui.show_properties(&entry.path);
            return;
        }
        self.open_settings(ui);
    }

    /// Handles a watched selection key (see `SelectionKey`).
    fn on_selection_key(&mut self, ui: &mut Ui, key: SelectionKey) {
        match key {
            SelectionKey::Open => self.open_selected_item(ui),
            SelectionKey::Clear => self.clear_selection(ui),
            SelectionKey::PermanentDelete => self.delete_selected_items(ui, true),
            SelectionKey::Refresh => self.refresh_spaces(ui),
            SelectionKey::Menu => self.open_context_menu(ui),
            SelectionKey::Home => self.scroll_current_space(ui, ScrollKey::Home),
            SelectionKey::End => self.scroll_current_space(ui, ScrollKey::End),
            SelectionKey::PageUp => self.scroll_current_space(ui, ScrollKey::PageUp),
            SelectionKey::PageDown => self.scroll_current_space(ui, ScrollKey::PageDown),
            SelectionKey::ZoomIn => self.zoom(ui, ZoomAction::In),
            SelectionKey::ZoomOut => self.zoom(ui, ZoomAction::Out),
            SelectionKey::ZoomReset => self.zoom(ui, ZoomAction::Reset),
        }
    }

    /// Opens the focused (last-selected) item, as Enter does in Explorer.
    fn open_selected_item(&mut self, ui: &mut Ui) {
        if let Some(path) = self
            .selection
            .items
            .last()
            .and_then(|item| self.config.desktop.item(*item))
            .map(|item| item.path.clone())
        {
            ui.open_path(&path);
        }
    }

    fn toggle_rollup(&mut self, ui: &mut Ui, id: SpaceId) {
        let near = self.space_center(id);
        // A rolled-up space hides its items, so it cannot keep one selected.
        if self.selection.surface == Some(Surface::Space(id)) {
            self.selection.clear();
        }
        self.commit(ui, Command::ToggleRollup { id }, near);
    }

    // ---- timers --------------------------------------------------------

    fn set_timer(ui: &mut Ui, timer: Timer, millis: u32) {
        ui.set_timer(timer as usize, millis);
    }

    fn clear_timer(ui: &mut Ui, timer: Timer) {
        ui.clear_timer(timer as usize);
    }

    /// Runs the work of a timer that fired.
    fn on_timer(&mut self, ui: &mut Ui, timer: Timer) {
        match timer {
            Timer::Autosave => self.flush_save(ui),
            Timer::Caret => self.blink_caret(ui),
            Timer::InfoTip => self.show_info_tip(ui),
            Timer::Update => self.on_update_timer(ui),
            Timer::Highlight => self.clear_highlight(ui),
            Timer::Preview => self.flush_preview(ui),
            // The type-to-select prefix expired; the next key starts fresh.
            Timer::Type => self.selection.clear_typed(),
            Timer::Health => self.health_tick(),
            Timer::UpdateIdle => self.on_update_idle_timer(ui),
            Timer::ZoomBar => self.on_zoom_bar_timer(ui),
        }
    }

    // ---- hotkey --------------------------------------------------------

    /// Runs the action of the binding whose hotkey `id` was pressed.
    fn on_hotkey(&mut self, ui: &mut Ui, id: i32) {
        let Some(binding) = Hotkeys::slot_for(id).map(|slot| &settings::HOTKEY_BINDINGS[slot])
        else {
            return;
        };
        match binding.action {
            HotkeyAction::ToggleSpaces => self.set_visible(ui, !self.visible),
            HotkeyAction::NewSpace => self.create_space(ui, cursor_pos()),
            HotkeyAction::OpenSettings => self.open_settings(ui),
        }
    }
}

impl EventHandler for App {
    fn on_event(&mut self, event: Event, ui: &mut Ui) {
        match event {
            Event::Started => {
                // DeskSpace's own menus and settings window follow the
                // `ui_mode` setting; the spaces follow their own Color mode.
                ui.set_ui_dark(self.ui_dark());
                self.hotkeys.register_all(ui, &self.config.window);
                ui.set_spaces_visible(self.visible);
                self.sync_desktop_show_entry();
                // Keep the stored Run path current after an install moves.
                if self.config.window.start_with_windows {
                    self.sync_start_with_windows(true);
                }
                // OLE is up by now, so the desktop view can be queried.
                let display = desktop_display_since(self.system.shell().check_boxes);
                self.system.set_shell(display);
                self.refresh_desktop_entries();
                self.apply_takeover(ui);
                self.sync_windows(ui);
                // Positions are derived from the grid; recompute them so a
                // stored layout from another icon size or DPI is refreshed.
                self.relayout_all(ui);
                self.prune_all_missing(ui);
                tracing::info!(
                    spaces = self.config.desktop.spaces().len(),
                    dark = self.system.dark(),
                    "spaces created"
                );
                if std::mem::take(&mut self.open_settings_on_start) {
                    self.open_settings(ui);
                }
                self.sync_update_schedule(ui);
                // Warm the installed-apps list, so "New shortcut ▸ From
                // installed apps…" opens at once.
                ui.prefetch_apps();
            }
            Event::Space { id, event } => self.on_space_event(ui, SpaceId(id), event),
            Event::EditKey(key) => self.apply_edit_key(ui, key),
            Event::DeleteSelection => self.delete_selected_items(ui, false),
            Event::Clipboard(ClipboardAction::Copy) => self.copy_selection(ui, false),
            Event::Clipboard(ClipboardAction::Cut) => self.copy_selection(ui, true),
            Event::Clipboard(ClipboardAction::Paste) => self.paste_clipboard(ui),
            Event::ClipboardChanged => self.sync_cut_with_clipboard(ui),
            Event::RenameSelection => self.begin_f2_rename(ui),
            Event::ShowProperties => self.show_properties(ui),
            Event::SelectionKey(key) => self.on_selection_key(ui, key),
            Event::SelectAll => self.select_all_in_space(ui),
            Event::Typed(ch) => self.on_typed(ui, ch),
            Event::Navigate(key) => self.on_navigate(ui, key),
            Event::PointerDownOutside { on_desktop } => {
                self.dismiss_info_tip(ui);
                if self.edit.is_active() {
                    self.finish_rename(ui, true);
                } else if on_desktop {
                    self.clear_selection(ui);
                }
            }
            Event::ShowSpaces => self.set_visible(ui, true),
            // A watched folder changed (I8): re-read the shell's loose items
            // and reconcile the layer. A no-op when the layer is not drawn.
            Event::FolderChanged => self.resync_desktop_layer(ui),
            Event::Hotkey { id } => self.on_hotkey(ui, id),
            Event::Tray(TrayEvent::Activated) => self.open_settings(ui),
            Event::Tray(TrayEvent::MenuRequested { x, y }) => {
                self.show_tray_menu(ui, Point::new(x, y));
            }
            Event::Settings(event) => self.on_settings_event(ui, event),
            Event::Timer { id } => {
                if let Some(timer) = Timer::from_id(id) {
                    self.on_timer(ui, timer);
                }
            }
            // Worker threads hand their result back through the waker.
            Event::Wake { id } if id == UPDATE_WAKE => self.on_update_checked(ui),
            Event::Wake { id } if id == UPDATE_DOWNLOADED_WAKE => self.on_update_downloaded(ui),
            Event::Wake { .. } => {}
            Event::Zoom(action) => self.zoom(ui, action),
            Event::SystemThemeChanged => {
                self.system.refresh();
                ui.set_ui_dark(self.ui_dark());
                // The desktop view can change its icon size with the theme;
                // refresh its display options, then restyle for dark mode.
                self.refresh_shell_display(ui);
                self.restyle_all(ui);
                self.refresh_settings(ui);
            }
            Event::CloseRequested => {
                self.hotkeys.unregister_all(ui);
                // A rename in progress and unapplied settings edits are
                // discarded.
                self.cancel_edit(ui);
                self.cancel_settings(ui);
                self.flush_save(ui);
                self.remove_desktop_show_entry();
            }
        }
        // The keyboard hook is on only while it is needed: editing captures
        // keys, and a selection or current space watches the item keys.
        ui.capture_text_input(self.edit.is_active());
        ui.watch_selection_keys(
            !self.edit.is_active() && !self.selection.items.is_empty(),
            !self.edit.is_active() && self.selection.surface.is_some(),
        );
        self.sync_outside_watch(ui);
    }
}

/// Deleting an empty space loses nothing, so it happens without asking.
fn delete_needs_confirmation(space: &Space) -> bool {
    !space.items().is_empty()
}

/// What deleting a space with `count` items, `stored` of them in the space
/// storage, does to the files.
fn delete_consequence(count: usize, stored: usize) -> String {
    let plural = |n: usize| if n == 1 { "item" } else { "items" };
    match (count, stored) {
        (0, _) => "It is empty.".to_string(),
        (_, 0) => format!(
            "Its {count} {} will be removed from DeskSpace. The files themselves are not deleted.",
            plural(count)
        ),
        _ => format!(
            "Its {count} {} will be removed from DeskSpace, and the {stored} {} moved into \
             the space will be moved back to the desktop. No files are deleted.",
            plural(count),
            if stored == 1 { "file" } else { "files" }
        ),
    }
}

pub(crate) fn tray_tooltip(desktop: &Desktop) -> String {
    let spaces = desktop.spaces().len();
    let items = desktop.space_item_count();
    let space_word = if spaces == 1 { "space" } else { "spaces" };
    let item_word = if items == 1 { "item" } else { "items" };
    format!("DeskSpace \u{2014} {spaces} {space_word}, {items} {item_word}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use deskspace_core::ItemFacts;
    use deskspace_core::{ItemId, Rect};
    use std::path::PathBuf;

    #[test]
    fn timer_ids_round_trip() {
        for timer in Timer::ALL {
            assert_eq!(Timer::from_id(timer as usize), Some(timer));
        }
        assert_eq!(Timer::from_id(0), None);
        assert_eq!(Timer::from_id(11), None);
    }

    #[test]
    fn item_cells_place_each_dragged_item_in_desktop_coordinates() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let desktop = &mut app.config.desktop;
        let id = fixtures::create_space(desktop, "S", Rect::new(100, 100, 240, 200));
        let first = fixtures::add_item(desktop, id, "C:/missing/1.txt");
        let second = fixtures::add_item(desktop, id, "C:/missing/2.txt");
        let space = app.config.desktop.space(id).unwrap();
        let content = space.content_rect();
        let at = |item| {
            let position = space.item(item).unwrap().position;
            Point::new(content.x + position.x, content.y + position.y)
        };

        // Drag order is kept, and an id not in the space is skipped.
        let cells = app.item_cells(Surface::Space(id), &[second, ItemId(9), first]);
        assert_eq!(
            cells,
            [
                (PathBuf::from("C:/missing/2.txt"), at(second)),
                (PathBuf::from("C:/missing/1.txt"), at(first)),
            ]
        );
    }

    #[test]
    fn autosave_coalesces_changes_into_one_write() {
        let mut save = Autosave::default();
        assert!(!save.take());
        save.mark();
        save.mark();
        assert!(save.take());
        assert!(!save.take());
    }

    #[test]
    fn menu_ids_are_unique() {
        // The ids the preset tables do not cover. Every other id lives here, so
        // a table entry colliding with one fails the uniqueness check below.
        let other = [
            menu::ROLL,
            menu::LOCK,
            menu::NEW_SPACE,
            menu::SETTINGS,
            menu::TOGGLE_VISIBLE,
            menu::EXIT,
            menu::DELETE,
            menu::FIT,
            menu::RENAME,
            menu::RENAME_ITEM,
            menu::SPACE_TINT,
            menu::SPACE_TEXT,
            menu::RESET_APPEARANCE,
            menu::SPACE_SETTINGS,
            menu::CHECK_UPDATES,
            menu::ADD_FILES,
        ];
        let mut ids: Vec<u32> = other.to_vec();
        ids.extend(OPACITY_PRESETS.iter().map(|(id, ..)| *id));
        ids.extend(ICON_SIZE_PRESETS.iter().map(|(id, ..)| *id));
        ids.extend(SORT_PRESETS.iter().map(|(id, ..)| *id));
        ids.extend(NEW_SHORTCUT_ENTRIES.iter().map(|(id, ..)| *id));
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "menu ids collide");
    }

    #[test]
    fn the_layer_id_is_the_desktop_surface() {
        assert_eq!(Surface::of(SpaceId(0)), Surface::Desktop);
        assert_eq!(Surface::of(SpaceId(7)), Surface::Space(SpaceId(7)));
    }

    #[test]
    fn takeover_activates_the_layer_and_icon_hiding() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        assert!(!app.layer_active());
        assert!(!app.desktop_takeover());

        // The additive dev flag draws the layer but does not hide icons.
        app.layer = LayerState::new(true, false);
        assert!(app.layer_active());
        assert!(!app.desktop_takeover());

        // The full takeover flag hides icons and implies the layer.
        app.layer = LayerState::new(false, true);
        assert!(app.layer_active());
        assert!(app.desktop_takeover());

        // The saved setting alone does the same.
        app.layer = LayerState::default();
        app.config.window.take_over_desktop = true;
        assert!(app.layer_active());
        assert!(app.desktop_takeover());
    }

    #[test]
    fn a_loose_item_is_hit_on_the_desktop_surface() {
        use deskspace_core::Item;
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let mut item = Item::new(ItemId(9), "C:/missing/loose.txt", ItemFacts::default());
        item.position = Point::new(40, 60);
        fixtures::apply(
            &mut app.config.desktop,
            Command::SyncDesktop { items: vec![item] },
        );
        let loose = app.config.desktop.layer().items()[0].id;

        assert_eq!(
            app.surface_item_at(Surface::Desktop, Point::new(50, 70)),
            Some(loose)
        );
        assert_eq!(
            app.surface_item_at(Surface::Desktop, Point::new(500, 500)),
            None
        );
        let cell = app.surface_layout(Surface::Desktop).cell();
        let entry = app.config.desktop.layer().item(loose).unwrap();
        assert_eq!(
            app.surface_cell(Surface::Desktop, entry),
            Rect::new(40, 60, cell.width, cell.height)
        );
    }

    #[test]
    fn only_spaces_with_items_ask_before_deleting() {
        let mut space = Space::new(SpaceId(1), "S", Rect::new(0, 0, 200, 200));
        assert!(!delete_needs_confirmation(&space));
        space.push_item(deskspace_core::Item::new(
            ItemId(1),
            "C:/a.txt",
            ItemFacts::default(),
        ));
        assert!(delete_needs_confirmation(&space));
    }

    #[test]
    fn delete_message_mentions_files_moved_back() {
        assert_eq!(delete_consequence(0, 0), "It is empty.");
        assert!(delete_consequence(1, 0).contains("1 item will be removed"));
        let mixed = delete_consequence(3, 2);
        assert!(mixed.contains("3 items"));
        assert!(mixed.contains("2 files moved into the space will be moved back"));
    }

    #[test]
    fn tooltip_pluralizes() {
        let mut desktop = Desktop::new();
        assert_eq!(
            tray_tooltip(&desktop),
            "DeskSpace \u{2014} 0 spaces, 0 items"
        );
        fixtures::create_space(&mut desktop, "A", Rect::new(0, 0, 10, 10));
        assert_eq!(
            tray_tooltip(&desktop),
            "DeskSpace \u{2014} 1 space, 0 items"
        );
    }
}
