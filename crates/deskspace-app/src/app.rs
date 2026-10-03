use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::Context;
use deskspace_config::{Config, ConfigStore};
use deskspace_core::geometry::{Point, Size};
use deskspace_core::{Command, Desktop, ItemId, Rect, ResizeEdges, Space, SpaceId};
use deskspace_platform::{
    CursorShape, Event, EventHandler, Hotkey, IconCache, ItemMenuChoice, MenuEntry, MouseButton,
    RunOptions, SettingsEvent, SpaceEvent, TrayEvent, Ui, desktop_directory, drag_threshold,
    hotkey_id, parse_hotkey, system_uses_dark_mode, work_area_at,
};

use crate::bootstrap::seed_default_desktop;
use crate::cli::Cli;
use crate::scene::{ViewState, space_scene, space_style};
use crate::settings::{self, Effect, Session};
use crate::storage::ItemStore;

const TOGGLE_HOTKEY: i32 = 0;
/// Width of the band along a space's edges that starts a resize.
const RESIZE_BORDER: i32 = 6;
/// `WHEEL_DELTA`: one notch of a standard mouse wheel.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
const WHEEL_DELTA: i32 = 120;

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
    /// The left button went down on an item; it becomes a shell drag once
    /// the pointer moves past the system drag threshold.
    Item {
        space: SpaceId,
        item: ItemId,
        origin: Point,
    },
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
    selected_space: Option<SpaceId>,
    selected_item: Option<ItemId>,
    hovered: Option<(SpaceId, ItemId)>,
    drag: Drag,
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
            icons: IconCache::new(32),
            system_dark: system_uses_dark_mode(),
            selected_space: None,
            selected_item: None,
            hovered: None,
            drag: Drag::None,
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
        self.config.desktop.apply(command, bounds).is_applied()
    }

    fn save(&self) {
        if let Err(error) = self.store.save(&self.config) {
            tracing::error!(%error, "failed to persist configuration");
        } else {
            tracing::debug!(path = %self.store.path().display(), "configuration saved");
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
            selected: self
                .selected_item
                .filter(|_| self.selected_space == Some(id)),
            scroll: self.scroll_of(id),
        }
    }

    /// The space's scroll offset, clamped to its current content.
    fn scroll_of(&self, id: SpaceId) -> i32 {
        let layout = self.config.desktop.layout;
        let max = self
            .config
            .desktop
            .space(id)
            .map_or(0, |space| space.max_scroll(layout.cell(), layout.padding));
        self.scroll.get(&id).copied().unwrap_or(0).clamp(0, max)
    }

    fn item_at(&self, id: SpaceId, point: Point) -> Option<ItemId> {
        let cell = self.config.desktop.layout.cell();
        let scroll = self.scroll_of(id);
        self.config
            .desktop
            .space(id)
            .and_then(|space| space.item_at(point, cell, scroll))
    }

    /// Pushes the space's current geometry and draw list to its window.
    fn refresh_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let view = self.view_for(id);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let palette = self.config.theme.palette(self.system_dark);
        let scene = space_scene(
            space,
            &self.config.theme,
            &palette,
            self.config.desktop.layout,
            view,
            &mut self.icons,
        );
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

    fn refresh_tray(&self, ui: &mut Ui) {
        ui.set_tray_tooltip(&tray_tooltip(&self.config.desktop));
    }

    /// Clears the item selection, as clicking elsewhere does in Explorer.
    /// Returns the space that showed it, which needs a new scene.
    fn take_selection(&mut self) -> Option<SpaceId> {
        let space = self.selected_space.take();
        self.selected_item.take().and(space)
    }

    fn clear_selection(&mut self, ui: &mut Ui) {
        if let Some(space) = self.take_selection() {
            self.refresh_space(ui, space);
        }
    }

    /// Asks for outside clicks only while there is a selection for them to
    /// clear, because watching them hooks every mouse event in the session.
    fn sync_outside_watch(&mut self, ui: &mut Ui) {
        let wanted = self.selected_item.is_some();
        if wanted != self.watching_outside {
            ui.watch_outside_clicks(wanted);
            self.watching_outside = wanted;
        }
    }

    fn set_visible(&mut self, ui: &mut Ui, visible: bool) {
        if !visible {
            self.clear_selection(ui);
        }
        self.visible = visible;
        ui.set_spaces_visible(visible);
        tracing::info!(visible, "toggled space visibility");
    }

    // ---- pointer -------------------------------------------------------

    fn on_space_event(&mut self, ui: &mut Ui, id: SpaceId, event: SpaceEvent) {
        match event {
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => self.begin_interaction(ui, id, Point::new(x, y)),
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
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
            | SpaceEvent::CaptureLost => self.end_drag(),
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
            SpaceEvent::MouseLeave if self.hovered.is_some_and(|(space, _)| space == id) => {
                self.hovered = None;
                self.refresh_space(ui, id);
            }
            SpaceEvent::FilesDropped {
                x,
                y,
                paths,
                keep_in_place,
            } => {
                self.add_dropped(ui, id, Point::new(x, y), paths, keep_in_place);
            }
            _ => {}
        }
    }

    fn begin_interaction(&mut self, ui: &mut Ui, id: SpaceId, point: Point) {
        let previous = self.selected_space.replace(id);
        self.selected_item = self.item_at(id, point);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let edges = space.resize_edges_at(point, RESIZE_BORDER);
        if edges.any() {
            self.drag = Drag::Resize { id, edges };
            self.selected_item = None;
        } else if let Some(item) = self.selected_item {
            self.drag = Drag::Item {
                space: id,
                item,
                origin: point,
            };
        } else if !space.locked && space.title_bar().contains(point) {
            self.drag = Drag::Space {
                id,
                grab: Point::new(point.x - space.rect.x, point.y - space.rect.y),
            };
        }
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            self.refresh_space(ui, previous);
        }
        self.refresh_space(ui, id);
    }

    /// Selects the item under `point` (or nothing) without starting a drag,
    /// as a right-click does before its menu opens.
    fn select_at(&mut self, ui: &mut Ui, id: SpaceId, point: Point) {
        let previous = self.selected_space.replace(id);
        self.selected_item = self.item_at(id, point);
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
            } => {
                let threshold = drag_threshold();
                if (point.x - origin.x).abs() >= threshold.width
                    || (point.y - origin.y).abs() >= threshold.height
                {
                    self.drag = Drag::None;
                    self.drag_item_out(ui, space, item);
                }
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
        let layout = self.config.desktop.layout;
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

    fn end_drag(&mut self) {
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::None | Drag::Item { .. } => {}
            Drag::Space { .. } | Drag::Resize { .. } => self.save(),
        }
    }

    fn update_hover(&mut self, ui: &mut Ui, id: SpaceId, point: Point) {
        let hovered = self.item_at(id, point).map(|item| (id, item));
        if hovered == self.hovered {
            return;
        }
        let previous = std::mem::replace(&mut self.hovered, hovered);
        if let Some((space, _)) = previous.filter(|(space, _)| *space != id) {
            self.refresh_space(ui, space);
        }
        self.refresh_space(ui, id);
    }

    fn toggle_rollup(&mut self, ui: &mut Ui, id: SpaceId) {
        let near = self.space_center(id);
        if self.dispatch(Command::ToggleRollup { id }, near) {
            // A rolled-up space hides its items, so it cannot keep one
            // selected.
            if self.selected_space == Some(id) {
                self.take_selection();
            }
            self.refresh_space(ui, id);
            self.save();
        }
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
            let command = match self.config.desktop.item_with_path(&path) {
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
                None => Command::AddItem {
                    space: target,
                    path: self.place_dropped(path, keep_in_place),
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
    /// it already is.
    fn place_dropped(&self, path: PathBuf, keep_in_place: bool) -> PathBuf {
        if keep_in_place {
            return path;
        }
        match self.storage.adopt(&path) {
            Ok(stored) => {
                tracing::info!(from = %path.display(), to = %stored.display(), "moved into space storage");
                stored
            }
            Err(error) => {
                tracing::info!(%error, "keeping dropped file in place");
                path
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
            if self.selected_item == Some(item) {
                self.selected_item = None;
            }
            if self.hovered.is_some_and(|(_, hovered)| hovered == item) {
                self.hovered = None;
            }
        }
        self.refresh_space(ui, id);
        self.refresh_tray(ui);
        self.save();
    }

    fn double_click(&mut self, ui: &mut Ui, id: SpaceId, point: Point) {
        let layout = self.config.desktop.layout;
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

    /// Drags an item out like Explorer. Wherever it lands, the drop target
    /// decides what happens to the file; afterwards the item leaves the space
    /// if the file is gone from its path (moved elsewhere) or it was dropped
    /// on the desktop. Drops on spaces arrive as `FilesDropped` after this
    /// returns and move the item between spaces.
    fn drag_item_out(&mut self, ui: &mut Ui, space: SpaceId, item: ItemId) {
        let Some(path) = self.config.desktop.item(item).map(|item| item.path.clone()) else {
            return;
        };
        let Some(result) = ui.drag_out(&path) else {
            return;
        };
        tracing::debug!(?result, path = %path.display(), "item drag ended");
        if !path.exists() {
            self.prune_missing(ui, space);
        } else if result.over_desktop && self.release_item(item) {
            self.selected_item = None;
            self.refresh_space(ui, space);
            self.refresh_tray(ui);
            self.save();
        }
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
        let entries = [MenuEntry::item(menu::REMOVE_ITEM, remove_label)];
        match ui.item_menu(&path, &entries, at) {
            Some(ItemMenuChoice::App(menu::REMOVE_ITEM)) => {
                if self.release_item(item) {
                    self.selected_item = None;
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
                self.selected_item = None;
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
            MenuEntry::item(menu::LOCK, "Lock position").checked(space.locked),
        ];
        if space.user_sized && !space.locked {
            entries.push(MenuEntry::item(menu::FIT, "Fit to contents"));
        }
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
            Some(menu::DELETE) => self.delete_space(ui, id),
            Some(menu::FIT) => {
                if self.dispatch(Command::FitToContents { id }, at) {
                    self.scroll.remove(&id);
                    self.refresh_space(ui, id);
                    self.save();
                }
            }
            Some(menu::LOCK) => {
                if self.dispatch(Command::ToggleLock { id }, at) {
                    self.refresh_space(ui, id);
                    self.save();
                }
            }
            Some(choice) => self.run_common_menu(ui, choice, at),
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
            self.save();
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
                    self.restyle_all(ui);
                }
                self.refresh_settings(ui);
            }
            SettingsEvent::Button {
                field: settings::field::RESET,
            } => {
                settings::reset_appearance(&mut self.config, session);
                self.restyle_all(ui);
                self.refresh_settings(ui);
            }
            SettingsEvent::Button { .. } => {}
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
            session.revert(&mut self.config);
            if restyle {
                self.restyle_all(ui);
            }
        }
    }
}

impl EventHandler for App {
    fn on_event(&mut self, event: Event, ui: &mut Ui) {
        match event {
            Event::Started => {
                // DeskSpace's own menus and settings window follow Windows'
                // app mode; the spaces follow their own Color mode setting.
                ui.set_ui_dark(self.system_dark);
                let hotkey = self.config.window.toggle_hotkey.clone();
                if let Err(message) = self.register_hotkey(ui, &hotkey) {
                    tracing::warn!(message, "toggle hotkey unavailable");
                }
                ui.set_spaces_visible(self.visible);
                self.sync_windows(ui);
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
            Event::PointerDownOutside => self.clear_selection(ui),
            Event::Hotkey { id } if id == hotkey_id(TOGGLE_HOTKEY) => {
                self.set_visible(ui, !self.visible);
            }
            Event::Hotkey { .. } => {}
            Event::Tray(TrayEvent::Activated) => self.open_settings(ui),
            Event::Tray(TrayEvent::MenuRequested { x, y }) => {
                self.show_tray_menu(ui, Point::new(x, y));
            }
            Event::Settings(event) => self.on_settings_event(ui, event),
            Event::SystemThemeChanged => {
                self.system_dark = system_uses_dark_mode();
                ui.set_ui_dark(self.system_dark);
                self.restyle_all(ui);
                self.refresh_settings(ui);
            }
            Event::CloseRequested => {
                if let Some(hotkey) = self.hotkey.take() {
                    ui.unregister_hotkey(hotkey);
                }
                // Unapplied edits in an open settings window are discarded.
                self.cancel_settings(ui);
                self.save();
            }
        }
        self.sync_outside_watch(ui);
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    deskspace_platform::enable_per_monitor_awareness();

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

    let hidden = cli.hidden || config.window.start_hidden;
    let options = RunOptions {
        placement: cli.orientation(),
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
        app.selected_item = item;
        app
    }

    #[test]
    fn taking_a_selection_reports_the_space_to_repaint() {
        let mut app = app_with_selection(Some(SpaceId(2)), Some(ItemId(5)));
        assert_eq!(app.take_selection(), Some(SpaceId(2)));
        assert_eq!((app.selected_space, app.selected_item), (None, None));
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
