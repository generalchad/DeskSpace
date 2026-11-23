//! `App` view: space windows, scenes, layout, and the desktop layer.

mod cache;
mod layer;
mod look;
mod system_look;

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use deskspace_core::animation::Kind;
use deskspace_core::geometry::{Point, Rect};
use deskspace_core::grouping::Grouping;
use deskspace_core::{
    Command, Extensions, Item, ItemId, ItemKind, Space, SpaceId, SpaceView, ViewMode,
};
use deskspace_platform::{
    CheckBoxMode, ITEM_ICON_SIZE, SpaceScene, Ui, Way, local_today, space_dpi, type_name,
    work_area_at,
};

use crate::scene::{SurfaceLook, ViewState, space_scene_with_details, view_cells};
use crate::settings::view_for_digit;

use super::animation::Arrival;
use super::rename::EditTarget;
use super::{App, IconUser, Surface};
use cache::{CachedGrouping, grouping_stamp};

pub(crate) use cache::ViewCache;
pub(crate) use layer::LayerState;
pub(crate) use system_look::SystemLook;

impl App {
    /// Creates windows for new spaces and destroys windows of removed ones.
    pub(super) fn sync_windows(&mut self, ui: &mut Ui) {
        self.sync_windows_as(ui, Arrival::New);
    }

    /// `sync_windows` at startup, when every space appears in the startup
    /// order (ROADMAP D12).
    pub(super) fn sync_windows_at_startup(&mut self, ui: &mut Ui) {
        self.sync_windows_as(ui, Arrival::Startup);
    }

    /// Creates a window for every space that has none, brings each in with
    /// the Startup animation once its scene is pushed, and destroys the
    /// windows of spaces that are gone.
    fn sync_windows_as(&mut self, ui: &mut Ui, arrival: Arrival) {
        let existing: HashSet<u64> = ui.space_ids().into_iter().collect();
        let ids: Vec<SpaceId> = self.config.desktop.spaces().iter().map(|f| f.id).collect();
        let mut arrived = Vec::new();
        for id in &ids {
            if existing.contains(&id.0) {
                continue;
            }
            let Some(rect) = self.config.desktop.space(*id).map(Space::visible_rect) else {
                continue;
            };
            if let Err(error) = ui.create_space(id.0, rect, self.style_for(*id)) {
                tracing::error!(%error, space = id.0, "could not create space window");
                continue;
            }
            self.refresh_space(ui, *id);
            arrived.push((*id, rect));
        }
        for (id, play) in self.arrival_plays(&arrived, arrival) {
            ui.show_space(id.0, play);
        }
        let delete = self.play(Kind::Delete, Way::Out);
        for stale in existing.difference(&ids.iter().map(|id| id.0).collect()) {
            ui.destroy_space(*stale, delete);
        }
        self.refresh_tray(ui);
    }

    pub(super) fn view_for(&self, surface: Surface) -> ViewState {
        let check_boxes = self.surface_check_box_mode(surface);
        ViewState {
            grouping: match surface {
                Surface::Space(id) => self
                    .config
                    .desktop
                    .space(id)
                    .and_then(|space| self.shown_grouping(space)),
                Surface::Desktop => None,
            },
            column_drop: self.column_drop_on(surface),
            hovered: self.pointer.hovered_item_on(surface),
            selected: if self.selection.surface == Some(surface) {
                self.selection.items.clone()
            } else {
                Vec::new()
            },
            dragged: self.pointer.dragged_on(surface),
            scroll: self.surface_scroll(surface),
            scroll_x: match surface {
                Surface::Space(id) => self.view_cache.scroll_x_of(id),
                Surface::Desktop => 0,
            },
            show_check_boxes: check_boxes != CheckBoxMode::Hidden,
            check_boxes_persistent: check_boxes == CheckBoxMode::Independent,
            checked: self.selection.checked.clone(),
            show_extensions: self.shows_extensions(surface),
            show_compressed_color: self.surface_match_explorer(surface)
                && self.system.shell().show_compressed_color,
            cut: self.selection.cut.iter().copied().collect(),
            marquee: self.pointer.marquee_on(surface).and_then(|rect| {
                let origin = self.surface_window_rect(surface)?.origin();
                Some(Rect::new(
                    rect.x - origin.x,
                    rect.y - origin.y,
                    rect.width,
                    rect.height,
                ))
            }),
            highlight: self
                .settings_ui
                .highlighted()
                .is_some_and(|id| surface == Surface::Space(id)),
            highlight_fades: self.animations_play(),
            focused: (self.selection.surface == Some(surface))
                .then_some(self.selection.focused)
                .flatten(),
            folder_unavailable: match surface {
                Surface::Space(id) => self.unavailable_lenses.contains(&id),
                Surface::Desktop => false,
            },
            filtered_out: match surface {
                Surface::Space(id) => self.filtered_out(id),
                Surface::Desktop => false,
            },
            subfolder: match surface {
                Surface::Space(id) => self.subfolder_of(id),
                Surface::Desktop => PathBuf::new(),
            },
            hovered_status: self
                .pointer
                .status_hover
                .filter(|(owner, _)| *owner == surface)
                .map(|(_, index)| index),
            notice: match surface {
                Surface::Space(id) => self.notices.get(&id).cloned(),
                Surface::Desktop => None,
            },
        }
    }

