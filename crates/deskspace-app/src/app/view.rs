//! `App` view: space windows, scenes, layout, and the desktop layer (R1).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use deskspace_core::animation::Kind;
use deskspace_core::geometry::{Point, Rect};
use deskspace_core::layout::GridSpec;
use deskspace_core::theme::ICON_SIZE_RANGE;
use deskspace_core::view_layout::{ViewLayout, ViewMetrics};
use deskspace_core::{
    Color, Command, Extensions, Item, ItemId, Palette, SpaceId, SpaceView, Theme, ViewMode,
};
use deskspace_platform::{
    CheckBoxMode, DesktopEntry, ITEM_ICON_SIZE, ItemMetrics, ShellDisplay, SpaceScene, SpaceStyle,
    Ui, Way, arm_watchdog, desktop_display_since, enumerate_desktop_items, foreground_window,
    hide_desktop_icons, rehide_desktop_icons, restore_desktop_icons, space_dpi,
    system_accent_color, system_animations, system_uses_dark_mode, virtual_desktop_rect,
    work_area_at,
};

use crate::scene::{
    SurfaceLook, ViewState, desktop_layer_scene, layer_style, space_scene_with_details,
    space_style, view_cells,
};
use crate::settings::view_for_digit;
use crate::storage::path_key;

use super::animation::Arrival;
use super::rename::EditTarget;
use super::{App, Surface, Timer};

/// How often, while takeover is on, to re-hide Explorer's icon layer in case
/// it recreated it without broadcasting `TaskbarCreated`.
const HEALTH_MS: u32 = 5_000;

/// What Windows reports and DeskSpace follows: dark mode, the accent color,
/// and Explorer's desktop display options.
#[derive(Debug)]
pub(crate) struct SystemLook {
    dark: bool,
    /// Windows' accent color; `None` when it cannot be read.
    accent: Option<Color>,
    /// Explorer's display options, refreshed by `App::refresh_shell_display`,
    /// which also reacts to their change.
    shell: ShellDisplay,
    /// Windows' "Animation effects" switch; while it is off nothing animates
    /// (ROADMAP D12).
    animations: bool,
}

impl SystemLook {
    /// Reads dark mode and the accent now. Explorer's display is read once
    /// OLE is up, on `Started`.
    pub(crate) fn read() -> Self {
        Self {
            dark: system_uses_dark_mode(),
            accent: system_accent_color(),
            shell: ShellDisplay::default(),
            animations: system_animations(),
        }
    }

    /// Re-reads dark mode and the accent after a Windows theme change.
    /// Returns whether either changed.
    pub(super) fn refresh(&mut self) -> bool {
        let dark = std::mem::replace(&mut self.dark, system_uses_dark_mode());
        let accent = std::mem::replace(&mut self.accent, system_accent_color());
        (dark, accent) != (self.dark, self.accent)
    }

    pub(super) fn dark(&self) -> bool {
        self.dark
    }

    pub(super) fn animations(&self) -> bool {
        self.animations
    }

    /// Re-reads Windows' "Animation effects" switch after it changed.
    pub(super) fn refresh_animations(&mut self) {
        self.animations = system_animations();
    }

    pub(super) fn accent(&self) -> Option<Color> {
        self.accent
    }

    pub(super) fn shell(&self) -> &ShellDisplay {
        &self.shell
    }

    pub(super) fn set_shell(&mut self, shell: ShellDisplay) {
        self.shell = shell;
    }

    #[cfg(test)]
    pub(super) fn set_accent(&mut self, accent: Option<Color>) {
        self.accent = accent;
    }
}

/// Per-space view state that is not persisted, keyed by `SpaceId`.
#[derive(Debug, Default)]
pub(crate) struct ViewCache {
    /// Scroll offset in pixels. May exceed the current maximum after a
    /// resize, so the app reads it through `App::scroll_of`, which clamps.
    scroll: HashMap<SpaceId, i32>,
    /// Each space's icon factor at its last re-flow: a fixed view's factor,
    /// or `App::icon_scale`'s, without DPI. A change to it can keep the
    /// space's size first (`App::keep_size_on_zoom`).
    icon_scales: HashMap<SpaceId, f32>,
}

