use std::collections::HashMap;

use deskspace_config::{Config, ConfigStore};
use deskspace_core::geometry::Point;
use deskspace_core::{Color, Command, Desktop, SortKey, Space, SpaceId};
use deskspace_platform::{
    ClipboardAction, DesktopEntry, Event, EventHandler, Hotkey, IconCache, SelectionKey,
    ShellDisplay, TrayEvent, Ui, cursor_pos, desktop_display, hotkey_id, parse_hotkey,
    system_accent_color, system_uses_dark_mode,
};

use crate::settings::{self, Session};
use crate::storage::ItemStore;

mod clipboard;
mod files;
mod hit;
mod info_tip;
mod menus;
mod persist;
mod pointer;
mod rename;
mod selection;
mod settings_session;
mod view;
mod visibility;

use pointer::PointerState;
use rename::EditState;
use selection::Selection;

const TOGGLE_HOTKEY: i32 = 0;
/// Registration indices for the global hotkeys. `settings::HOTKEY_BINDINGS`
/// holds the matching table; a test keeps the two in step.
const NEW_SPACE_HOTKEY: i32 = 1;
const SETTINGS_HOTKEY: i32 = 2;
/// How many global hotkeys can be registered at once.
const HOTKEY_COUNT: usize = 3;
/// Width of the band along a space's edges that starts a resize.
const RESIZE_BORDER: i32 = 6;
/// `WHEEL_DELTA`: one notch of a standard mouse wheel.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
const WHEEL_DELTA: i32 = 120;
/// Icon-size percent changed by one Ctrl+wheel notch.
const ICON_SIZE_WHEEL_STEP: i32 = 10;
/// Timer id for the debounced configuration save.
const AUTOSAVE_TIMER: usize = 1;
/// How long after the last change the configuration is written.
const AUTOSAVE_DELAY_MS: u32 = 700;
/// Timer id for the blinking edit caret.
const CARET_TIMER: usize = 2;
/// Caret blink half-period, matching Windows' default.
const CARET_BLINK_MS: u32 = 530;
/// Timer id for the delayed item info tip.
const INFO_TIP_TIMER: usize = 3;
/// How long the pointer must rest on an item before its info tip appears,
/// matching Explorer's tooltip delay.
const INFO_TIP_DELAY_MS: u32 = 600;
/// Timer id for the deferred settings preview (id 4 is reserved by TASKS.md
/// E10).
const PREVIEW_TIMER: usize = 6;
/// How long a settings preview may lag the edit that caused it. `WM_TIMER`
/// is only delivered once the queue is empty, so a burst of slider ticks
/// collapses into one restyle or re-flow.
const PREVIEW_DELAY_MS: u32 = 15;
/// Timer id for the settings-window highlight on a space.
const HIGHLIGHT_TIMER: usize = 5;
/// How long the space selected in the settings window stays highlighted.
const HIGHLIGHT_MS: u32 = 1200;
/// Timer id for the type-to-select prefix buffer.
const TYPE_TIMER: usize = 7;
/// How long a type-to-select prefix lasts without another keystroke.
const TYPE_DELAY_MS: u32 = 900;
/// Timer id for the desktop-takeover health check.
const HEALTH_TIMER: usize = 8;
/// How often, while takeover is on, to re-hide Explorer's icon layer in case
/// it recreated it without broadcasting `TaskbarCreated`.
const HEALTH_MS: u32 = 5_000;

