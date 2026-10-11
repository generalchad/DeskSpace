use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use deskspace_config::{Config, ConfigStore};
use deskspace_core::geometry::{Point, Size};
use deskspace_core::theme::ICON_SIZE_RANGE;
use deskspace_core::{
    Color, Command, Desktop, ItemId, Palette, Rect, ResizeEdges, SortKey, Space, SpaceId,
    SpaceLook, Theme,
};
use deskspace_platform::{
    CheckBoxMode, ClipboardAction, CursorShape, DesktopEntry, DragImage, Event, EventHandler,
    FieldValue, Hotkey, IconCache, ItemMenuChoice, MenuEntry, MouseButton, SelectionKey,
    SettingsEvent, ShellDisplay, SpaceEvent, TextEdit, TrayEvent, Ui, desktop_directory,
    desktop_display, drag_threshold, hotkey_id, notify_moved, parse_hotkey, system_accent_color,
    system_uses_dark_mode, virtual_desktop_rect, work_area_at,
};

use crate::scene::{desktop_layer_drag_scene, drag_scene};
use crate::settings::{self, Effect, Session, SpaceSnapshot};
use crate::storage::ItemStore;

use self::selection::marquee_selection;

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

const TOGGLE_HOTKEY: i32 = 0;
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

enum Drag {
    None,
    Space {
        id: SpaceId,
        grab: Point,
    },
    Resize {
        id: SpaceId,
        edges: ResizeEdges,
    },
    /// The left button went down on an item. Past the drag threshold it
    /// rearranges the item within its surface; once the pointer leaves it the
    /// drag becomes a shell drag, as dragging an icon out of a folder.
    Item {
        surface: Surface,
        item: ItemId,
        origin: Point,
        active: bool,
    },
    /// Rubber-band selection over empty content.
    Marquee {
        surface: Surface,
        origin: Point,
        /// Ctrl/Shift: add to the selection instead of replacing it.
        additive: bool,
    },
}

/// A left press on an already-selected item without Ctrl/Shift, resolved on
/// release if it did not become a drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingClick {
    /// The item was part of a multi-selection: collapse the selection to it,
    /// so dragging one member moves the whole group but a plain click selects
    /// just this item.
    Collapse(ItemId),
    /// The item was already the only selected one: a second click starts an
    /// inline rename, as a slow double-click does in Explorer.
    Rename(ItemId),
}

/// Classifies a plain (no Ctrl/Shift) press on an already-selected item.
fn pending_click(selected: &[ItemId], item: ItemId) -> PendingClick {
    if selected == [item] {
        PendingClick::Rename(item)
    } else {
        PendingClick::Collapse(item)
    }
}

/// What releasing such a press does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingClickOutcome {
    Rename,
    Collapse,
    Nothing,
}

fn resolve_pending_click(
    pending: Option<PendingClick>,
    item: ItemId,
    still_selected: bool,
) -> PendingClickOutcome {
    match pending {
        Some(PendingClick::Rename(pending_item)) if pending_item == item && still_selected => {
            PendingClickOutcome::Rename
        }
        Some(PendingClick::Collapse(pending_item)) if pending_item == item && still_selected => {
            PendingClickOutcome::Collapse
        }
        _ => PendingClickOutcome::Nothing,
    }
}

/// What an inline edit is renaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditTarget {
    /// The space's title.
    Title,
    /// An item's file name. `shows_extension` is true when the edited label
    /// includes the file extension, so the typed text is the whole name.
    Item { id: ItemId, shows_extension: bool },
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