    /// The display order `space` is drawn in, if it groups its items.
    fn shown_grouping(&self, space: &Space) -> Option<Arc<Grouping>> {
        space.view().group?;
        self.view_cache.grouping(space.id)
    }

    /// Brings a grouped space's display order up to date, and drops from the
    /// selection any item that is now folded, so a delete never acts on files
    /// the user cannot see (D18). A repaint reaches here after every change to
    /// a space, so date groups move on at the next one after midnight.
    pub(super) fn sync_grouping(&mut self, id: SpaceId) {
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let Some(group) = space.view().group else {
            self.view_cache.groupings.remove(&id);
            return;
        };
        let today = local_today();
        let no_folds = HashSet::new();
        let folded = self.view_cache.folded_groups.get(&id).unwrap_or(&no_folds);
        let stamp = grouping_stamp(space.items(), group, today, folded);
        if self
            .view_cache
            .groupings
            .get(&id)
            .is_some_and(|cached| cached.stamp == stamp)
        {
            return;
        }
        // One shell lookup per extension: this walks every item of a lens on the
        // UI thread, and `type_name` builds its cache key for each call.
        let mut by_extension: HashMap<OsString, String> = HashMap::new();
        let mut type_of = |item: &Item| {
            if item.kind == ItemKind::Folder {
                return type_name(&item.path, true).unwrap_or_default();
            }
            let extension = item.path.extension().unwrap_or_default();
            if let Some(name) = by_extension.get(extension) {
                return name.clone();
            }
            let name = type_name(&item.path, false).unwrap_or_default();
            by_extension.insert(extension.to_owned(), name.clone());
            name
        };
        let groups = crate::grouping::groups(
            space.items(),
            group,
            &mut crate::grouping::GroupFacts {
                today,
                type_name: &mut type_of,
            },
        );
        let grouping = Arc::new(Grouping::new(groups, space.items().len(), |name| {
            folded.contains(name)
        }));
        let hidden: Vec<ItemId> = space
            .items()
            .iter()
            .enumerate()
            .filter(|(index, _)| grouping.position_of(*index).is_none())
            .map(|(_, item)| item.id)
            .collect();
        self.view_cache
            .groupings
            .insert(id, CachedGrouping { stamp, grouping });
        if self.selection.surface == Some(Surface::Space(id)) {
            self.selection.drop_items(&hidden);
        }
    }

    /// Folds or opens the group whose header holds `point`, if any.
    pub(super) fn group_header_clicked(&mut self, ui: &mut Ui, id: SpaceId, point: Point) -> bool {
        let Some(name) = self.group_name_at(id, point) else {
            return false;
        };
        self.view_cache.toggle_group(id, &name);
        if let Some(previous) = self.selection.enter(Surface::Space(id)) {
            self.refresh_surface(ui, previous);
        }
        self.refresh_space(ui, id);
        true
    }

