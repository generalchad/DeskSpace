//! `App` view: space windows, scenes, layout, and the desktop layer (R1).

use std::collections::HashSet;

use deskspace_core::geometry::Rect;
use deskspace_core::layout::GridSpec;
use deskspace_core::theme::ICON_SIZE_RANGE;
use deskspace_core::{Command, Item, ItemId, Palette, SpaceId, Theme};
use deskspace_platform::{
    CheckBoxMode, ITEM_ICON_SIZE, ItemMetrics, SpaceStyle, Ui, arm_watchdog, desktop_directory,
    desktop_display_since, enumerate_desktop_items, hide_desktop_icons, rehide_desktop_icons,
    restore_desktop_icons, space_dpi, virtual_desktop_rect, work_area_at,
};

use crate::scene::{ViewState, desktop_layer_scene, layer_style, space_scene, space_style};

use super::rename::EditTarget;
use super::{App, HEALTH_MS, HEALTH_TIMER, Surface, tray_tooltip};

impl App {
    /// Creates windows for new spaces and destroys windows of removed ones.
    pub(super) fn sync_windows(&mut self, ui: &mut Ui) {
        let existing: HashSet<u64> = ui.space_ids().into_iter().collect();
        let ids: Vec<SpaceId> = self.config.desktop.spaces.iter().map(|f| f.id).collect();
        for id in &ids {
            if existing.contains(&id.0) {
                continue;
            }
            let Some(rect) = self.config.desktop.space(*id).map(|f| f.visible_rect()) else {
                continue;
            };
            if let Err(error) = ui.create_space(id.0, rect, self.style_for(*id)) {
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

    pub(super) fn view_for(&self, surface: Surface) -> ViewState {
        let check_boxes = self.surface_check_box_mode(surface);
        ViewState {
            hovered: self.pointer.hovered_item_on(surface),
            selected: if self.selection.surface == Some(surface) {
                self.selection.items.clone()
            } else {
                Vec::new()
            },
            dragged: self.pointer.dragged_on(surface),
            scroll: self.surface_scroll(surface),
            show_check_boxes: check_boxes != CheckBoxMode::Hidden,
            check_boxes_persistent: check_boxes == CheckBoxMode::Independent,
            checked: self.selection.checked.clone(),
            show_extensions: self.surface_match_explorer(surface)
                && self.shell_display.show_extensions,
            show_compressed_color: self.surface_match_explorer(surface)
                && self.shell_display.show_compressed_color,
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
                .highlighted
                .is_some_and(|id| surface == Surface::Space(id)),
            focused: (self.selection.surface == Some(surface))
                .then_some(self.selection.focused)
                .flatten(),
        }
    }

    /// The theme a space is drawn with: its own look while customized, else
    /// the global theme. A missing space gets the global theme.
    pub(super) fn theme_for(&self, id: SpaceId) -> &Theme {
        let global = &self.config.theme;
        self.config
            .desktop
            .space(id)
            .map_or(global, |space| space.theme(global))
    }

    /// The unscaled grid a space lays out on, with its own spacing while
    /// customized.
    pub(super) fn grid_for(&self, id: SpaceId) -> GridSpec {
        let global = self.config.desktop.layout;
        self.config
            .desktop
            .space(id)
            .map_or(global, |space| space.grid(global))
    }

    /// Whether a space mirrors Explorer's display settings.
    pub(super) fn match_explorer(&self, id: SpaceId) -> bool {
        self.theme_for(id).match_explorer()
    }

    /// The concrete colors for a space's scene: its theme's palette, with the
    /// selection highlight and border replaced by Windows' accent when the
    /// theme asks for it and an accent is known.
    pub(super) fn palette_for(&self, id: SpaceId) -> Palette {
        self.surface_palette(Surface::Space(id))
    }

    /// The window style for a space.
    pub(super) fn style_for(&self, id: SpaceId) -> SpaceStyle {
        space_style(self.theme_for(id), self.system_dark)
    }

    /// Whether DeskSpace's own chrome (the settings window and menus) is dark.
    /// `Theme.mode` colors the spaces; this follows the global `ui_mode`.
    pub(super) fn ui_dark(&self) -> bool {
        self.config.theme.ui_mode.resolve(self.system_dark)
    }

    /// The item-icon scale of `theme` as a factor, without per-monitor DPI.
    /// When mirroring Explorer's display, the desktop's own icon size (in
    /// pixels) wins, so spaces are 1:1 with the desktop; otherwise the theme's
    /// icon-size setting applies.
    pub(super) fn icon_scale(&self, theme: &Theme) -> f32 {
        if theme.match_explorer()
            && let Some(pixels) = self.shell_display.icon_size
        {
            return (pixels as f32 / ITEM_ICON_SIZE as f32).max(0.25);
        }
        theme.icon_size.clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as f32 / 100.0
    }

    /// The full icon factor for one space: the icon-size factor times the
    /// monitor's DPI. Explorer's own icon size is already in physical pixels,
    /// so mirroring does not multiply by DPI again.
    pub(super) fn space_scale(&self, id: SpaceId) -> f32 {
        let theme = self.theme_for(id);
        let scale = self.icon_scale(theme);
        if theme.match_explorer() && self.shell_display.icon_size.is_some() {
            return scale;
        }
        scale * space_dpi(id.0)
    }

    /// `grid` scaled for an icon/DPI scale. When `theme` mirrors Explorer, its
    /// item spacing (the full cell size in physical pixels) replaces the cell
    /// and gap, so the grid step matches the desktop exactly. The stored grids
    /// are never scaled, so the settings form keeps editing logical values.
    pub(super) fn base_layout(&self, theme: &Theme, grid: GridSpec, scale: f32) -> GridSpec {
        let mut layout = grid.scaled(scale);
        if theme.match_explorer()
            && let Some(spacing) = self.shell_display.icon_spacing
        {
            layout.cell_width = spacing.width.max(1);
            layout.cell_height = spacing.height.max(1);
            layout.gap = 0;
        }
        layout
    }

    /// The scaled grid for one space, including its monitor's DPI.
    pub(super) fn layout_for(&self, id: SpaceId) -> GridSpec {
        self.base_layout(self.theme_for(id), self.grid_for(id), self.space_scale(id))
    }

    /// The logical global grid (icon size only, no per-monitor DPI), for
    /// callers that have no space at hand.
    pub(super) fn effective_layout(&self) -> GridSpec {
        let theme = &self.config.theme;
        self.base_layout(theme, self.config.desktop.layout, self.icon_scale(theme))
    }

    /// Scaled icon, label, and check-box sizes for one space.
    pub(super) fn metrics_for(&self, id: SpaceId) -> ItemMetrics {
        ItemMetrics::for_scale(self.space_scale(id))
    }

    /// Re-reads Explorer's display options and repaints if they changed. A
    /// changed icon size or item spacing re-flows every space too.
    pub(super) fn refresh_shell_display(&mut self, ui: &mut Ui) {
        let display = desktop_display_since(self.shell_display.check_boxes);
        if display == self.shell_display {
            return;
        }
        let any_matching = self
            .config
            .desktop
            .spaces
            .iter()
            .any(|space| space.theme(&self.config.theme).match_explorer());
        let relayout = any_matching
            && (display.icon_size != self.shell_display.icon_size
                || display.icon_spacing != self.shell_display.icon_spacing);
        self.shell_display = display;
        // Independent ticks do not survive a switch out of that mode.
        if self.shell_display.check_boxes != CheckBoxMode::Independent {
            self.selection.checked.clear();
        }
        if relayout {
            self.relayout_all(ui);
        } else {
            self.restyle_all(ui);
        }
    }

    /// Reads the desktop's loose items from the shell, with their positions,
    /// now that OLE is initialized. The M2 desktop layer will draw them.
    pub(super) fn refresh_desktop_entries(&mut self) {
        self.desktop_entries = match enumerate_desktop_items() {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(%error, "could not enumerate desktop items");
                Vec::new()
            }
        };
        tracing::debug!(
            items = self.desktop_entries.len(),
            positioned = self
                .desktop_entries
                .iter()
                .filter(|entry| entry.position.is_some())
                .count(),
            "read desktop items from the shell"
        );
    }

    /// Starts (or resumes) watching the user's Desktop folder so the layer
    /// follows files added, removed, or renamed outside DeskSpace. A no-op once
    /// the same folder is already watched.
    pub(super) fn watch_desktop_folder(&mut self, ui: &mut Ui) {
        match desktop_directory() {
            Ok(directory) => ui.watch_folder(&directory),
            Err(error) => tracing::warn!(%error, "could not resolve the Desktop folder to watch"),
        }
    }

    /// The window style for the desktop layer (chrome-less, transparent).
    pub(super) fn layer_style(&self) -> SpaceStyle {
        layer_style(&self.config.theme, self.system_dark)
    }

    /// Whether the desktop layer window is drawn: the additive
    /// `--takeover-layer`, or the full takeover (`--takeover` or the setting).
    pub(super) fn layer_active(&self) -> bool {
        self.takeover_layer || self.desktop_takeover()
    }

    /// Whether DeskSpace takes over the desktop by hiding Explorer's icons:
    /// the hidden `--takeover` flag or the saved setting.
    pub(super) fn desktop_takeover(&self) -> bool {
        self.takeover || self.config.window.take_over_desktop
    }

    /// The loose desktop items for the layer: the desktop entries that carry a
    /// position and are not already held by a space. The `ItemId`s are
    /// placeholders; `SyncDesktop` reconciles them by path.
    pub(super) fn desktop_layer_items(&self) -> Vec<Item> {
        self.desktop_entries
            .iter()
            .filter_map(|entry| {
                let position = entry.position?;
                let in_space = self
                    .config
                    .desktop
                    .spaces
                    .iter()
                    .flat_map(|space| space.items.iter())
                    .any(|item| crate::storage::same_path(&item.path, &entry.path));
                if in_space {
                    return None;
                }
                let mut item = Item::new(ItemId(0), &entry.path)
                    .with_label(entry.label.clone())
                    .with_kind(entry.kind);
                item.position = position;
                Some(item)
            })
            .collect()
    }

    /// Creates the layer window if needed, refreshes the model's loose items
    /// from the shell snapshot, and repaints. Returns whether the layer window
    /// is up; a no-op returning false unless `layer_active`.
    pub(super) fn sync_desktop_layer(&mut self, ui: &mut Ui) -> bool {
        if !self.layer_active() {
            return false;
        }
        let rect = virtual_desktop_rect();
        if let Err(error) = ui.create_desktop_layer(rect, self.layer_style()) {
            tracing::warn!(%error, "could not create the desktop layer window");
            return false;
        }
        let items = self.desktop_layer_items();
        self.config.desktop.apply_with_layout(
            Command::SyncDesktop { items },
            self.effective_layout(),
            rect,
        );
        self.refresh_desktop_layer(ui);
        true
    }

    /// Brings the layer and Explorer-icon hiding in line with the settings:
    /// draw the layer, then hide or restore Explorer's icons. Hiding waits
    /// until the layer is up, so a failure never leaves the desktop blank.
    pub(super) fn apply_takeover(&mut self, ui: &mut Ui) {
        let takeover = self.desktop_takeover();
        // Follow the Desktop folder while the layer is drawn, so a file added
        // or removed outside DeskSpace reaches the layer (I8).
        if self.layer_active() {
            self.watch_desktop_folder(ui);
        } else {
            ui.unwatch_folder();
        }
        let layer_up = self.sync_desktop_layer(ui);
        if takeover && layer_up {
            // Tests must not hide the developer's real desktop.
            if !cfg!(test) {
                // The watchdog must be running before the icons are hidden, so
                // a crash cannot leave the desktop blank.
                arm_watchdog();
                hide_desktop_icons();
            }
            ui.set_timer(HEALTH_TIMER, HEALTH_MS);
        } else {
            if !cfg!(test) {
                restore_desktop_icons();
            }
            ui.clear_timer(HEALTH_TIMER);
        }
        if !self.layer_active() {
            ui.destroy_desktop_layer();
        }
    }

    /// The health timer: re-hide Explorer's icons if it recreated its layer
    /// without broadcasting `TaskbarCreated`.
    pub(super) fn health_tick(&self) {
        if !cfg!(test) {
            rehide_desktop_icons();
        }
    }

    /// Pushes the desktop layer's rect and draw list to its window.
    pub(super) fn refresh_desktop_layer(&mut self, ui: &mut Ui) {
        if !self.layer_active() {
            return;
        }
        let rect = virtual_desktop_rect();
        let view = self.view_for(Surface::Desktop);
        let theme = &self.config.theme;
        let palette = self.surface_palette(Surface::Desktop);
        let scene = desktop_layer_scene(
            self.config.desktop.layer(),
            rect.origin(),
            theme,
            &palette,
            self.surface_layout(Surface::Desktop),
            self.surface_metrics(Surface::Desktop),
            view,
            &mut self.icons,
        );
        ui.set_desktop_layer_rect(rect);
        ui.set_desktop_layer_scene(scene);
    }

    /// Repaints a surface; the layer repaints through its own window.
    pub(super) fn refresh_surface(&mut self, ui: &mut Ui, surface: Surface) {
        match surface {
            Surface::Space(id) => self.refresh_space(ui, id),
            Surface::Desktop => self.refresh_desktop_layer(ui),
        }
    }

    /// Pushes the space's current geometry and draw list to its window.
    pub(super) fn refresh_space(&mut self, ui: &mut Ui, id: SpaceId) {
        let view = self.view_for(Surface::Space(id));
        let Some(space) = self.config.desktop.space(id) else {
            return;
        };
        let palette = self.palette_for(id);
        let mut scene = space_scene(
            space,
            space.theme(&self.config.theme),
            &palette,
            self.layout_for(id),
            self.metrics_for(id),
            view,
            &mut self.icons,
        );
        if let Some(session) = self
            .edit
            .session()
            .filter(|session| session.surface == Surface::Space(id))
        {
            match session.target {
                EditTarget::Title => scene.editing = Some(session.text.clone()),
                EditTarget::Item { id: item, .. } => {
                    scene.editing_item = Some((item.0, session.text.clone()));
                }
            }
        }
        scene.caret_visible = self.edit.caret_visible();
        ui.set_space_rect(id.0, space.visible_rect());
        ui.set_space_scene(id.0, scene);
    }

    pub(super) fn restyle_all(&mut self, ui: &mut Ui) {
        let ids: Vec<SpaceId> = self.config.desktop.spaces.iter().map(|f| f.id).collect();
        for id in ids {
            self.restyle_space(ui, id);
        }
    }

    /// Re-applies one space's window style and repaints it.
    pub(super) fn restyle_space(&mut self, ui: &mut Ui, id: SpaceId) {
        ui.set_space_style(id.0, self.style_for(id));
        self.refresh_space(ui, id);
    }

    /// Re-flows every space for a changed grid: growing spaces resize to fit,
    /// user-sized spaces keep their size and reflow.
    pub(super) fn relayout_all(&mut self, ui: &mut Ui) {
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
    pub(super) fn reflow_space(&mut self, ui: &mut Ui, id: SpaceId) {
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
    pub(super) fn on_dpi_changed(&mut self, ui: &mut Ui, id: SpaceId) {
        self.reflow_space(ui, id);
        self.refresh_desktop_layer(ui);
        self.schedule_save(ui);
    }

    pub(super) fn refresh_tray(&self, ui: &mut Ui) {
        ui.set_tray_tooltip(&tray_tooltip(&self.config.desktop));
    }

    /// Re-reads Explorer's display options and re-flows every space, as F5
    /// refreshes the desktop.
    pub(super) fn refresh_spaces(&mut self, ui: &mut Ui) {
        self.refresh_shell_display(ui);
        self.relayout_all(ui);
        self.refresh_desktop_layer(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::Preset;
    use deskspace_core::geometry::Size;
    use deskspace_platform::ShellDisplay;

    /// An app with a known Explorer display snapshot. `space_dpi` is `1.0`
    /// outside the UI thread, so the scale assertions are deterministic.
    fn app_with_display(display: ShellDisplay) -> App {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        app.shell_display = display;
        app
    }

    #[test]
    fn icon_size_follows_explorer_when_matching() {
        // Default theme: `match_explorer` on, icon size 100.
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(2 * ITEM_ICON_SIZE),
            ..ShellDisplay::default()
        });
        assert_eq!(app.icon_scale(&app.config.theme), 2.0);
        // Explorer's size wins, so the DeskSpace setting is inert.
        app.config.theme.icon_size = 200;
        assert_eq!(app.icon_scale(&app.config.theme), 2.0);
        app.config.theme.icon_size = 50;
        assert_eq!(app.icon_scale(&app.config.theme), 2.0);
    }

    #[test]
    fn icon_size_setting_applies_without_matching() {
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(2 * ITEM_ICON_SIZE),
            ..ShellDisplay::default()
        });
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 150;
        assert_eq!(app.icon_scale(&app.config.theme), 1.5);
        app.config.theme.icon_size = 50;
        assert_eq!(app.icon_scale(&app.config.theme), 0.5);
    }

    #[test]
    fn matching_without_a_reported_size_falls_back_to_the_setting() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.icon_size = 75;
        assert_eq!(app.icon_scale(&app.config.theme), 0.75);
    }

    #[test]
    fn explorer_spacing_replaces_the_cell_when_matching() {
        let app = app_with_display(ShellDisplay {
            icon_spacing: Some(Size::new(120, 130)),
            ..ShellDisplay::default()
        });
        let layout = app.base_layout(&app.config.theme, app.config.desktop.layout, 1.0);
        assert_eq!(
            (layout.cell_width, layout.cell_height, layout.gap),
            (120, 130, 0)
        );
    }

    #[test]
    fn grid_spacing_applies_without_matching() {
        let mut app = app_with_display(ShellDisplay {
            icon_spacing: Some(Size::new(120, 130)),
            ..ShellDisplay::default()
        });
        app.config.theme.preset = Preset::Custom;
        app.config.desktop.layout.gap = 20;
        let layout = app.base_layout(&app.config.theme, app.config.desktop.layout, 1.0);
        assert_eq!(
            (layout.cell_width, layout.cell_height, layout.gap),
            (96, 104, 20)
        );
    }

    #[test]
    fn space_layout_and_metrics_follow_the_icon_size() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 200;
        let layout = app.layout_for(SpaceId(1));
        assert_eq!((layout.cell_width, layout.cell_height), (192, 208));
        assert_eq!(app.metrics_for(SpaceId(1)).icon_size, 96);
        app.config.theme.icon_size = 50;
        let layout = app.layout_for(SpaceId(1));
        assert_eq!((layout.cell_width, layout.cell_height), (48, 52));
        assert_eq!(app.metrics_for(SpaceId(1)).icon_size, 24);
    }

    #[test]
    fn a_matching_space_draws_explorer_icon_metrics() {
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(ITEM_ICON_SIZE),
            ..ShellDisplay::default()
        });
        app.config.theme.icon_size = 200;
        assert_eq!(app.metrics_for(SpaceId(1)).icon_size, ITEM_ICON_SIZE);
        assert_eq!(app.layout_for(SpaceId(1)).cell_width, 96);
    }

    #[test]
    fn effective_layout_uses_the_global_icon_size() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 200;
        assert_eq!(app.effective_layout().cell_width, 192);
    }
}