/// An in-progress inline rename. The platform captures keys while it lives.
struct EditSession {
    surface: Surface,
    target: EditTarget,
    original: String,
    text: TextEdit,
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

fn cursor_for(edges: ResizeEdges) -> CursorShape {
    match (edges.left, edges.top, edges.right, edges.bottom) {
        (true, true, _, _) | (_, _, true, true) => CursorShape::SizeNorthWestSouthEast,
        (true, _, _, true) | (_, true, true, _) => CursorShape::SizeNorthEastSouthWest,
        (true, ..) | (_, _, true, _) => CursorShape::SizeWestEast,
        (_, true, ..) | (.., true) => CursorShape::SizeNorthSouth,
        _ => CursorShape::Arrow,
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
    selected_surface: Option<Surface>,
    /// Items selected in `selected_surface`, in the order they were selected.
    selected_items: Vec<ItemId>,
    /// The item a Shift+click or Shift+arrow extends the selection from.
    selection_anchor: Option<ItemId>,
    /// The item with keyboard focus, drawn with a focus rectangle. Follows the
    /// selection, but Ctrl+arrow moves it on its own.
    focused: Option<ItemId>,
    /// A left press on an already-selected item without Ctrl/Shift, resolved
    /// on release if the press did not become a drag.
    pending_click: Option<PendingClick>,
    /// Items on the clipboard as a cut, shown ghosted until pasted.
    cut_items: HashSet<ItemId>,
    /// Ticked items in independent item-check-box mode. Session-only.
    checked_items: Vec<ItemId>,
    hovered: Option<(Surface, ItemId)>,
    drag: Drag,
    /// The items whose slots are drawn faded while they are dragged.
    dragged: Option<(Surface, Vec<ItemId>)>,
    /// The rubber-band selection rectangle in desktop coordinates and the
    /// surface it belongs to, while the user drags over empty content.
    marquee: Option<(Surface, Rect)>,
    /// The picture under the pointer during an item drag, shown by the ghost
    /// window inside the space and by the shell once the drag leaves it.
    drag_image: Option<DragImage>,
    /// The in-progress inline rename, of a space title or an item label.
    edit: Option<EditSession>,
    /// Whether the edit caret is currently shown; toggled by `CARET_TIMER`.
    caret_visible: bool,
    /// Pending configuration write, debounced by `AUTOSAVE_TIMER`.
    autosave: Autosave,
    /// Per-space scroll offset in pixels. Not persisted; may exceed the
    /// current maximum after a resize, so read it through `scroll_of`.
    scroll: HashMap<SpaceId, i32>,
    visible: bool,
    hotkey: Option<Hotkey>,
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
            selected_surface: None,
            selected_items: Vec::new(),
            selection_anchor: None,
            focused: None,
            pending_click: None,
            cut_items: HashSet::new(),
            checked_items: Vec::new(),
            hovered: None,
            drag: Drag::None,
            dragged: None,
            marquee: None,
            drag_image: None,
            edit: None,
            caret_visible: true,
            autosave: Autosave::default(),
            scroll: HashMap::new(),
            visible: !hidden,
            hotkey: None,
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
            .selected_items
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

    /// Opens the context menu for the focused item, or for the current space
    /// when no item is selected, as the Menu key or Shift+F10 does.
    fn open_context_menu(&mut self, ui: &mut Ui) {
        let Some(surface) = self.selected_surface else {
            return;
        };
        let Some(at) = self.menu_anchor() else {
            return;
        };
        match self.selected_items.last().copied() {
            Some(item) => self.show_item_menu(ui, surface, item, at),
            None => {
                if let Surface::Space(id) = surface {
                    self.show_space_menu(ui, id, at);
                }
            }
        }
    }

    /// Where to anchor a keyboard-opened menu: the center of the focused item's
    /// cell, or the current surface's center when no item is selected.
    fn menu_anchor(&self) -> Option<Point> {
        let surface = self.selected_surface?;
        let Some(item) = self.selected_items.last().copied() else {
            return Some(self.surface_window_rect(surface)?.center());
        };
        let entry = self.surface_item(surface, item)?;
        let cell = self.surface_cell(surface, entry);
        Some(Point::new(
            cell.x + cell.width / 2,
            cell.y + cell.height / 2,
        ))
    }

    /// Opens the focused (last-selected) item, as Enter does in Explorer.
    fn open_selected_item(&mut self, ui: &mut Ui) {
        if let Some(path) = self
            .selected_items
            .last()
            .and_then(|item| self.config.desktop.item(*item))
            .map(|item| item.path.clone())
        {
            ui.open_path(&path);
        }
    }

    // ---- per-space appearance ------------------------------------------

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
        if self.dispatch(command, at) {
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
        if self.dispatch(command, at) {
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
        if let (Some(session), Some(space)) =
            (self.settings.as_mut(), self.config.desktop.space(id))
        {
            session.resnapshot(space);
            self.refresh_settings(ui);
        }
    }

    fn sort_space(&mut self, ui: &mut Ui, id: SpaceId, key: SortKey, descending: bool, at: Point) {
        self.apply_to_space(
            ui,
            id,
            Command::SortSpace {
                id,
                key,
                descending,
            },
            at,
        );
    }

    // ---- pointer -------------------------------------------------------

    fn on_space_event(&mut self, ui: &mut Ui, id: SpaceId, event: SpaceEvent) {
        // The chrome-less desktop layer reuses the space window: id 0 is it,
        // any other id a real space. A space window can outlive its space by
        // one queued event, so ignore anything they report.
        let surface = Surface::of(id);
        let exists = match surface {
            Surface::Space(id) => self.config.desktop.space(id).is_some(),
            Surface::Desktop => self.layer_active(),
        };
        if !exists {
            return;
        }
        // Clicking anywhere commits a rename in progress before the click is
        // handled, as clicking away from a title does elsewhere.
        if self.edit.is_some()
            && matches!(
                &event,
                SpaceEvent::MouseDown { .. } | SpaceEvent::DoubleClick { .. }
            )
        {
            self.finish_rename(ui, true);
        }
        // A space is part of the desktop, and never takes activation, so a
        // click here is made to behave like one on the desktop: focus it and
        // clear its icon selection.
        if matches!(&event, SpaceEvent::MouseDown { .. }) {
            self.dismiss_info_tip(ui);
            self.refresh_shell_display(ui);
            ui.on_desktop_click();
        }
        match event {
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ctrl,
                shift,
            } => self.begin_interaction(ui, surface, Point::new(x, y), ctrl, shift),
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
                ..
            } => self.select_at(ui, surface, Point::new(x, y)),
            SpaceEvent::MouseMove { x, y } => {
                let point = Point::new(x, y);
                if !self.drag_to(ui, point) {
                    if let Surface::Space(id) = surface {
                        self.update_cursor(ui, id, point);
                    }
                    self.update_hover(ui, surface, point);
                }
            }
            SpaceEvent::Wheel { x, y, delta, ctrl } => {
                if ctrl {
                    self.adjust_icon_size(ui, delta);
                } else if let Surface::Space(id) = surface {
                    self.scroll_by(ui, id, Point::new(x, y), delta);
                }
            }
            SpaceEvent::MouseUp {
                button: MouseButton::Left,
                ..
            }
            | SpaceEvent::CaptureLost => self.end_drag(ui),
            SpaceEvent::MouseUp {
                x,
                y,
                button: MouseButton::Right,
            } => {
                let point = Point::new(x, y);
                match self.surface_item_at(surface, point) {
                    Some(item) => self.show_item_menu(ui, surface, item, point),
                    None => {
                        if let Surface::Space(id) = surface {
                            self.show_space_menu(ui, id, point);
                        }
                    }
                }
            }
            SpaceEvent::DoubleClick { x, y } => self.double_click(ui, surface, Point::new(x, y)),
            SpaceEvent::MouseLeave => {
                self.dismiss_info_tip(ui);
                if self.hovered.is_some_and(|(owner, _)| owner == surface) {
                    self.hovered = None;
                    self.refresh_surface(ui, surface);
                }
            }
            SpaceEvent::FilesDropped {
                x,
                y,
                paths,
                keep_in_place,
            } => {
                // Only a space registers an OLE drop target, so `id` is a
                // space here.
                if let Surface::Space(space) = surface {
                    self.add_dropped(ui, space, Point::new(x, y), paths, keep_in_place);
                }
            }
            SpaceEvent::DpiChanged { .. } => {
                if let Surface::Space(id) = surface {
                    self.on_dpi_changed(ui, id);
                }
            }
            _ => {}
        }
    }

    fn begin_interaction(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        point: Point,
        ctrl: bool,
        shift: bool,
    ) {
        // A click on an item's check box either toggles that item in the
        // selection (auto-check-select) or the independent tick set, and moves
        // the highlight to it.
        if let Some(item) = self.checkbox_at(surface, point) {
            let previous = self.selected_surface.replace(surface);
            if self.surface_check_box_mode(surface) == CheckBoxMode::Independent {
                if let Some(index) = self.checked_items.iter().position(|entry| *entry == item) {
                    self.checked_items.remove(index);
                } else {
                    self.checked_items.push(item);
                }
                self.selected_items = vec![item];
            } else {
                self.toggle_selected(item);
            }
            self.selection_anchor = Some(item);
            self.focused = Some(item);
            if let Some(previous) = previous.filter(|previous| *previous != surface) {
                self.refresh_surface(ui, previous);
            }
            self.refresh_surface(ui, surface);
            return;
        }
        let previous = self.selected_surface.replace(surface);
        self.pending_click = None;
        let item = self.surface_item_at(surface, point);
        match item {
            Some(item) => {
                if shift {
                    self.extend_selection(surface, item);
                } else if ctrl {
                    self.toggle_selected(item);
                } else if self.is_selected(item) {
                    // Keep the whole selection so a drag of one member moves
                    // the group. On release, collapse it and maybe rename.
                    self.pending_click = Some(pending_click(&self.selected_items, item));
                } else {
                    self.selected_items = vec![item];
                }
                self.selection_anchor = Some(item);
                self.focused = Some(item);
            }
            None => {
                if !ctrl && !shift {
                    self.selected_items.clear();
                    self.focused = None;
                }
            }
        }
        match surface {
            Surface::Space(id) => {
                let Some(space) = self.config.desktop.space(id) else {
                    return;
                };
                let edges = space.resize_edges_at(point, RESIZE_BORDER);
                if edges.any() {
                    self.drag = Drag::Resize { id, edges };
                    self.selected_items.clear();
                } else if let Some(item) = item {
                    self.drag = Drag::Item {
                        surface,
                        item,
                        origin: point,
                        active: false,
                    };
                } else if !space.locked && space.title_bar().contains(point) {
                    self.drag = Drag::Space {
                        id,
                        grab: Point::new(point.x - space.rect.x, point.y - space.rect.y),
                    };
                } else if space.content_rect().contains(point) {
                    // Empty content: start a rubber-band selection.
                    self.drag = Drag::Marquee {
                        surface,
                        origin: point,
                        additive: ctrl || shift,
                    };
                }
            }
            Surface::Desktop => {
                self.drag = match item {
                    Some(item) => Drag::Item {
                        surface,
                        item,
                        origin: point,
                        active: false,
                    },
                    None => Drag::Marquee {
                        surface,
                        origin: point,
                        additive: ctrl || shift,
                    },
                };
            }
        }
        if let Some(previous) = previous.filter(|previous| *previous != surface) {
            self.refresh_surface(ui, previous);
        }
        self.refresh_surface(ui, surface);
    }

    /// Selects the item under `point` (or nothing) without starting a drag,
    /// as a right-click does before its menu opens. Right-clicking an item
    /// that is already part of a multi-selection keeps the selection.
    fn select_at(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let previous = self.selected_surface.replace(surface);
        let item = self.surface_item_at(surface, point);
        match item {
            Some(item) if !self.is_selected(item) => {
                self.selected_items = vec![item];
                self.selection_anchor = Some(item);
                self.focused = Some(item);
            }
            Some(item) => self.focused = Some(item),
            None => {
                self.selected_items.clear();
                self.focused = None;
            }
        }
        if let Some(previous) = previous.filter(|previous| *previous != surface) {
            self.refresh_surface(ui, previous);
        }
        self.refresh_surface(ui, surface);
    }

    fn drag_to(&mut self, ui: &mut Ui, point: Point) -> bool {
        match self.drag {
            Drag::None => false,
            Drag::Item {
                surface,
                item,
                origin,
                active,
            } => {
                if !active {
                    let threshold = drag_threshold();
                    if (point.x - origin.x).abs() < threshold.width
                        && (point.y - origin.y).abs() < threshold.height
                    {
                        return true;
                    }
                    self.drag = Drag::Item {
                        surface,
                        item,
                        origin,
                        active: true,
                    };
                    self.begin_drag_visual(ui, surface, item, origin, point);
                }
                // Still over the surface: rearrange (a space) or reposition (a
                // loose item). Left it: hand off to the shell, which moves the
                // file to another space or app.
                if self.surface_contains(surface, point) {
                    ui.move_drag_ghost(point);
                    match surface {
                        Surface::Space(id) => self.reorder_item_at(ui, id, item, point),
                        Surface::Desktop => self.move_desktop_item(ui, item, origin, point),
                    }
                } else {
                    self.drag = Drag::None;
                    // The shell draws the same image from here on.
                    ui.hide_drag_ghost();
                    self.drag_item_out(ui, surface, item);
                    self.end_drag_visual(ui);
                }
                true
            }
            Drag::Marquee {
                surface,
                origin,
                additive,
            } => {
                self.update_marquee(ui, surface, origin, additive, point);
                true
            }
            Drag::Space { id, grab } => {
                let Some(space) = self.config.desktop.space(id) else {
                    self.drag = Drag::None;
                    return false;
                };
                let rect = Rect::new(
                    point.x - grab.x,
                    point.y - grab.y,
                    space.rect.width,
                    space.rect.height,
                );
                if self.dispatch(Command::SetSpaceRect { id, rect }, point) {
                    if let Some(space) = self.config.desktop.space(id) {
                        ui.set_space_rect(id.0, space.visible_rect());
                    }
                }
                true
            }
            Drag::Resize { id, edges } => {
                let before = self.config.desktop.space(id).map(|space| space.rect);
                let command = Command::ResizeSpace {
                    id,
                    edges,
                    to: point,
                };
                if self.dispatch(command, point)
                    && self.config.desktop.space(id).map(|space| space.rect) != before
                {
                    self.refresh_space(ui, id);
                }
                true
            }
        }
    }

    /// Repositions a loose desktop item as it is dragged, so it follows the
    /// pointer at its absolute virtual-desktop position.
    fn move_desktop_item(&mut self, ui: &mut Ui, item: ItemId, origin: Point, point: Point) {
        let Some(entry) = self.config.desktop.layer().item(item) else {
            return;
        };
        let start = entry.position;
        let position = Point::new(start.x + point.x - origin.x, start.y + point.y - origin.y);
        if self.dispatch(Command::SetDesktopItemPosition { item, position }, point) {
            self.refresh_desktop_layer(ui);
        }
    }

    fn update_cursor(&self, ui: &mut Ui, id: SpaceId, point: Point) {
        let edges = self
            .config
            .desktop
            .space(id)
            .map_or(ResizeEdges::NONE, |space| {
                space.resize_edges_at(point, RESIZE_BORDER)
            });
        ui.set_space_cursor(id.0, cursor_for(edges));
    }

    /// Ctrl+wheel changes the global icon size, as it does on the desktop:
    /// spaces that follow Global resize together, while a customized space
    /// keeps its own size. Persisted through the settings, not the config's
    /// base grid.
    fn adjust_icon_size(&mut self, ui: &mut Ui, delta: i32) {
        let step = delta * ICON_SIZE_WHEEL_STEP / WHEEL_DELTA;
        if step == 0 {
            return;
        }
        let current = self
            .config
            .theme
            .icon_size
            .clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as i32;
        let next = (current + step).clamp(ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into());
        if next as u8 == self.config.theme.icon_size {
            return;
        }
        self.config.theme.icon_size = next as u8;
        self.relayout_all(ui);
        self.schedule_save(ui);
        self.refresh_settings(ui);
    }

    fn scroll_by(&mut self, ui: &mut Ui, id: SpaceId, point: Point, delta: i32) {
        let layout = self.layout_for(id);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let max = space.max_scroll(layout.cell(), layout.padding);
        let current = self.scroll_of(id);
        let next = (current - delta * layout.scroll_step() / WHEEL_DELTA).clamp(0, max);
        if next == current {
            return;
        }
        self.scroll.insert(id, next);
        // Items moved under a still cursor, so the hovered one changed too.
        let surface = Surface::Space(id);
        self.hovered = self
            .surface_item_at(surface, point)
            .map(|item| (surface, item));
        self.refresh_space(ui, id);
    }

    fn end_drag(&mut self, ui: &mut Ui) {
        let pending = self.pending_click.take();
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::None => {}
            Drag::Item {
                surface,
                item,
                active: false,
                ..
            } => {
                // A press on an already-selected item that never became a drag
                // is a plain click: collapse the selection, or start a rename
                // when it was already the only selected item. Only a space
                // renames inline.
                match resolve_pending_click(pending, item, self.is_selected(item)) {
                    PendingClickOutcome::Rename => {
                        if let Surface::Space(id) = surface {
                            self.begin_item_rename(ui, id, item);
                        }
                    }
                    PendingClickOutcome::Collapse => {
                        self.selected_items = vec![item];
                        self.selection_anchor = Some(item);
                        self.focused = Some(item);
                        self.refresh_surface(ui, surface);
                    }
                    PendingClickOutcome::Nothing => {}
                }
            }
            Drag::Item { active: true, .. } => {
                self.end_drag_visual(ui);
                self.schedule_save(ui);
            }
            Drag::Marquee { surface, .. } => {
                // The selection already updated live; just drop the rectangle.
                self.marquee = None;
                self.refresh_surface(ui, surface);
            }
            Drag::Space { .. } | Drag::Resize { .. } => self.schedule_save(ui),
        }
    }

