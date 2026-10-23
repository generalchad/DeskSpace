//! `App` view: space windows, scenes, layout, and the desktop layer (R1).

use std::collections::{HashMap, HashSet};

use deskspace_core::animation::Kind;
use deskspace_core::geometry::{Point, Rect};
use deskspace_core::layout::GridSpec;
use deskspace_core::theme::ICON_SIZE_RANGE;
use deskspace_core::{Color, Command, Item, ItemId, Palette, SpaceId, SpaceView, Theme};
use deskspace_platform::{
    CheckBoxMode, DesktopEntry, ITEM_ICON_SIZE, ItemMetrics, ShellDisplay, SpaceStyle, Ui, Way,
    arm_watchdog, desktop_display_since, enumerate_desktop_items, foreground_window,
    hide_desktop_icons, item_facts, rehide_desktop_icons, restore_desktop_icons, space_dpi,
    system_accent_color, system_animations, system_uses_dark_mode, virtual_desktop_rect,
    work_area_at,
};

use crate::scene::{
    SurfaceLook, ViewState, desktop_layer_scene, layer_style, space_scene, space_style,
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
    pub(super) fn refresh(&mut self) {
        self.dark = system_uses_dark_mode();
        self.accent = system_accent_color();
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
                && self.system.shell().show_extensions,
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
        space_style(self.theme_for(id), self.system.dark())
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

    /// Scaled icon, label, and check-box sizes for one space.
    pub(super) fn metrics_for(&self, id: SpaceId) -> ItemMetrics {
        ItemMetrics::for_scale(self.space_scale(id))
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
                let mut item = Item::new(ItemId(0), &entry.path, item_facts(&entry.path))
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
    /// Logs when a taskbar falls under the desktop layer, which then takes
    /// its clicks, and when it is clear again, with what Windows reports at
    /// that moment. Diagnostic only: an open bug whose trigger is unknown.
    pub(super) fn check_taskbars(&mut self, ui: &Ui) {
        if !self.layer_active() {
            self.layer.taskbar_blocked = false;
            return;
        }
        let taskbars = ui.taskbars();
        let blocked = taskbars.iter().any(|taskbar| taskbar.is_blocked());
        if blocked == std::mem::replace(&mut self.layer.taskbar_blocked, blocked) {
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
        if blocked {
            tracing::warn!(
                taskbars = ?state,
                ?foreground,
                "a taskbar is under the desktop layer; its clicks go to the layer"
            );
        } else {
            tracing::info!(taskbars = ?state, "every taskbar is above the desktop layer again");
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
        };
        let scene = desktop_layer_scene(
            self.config.desktop.layer(),
            rect.origin(),
            look,
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
        let look = SurfaceLook {
            theme: space.theme(&self.config.theme),
            palette: &palette,
            layout: self.layout_for(id),
            metrics: self.metrics_for(id),
        };
        let mut scene = space_scene(space, look, view, &mut self.icons);
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

    /// Applies a view choice to space `id`, reflows that space alone, and
    /// keeps an open settings window's snapshot. Shared by the View menu and
    /// the Ctrl+Shift keys.
    pub(super) fn set_space_view(&mut self, ui: &mut Ui, id: SpaceId, view: SpaceView, at: Point) {
        if self
            .commit(ui, Command::SetSpaceView { id, view }, at)
            .is_applied()
        {
            self.reflow_space(ui, id);
            self.resnapshot_settings(ui, id);
        }
    }

    /// Ctrl+Shift+1 through 8 (W13): switches the current space to the view
    /// its `VIEW_MODES` digit names. A digit with no view yet, or no space
    /// current, does nothing.
    pub(super) fn set_view_by_digit(&mut self, ui: &mut Ui, digit: u8) {
        let Some(Surface::Space(id)) = self.selection.surface else {
            return;
        };
        let Some(mode) = view_for_digit(digit) else {
            return;
        };
        let at = self.space_center(id);
        self.set_space_view(ui, id, SpaceView { mode }, at);
    }

    /// When `id`'s icon factor changed since its last re-flow (a zoom, a view
    /// change, the icon-size setting, a preset, or Explorer's own size; never
    /// a DPI move, which the factor leaves out), pins a space that sizes
    /// itself to its current size first. It then re-flows into fewer or more
    /// columns, as an Explorer window does when its icon size changes
    /// (`TASKS.md` F15).
    fn keep_size_on_zoom(&mut self, id: SpaceId) {
        let factor = self
            .fixed_icon_scale(id)
            .unwrap_or_else(|| self.icon_scale(self.theme_for(id)));
        let previous = self.view_cache.record_icon_scale(id, factor);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{apply, create_space};
    use deskspace_config::{Config, ConfigStore};
    use deskspace_core::Preset;
    use deskspace_core::geometry::Size;
    use deskspace_core::{SpaceView, ViewMode};
    use deskspace_platform::ShellDisplay;

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
                view: SpaceView {
                    mode: ViewMode::LargeIcons,
                },
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
                view: SpaceView {
                    mode: ViewMode::LargeIcons,
                },
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
