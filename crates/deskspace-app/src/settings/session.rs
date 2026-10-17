//! The settings session: Cancel snapshots, staged hotkey text, and what
//! a revert needs to reflow or restyle.

use deskspace_core::Rect;

use super::ids::page;
use super::tables::HOTKEY_BINDINGS;
use super::*;

/// A space's customization, snapshotted so Cancel can restore it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceSnapshot {
    pub id: SpaceId,
    pub custom: bool,
    pub look: Option<SpaceLook>,
}

impl SpaceSnapshot {
    pub fn of(space: &Space) -> Self {
        Self {
            id: space.id,
            custom: space.custom(),
            look: space.look().cloned(),
        }
    }

    /// The command that puts the space back the way it was.
    pub fn restore(&self) -> Command {
        Command::SetSpaceLook {
            id: self.id,
            custom: self.custom,
            look: self.look.clone(),
        }
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
    pub dirty: bool,
    /// The scope being edited: `None` for Global, or a space.
    pub scope: Option<SpaceId>,
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
            dirty: false,
            scope: None,
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

    /// Switches to `scope`. A page the new scope does not list (a Global-only
    /// page for a space, a space's Shortcuts page for Global) moves back to
    /// the first one.
    pub fn select_scope(&mut self, scope: Option<SpaceId>) {
        self.scope = scope;
        if !listed_pages(scope).contains(&self.page) {
            self.page = page::APPEARANCE;
        }
    }

    /// Selects the `entry`-th page the categories pane lists for the scope.
    pub fn select_entry(&mut self, entry: usize) {
        if let Some(page) = listed_pages(self.scope).get(entry) {
            self.page = *page;
        }
    }

    /// Forgets a space that no longer exists: its Cancel snapshot is dropped,
    /// and if it was the selected scope, selection returns to Global. Called
    /// after a space is deleted from the window, so Cancel cannot resurrect it.
    pub fn forget_space(&mut self, id: SpaceId) {
        if self.scope == Some(id) {
            self.scope = None;
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
    use deskspace_core::Rect;

    fn setup() -> (Config, Session) {
        let config = Config::new();
        let session = Session::new(&config);
        (config, session)
    }

    fn config_with_space(title: &str) -> (Config, SpaceId) {
        let mut config = Config::new();
        let id = config.desktop.add_space(title, Rect::new(0, 0, 200, 200));
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
        let space = config.desktop.space_mut(id).unwrap();
        space.custom = true;
        space.look = Some(SpaceLook::default());
        session.resnapshot(config.desktop.space(id).unwrap());
        assert!(session.saved_spaces[0].custom);
        assert_eq!(session.saved_spaces.len(), 1);
    }

    #[test]
    fn forgetting_a_deleted_space_returns_to_global() {
        let (config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.select_scope(Some(id));
        assert_eq!(session.saved_spaces.len(), 1);
        session.forget_space(id);
        assert_eq!(session.scope, None);
        assert!(session.saved_spaces.is_empty());
    }
}