    /// Fades the dragged items' slots and floats their group drag image under
    /// the pointer, as Explorer does once an icon drag starts. `origin` is
    /// where the button went down, so the image keeps the spot that was
    /// grabbed.
    fn begin_drag_visual(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        item: ItemId,
        origin: Point,
        point: Point,
    ) {
        // The whole active set drags, as in Explorer, so the image is that set
        // rendered as it is now, before the slots fade.
        let ids = self.drag_items(item);
        let id_values: Vec<u64> = ids.iter().map(|id| id.0).collect();
        let view = self.view_for(surface);
        let layout = self.surface_layout(surface);
        let metrics = self.surface_metrics(surface);
        let palette = self.surface_palette(surface);
        let grab = self
            .surface_window_rect(surface)
            .map(|rect| Point::new(origin.x - rect.x, origin.y - rect.y));
        self.drag_image = match surface {
            Surface::Space(id) => self.config.desktop.space(id).and_then(|entry| {
                let scene = drag_scene(
                    entry,
                    &ids,
                    entry.theme(&self.config.theme),
                    &palette,
                    layout,
                    metrics,
                    view,
                    &mut self.icons,
                );
                DragImage::of_selection(&scene, &id_values, grab?)
            }),
            Surface::Desktop => {
                let scene = desktop_layer_drag_scene(
                    self.config.desktop.layer(),
                    virtual_desktop_rect().origin(),
                    &ids,
                    &self.config.theme,
                    &palette,
                    layout,
                    metrics,
                    view,
                    &mut self.icons,
                );
                grab.and_then(|grab| DragImage::of_selection(&scene, &id_values, grab))
            }
        };
        self.dragged = Some((surface, ids));
        self.pending_click = None;
        if let Some(image) = &self.drag_image {
            ui.show_drag_ghost(image, point);
        }
        self.refresh_surface(ui, surface);
    }

    fn end_drag_visual(&mut self, ui: &mut Ui) {
        ui.hide_drag_ghost();
        self.drag_image = None;
        if let Some((surface, _)) = self.dragged.take() {
            self.refresh_surface(ui, surface);
        }
    }

