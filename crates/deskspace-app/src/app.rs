use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Context;
use deskspace_config::{Config, ConfigStore};
use deskspace_core::geometry::{Point, Size};
use deskspace_core::layout::GridSpec;
use deskspace_core::theme::ICON_SIZE_RANGE;
use deskspace_core::{
    Color, Command, Desktop, ItemId, ItemKind, Palette, Rect, ResizeEdges, SortKey, Space, SpaceId,
};
use deskspace_platform::{
    CheckBoxMode, ClipboardAction, ClipboardFiles, CursorShape, DragImage, EditKey, EditOutcome,
    Event, EventHandler, Hotkey, ITEM_ICON_SIZE, IconCache, ItemMenuChoice, ItemMetrics, MenuEntry,
    MouseButton, Placement, RunOptions, SelectionKey, SettingsEvent, ShellDisplay, SpaceEvent,
    TextEdit, TrayEvent, Ui, desktop_directory, desktop_display, drag_threshold, hotkey_id,
    item_check_box_hit, notify_moved, parse_hotkey, set_start_with_windows, space_dpi,
    system_accent_color, system_uses_dark_mode, work_area_at,
};

use crate::bootstrap::seed_default_desktop;
use crate::cli::Cli;
use crate::scene::{ViewState, drag_scene, item_label, space_scene, space_style};
use crate::settings::{self, Effect, Session};
use crate::storage::ItemStore;

const TOGGLE_HOTKEY: i32 = 0;
/// Width of the band along a space's edges that starts a resize.
const RESIZE_BORDER: i32 = 6;
/// `WHEEL_DELTA`: one notch of a standard mouse wheel.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
const WHEEL_DELTA: i32 = 120;
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
    pub const ICON_SIZE_DEFAULT: u32 = 40;
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

/// The opacity presets the Appearance submenu offers: menu id, label, and the
/// stored opacity. Drives both the submenu and how a choice is handled.
const OPACITY_PRESETS: [(u32, &str, u8); 5] = [
    (menu::OPACITY_100, "Opacity 100%", 255),
    (menu::OPACITY_80, "Opacity 80%", 204),
    (menu::OPACITY_60, "Opacity 60%", 153),
    (menu::OPACITY_40, "Opacity 40%", 102),
    (menu::OPACITY_20, "Opacity 20%", 51),
];