mod menu {
    pub const ROLL: u32 = 1;
    pub const LOCK: u32 = 2;
    pub const NEW_SPACE: u32 = 3;
    pub const SETTINGS: u32 = 4;
    pub const TOGGLE_VISIBLE: u32 = 5;
    pub const EXIT: u32 = 6;
    pub const DELETE: u32 = 7;
    pub const FIT: u32 = 8;
    pub const REMOVE_ITEM: u32 = 9;
    pub const RENAME: u32 = 10;
    pub const RENAME_ITEM: u32 = 11;
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
}

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
/// (`Ui::create_desktop_layer`). Real spaces get ids from `Desktop::add_space`
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
    system_dark: bool,
    /// Windows' accent color, refreshed on `SystemThemeChanged`; `None` when it
    /// cannot be read.
    system_accent: Option<Color>,
    /// Explorer's current display options, refreshed on change.
    shell_display: ShellDisplay,
    /// The desktop's loose items read from the shell on `Started`. The M2
    /// desktop layer consumes them; until then they are only diagnostics.
    desktop_entries: Vec<DesktopEntry>,
    /// The item selection: the current surface, its items, the Shift anchor,
    /// the keyboard focus, and the cut and check-box marks.
    selection: Selection,
    /// The transient pointer state: the in-progress drag, the hover, the
    /// rubber-band rectangle, and the drag image.
    pointer: PointerState,
    /// The in-progress inline rename, of a space title or an item label.
    edit: EditState,
    /// Pending configuration write, debounced by `AUTOSAVE_TIMER`.
    autosave: Autosave,
    /// Per-space scroll offset in pixels. Not persisted; may exceed the
    /// current maximum after a resize, so read it through `scroll_of`.
    scroll: HashMap<SpaceId, i32>,
    visible: bool,
    /// The registered global hotkeys, indexed by `hotkey_id`'s index.
    hotkeys: [Option<Hotkey>; HOTKEY_COUNT],
    /// Whether the platform is reporting clicks outside spaces; kept on only
    /// while an item is selected.
    watching_outside: bool,
    /// A settings edit is waiting for `PREVIEW_TIMER` to restyle or re-flow
    /// every space.
    pending_preview: Preview,
    settings: Option<Session>,
    /// The space highlighted for the settings window, until `HIGHLIGHT_TIMER`.
    highlighted: Option<SpaceId>,
    /// The keys typed so far for type-to-select, cleared by `TYPE_TIMER`.
    type_buffer: String,
    open_settings_on_start: bool,
    /// Draw the loose desktop items on a chrome-less layer window (hidden
    /// `--takeover-layer`, M2 takeover). Additive: Explorer still draws its
    /// own icons.
    pub(crate) takeover_layer: bool,
    /// Hide Explorer's icons and draw the loose items on the layer (hidden
    /// `--takeover`, M2 takeover). Implies the layer.
    pub(crate) takeover: bool,
}

