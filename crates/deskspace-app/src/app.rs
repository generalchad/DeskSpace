use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use deskspace_config::{Config, ConfigStore};
use deskspace_core::geometry::Point;
use deskspace_core::{Command, Desktop, spaceId, ItemId, Rect, ResizeEdges};
use deskspace_platform::{
    CursorShape, Event, EventHandler, spaceEvent, Hotkey, IconCache, MenuEntry, MouseButton,
    RunOptions, SettingsEvent, TrayEvent, Ui, hotkey_id, parse_hotkey, system_uses_dark_mode,
    work_area_at,
};

use crate::bootstrap::seed_default_desktop;
use crate::cli::Cli;
use crate::scene::{ViewState, space_scene, space_style};
use crate::settings::{self, Effect, Session};

const TOGGLE_HOTKEY: i32 = 0;
/// Width of the band along a space's edges that starts a resize.
const RESIZE_BORDER: i32 = 6;
/// `WHEEL_DELTA`: one notch of a standard mouse wheel.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
const WHEEL_DELTA: i32 = 120;

mod menu {
    pub const ROLL: u32 = 1;
    pub const LOCK: u32 = 2;
    pub const NEW_space: u32 = 3;
    pub const SETTINGS: u32 = 4;
    pub const TOGGLE_VISIBLE: u32 = 5;
    pub const EXIT: u32 = 6;
    pub const DELETE: u32 = 7;
    pub const FIT: u32 = 8;
}

enum Drag {
    None,
    space { id: spaceId, grab: Point },
    Resize { id: spaceId, edges: ResizeEdges },
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
    icons: IconCache,
    system_dark: bool,
    selected_space: Option<spaceId>,
    selected_item: Option<ItemId>,
    hovered: Option<(spaceId, ItemId)>,
    drag: Drag,
    /// Per-space scroll offset in pixels. Not persisted; may exceed the
    /// current maximum after a resize, so read it through `scroll_of`.
    scroll: HashMap<spaceId, i32>,
    visible: bool,
    hotkey: Option<Hotkey>,
    settings: Option<Session>,
    open_settings_on_start: bool,
}