/// The icon-size choices the "Icon size" submenu offers: menu id, label, and
/// the per-space percent. `None` follows the global setting.
const ICON_SIZE_PRESETS: [(u32, &str, Option<u8>); 5] = [
    (menu::ICON_SIZE_DEFAULT, "Default", None),
    (menu::ICON_SIZE_SMALL, "Small", Some(75)),
    (menu::ICON_SIZE_MEDIUM, "Medium", Some(100)),
    (menu::ICON_SIZE_LARGE, "Large", Some(150)),
    (menu::ICON_SIZE_XLARGE, "Extra large", Some(200)),
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
    /// rearranges the item within its space; once the pointer leaves the
    /// space it becomes a shell drag, as dragging an icon out of a folder.
    Item {
        space: SpaceId,
        item: ItemId,
        origin: Point,
        active: bool,
    },
    /// Rubber-band selection over empty content.
    Marquee {
        space: SpaceId,
        origin: Point,
        /// Ctrl/Shift: add to the selection instead of replacing it.
        additive: bool,
    },
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

/// An in-progress inline rename. The platform captures keys while it lives.
struct EditSession {
    space: SpaceId,
    target: EditTarget,
    original: String,
    text: TextEdit,
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
    store: ConfigStore,
    storage: ItemStore,
    icons: IconCache,
    system_dark: bool,
    /// Windows' accent color, refreshed on `SystemThemeChanged`; `None` when it
    /// cannot be read.
    system_accent: Option<Color>,
    /// Explorer's current display options, refreshed on change.
    shell_display: ShellDisplay,
    selected_space: Option<SpaceId>,
    /// Items selected in `selected_space`, in the order they were selected.
    selected_items: Vec<ItemId>,
    /// The item a Shift+click extends the selection from.
    selection_anchor: Option<ItemId>,
    /// An already-selected item pressed without Ctrl/Shift: if the press does
    /// not become a drag, the selection collapses to it on release, so dragging
    /// one member of a multi-selection moves the whole group.
    pending_select: Option<ItemId>,
    /// Items on the clipboard as a cut, shown ghosted until pasted.
    cut_items: HashSet<ItemId>,
    /// Ticked items in independent item-check-box mode. Session-only.
    checked_items: Vec<ItemId>,
    hovered: Option<(SpaceId, ItemId)>,
    drag: Drag,
    /// The items whose slots are drawn faded while they are dragged.
    dragged: Option<(SpaceId, Vec<ItemId>)>,
    /// The rubber-band selection rectangle in desktop coordinates and the
    /// space it belongs to, while the user drags over empty content.
    marquee: Option<(SpaceId, Rect)>,
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
    settings: Option<Session>,
    open_settings_on_start: bool,
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
            selected_space: None,
            selected_items: Vec::new(),
            selection_anchor: None,
            pending_select: None,
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
            settings: None,
            open_settings_on_start: open_settings,
        }
    }

    fn dispatch(&mut self, command: Command, near: Point) -> bool {
        let bounds = work_area_at(near);
        // Use the grid of the space under the action so a command that moves or
        // arranges items lands on the same monitor's scale.
        let layout = self
            .config
            .desktop
            .space_at(near)
            .map_or_else(|| self.effective_layout(), |id| self.layout_for(id));
        self.config
            .desktop
            .apply_with_layout(command, layout, bounds)
            .is_applied()
    }

    fn save(&self) {
        if let Err(error) = self.store.save(&self.config) {
            tracing::error!(%error, "failed to persist configuration");
        } else {
            tracing::debug!(path = %self.store.path().display(), "configuration saved");
        }
    }

    /// Defers a configuration write until the user pauses, restarting the
    /// timer on every change so a burst of them writes once.
    fn schedule_save(&mut self, ui: &mut Ui) {
        self.autosave.mark();
        ui.set_timer(AUTOSAVE_TIMER, AUTOSAVE_DELAY_MS);
    }

    /// Writes any deferred change now and stops the autosave timer. Used when
    /// the timer fires and when the app closes, so nothing is lost.
    fn flush_save(&mut self, ui: &mut Ui) {
        if self.autosave.take() {
            ui.clear_timer(AUTOSAVE_TIMER);
        }
        self.save();
    }

    /// Applies `command`, then repaints the space and schedules a save when the
    /// domain changed. Returns whether the command applied.
    fn apply_to_space(&mut self, ui: &mut Ui, id: SpaceId, command: Command, at: Point) -> bool {
        if self.dispatch(command, at) {
            self.refresh_space(ui, id);
            self.schedule_save(ui);
            true
        } else {
            false
        }
    }

    // ---- space windows -------------------------------------------------

    /// Creates windows for new spaces and destroys windows of removed ones.
    fn sync_windows(&mut self, ui: &mut Ui) {
        let existing: HashSet<u64> = ui.space_ids().into_iter().collect();
        let style = space_style(&self.config.theme, self.system_dark);
        let ids: Vec<SpaceId> = self.config.desktop.spaces.iter().map(|f| f.id).collect();
        for id in &ids {
            if existing.contains(&id.0) {
                continue;
            }
            let Some(rect) = self.config.desktop.space(*id).map(|f| f.visible_rect()) else {
                continue;
            };
            if let Err(error) = ui.create_space(id.0, rect, style) {
                tracing::error!(%error, space = id.0, "could not create space window");
                continue;
            }
            self.refresh_space(ui, *id);
        }
        for stale in existing.difference(&ids.iter().map(|id| id.0).collect()) {
            ui.destroy_space(*stale);
        }
        self.refresh_tray(ui);
    }

    fn view_for(&self, id: SpaceId) -> ViewState {
        ViewState {
            hovered: self
                .hovered
                .filter(|(space, _)| *space == id)
                .map(|(_, item)| item),
            selected: if self.selected_space == Some(id) {
                self.selected_items.clone()
            } else {
                Vec::new()
            },
            dragged: self
                .dragged
                .as_ref()
                .filter(|(space, _)| *space == id)
                .map(|(_, items)| items.clone())
                .unwrap_or_default(),
            scroll: self.scroll_of(id),
            show_check_boxes: self.check_box_mode() != CheckBoxMode::Hidden,
            check_boxes_persistent: self.check_box_mode() == CheckBoxMode::Independent,
            checked: self.checked_items.clone(),
            show_extensions: self.match_explorer() && self.shell_display.show_extensions,
            show_compressed_color: self.match_explorer()
                && self.shell_display.show_compressed_color,
            cut: self.cut_items.iter().copied().collect(),
            marquee: self.marquee.and_then(|(owner, rect)| {
                if owner != id {
                    return None;
                }
                let origin = self.config.desktop.space(id)?.rect;
                Some(Rect::new(
                    rect.x - origin.x,
                    rect.y - origin.y,
                    rect.width,
                    rect.height,
                ))
            }),
        }
    }

    /// Whether Explorer's display settings are mirrored.
    fn match_explorer(&self) -> bool {
        self.config.theme.match_explorer
    }

    /// How the desktop shows item check boxes, honoring `match_explorer`.
    fn check_box_mode(&self) -> CheckBoxMode {
        if self.match_explorer() {
            self.shell_display.check_boxes
        } else {
            CheckBoxMode::Hidden
        }
    }

    /// The concrete colors for a space's scene: the theme's palette, with the
    /// selection highlight and border replaced by Windows' accent when the
    /// setting is on and an accent is known.
    fn palette(&self) -> Palette {
        let palette = self.config.theme.palette(self.system_dark);
        match (self.config.theme.use_accent, self.system_accent) {
            (true, Some(accent)) => palette.with_accent(accent),
            _ => palette,
        }
    }

    /// Whether DeskSpace's own chrome (the settings window and menus) is dark.
    /// `Theme.mode` colors the spaces; this follows `Theme.ui_mode`.
    fn ui_dark(&self) -> bool {
        self.config.theme.ui_mode.resolve(self.system_dark)
    }

    /// The item-icon scale as a factor, without per-monitor DPI. When mirroring
    /// Explorer's display, the desktop's own icon size (in pixels) wins, so
    /// spaces are 1:1 with the desktop; otherwise the DeskSpace icon-size
    /// setting applies.
    fn icon_scale(&self) -> f32 {
        if self.match_explorer()
            && let Some(pixels) = self.shell_display.icon_size
        {
            return (pixels as f32 / ITEM_ICON_SIZE as f32).max(0.25);
        }
        self.config
            .theme
            .icon_size
            .clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as f32
            / 100.0
    }

    /// The full icon factor for one space: the icon-size factor times the
    /// monitor's DPI. Explorer's own icon size is already in physical pixels,
    /// so mirroring does not multiply by DPI again. A per-space override wins
    /// over both Explorer and the global setting.
    fn space_scale(&self, id: SpaceId) -> f32 {
        if let Some(percent) = self
            .config
            .desktop
            .space(id)
            .and_then(|space| space.icon_size)
        {
            return percent.clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as f32 / 100.0
                * space_dpi(id.0);
        }
        if self.match_explorer() && self.shell_display.icon_size.is_some() {
            return self.icon_scale();
        }
        self.icon_scale() * space_dpi(id.0)
    }

    /// The layout grid for a given icon/DPI scale. When mirroring Explorer, its
    /// item spacing (the full cell size in physical pixels) replaces the cell
    /// and gap, so the grid step matches the desktop exactly. The stored
    /// `config.desktop.layout` is never scaled, so the settings form keeps
    /// editing the logical values.
    fn base_layout(&self, scale: f32) -> GridSpec {
        let mut layout = self.config.desktop.layout.scaled(scale);
        if self.match_explorer()
            && let Some(spacing) = self.shell_display.icon_spacing
        {
            layout.cell_width = spacing.width.max(1);
            layout.cell_height = spacing.height.max(1);
            layout.gap = 0;
        }
        layout
    }

    /// The scaled grid for one space, including its monitor's DPI. A per-space
    /// icon-size override ignores Explorer's item spacing too, so the whole
    /// grid stays consistent with the overridden icon scale.
    fn layout_for(&self, id: SpaceId) -> GridSpec {
        let scale = self.space_scale(id);
        if self
            .config
            .desktop
            .space(id)
            .and_then(|space| space.icon_size)
            .is_some()
        {
            return self.config.desktop.layout.scaled(scale);
        }
        self.base_layout(scale)
    }

    /// The logical grid (icon size only, no per-monitor DPI), for callers that
    /// have no space at hand.
    fn effective_layout(&self) -> GridSpec {
        self.base_layout(self.icon_scale())
    }

    /// Scaled icon, label, and check-box sizes for one space.
    fn metrics_for(&self, id: SpaceId) -> ItemMetrics {
        ItemMetrics::for_scale(self.space_scale(id))
    }

    /// Re-reads Explorer's display options and repaints if they changed. A
    /// changed icon size or item spacing re-flows every space too.
    fn refresh_shell_display(&mut self, ui: &mut Ui) {
        let display = desktop_display();
        if display == self.shell_display {
            return;
        }
        let relayout = self.match_explorer()
            && (display.icon_size != self.shell_display.icon_size
                || display.icon_spacing != self.shell_display.icon_spacing);
        self.shell_display = display;
        // Independent ticks do not survive a switch out of that mode.
        if self.check_box_mode() != CheckBoxMode::Independent {
            self.checked_items.clear();
        }
        if relayout {
            self.relayout_all(ui);
        } else {
            self.restyle_all(ui);
        }
    }

    /// The space's scroll offset, clamped to its current content.
    fn scroll_of(&self, id: SpaceId) -> i32 {
        let layout = self.layout_for(id);
        let max = self
            .config
            .desktop
            .space(id)
            .map_or(0, |space| space.max_scroll(layout.cell(), layout.padding));
        self.scroll.get(&id).copied().unwrap_or(0).clamp(0, max)
    }

    /// The item whose Explorer-style check box contains `point`, when check
    /// boxes are shown. `point` is in desktop coordinates.
    fn checkbox_at(&self, id: SpaceId, point: Point) -> Option<ItemId> {
        if self.check_box_mode() == CheckBoxMode::Hidden {
            return None;
        }
        let item = self.item_at(id, point)?;
        let space = self.config.desktop.space(id)?;
        let entry = space.items.iter().find(|entry| entry.id == item)?;
        let layout = self.layout_for(id);
        let content = space.content_rect();
        let cell = Rect::new(
            content.x + entry.position.x,
            content.y + entry.position.y - self.scroll_of(id),
            layout.cell_width,
            layout.cell_height,
        );
        item_check_box_hit(cell, self.metrics_for(id))
            .contains(point)
            .then_some(item)
    }

    fn item_at(&self, id: SpaceId, point: Point) -> Option<ItemId> {
        let cell = self.layout_for(id).cell();
        let scroll = self.scroll_of(id);
        self.config
            .desktop
            .space(id)
            .and_then(|space| space.item_at(point, cell, scroll))
    }

    /// The item holding the file at `path`, comparing identity rather than the
    /// literal string so a shell-reported short name or case difference still
    /// matches. A path in a space must be moved, not added again.
    fn find_item_by_file(&self, path: &Path) -> Option<ItemId> {
        self.config
            .desktop
            .spaces
            .iter()
            .flat_map(|space| space.items.iter())
            .find(|item| crate::storage::same_path(&item.path, path))
            .map(|item| item.id)
    }

    /// Pushes the space's current geometry and draw list to its window.
    fn refresh_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let view = self.view_for(id);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let palette = self.palette();
        let mut scene = space_scene(
            space,
            &self.config.theme,
            &palette,
            self.layout_for(id),
            self.metrics_for(id),
            view,
            &mut self.icons,
        );
        if let Some(session) = self.edit.as_ref().filter(|session| session.space == id) {
            match session.target {
                EditTarget::Title => scene.editing = Some(session.text.clone()),
                EditTarget::Item { id: item, .. } => {
                    scene.editing_item = Some((item.0, session.text.clone()));
                }
            }
        }
        scene.caret_visible = self.caret_visible;
        ui.set_space_rect(id.0, space.visible_rect());
        ui.set_space_scene(id.0, scene);
    }

    fn restyle_all(&mut self, ui: &mut Ui) {
        let style = space_style(&self.config.theme, self.system_dark);
        let ids: Vec<SpaceId> = self.config.desktop.spaces.iter().map(|f| f.id).collect();
        for id in ids {
            ui.set_space_style(id.0, style);
            self.refresh_space(ui, id);
        }
    }

    /// Re-flows every space for a changed grid: growing spaces resize to fit,
    /// user-sized spaces keep their size and reflow.
    fn relayout_all(&mut self, ui: &mut Ui) {
        let ids: Vec<SpaceId> = self
            .config
            .desktop
            .spaces
            .iter()
            .map(|space| space.id)
            .collect();
        for id in ids {
            self.reflow_space(ui, id);
        }
    }

    /// Re-flows one space for its current grid (icon size and monitor DPI) and
    /// repaints it.
    fn reflow_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let bounds = work_area_at(space.rect.center());
        let layout = self.layout_for(id);
        self.config
            .desktop
            .apply_with_layout(Command::Reflow { id }, layout, bounds);
        self.refresh_space(ui, id);
    }

    /// A space moved to a monitor with a different DPI: reflow it so icons and
    /// spacing match that display.
    fn on_dpi_changed(&mut self, ui: &mut Ui, id: SpaceId) {
        self.reflow_space(ui, id);
        self.schedule_save(ui);
    }

    fn refresh_tray(&self, ui: &mut Ui) {
        ui.set_tray_tooltip(&tray_tooltip(&self.config.desktop));
    }

    /// Clears the item selection, as clicking elsewhere does in Explorer.
    /// Returns the space that showed it, which needs a new scene.
    fn take_selection(&mut self) -> Option<SpaceId> {
        let space = self.selected_space.take();
        let had = !std::mem::take(&mut self.selected_items).is_empty();
        self.selection_anchor = None;
        had.then_some(space).flatten()
    }

    fn clear_selection(&mut self, ui: &mut Ui) {
        if let Some(space) = self.take_selection() {
            self.refresh_space(ui, space);
        }
    }

    /// Asks for outside clicks only while there is something for them to
    /// clear, because watching them hooks every mouse event in the session.
    fn sync_outside_watch(&mut self, ui: &mut Ui) {
        let wanted =
            !self.selected_items.is_empty() || self.edit.is_some() || self.selected_space.is_some();
        if wanted != self.watching_outside {
            ui.watch_outside_clicks(wanted);
            self.watching_outside = wanted;
        }
    }

    fn set_visible(&mut self, ui: &mut Ui, visible: bool) {
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
    fn sync_desktop_show_entry(&self) {
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
        // A real .ico beside the storage folder, so the menu shows the app's
        // icon instead of the executable's placeholder.
        let icon_path = self.storage.root().with_file_name("deskspace.ico");
        if let Err(error) = deskspace_platform::write_icon(&icon_path) {
            tracing::warn!(%error, "could not write the app icon");
        }
        let command = format!("\"{exe}\" --show");
        let icon = icon_path.to_string_lossy();
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
    fn remove_desktop_show_entry(&self) {
        if cfg!(test) {
            return;
        }
        if let Err(error) = deskspace_platform::set_desktop_show_entry(false, "", "", "") {
            tracing::warn!(%error, "could not remove the desktop Show spaces entry");
        }
    }

    /// Writes or clears the per-user `Run` value that starts DeskSpace at
    /// sign-in. Rewriting it on every start keeps the stored path current.
    fn sync_start_with_windows(&self, enabled: bool) {
        // Tests must not touch the real per-user registry.
        if cfg!(test) {
            return;
        }
        if let Err(error) = set_start_with_windows(enabled) {
            tracing::warn!(%error, "could not update the start-with-Windows entry");
        }
    }

    // ---- rename --------------------------------------------------------

    /// Starts inline editing of a space's title, capturing keys until commit
    /// or cancel.
    fn begin_rename(&mut self, ui: &mut Ui, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let original = space.title.clone();
        self.edit = Some(EditSession {
            space: id,
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
    fn begin_f2_rename(&mut self, ui: &mut Ui) {
        let Some(space) = self.selected_space else {
            return;
        };
        match self.selected_items.last().copied() {
            Some(item) => self.begin_item_rename(ui, space, item),
            None => self.begin_rename(ui, space),
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
        }
    }

    /// Re-reads Explorer's display options and re-flows every space, as F5
    /// refreshes the desktop.
    fn refresh_spaces(&mut self, ui: &mut Ui) {
        self.refresh_shell_display(ui);
        self.relayout_all(ui);
    }

    /// Opens the context menu for the focused item, or for the current space
    /// when no item is selected, as the Menu key or Shift+F10 does.
    fn open_context_menu(&mut self, ui: &mut Ui) {
        let Some(space) = self.selected_space else {
            return;
        };
        let Some(at) = self.menu_anchor() else {
            return;
        };
        match self.selected_items.last().copied() {
            Some(item) => self.show_item_menu(ui, space, item, at),
            None => self.show_space_menu(ui, space, at),
        }
    }

    /// Where to anchor a keyboard-opened menu: the center of the focused item's
    /// cell, or the current space's center when no item is selected.
    fn menu_anchor(&self) -> Option<Point> {
        let id = self.selected_space?;
        let space = self.config.desktop.space(id)?;
        let Some(item) = self.selected_items.last().copied() else {
            return Some(space.rect.center());
        };
        let entry = space.items.iter().find(|entry| entry.id == item)?;
        let layout = self.layout_for(id);
        let content = space.content_rect();
        Some(Point::new(
            content.x + entry.position.x + layout.cell_width / 2,
            content.y + entry.position.y - self.scroll_of(id) + layout.cell_height / 2,
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

    fn begin_item_rename(&mut self, ui: &mut Ui, space: SpaceId, item: ItemId) {
        let show_extensions = self.match_explorer() && self.shell_display.show_extensions;
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
            space,
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

    fn apply_edit_key(&mut self, ui: &mut Ui, key: EditKey) {
        let Some(session) = self.edit.as_mut() else {
            return;
        };
        let space = session.space;
        let outcome = session.text.apply(key);
        match outcome {
            EditOutcome::Changed => {
                // Typing restarts the blink with the caret shown.
                self.caret_visible = true;
                self.refresh_space(ui, space);
            }
            EditOutcome::Commit => self.finish_rename(ui, true),
            EditOutcome::Cancel => self.finish_rename(ui, false),
            EditOutcome::Ignored => {}
        }
    }

    /// Ends the rename. An empty or unchanged title is dropped on commit, so
    /// pressing Enter without editing never changes or clears the name.
    fn finish_rename(&mut self, ui: &mut Ui, commit: bool) {
        let Some(session) = self.edit.take() else {
            return;
        };
        ui.capture_text_input(false);
        ui.clear_timer(CARET_TIMER);
        let id = session.space;
        match session.target {
            EditTarget::Title => {
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
        self.refresh_space(ui, id);
    }

    /// Renames an item's file to the edited text and updates the model. The
    /// original extension is kept when the label did not show it, as Explorer
    /// does. A cancelled or skipped shell rename leaves the item unchanged.
    fn commit_item_rename(&mut self, ui: &mut Ui, id: ItemId, shows_extension: bool, typed: &str) {
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

    fn cancel_edit(&mut self, ui: &mut Ui) {
        if self.edit.is_some() {
            self.finish_rename(ui, false);
        }
    }

    /// Selects every item in the current space, as Ctrl+A does.
    fn select_all_in_space(&mut self, ui: &mut Ui) {
        let Some(space) = self.selected_space else {
            return;
        };
        let ids: Vec<ItemId> = self
            .config
            .desktop
            .space(space)
            .map(|space| space.items.iter().map(|item| item.id).collect())
            .unwrap_or_default();
        if ids.is_empty() {
            return;
        }
        self.selected_items = ids;
        self.selection_anchor = self.selected_items.first().copied();
        self.refresh_space(ui, space);
    }

    /// Toggles the edit caret so it blinks, as Windows does.
    fn blink_caret(&mut self, ui: &mut Ui) {
        let Some(space) = self.edit.as_ref().map(|session| session.space) else {
            ui.clear_timer(CARET_TIMER);
            return;
        };
        self.caret_visible = !self.caret_visible;
        self.refresh_space(ui, space);
    }

    // ---- per-space appearance ------------------------------------------

    fn set_space_opacity(&mut self, ui: &mut Ui, id: SpaceId, opacity: u8, at: Point) {
        self.apply_to_space(ui, id, Command::SetSpaceOpacity { id, opacity }, at);
    }

    fn set_space_icon_size(&mut self, ui: &mut Ui, id: SpaceId, icon_size: Option<u8>, at: Point) {
        if self.dispatch(Command::SetSpaceIconSize { id, icon_size }, at) {
            self.reflow_space(ui, id);
            self.schedule_save(ui);
        }
    }

    fn pick_space_color(&mut self, ui: &mut Ui, id: SpaceId, at: Point, which: SpaceColor) {
        let palette = self.palette();
        let Some(current) = self.config.desktop.space(id).map(|space| match which {
            SpaceColor::Tint => space.tint.unwrap_or(palette.tint).with_alpha(255),
            SpaceColor::Text => space.text.unwrap_or(palette.title_text),
        }) else {
            return;
        };
        if let Some(color) = ui.pick_color(current) {
            let command = match which {
                SpaceColor::Tint => Command::SetSpaceTint {
                    id,
                    tint: Some(color),
                },
                SpaceColor::Text => Command::SetSpaceText {
                    id,
                    text: Some(color),
                },
            };
            self.apply_to_space(ui, id, command, at);
        }
    }

    fn reset_space_appearance(&mut self, ui: &mut Ui, id: SpaceId, at: Point) {
        let opacity = self.dispatch(Command::SetSpaceOpacity { id, opacity: 255 }, at);
        let tint = self.dispatch(Command::SetSpaceTint { id, tint: None }, at);
        let text = self.dispatch(Command::SetSpaceText { id, text: None }, at);
        let icon_size = self.dispatch(
            Command::SetSpaceIconSize {
                id,
                icon_size: None,
            },
            at,
        );
        if opacity && tint && text && icon_size {
            self.reflow_space(ui, id);
            self.schedule_save(ui);
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
            } => self.begin_interaction(ui, id, Point::new(x, y), ctrl, shift),
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
                ..
            } => self.select_at(ui, id, Point::new(x, y)),
            SpaceEvent::MouseMove { x, y } => {
                let point = Point::new(x, y);
                if !self.drag_to(ui, point) {
                    self.update_cursor(ui, id, point);
                    self.update_hover(ui, id, point);
                }
            }
            SpaceEvent::Wheel { x, y, delta } => self.scroll_by(ui, id, Point::new(x, y), delta),
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
                match self.item_at(id, point) {
                    Some(item) => self.show_item_menu(ui, id, item, point),
                    None => self.show_space_menu(ui, id, point),
                }
            }
            SpaceEvent::DoubleClick { x, y } => self.double_click(ui, id, Point::new(x, y)),
            SpaceEvent::MouseLeave => {
                self.dismiss_info_tip(ui);
                if self.hovered.is_some_and(|(space, _)| space == id) {
                    self.hovered = None;
                    self.refresh_space(ui, id);
                }
            }
            SpaceEvent::FilesDropped {
                x,
                y,
                paths,
                keep_in_place,
            } => {
                self.add_dropped(ui, id, Point::new(x, y), paths, keep_in_place);
            }
            SpaceEvent::DpiChanged { .. } => self.on_dpi_changed(ui, id),
            _ => {}
        }
    }

    fn begin_interaction(
        &mut self,
        ui: &mut Ui,
        id: SpaceId,
        point: Point,
        ctrl: bool,
        shift: bool,
    ) {
        // A click on an item's check box either toggles that item in the
        // selection (auto-check-select) or the independent tick set, and moves
        // the highlight to it.
        if let Some(item) = self.checkbox_at(id, point) {
            let previous = self.selected_space.replace(id);
            if self.check_box_mode() == CheckBoxMode::Independent {
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
            if let Some(previous) = previous.filter(|previous| *previous != id) {
                self.refresh_space(ui, previous);
            }
            self.refresh_space(ui, id);
            return;
        }
        let previous = self.selected_space.replace(id);
        self.pending_select = None;
        let item = self.item_at(id, point);
        match item {
            Some(item) => {
                if shift {
                    self.extend_selection(id, item);
                } else if ctrl {
                    self.toggle_selected(item);
                } else if self.is_selected(item) {
                    // Keep the whole selection so a drag of one member moves
                    // the group; collapse it on release if this was a click.
                    self.pending_select = Some(item);
                } else {
                    self.selected_items = vec![item];
                }
                self.selection_anchor = Some(item);
            }
            None => {
                if !ctrl && !shift {
                    self.selected_items.clear();
                }
            }
        }
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let edges = space.resize_edges_at(point, RESIZE_BORDER);
        if edges.any() {
            self.drag = Drag::Resize { id, edges };
            self.selected_items.clear();
        } else if let Some(item) = item {
            self.drag = Drag::Item {
                space: id,
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
                space: id,
                origin: point,
                additive: ctrl || shift,
            };
        }
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            self.refresh_space(ui, previous);
        }
        self.refresh_space(ui, id);
    }

    fn is_selected(&self, item: ItemId) -> bool {
        self.selected_items.contains(&item)
    }

    /// Adds `item` to the selection, or removes it if it was already selected.
    fn toggle_selected(&mut self, item: ItemId) {
        if let Some(index) = self.selected_items.iter().position(|entry| *entry == item) {
            self.selected_items.remove(index);
        } else {
            self.selected_items.push(item);
        }
    }

    /// Selects every item between the anchor and `item` in the space's order,
    /// as a Shift+click does.
    fn extend_selection(&mut self, space: SpaceId, item: ItemId) {
        let Some(anchor) = self.selection_anchor else {
            self.selected_items = vec![item];
            return;
        };
        let Some(space) = self.config.desktop.space(space) else {
            return;
        };
        let index = |target| space.items.iter().position(|entry| entry.id == target);
        let (Some(a), Some(b)) = (index(anchor), index(item)) else {
            self.selected_items = vec![item];
            return;
        };
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        self.selected_items = space.items[start..=end]
            .iter()
            .map(|entry| entry.id)
            .collect();
    }

    /// Selects the item under `point` (or nothing) without starting a drag,
    /// as a right-click does before its menu opens. Right-clicking an item
    /// that is already part of a multi-selection keeps the selection.
    fn select_at(&mut self, ui: &mut Ui, id: SpaceId, point: Point) {
        let previous = self.selected_space.replace(id);
        let item = self.item_at(id, point);
        match item {
            Some(item) if !self.is_selected(item) => {
                self.selected_items = vec![item];
                self.selection_anchor = Some(item);
            }
            Some(_) => {}
            None => self.selected_items.clear(),
        }
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            self.refresh_space(ui, previous);
        }
        self.refresh_space(ui, id);
    }

    fn drag_to(&mut self, ui: &mut Ui, point: Point) -> bool {
        match self.drag {
            Drag::None => false,
            Drag::Item {
                space,
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
                        space,
                        item,
                        origin,
                        active: true,
                    };
                    self.begin_drag_visual(ui, space, item, origin, point);
                }
                // Still over the space: rearrange. Left it: hand off to the
                // shell, which moves the file to another space or app.
                let inside = self
                    .config
                    .desktop
                    .space(space)
                    .is_some_and(|space| space.visible_rect().contains(point));
                if inside {
                    ui.move_drag_ghost(point);
                    self.reorder_item_at(ui, space, item, point);
                } else {
                    self.drag = Drag::None;
                    // The shell draws the same image from here on.
                    ui.hide_drag_ghost();
                    self.drag_item_out(ui, space, item);
                    self.end_drag_visual(ui);
                }
                true
            }
            Drag::Marquee {
                space: id,
                origin,
                additive,
            } => {
                self.update_marquee(ui, id, origin, additive, point);
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
        self.hovered = self.item_at(id, point).map(|item| (id, item));
        self.refresh_space(ui, id);
    }

    fn end_drag(&mut self, ui: &mut Ui) {
        let pending = self.pending_select.take();
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::None => {}
            Drag::Item {
                space,
                item,
                active: false,
                ..
            } => {
                // A press on an already-selected item that never became a drag
                // is a plain click: collapse the selection to it.
                if pending == Some(item) && self.is_selected(item) {
                    self.selected_items = vec![item];
                    self.selection_anchor = Some(item);
                    self.refresh_space(ui, space);
                }
            }
            Drag::Item { active: true, .. } => {
                self.end_drag_visual(ui);
                self.schedule_save(ui);
            }
            Drag::Marquee { space, .. } => {
                // The selection already updated live; just drop the rectangle.
                self.marquee = None;
                self.refresh_space(ui, space);
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
        space: SpaceId,
        item: ItemId,
        origin: Point,
        point: Point,
    ) {
        // The whole active set drags, as in Explorer, so the image is that set
        // rendered as it is now, before the slots fade.
        let ids = self.drag_items(item);
        let id_values: Vec<u64> = ids.iter().map(|id| id.0).collect();
        let view = self.view_for(space);
        let layout = self.layout_for(space);
        let metrics = self.metrics_for(space);
        let palette = self.palette();
        self.drag_image = self.config.desktop.space(space).and_then(|entry| {
            let scene = drag_scene(
                entry,
                &ids,
                &self.config.theme,
                &palette,
                layout,
                metrics,
                view,
                &mut self.icons,
            );
            let grab = Point::new(origin.x - entry.rect.x, origin.y - entry.rect.y);
            DragImage::of_selection(&scene, &id_values, grab)
        });
        self.dragged = Some((space, ids));
        self.pending_select = None;
        if let Some(image) = &self.drag_image {
            ui.show_drag_ghost(image, point);
        }
        self.refresh_space(ui, space);
    }

    fn end_drag_visual(&mut self, ui: &mut Ui) {
        ui.hide_drag_ghost();
        self.drag_image = None;
        if let Some((space, _)) = self.dragged.take() {
            self.refresh_space(ui, space);
        }
    }

    fn update_hover(&mut self, ui: &mut Ui, id: SpaceId, point: Point) {
        let hovered = self.item_at(id, point).map(|item| (id, item));
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
        if let Some((space, _)) = previous.filter(|(space, _)| *space != id) {
            self.refresh_space(ui, space);
        }
        self.refresh_space(ui, id);
    }

    /// Shows the info tip for the hovered item, once the pointer has rested on
    /// it, as Explorer does.
    fn show_info_tip(&mut self, ui: &mut Ui) {
        ui.clear_timer(INFO_TIP_TIMER);
        let Some((space_id, item)) = self.hovered else {
            return;
        };
        let Some(entry) = self.config.desktop.item(item) else {
            return;
        };
        let Some(space) = self.config.desktop.space(space_id) else {
            return;
        };
        let content = space.content_rect();
        let scroll = self.scroll_of(space_id);
        let path = entry.path.clone();
        let at = Point::new(
            content.x + entry.position.x,
            content.y + entry.position.y - scroll,
        );
        ui.show_info_tip(&path, at);
    }

    /// Hides any info tip and cancels its delay, for example when a click or
    /// drag interrupts the hover.
    fn dismiss_info_tip(&mut self, ui: &mut Ui) {
        ui.hide_info_tip();
        ui.clear_timer(INFO_TIP_TIMER);
    }

    fn toggle_rollup(&mut self, ui: &mut Ui, id: SpaceId) {
        let near = self.space_center(id);
        // A rolled-up space hides its items, so it cannot keep one selected.
        if self.selected_space == Some(id) {
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
        self.refresh_tray(ui);
        self.save();
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
        let Some(space) = self.selected_space else {
            return;
        };
        let items = self.active_items();
        if items.is_empty() {
            return;
        }
        self.selected_items.clear();
        self.selection_anchor = None;
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

    /// Puts the selected items on the shell clipboard, cut (to be moved on
    /// paste) or copied. Cut items are drawn ghosted until the paste.
    fn copy_selection(&mut self, ui: &mut Ui, cut: bool) {
        let active = self.active_items();
        let paths: Vec<PathBuf> = active
            .iter()
            .filter_map(|item| self.config.desktop.item(*item))
            .map(|item| item.path.clone())
            .filter(|path| path.exists())
            .collect();
        if paths.is_empty() {
            return;
        }
        if !ui.set_clipboard_files(&paths, cut) {
            return;
        }
        // Replacing the cut set un-ghosts the previous items, which may live in
        // other spaces, so repaint every space that showed one.
        let mut touched: HashSet<SpaceId> = self
            .cut_items
            .iter()
            .filter_map(|item| self.config.desktop.space_containing(*item))
            .collect();
        self.cut_items = if cut {
            active.iter().copied().collect()
        } else {
            HashSet::new()
        };
        touched.extend(
            self.cut_items
                .iter()
                .filter_map(|item| self.config.desktop.space_containing(*item)),
        );
        if let Some(space) = self.selected_space {
            touched.insert(space);
        }
        for id in touched {
            self.refresh_space(ui, id);
        }
    }

    /// Clears cut ghosting when the clipboard no longer holds the cut, for
    /// example after another app (or the desktop) copies, cuts, or pastes it.
    /// A paste elsewhere has moved the files, so any item whose file is gone is
    /// dropped. A failed clipboard read keeps the ghost rather than clearing it
    /// wrongly, since another process can hold the clipboard for a moment.
    fn sync_cut_with_clipboard(&mut self, ui: &mut Ui) {
        if self.cut_items.is_empty() {
            return;
        }
        let cut_paths: Vec<PathBuf> = self
            .cut_items
            .iter()
            .filter_map(|item| self.config.desktop.item(*item))
            .map(|item| item.path.clone())
            .collect();
        let Some(clipboard) = ui.clipboard_files() else {
            return;
        };
        if cut_matches_clipboard(&clipboard, &cut_paths) {
            return;
        }
        let touched: HashSet<SpaceId> = self
            .cut_items
            .iter()
            .filter_map(|item| self.config.desktop.space_containing(*item))
            .collect();
        self.cut_items.clear();
        for id in touched {
            self.prune_missing(ui, id);
            self.refresh_space(ui, id);
        }
    }

    /// Adds the clipboard's files to the current space, as a drop would: a cut
    /// moves them (items already in a space just change space, sharing the
    /// file), a copy duplicates them into the store. Physical copies and moves
    /// go through the shell's file operation so Explorer's progress UI and undo
    /// apply.
    fn paste_clipboard(&mut self, ui: &mut Ui) {
        let Some(target) = self.selected_space else {
            return;
        };
        let Some(clipboard) = ui.clipboard_files() else {
            return;
        };
        if clipboard.paths.is_empty() {
            return;
        }
        let at = self.space_center(target);
        let plan = plan_paste(&clipboard.paths, clipboard.cut, |path| {
            self.find_item_by_file(path)
        });
        let PastePlan {
            moved,
            external_moves,
            copies,
        } = plan;
        let mut touched = HashSet::from([target]);

        for item in &moved {
            match self.config.desktop.space_containing(*item) {
                Some(source) if source == target => continue,
                Some(source) => {
                    touched.insert(source);
                }
                None => {}
            }
            self.dispatch(Command::MoveItem { id: *item, target }, at);
        }

        let store = self.storage.root().to_path_buf();
        let mut created: Vec<PathBuf> = Vec::new();
        if !external_moves.is_empty() {
            if let Some(paths) = ui.transfer_files(&external_moves, &store, true) {
                created.extend(paths);
            }
        }
        if !copies.is_empty() {
            if let Some(paths) = ui.transfer_files(&copies, &store, false) {
                created.extend(paths);
            }
        }
        for path in &created {
            // Guard against a shell result that no longer exists; listing it
            // would leave a label with no icon.
            if !path.exists() {
                tracing::warn!(path = %path.display(), "pasted file is missing; ignored");
                continue;
            }
            self.dispatch(
                Command::AddItem {
                    space: target,
                    path: path.clone(),
                },
                at,
            );
        }

        // Select what was pasted, as Explorer does.
        let mut pasted: Vec<ItemId> = moved
            .iter()
            .copied()
            .filter(|item| self.config.desktop.space_containing(*item) == Some(target))
            .collect();
        pasted.extend(
            created
                .iter()
                .filter_map(|path| self.find_item_by_file(path)),
        );
        if !pasted.is_empty() {
            self.selected_space = Some(target);
            self.selection_anchor = pasted.first().copied();
            self.selected_items = pasted;
        }

        if clipboard.cut {
            // Un-ghost every cut item, including any not part of this paste,
            // and repaint the spaces that showed them.
            for item in std::mem::take(&mut self.cut_items) {
                if let Some(space) = self.config.desktop.space_containing(item) {
                    touched.insert(space);
                }
            }
            ui.clear_clipboard();
        }
        for id in &touched {
            self.refresh_space(ui, *id);
        }
        self.refresh_tray(ui);
        self.save();
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

    fn double_click(&mut self, ui: &mut Ui, id: SpaceId, point: Point) {
        let layout = self.layout_for(id);
        let scroll = self.scroll_of(id);
        let target = self
            .config
            .desktop
            .space(id)
            .map_or(DoubleClickTarget::Nothing, |space| {
                double_click_target(space, layout.cell(), scroll, point)
            });
        tracing::debug!(space = id.0, ?point, ?target, "double-click");
        match target {
            DoubleClickTarget::TitleBar => self.toggle_rollup(ui, id),
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
    fn drag_item_out(&mut self, ui: &mut Ui, space: SpaceId, item: ItemId) {
        // Dragging an item that is part of the active set drags the whole set,
        // as Explorer does; otherwise just that one item.
        let ids = self.drag_items(item);
        let (paths, positions): (Vec<PathBuf>, Vec<Point>) = self
            .item_cells(space, &ids)
            .into_iter()
            .filter(|(path, _)| path.exists())
            .unzip();
        // A stale item (its file moved or deleted since it was drawn) is
        // dropped instead of repeatedly failing to start a drag.
        if paths.is_empty() {
            self.prune_missing(ui, space);
            return;
        }
        let Some(result) = ui.drag_out(&paths, &positions, self.drag_image.as_ref()) else {
            return;
        };
        tracing::debug!(?result, files = paths.len(), "item drag ended");
        // The drop target may have moved some files out; drop any item whose
        // file is gone, wherever it lives.
        self.prune_all_missing(ui);
    }

    /// The path of each of `ids` in `space` and the top-left of its cell in
    /// desktop coordinates, where it is drawn now (off the visible area when
    /// scrolled out), in `ids` order.
    fn item_cells(&self, space: SpaceId, ids: &[ItemId]) -> Vec<(PathBuf, Point)> {
        let Some(entry) = self.config.desktop.space(space) else {
            return Vec::new();
        };
        let content = entry.content_rect();
        let scroll = self.scroll_of(space);
        ids.iter()
            .filter_map(|id| entry.items.iter().find(|item| item.id == *id))
            .map(|item| {
                let cell = Point::new(
                    content.x + item.position.x,
                    content.y + item.position.y - scroll,
                );
                (item.path.clone(), cell)
            })
            .collect()
    }

    /// The items an item operation acts on: the independent tick set when
    /// check boxes are separate and any are ticked, otherwise the selection.
    fn active_items(&self) -> Vec<ItemId> {
        if self.check_box_mode() == CheckBoxMode::Independent && !self.checked_items.is_empty() {
            self.checked_items.clone()
        } else {
            self.selected_items.clone()
        }
    }

    /// The items a drag started on `item` moves: the active set when it
    /// includes `item`, otherwise just `item`.
    fn drag_items(&self, item: ItemId) -> Vec<ItemId> {
        let active = self.active_items();
        if active.contains(&item) {
            active
        } else {
            vec![item]
        }
    }

    /// Updates the rubber-band rectangle and the live selection as the pointer
    /// moves over empty content.
    fn update_marquee(
        &mut self,
        ui: &mut Ui,
        id: SpaceId,
        origin: Point,
        additive: bool,
        point: Point,
    ) {
        let cell = self.layout_for(id).cell();
        let Some(content) = self
            .config
            .desktop
            .space(id)
            .map(|space| space.content_rect())
        else {
            return;
        };
        let scroll = self.scroll_of(id);
        let cells: Vec<(ItemId, Rect)> = self
            .config
            .desktop
            .space(id)
            .map(|space| {
                space
                    .items
                    .iter()
                    .map(|item| {
                        (
                            item.id,
                            Rect::new(
                                content.x + item.position.x,
                                content.y + item.position.y - scroll,
                                cell.width,
                                cell.height,
                            ),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let left = origin.x.min(point.x).max(content.x);
        let top = origin.y.min(point.y).max(content.y);
        let right = origin.x.max(point.x).min(content.right());
        let bottom = origin.y.max(point.y).min(content.bottom());
        let rect = Rect::new(left, top, (right - left).max(0), (bottom - top).max(0));
        self.marquee = Some((id, rect));

        self.selected_space = Some(id);
        self.selected_items = marquee_selection(&cells, rect, additive, &self.selected_items);
        self.selection_anchor = None;
        self.refresh_space(ui, id);
    }

    fn show_item_menu(&mut self, ui: &mut Ui, space: SpaceId, item: ItemId, at: Point) {
        let Some(path) = self.config.desktop.item(item).map(|item| item.path.clone()) else {
            return;
        };
        let remove_label = if self.storage.contains(&path) {
            "Move back to desktop"
        } else {
            "Remove from space"
        };
        let entries = [
            MenuEntry::item(menu::RENAME_ITEM, "Rename"),
            MenuEntry::item(menu::REMOVE_ITEM, remove_label),
        ];
        match ui.item_menu(&path, &entries, at) {
            Some(ItemMenuChoice::App(menu::RENAME_ITEM)) => {
                self.begin_item_rename(ui, space, item);
            }
            Some(ItemMenuChoice::App(menu::REMOVE_ITEM)) => {
                if self.release_item(item) {
                    self.selected_items.retain(|selected| *selected != item);
                    self.forget_item(item);
                    self.hovered = None;
                    self.refresh_space(ui, space);
                    self.refresh_tray(ui);
                    self.save();
                }
            }
            Some(ItemMenuChoice::Shell) => {
                self.icons.invalidate(&path);
                self.prune_missing(ui, space);
                self.refresh_space(ui, space);
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
            if self.selected_space == Some(id) {
                self.selected_space = None;
                self.selected_items.clear();
                self.selection_anchor = None;
            }
            if self.hovered.is_some_and(|(space, _)| space == id) {
                self.hovered = None;
            }
            self.scroll.remove(&id);
            self.sync_windows(ui);
            self.save();
        }
    }
    fn space_center(&self, id: SpaceId) -> Point {
        self.config
            .desktop
            .space(id)
            .map_or(Point::ZERO, |space| space.rect.center())
    }

    // ---- menus ---------------------------------------------------------

    fn visibility_entry(&self) -> MenuEntry {
        let label = if self.visible {
            "Hide spaces"
        } else {
            "Show spaces"
        };
        MenuEntry::item(menu::TOGGLE_VISIBLE, label)
    }

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
        entries.push(space_appearance_menu(space));
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
            Some(menu::RESET_APPEARANCE) => self.reset_space_appearance(ui, id, at),
            Some(choice) => {
                if let Some((_, _, opacity)) = OPACITY_PRESETS
                    .iter()
                    .find(|(preset, ..)| *preset == choice)
                {
                    self.set_space_opacity(ui, id, *opacity, at);
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
        if self.settings.is_none() {
            self.settings = Some(Session::new(&self.config));
        }
        if let Some(form) = self.settings_form() {
            ui.open_settings(&form);
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
        match event {
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
                    self.restyle_all(ui);
                } else if effect == Effect::Layout {
                    self.relayout_all(ui);
                }
                self.refresh_settings(ui);
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
            // The navigation rail arrives with the per-space settings surface.
            SettingsEvent::NavChanged { .. } => {}
            SettingsEvent::Apply => {
                self.apply_settings(ui);
                self.refresh_settings(ui);
            }
            SettingsEvent::Ok => {
                if self.apply_settings(ui) {
                    self.settings = None;
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
        self.save();
        if let Some(session) = self.settings.as_mut() {
            session.hotkey_text = self.config.window.toggle_hotkey.clone();
            session.commit(&self.config);
        }
        true
    }

    fn cancel_settings(&mut self, ui: &mut Ui) {
        if let Some(session) = self.settings.take() {
            let restyle = session.saved_theme != self.config.theme;
            // A revert re-flows when the grid changed: the layout itself, the
            // icon size that scales it, or a Match Explorer toggle, which
            // imports Explorer's own icon size and spacing.
            let relayout = settings::needs_relayout(&session, &self.config);
            session.revert(&mut self.config);
            ui.set_ui_dark(self.ui_dark());
            if restyle {
                self.restyle_all(ui);
            }
            if relayout {
                self.relayout_all(ui);
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
            Event::PointerDownOutside { on_desktop } => {
                self.dismiss_info_tip(ui);
                if self.edit.is_some() {
                    self.finish_rename(ui, true);
                } else if on_desktop {
                    self.clear_selection(ui);
                }
            }
            Event::ShowSpaces => self.set_visible(ui, true),
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
            Event::Timer { .. } => {}
            Event::SystemThemeChanged => {
                self.system_dark = system_uses_dark_mode();
                self.system_accent = system_accent_color();
                ui.set_ui_dark(self.ui_dark());
                // The desktop view can change its icon size with the theme;
                // refresh its display options, then restyle for dark mode.
                self.refresh_shell_display(ui);
                self.restyle_all(ui);
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
            self.edit.is_none() && self.selected_space.is_some(),
        );
        self.sync_outside_watch(ui);
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    deskspace_platform::enable_per_monitor_awareness();

    // `--show` from the desktop menu (or a shortcut): hand it to a running
    // instance and exit, rather than starting a second copy.
    if cli.show && deskspace_platform::signal_show_spaces() {
        return Ok(());
    }

    let store = match &cli.config {
        Some(path) => ConfigStore::with_path(PathBuf::from(path)),
        None => ConfigStore::discover()
            .context("could not find the per-user configuration directory")?,
    };

    let mut config = if cli.reset {
        if store.exists() {
            match store.backup("reset") {
                Ok(path) => {
                    tracing::info!(backup = %path.display(), "previous configuration backed up")
                }
                Err(error) => {
                    return Err(error).context(
                        "refusing to reset: could not back up the existing configuration",
                    );
                }
            }
        }
        Config::new()
    } else {
        store.load_or_default()
    };

    if config.desktop.spaces.is_empty() {
        seed_default_desktop(&mut config.desktop, !cli.no_icons);
    }

    // `--show` also forces them shown when it started a fresh instance, so a
    // stale desktop entry still works after an unclean exit.
    let hidden = !cli.show && (cli.hidden || config.window.start_hidden);
    let placement = match cli.placement() {
        Some(placement) => placement,
        None if config.window.always_on_desktop => Placement::DesktopBackground,
        None => Placement::TopLevel,
    };
    let options = RunOptions {
        placement,
        click_through: cli.click_through,
        on_top: cli.on_top,
        tray_tooltip: tray_tooltip(&config.desktop),
    };

    let app = App::new(config, store, hidden, cli.settings);
    tracing::info!(
        path = %app.store.path().display(),
        storage = %app.storage.root().display(),
        "starting DeskSpace"
    );
    deskspace_platform::run(options, Box::new(app)).context("the DeskSpace UI runtime failed")?;
    Ok(())
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

/// The "Appearance" submenu for a space: opacity presets, the icon size, custom
/// tint and text colors, and a reset to the theme.
fn space_appearance_menu(space: &Space) -> MenuEntry {
    let mut entries: Vec<MenuEntry> = OPACITY_PRESETS
        .iter()
        .map(|(id, label, value)| MenuEntry::item(*id, *label).checked(space.opacity == *value))
        .collect();
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::submenu(
        "Icon size",
        ICON_SIZE_PRESETS
            .iter()
            .map(|(id, label, value)| {
                MenuEntry::item(*id, *label).checked(space.icon_size == *value)
            })
            .collect(),
    ));
    entries.push(MenuEntry::Separator);
    entries.push(
        MenuEntry::item(menu::SPACE_TINT, "Custom tint\u{2026}").checked(space.tint.is_some()),
    );
    entries.push(
        MenuEntry::item(menu::SPACE_TEXT, "Custom text\u{2026}").checked(space.text.is_some()),
    );
    entries.push(MenuEntry::item(menu::RESET_APPEARANCE, "Reset appearance"));
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

/// Whether `clipboard` is the cut DeskSpace put there: the same paths in any
/// order, still marked as a cut.
fn cut_matches_clipboard(clipboard: &ClipboardFiles, cut: &[PathBuf]) -> bool {
    clipboard.cut
        && clipboard.paths.len() == cut.len()
        && cut.iter().all(|path| {
            clipboard
                .paths
                .iter()
                .any(|clip| crate::storage::same_path(clip, path))
        })
}

/// The selection a rubber-band rectangle produces: `base` when additive, plus
/// every item whose cell intersects `rect`.
fn marquee_selection(
    cells: &[(ItemId, Rect)],
    rect: Rect,
    additive: bool,
    base: &[ItemId],
) -> Vec<ItemId> {
    let mut selection = if additive { base.to_vec() } else { Vec::new() };
    for (item, cell) in cells {
        if cell.intersects(rect) && !selection.contains(item) {
            selection.push(*item);
        }
    }
    selection
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

/// What a paste does with each clipboard path: change an existing item's
/// space, move an external file into the store, or copy a file into the store.
#[derive(Debug, Default, PartialEq, Eq)]
struct PastePlan {
    moved: Vec<ItemId>,
    external_moves: Vec<PathBuf>,
    copies: Vec<PathBuf>,
}

fn plan_paste(paths: &[PathBuf], cut: bool, known: impl Fn(&Path) -> Option<ItemId>) -> PastePlan {
    let mut plan = PastePlan::default();
    for path in paths {
        if !path.exists() {
            continue;
        }
        match (known(path), cut) {
            (Some(item), true) => plan.moved.push(item),
            (Some(_), false) => plan.copies.push(path.clone()),
            (None, true) => plan.external_moves.push(path.clone()),
            (None, false) => plan.copies.push(path.clone()),
        }
    }
    plan
}

fn tray_tooltip(desktop: &Desktop) -> String {
    let spaces = desktop.spaces.len();
    let items = desktop.item_count();
    let space_word = if spaces == 1 { "space" } else { "spaces" };
    let item_word = if items == 1 { "item" } else { "items" };
    format!("DeskSpace \u{2014} {spaces} {space_word}, {items} {item_word}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_checks_are_the_operand() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        app.shell_display = ShellDisplay {
            check_boxes: CheckBoxMode::Independent,
            ..ShellDisplay::default()
        };
        app.selected_items = vec![ItemId(1)];
        app.checked_items = vec![ItemId(2), ItemId(3)];
        assert_eq!(app.active_items(), vec![ItemId(2), ItemId(3)]);
        // With nothing ticked the selection is used.
        app.checked_items.clear();
        assert_eq!(app.active_items(), vec![ItemId(1)]);
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
            app.checkbox_at(id, Point::new(checkbox.x + 3, checkbox.y + 3)),
            Some(ItemId(1))
        );
        assert_eq!(
            app.checkbox_at(id, Point::new(cell.x + 60, cell.y + 60)),
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
        let cells = app.item_cells(id, &[ItemId(2), ItemId(9), ItemId(1)]);
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
        app.selected_space = Some(id);

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

    fn app_with_selection(space: Option<SpaceId>, item: Option<ItemId>) -> App {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        app.selected_space = space;
        app.selected_items = item.into_iter().collect();
        app
    }

    #[test]
    fn ctrl_click_toggles_and_shift_click_selects_a_range() {
        use deskspace_core::Item;
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = app.config.desktop.add_space("S", Rect::new(0, 0, 400, 400));
        for n in 1..=4u64 {
            app.config
                .desktop
                .add_item(id, Item::new(ItemId(n), format!("C:/missing/{n}.txt")));
        }
        app.selected_space = Some(id);
        // Ctrl+click builds a selection and toggles items off again.
        app.toggle_selected(ItemId(1));
        app.toggle_selected(ItemId(3));
        assert_eq!(app.selected_items, vec![ItemId(1), ItemId(3)]);
        app.toggle_selected(ItemId(1));
        assert_eq!(app.selected_items, vec![ItemId(3)]);
        // Shift+click extends from the anchor across the space's order.
        app.selection_anchor = Some(ItemId(1));
        app.extend_selection(id, ItemId(4));
        assert_eq!(
            app.selected_items,
            vec![ItemId(1), ItemId(2), ItemId(3), ItemId(4)]
        );
    }

    #[test]
    fn taking_a_selection_reports_the_space_to_repaint() {
        let mut app = app_with_selection(Some(SpaceId(2)), Some(ItemId(5)));
        assert_eq!(app.take_selection(), Some(SpaceId(2)));
        assert_eq!((app.selected_space, app.selected_items.len()), (None, 0));
        // Nothing is left to clear, so nothing needs repainting.
        assert_eq!(app.take_selection(), None);
    }

    #[test]
    fn a_space_without_a_selected_item_needs_no_repaint() {
        let mut app = app_with_selection(Some(SpaceId(2)), None);
        assert_eq!(app.take_selection(), None);
        assert_eq!(app.selected_space, None);
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
    fn cut_matches_only_the_same_cut_files() {
        let cut = vec![PathBuf::from("C:/A/B.txt")];
        let matching = ClipboardFiles {
            paths: vec![PathBuf::from("c:/a/b.TXT")],
            cut: true,
        };
        assert!(cut_matches_clipboard(&matching, &cut));
        assert!(!cut_matches_clipboard(
            &ClipboardFiles {
                cut: false,
                ..matching.clone()
            },
            &cut
        ));
        assert!(!cut_matches_clipboard(
            &ClipboardFiles {
                paths: vec![PathBuf::from("C:/A/other.txt")],
                cut: true,
            },
            &cut
        ));
    }

    #[test]
    fn marquee_selects_intersecting_cells() {
        let cells = [
            (ItemId(1), Rect::new(0, 0, 10, 10)),
            (ItemId(2), Rect::new(20, 0, 10, 10)),
            (ItemId(3), Rect::new(0, 20, 10, 10)),
        ];
        assert_eq!(
            marquee_selection(&cells, Rect::new(5, 5, 20, 20), false, &[]),
            vec![ItemId(1), ItemId(2), ItemId(3)]
        );
        // A rectangle away from every cell selects nothing.
        assert!(marquee_selection(&cells, Rect::new(100, 100, 5, 5), false, &[]).is_empty());
        // Additive keeps the base selection and appends new hits.
        assert_eq!(
            marquee_selection(&cells, Rect::new(0, 0, 6, 6), true, &[ItemId(2)]),
            vec![ItemId(2), ItemId(1)]
        );
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

    #[test]
    fn plan_paste_classifies_known_and_external_paths() {
        let name = |suffix: &str| {
            let path = std::env::temp_dir()
                .join(format!("deskspace-paste-{suffix}-{}", std::process::id()));
            std::fs::write(&path, "x").unwrap();
            path
        };
        let external = name("external.txt");
        let known = name("known.txt");
        let known_id = ItemId(7);
        let paths = vec![external.clone(), known.clone()];
        let lookup = |path: &Path| (known.as_path() == path).then_some(known_id);

        let copy = plan_paste(&paths, false, lookup);
        assert_eq!(copy.copies, paths);
        assert!(copy.moved.is_empty() && copy.external_moves.is_empty());

        let cut = plan_paste(&paths, true, lookup);
        assert_eq!(cut.external_moves, vec![external.clone()]);
        assert_eq!(cut.moved, vec![known_id]);
        assert!(cut.copies.is_empty());

        let _ = std::fs::remove_file(&external);
        let _ = std::fs::remove_file(&known);
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
