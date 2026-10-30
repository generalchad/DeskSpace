//! The settings session: Cancel snapshots, staged hotkey text, and what
//! a revert needs to reflow or restyle.

use std::collections::HashMap;

use deskspace_core::Rect;
use deskspace_core::source::FolderView;

use super::ids::page;
use super::tables::HOTKEY_BINDINGS;
use super::*;

/// A space's customization, its view, and a lens's folder view, snapshotted
/// so Cancel can restore them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceSnapshot {
    pub id: SpaceId,
    pub title: String,
    pub custom: bool,
    pub look: Option<SpaceLook>,
    /// The space's view settings (W20).
    pub view: SpaceView,
    /// A lens's folder view; `None` for a normal space.
    pub folder: Option<FolderView>,
}

impl SpaceSnapshot {
    pub fn of(space: &Space) -> Self {
        Self {
            id: space.id,
            title: space.title().to_owned(),
            custom: space.custom(),
            look: space.look().cloned(),
            view: space.view(),
            folder: space.folder_view().cloned(),
        }
    }

    /// The command that puts the space's look back.
    pub fn restore(&self) -> Command {
        Command::SetSpaceLook {
            id: self.id,
            custom: self.custom,
            look: self.look.clone(),
        }
    }

    /// The command that puts the space's view settings back.
    pub fn restore_view(&self) -> Command {
        Command::SetSpaceView {
            id: self.id,
            view: self.view,
        }
    }

    /// The command that puts a lens's folder view back, if it had one.
    pub fn restore_folder(&self) -> Option<Command> {
        self.folder
            .clone()
            .map(|view| Command::SetLensView { id: self.id, view })
    }

    /// The command that puts a lens's title mode and text back.
    pub fn restore_title(&self) -> Option<Command> {
        let view = self.folder.as_ref()?;
        Some(Command::SetLensTitle {
            id: self.id,
            title: view.title,
            text: self.title.clone(),
        })
    }
}

/// Edits in progress while the settings window is open. Global edits are
/// applied to the live config immediately for preview, and space edits are
/// dispatched at once; `saved_*` is what Cancel restores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub saved_theme: Theme,
    pub saved_window: WindowSettings,
    pub saved_layout: GridSpec,
    /// The binding text as typed, per `HOTKEY_BINDINGS`, committed on Apply.
    pub hotkey_texts: [String; HOTKEY_BINDINGS.len()],
    pub hotkey_errors: [Option<String>; HOTKEY_BINDINGS.len()],
    /// A lens filter field's text as typed, by space and field id, so the
    /// form shows it back unchanged; the lens holds the parsed list (W35).
    pub filter_texts: HashMap<(SpaceId, u32), String>,
    pub dirty: bool,
    /// The scope being edited: `None` for Global, or a space.
    pub scope: Option<SpaceId>,
    /// The kind of the selected scope, so its page list is known without
    /// looking the space up again.
    pub scope_kind: ScopeKind,
    /// The selected page, an index into the form's pages (`page::*`).
    pub page: usize,
    /// Every space's customization when the window opened or was last
    /// applied, which Cancel restores.
    pub saved_spaces: Vec<SpaceSnapshot>,
    /// Spaces an icon-size preview pinned at the size they had (`TASKS.md`
    /// F15). Cancel lets those still at that size fit their items again.
    pub pinned: Vec<(SpaceId, Rect)>,
}

/// Every binding's saved text, in `HOTKEY_BINDINGS` order.
fn saved_hotkey_texts(window: &WindowSettings) -> [String; HOTKEY_BINDINGS.len()] {
    std::array::from_fn(|slot| HOTKEY_BINDINGS[slot].text(window).to_string())
}

impl Session {
    pub fn new(config: &Config) -> Self {
        Self {
            saved_theme: config.theme.clone(),
            saved_window: config.window.clone(),
            saved_layout: config.desktop.layout,
            hotkey_texts: saved_hotkey_texts(&config.window),
            hotkey_errors: Default::default(),
            filter_texts: HashMap::new(),
            dirty: false,
            scope: None,
            scope_kind: ScopeKind::Global,
            page: page::APPEARANCE,
            saved_spaces: snapshot_spaces(config),
            pinned: Vec::new(),
        }
    }

    /// Marks the current config as the new baseline after a successful save.
    pub fn commit(&mut self, config: &Config) {
        self.saved_theme = config.theme.clone();
        self.saved_window = config.window.clone();
        self.saved_layout = config.desktop.layout;
        self.saved_spaces = snapshot_spaces(config);
        self.pinned.clear();
        self.hotkey_texts = saved_hotkey_texts(&config.window);
        self.hotkey_errors = Default::default();
        self.dirty = false;
    }

    /// Restores the global settings. Spaces are restored by the app, through
    /// the commands from `SpaceSnapshot::restore`.
    pub fn revert(&self, config: &mut Config) {
        config.theme = self.saved_theme.clone();
        config.window = self.saved_window.clone();
        config.desktop.layout = self.saved_layout;
    }