impl App {
    pub fn new(config: Config, store: ConfigStore, hidden: bool, open_settings: bool) -> Self {
        Self {
            config,
            storage: ItemStore::new(store.storage_dir()),
            store,
            icons: IconCache::new(),
            system_dark: system_uses_dark_mode(),
            system_accent: system_accent_color(),
            // Read for real in `Started`, once OLE is initialized.
            shell_display: ShellDisplay::default(),
            desktop_entries: Vec::new(),
            selection: Selection::default(),
            pointer: PointerState::default(),
            edit: EditState::default(),
            autosave: Autosave::default(),
            scroll: HashMap::new(),
            visible: !hidden,
            hotkeys: [None; HOTKEY_COUNT],
            watching_outside: false,
            pending_preview: Preview::default(),
            settings: None,
            highlighted: None,
            type_buffer: String::new(),
            open_settings_on_start: open_settings,
            takeover_layer: false,
            takeover: false,
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
        self.apply_to_space(ui, id, Command::ToggleRollup { id }, near);
    }

    // ---- hotkey --------------------------------------------------------

    /// Registers the binding at `index` to `text`, replacing any previous one.
    /// An empty `text` leaves the action unbound.
    fn register_hotkey(&mut self, ui: &mut Ui, index: i32, text: &str) -> Result<(), String> {
        if let Some(previous) = self.hotkeys[index as usize].take() {
            ui.unregister_hotkey(previous);
        }
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        let spec =
            parse_hotkey(text).ok_or_else(|| format!("\"{text}\" is not a valid hotkey."))?;
        match ui.register_hotkey(index, spec) {
            Ok(hotkey) => {
                self.hotkeys[index as usize] = Some(hotkey);
                Ok(())
            }
            Err(error) => {
                tracing::warn!(%error, hotkey = text, index, "could not register hotkey");
                Err(format!("{text} is already in use by another app."))
            }
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
                for binding in &settings::HOTKEY_BINDINGS {
                    let text =
                        settings::hotkey_setting(&self.config.window, binding.index).to_string();
                    if let Err(message) = self.register_hotkey(ui, binding.index, &text) {
                        tracing::warn!(message, "hotkey unavailable");
                    }
                }
                ui.set_spaces_visible(self.visible);
                self.sync_desktop_show_entry();
                // Keep the stored Run path current after an install moves.
                if self.config.window.start_with_windows {
                    self.sync_start_with_windows(true);
                }
                // OLE is up by now, so the desktop view can be queried.
                self.shell_display = desktop_display();
                self.refresh_desktop_entries();
                self.apply_takeover(ui);
                self.sync_windows(ui);
                // Positions are derived from the grid; recompute them so a
                // stored layout from another icon size or DPI is refreshed.
                self.relayout_all(ui);
                self.prune_all_missing(ui);
                tracing::info!(
                    spaces = self.config.desktop.spaces.len(),
                    dark = self.system_dark,
                    "spaces created"
                );
                if std::mem::take(&mut self.open_settings_on_start) {
                    self.open_settings(ui);
                }
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
            Event::Hotkey { id } if id == hotkey_id(TOGGLE_HOTKEY) => {
                self.set_visible(ui, !self.visible);
            }
            Event::Hotkey { id } if id == hotkey_id(NEW_SPACE_HOTKEY) => {
                self.create_space(ui, cursor_pos());
            }
            Event::Hotkey { id } if id == hotkey_id(SETTINGS_HOTKEY) => self.open_settings(ui),
            Event::Hotkey { .. } => {}
            Event::Tray(TrayEvent::Activated) => self.open_settings(ui),
            Event::Tray(TrayEvent::MenuRequested { x, y }) => {
                self.show_tray_menu(ui, Point::new(x, y));
            }
            Event::Settings(event) => self.on_settings_event(ui, event),
            Event::Timer { id } if id == AUTOSAVE_TIMER => self.flush_save(ui),
            Event::Timer { id } if id == CARET_TIMER => self.blink_caret(ui),
            Event::Timer { id } if id == INFO_TIP_TIMER => self.show_info_tip(ui),
            Event::Timer { id } if id == PREVIEW_TIMER => self.flush_preview(ui),
            Event::Timer { id } if id == HIGHLIGHT_TIMER => self.clear_highlight(ui),
            // The type-to-select prefix expired; the next key starts fresh.
            Event::Timer { id } if id == TYPE_TIMER => self.type_buffer.clear(),
            Event::Timer { id } if id == HEALTH_TIMER => self.health_tick(),
            Event::Timer { .. } => {}
            Event::SystemThemeChanged => {
                self.system_dark = system_uses_dark_mode();
                self.system_accent = system_accent_color();
                ui.set_ui_dark(self.ui_dark());
                // The desktop view can change its icon size with the theme;
                // refresh its display options, then restyle for dark mode.
                self.refresh_shell_display(ui);
                self.restyle_all(ui);
                self.refresh_desktop_layer(ui);
                self.refresh_settings(ui);
            }
            Event::CloseRequested => {
                for hotkey in self.hotkeys.iter_mut().filter_map(Option::take) {
                    ui.unregister_hotkey(hotkey);
                }
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
    !space.items.is_empty()
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
    let spaces = desktop.spaces.len();
    let items = desktop.space_item_count();
    let space_word = if spaces == 1 { "space" } else { "spaces" };
    let item_word = if items == 1 { "item" } else { "items" };
    format!("DeskSpace \u{2014} {spaces} {space_word}, {items} {item_word}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::{ItemId, Rect};
    use std::path::PathBuf;

    #[test]
    fn item_cells_place_each_dragged_item_in_desktop_coordinates() {
        use deskspace_core::Item;
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = app
            .config
            .desktop
            .add_space("S", Rect::new(100, 100, 240, 200));
        for (n, position) in [(1, Point::new(12, 12)), (2, Point::new(118, 126))] {
            let mut item = Item::new(ItemId(n), format!("C:/missing/{n}.txt"));
            item.position = position;
            app.config.desktop.add_item(id, item);
        }
        let content = app.config.desktop.space(id).unwrap().content_rect();

        // Drag order is kept, and an id not in the space is skipped.
        let cells = app.item_cells(Surface::Space(id), &[ItemId(2), ItemId(9), ItemId(1)]);
        assert_eq!(
            cells,
            [
                (
                    PathBuf::from("C:/missing/2.txt"),
                    Point::new(content.x + 118, content.y + 126)
                ),
                (
                    PathBuf::from("C:/missing/1.txt"),
                    Point::new(content.x + 12, content.y + 12)
                ),
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
            menu::REMOVE_ITEM,
            menu::RENAME,
            menu::RENAME_ITEM,
            menu::SPACE_TINT,
            menu::SPACE_TEXT,
            menu::RESET_APPEARANCE,
        ];
        let mut ids: Vec<u32> = other.to_vec();
        ids.extend(OPACITY_PRESETS.iter().map(|(id, ..)| *id));
        ids.extend(ICON_SIZE_PRESETS.iter().map(|(id, ..)| *id));
        ids.extend(SORT_PRESETS.iter().map(|(id, ..)| *id));
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "menu ids collide");
    }

    #[test]
    fn hotkey_indices_match_the_settings_bindings() {
        let bindings = crate::settings::HOTKEY_BINDINGS;
        assert_eq!(bindings.len(), HOTKEY_COUNT);
        assert_eq!(bindings[0].index, TOGGLE_HOTKEY);
        assert_eq!(bindings[1].index, NEW_SPACE_HOTKEY);
        assert_eq!(bindings[2].index, SETTINGS_HOTKEY);
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
        app.takeover_layer = true;
        assert!(app.layer_active());
        assert!(!app.desktop_takeover());

        // The full takeover flag hides icons and implies the layer.
        app.takeover_layer = false;
        app.takeover = true;
        assert!(app.layer_active());
        assert!(app.desktop_takeover());

        // The saved setting alone does the same.
        app.takeover = false;
        app.config.window.take_over_desktop = true;
        assert!(app.layer_active());
        assert!(app.desktop_takeover());
    }

    #[test]
    fn a_loose_item_is_hit_on_the_desktop_surface() {
        use deskspace_core::Item;
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let mut item = Item::new(ItemId(9), "C:/missing/loose.txt");
        item.position = Point::new(40, 60);
        app.config.desktop.layer_mut().push_item(item);

        assert_eq!(
            app.surface_item_at(Surface::Desktop, Point::new(50, 70)),
            Some(ItemId(9))
        );
        assert_eq!(
            app.surface_item_at(Surface::Desktop, Point::new(500, 500)),
            None
        );
        let cell = app.surface_layout(Surface::Desktop).cell();
        let entry = app.config.desktop.layer().item(ItemId(9)).unwrap();
        assert_eq!(
            app.surface_cell(Surface::Desktop, entry),
            Rect::new(40, 60, cell.width, cell.height)
        );
    }

    #[test]
    fn only_spaces_with_items_ask_before_deleting() {
        let mut space = Space::new(SpaceId(1), "S", Rect::new(0, 0, 200, 200));
        assert!(!delete_needs_confirmation(&space));
        space.push_item(deskspace_core::Item::new(ItemId(1), "C:/a.txt"));
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
        desktop.add_space("A", Rect::new(0, 0, 10, 10));
        assert_eq!(
            tray_tooltip(&desktop),
            "DeskSpace \u{2014} 1 space, 0 items"
        );
    }
}