    /// The name of the group whose header holds `point`, if any.
    fn group_name_at(&self, id: SpaceId, point: Point) -> Option<String> {
        let space = self.config.desktop.space(id)?;
        let content = space.content_rect();
        if !content.contains(point) {
            return None;
        }
        let grouping = self.shown_grouping(space)?;
        let layout = self.view_layout(id)?;
        let local = layout.layout_point(
            Point::new(point.x - content.x, point.y - content.y),
            self.scroll_of(id),
        );
        let group = layout.group_at(local)?;
        Some(grouping.groups().get(group)?.name.clone())
    }

    /// Repaints a surface; the layer repaints through its own window.
    pub(super) fn refresh_surface(&mut self, ui: &mut Ui, surface: Surface) {
        match surface {
            Surface::Space(id) => self.refresh_space(ui, id),
            Surface::Desktop => self.refresh_desktop_layer(ui),
        }
    }

    /// Whether a surface's labels show filename extensions: the space's own
    /// choice, or under `Automatic` Explorer's own option on a surface
    /// that mirrors Explorer's display.
    pub(super) fn shows_extensions(&self, surface: Surface) -> bool {
        if let Surface::Space(id) = surface
            && let Some(space) = self.config.desktop.space(id)
        {
            match space.view().extensions {
                Extensions::Show => return true,
                Extensions::Hide => return false,
                Extensions::Automatic => {}
            }
        }
        self.surface_match_explorer(surface) && self.system.shell().show_extensions
    }

