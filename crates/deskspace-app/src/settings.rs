//! The settings window's content and how edits map onto the configuration.
//! Pure functions over `Config`, so the form logic is testable without Win32.
//!
//! The window has three panes: the scopes (Global, then each space indented
//! under it), the pages of the selected scope, and the page itself. Global
//! edits write the config directly, as a live preview. A space follows Global
//! until its "Customize this space" switch is on; its edits then become
//! `Command`s the app dispatches, so `Desktop` is only changed through them.

use std::path::Path;

use deskspace_config::{Config, NEW_SPACE_SIZE_RANGE, WindowSettings};
use deskspace_core::layout::{GAP_RANGE, SNAP_RANGE};
use deskspace_core::theme::{
    ICON_SIZE_RANGE, LABEL_SIZE_RANGE, OVERLAY_SIZE_DEFAULT, OVERLAY_SIZE_RANGE, corner,
};
use deskspace_core::{
    Backdrop, Command, GridSpec, LabelShadow, Preset, Space, SpaceId, SpaceLook, Theme, ThemeMode,
};
use deskspace_platform::{
    Field, FieldValue, SettingsButton, SettingsForm, SettingsHeader, SettingsNav, SettingsNavEntry,
    SettingsNavGroup, SettingsPage, SettingsScope, SettingsSwitch, SliderMark,
};

pub mod field {
    pub const SPACES_HEADING: u32 = 1;
    pub const MODE: u32 = 2;
    pub const BACKDROP: u32 = 3;
    pub const CORNERS: u32 = 4;
    pub const TINT_OPACITY: u32 = 5;
    pub const TITLE_SHADING: u32 = 6;
    pub const BORDER: u32 = 7;
    pub const COLORS_HEADING: u32 = 8;
    pub const CUSTOM_COLORS: u32 = 9;
    pub const TINT: u32 = 10;
    pub const TEXT: u32 = 11;
    pub const RESET: u32 = 12;
    pub const COMPATIBILITY_NOTE: u32 = 13;
    pub const LABELS_HEADING: u32 = 14;
    pub const LABEL_LINES: u32 = 15;
    pub const LABEL_SIZE: u32 = 16;
    pub const LABEL_SHADOW: u32 = 17;
    pub const LAYOUT_HEADING: u32 = 18;
    pub const GRID_GAP: u32 = 19;
    pub const GRID_PADDING: u32 = 41;
    pub const PRESET: u32 = 42;
    pub const ICON_SIZE: u32 = 43;

    pub const ADVANCED_HEADING: u32 = 44;
    pub const OVERLAY_SIZE: u32 = 45;
    pub const OVERLAY_NOTE: u32 = 46;

    pub const BEHAVIOR_HEADING: u32 = 20;
    pub const HOTKEY: u32 = 21;
    pub const HOTKEY_NOTE: u32 = 22;
    pub const START_HIDDEN: u32 = 23;
    pub const SNAP: u32 = 24;
    pub const ALWAYS_ON_DESKTOP: u32 = 25;
    pub const ALWAYS_ON_DESKTOP_NOTE: u32 = 26;

    pub const ABOUT_HEADING: u32 = 30;
    pub const ABOUT_SUMMARY: u32 = 31;
    pub const ABOUT_LICENSE: u32 = 32;
    pub const CONFIG_PATH: u32 = 33;
    pub const USE_ACCENT: u32 = 47;
    pub const UI_MODE: u32 = 48;
    pub const START_WITH_WINDOWS: u32 = 49;
    pub const TAKE_OVER_DESKTOP: u32 = 50;

    pub const HOTKEYS_HEADING: u32 = 51;
    pub const HOTKEY_NEW_SPACE: u32 = 52;
    pub const HOTKEY_NEW_SPACE_NOTE: u32 = 53;
    pub const HOTKEY_SETTINGS: u32 = 54;
    pub const HOTKEY_SETTINGS_NOTE: u32 = 55;

    pub const NEW_SPACES_HEADING: u32 = 56;
    pub const NEW_SPACE_TITLE: u32 = 57;
    pub const NEW_SPACE_WIDTH: u32 = 58;
    pub const NEW_SPACE_HEIGHT: u32 = 59;
    pub const NEW_SPACE_LOCKED: u32 = 60;
    pub const NEW_SPACES_NOTE: u32 = 61;
    pub const FIT_CONTENTS: u32 = 62;
}

/// The pages, in rail order. The first `LOOK_PAGE_COUNT` hold look settings,
/// which a space can override; General, New spaces, Hotkeys, and About are
/// Global only.
pub mod page {
    pub const APPEARANCE: usize = 0;
    pub const LAYOUT: usize = 2;
    pub const ABOUT: usize = 8;
}

/// How many pages hold look settings (Appearance through Advanced).
pub const LOOK_PAGE_COUNT: usize = 5;

const MODES: [(ThemeMode, &str); 3] = [
    (ThemeMode::System, "Use Windows setting"),
    (ThemeMode::Dark, "Dark"),
    (ThemeMode::Light, "Light"),
];
const BACKDROPS: [(Backdrop, &str); 4] = [
    (Backdrop::Acrylic, "Acrylic (frosted)"),
    (Backdrop::Blur, "Blur"),
    (Backdrop::Clear, "Clear glass (no blur)"),
    (Backdrop::Solid, "Solid"),
];
const LABEL_LINES_OPTIONS: [(u8, &str); 2] = [(1, "One line"), (2, "Two lines")];
const LABEL_SHADOW_OPTIONS: [(LabelShadow, &str); 3] = [
    (LabelShadow::Auto, "Automatic (when nearly transparent)"),
    (LabelShadow::On, "Always"),
    (LabelShadow::Off, "Never"),
];

/// The item-display presets the Appearance page offers, in dropdown order.
/// "Match Explorer's display" mirrors Explorer's icon size, spacing, check
/// boxes, and extensions; "Custom" uses the fields below. The selection maps
/// onto `Theme.preset`.
const PRESETS: [&str; 2] = ["Match Explorer's display", "Custom"];

fn preset_index(preset: Preset) -> usize {
    match preset {
        Preset::MatchExplorer => 0,
        Preset::Custom => 1,
    }
}

fn corner_marks() -> Vec<SliderMark> {
    vec![
        SliderMark::new(corner::SQUARE.into(), "Square"),
        SliderMark::new(corner::WINDOWS.into(), "Windows 11"),
        SliderMark::new(corner::ROUND.into(), "Round"),
        SliderMark::new(corner::MAX.into(), "Max"),
    ]
}