impl ViewCache {
    /// The stored scroll offset, unclamped; 0 when never scrolled.
    pub(super) fn scroll_of(&self, id: SpaceId) -> i32 {
        self.scroll.get(&id).copied().unwrap_or(0)
    }

    pub(super) fn set_scroll(&mut self, id: SpaceId, px: i32) {
        self.scroll.insert(id, px);
    }

    /// Records `id`'s icon factor and returns the one from its last re-flow.
    fn record_icon_scale(&mut self, id: SpaceId, factor: f32) -> Option<f32> {
        self.icon_scales.insert(id, factor)
    }

    /// Drops everything kept for a deleted space.
    pub(super) fn forget(&mut self, id: SpaceId) {
        self.scroll.remove(&id);
        self.icon_scales.remove(&id);
    }
}

/// The desktop layer's state: the hidden command-line flags that turn it on
/// and the loose items last read from the shell. `App::layer_active` and
/// `App::desktop_takeover` combine the flags with the saved setting.
#[derive(Debug, Default)]
pub(crate) struct LayerState {
    /// Draw the loose desktop items on a chrome-less layer window (hidden
    /// `--takeover-layer`, M2 takeover). Additive: Explorer still draws its
    /// own icons.
    layer_flag: bool,
    /// Hide Explorer's icons and draw the loose items on the layer (hidden
    /// `--takeover`, M2 takeover). Implies the layer.
    takeover_flag: bool,
    /// The desktop's loose items read from the shell, which the layer draws.
    entries: Vec<DesktopEntry>,
    /// Whether a taskbar was last seen under the layer (`check_taskbars`).
    taskbar_blocked: bool,
    /// The debug window's takeover switch for this session (ROADMAP D14),
    /// which wins over the flag and the setting and is never saved.
    session_takeover: Option<bool>,
}