    /// Pushes the space's current geometry and draw list to its window.
    pub(super) fn refresh_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let started = Instant::now();
        self.sync_grouping(id);
        let view = self.view_for(Surface::Space(id));
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let palette = self.palette_for(id);
        let look = SurfaceLook {
            theme: space.theme(&self.config.theme),
            palette: &palette,
            layout: self.layout_for(id),
            metrics: self.metrics_for(id),
            scale: space_dpi(id.0),
        };
        let mut details = |item: &Item| view_cells(space.view(), item);
        let row_cells = (matches!(
            space.view().mode,
            ViewMode::Details | ViewMode::Tiles | ViewMode::Content
        ))
        .then_some(&mut details as &mut dyn FnMut(&Item) -> Vec<String>);
        let title_icon = self
            .space_icons
            .space_icon(space, palette.title_text, space.title_height);
        self.icons.begin_scene(IconUser::Space(id.0));
        let mut scene =
            space_scene_with_details(space, look, view, row_cells, &mut self.icons, title_icon);
        self.icons.end_scene();
        let rect = space.visible_rect();
        self.show_edit(&mut scene, Surface::Space(id));
        let elapsed = started.elapsed();
        let items = scene.items.len();
        let view = space.view().mode;
        if elapsed > deskspace_platform::SCENE_STEP_BUDGET {
            tracing::debug!(
                space = id.0,
                items,
                view = ?view,
                elapsed_us = elapsed.as_micros(),
                "scene: built"
            );
        } else {
            tracing::trace!(
                space = id.0,
                items,
                view = ?view,
                elapsed_us = elapsed.as_micros(),
                "scene: built"
            );
        }
        ui.set_space_rect(id.0, rect);
        ui.set_space_scene(id.0, scene);
    }

    /// Draws the rename in progress on `surface`, if any, into its scene.
    fn show_edit(&self, scene: &mut SpaceScene, surface: Surface) {
        if let Some(session) = self
            .edit
            .session()
            .filter(|session| session.surface == surface)
        {
            match session.target {
                EditTarget::Title => {
                    scene.editing = Some(session.text.clone());
                    self.show_title_icon_button(scene, surface);
                }
                EditTarget::Item { id: item, .. } => {
                    scene.editing_item = Some((item.0, session.text.clone()));
                }
            }
        }
        scene.caret_visible = self.edit.caret_visible();
    }

    /// Makes the space's title icon a button while its title is renamed: the
    /// picture it has, or the placeholder, with the hover fill when the
    /// pointer is over it (D16, D26).
    fn show_title_icon_button(&self, scene: &mut SpaceScene, surface: Surface) {
        let Surface::Space(id) = surface else {
            return;
        };
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let rect = crate::scene::title_icon_rect_of(space, self.is_browsed(id));
        let image = scene.title_icon.take().and_then(|icon| icon.image);
        scene.title_icon = Some(crate::scene::title_icon_button(
            rect,
            image,
            self.pointer.title_icon_hover == Some(id),
        ));
    }

    /// The work behind `Event::SystemThemeChanged`: re-read Windows' look and
    /// repaint what it changes.
    pub(super) fn on_system_theme_changed(&mut self, ui: &mut Ui) {
        self.system.refresh();
        ui.set_ui_dark(self.ui_dark());
        // The desktop view can change its icon size with the theme;
        // refresh its display options, then restyle for dark mode.
        self.refresh_shell_display(ui);
        self.restyle_all(ui);
        self.refresh_settings(ui);
    }

    /// Re-applies every space's style and repaints them and the desktop
    /// layer.
    pub(super) fn restyle_all(&mut self, ui: &mut Ui) {
        let ids: Vec<SpaceId> = self.config.desktop.spaces().iter().map(|f| f.id).collect();
        for id in ids {
            self.restyle_space(ui, id);
        }
        self.refresh_desktop_layer(ui);
    }

    /// Re-applies one space's window style and repaints it.
    pub(super) fn restyle_space(&mut self, ui: &mut Ui, id: SpaceId) {
        ui.set_space_style(id.0, self.style_for(id));
        self.refresh_space(ui, id);
    }

    /// Re-flows every space for a changed grid (growing spaces resize to fit,
    /// user-sized spaces keep their size and reflow) and repaints the desktop
    /// layer, whose icons share the global scale.
    pub(super) fn relayout_all(&mut self, ui: &mut Ui) {
        let ids: Vec<SpaceId> = self
            .config
            .desktop
            .spaces()
            .iter()
            .map(|space| space.id)
            .collect();
        for id in ids {
            self.reflow_space(ui, id);
        }
        self.refresh_desktop_layer(ui);
    }

    /// Re-flows one space for its current grid (icon size and monitor DPI) and
    /// repaints it.
    pub(super) fn reflow_space(&mut self, ui: &mut Ui, id: SpaceId) {
        self.keep_size_on_zoom(id);
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let bounds = work_area_at(space.rect().center());
        let layout = self.layout_for(id);
        self.config
            .desktop
            .apply_with_layout(Command::Reflow { id }, layout, bounds);
        self.refresh_space(ui, id);
    }

    /// The command that fits space `id` to its contents, with the cells it is
    /// drawn with.
    pub(super) fn fit_command(&self, id: SpaceId) -> Command {
        let space = self.config.desktop.space(id);
        let grouping = space.and_then(|space| self.shown_grouping(space));
        Command::FitToContents {
            id,
            metrics: self.metrics_of(id),
            grouping,
        }
    }

    /// Applies a view choice to space `id` as one undo entry named `label`,
    /// reflows that space alone, and keeps an open settings window's
    /// snapshot. Shared by the View menu, the Ctrl+Shift keys, and the status
    /// bar's buttons.
    pub(super) fn set_space_view_labelled(
        &mut self,
        ui: &mut Ui,
        id: SpaceId,
        view: SpaceView,
        at: Point,
        label: &str,
    ) -> bool {
        let (applied, effects) = self.change_view(id, view, at, label);
        self.run(ui, effects);
        if applied {
            self.view_cache.set_scroll_x(id, 0);
            self.reflow_space(ui, id);
            self.resnapshot_settings(ui, id);
        }
        applied
    }

    /// `set_space_view_labelled` under the name the menus show.
    pub(super) fn set_space_view(&mut self, ui: &mut Ui, id: SpaceId, view: SpaceView, at: Point) {
        self.set_space_view_labelled(ui, id, view, at, "Change view");
    }

    /// The undo-recording half of a view change, without a `Ui`, so tests can
    /// check what an entry captures.
    pub(super) fn change_view(
        &mut self,
        id: SpaceId,
        view: SpaceView,
        at: Point,
        label: &str,
    ) -> (bool, Vec<super::effect::Effect>) {
        self.record_outcome(label, Command::SetSpaceView { id, view }, at, Vec::new())
    }

    /// Ctrl+Shift+1 through 8: switches the current space to the view
    /// its `VIEW_MODES` digit names. A digit with no view yet, or no space
    /// current, does nothing.
    pub(super) fn set_view_by_digit(&mut self, ui: &mut Ui, digit: u8) {
        let Some((id, view)) = self.view_by_digit(digit) else {
            return;
        };
        let at = self.space_center(id);
        self.set_space_view(ui, id, view, at);
    }

    /// The current space and the view Ctrl+Shift+`digit` gives it, without a
    /// `Ui`, so tests can check it.
    fn view_by_digit(&self, digit: u8) -> Option<(SpaceId, SpaceView)> {
        let Some(Surface::Space(id)) = self.selection.surface else {
            return None;
        };
        let mode = view_for_digit(digit)?;
        Some((id, self.config.desktop.space(id)?.view().with_mode(mode)))
    }

    /// When `id`'s icon factor changed since its last re-flow (a zoom, a view
    /// change, the icon-size setting, a preset, or Explorer's own size; never
    /// a DPI move, which the factor leaves out), pins a space that sizes
    /// itself to its current size first. It then re-flows into fewer or more
    /// columns, as an Explorer window does when its icon size changes.
    /// An order-placed view draws a fixed icon size, so it is
    /// never pinned.
    fn keep_size_on_zoom(&mut self, id: SpaceId) {
        let mode = self.view_mode(id);
        let factor = mode.item_icon_px().map_or_else(
            || self.icon_scale(self.theme_for(id)),
            |px| px as f32 / ITEM_ICON_SIZE as f32,
        );
        let previous = self.view_cache.record_icon_scale(id, factor);
        if mode.places_by_order() {
            return;
        }
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        if space.user_sized() || previous.is_none_or(|previous| previous == factor) {
            return;
        }
        let rect = space.rect();
        self.config.desktop.apply_with_layout(
            Command::KeepSize { id },
            self.layout_for(id),
            work_area_at(rect.center()),
        );
        if let Some(session) = self.settings_ui.session_mut() {
            session.pinned.push((id, rect));
        }
    }

    /// A space moved to a monitor with a different DPI: reflow it so icons and
    /// spacing match that display.
    pub(super) fn on_dpi_changed(&mut self, ui: &mut Ui, id: SpaceId) {
        self.reflow_space(ui, id);
        self.refresh_desktop_layer(ui);
        self.schedule_save(ui);
    }

    /// `on_dpi_changed` for a space event, which may name the desktop layer.
    pub(super) fn on_surface_dpi_changed(&mut self, ui: &mut Ui, surface: Surface) {
        if let Surface::Space(id) = surface {
            self.on_dpi_changed(ui, id);
        }
    }

    pub(super) fn refresh_tray(&self, ui: &mut Ui) {
        ui.set_tray_tooltip(&self.tray_text());
    }

    /// Re-reads Explorer's display options and re-flows every space, as F5
    /// refreshes the desktop.
    pub(super) fn refresh_spaces(&mut self, ui: &mut Ui) {
        self.refresh_shell_display(ui);
        self.relayout_all(ui);
    }

    /// Repaints every space without re-reading the shell (for example after a
    /// title icon's set finished loading).
    pub(super) fn repaint_spaces(&mut self, ui: &mut Ui) {
        let ids: Vec<SpaceId> = self
            .config
            .desktop
            .spaces()
            .iter()
            .map(|space| space.id)
            .collect();
        for id in ids {
            self.refresh_space(ui, id);
        }
    }
}