const HOTKEY_HINT: &str = "Combine Ctrl, Alt, Shift, or Win with a letter, digit, or F1-F24, \
     for example Ctrl+Alt+D. Empty leaves the action unbound.";

/// A global hotkey the Hotkeys page can bind: the field that edits it, the note
/// field below it, its label, and the registration index the app passes to
/// `Ui::register_hotkey`.
#[derive(Clone, Copy, Debug)]
pub struct HotkeyBinding {
    pub field: u32,
    pub note: u32,
    pub label: &'static str,
    pub index: i32,
}

/// The global hotkeys, in registration order. The app iterates this to register
/// and to apply each binding, so a new binding is one entry here plus its
/// `WindowSettings` field.
pub const HOTKEY_BINDINGS: [HotkeyBinding; 3] = [
    HotkeyBinding {
        field: field::HOTKEY,
        note: field::HOTKEY_NOTE,
        label: "Show or hide spaces",
        index: 0,
    },
    HotkeyBinding {
        field: field::HOTKEY_NEW_SPACE,
        note: field::HOTKEY_NEW_SPACE_NOTE,
        label: "Create a new space",
        index: 1,
    },
    HotkeyBinding {
        field: field::HOTKEY_SETTINGS,
        note: field::HOTKEY_SETTINGS_NOTE,
        label: "Open settings",
        index: 2,
    },
];

/// The stored text of the binding at `index`.
pub fn hotkey_setting(window: &WindowSettings, index: i32) -> &str {
    match index {
        0 => &window.toggle_hotkey,
        1 => &window.new_space_hotkey,
        2 => &window.settings_hotkey,
        _ => "",
    }
}

/// Sets the stored text of the binding at `index`.
pub fn set_hotkey_setting(window: &mut WindowSettings, index: i32, text: String) {
    match index {
        0 => window.toggle_hotkey = text,
        1 => window.new_space_hotkey = text,
        2 => window.settings_hotkey = text,
        _ => {}
    }
}

fn is_hotkey_field(id: u32) -> bool {
    HOTKEY_BINDINGS.iter().any(|binding| binding.field == id)
}

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
            custom: space.custom,
            look: space.look.clone(),
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
    /// The selected page, an index into the form's pages.
    pub page: usize,
    /// Every space's customization when the window opened or was last
    /// applied, which Cancel restores.
    pub saved_spaces: Vec<SpaceSnapshot>,
}

impl Session {
    pub fn new(config: &Config) -> Self {
        Self {
            saved_theme: config.theme.clone(),
            saved_window: config.window.clone(),
            saved_layout: config.desktop.layout,
            hotkey_texts: std::array::from_fn(|slot| {
                hotkey_setting(&config.window, HOTKEY_BINDINGS[slot].index).to_string()
            }),
            hotkey_errors: Default::default(),
            dirty: false,
            scope: None,
            page: page::APPEARANCE,
            saved_spaces: snapshot_spaces(config),
        }
    }