    /// Takes a change made outside the window (the space's context menu,
    /// which commits at once) into the space's Cancel snapshot, so Cancel
    /// does not undo it.
    pub fn resnapshot(&mut self, space: &Space) {
        let snapshot = SpaceSnapshot::of(space);
        match self
            .saved_spaces
            .iter_mut()
            .find(|saved| saved.id == space.id)
        {
            Some(saved) => *saved = snapshot,
            None => self.saved_spaces.push(snapshot),
        }
    }

    /// Switches to `scope` of `kind`. A page the new scope does not list (a
    /// Global-only page for a space, a lens page for a normal space) moves
    /// back to the first one.
    pub fn select_scope(&mut self, scope: Option<SpaceId>, kind: ScopeKind) {
        self.scope = scope;
        self.scope_kind = kind;
        if !listed_pages(kind).contains(&self.page) {
            self.page = page::APPEARANCE;
        }
    }

    /// Selects the `entry`-th page the categories pane lists for the scope.
    pub fn select_entry(&mut self, entry: usize) {
        if let Some(page) = listed_pages(self.scope_kind).get(entry) {
            self.page = *page;
        }
    }

    /// Forgets a space that no longer exists: its Cancel snapshot is dropped,
    /// and if it was the selected scope, selection returns to Global. Called
    /// after a space is deleted from the window, so Cancel cannot resurrect it.
    pub fn forget_space(&mut self, id: SpaceId) {
        if self.scope == Some(id) {
            self.scope = None;
            self.scope_kind = ScopeKind::Global;
        }
        self.saved_spaces.retain(|snapshot| snapshot.id != id);
    }
}

fn snapshot_spaces(config: &Config) -> Vec<SpaceSnapshot> {
    config
        .desktop
        .spaces()
        .iter()
        .map(SpaceSnapshot::of)
        .collect()
}

/// Whether reverting to `saved` needs the spaces re-flowed rather than just
/// restyled: the grid, the icon size that scales it, or Explorer matching,
/// which imports the desktop's own icon size and item spacing.
pub fn needs_relayout(saved: &Session, config: &Config) -> bool {
    saved.saved_layout != config.desktop.layout
        || saved.saved_theme.icon_size != config.theme.icon_size
        || saved.saved_theme.preset != config.theme.preset
}