    fn update_hover(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let hovered = self
            .surface_item_at(surface, point)
            .map(|item| (surface, item));
        if hovered == self.hovered {
            return;
        }
        let previous = std::mem::replace(&mut self.hovered, hovered);
        // A different item (or none) cancels any shown tip and its delay.
        ui.hide_info_tip();
        ui.clear_timer(INFO_TIP_TIMER);
        if hovered.is_some() {
            ui.set_timer(INFO_TIP_TIMER, INFO_TIP_DELAY_MS);
        }
        if let Some((owner, _)) = previous.filter(|(owner, _)| *owner != surface) {
            self.refresh_surface(ui, owner);
        }
        self.refresh_surface(ui, surface);
    }

    fn toggle_rollup(&mut self, ui: &mut Ui, id: SpaceId) {
        let near = self.space_center(id);
        // A rolled-up space hides its items, so it cannot keep one selected.
        if self.selected_surface == Some(Surface::Space(id)) {
            self.take_selection();
        }
        self.apply_to_space(ui, id, Command::ToggleRollup { id }, near);
    }

    /// Adds dropped files to `target`. New files are moved into the space
    /// storage (ROADMAP D3) unless Shift was held, or they cannot be moved
    /// without copying; then the space refers to them where they are. A path
    /// already shown in another space moves to `target` instead of appearing
    /// twice; that never touches the file.
    fn add_dropped(
        &mut self,
        ui: &mut Ui,
        target: SpaceId,
        at: Point,
        paths: Vec<PathBuf>,
        keep_in_place: bool,
    ) {
        let mut touched = HashSet::from([target]);
        for path in paths {
            // A drop can name a file that no longer exists (for example a
            // stale desktop icon). Listing it would show a label without an
            // icon, so it is skipped.
            if !path.exists() {
                tracing::warn!(path = %path.display(), "dropped file does not exist; ignored");
                continue;
            }
            let command = match self.find_item_by_file(&path) {
                Some(item) => {
                    match self.config.desktop.space_containing(item) {
                        Some(source) if source == target => continue,
                        Some(source) => {
                            touched.insert(source);
                        }
                        None => {}
                    }
                    Command::MoveItem { id: item, target }
                }
                None => match self.place_dropped(path, keep_in_place) {
                    Some(path) => Command::AddItem {
                        space: target,
                        path,
                    },
                    None => continue,
                },
            };
            self.dispatch(command, at);
        }
        for id in &touched {
            self.refresh_space(ui, *id);
        }
        // A loose desktop item that was dropped in has moved off the desktop,
        // so reconcile the layer now instead of at the next shell refresh.
        if self.layer_active() {
            self.resync_desktop_layer(ui);
        }
        self.refresh_tray(ui);
        self.save();
    }

    /// Re-reads the shell's loose desktop items and reconciles the layer. Used
    /// after an operation that moved or renamed a loose file, so the layer
    /// follows without waiting for the next shell change.
    fn resync_desktop_layer(&mut self, ui: &mut Ui) {
        if !self.layer_active() {
            return;
        }
        self.refresh_desktop_entries();
        self.sync_desktop_layer(ui);
    }

    /// Where a newly dropped file will live: in the storage folder, or where
    /// it already is. `None` if it vanished before it could be moved.
    fn place_dropped(&self, path: PathBuf, keep_in_place: bool) -> Option<PathBuf> {
        if keep_in_place {
            return Some(path);
        }
        match self.storage.adopt(&path) {
            Ok(stored) => {
                if stored != path {
                    tracing::info!(from = %path.display(), to = %stored.display(), "moved into space storage");
                    notify_moved(&path, &stored);
                }
                Some(stored)
            }
            Err(error) if !path.exists() => {
                tracing::warn!(%error, "dropped file disappeared; ignored");
                None
            }
            Err(error) => {
                tracing::info!(%error, "keeping dropped file in place");
                Some(path)
            }
        }
    }

    /// Takes an item out of its space. A file that was moved into the space
    /// storage goes back to the desktop first, so nothing is left behind in
    /// a folder the user never sees. Returns false if that move failed; the
    /// item then stays.
    fn release_item(&mut self, id: ItemId) -> bool {
        let Some(path) = self.config.desktop.item(id).map(|item| item.path.clone()) else {
            return false;
        };
        if self.storage.contains(&path) && path.exists() {
            let desktop = match desktop_directory() {
                Ok(desktop) => desktop,
                Err(error) => {
                    tracing::error!(%error, "cannot find the desktop folder");
                    return false;
                }
            };
            match self.storage.release(&path, &desktop) {
                Ok(back) => {
                    tracing::info!(to = %back.display(), "moved back to the desktop");
                    notify_moved(&path, &back);
                }
                Err(error) => {
                    tracing::error!(%error, "could not move the item back to the desktop");
                    return false;
                }
            }
        }
        let near = self
            .config
            .desktop
            .space_containing(id)
            .map_or(Point::ZERO, |space| self.space_center(space));
        self.dispatch(Command::RemoveItem { id }, near)
    }

    /// Deletes every selected item's file with the shell, then drops the items
    /// whose files are gone. Unless `permanent` is set they go to the Recycle
    /// Bin, as Explorer's Delete key does; `permanent` is Shift+Delete. A file
    /// the shell could not delete (for example one in use) stays listed. An item
    /// whose file is already missing is dropped without a shell operation. This
    /// is distinct from the item menu's "Remove from space", which returns a
    /// stored file to the desktop.
    fn delete_selected_items(&mut self, ui: &mut Ui, permanent: bool) {
        // Deleting a loose desktop item would need a way to drop it from the
        // layer, so only a space's items are deleted here.
        let Some(Surface::Space(space)) = self.selected_surface else {
            return;
        };
        let items = self.active_items();
        if items.is_empty() {
            return;
        }
        self.selected_items.clear();
        self.selection_anchor = None;
        self.focused = None;
        self.hovered = None;
        let paths: Vec<PathBuf> = items
            .iter()
            .filter_map(|item| self.config.desktop.item(*item))
            .filter(|item| item.path.exists())
            .map(|item| item.path.clone())
            .collect();
        if !paths.is_empty() {
            ui.delete_files(&paths, permanent);
        }
        // Drop every item whose file is gone, whether the shell removed it or
        // it was already stale, so no label without an icon is left behind.
        let near = self.space_center(space);
        for item in items {
            let gone = self
                .config
                .desktop
                .item(item)
                .is_none_or(|item| !item.path.exists());
            if gone {
                self.forget_item(item);
                self.dispatch(Command::RemoveItem { id: item }, near);
            }
        }
        self.refresh_space(ui, space);
        self.refresh_tray(ui);
        self.save();
    }

    /// Forgets an item's transient per-item state (cut ghosting and any
    /// independent check) after it is removed or its file is gone.
    fn forget_item(&mut self, item: ItemId) {
        self.cut_items.remove(&item);
        self.checked_items.retain(|checked| *checked != item);
    }

    /// Drops items whose files no longer exist in every space. Called once at
    /// startup so leftovers from a move made while DeskSpace was closed go away.
    fn prune_all_missing(&mut self, ui: &mut Ui) {
        let ids: Vec<SpaceId> = self
            .config
            .desktop
            .spaces
            .iter()
            .map(|space| space.id)
            .collect();
        for id in ids {
            self.prune_missing(ui, id);
        }
    }

    /// Removes items whose files no longer exist, for example after a shell
    /// command deleted one or a drag moved it elsewhere.
    fn prune_missing(&mut self, ui: &mut Ui, id: SpaceId) {
        let missing: Vec<ItemId> = self
            .config
            .desktop
            .space(id)
            .map(|space| {
                space
                    .items
                    .iter()
                    .filter(|item| !item.path.exists())
                    .map(|item| item.id)
                    .collect()
            })
            .unwrap_or_default();
        if missing.is_empty() {
            return;
        }
        for item in missing {
            let near = self.space_center(id);
            self.dispatch(Command::RemoveItem { id: item }, near);
            self.selected_items.retain(|selected| *selected != item);
            self.forget_item(item);
            if self.hovered.is_some_and(|(_, hovered)| hovered == item) {
                self.hovered = None;
            }
        }
        self.refresh_space(ui, id);
        self.refresh_tray(ui);
        self.save();
    }