    /// Marks the current config as the new baseline after a successful save.
    pub fn commit(&mut self, config: &Config) {
        self.saved_theme = config.theme.clone();
        self.saved_window = config.window.clone();
        self.saved_layout = config.desktop.layout;
        self.saved_spaces = snapshot_spaces(config);
        self.hotkey_texts = std::array::from_fn(|slot| {
            hotkey_setting(&config.window, HOTKEY_BINDINGS[slot].index).to_string()
        });
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

    /// Switches to `scope`. A space has only the look pages, so a Global-only
    /// page moves back to the first one.
    pub fn select_scope(&mut self, scope: Option<SpaceId>) {
        self.scope = scope;
        if scope.is_some() && self.page >= LOOK_PAGE_COUNT {
            self.page = page::APPEARANCE;
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
        .spaces
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

fn index_of<T: PartialEq + Copy>(table: &[(T, &str)], value: T) -> usize {
    table
        .iter()
        .position(|(entry, _)| *entry == value)
        .unwrap_or(0)
}

fn labels<T>(table: &[(T, &'static str)]) -> Vec<&'static str> {
    table.iter().map(|(_, label)| *label).collect()
}

/// The space the session edits, if it still exists; `None` is Global.
fn scope_space<'a>(config: &'a Config, session: &Session) -> Option<&'a Space> {
    session.scope.and_then(|id| config.desktop.space(id))
}

/// The scope index (in the scopes pane) of the session's scope: 0 for
/// Global, then the spaces in order.
pub fn scope_index(config: &Config, session: &Session) -> usize {
    scope_space(config, session)
        .and_then(|space| {
            config
                .desktop
                .spaces
                .iter()
                .position(|other| other.id == space.id)
        })
        .map_or(0, |index| index + 1)
}

/// The scope at `index` in the scopes pane: `None` for Global (0) or an index
/// past the spaces.
pub fn scope_at(config: &Config, index: usize) -> Option<SpaceId> {
    index
        .checked_sub(1)
        .and_then(|index| config.desktop.spaces.get(index))
        .map(|space| space.id)
}

pub fn build_form(
    config: &Config,
    session: &Session,
    system_dark: bool,
    config_path: &Path,
) -> SettingsForm {
    let space = scope_space(config, session);
    let global = &config.theme;
    // A space shows what it is drawn with: its own look while customized,
    // else the global values, greyed until the switch is turned on.
    let theme = space.map_or(global, |space| space.theme(global));
    let grid = space.map_or(config.desktop.layout, |space| {
        space.grid(config.desktop.layout)
    });
    let editable = space.is_none_or(|space| space.custom);

    let mut pages = look_pages(theme, &grid, system_dark, space.is_some());
    if !editable {
        for field in pages.iter_mut().flat_map(|page| page.fields.iter_mut()) {
            field.enabled = false;
        }
    }
    // Always built, so the form keeps its shape when the scope changes; it
    // applies only to a resized, unlocked space and is grey for Global. Fitting
    // is geometry, not a look setting, so it is not caught by the `editable`
    // disable above and stays available while a space follows Global, matching
    // the space menu's "Fit to contents".
    pages[page::LAYOUT].fields.push(
        Field::button(field::FIT_CONTENTS, "", "Fit to contents")
            .enabled(space.is_some_and(|space| space.user_sized && !space.locked)),
    );
    pages.push(general_page(config));
    pages.push(new_spaces_page(config));
    pages.push(hotkeys_page(session));
    pages.push(about_page(config_path));
    debug_assert_eq!(pages.len(), page::ABOUT + 1);

    let page_count = if space.is_some() {
        LOOK_PAGE_COUNT
    } else {
        pages.len()
    };
    let entries = pages[..page_count]
        .iter()
        .enumerate()
        .map(|(index, page)| SettingsNavEntry {
            label: page.title.clone(),
            page: index,
            marked: false,
        })
        .collect();

    let mut scopes = vec![SettingsScope {
        label: "Global".into(),
        indent: false,
        dimmed: false,
    }];
    scopes.extend(config.desktop.spaces.iter().map(|space| SettingsScope {
        label: space.title.clone(),
        indent: true,
        dimmed: !space.custom,
    }));
    let header = match space {
        None => SettingsHeader {
            title: "Global settings".into(),
            switch: None,
            button: None,
        },
        Some(space) => SettingsHeader {
            title: space.title.clone(),
            switch: Some(SettingsSwitch {
                label: "Customize this space".into(),
                on: space.custom,
            }),
            button: Some(SettingsButton {
                // The ellipsis promises a confirmation dialog, which only a
                // non-empty space shows (matching the space menu).
                label: if space.items.is_empty() {
                    "Delete space".into()
                } else {
                    "Delete space\u{2026}".into()
                },
            }),
        },
    };

    SettingsForm {
        title: "DeskSpace Settings".into(),
        pages,
        apply_enabled: session.dirty,
        nav: Some(SettingsNav {
            scopes,
            selected_scope: scope_index(config, session),
            groups: vec![SettingsNavGroup {
                title: String::new(),
                entries,
            }],
            selected: session.page.min(page_count - 1),
            header: Some(header),
        }),
    }
}

/// The pages a space can override: Appearance, Labels, Layout, Colors, and
/// Advanced, showing `theme` and `grid`.
fn look_pages(
    theme: &Theme,
    grid: &GridSpec,
    system_dark: bool,
    for_space: bool,
) -> Vec<SettingsPage> {
    let palette = theme.palette(system_dark);
    let solid = theme.backdrop == Backdrop::Solid;

    // Appearance is split across several pages so no page is tall enough to
    // clip on a 1080-pixel display.
    let appearance = SettingsPage {
        title: "Appearance".into(),
        fields: vec![
            Field::heading(field::SPACES_HEADING, "Spaces"),
            Field::choice(
                field::MODE,
                "Color mode",
                &labels(&MODES),
                index_of(&MODES, theme.mode),
            ),
            Field::choice(
                field::BACKDROP,
                "Background material",
                &labels(&BACKDROPS),
                index_of(&BACKDROPS, theme.backdrop),
            ),
            Field::slider(
                field::CORNERS,
                "Corner radius",
                (corner::SQUARE.into(), corner::MAX.into()),
                theme.corner_radius.min(corner::MAX).into(),
            )
            .suffix(" px")
            .marks(corner_marks()),
            Field::slider(
                field::TINT_OPACITY,
                "Tint opacity",
                (0, 100),
                theme.tint_opacity.min(100) as i32,
            )
            .enabled(!solid),
            Field::slider(
                field::TITLE_SHADING,
                "Title bar shading",
                (0, 100),
                theme.title_shading.min(100) as i32,
            ),
            Field::toggle(field::BORDER, "Show window border", theme.show_border),
            Field::choice(
                field::PRESET,
                "Item preset",
                &PRESETS,
                preset_index(theme.preset),
            ),
        ],
    };

    let labels_page = SettingsPage {
        title: "Labels".into(),
        fields: vec![
            Field::heading(field::LABELS_HEADING, "Labels"),
            Field::choice(
                field::LABEL_LINES,
                "Label lines",
                &labels(&LABEL_LINES_OPTIONS),
                index_of(&LABEL_LINES_OPTIONS, theme.label_lines.clamp(1, 2)),
            ),
            Field::slider(
                field::LABEL_SIZE,
                "Label size",
                (LABEL_SIZE_RANGE.0.into(), LABEL_SIZE_RANGE.1.into()),
                theme
                    .label_size
                    .clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1) as i32,
            ),
            Field::choice(
                field::LABEL_SHADOW,
                "Label shadow",
                &labels(&LABEL_SHADOW_OPTIONS),
                index_of(&LABEL_SHADOW_OPTIONS, theme.label_shadow),
            ),
        ],
    };

    let layout_page = SettingsPage {
        title: "Layout".into(),
        fields: vec![
            Field::heading(field::LAYOUT_HEADING, "Layout"),
            Field::slider(
                field::ICON_SIZE,
                "Icon size (%)",
                (ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into()),
                theme.icon_size.clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as i32,
            )
            .editable()
            .marks(vec![
                SliderMark::new(ICON_SIZE_RANGE.0.into(), "Small"),
                SliderMark::new(100, "Default"),
                SliderMark::new(ICON_SIZE_RANGE.1.into(), "Large"),
            ])
            .enabled(!theme.match_explorer()),
            Field::slider(
                field::GRID_GAP,
                "Icon spacing",
                (GAP_RANGE.0, GAP_RANGE.1),
                grid.gap.clamp(GAP_RANGE.0, GAP_RANGE.1),
            )
            .suffix(" px")
            .enabled(!theme.match_explorer()),
            Field::slider(
                field::GRID_PADDING,
                "Edge padding",
                (GAP_RANGE.0, GAP_RANGE.1),
                grid.padding.clamp(GAP_RANGE.0, GAP_RANGE.1),
            )
            .suffix(" px")
            .enabled(!theme.match_explorer()),
        ],
    };

    let reset = if for_space {
        "Match global appearance"
    } else {
        "Restore default appearance"
    };
    let colors_page = SettingsPage {
        title: "Colors".into(),
        fields: vec![
            Field::heading(field::COLORS_HEADING, "Colors"),
            Field::toggle(
                field::CUSTOM_COLORS,
                "Use custom colors instead of the color mode's",
                theme.custom_colors,
            ),
            Field::color(field::TINT, "Background tint", palette.tint.with_alpha(255))
                .enabled(theme.custom_colors),
            Field::color(field::TEXT, "Text", palette.title_text).enabled(theme.custom_colors),
            Field::toggle(
                field::USE_ACCENT,
                "Use Windows accent color for the selection and border",
                theme.use_accent,
            ),
            Field::button(field::RESET, "", reset),
            Field::note(
                field::COMPATIBILITY_NOTE,
                "Clear glass at 0% tint and 0% title shading, without a border, \
                 shows only the icons and labels. Acrylic and blur use Windows 11's \
                 own corners, up to 8 px; larger radii apply to clear glass and \
                 solid. Desktop-hosted spaces are drawn opaque.",
            ),
        ],
    };

    let advanced_page = SettingsPage {
        title: "Advanced".into(),
        fields: vec![
            Field::heading(field::ADVANCED_HEADING, "Advanced"),
            Field::slider(
                field::OVERLAY_SIZE,
                "Shortcut overlay size (%)",
                (OVERLAY_SIZE_RANGE.0.into(), OVERLAY_SIZE_RANGE.1.into()),
                theme
                    .overlay_size
                    .clamp(OVERLAY_SIZE_RANGE.0, OVERLAY_SIZE_RANGE.1)
                    .into(),
            )
            .editable()
            .marks(vec![
                SliderMark::new(OVERLAY_SIZE_RANGE.0.into(), "Small"),
                SliderMark::new(OVERLAY_SIZE_DEFAULT.into(), "Default"),
                SliderMark::new(OVERLAY_SIZE_RANGE.1.into(), "Explorer"),
            ]),
            Field::note(
                field::OVERLAY_NOTE,
                "How large the shell's corner badge (the shortcut arrow, a sync \
                 badge) is drawn relative to the icon. Explorer uses 100%; \
                 smaller is often easier on the eye.",
            ),
        ],
    };

    vec![
        appearance,
        labels_page,
        layout_page,
        colors_page,
        advanced_page,
    ]
}

fn general_page(config: &Config) -> SettingsPage {
    SettingsPage {
        title: "General".into(),
        fields: vec![
            Field::heading(field::BEHAVIOR_HEADING, "Behavior"),
            Field::toggle(
                field::START_HIDDEN,
                "Start with spaces hidden",
                config.window.start_hidden,
            ),
            Field::slider(
                field::SNAP,
                "Snap to grid",
                (SNAP_RANGE.0, SNAP_RANGE.1),
                config.desktop.layout.snap.clamp(SNAP_RANGE.0, SNAP_RANGE.1),
            )
            .suffix(" px"),
            Field::toggle(
                field::ALWAYS_ON_DESKTOP,
                "Keep spaces on the desktop",
                config.window.always_on_desktop,
            ),
            Field::note(
                field::ALWAYS_ON_DESKTOP_NOTE,
                "Desktop-hosted spaces sit behind other windows and survive Show \
                 Desktop, but have no blur. Takes effect next launch.",
            ),
            Field::toggle(
                field::START_WITH_WINDOWS,
                "Start DeskSpace when you sign in",
                config.window.start_with_windows,
            ),
            Field::toggle(
                field::TAKE_OVER_DESKTOP,
                "Take over the desktop (hide Explorer's icons)",
                config.window.take_over_desktop,
            ),
            Field::choice(
                field::UI_MODE,
                "Settings and menus",
                &labels(&MODES),
                index_of(&MODES, config.theme.ui_mode),
            ),
        ],
    }
}

fn new_spaces_page(config: &Config) -> SettingsPage {
    let window = &config.window;
    let size_slider = |id, label, value: i32| {
        Field::slider(
            id,
            label,
            (NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1),
            value.clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1),
        )
        .editable()
        .suffix(" px")
    };
    SettingsPage {
        title: "New spaces".into(),
        fields: vec![
            Field::heading(field::NEW_SPACES_HEADING, "Defaults for new spaces"),
            Field::text(
                field::NEW_SPACE_TITLE,
                "Default title",
                &window.new_space_title,
            ),
            size_slider(
                field::NEW_SPACE_WIDTH,
                "Default width",
                window.new_space_width,
            ),
            size_slider(
                field::NEW_SPACE_HEIGHT,
                "Default height",
                window.new_space_height,
            ),
            Field::toggle(
                field::NEW_SPACE_LOCKED,
                "Start new spaces locked",
                window.new_space_locked,
            ),
            Field::note(
                field::NEW_SPACES_NOTE,
                "New spaces otherwise follow the Global appearance and layout. \
                 A locked space keeps its size and scrolls instead of growing.",
            ),
        ],
    }
}

fn hotkeys_page(session: &Session) -> SettingsPage {
    let mut fields = vec![Field::heading(field::HOTKEYS_HEADING, "Global hotkeys")];
    for (slot, binding) in HOTKEY_BINDINGS.iter().enumerate() {
        fields.push(Field::text(
            binding.field,
            binding.label,
            &session.hotkey_texts[slot],
        ));
        fields.push(Field::note(
            binding.note,
            session.hotkey_errors[slot]
                .as_deref()
                .unwrap_or(HOTKEY_HINT),
        ));
    }
    SettingsPage {
        title: "Hotkeys".into(),
        fields,
    }
}

fn about_page(config_path: &Path) -> SettingsPage {
    SettingsPage {
        title: "About".into(),
        fields: vec![
            Field::heading(
                field::ABOUT_HEADING,
                format!("DeskSpace {}", env!("CARGO_PKG_VERSION")),
            ),
            Field::note(
                field::ABOUT_SUMMARY,
                "A free and open-source desktop space organizer for Windows.",
            ),
            Field::note(
                field::ABOUT_LICENSE,
                "Copyright 2026 Timothy W. Brown. Licensed under the Apache License 2.0.",
            ),
            Field::read_only_text(
                field::CONFIG_PATH,
                "Configuration file",
                config_path.display().to_string(),
            ),
        ],
    }
}

/// What an edit changed, so the caller knows what to refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    /// The look changed; restyle and repaint.
    Appearance,
    /// The grid changed; re-flow and repaint.
    Layout,
    /// Only pending (unapplied) settings changed.
    Pending,
}

/// Applies a look-page edit to `look`. `None` when `id` is not a look field.
fn apply_look_field(
    look: &mut SpaceLook,
    id: u32,
    value: &FieldValue,
    system_dark: bool,
) -> Option<Effect> {
    let theme = &mut look.theme;
    Some(match (id, value) {
        (field::MODE, FieldValue::Choice(index)) => {
            theme.mode = MODES.get(*index).map_or(theme.mode, |entry| entry.0);
            Effect::Appearance
        }
        (field::BACKDROP, FieldValue::Choice(index)) => {
            theme.backdrop = BACKDROPS
                .get(*index)
                .map_or(theme.backdrop, |entry| entry.0);
            Effect::Appearance
        }
        (field::CORNERS, FieldValue::Slider(radius)) => {
            theme.corner_radius = (*radius).clamp(0, corner::MAX.into()) as u8;
            Effect::Appearance
        }
        (field::TINT_OPACITY, FieldValue::Slider(value)) => {
            theme.tint_opacity = (*value).clamp(0, 100) as u8;
            Effect::Appearance
        }
        (field::TITLE_SHADING, FieldValue::Slider(value)) => {
            theme.title_shading = (*value).clamp(0, 100) as u8;
            Effect::Appearance
        }
        (field::BORDER, FieldValue::Toggle(on)) => {
            theme.show_border = *on;
            Effect::Appearance
        }
        (field::PRESET, FieldValue::Choice(index)) => {
            theme.preset = if *index == 0 {
                Preset::MatchExplorer
            } else {
                Preset::Custom
            };
            // Mirroring also imports Explorer's icon size and item spacing, so
            // the grid has to re-flow, not just restyle.
            Effect::Layout
        }
        (field::LABEL_LINES, FieldValue::Choice(index)) => {
            theme.label_lines = LABEL_LINES_OPTIONS
                .get(*index)
                .map_or(theme.label_lines, |entry| entry.0);
            Effect::Appearance
        }
        (field::LABEL_SIZE, FieldValue::Slider(value)) => {
            theme.label_size =
                (*value).clamp(LABEL_SIZE_RANGE.0.into(), LABEL_SIZE_RANGE.1.into()) as u8;
            Effect::Appearance
        }
        (field::LABEL_SHADOW, FieldValue::Choice(index)) => {
            theme.label_shadow = LABEL_SHADOW_OPTIONS
                .get(*index)
                .map_or(theme.label_shadow, |entry| entry.0);
            Effect::Appearance
        }
        (field::ICON_SIZE, FieldValue::Slider(value)) => {
            theme.icon_size =
                (*value).clamp(ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into()) as u8;
            Effect::Layout
        }
        (field::OVERLAY_SIZE, FieldValue::Slider(value)) => {
            theme.overlay_size =
                (*value).clamp(OVERLAY_SIZE_RANGE.0.into(), OVERLAY_SIZE_RANGE.1.into()) as u8;
            Effect::Appearance
        }
        (field::GRID_GAP, FieldValue::Slider(value)) => {
            look.gap = (*value).clamp(GAP_RANGE.0, GAP_RANGE.1);
            Effect::Layout
        }
        (field::GRID_PADDING, FieldValue::Slider(value)) => {
            look.padding = (*value).clamp(GAP_RANGE.0, GAP_RANGE.1);
            Effect::Layout
        }
        (field::CUSTOM_COLORS, FieldValue::Toggle(on)) => {
            if *on && !theme.custom_colors {
                // Start from what is on screen rather than a stale color.
                let palette = theme.palette(system_dark);
                theme.tint = palette.tint.with_alpha(255);
                theme.text = palette.title_text;
            }
            theme.custom_colors = *on;
            Effect::Appearance
        }
        (field::TINT, FieldValue::Color(color)) => {
            theme.tint = color.with_alpha(255);
            Effect::Appearance
        }
        (field::TEXT, FieldValue::Color(color)) => {
            theme.text = color.with_alpha(255);
            Effect::Appearance
        }
        (field::USE_ACCENT, FieldValue::Toggle(on)) => {
            theme.use_accent = *on;
            Effect::Appearance
        }
        _ => return None,
    })
}

/// Applies an edit to the global settings.
pub fn apply_field(
    config: &mut Config,
    session: &mut Session,
    id: u32,
    value: FieldValue,
    system_dark: bool,
) -> Effect {
    let mut look = SpaceLook::from_global(&config.theme, &config.desktop.layout);
    let effect = match apply_look_field(&mut look, id, &value, system_dark) {
        Some(effect) => {
            config.theme = look.theme;
            config.desktop.layout.gap = look.gap;
            config.desktop.layout.padding = look.padding;
            effect
        }
        None => match (id, value) {
            (field::UI_MODE, FieldValue::Choice(index)) => {
                config.theme.ui_mode = MODES
                    .get(index)
                    .map_or(config.theme.ui_mode, |entry| entry.0);
                Effect::Appearance
            }
            (field::START_WITH_WINDOWS, FieldValue::Toggle(on)) => {
                config.window.start_with_windows = on;
                Effect::Pending
            }
            (field::TAKE_OVER_DESKTOP, FieldValue::Toggle(on)) => {
                config.window.take_over_desktop = on;
                Effect::Pending
            }
            (id, FieldValue::Text(text)) if is_hotkey_field(id) => {
                let slot = HOTKEY_BINDINGS
                    .iter()
                    .position(|binding| binding.field == id)
                    .expect("a hotkey field is in the table");
                session.hotkey_texts[slot] = text;
                session.hotkey_errors[slot] = None;
                Effect::Pending
            }
            (field::START_HIDDEN, FieldValue::Toggle(on)) => {
                config.window.start_hidden = on;
                Effect::Pending
            }
            (field::SNAP, FieldValue::Slider(value)) => {
                config.desktop.layout.snap = value.clamp(SNAP_RANGE.0, SNAP_RANGE.1);
                Effect::Pending
            }
            (field::ALWAYS_ON_DESKTOP, FieldValue::Toggle(on)) => {
                config.window.always_on_desktop = on;
                Effect::Pending
            }
            (field::NEW_SPACE_TITLE, FieldValue::Text(text)) => {
                config.window.new_space_title = text;
                Effect::Pending
            }
            (field::NEW_SPACE_WIDTH, FieldValue::Slider(value)) => {
                config.window.new_space_width =
                    value.clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1);
                Effect::Pending
            }
            (field::NEW_SPACE_HEIGHT, FieldValue::Slider(value)) => {
                config.window.new_space_height =
                    value.clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1);
                Effect::Pending
            }
            (field::NEW_SPACE_LOCKED, FieldValue::Toggle(on)) => {
                config.window.new_space_locked = on;
                Effect::Pending
            }
            _ => Effect::None,
        },
    };
    if effect != Effect::None {
        session.dirty = true;
    }
    effect
}

