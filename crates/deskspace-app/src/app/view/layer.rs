//! The desktop layer and takeover state for `App`.

use std::collections::HashSet;

use deskspace_core::{Command, Item, ItemId, Space};
use deskspace_platform::{
    CheckBoxMode, DesktopEntry, IconUser, SpaceStyle, TaskbarRow, Ui, arm_watchdog,
    desktop_display_since, enumerate_desktop_items, foreground_window, hide_desktop_icons,
    rehide_desktop_icons, restore_desktop_icons, virtual_desktop_rect,
};

use crate::scene::{SurfaceLook, desktop_layer_scene, layer_style};
use crate::storage::path_key;

use super::super::{App, Surface, Timer};
use super::changed_in_place;

/// How often, while takeover is on, to re-hide Explorer's icon layer in case
/// it recreated it without broadcasting `TaskbarCreated`.
const HEALTH_MS: u32 = 5_000;

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

    /// How many loose desktop items the shell listed.
    pub(in crate::app) fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl App {
    /// Re-reads Explorer's display options and repaints if they changed. A
    /// changed icon size or item spacing re-flows every surface that matches
    /// Explorer too.
    pub(in crate::app) fn refresh_shell_display(&mut self, ui: &mut Ui) {
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
    pub(in crate::app) fn refresh_desktop_entries(&mut self) {
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
    pub(in crate::app) fn layer_style(&self) -> SpaceStyle {
        layer_style(&self.config.theme, self.system.dark())
    }

    /// Whether the desktop layer window is drawn: the additive
    /// `--takeover-layer`, or the full takeover (`--takeover` or the setting).
    pub(in crate::app) fn layer_active(&self) -> bool {
        self.layer.layer_flag || self.desktop_takeover()
    }

    /// Whether DeskSpace takes over the desktop by hiding Explorer's icons:
    /// the debug window's session switch when set, else the hidden
    /// `--takeover` flag or the saved setting.
    pub(in crate::app) fn desktop_takeover(&self) -> bool {
        self.layer
            .session_takeover
            .unwrap_or(self.layer.takeover_flag || self.config.window.take_over_desktop)
    }

    /// Takes over the desktop, or gives it back, for this session only, as
    /// the setting would; the saved setting does not change.
    pub(in crate::app) fn set_session_takeover(&mut self, ui: &mut Ui, on: bool) {
        self.layer.session_takeover = Some(on);
        self.apply_takeover(ui);
    }

    /// The loose desktop items for the layer: the desktop entries that carry a
    /// position and are not already held by a space. The `ItemId`s are
    /// placeholders; `SyncDesktop` reconciles them by path.
    pub(in crate::app) fn desktop_layer_items(&self) -> Vec<Item> {
        let in_spaces: HashSet<_> = self
            .config
            .desktop
            .spaces()
            .iter()
            .flat_map(Space::items)
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
    pub(in crate::app) fn sync_desktop_layer(&mut self, ui: &mut Ui) -> bool {
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
            self.layer_layout(),
            rect,
        );
        self.refresh_desktop_layer(ui);
        true
    }

    /// Brings the layer and Explorer-icon hiding in line with the settings:
    /// draw the layer, then hide or restore Explorer's icons. Hiding waits
    /// until the layer is up, so a failure never leaves the desktop blank.
    pub(in crate::app) fn apply_takeover(&mut self, ui: &mut Ui) {
        let takeover = self.desktop_takeover();
        // Follow the Desktop folder while the layer is drawn, and every
        // lens's folder, so files added or removed outside DeskSpace reach
        // what DeskSpace draws (M4 phase 2).
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
    pub(in crate::app) fn check_taskbars(&mut self, ui: &mut Ui) {
        if !self.layer_active() {
            self.layer.taskbar_blocked = false;
            return;
        }
        let taskbars = ui.taskbars();
        let found = taskbars.iter().any(TaskbarRow::is_blocked);
        let blocked = found && {
            ui.sink_desktop_layer();
            ui.taskbars().iter().any(TaskbarRow::is_blocked)
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

    pub(in crate::app) fn health_tick(&mut self, ui: &mut Ui) {
        self.check_taskbars(ui);
        if !cfg!(test) {
            rehide_desktop_icons();
        }
    }

    /// Pushes the desktop layer's rect and draw list to its window.
    pub(in crate::app) fn refresh_desktop_layer(&mut self, ui: &mut Ui) {
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
        self.icons.begin_scene(IconUser::DesktopLayer);
        let mut scene = desktop_layer_scene(
            self.config.desktop.layer(),
            rect.origin(),
            look,
            view,
            &mut self.icons,
        );
        self.icons.end_scene();
        self.show_edit(&mut scene, Surface::Desktop);
        ui.set_desktop_layer_rect(rect);
        ui.set_desktop_layer_scene(scene);
    }
}

#[cfg(test)]
mod tests {
    use deskspace_config::{Config, ConfigStore};

    use super::*;

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
}