    fn double_click(&mut self, ui: &mut Ui, surface: Surface, point: Point) {
        let layout = self.surface_layout(surface);
        let target = match surface {
            Surface::Space(id) => {
                self.config
                    .desktop
                    .space(id)
                    .map_or(DoubleClickTarget::Nothing, |space| {
                        double_click_target(
                            space,
                            layout.cell(),
                            self.surface_scroll(surface),
                            point,
                        )
                    })
            }
            Surface::Desktop => self
                .surface_item_at(surface, point)
                .map_or(DoubleClickTarget::Nothing, DoubleClickTarget::Item),
        };
        tracing::debug!(?surface, ?point, ?target, "double-click");
        match target {
            DoubleClickTarget::TitleBar => {
                if let Surface::Space(id) = surface {
                    self.toggle_rollup(ui, id);
                }
            }
            DoubleClickTarget::Item(item) => {
                if let Some(path) = self.config.desktop.item(item).map(|item| item.path.clone()) {
                    ui.open_path(&path);
                }
            }
            DoubleClickTarget::Nothing => {}
        }
    }

    /// Places the dragged item in the grid slot under `point`, reflowing the
    /// space. Used while an item is dragged within its own space.
    fn reorder_item_at(&mut self, ui: &mut Ui, space: SpaceId, item: ItemId, point: Point) {
        let layout = self.layout_for(space);
        let moving = self.drag_items(item);
        let target = self.config.desktop.space(space).and_then(|space| {
            let content = space.content_rect();
            let columns = if space.user_sized {
                layout.columns_for(space.rect.width)
            } else {
                layout.columns
            }
            .max(1);
            let step_x = layout.cell_width + layout.gap;
            let step_y = layout.cell_height + layout.gap;
            let column = ((point.x - content.x - layout.padding) / step_x).clamp(0, columns - 1);
            let row = ((point.y - content.y - layout.padding) / step_y).max(0);
            let count = space.items.len();
            if count == 0 {
                return None;
            }
            let index = ((row * columns + column) as usize).min(count - 1);
            let current = space.items.iter().position(|entry| entry.id == item)?;
            Some((index, current))
        });
        let Some((index, current)) = target else {
            return;
        };
        if index == current {
            return;
        }
        let command = if moving.len() > 1 {
            Command::ReorderItems { ids: moving, index }
        } else {
            Command::ReorderItem { id: item, index }
        };
        if self.dispatch(command, point) {
            self.refresh_space(ui, space);
        }
    }

    /// Drags an item out like Explorer. Wherever it lands, the drop target
    /// alone decides what happens to the file, and has finished by the time
    /// the drag returns. The item leaves the space only if its file is gone
    /// (moved elsewhere); after a copy, link, or cancel it stays, as in
    /// Explorer. Drops on spaces arrive as `FilesDropped` after this returns
    /// and move the item between spaces.
    ///
    /// DeskSpace must never move the file itself here. It used to move a
    /// file dropped on the desktop back out of storage even when the desktop
    /// had copied it, which left `- Copy` and ` (2)` duplicates behind.
    fn drag_item_out(&mut self, ui: &mut Ui, surface: Surface, item: ItemId) {
        // Dragging an item that is part of the active set drags the whole set,
        // as Explorer does; otherwise just that one item.
        let ids = self.drag_items(item);
        let (paths, positions): (Vec<PathBuf>, Vec<Point>) = self
            .item_cells(surface, &ids)
            .into_iter()
            .filter(|(path, _)| path.exists())
            .unzip();
        // A stale item (its file moved or deleted since it was drawn) is
        // dropped instead of repeatedly failing to start a drag.
        if paths.is_empty() {
            match surface {
                Surface::Space(id) => self.prune_missing(ui, id),
                Surface::Desktop => self.resync_desktop_layer(ui),
            }
            return;
        }
        let Some(result) = ui.drag_out(&paths, &positions, self.drag_image.as_ref()) else {
            return;
        };
        tracing::debug!(?result, files = paths.len(), "item drag ended");
        // The drop target may have moved some files out; drop any item whose
        // file is gone, wherever it lives, and reconcile the loose layer.
        self.prune_all_missing(ui);
        self.resync_desktop_layer(ui);
    }

    /// Updates the rubber-band rectangle and the live selection as the pointer
    /// moves over empty content.
    fn update_marquee(
        &mut self,
        ui: &mut Ui,
        surface: Surface,
        origin: Point,
        additive: bool,
        point: Point,
    ) {
        let Some(content) = self.surface_content(surface) else {
            return;
        };
        let cells = self.surface_cells(surface);
        let left = origin.x.min(point.x).max(content.x);
        let top = origin.y.min(point.y).max(content.y);
        let right = origin.x.max(point.x).min(content.right());
        let bottom = origin.y.max(point.y).min(content.bottom());
        let rect = Rect::new(left, top, (right - left).max(0), (bottom - top).max(0));
        self.marquee = Some((surface, rect));

        self.selected_surface = Some(surface);
        self.selected_items = marquee_selection(&cells, rect, additive, &self.selected_items);
        self.selection_anchor = None;
        self.focused = self.selected_items.last().copied();
        self.refresh_surface(ui, surface);
    }