impl App {
    pub fn new(config: Config, store: ConfigStore, hidden: bool, open_settings: bool) -> Self {
        Self {
            config,
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
        let ids: Vec<spaceId> = self.config.desktop.spaces.iter().map(|f| f.id).collect();
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

    fn view_for(&self, id: spaceId) -> ViewState {
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
    fn scroll_of(&self, id: spaceId) -> i32 {
        let layout = self.config.desktop.layout;
        let max = self
            .config
            .desktop
            .space(id)
            .map_or(0, |space| space.max_scroll(layout.cell(), layout.padding));
        self.scroll.get(&id).copied().unwrap_or(0).clamp(0, max)
    }

    fn item_at(&self, id: spaceId, point: Point) -> Option<ItemId> {
        let cell = self.config.desktop.layout.cell();
        let scroll = self.scroll_of(id);
        self.config
            .desktop
            .space(id)
            .and_then(|space| space.item_at(point, cell, scroll))
    }

    /// Pushes the space's current geometry and draw list to its window.
    fn refresh_space(&mut self, ui: &mut Ui, id: spaceId) {
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
        let ids: Vec<spaceId> = self.config.desktop.spaces.iter().map(|f| f.id).collect();
        for id in ids {
            ui.set_space_style(id.0, style);
            self.refresh_space(ui, id);
        }
    }

    fn refresh_tray(&self, ui: &mut Ui) {
        ui.set_tray_tooltip(&tray_tooltip(&self.config.desktop));
    }

    fn set_visible(&mut self, ui: &mut Ui, visible: bool) {
        self.visible = visible;
        ui.set_spaces_visible(visible);
        tracing::info!(visible, "toggled space visibility");
    }

    // ---- pointer -------------------------------------------------------

    fn on_space_event(&mut self, ui: &mut Ui, id: spaceId, event: spaceEvent) {
        match event {
            spaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => self.begin_interaction(ui, id, Point::new(x, y)),
            spaceEvent::MouseMove { x, y } => {
                let point = Point::new(x, y);
                if !self.drag_to(ui, point) {
                    self.update_cursor(ui, id, point);
                    self.update_hover(ui, id, point);
                }
            }
            spaceEvent::Wheel { x, y, delta } => self.scroll_by(ui, id, Point::new(x, y), delta),
            spaceEvent::MouseUp {
                button: MouseButton::Left,
                ..
            }
            | spaceEvent::CaptureLost => self.end_drag(),
            spaceEvent::MouseUp {
                x,
                y,
                button: MouseButton::Right,
            } => self.show_space_menu(ui, id, Point::new(x, y)),
            spaceEvent::DoubleClick { x, y } => {
                let on_title = self
                    .config
                    .desktop
                    .space(id)
                    .is_some_and(|space| space.title_bar().contains(Point::new(x, y)));
                if on_title {
                    self.toggle_rollup(ui, id);
                }
            }
            spaceEvent::MouseLeave if self.hovered.is_some_and(|(space, _)| space == id) => {
                self.hovered = None;
                self.refresh_space(ui, id);
            }
            spaceEvent::FilesDropped { x, y, paths } => {
                self.add_dropped(ui, id, Point::new(x, y), paths);
            }
            _ => {}
        }
    }

    fn begin_interaction(&mut self, ui: &mut Ui, id: spaceId, point: Point) {
        let previous = self.selected_space.replace(id);
        self.selected_item = self.item_at(id, point);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let edges = space.resize_edges_at(point, RESIZE_BORDER);
        if edges.any() {
            self.drag = Drag::Resize { id, edges };
            self.selected_item = None;
        } else if !space.locked && space.title_bar().contains(point) {
            self.drag = Drag::space {
                id,
                grab: Point::new(point.x - space.rect.x, point.y - space.rect.y),
            };
        }
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            self.refresh_space(ui, previous);
        }
        self.refresh_space(ui, id);
    }

    fn drag_to(&mut self, ui: &mut Ui, point: Point) -> bool {
        match self.drag {
            Drag::None => false,
            Drag::space { id, grab } => {
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
                if self.dispatch(Command::SetspaceRect { id, rect }, point) {
                    if let Some(space) = self.config.desktop.space(id) {
                        ui.set_space_rect(id.0, space.visible_rect());
                    }
                }
                true
            }
            Drag::Resize { id, edges } => {
                let before = self.config.desktop.space(id).map(|space| space.rect);
                let command = Command::Resizespace {
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

    fn update_cursor(&self, ui: &mut Ui, id: spaceId, point: Point) {
        let edges = self
            .config
            .desktop
            .space(id)
            .map_or(ResizeEdges::NONE, |space| {
                space.resize_edges_at(point, RESIZE_BORDER)
            });
        ui.set_space_cursor(id.0, cursor_for(edges));
    }

    fn scroll_by(&mut self, ui: &mut Ui, id: spaceId, point: Point, delta: i32) {
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
        if !matches!(self.drag, Drag::None) {
            self.drag = Drag::None;
            self.save();
        }
    }

    fn update_hover(&mut self, ui: &mut Ui, id: spaceId, point: Point) {
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

    fn toggle_rollup(&mut self, ui: &mut Ui, id: spaceId) {
        let near = self.space_center(id);
        if self.dispatch(Command::ToggleRollup { id }, near) {
            self.refresh_space(ui, id);
            self.save();
        }
    }

    /// Adds dropped files to `target`. A path already shown in another space
    /// moves there instead of appearing twice. Files themselves are not
    /// touched; moving them into a managed store is roadmap item D3/M2.
    fn add_dropped(&mut self, ui: &mut Ui, target: spaceId, at: Point, paths: Vec<PathBuf>) {
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
                    path,
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

    fn delete_space(&mut self, ui: &mut Ui, id: spaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let count = space.items.len();
        let question = format!(
            "Delete the space \"{}\"?\n\nIts {} {} will be removed from DeskSpace. \
             The files themselves are not deleted.",
            space.title,
            count,
            if count == 1 { "item" } else { "items" }
        );
        if !ui.confirm("Delete space", &question) {
            return;
        }
        let near = self.space_center(id);
        if self.dispatch(Command::Deletespace { id }, near) {
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

    fn space_center(&self, id: spaceId) -> Point {
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

    fn show_space_menu(&mut self, ui: &mut Ui, id: spaceId, at: Point) {
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
        entries.extend([
            MenuEntry::item(menu::DELETE, "Delete space\u{2026}"),
            MenuEntry::Separator,
            MenuEntry::item(menu::NEW_space, "New space"),
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
            MenuEntry::item(menu::NEW_space, "New space"),
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
            menu::NEW_space => self.create_space(ui, at),
            menu::SETTINGS => self.open_settings(ui),
            menu::TOGGLE_VISIBLE => self.set_visible(ui, !self.visible),
            menu::EXIT => ui.quit(),
            _ => {}
        }
    }

    fn create_space(&mut self, ui: &mut Ui, at: Point) {
        let command = Command::Createspace {
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
            Event::space { id, event } => self.on_space_event(ui, spaceId(id), event),
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
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    deskspace_platform::enable_per_monitor_awareness();

    let store = match &cli.config {
        Some(path) => ConfigStore::with_path(PathBuf::from(path)),
        None => ConfigStore::discover()?,
    };

    let mut config = if cli.reset {
        if store.exists() {
            match store.backup("reset") {
                Ok(path) => {
                    tracing::info!(backup = %path.display(), "previous configuration backed up")
                }
                Err(error) => {
                    anyhow::bail!(
                        "refusing to reset: could not back up the existing configuration: {error}"
                    )
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
    tracing::info!(path = %app.store.path().display(), "starting DeskSpace");
    deskspace_platform::run(options, Box::new(app))?;
    Ok(())
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