/// Whether reverting to `saved` needs the spaces restyled. `ui_mode` is
/// excluded: it only affects DeskSpace's own chrome (the settings window and
/// menus), which the app updates separately, so a change to it alone must not
/// repaint the spaces.
pub fn needs_restyle(saved: &Session, config: &Config) -> bool {
    let mut saved_theme = saved.saved_theme.clone();
    saved_theme.ui_mode = config.theme.ui_mode;
    saved_theme != config.theme
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{apply, create_lens, create_space};
    use deskspace_core::Rect;
    use std::path::Path;

    fn setup() -> (Config, Session) {
        let config = Config::new();
        let session = Session::new(&config);
        (config, session)
    }

    fn config_with_space(title: &str) -> (Config, SpaceId) {
        let mut config = Config::new();
        let id = create_space(&mut config.desktop, title, Rect::new(0, 0, 200, 200));
        (config, id)
    }

    #[test]
    fn needs_relayout_tracks_grid_affecting_settings() {
        let (config, session) = setup();
        assert!(!needs_relayout(&session, &config));

        let mut preset = config.clone();
        preset.theme.preset = Preset::Custom;
        assert!(needs_relayout(&session, &preset));

        let mut icon_size = config.clone();
        icon_size.theme.icon_size = icon_size.theme.icon_size.wrapping_add(10);
        assert!(needs_relayout(&session, &icon_size));

        let mut gap = config.clone();
        gap.desktop.layout.gap = gap.desktop.layout.gap.wrapping_add(1);
        assert!(needs_relayout(&session, &gap));

        let mut restyled = config.clone();
        restyled.theme.tint_opacity = restyled.theme.tint_opacity.wrapping_add(1);
        assert!(!needs_relayout(&session, &restyled));
    }

    #[test]
    fn needs_restyle_ignores_ui_mode() {
        let (config, session) = setup();

        let mut ui_mode = config.clone();
        ui_mode.theme.ui_mode = ThemeMode::Dark;
        assert!(
            !needs_restyle(&session, &ui_mode),
            "a chrome-only change must not repaint the spaces"
        );

        let mut restyled = config.clone();
        restyled.theme.tint_opacity = restyled.theme.tint_opacity.wrapping_add(1);
        assert!(needs_restyle(&session, &restyled));
    }

    #[test]
    fn cancel_restores_the_saved_grid() {
        let (mut config, mut session) = setup();
        apply_field(
            &mut config,
            &mut session,
            field::GRID_GAP,
            FieldValue::Slider(0),
            true,
        );
        assert_ne!(config.desktop.layout, Config::new().desktop.layout);
        session.revert(&mut config);
        assert_eq!(config.desktop.layout, Config::new().desktop.layout);
    }

    #[test]
    fn apply_forgets_the_spaces_a_preview_pinned() {
        let (config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.pinned.push((id, Rect::new(0, 0, 200, 150)));
        session.commit(&config);
        assert!(session.pinned.is_empty());
    }

    #[test]
    fn snapshots_restore_and_follow_menu_changes() {
        let (mut config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        assert_eq!(
            session.saved_spaces[0].restore(),
            Command::SetSpaceLook {
                id,
                custom: false,
                look: None
            }
        );
        apply(
            &mut config.desktop,
            Command::SetSpaceLook {
                id,
                custom: true,
                look: Some(SpaceLook::default()),
            },
        );
        session.resnapshot(config.desktop.space(id).unwrap());
        assert!(session.saved_spaces[0].custom);
        assert_eq!(session.saved_spaces.len(), 1);
    }

    #[test]
    fn a_snapshot_restores_the_view() {
        let (mut config, id) = config_with_space("Work");
        let session = Session::new(&config);
        let snapshot = session.saved_spaces[0].clone();
        assert_eq!(snapshot.view, SpaceView::default());
        let view = SpaceView {
            mode: ViewMode::LargeIcons,
            status_bar: true,
            ..SpaceView::default()
        };
        apply(&mut config.desktop, Command::SetSpaceView { id, view });
        assert_eq!(config.desktop.space(id).unwrap().view(), view);
        apply(&mut config.desktop, snapshot.restore_view());
        assert_eq!(
            config.desktop.space(id).unwrap().view(),
            SpaceView::default()
        );
    }

    #[test]
    fn a_lens_snapshot_restores_its_view() {
        let mut config = Config::new();
        let lens = create_lens(
            &mut config.desktop,
            "Downloads",
            Rect::new(0, 0, 200, 200),
            "C:/Temp/lens-test",
        );
        let session = Session::new(&config);
        let snapshot = session
            .saved_spaces
            .iter()
            .find(|snapshot| snapshot.id == lens)
            .unwrap()
            .clone();
        assert!(!snapshot.folder.as_ref().unwrap().show_hidden);

        let mut view = snapshot.folder.clone().unwrap();
        view.show_hidden = true;
        apply(&mut config.desktop, Command::SetLensView { id: lens, view });
        assert!(
            config
                .desktop
                .space(lens)
                .unwrap()
                .folder_view()
                .unwrap()
                .show_hidden
        );

        apply(&mut config.desktop, snapshot.restore_folder().unwrap());
        assert!(
            !config
                .desktop
                .space(lens)
                .unwrap()
                .folder_view()
                .unwrap()
                .show_hidden
        );
    }

    #[test]
    fn a_lens_snapshot_restores_its_title() {
        let mut config = Config::new();
        let lens = create_lens(
            &mut config.desktop,
            "Downloads",
            Rect::new(0, 0, 200, 200),
            "C:/Temp/lens-test",
        );
        let session = Session::new(&config);
        let snapshot = session
            .saved_spaces
            .iter()
            .find(|snapshot| snapshot.id == lens)
            .unwrap()
            .clone();
        assert_eq!(snapshot.title, "Downloads");

        apply(
            &mut config.desktop,
            Command::SetLensTitle {
                id: lens,
                title: LensTitle::Custom,
                text: "Mine".into(),
            },
        );
        assert_eq!(config.desktop.space(lens).unwrap().title(), "Mine");

        apply(&mut config.desktop, snapshot.restore_title().unwrap());
        let space = config.desktop.space(lens).unwrap();
        assert_eq!(space.title(), "Downloads");
        assert_eq!(space.folder_view().unwrap().title, LensTitle::FolderName);
    }

    #[test]
    fn a_menu_folder_change_is_kept_in_the_snapshot() {
        let mut config = Config::new();
        let lens = create_lens(
            &mut config.desktop,
            "Downloads",
            Rect::new(0, 0, 200, 200),
            "C:/Temp/lens-old",
        );
        let mut session = Session::new(&config);
        let old = session
            .saved_spaces
            .iter()
            .find(|snapshot| snapshot.id == lens)
            .unwrap()
            .clone();

        apply(
            &mut config.desktop,
            Command::SetLensFolder {
                id: lens,
                path: "C:/Temp/lens-new".into(),
                title: "Downloads".into(),
            },
        );
        session.resnapshot(config.desktop.space(lens).unwrap());
        let kept = session
            .saved_spaces
            .iter()
            .find(|snapshot| snapshot.id == lens)
            .unwrap();
        assert_eq!(
            kept.folder.as_ref().unwrap().path.as_path(),
            Path::new("C:/Temp/lens-new")
        );
        assert_eq!(
            old.folder.as_ref().unwrap().path.as_path(),
            Path::new("C:/Temp/lens-old"),
            "the snapshot taken at open had the old path"
        );
    }

    #[test]
    fn forgetting_a_deleted_space_returns_to_global() {
        let (config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.select_scope(Some(id), ScopeKind::Space);
        assert_eq!(session.saved_spaces.len(), 1);
        session.forget_space(id);
        assert_eq!(session.scope, None);
        assert!(session.saved_spaces.is_empty());
    }
}
