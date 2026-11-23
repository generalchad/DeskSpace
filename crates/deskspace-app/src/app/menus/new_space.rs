//! Creating new spaces and lenses, and choosing where they open.

use std::path::PathBuf;

use deskspace_config::NEW_SPACE_SIZE_RANGE;
use deskspace_core::geometry::{Point, Rect, Size};
use deskspace_core::placement::{DEFAULT_SPACE_GAP, drop_space_origin, new_space_origin};
use deskspace_core::source::{FolderView, lens_title};
use deskspace_core::{Command, Space, SpaceId, SpaceView};
use deskspace_platform::{Ui, monitor_dpi_at, scale_for, work_area_at};

use super::super::App;

impl App {
    /// New space (menus, the hotkey, and the debug window's test spaces): in
    /// line with the top row of spaces on the monitor at `at` (ROADMAP D10).
    /// Returns its id.
    pub(in crate::app) fn create_space(&mut self, ui: &mut Ui, at: Point) -> Option<SpaceId> {
        let (origin, size) = self.new_space_spot(at);
        self.open_new_space(ui, origin, size)
    }

    /// New lens…: asks for a folder, then opens a lens on it where New space
    /// would put a space, titled with the folder's name. DeskSpace's storage
    /// folder is refused with a message. Returns its id.
    pub(in crate::app) fn create_lens(&mut self, ui: &mut Ui, at: Point) -> Option<SpaceId> {
        let folder = ui.pick_folder("Choose a folder for the new lens")?;
        if !self.storage.allows_lens(&folder) {
            ui.alert("New lens", &self.storage_refusal());
            return None;
        }
        let (origin, size) = self.new_space_spot(at);
        let (view, space_view) = self.new_lens_settings(folder.clone());
        let title = lens_title(&view, "", &self.profile);
        let command = Command::CreateLens {
            title,
            at: origin,
            size,
            path: folder,
            locked: self.config.window.new_space_locked,
            view: space_view,
        };
        let id = self.show_new_space(ui, command, origin)?;
        self.sync_watches(ui);
        Some(id)
    }

    /// The folder view and space view a new lens starts with, from the Lenses
    /// page's defaults. Pure, so the defaults are tested without a
    /// folder dialog.
    fn new_lens_settings(&self, path: PathBuf) -> (FolderView, SpaceView) {
        let defaults = self.config.window.new_lens.clone();
        (
            FolderView {
                path,
                title: defaults.title,
                show_hidden: defaults.show_hidden,
                ..FolderView::default()
            },
            defaults.view,
        )
    }

    /// Why a lens cannot show DeskSpace's storage folder.
    pub(super) fn storage_refusal(&self) -> String {
        format!(
            "DeskSpace keeps its own files in \u{201C}{}\u{201D}, so a lens cannot show that \
             folder or one inside it.",
            self.storage.root().display()
        )
    }

    /// Where New space puts a new space for the monitor at `at`, and its
    /// size: in line with the top row of spaces there (ROADMAP D10).
    fn new_space_spot(&self, at: Point) -> (Point, Size) {
        let size = self.new_space_size();
        let spaces = self.space_rects();
        let origin = new_space_origin(&spaces, size, work_area_at(at), gap_at(at)).unwrap_or(at);
        (origin, size)
    }

    /// A space for items dropped on the desktop layer (ROADMAP D7): as near
    /// the drop point `at` as it fits with a gutter around it, else where
    /// New space would put it. Returns its id.
    pub(in crate::app) fn create_space_for_drop(
        &mut self,
        ui: &mut Ui,
        at: Point,
    ) -> Option<SpaceId> {
        let size = self.new_space_size();
        let spaces = self.space_rects();
        let (bounds, gap) = (work_area_at(at), gap_at(at));
        let origin = drop_space_origin(at, &spaces, size, bounds, gap)
            .or_else(|| new_space_origin(&spaces, size, bounds, gap))
            .unwrap_or(at);
        self.open_new_space(ui, origin, size)
    }

    /// Creates a space at `origin` with the configured title and lock state,
    /// shows it, and returns its id.
    fn open_new_space(&mut self, ui: &mut Ui, origin: Point, size: Size) -> Option<SpaceId> {
        let id = self.create_new_space(origin, size)?;
        if self.config.window.new_space_icon == deskspace_config::NewSpaceIcon::Automatic {
            self.need_icon_set(ui, deskspace_config::default_set());
        }
        if !self.visible {
            self.set_visible(ui, true);
        }
        self.sync_windows(ui);
        self.schedule_save(ui);
        Some(id)
    }

    pub(in crate::app) fn create_new_space(
        &mut self,
        origin: Point,
        size: Size,
    ) -> Option<SpaceId> {
        let window = &self.config.window;
        let title = window.new_space_title.trim();
        let title = if title.is_empty() {
            "New space".to_string()
        } else {
            title.to_string()
        };
        let id = self.config.desktop.next_space_id();
        let command = new_space_command(window, origin, size, title);
        self.undo.begin("New space");
        // Not `commit`: a new space needs a window (`sync_windows`), not a
        // repaint.
        let applied = self.dispatch(command, origin).is_applied();
        self.undo.finish(&self.config.desktop);
        applied.then_some(id)
    }