/// The paths of `fresh` items whose file was modified since `known` read it:
/// same path, another modification time.
pub(super) fn changed_in_place<'a>(known: &[Item], fresh: &'a [Item]) -> Vec<&'a Path> {
    let modified: HashMap<&Path, Option<u64>> = known
        .iter()
        .map(|item| (item.path.as_path(), item.modified))
        .collect();
    fresh
        .iter()
        .filter(|item| {
            modified
                .get(item.path.as_path())
                .is_some_and(|&before| before != item.modified)
        })
        .map(|item| item.path.as_path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{apply, create_space};
    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::{Extensions, GroupBy, ItemFacts, Preset, SpaceView, ViewMode};
    use deskspace_platform::ShellDisplay;

    /// An app with one space grouped by Name, holding `apple` and `avocado` in
    /// one group and `zebra`, stored between them, in another.
    fn grouped_app() -> (App, SpaceId, [ItemId; 3]) {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let desktop = &mut app.config.desktop;
        let id = create_space(desktop, "Grouped", Rect::new(0, 0, 600, 400));
        let items = [
            crate::fixtures::add_item(desktop, id, "C:/missing/apple.txt"),
            crate::fixtures::add_item(desktop, id, "C:/missing/zebra.txt"),
            crate::fixtures::add_item(desktop, id, "C:/missing/avocado.txt"),
        ];
        apply(
            desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView {
                    group: Some(GroupBy {
                        key: deskspace_core::SortKey::Name,
                        descending: false,
                    }),
                    ..SpaceView::default().with_mode(ViewMode::SmallIcons)
                },
            },
        );
        app.sync_grouping(id);
        (app, id, items)
    }

    fn shown_ids(app: &App, id: SpaceId) -> Vec<ItemId> {
        app.surface_shown_items(Surface::Space(id))
            .iter()
            .map(|item| item.id)
            .collect()
    }

    #[test]
    fn a_grouped_space_shows_its_groups_and_keeps_its_stored_order() {
        let (app, id, [apple, zebra, avocado]) = grouped_app();
        assert_eq!(shown_ids(&app, id), [apple, avocado, zebra]);
        let stored: Vec<ItemId> = app
            .config
            .desktop
            .space(id)
            .unwrap()
            .items()
            .iter()
            .map(|item| item.id)
            .collect();
        assert_eq!(stored, [apple, zebra, avocado], "the stored order stands");
    }

    #[test]
    fn hit_tests_find_the_item_a_grouped_cell_shows() {
        let (app, id, [_, zebra, _]) = grouped_app();
        let surface = Surface::Space(id);
        let cell = app
            .surface_cells(surface)
            .into_iter()
            .find(|(item, _)| *item == zebra)
            .map(|(_, rect)| rect)
            .unwrap();
        assert_eq!(app.surface_item_at(surface, cell.center()), Some(zebra));
    }

    #[test]
    fn folding_a_group_drops_its_items_from_the_selection_and_the_cells() {
        let (mut app, id, [apple, zebra, avocado]) = grouped_app();
        let surface = Surface::Space(id);
        app.selection.select_items(surface, vec![apple, zebra]);
        app.selection.checked = vec![avocado];
        app.selection.focused = Some(apple);

        app.view_cache.toggle_group(id, "A - H");
        app.sync_grouping(id);

        assert_eq!(app.selection.items, [zebra]);
        assert!(app.selection.checked.is_empty());
        assert_eq!(app.selection.focused, None);
        assert_eq!(shown_ids(&app, id), [zebra], "Ctrl+A reads this list");
        let cells = app.surface_cells(surface);
        assert_eq!(cells.len(), 1, "the marquee and the arrows read these");
        assert_eq!(cells[0].0, zebra);

        app.view_cache.toggle_group(id, "A - H");
        app.sync_grouping(id);
        assert_eq!(shown_ids(&app, id), [apple, avocado, zebra]);
        assert_eq!(app.selection.items, [zebra], "opening selects nothing back");
    }

    #[test]
    fn a_group_header_click_finds_its_group_under_the_scroll() {
        let (app, id, _) = grouped_app();
        let space = app.config.desktop.space(id).unwrap();
        let content = space.content_rect();
        let layout = app.view_layout(id).unwrap();
        let header = layout.group_rect(1).unwrap().center();
        let point = Point::new(content.x + header.x, content.y + header.y);
        assert_eq!(app.group_name_at(id, point).as_deref(), Some("Q - Z"));
        let row = Point::new(content.x + 20, content.y + layout.rect(0).center().y);
        assert_eq!(app.group_name_at(id, row), None);
    }

    #[test]
    fn a_grouping_is_rebuilt_only_when_what_it_reads_changes() {
        let (mut app, id, [apple, zebra, avocado]) = grouped_app();
        let first = app.view_cache.grouping(id).unwrap();
        app.sync_grouping(id);
        assert!(
            Arc::ptr_eq(&first, &app.view_cache.grouping(id).unwrap()),
            "nothing changed, so nothing is rebuilt"
        );
        apply(
            &mut app.config.desktop,
            Command::RenameItem {
                id: apple,
                path: "C:/missing/yak.txt".into(),
                label: "yak".into(),
            },
        );
        app.sync_grouping(id);
        assert!(!Arc::ptr_eq(&first, &app.view_cache.grouping(id).unwrap()));
        assert_eq!(
            shown_ids(&app, id),
            [avocado, apple, zebra],
            "the renamed item moved to its new group"
        );
    }

    #[test]
    fn ungrouping_forgets_the_display_order() {
        let (mut app, id, [apple, zebra, avocado]) = grouped_app();
        apply(
            &mut app.config.desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView::default().with_mode(ViewMode::SmallIcons),
            },
        );
        app.sync_grouping(id);
        assert!(app.view_cache.grouping(id).is_none());
        assert_eq!(shown_ids(&app, id), [apple, zebra, avocado]);
    }

    #[test]
    fn a_stale_display_order_is_never_read_once_ungrouped() {
        let (mut app, id, [apple, zebra, avocado]) = grouped_app();
        apply(
            &mut app.config.desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView::default().with_mode(ViewMode::SmallIcons),
            },
        );
        // Before the next repaint brings the cache up to date.
        assert_eq!(shown_ids(&app, id), [apple, zebra, avocado]);
    }

    #[test]
    fn only_items_modified_at_the_same_path_changed_in_place() {
        let item = |path: &str, modified| {
            let facts = ItemFacts {
                modified,
                ..ItemFacts::default()
            };
            Item::new(ItemId(0), path, facts)
        };
        let known = [
            item("C:/d/same.txt", Some(1)),
            item("C:/d/edited.txt", Some(1)),
            item("C:/d/gone.txt", Some(1)),
        ];
        let fresh = [
            item("C:/d/same.txt", Some(1)),
            item("C:/d/edited.txt", Some(2)),
            item("C:/d/new.txt", Some(5)),
        ];
        assert_eq!(
            changed_in_place(&known, &fresh),
            [Path::new("C:/d/edited.txt")]
        );
    }

    #[test]
    fn each_extension_choice_decides_its_space_and_automatic_follows_explorer() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = create_space(&mut app.config.desktop, "A", Rect::new(0, 0, 100, 100));
        let surface = Surface::Space(id);
        app.system.set_shell(ShellDisplay {
            show_extensions: true,
            ..ShellDisplay::default()
        });
        // Automatic is today's rule, so Explorer's option counts only while
        // the space mirrors Explorer's display.
        assert!(app.config.theme.match_explorer(), "the default preset");
        assert!(app.shows_extensions(surface));
        app.config.theme.preset = Preset::Custom;
        assert!(!app.shows_extensions(surface), "hidden under Custom");
        app.system.set_shell(ShellDisplay::default());
        assert!(!app.shows_extensions(surface), "Explorer hides them too");
        // Show and Hide decide for the space, whatever Explorer does.
        for (extensions, shown) in [(Extensions::Show, true), (Extensions::Hide, false)] {
            apply(
                &mut app.config.desktop,
                Command::SetSpaceView {
                    id,
                    view: SpaceView {
                        extensions,
                        ..SpaceView::default()
                    },
                },
            );
            assert_eq!(
                app.shows_extensions(surface),
                shown,
                "{extensions:?} decides over both presets"
            );
        }
    }

    #[test]
    fn a_view_key_keeps_the_sort_rule() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let id = create_space(&mut app.config.desktop, "A", Rect::new(0, 0, 100, 100));
        let sorted = SpaceView {
            sort: deskspace_core::SortRule {
                key: deskspace_core::SortKey::Modified,
                descending: true,
                folders_first: false,
            },
            keep_sorted: false,
            status_bar: true,
            ..SpaceView::default()
        };
        apply(
            &mut app.config.desktop,
            Command::SetSpaceView {
                id,
                view: sorted.clone(),
            },
        );
        assert_eq!(app.view_by_digit(6), None, "no space is current");

        app.selection.surface = Some(Surface::Space(id));
        let (target, view) = app.view_by_digit(6).unwrap();
        assert_eq!(target, id);
        assert_eq!(view, sorted.with_mode(ViewMode::Details));
        assert_eq!(view.sort, sorted.sort);
        assert!(!view.keep_sorted);
        assert!(view.status_bar);
    }
}