    fn show_item_menu(&mut self, ui: &mut Ui, surface: Surface, item: ItemId, at: Point) {
        let Some(path) = self.config.desktop.item(item).map(|item| item.path.clone()) else {
            return;
        };
        // A loose desktop item has no space to remove it from, so only
        // Explorer's own menu is offered for it.
        let entries = match surface {
            Surface::Space(_) => {
                let remove_label = if self.storage.contains(&path) {
                    "Move back to desktop"
                } else {
                    "Remove from space"
                };
                vec![
                    MenuEntry::item(menu::RENAME_ITEM, "Rename"),
                    MenuEntry::item(menu::REMOVE_ITEM, remove_label),
                ]
            }
            Surface::Desktop => Vec::new(),
        };
        match ui.item_menu(&path, &entries, at) {
            Some(ItemMenuChoice::App(menu::RENAME_ITEM)) => {
                if let Surface::Space(id) = surface {
                    self.begin_item_rename(ui, id, item);
                }
            }
            Some(ItemMenuChoice::App(menu::REMOVE_ITEM)) => {
                if self.release_item(item) {
                    self.selected_items.retain(|selected| *selected != item);
                    self.forget_item(item);
                    self.hovered = None;
                    self.refresh_surface(ui, surface);
                    self.refresh_tray(ui);
                    self.save();
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

    fn delete_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let stored = space
            .items
            .iter()
            .filter(|item| self.storage.contains(&item.path))
            .count();
        if delete_needs_confirmation(space) {
            let question = format!(
                "Delete the space \"{}\"?\n\n{}",
                space.title,
                delete_consequence(space.items.len(), stored)
            );
            if !ui.confirm("Delete space", &question) {
                return;
            }
        }
        // Return stored files to the desktop before the space goes; if any
        // cannot be moved, keep the space so its files stay reachable.
        let items: Vec<ItemId> = space.items.iter().map(|item| item.id).collect();
        let kept = items
            .into_iter()
            .filter(|item| !self.release_item(*item))
            .count();
        if kept > 0 {
            tracing::error!(
                kept,
                "space kept because some files could not be moved back"
            );
            self.refresh_space(ui, id);
            self.save();
            return;
        }
        let near = self.space_center(id);
        if self.dispatch(Command::DeleteSpace { id }, near) {
            if self.selected_surface == Some(Surface::Space(id)) {
                self.selected_surface = None;
                self.selected_items.clear();
                self.selection_anchor = None;
                self.focused = None;
            }
            if self
                .hovered
                .is_some_and(|(owner, _)| owner == Surface::Space(id))
            {
                self.hovered = None;
            }
            self.scroll.remove(&id);
            self.sync_windows(ui);
            self.save();
        }
    }
    /// Deletes the space shown in the settings window, from the header's
    /// "Delete space" button. Deletion is immediate and Cancel does not bring
    /// the space back, matching the space menu. The window falls back to
    /// Global when the scoped space is gone.
    fn delete_space_from_settings(&mut self, ui: &mut Ui, id: SpaceId) {
        self.delete_space(ui, id);
        if self.config.desktop.space(id).is_some() {
            // Not confirmed, or kept because a stored file could not be moved
            // back to the desktop.
            self.refresh_settings(ui);
            return;
        }
        if let Some(session) = self.settings.as_mut() {
            session.forget_space(id);
        }
        self.clear_highlight(ui);
        self.refresh_settings(ui);
    }

    // ---- menus ---------------------------------------------------------

    fn show_space_menu(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let roll = if space.rolled_up {
            "Roll down"
        } else {
            "Roll up"
        };
        let mut entries = vec![
            MenuEntry::item(menu::ROLL, roll),
            MenuEntry::item(menu::RENAME, "Rename"),
            MenuEntry::item(menu::LOCK, "Lock position").checked(space.locked),
        ];
        if space.user_sized && !space.locked {
            entries.push(MenuEntry::item(menu::FIT, "Fit to contents"));
        }
        entries.push(MenuEntry::Separator);
        entries.push(space_appearance_menu(
            space,
            space.theme(&self.config.theme),
        ));
        entries.push(space_sort_menu());
        entries.push(MenuEntry::Separator);
        // The ellipsis promises a dialog, which only non-empty spaces show.
        let delete = if delete_needs_confirmation(space) {
            "Delete space\u{2026}"
        } else {
            "Delete space"
        };
        entries.extend([
            MenuEntry::item(menu::DELETE, delete),
            MenuEntry::Separator,
            MenuEntry::item(menu::NEW_SPACE, "New space"),
            MenuEntry::Separator,
            MenuEntry::item(menu::SETTINGS, "Settings\u{2026}"),
            self.visibility_entry(),
            MenuEntry::item(menu::EXIT, "Exit DeskSpace"),
        ]);
        match ui.popup_menu(&entries, at) {
            Some(menu::ROLL) => self.toggle_rollup(ui, id),
            Some(menu::RENAME) => self.begin_rename(ui, id),
            Some(menu::DELETE) => self.delete_space(ui, id),
            Some(menu::FIT) => {
                if self.apply_to_space(ui, id, Command::FitToContents { id }, at) {
                    self.scroll.remove(&id);
                }
            }
            Some(menu::LOCK) => {
                self.apply_to_space(ui, id, Command::ToggleLock { id }, at);
            }
            Some(menu::SPACE_TINT) => self.pick_space_color(ui, id, at, SpaceColor::Tint),
            Some(menu::SPACE_TEXT) => self.pick_space_color(ui, id, at, SpaceColor::Text),
            Some(menu::RESET_APPEARANCE) => self.use_global_appearance(ui, id, at),
            Some(menu::SPACE_SETTINGS) => self.open_settings_with(ui, Some(id)),
            Some(choice) => {
                if let Some((_, _, percent)) = OPACITY_PRESETS
                    .iter()
                    .find(|(preset, ..)| *preset == choice)
                {
                    self.set_space_tint_opacity(ui, id, *percent, at);
                } else if let Some((_, _, icon_size)) = ICON_SIZE_PRESETS
                    .iter()
                    .find(|(preset, ..)| *preset == choice)
                {
                    self.set_space_icon_size(ui, id, *icon_size, at);
                } else if let Some((_, _, key, descending)) =
                    SORT_PRESETS.iter().find(|(preset, ..)| *preset == choice)
                {
                    self.sort_space(ui, id, *key, *descending, at);
                } else {
                    self.run_common_menu(ui, choice, at);
                }
            }
            None => {}
        }
    }

    fn show_tray_menu(&mut self, ui: &mut Ui, at: Point) {
        let entries = [
            MenuEntry::item(menu::SETTINGS, "Settings\u{2026}"),
            self.visibility_entry(),
            MenuEntry::item(menu::NEW_SPACE, "New space"),
            MenuEntry::Separator,
            MenuEntry::item(menu::EXIT, "Exit DeskSpace"),
        ];
        if let Some(choice) = ui.popup_menu(&entries, at) {
            let work = work_area_at(at);
            self.run_common_menu(ui, choice, work.center());
        }
    }

    fn run_common_menu(&mut self, ui: &mut Ui, choice: u32, at: Point) {
        match choice {
            menu::NEW_SPACE => self.create_space(ui, at),
            menu::SETTINGS => self.open_settings(ui),
            menu::TOGGLE_VISIBLE => self.set_visible(ui, !self.visible),
            menu::EXIT => ui.quit(),
            _ => {}
        }
    }

    fn create_space(&mut self, ui: &mut Ui, at: Point) {
        let command = Command::CreateSpace {
            title: "New space".to_string(),
            at,
        };
        if self.dispatch(command, at) {
            if !self.visible {
                self.set_visible(ui, true);
            }
            self.sync_windows(ui);
            self.schedule_save(ui);
        }
    }

    // ---- hotkey --------------------------------------------------------

    fn register_hotkey(&mut self, ui: &mut Ui, text: &str) -> Result<(), String> {
        let spec =
            parse_hotkey(text).ok_or_else(|| format!("\"{text}\" is not a valid hotkey."))?;
        if let Some(previous) = self.hotkey.take() {
            ui.unregister_hotkey(previous);
        }
        match ui.register_hotkey(TOGGLE_HOTKEY, spec) {
            Ok(hotkey) => {
                self.hotkey = Some(hotkey);
                Ok(())
            }
            Err(error) => {
                tracing::warn!(%error, hotkey = text, "could not register toggle hotkey");
                Err(format!("{text} is already in use by another app."))
            }
        }
    }

    // ---- settings ------------------------------------------------------

    fn settings_form(&self) -> Option<deskspace_platform::SettingsForm> {
        self.settings.as_ref().map(|session| {
            settings::build_form(&self.config, session, self.system_dark, self.store.path())
        })
    }

    fn open_settings(&mut self, ui: &mut Ui) {
        self.open_settings_with(ui, None);
    }

    /// Opens the settings window, or brings it to the front, on `scope` when
    /// one is given (a space's "Space settings…"); otherwise where it was.
    fn open_settings_with(&mut self, ui: &mut Ui, scope: Option<SpaceId>) {
        let config = &self.config;
        let session = self.settings.get_or_insert_with(|| Session::new(config));
        if scope.is_some() {
            session.select_scope(scope);
        }
        if let Some(form) = self.settings_form() {
            ui.open_settings(&form);
        }
        if let Some(id) = scope {
            self.watch_highlight(ui, id);
        }
    }

    /// Highlights a space so it can be found on the desktop, and arms the
    /// timer that clears it. Selecting another space moves the highlight.
    fn watch_highlight(&mut self, ui: &mut Ui, id: SpaceId) {
        if self.highlighted != Some(id) {
            self.clear_highlight(ui);
            self.highlighted = Some(id);
            self.refresh_space(ui, id);
        }
        ui.set_timer(HIGHLIGHT_TIMER, HIGHLIGHT_MS);
    }

    /// Removes the settings-window highlight, if any.
    fn clear_highlight(&mut self, ui: &mut Ui) {
        if let Some(id) = self.highlighted.take() {
            ui.clear_timer(HIGHLIGHT_TIMER);
            if self.config.desktop.space(id).is_some() {
                self.refresh_space(ui, id);
            }
        }
    }

    fn refresh_settings(&self, ui: &mut Ui) {
        if let Some(form) = self.settings_form() {
            ui.update_settings(&form);
        }
    }

    fn on_settings_event(&mut self, ui: &mut Ui, event: SettingsEvent) {
        let Some(session) = self.settings.as_mut() else {
            return;
        };
        let editing_space = session.scope.is_some();
        match event {
            SettingsEvent::Changed { field, value } if editing_space => {
                self.on_space_field(ui, field, value);
            }
            SettingsEvent::Changed { field, value } => {
                let effect = settings::apply_field(
                    &mut self.config,
                    session,
                    field,
                    value,
                    self.system_dark,
                );
                if effect == Effect::Appearance {
                    ui.set_ui_dark(self.ui_dark());
                    self.schedule_preview(ui, Preview::restyle_all());
                } else if effect == Effect::Layout {
                    self.schedule_preview(ui, Preview::relayout_all());
                }
                self.refresh_settings(ui);
            }
            SettingsEvent::Button {
                field: settings::field::RESET,
            } if editing_space => {
                let command = settings::match_global(&self.config, session);
                self.apply_space_setting(ui, command);
            }
            SettingsEvent::Button {
                field: settings::field::RESET,
            } => {
                settings::reset_appearance(&mut self.config, session);
                ui.set_ui_dark(self.ui_dark());
                self.restyle_all(ui);
                self.refresh_settings(ui);
            }
            SettingsEvent::Button { .. } => {}
            SettingsEvent::HeaderSwitch { on } => {
                let command = settings::customize_space(&self.config, session, on);
                self.apply_space_setting(ui, command);
            }
            SettingsEvent::HeaderButton => {
                if let Some(id) = session.scope {
                    self.delete_space_from_settings(ui, id);
                }
            }
            SettingsEvent::ScopeChanged { scope } => {
                let id = settings::scope_at(&self.config, scope);
                session.select_scope(id);
                self.refresh_settings(ui);
                match id {
                    Some(id) => self.watch_highlight(ui, id),
                    None => self.clear_highlight(ui),
                }
            }
            SettingsEvent::NavChanged { entry } => {
                // The categories pane lists the pages in order, one entry each.
                session.page = entry;
                self.refresh_settings(ui);
            }
            SettingsEvent::Apply => {
                self.apply_settings(ui);
                self.refresh_settings(ui);
            }
            SettingsEvent::Ok => {
                if self.apply_settings(ui) {
                    self.settings = None;
                    self.clear_highlight(ui);
                    ui.close_settings();
                } else {
                    self.refresh_settings(ui);
                }
            }
            SettingsEvent::Cancel => {
                self.cancel_settings(ui);
                ui.close_settings();
            }
        }
    }

    /// Defers the space refresh a settings edit needs. The timer is armed once
    /// and not restarted, so the spaces keep following a slider drag while the
    /// ticks in between collapse into one pass.
    fn schedule_preview(&mut self, ui: &mut Ui, preview: Preview) {
        if !self.pending_preview.is_pending() {
            ui.set_timer(PREVIEW_TIMER, PREVIEW_DELAY_MS);
        }
        self.pending_preview.merge(preview);
    }

    /// Runs the deferred settings preview.
    fn flush_preview(&mut self, ui: &mut Ui) {
        ui.clear_timer(PREVIEW_TIMER);
        let preview = std::mem::take(&mut self.pending_preview);
        if preview.restyle_all {
            self.restyle_all(ui);
        } else {
            for id in preview.restyle {
                self.restyle_space(ui, id);
            }
        }
        if preview.relayout_all {
            self.relayout_all(ui);
        } else {
            for id in preview.relayout {
                self.reflow_space(ui, id);
            }
        }
    }

    /// An edit on a customized space's page: dispatch its `SetSpaceLook`
    /// at once, and preview it on that space alone.
    fn on_space_field(&mut self, ui: &mut Ui, field: u32, value: FieldValue) {
        let Some(session) = self.settings.as_mut() else {
            return;
        };
        let edit = settings::space_edit(&self.config, session, field, value, self.system_dark);
        if let Some((id, command, effect)) = edit
            && self.dispatch(command, self.space_center(id))
        {
            let preview = match effect {
                Effect::Layout => Preview::relayout(id),
                _ => Preview::restyle(id),
            };
            self.schedule_preview(ui, preview);
        }
        self.refresh_settings(ui);
    }

    /// Dispatches a space command from the settings window (the Customize
    /// switch, "Match global appearance") and shows its result.
    fn apply_space_setting(&mut self, ui: &mut Ui, command: Option<Command>) {
        let id = match &command {
            Some(Command::CustomizeSpace { id, .. } | Command::SetSpaceLook { id, .. }) => *id,
            _ => return self.refresh_settings(ui),
        };
        if let Some(command) = command
            && self.dispatch(command, self.space_center(id))
        {
            self.restyle_space(ui, id);
            self.reflow_space(ui, id);
        }
        self.refresh_settings(ui);
    }

    /// Commits pending settings. Returns false (and keeps the window open)
    /// when the hotkey cannot be used.
    fn apply_settings(&mut self, ui: &mut Ui) -> bool {
        let Some(text) = self
            .settings
            .as_ref()
            .map(|s| s.hotkey_text.trim().to_string())
        else {
            return false;
        };
        if text != self.config.window.toggle_hotkey || self.hotkey.is_none() {
            if let Err(message) = self.register_hotkey(ui, &text) {
                // Put the previous binding back so the hotkey keeps working.
                let previous = self.config.window.toggle_hotkey.clone();
                let _ = self.register_hotkey(ui, &previous);
                if let Some(session) = self.settings.as_mut() {
                    session.hotkey_error = Some(message);
                }
                return false;
            }
            self.config.window.toggle_hotkey = text;
        }
        let start_changed = self.settings.as_ref().is_some_and(|session| {
            session.saved_window.start_with_windows != self.config.window.start_with_windows
        });
        if start_changed {
            self.sync_start_with_windows(self.config.window.start_with_windows);
        }
        let takeover_changed = self.settings.as_ref().is_some_and(|session| {
            session.saved_window.take_over_desktop != self.config.window.take_over_desktop
        });
        if takeover_changed {
            self.apply_takeover(ui);
        }
        self.save();
        if let Some(session) = self.settings.as_mut() {
            session.hotkey_text = self.config.window.toggle_hotkey.clone();
            session.commit(&self.config);
        }
        true
    }

    fn cancel_settings(&mut self, ui: &mut Ui) {
        let Some(session) = self.settings.take() else {
            return;
        };
        // The revert below restyles and re-flows from the saved settings, which
        // covers any preview still waiting for its timer.
        if std::mem::take(&mut self.pending_preview).is_pending() {
            ui.clear_timer(PREVIEW_TIMER);
        }
        self.clear_highlight(ui);
        let restyle = settings::needs_restyle(&session, &self.config);
        // A revert re-flows when the grid changed: the layout itself, the
        // icon size that scales it, or a Match Explorer toggle, which
        // imports Explorer's own icon size and spacing.
        let relayout = settings::needs_relayout(&session, &self.config);
        self.restore_spaces(ui, &session.saved_spaces);
        session.revert(&mut self.config);
        ui.set_ui_dark(self.ui_dark());
        if restyle {
            self.restyle_all(ui);
        }
        if relayout {
            self.relayout_all(ui);
        }
    }

    /// Cancel: puts back every space whose customization changed while the
    /// window was open. Spaces deleted meanwhile are skipped.
    fn restore_spaces(&mut self, ui: &mut Ui, saved: &[SpaceSnapshot]) {
        for snapshot in saved {
            let Some(space) = self.config.desktop.space(snapshot.id) else {
                continue;
            };
            if SpaceSnapshot::of(space) == *snapshot {
                continue;
            }
            let at = space.rect.center();
            if self.dispatch(snapshot.restore(), at) {
                self.restyle_space(ui, snapshot.id);
                self.reflow_space(ui, snapshot.id);
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
                let hotkey = self.config.window.toggle_hotkey.clone();
                if let Err(message) = self.register_hotkey(ui, &hotkey) {
                    tracing::warn!(message, "toggle hotkey unavailable");
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
                if self.edit.is_some() {
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
                if let Some(hotkey) = self.hotkey.take() {
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
        ui.capture_text_input(self.edit.is_some());
        ui.watch_selection_keys(
            self.edit.is_none() && !self.selected_items.is_empty(),
            self.edit.is_none() && self.selected_surface.is_some(),
        );
        self.sync_outside_watch(ui);
    }
}

/// What a double-click at `point` (desktop coordinates) acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoubleClickTarget {
    /// Rolls the space up or down.
    TitleBar,
    /// Opens the item.
    Item(ItemId),
    Nothing,
}

fn double_click_target(space: &Space, cell: Size, scroll: i32, point: Point) -> DoubleClickTarget {
    if space.title_bar().contains(point) {
        DoubleClickTarget::TitleBar
    } else if let Some(item) = space.item_at(point, cell, scroll) {
        DoubleClickTarget::Item(item)
    } else {
        DoubleClickTarget::Nothing
    }
}

/// The "Appearance" submenu for a space drawn with `theme`: tint-opacity
/// presets, the icon size, custom tint and text colors, a way back to the
/// global appearance, and the space's settings page. Every edit customizes
/// the space.
fn space_appearance_menu(space: &Space, theme: &Theme) -> MenuEntry {
    let mut entries: Vec<MenuEntry> = OPACITY_PRESETS
        .iter()
        .map(|(id, label, value)| {
            MenuEntry::item(*id, *label).checked(theme.tint_opacity == *value)
        })
        .collect();
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::submenu(
        "Icon size",
        ICON_SIZE_PRESETS
            .iter()
            .map(|(id, label, value)| {
                MenuEntry::item(*id, *label).checked(theme.icon_size == *value)
            })
            .collect(),
    ));
    entries.push(MenuEntry::Separator);
    entries.push(
        MenuEntry::item(menu::SPACE_TINT, "Custom tint\u{2026}").checked(theme.custom_colors),
    );
    entries.push(
        MenuEntry::item(menu::SPACE_TEXT, "Custom text\u{2026}").checked(theme.custom_colors),
    );
    entries.push(
        MenuEntry::item(menu::RESET_APPEARANCE, "Use global appearance").enabled(space.custom),
    );
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::item(
        menu::SPACE_SETTINGS,
        "Space settings\u{2026}",
    ));
    MenuEntry::submenu("Appearance", entries)
}

/// The "Sort by" submenu; a one-off reorder of the space's items.
fn space_sort_menu() -> MenuEntry {
    MenuEntry::submenu(
        "Sort by",
        SORT_PRESETS
            .iter()
            .map(|(id, label, ..)| MenuEntry::item(*id, *label))
            .collect(),
    )
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
    use deskspace_platform::item_check_box_hit;

    #[test]
    fn a_second_click_on_the_only_selected_item_renames() {
        assert_eq!(
            pending_click(&[ItemId(1)], ItemId(1)),
            PendingClick::Rename(ItemId(1))
        );
        // Part of a multi-selection (or no selection) collapses instead.
        assert_eq!(
            pending_click(&[ItemId(1), ItemId(2)], ItemId(1)),
            PendingClick::Collapse(ItemId(1))
        );
        assert_eq!(
            pending_click(&[], ItemId(1)),
            PendingClick::Collapse(ItemId(1))
        );
    }

    #[test]
    fn releasing_a_second_click_starts_a_rename() {
        assert_eq!(
            resolve_pending_click(Some(PendingClick::Rename(ItemId(1))), ItemId(1), true),
            PendingClickOutcome::Rename
        );
        assert_eq!(
            resolve_pending_click(Some(PendingClick::Collapse(ItemId(1))), ItemId(1), true),
            PendingClickOutcome::Collapse
        );
        // A press on some other item, or an item that is no longer selected,
        // does nothing.
        assert_eq!(
            resolve_pending_click(Some(PendingClick::Rename(ItemId(1))), ItemId(2), true),
            PendingClickOutcome::Nothing
        );
        assert_eq!(
            resolve_pending_click(Some(PendingClick::Rename(ItemId(1))), ItemId(1), false),
            PendingClickOutcome::Nothing
        );
        assert_eq!(
            resolve_pending_click(None, ItemId(1), true),
            PendingClickOutcome::Nothing
        );
    }

    #[test]
    fn checkbox_hit_test_matches_the_icon_corner() {
        use deskspace_core::Item;
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = app
            .config
            .desktop
            .add_space("S", Rect::new(100, 100, 240, 200));
        let mut item = Item::new(ItemId(1), "C:/missing/a.txt");
        item.position = Point::ZERO;
        app.config.desktop.add_item(id, item);
        app.shell_display = ShellDisplay {
            check_boxes: CheckBoxMode::Selection,
            ..ShellDisplay::default()
        };
        let layout = app.config.desktop.layout;
        let cell = Rect::new(100, 100 + 28, layout.cell_width, layout.cell_height);
        let checkbox = item_check_box_hit(cell, app.metrics_for(id));
        assert_eq!(
            app.checkbox_at(
                Surface::Space(id),
                Point::new(checkbox.x + 3, checkbox.y + 3)
            ),
            Some(ItemId(1))
        );
        assert_eq!(
            app.checkbox_at(Surface::Space(id), Point::new(cell.x + 60, cell.y + 60)),
            None
        );
    }

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
    fn keyboard_menu_anchor_follows_the_focus() {
        use deskspace_core::Item;
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = app
            .config
            .desktop
            .add_space("S", Rect::new(100, 100, 240, 200));
        let mut item = Item::new(ItemId(1), "C:/missing/a.txt");
        item.position = Point::new(12, 12);
        app.config.desktop.add_item(id, item);
        app.selected_surface = Some(Surface::Space(id));

        let (rect, content) = {
            let space = app.config.desktop.space(id).unwrap();
            (space.rect, space.content_rect())
        };
        let layout = app.config.desktop.layout;

        app.selected_items.clear();
        assert_eq!(app.menu_anchor(), Some(rect.center()));

        app.selected_items = vec![ItemId(1)];
        assert_eq!(
            app.menu_anchor(),
            Some(Point::new(
                content.x + 12 + layout.cell_width / 2,
                content.y + 12 + layout.cell_height / 2,
            ))
        );
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
    fn resize_cursor_matches_grabbed_edges() {
        let edges = |left, top, right, bottom| ResizeEdges {
            left,
            top,
            right,
            bottom,
        };
        assert_eq!(cursor_for(ResizeEdges::NONE), CursorShape::Arrow);
        assert_eq!(
            cursor_for(edges(true, false, false, false)),
            CursorShape::SizeWestEast
        );
        assert_eq!(
            cursor_for(edges(false, false, false, true)),
            CursorShape::SizeNorthSouth
        );
        assert_eq!(
            cursor_for(edges(false, false, true, true)),
            CursorShape::SizeNorthWestSouthEast
        );
        assert_eq!(
            cursor_for(edges(false, true, true, false)),
            CursorShape::SizeNorthEastSouthWest
        );
    }

    #[test]
    fn double_click_rolls_on_the_title_and_opens_items() {
        let layout = deskspace_core::GridSpec::default();
        let mut space = Space::new(SpaceId(1), "S", Rect::new(100, 100, 240, 400));
        let mut item = deskspace_core::Item::new(ItemId(7), "C:/a.lnk");
        item.position = Point::new(12, 12);
        space.push_item(item);
        let target =
            |x, y, scroll| double_click_target(&space, layout.cell(), scroll, Point::new(x, y));
        let in_item = (100 + 12 + 10, 100 + space.title_height + 12 + 10);

        assert_eq!(target(150, 110, 0), DoubleClickTarget::TitleBar);
        assert_eq!(
            target(in_item.0, in_item.1, 0),
            DoubleClickTarget::Item(ItemId(7))
        );
        assert_eq!(target(300, 450, 0), DoubleClickTarget::Nothing);
        // Scrolled past the item, the same point is empty content.
        assert_eq!(
            target(in_item.0, in_item.1, 200),
            DoubleClickTarget::Nothing
        );
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