    /// Applies `command`, which creates a lens at `origin`, gives it a window,
    /// and returns its id. Spaces are shown first if hidden. New space has its
    /// own path (`create_new_space`) because the undo tests build one without
    /// a `Ui`.
    fn show_new_space(&mut self, ui: &mut Ui, command: Command, origin: Point) -> Option<SpaceId> {
        let id = self.config.desktop.next_space_id();
        let label = if matches!(command, Command::CreateLens { .. }) {
            "New lens"
        } else {
            "New space"
        };
        self.undo.begin(label);
        // Not `commit`: a new space needs a window (`sync_windows`), not a
        // repaint.
        let applied = self.dispatch(command, origin).is_applied();
        self.undo.finish(&self.config.desktop);
        if !applied {
            return None;
        }
        if !self.visible {
            self.set_visible(ui, true);
        }
        self.sync_windows(ui);
        self.schedule_save(ui);
        Some(id)
    }

    /// The configured size of a new space, clamped to its range.
    fn new_space_size(&self) -> Size {
        let window = &self.config.window;
        Size::new(
            window
                .new_space_width
                .clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1),
            window
                .new_space_height
                .clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1),
        )
    }

    fn space_rects(&self) -> Vec<Rect> {
        self.config
            .desktop
            .spaces()
            .iter()
            .map(Space::rect)
            .collect()
    }
}

/// The command a new space is made with, from the window's defaults: its
/// title, place, size, lock, view, and the icon the New spaces page asks
/// for.
fn new_space_command(
    window: &deskspace_config::WindowSettings,
    origin: Point,
    size: Size,
    title: String,
) -> Command {
    Command::CreateSpace {
        title,
        at: origin,
        size,
        locked: window.new_space_locked,
        icon: window.new_space_icon.space_icon(),
        view: window.new_space_view.clone(),
    }
}

/// The default gutter around a new space, scaled for the monitor at `at`.
fn gap_at(at: Point) -> i32 {
    let scale = scale_for(monitor_dpi_at(at));
    (DEFAULT_SPACE_GAP as f32 * scale).round() as i32
}

#[cfg(test)]
mod tests {
    use deskspace_config::{Config, ConfigStore};

    use crate::fixtures;

    use super::*;

    #[test]
    fn a_new_space_takes_the_configured_icon() {
        use deskspace_config::NewSpaceIcon;
        use deskspace_core::{Desktop, SpaceIcon};

        let mut config = Config::new();
        config.window.new_space_icon = NewSpaceIcon::Automatic;
        let mut desktop = Desktop::new();
        let id = desktop.next_space_id();
        fixtures::apply(
            &mut desktop,
            new_space_command(
                &config.window,
                Point::new(0, 0),
                Size::new(240, 120),
                "Games".to_owned(),
            ),
        );
        assert_eq!(desktop.space(id).unwrap().icon(), &SpaceIcon::Automatic);

        config.window.new_space_icon = NewSpaceIcon::None;
        let mut desktop = Desktop::new();
        let id = desktop.next_space_id();
        fixtures::apply(
            &mut desktop,
            new_space_command(
                &config.window,
                Point::new(0, 0),
                Size::new(240, 120),
                "Games".to_owned(),
            ),
        );
        assert!(desktop.space(id).unwrap().icon().is_none());
    }

    #[test]
    fn a_new_space_takes_the_configured_view() {
        use deskspace_core::{Desktop, SpaceView, ViewMode};

        let mut config = Config::new();
        config.window.new_space_view = SpaceView {
            status_bar: true,
            ..SpaceView::default().with_mode(ViewMode::Details)
        };
        let mut desktop = Desktop::new();
        let id = desktop.next_space_id();
        fixtures::apply(
            &mut desktop,
            new_space_command(
                &config.window,
                Point::new(0, 0),
                Size::new(240, 120),
                "Files".to_owned(),
            ),
        );
        let view = desktop.space(id).unwrap().view();
        assert_eq!(view.mode, ViewMode::Details);
        assert!(view.status_bar);
    }

    #[test]
    fn a_new_lens_uses_the_configured_defaults() {
        use deskspace_core::ViewMode;

        let store = ConfigStore::with_path("C:/nowhere/deskspace.toml");
        let mut app = App::new(Config::new(), store, false, false);
        let defaults = &mut app.config.window.new_lens;
        defaults.view.mode = ViewMode::LargeIcons;
        defaults.view.status_bar = false;
        defaults.view.sort.key = deskspace_core::SortKey::Size;
        defaults.title = deskspace_core::LensTitle::FullPath;
        defaults.show_hidden = true;

        let (folder, space_view) = app.new_lens_settings("C:/Temp/lens-test".into());
        assert_eq!(space_view.mode, ViewMode::LargeIcons);
        assert!(!space_view.status_bar);
        assert_eq!(space_view.sort.key, deskspace_core::SortKey::Size);
        assert!(folder.show_hidden);
        assert_eq!(folder.title, deskspace_core::LensTitle::FullPath);
    }
}