impl LayerState {
    pub(crate) fn new(layer_flag: bool, takeover_flag: bool) -> Self {
        Self {
            layer_flag,
            takeover_flag,
            entries: Vec::new(),
            session_takeover: None,
            taskbar_blocked: false,
        }
    }
}

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
            let Some(rect) = self.config.desktop.space(*id).map(|f| f.visible_rect()) else {
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
            column_drop: self.column_drop_on(surface),
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

    /// The concrete colors for a space's scene: its theme's palette, with the
    /// selection highlight and border replaced by Windows' accent when the
    /// theme asks for it and an accent is known.
    pub(super) fn palette_for(&self, id: SpaceId) -> Palette {
        self.surface_palette(Surface::Space(id))
    }

    /// The window style for a space.
    pub(super) fn style_for(&self, id: SpaceId) -> SpaceStyle {
        let mut style = space_style(self.theme_for(id), self.system.dark());
        if self.debug.as_ref().is_some_and(|tools| tools.hide_shadows) {
            style.shadow = None;
        }
        style
    }

    /// Whether DeskSpace's own chrome (the settings window and menus) is dark.
    /// `Theme.mode` colors the spaces; this follows the global `ui_mode`.
    pub(super) fn ui_dark(&self) -> bool {
        self.config.theme.ui_mode.resolve(self.system.dark())
    }

    /// The item-icon scale of `theme` as a factor, without per-monitor DPI.
    /// When mirroring Explorer's display, the desktop's own icon size (in
    /// pixels) wins, so spaces are 1:1 with the desktop; otherwise the theme's
    /// icon-size setting applies.
    pub(super) fn icon_scale(&self, theme: &Theme) -> f32 {
        if theme.match_explorer()
            && let Some(pixels) = self.system.shell().icon_size
        {
            return (pixels as f32 / ITEM_ICON_SIZE as f32).max(0.25);
        }
        theme.icon_size.clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as f32 / 100.0
    }

    /// The full icon factor for one space: the icon-size factor times the
    /// monitor's DPI. Explorer's own icon size is already in physical pixels,
    /// so mirroring does not multiply by DPI again. A fixed icon view ignores
    /// both the look and Explorer, and scales only for DPI.
    pub(super) fn space_scale(&self, id: SpaceId) -> f32 {
        if let Some(scale) = self.fixed_icon_scale(id) {
            return scale * space_dpi(id.0);
        }
        let theme = self.theme_for(id);
        let scale = self.icon_scale(theme);
        if theme.match_explorer() && self.system.shell().icon_size.is_some() {
            return scale;
        }
        scale * space_dpi(id.0)
    }

    /// The icon factor a fixed view asks for, without DPI, or `None` when the
    /// space follows the look's icon size.
    fn fixed_icon_scale(&self, id: SpaceId) -> Option<f32> {
        let px = self.config.desktop.space(id)?.view().mode.icon_px()?;
        Some(px as f32 / ITEM_ICON_SIZE as f32)
    }

    /// `grid` scaled for an icon/DPI scale, with the title bar scaled by
    /// `title_scale` (the DPI alone, so a zoom leaves it alone). When
    /// `fixed_icon` is false and `theme` mirrors Explorer, its item spacing
    /// (the full cell size in physical pixels) replaces the cell and gap, so
    /// the grid step matches the desktop exactly. A fixed icon view keeps its
    /// own scaled grid. The stored grids are never scaled, so the settings form
    /// keeps editing logical values.
    pub(super) fn base_layout(
        &self,
        theme: &Theme,
        grid: GridSpec,
        scale: f32,
        title_scale: f32,
        fixed_icon: bool,
    ) -> GridSpec {
        let mut layout = grid.scaled_with_title(scale, title_scale);
        if !fixed_icon
            && theme.match_explorer()
            && let Some(spacing) = self.system.shell().icon_spacing
        {
            layout.cell_width = spacing.width.max(1);
            layout.cell_height = spacing.height.max(1);
            layout.gap = 0;
        }
        layout
    }

    /// The scaled grid for one space, including its monitor's DPI.
    pub(super) fn layout_for(&self, id: SpaceId) -> GridSpec {
        let fixed_icon = self.fixed_icon_scale(id).is_some();
        self.base_layout(
            self.theme_for(id),
            self.grid_for(id),
            self.space_scale(id),
            space_dpi(id.0),
            fixed_icon,
        )
    }

    /// The logical global grid (icon size only, no per-monitor DPI), for
    /// callers that have no space at hand.
    pub(super) fn effective_layout(&self) -> GridSpec {
        let theme = &self.config.theme;
        self.base_layout(
            theme,
            self.config.desktop.layout,
            self.icon_scale(theme),
            1.0,
            false,
        )
    }

    /// Scaled icon, label, and check-box sizes for one space. An order-placed
    /// view draws its own fixed icon size and scales the rest for the display
    /// alone, so the look and a zoom leave it alone (W16).
    pub(super) fn metrics_for(&self, id: SpaceId) -> ItemMetrics {
        let mode = self.view_mode(id);
        if mode.places_by_order() {
            let dpi = space_dpi(id.0);
            let icon_px = mode.item_icon_px().unwrap_or(ITEM_ICON_SIZE);
            return ItemMetrics {
                icon_size: ((icon_px as f32 * dpi).round() as i32).max(1),
                ..ItemMetrics::for_scale(dpi)
            };
        }
        ItemMetrics::for_scale(self.space_scale(id))
    }

    /// The space's view mode, or the default when it is gone.
    pub(super) fn view_mode(&self, id: SpaceId) -> ViewMode {
        self.config
            .desktop
            .space(id)
            .map_or(ViewMode::default(), |space| space.view().mode)
    }

    /// Where `id`'s items sit under its current view, with the same metrics and
    /// scale the scene builder uses.
    pub(super) fn view_layout(&self, id: SpaceId) -> Option<ViewLayout> {
        let space = self.config.desktop.space(id)?;
        let metrics = ViewMetrics::for_view(space.view(), self.layout_for(id), space_dpi(id.0));
        Some(ViewLayout::for_space(space, metrics))
    }

    /// Re-reads Explorer's display options and repaints if they changed. A
    /// changed icon size or item spacing re-flows every surface that matches
    /// Explorer too.
    pub(super) fn refresh_shell_display(&mut self, ui: &mut Ui) {
        let display = desktop_display_since(self.system.shell().check_boxes);
        if display == *self.system.shell() {
            return;
        }
        let resized = display.icon_size != self.system.shell().icon_size
            || display.icon_spacing != self.system.shell().icon_spacing;
        let any_space_matching = self
            .config
            .desktop
            .spaces()
            .iter()
            .any(|space| space.theme(&self.config.theme).match_explorer());
        let layer_matching = self.layer_active() && self.config.theme.match_explorer();
        self.system.shell = display;
        // Independent ticks do not survive a switch out of that mode.
        if self.system.shell().check_boxes != CheckBoxMode::Independent {
            self.selection.checked.clear();
        }
        if resized && (any_space_matching || layer_matching) {
            self.relayout_all(ui);
        } else {
            self.restyle_all(ui);
        }
        // Explorer re-spaces its own (hidden) icons for the new size, and the
        // layer draws them where Explorer placed them, so read them again.
        if resized && layer_matching {
            self.resync_desktop_layer(ui);
        }
    }

    /// Reads the desktop's loose items from the shell, with their positions,
    /// now that OLE is initialized, for the desktop layer to draw.
    pub(super) fn refresh_desktop_entries(&mut self) {
        self.layer.entries = match enumerate_desktop_items() {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(%error, "could not enumerate desktop items");
                Vec::new()
            }
        };
        tracing::debug!(
            items = self.layer.entries.len(),
            positioned = self
                .layer
                .entries
                .iter()
                .filter(|entry| entry.position.is_some())
                .count(),
            "read desktop items from the shell"
        );
    }

    /// The window style for the desktop layer (chrome-less, transparent).
    pub(super) fn layer_style(&self) -> SpaceStyle {
        layer_style(&self.config.theme, self.system.dark())
    }

    /// Whether the desktop layer window is drawn: the additive
    /// `--takeover-layer`, or the full takeover (`--takeover` or the setting).
    pub(super) fn layer_active(&self) -> bool {
        self.layer.layer_flag || self.desktop_takeover()
    }

    /// Whether DeskSpace takes over the desktop by hiding Explorer's icons:
    /// the debug window's session switch when set, else the hidden
    /// `--takeover` flag or the saved setting.
    pub(super) fn desktop_takeover(&self) -> bool {
        self.layer
            .session_takeover
            .unwrap_or(self.layer.takeover_flag || self.config.window.take_over_desktop)
    }

    /// Takes over the desktop, or gives it back, for this session only, as
    /// the setting would; the saved setting does not change.
    pub(super) fn set_session_takeover(&mut self, ui: &mut Ui, on: bool) {
        self.layer.session_takeover = Some(on);
        self.apply_takeover(ui);
    }

    /// The loose desktop items for the layer: the desktop entries that carry a
    /// position and are not already held by a space. The `ItemId`s are
    /// placeholders; `SyncDesktop` reconciles them by path.
    pub(super) fn desktop_layer_items(&self) -> Vec<Item> {
        let in_spaces: HashSet<_> = self
            .config
            .desktop
            .spaces()
            .iter()
            .flat_map(|space| space.items())
            .map(|item| path_key(&item.path))
            .collect();
        self.layer
            .entries
            .iter()
            .filter_map(|entry| {
                let position = entry.position?;
                if in_spaces.contains(&path_key(&entry.path)) {
                    return None;
                }
                let mut item =
                    Item::new(ItemId(0), &entry.path, entry.facts).with_label(entry.label.clone());
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
        // A file changed in place keeps its path, so its cached icon would
        // outlive the change (a new thumbnail, a re-pointed shortcut).
        for path in changed_in_place(self.config.desktop.layer().items(), &items) {
            self.icons.invalidate(path);
        }
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
        // Follow the Desktop folder while the layer is drawn, and every
        // lens's folder, so files added or removed outside DeskSpace reach
        // what DeskSpace draws (I8, M4 phase 2).
        self.sync_watches(ui);
        let layer_up = self.sync_desktop_layer(ui);
        if takeover && layer_up {
            // Tests must not hide the developer's real desktop.
            if !cfg!(test) {
                // The watchdog must be running before the icons are hidden, so
                // a crash cannot leave the desktop blank.
                arm_watchdog();
                hide_desktop_icons();
            }
            Self::set_timer(ui, Timer::Health, HEALTH_MS);
        } else {
            if !cfg!(test) {
                restore_desktop_icons();
            }
            Self::clear_timer(ui, Timer::Health);
        }
        if !self.layer_active() {
            ui.destroy_desktop_layer();
        }
    }

    /// The health timer: re-hide Explorer's icons if it recreated its layer
    /// without broadcasting `TaskbarCreated`.
    /// Repairs a taskbar fallen under the desktop layer, which then takes its
    /// clicks, by sending the layer back below it. Explorer drops a taskbar's
    /// topmost style and sends it down when it thinks a full-screen app is
    /// up; the trigger is not known yet (seen 2026-10-18), so each
    /// occurrence is logged with what Windows reports, and whether the
    /// repair held.
    pub(super) fn check_taskbars(&mut self, ui: &mut Ui) {
        if !self.layer_active() {
            self.layer.taskbar_blocked = false;
            return;
        }
        let taskbars = ui.taskbars();
        let found = taskbars.iter().any(|taskbar| taskbar.is_blocked());
        let blocked = found && {
            ui.sink_desktop_layer();
            ui.taskbars().iter().any(|taskbar| taskbar.is_blocked())
        };
        let was_blocked = std::mem::replace(&mut self.layer.taskbar_blocked, blocked);
        // A repaired occurrence logs each time; one the repair cannot fix
        // logs once, and again when it clears.
        if (found && was_blocked) || (!found && !was_blocked) {
            return;
        }
        let state: Vec<String> = taskbars
            .iter()
            .map(|taskbar| {
                format!(
                    "{} topmost={} above_layer={:?} rect={:?}",
                    taskbar.class, taskbar.topmost, taskbar.above_layer, taskbar.rect
                )
            })
            .collect();
        let foreground = foreground_window()
            .map(|window| format!("{} {:?} {:?}", window.class, window.title, window.rect));
        if !found {
            tracing::info!(taskbars = ?state, "every taskbar is above the desktop layer again");
        } else if blocked {
            tracing::warn!(
                taskbars = ?state,
                ?foreground,
                "a taskbar is under the desktop layer, and sinking the layer did not help; its clicks go to the layer"
            );
        } else {
            tracing::warn!(
                taskbars = ?state,
                ?foreground,
                "a taskbar was under the desktop layer; sank the layer below it"
            );
        }
    }

    pub(super) fn health_tick(&mut self, ui: &mut Ui) {
        self.check_taskbars(ui);
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
        let palette = self.surface_palette(Surface::Desktop);
        let look = SurfaceLook {
            theme: &self.config.theme,
            palette: &palette,
            layout: self.surface_layout(Surface::Desktop),
            metrics: self.surface_metrics(Surface::Desktop),
            scale: 1.0,
        };
        let mut scene = desktop_layer_scene(
            self.config.desktop.layer(),
            rect.origin(),
            look,
            view,
            &mut self.icons,
        );
        self.show_edit(&mut scene, Surface::Desktop);
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

    /// Whether a surface's labels show filename extensions: the space's own
    /// choice (W28), or under `Automatic` Explorer's own option on a surface
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
        let mut scene =
            space_scene_with_details(space, look, view, row_cells, &mut self.icons, title_icon);
        let rect = space.visible_rect();
        self.show_edit(&mut scene, Surface::Space(id));
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
                EditTarget::Title => scene.editing = Some(session.text.clone()),
                EditTarget::Item { id: item, .. } => {
                    scene.editing_item = Some((item.0, session.text.clone()));
                }
            }
        }
        scene.caret_visible = self.edit.caret_visible();
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
        let view = self
            .config
            .desktop
            .space(id)
            .map_or_else(SpaceView::default, |space| space.view().clone());
        Command::FitToContents {
            id,
            metrics: ViewMetrics::for_view(&view, self.layout_for(id), space_dpi(id.0)),
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

    /// Ctrl+Shift+1 through 8 (W13): switches the current space to the view
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
    /// columns, as an Explorer window does when its icon size changes
    /// (`TASKS.md` F15). An order-placed view draws a fixed icon size, so it is
    /// never pinned (W16).
    fn keep_size_on_zoom(&mut self, id: SpaceId) {
        let mode = self.view_mode(id);
        let factor = mode.item_icon_px().map_or_else(
            || {
                self.fixed_icon_scale(id)
                    .unwrap_or_else(|| self.icon_scale(self.theme_for(id)))
            },
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
fn changed_in_place<'a>(known: &[Item], fresh: &'a [Item]) -> Vec<&'a Path> {
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
    use deskspace_core::geometry::Size;
    use deskspace_core::{Extensions, SpaceView, ViewMode};
    use deskspace_core::{ItemFacts, Preset};
    use deskspace_platform::ShellDisplay;

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
    fn the_session_takeover_wins_without_touching_the_setting() {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        app.config.window.take_over_desktop = true;
        app.layer.session_takeover = Some(false);
        assert!(!app.desktop_takeover());
        assert!(!app.layer_active());
        assert!(app.config.window.take_over_desktop, "the setting is kept");
        app.config.window.take_over_desktop = false;
        app.layer.session_takeover = Some(true);
        assert!(app.desktop_takeover());
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

    #[test]
    fn view_cache_forgets_a_deleted_space() {
        let mut cache = ViewCache::default();
        let id = SpaceId(1);
        assert_eq!(cache.scroll_of(id), 0);
        cache.set_scroll(id, 40);
        assert_eq!(cache.record_icon_scale(id, 1.0), None);
        assert_eq!(cache.record_icon_scale(id, 1.5), Some(1.0));
        assert_eq!(cache.scroll_of(id), 40);

        cache.forget(id);
        assert_eq!(cache.scroll_of(id), 0);
        assert_eq!(cache.record_icon_scale(id, 1.5), None, "no stale factor");
    }

    /// An app with a known Explorer display snapshot. `space_dpi` is `1.0`
    /// outside the UI thread, so the scale assertions are deterministic.
    fn app_with_display(display: ShellDisplay) -> App {
        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        app.system.set_shell(display);
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
        let layout = app.base_layout(
            &app.config.theme,
            app.config.desktop.layout,
            1.0,
            1.0,
            false,
        );
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
        let layout = app.base_layout(
            &app.config.theme,
            app.config.desktop.layout,
            1.0,
            1.0,
            false,
        );
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
    fn a_fixed_icon_view_ignores_the_look_and_explorer() {
        let mut app = app_with_display(ShellDisplay {
            icon_size: Some(ITEM_ICON_SIZE),
            icon_spacing: Some(Size::new(120, 130)),
            ..ShellDisplay::default()
        });
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(0, 0, 300, 300));
        apply(
            &mut app.config.desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView::default().with_mode(ViewMode::LargeIcons),
            },
        );
        // Large icons are 96 px at 96 DPI; the grid cell is twice the base
        // 48 px icon, so 192 px wide.
        assert_eq!(app.metrics_for(id).icon_size, 96);
        assert_eq!(app.layout_for(id).cell_width, 192);
        // Neither the look's size nor Explorer's spacing overrides a fixed
        // icon view.
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 50;
        assert_eq!(app.metrics_for(id).icon_size, 96);
        assert_eq!(app.layout_for(id).cell_width, 192);
    }

    #[test]
    fn a_look_zoom_does_not_move_a_fixed_view_space() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 100;
        let id = create_space(&mut app.config.desktop, "Work", Rect::new(0, 0, 300, 300));
        apply(
            &mut app.config.desktop,
            Command::SetSpaceView {
                id,
                view: SpaceView::default().with_mode(ViewMode::LargeIcons),
            },
        );
        let before = (app.metrics_for(id), app.layout_for(id));
        app.config.theme.icon_size = 150;
        assert_eq!((app.metrics_for(id), app.layout_for(id)), before);
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

    #[test]
    fn a_zoom_leaves_the_title_bar_alone() {
        let mut app = app_with_display(ShellDisplay::default());
        app.config.theme.preset = Preset::Custom;
        app.config.theme.icon_size = 200;
        assert_eq!(
            app.effective_layout().title_height,
            app.config.desktop.layout.title_height
        );
    }
}