/// The command for an edit on a customized space's page, with what it changes.
/// `None` when Global is selected, the space follows Global, or `id` is not
/// a look field.
pub fn space_edit(
    config: &Config,
    session: &mut Session,
    id: u32,
    value: FieldValue,
    system_dark: bool,
) -> Option<(SpaceId, Command, Effect)> {
    let space = scope_space(config, session).filter(|space| space.custom)?;
    let mut look = space
        .look
        .clone()
        .unwrap_or_else(|| SpaceLook::from_global(&config.theme, &config.desktop.layout));
    let effect = apply_look_field(&mut look, id, &value, system_dark)?;
    session.dirty = true;
    let command = Command::SetSpaceLook {
        id: space.id,
        custom: true,
        look: Some(look),
    };
    Some((space.id, command, effect))
}

/// The command for the header switch: customize the selected space, or let
/// it follow Global again (keeping its look for later).
pub fn customize_space(config: &Config, session: &mut Session, on: bool) -> Option<Command> {
    let space = scope_space(config, session)?;
    session.dirty = true;
    Some(Command::CustomizeSpace {
        id: space.id,
        on,
        seed: SpaceLook::from_global(&config.theme, &config.desktop.layout),
    })
}

/// The command for a customized space's "Match global appearance": a look
/// copied from the global settings.
pub fn match_global(config: &Config, session: &mut Session) -> Option<Command> {
    let space = scope_space(config, session).filter(|space| space.custom)?;
    session.dirty = true;
    Some(Command::SetSpaceLook {
        id: space.id,
        custom: true,
        look: Some(SpaceLook::from_global(
            &config.theme,
            &config.desktop.layout,
        )),
    })
}

pub fn reset_appearance(config: &mut Config, session: &mut Session) {
    config.theme = Theme::default();
    session.dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::Color;
    use deskspace_platform::FieldKind;

    fn setup() -> (Config, Session) {
        let config = Config::new();
        let session = Session::new(&config);
        (config, session)
    }

    #[test]
    fn form_reflects_the_theme_and_disables_dependent_fields() {
        let (mut config, session) = setup();
        config.theme.backdrop = Backdrop::Solid;
        let form = build_form(&config, &session, true, Path::new("C:/cfg.toml"));
        let opacity = form.field(field::TINT_OPACITY).unwrap();
        assert!(!opacity.enabled);
        assert!(!form.field(field::TINT).unwrap().enabled);
        assert!(matches!(
            form.field(field::BACKDROP).unwrap().kind,
            FieldKind::Choice { selected: 3, .. }
        ));
        assert!(!form.apply_enabled);
    }

    #[test]
    fn layout_fields_are_disabled_while_matching_explorer() {
        let (mut config, session) = setup();
        // `match_explorer` defaults on, so Explorer owns the icon size and
        // spacing and the Layout controls are inert.
        let form = build_form(&config, &session, true, Path::new("C:/cfg.toml"));
        for id in [field::ICON_SIZE, field::GRID_GAP, field::GRID_PADDING] {
            assert!(!form.field(id).unwrap().enabled);
        }
        config.theme.preset = Preset::Custom;
        let form = build_form(&config, &session, true, Path::new("C:/cfg.toml"));
        for id in [field::ICON_SIZE, field::GRID_GAP, field::GRID_PADDING] {
            assert!(form.field(id).unwrap().enabled);
        }
    }

    #[test]
    fn ui_mode_choice_applies_live() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::UI_MODE,
            FieldValue::Choice(1),
            true,
        );
        assert_eq!(effect, Effect::Appearance);
        assert_eq!(config.theme.ui_mode, ThemeMode::Dark);
    }

    #[test]
    fn start_with_windows_is_pending() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::START_WITH_WINDOWS,
            FieldValue::Toggle(true),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(config.window.start_with_windows);
    }

    #[test]
    fn take_over_desktop_is_pending() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::TAKE_OVER_DESKTOP,
            FieldValue::Toggle(true),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(config.window.take_over_desktop);
    }

    #[test]
    fn preset_choice_applies_live() {
        let (mut config, mut session) = setup();
        assert_eq!(config.theme.preset, Preset::MatchExplorer);
        let effect = apply_field(
            &mut config,
            &mut session,
            field::PRESET,
            FieldValue::Choice(1),
            true,
        );
        assert_eq!(effect, Effect::Layout);
        assert_eq!(config.theme.preset, Preset::Custom);
        assert!(!config.theme.match_explorer());
        apply_field(
            &mut config,
            &mut session,
            field::PRESET,
            FieldValue::Choice(0),
            true,
        );
        assert_eq!(config.theme.preset, Preset::MatchExplorer);
    }

    #[test]
    fn cancel_restores_the_saved_settings() {
        let (mut config, mut session) = setup();
        apply_field(
            &mut config,
            &mut session,
            field::TINT_OPACITY,
            FieldValue::Slider(90),
            true,
        );
        apply_field(
            &mut config,
            &mut session,
            field::START_HIDDEN,
            FieldValue::Toggle(true),
            true,
        );
        session.revert(&mut config);
        assert_eq!(config, Config::new());
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
    fn enabling_custom_colors_starts_from_the_current_palette() {
        let (mut config, mut session) = setup();
        config.theme.mode = ThemeMode::Light;
        apply_field(
            &mut config,
            &mut session,
            field::CUSTOM_COLORS,
            FieldValue::Toggle(true),
            true,
        );
        assert_eq!(config.theme.tint, Color::rgb(243, 243, 243));
        assert_eq!(config.theme.text, Color::rgb(26, 26, 26));
    }

    #[test]
    fn hotkey_text_is_pending_until_applied() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::HOTKEY,
            FieldValue::Text("Ctrl+Shift+F".into()),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert_eq!(config.window.toggle_hotkey, "Ctrl+Alt+D");
        assert_eq!(session.hotkey_texts[0], "Ctrl+Shift+F");
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(form.apply_enabled);
    }

    #[test]
    fn every_hotkey_binding_has_an_editable_field() {
        let (config, session) = setup();
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(session.hotkey_texts.len(), HOTKEY_BINDINGS.len());
        for binding in HOTKEY_BINDINGS {
            let field = form.field(binding.field).expect("its text field");
            assert!(field.enabled);
            assert!(form.field(binding.note).is_some(), "its hint note");
        }
    }

    #[test]
    fn a_second_hotkey_is_staged_like_the_first() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::HOTKEY_NEW_SPACE,
            FieldValue::Text("Ctrl+Alt+N".into()),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(config.window.new_space_hotkey.is_empty());
        assert_eq!(session.hotkey_texts[1], "Ctrl+Alt+N");
    }

    #[test]
    fn new_space_defaults_are_pending_and_update_the_config() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::NEW_SPACE_TITLE,
                FieldValue::Text("Inbox".into()),
                true,
            ),
            Effect::Pending
        );
        assert_eq!(config.window.new_space_title, "Inbox");
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::NEW_SPACE_WIDTH,
                FieldValue::Slider(320),
                true,
            ),
            Effect::Pending
        );
        assert_eq!(config.window.new_space_width, 320);
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::NEW_SPACE_HEIGHT,
                FieldValue::Slider(9_000),
                true,
            ),
            Effect::Pending
        );
        assert_eq!(config.window.new_space_height, NEW_SPACE_SIZE_RANGE.1);
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::NEW_SPACE_LOCKED,
                FieldValue::Toggle(true),
                true,
            ),
            Effect::Pending
        );
        assert!(config.window.new_space_locked);
        assert!(session.dirty);
    }

    #[test]
    fn new_spaces_page_shows_the_configured_defaults() {
        let (mut config, session) = setup();
        config.window.new_space_title = "Inbox".into();
        config.window.new_space_width = 300;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(form.field(field::NEW_SPACE_TITLE).unwrap().enabled);
        assert_eq!(slider_value(&form, field::NEW_SPACE_WIDTH), 300);
        assert_eq!(
            form.field(field::NEW_SPACE_TITLE).unwrap().label,
            "Default title"
        );
    }

    #[test]
    fn slider_values_are_clamped() {
        let (mut config, mut session) = setup();
        apply_field(
            &mut config,
            &mut session,
            field::TITLE_SHADING,
            FieldValue::Slider(250),
            true,
        );
        assert_eq!(config.theme.title_shading, 100);
    }

    #[test]
    fn grid_sliders_report_a_layout_effect_and_clamp() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::GRID_GAP,
            FieldValue::Slider(30),
            true,
        );
        assert_eq!(effect, Effect::Layout);
        assert_eq!(config.desktop.layout.gap, 24);
        apply_field(
            &mut config,
            &mut session,
            field::GRID_PADDING,
            FieldValue::Slider(4),
            true,
        );
        assert_eq!(config.desktop.layout.padding, 4);
    }

    #[test]
    fn icon_size_reports_a_layout_effect_and_clamps() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::ICON_SIZE,
                FieldValue::Slider(150),
                true,
            ),
            Effect::Layout
        );
        assert_eq!(config.theme.icon_size, 150);
        apply_field(
            &mut config,
            &mut session,
            field::ICON_SIZE,
            FieldValue::Slider(10),
            true,
        );
        assert_eq!(config.theme.icon_size, 50);
        apply_field(
            &mut config,
            &mut session,
            field::ICON_SIZE,
            FieldValue::Slider(500),
            true,
        );
        assert_eq!(config.theme.icon_size, 200);
    }

    #[test]
    fn overlay_slider_marks_the_default_and_explorer_sizes() {
        let (config, session) = setup();
        let form = build_form(&config, &session, true, Path::new("x"));
        match &form.field(field::OVERLAY_SIZE).unwrap().kind {
            FieldKind::Slider { value, marks, .. } => {
                assert_eq!(*value, OVERLAY_SIZE_DEFAULT.into());
                assert!(marks.iter().any(
                    |mark| mark.value == OVERLAY_SIZE_DEFAULT.into() && mark.label == "Default"
                ));
                assert!(
                    marks
                        .iter()
                        .any(|mark| mark.value == OVERLAY_SIZE_RANGE.1.into())
                );
            }
            other => panic!("expected a slider, got {other:?}"),
        }
    }

    #[test]
    fn overlay_size_reports_an_appearance_effect_and_clamps() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::OVERLAY_SIZE,
                FieldValue::Slider(60),
                true,
            ),
            Effect::Appearance
        );
        assert_eq!(config.theme.overlay_size, 60);
        apply_field(
            &mut config,
            &mut session,
            field::OVERLAY_SIZE,
            FieldValue::Slider(0),
            true,
        );
        assert_eq!(config.theme.overlay_size, OVERLAY_SIZE_RANGE.0);
        apply_field(
            &mut config,
            &mut session,
            field::OVERLAY_SIZE,
            FieldValue::Slider(250),
            true,
        );
        assert_eq!(config.theme.overlay_size, OVERLAY_SIZE_RANGE.1);
    }

    #[test]
    fn label_style_edits_apply_live_and_clamp() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::LABEL_LINES,
                FieldValue::Choice(0),
                true,
            ),
            Effect::Appearance
        );
        assert_eq!(config.theme.label_lines, 1);
        apply_field(
            &mut config,
            &mut session,
            field::LABEL_SIZE,
            FieldValue::Slider(200),
            true,
        );
        assert_eq!(config.theme.label_size, 160);
        apply_field(
            &mut config,
            &mut session,
            field::LABEL_SHADOW,
            FieldValue::Choice(2),
            true,
        );
        assert_eq!(config.theme.label_shadow, LabelShadow::Off);
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
    fn corner_radius_is_a_marked_slider() {
        let (mut config, mut session) = setup();
        let form = build_form(&config, &session, true, Path::new("x"));
        match &form.field(field::CORNERS).unwrap().kind {
            FieldKind::Slider {
                value, max, marks, ..
            } => {
                assert_eq!(*value, 8);
                assert_eq!(*max, 24);
                assert!(marks.iter().any(|mark| mark.value == 8));
            }
            other => panic!("expected a slider, got {other:?}"),
        }
        apply_field(
            &mut config,
            &mut session,
            field::CORNERS,
            FieldValue::Slider(40),
            true,
        );
        assert_eq!(config.theme.corner_radius, corner::MAX);
    }

    #[test]
    fn hotkey_errors_replace_the_hint() {
        let (config, mut session) = setup();
        session.hotkey_errors[0] = Some("bad".into());
        session.hotkey_errors[2] = Some("worse".into());
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(form.field(field::HOTKEY_NOTE).unwrap().label, "bad");
        assert_eq!(
            form.field(field::HOTKEY_SETTINGS_NOTE).unwrap().label,
            "worse"
        );
    }

    use deskspace_core::Rect;

    fn config_with_space(title: &str) -> (Config, SpaceId) {
        let mut config = Config::new();
        let id = config.desktop.add_space(title, Rect::new(0, 0, 200, 200));
        (config, id)
    }

    fn nav(form: &SettingsForm) -> &SettingsNav {
        form.nav.as_ref().expect("the rail is set")
    }

    fn slider_value(form: &SettingsForm, id: u32) -> i32 {
        match &form.field(id).unwrap().kind {
            FieldKind::Slider { value, .. } => *value,
            other => panic!("expected a slider, got {other:?}"),
        }
    }

    #[test]
    fn global_lists_every_space_dimmed_until_customized() {
        let (mut config, _) = config_with_space("Work");
        let games = config.desktop.add_space("Games", Rect::new(0, 0, 200, 200));
        config.desktop.space_mut(games).unwrap().custom = true;
        let session = Session::new(&config);
        let form = build_form(&config, &session, true, Path::new("x"));
        let nav = nav(&form);
        let scopes: Vec<(&str, bool, bool)> = nav
            .scopes
            .iter()
            .map(|scope| (scope.label.as_str(), scope.indent, scope.dimmed))
            .collect();
        assert_eq!(
            scopes,
            [
                ("Global", false, false),
                ("Work", true, true),
                ("Games", true, false)
            ]
        );
        assert_eq!(nav.selected_scope, 0);
        assert_eq!(nav.entries().count(), page::ABOUT + 1);
        let header = nav.header.as_ref().unwrap();
        assert_eq!(header.title, "Global settings");
        assert!(header.switch.is_none());
        assert!(header.button.is_none(), "Global cannot be deleted");
    }

    #[test]
    fn a_space_shows_only_look_pages_and_a_customize_switch() {
        let (config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.page = page::ABOUT;
        session.select_scope(Some(id));
        assert_eq!(session.page, page::APPEARANCE, "About is Global only");
        let form = build_form(&config, &session, true, Path::new("x"));
        let nav = nav(&form);
        assert_eq!(nav.selected_scope, 1);
        let labels: Vec<&str> = nav.entries().map(|entry| entry.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Appearance", "Labels", "Layout", "Colors", "Advanced"]
        );
        let switch = nav.header.as_ref().unwrap().switch.as_ref().unwrap();
        assert_eq!(switch.label, "Customize this space");
        assert!(!switch.on);
        let button = nav.header.as_ref().unwrap().button.as_ref().unwrap();
        assert_eq!(button.label, "Delete space");
        // The page shape never changes: General and About are still built.
        assert_eq!(form.pages.len(), page::ABOUT + 1);
    }

    #[test]
    fn a_space_that_follows_global_shows_global_values_greyed() {
        let (mut config, id) = config_with_space("Work");
        config.theme.tint_opacity = 70;
        let mut kept = SpaceLook::from_global(&config.theme, &config.desktop.layout);
        kept.theme.tint_opacity = 10;
        config.desktop.space_mut(id).unwrap().look = Some(kept);
        let mut session = Session::new(&config);
        session.select_scope(Some(id));

        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(slider_value(&form, field::TINT_OPACITY), 70);
        assert!(!form.field(field::TINT_OPACITY).unwrap().enabled);
        assert!(!form.field(field::MODE).unwrap().enabled);

        config.desktop.space_mut(id).unwrap().custom = true;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(slider_value(&form, field::TINT_OPACITY), 10, "its own look");
        assert!(form.field(field::TINT_OPACITY).unwrap().enabled);
    }

    #[test]
    fn a_space_offers_fit_to_contents_on_the_layout_page() {
        let (mut config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.select_scope(Some(id));

        // An auto-sized space already fits, so the button is present but grey.
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(
            form.pages[page::LAYOUT]
                .fields
                .iter()
                .any(|field| field.id == field::FIT_CONTENTS)
        );
        assert!(!form.field(field::FIT_CONTENTS).unwrap().enabled);

        config.desktop.space_mut(id).unwrap().user_sized = true;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(form.field(field::FIT_CONTENTS).unwrap().enabled);

        // A locked space cannot be fit, matching the space menu.
        config.desktop.space_mut(id).unwrap().locked = true;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(!form.field(field::FIT_CONTENTS).unwrap().enabled);
    }

    #[test]
    fn global_fit_to_contents_is_present_but_disabled() {
        let (config, _) = config_with_space("Work");
        let session = Session::new(&config);
        let form = build_form(&config, &session, true, Path::new("x"));
        // Present for Global too, so the page shape does not change with the
        // scope, but greyed because no space is being edited.
        let field = form.field(field::FIT_CONTENTS).expect("present");
        assert!(!field.enabled);
    }

    #[test]
    fn the_form_shape_is_stable_across_scopes() {
        // The settings window diffs a new form onto the live controls and
        // panics if the page shape changes, so Global and a space must build
        // the same pages and fields (selecting a space must not add a field).
        let (config, id) = config_with_space("Work");
        let global = build_form(&config, &Session::new(&config), true, Path::new("x"));
        let mut session = Session::new(&config);
        session.select_scope(Some(id));
        let space = build_form(&config, &session, true, Path::new("x"));
        assert!(global.same_shape(&space));
    }

    #[test]
    fn space_edits_become_commands_and_leave_the_global_theme_alone() {
        let (mut config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.select_scope(Some(id));
        let edit = |config: &Config, session: &mut Session| {
            space_edit(
                config,
                session,
                field::ICON_SIZE,
                FieldValue::Slider(150),
                true,
            )
        };
        assert!(edit(&config, &mut session).is_none(), "follows Global");

        config.desktop.space_mut(id).unwrap().custom = true;
        let (space, command, effect) = edit(&config, &mut session).unwrap();
        assert_eq!(space, id);
        assert_eq!(effect, Effect::Layout);
        assert!(session.dirty);
        match command {
            Command::SetSpaceLook {
                custom: true,
                look: Some(look),
                ..
            } => {
                assert_eq!(look.theme.icon_size, 150);
                assert_eq!(look.theme.tint_opacity, config.theme.tint_opacity);
            }
            other => panic!("expected SetSpaceLook, got {other:?}"),
        }
        assert_eq!(config.theme.icon_size, 100);
        assert!(
            space_edit(
                &config,
                &mut session,
                field::HOTKEY,
                FieldValue::Text("x".into()),
                true
            )
            .is_none(),
            "General is Global only"
        );
    }

    #[test]
    fn customizing_seeds_from_global_and_match_global_copies_it() {
        let (mut config, id) = config_with_space("Work");
        config.theme.corner_radius = 4;
        let mut session = Session::new(&config);
        assert!(
            customize_space(&config, &mut session, true).is_none(),
            "Global"
        );
        session.select_scope(Some(id));
        match customize_space(&config, &mut session, true) {
            Some(Command::CustomizeSpace {
                id: target,
                on,
                seed,
            }) => {
                assert_eq!(target, id);
                assert!(on);
                assert_eq!(seed.theme.corner_radius, 4);
            }
            other => panic!("expected CustomizeSpace, got {other:?}"),
        }
        assert!(
            match_global(&config, &mut session).is_none(),
            "not customized"
        );
        config.desktop.space_mut(id).unwrap().custom = true;
        assert!(matches!(
            match_global(&config, &mut session),
            Some(Command::SetSpaceLook { custom: true, look: Some(look), .. })
                if look.theme.corner_radius == 4
        ));
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

    #[test]
    fn scope_indices_map_to_spaces() {
        let (config, id) = config_with_space("Work");
        assert_eq!(scope_at(&config, 0), None);
        assert_eq!(scope_at(&config, 1), Some(id));
        assert_eq!(scope_at(&config, 2), None);
        let mut session = Session::new(&config);
        assert_eq!(scope_index(&config, &session), 0);
        session.select_scope(Some(id));
        assert_eq!(scope_index(&config, &session), 1);
        session.select_scope(Some(SpaceId(999)));
        assert_eq!(scope_index(&config, &session), 0, "a deleted space");
    }
}
