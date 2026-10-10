//! The settings window's content and how edits map onto the configuration.
//! Pure functions over `Config`, so the form logic is testable without Win32.

use std::path::Path;

use deskspace_config::{Config, WindowSettings};
use deskspace_core::layout::{GAP_RANGE, SNAP_RANGE};
use deskspace_core::theme::{
    ICON_SIZE_RANGE, LABEL_SIZE_RANGE, OVERLAY_SIZE_DEFAULT, OVERLAY_SIZE_RANGE, corner,
};
use deskspace_core::{
    Backdrop, Color, Command, GridSpec, LabelShadow, Palette, Space, SpaceId, Theme, ThemeMode,
};
use deskspace_platform::{
    Field, FieldValue, SettingsForm, SettingsNav, SettingsNavEntry, SettingsNavGroup, SettingsPage,
    SliderMark,
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
    pub const MATCH_EXPLORER: u32 = 42;
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

    // The shared Space page (the rail's Spaces group). Fixed shape; the app
    // fills it with the selected space's values.
    pub const SPACE_NOTE: u32 = 50;
    pub const SPACE_OPACITY: u32 = 51;
    pub const SPACE_ICON_SIZE: u32 = 52;
    pub const SPACE_TINT_TOGGLE: u32 = 53;
    pub const SPACE_TINT: u32 = 54;
    pub const SPACE_TEXT_TOGGLE: u32 = 55;
    pub const SPACE_TEXT: u32 = 56;
    pub const SPACE_RESET: u32 = 57;
}

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

/// The per-space icon-size choices: label and percent (`None` follows the
/// global setting). Shared by the Space settings page and the space menu.
pub const SPACE_ICON_SIZES: [(&str, Option<u8>); 5] = [
    ("Default", None),
    ("Small", Some(75)),
    ("Medium", Some(100)),
    ("Large", Some(150)),
    ("Extra large", Some(200)),
];

/// The number of global pages in the Settings rail group; also the page index
/// of the shared Space page.
pub const GLOBAL_PAGE_COUNT: usize = 7;

/// Whether `id` is a field on the shared Space page.
pub fn is_space_field(id: u32) -> bool {
    (field::SPACE_NOTE..=field::SPACE_RESET).contains(&id)
}

/// The spaces currently listed in the rail's Spaces group, in order: the
/// session's listed spaces that still exist. Deleted spaces drop out.
pub fn listed_spaces(config: &Config, session: &Session) -> Vec<SpaceId> {
    session
        .listed_spaces
        .iter()
        .copied()
        .filter(|id| config.desktop.space(*id).is_some())
        .collect()
}

/// The space shown on the shared Space page for this session, if its selected
/// rail entry is one. `None` for a global page or an empty Spaces group.
pub fn selected_space(config: &Config, session: &Session) -> Option<SpaceId> {
    session
        .nav_selected
        .checked_sub(GLOBAL_PAGE_COUNT)
        .and_then(|index| listed_spaces(config, session).get(index).copied())
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
     for example Ctrl+Alt+D.";

/// A space's appearance values, snapshotted so Cancel can restore them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceAppearance {
    pub id: SpaceId,
    pub opacity: u8,
    pub tint: Option<Color>,
    pub text: Option<Color>,
    pub icon_size: Option<u8>,
}

impl SpaceAppearance {
    pub fn of(space: &Space) -> Self {
        Self {
            id: space.id,
            opacity: space.opacity,
            tint: space.tint,
            text: space.text,
            icon_size: space.icon_size,
        }
    }
}

/// Edits in progress while the settings window is open. Appearance edits are
/// applied to the live config immediately for preview; `saved_*` is what
/// Cancel restores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub saved_theme: Theme,
    pub saved_window: WindowSettings,
    pub saved_layout: GridSpec,
    pub hotkey_text: String,
    pub hotkey_error: Option<String>,
    pub dirty: bool,
    /// The spaces shown in the rail's Spaces group: those with overrides when
    /// the window opened, plus any that gain one while it is open. Sticky for
    /// the session, so resetting the last override does not drop the entry.
    pub listed_spaces: Vec<SpaceId>,
    /// The selected rail entry, an index into the flattened `SettingsNav`.
    pub nav_selected: usize,
    /// The per-space appearance snapshot Cancel restores.
    pub saved_spaces: Vec<SpaceAppearance>,
}

impl Session {
    pub fn new(config: &Config) -> Self {
        Self {
            saved_theme: config.theme.clone(),
            saved_window: config.window.clone(),
            saved_layout: config.desktop.layout,
            hotkey_text: config.window.toggle_hotkey.clone(),
            hotkey_error: None,
            dirty: false,
            listed_spaces: config
                .desktop
                .spaces
                .iter()
                .filter(|space| space.has_appearance_overrides())
                .map(|space| space.id)
                .collect(),
            nav_selected: 0,
            saved_spaces: config
                .desktop
                .spaces
                .iter()
                .map(SpaceAppearance::of)
                .collect(),
        }
    }

    /// Marks the current config as the new baseline after a successful save.
    pub fn commit(&mut self, config: &Config) {
        self.saved_theme = config.theme.clone();
        self.saved_window = config.window.clone();
        self.saved_layout = config.desktop.layout;
        self.saved_spaces = config
            .desktop
            .spaces
            .iter()
            .map(SpaceAppearance::of)
            .collect();
        self.hotkey_error = None;
        self.dirty = false;
    }

    pub fn revert(&self, config: &mut Config) {
        config.theme = self.saved_theme.clone();
        config.window = self.saved_window.clone();
        config.desktop.layout = self.saved_layout;
    }
}

/// Whether reverting to `saved` needs the spaces re-flowed rather than just
/// restyled: the grid, the icon size that scales it, or Explorer matching,
/// which imports the desktop's own icon size and item spacing.
pub fn needs_relayout(saved: &Session, config: &Config) -> bool {
    saved.saved_layout != config.desktop.layout
        || saved.saved_theme.icon_size != config.theme.icon_size
        || saved.saved_theme.match_explorer != config.theme.match_explorer
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

/// The Opacity slider's percent for a stored space opacity (`0..=255`), where
/// 255 is the theme's 100 %.
fn opacity_percent(opacity: u8) -> i32 {
    ((opacity as u32 * 100 + 127) / 255) as i32
}

/// The stored opacity byte for an Opacity slider percent, where 100 % is 255.
/// The inverse of [`opacity_percent`].
fn opacity_byte(percent: i32) -> u8 {
    ((percent.clamp(0, 100) as u32 * 255 + 50) / 100) as u8
}

/// Turns an edit on the shared Space page into the `Command` that applies it,
/// or `None` when `field` is not one of its fields. Pure, so the rail's edits
/// are testable without Win32.
pub fn space_field_command(
    space: &Space,
    field: u32,
    value: FieldValue,
    palette: &Palette,
) -> Option<Command> {
    let id = space.id;
    match (field, value) {
        (field::SPACE_OPACITY, FieldValue::Slider(percent)) => Some(Command::SetSpaceOpacity {
            id,
            opacity: opacity_byte(percent),
        }),
        (field::SPACE_ICON_SIZE, FieldValue::Choice(index)) => Some(Command::SetSpaceIconSize {
            id,
            icon_size: SPACE_ICON_SIZES.get(index).and_then(|(_, value)| *value),
        }),
        (field::SPACE_TINT_TOGGLE, FieldValue::Toggle(on)) => Some(Command::SetSpaceTint {
            id,
            tint: on.then(|| palette.tint.with_alpha(255)),
        }),
        (field::SPACE_TINT, FieldValue::Color(color)) => Some(Command::SetSpaceTint {
            id,
            tint: Some(color),
        }),
        (field::SPACE_TEXT_TOGGLE, FieldValue::Toggle(on)) => Some(Command::SetSpaceText {
            id,
            text: on.then_some(palette.title_text),
        }),
        (field::SPACE_TEXT, FieldValue::Color(color)) => Some(Command::SetSpaceText {
            id,
            text: Some(color),
        }),
        _ => None,
    }
}

/// The index of `icon_size` in [`SPACE_ICON_SIZES`], or 0 (`Default`).
fn icon_size_index(icon_size: Option<u8>) -> usize {
    SPACE_ICON_SIZES
        .iter()
        .position(|(_, value)| *value == icon_size)
        .unwrap_or(0)
}

pub fn build_form(
    config: &Config,
    session: &Session,
    system_dark: bool,
    config_path: &Path,
) -> SettingsForm {
    let theme = &config.theme;
    let palette = theme.palette(system_dark);
    let solid = theme.backdrop == Backdrop::Solid;

    // Appearance is split across several tabs so no page is tall enough to
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
            Field::toggle(
                field::MATCH_EXPLORER,
                "Match Explorer's display (check boxes, file extensions)",
                theme.match_explorer,
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
            ]),
            Field::slider(
                field::GRID_GAP,
                "Icon spacing",
                (GAP_RANGE.0, GAP_RANGE.1),
                config.desktop.layout.gap.clamp(GAP_RANGE.0, GAP_RANGE.1),
            )
            .suffix(" px"),
            Field::slider(
                field::GRID_PADDING,
                "Edge padding",
                (GAP_RANGE.0, GAP_RANGE.1),
                config
                    .desktop
                    .layout
                    .padding
                    .clamp(GAP_RANGE.0, GAP_RANGE.1),
            )
            .suffix(" px"),
        ],
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
            Field::button(field::RESET, "", "Restore default appearance"),
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

    let general = SettingsPage {
        title: "General".into(),
        fields: vec![
            Field::heading(field::BEHAVIOR_HEADING, "Behavior"),
            Field::text(field::HOTKEY, "Show/hide hotkey", &session.hotkey_text),
            Field::note(
                field::HOTKEY_NOTE,
                session.hotkey_error.as_deref().unwrap_or(HOTKEY_HINT),
            ),
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
            Field::choice(
                field::UI_MODE,
                "Settings and menus",
                &labels(&MODES),
                index_of(&MODES, theme.ui_mode),
            ),
        ],
    };

    let about = SettingsPage {
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
    };

    let settings_pages = vec![
        appearance,
        labels_page,
        layout_page,
        colors_page,
        advanced_page,
        general,
        about,
    ];
    let space_page_index = settings_pages.len();
    debug_assert_eq!(space_page_index, GLOBAL_PAGE_COUNT);

    // Spaces deleted since the window opened drop out of the rail; the rest
    // keep their entries for the session (the sticky rule).
    let listed = listed_spaces(config, session);
    let selected = session
        .nav_selected
        .checked_sub(space_page_index)
        .and_then(|index| listed.get(index))
        .and_then(|id| config.desktop.space(*id));

    let settings_group = SettingsNavGroup {
        title: "Settings".into(),
        entries: settings_pages
            .iter()
            .enumerate()
            .map(|(page, entry)| SettingsNavEntry {
                label: entry.title.clone(),
                page,
                marked: false,
            })
            .collect(),
    };
    let spaces_group = SettingsNavGroup {
        title: "Spaces".into(),
        entries: listed
            .iter()
            .filter_map(|id| config.desktop.space(*id))
            .map(|space| SettingsNavEntry {
                label: space.title.clone(),
                page: space_page_index,
                marked: space.has_appearance_overrides(),
            })
            .collect(),
    };

    let mut pages = settings_pages;
    pages.push(space_page(selected, &palette));
    let entry_count = space_page_index + spaces_group.entries.len();
    let selected = session.nav_selected.min(entry_count.saturating_sub(1));

    SettingsForm {
        title: "DeskSpace Settings".into(),
        pages,
        apply_enabled: session.dirty,
        nav: Some(SettingsNav {
            groups: vec![settings_group, spaces_group],
            selected,
        }),
    }
}

/// The shared Space page for `space`, or the empty state when none is
/// selected. Its shape is fixed so `same_shape` holds while the selection
/// changes; the fields are disabled unless a space is shown.
fn space_page(space: Option<&Space>, palette: &Palette) -> SettingsPage {
    const EMPTY_NOTE: &str =
        "No spaces are customized yet. Right-click a space and choose Appearance.";
    let (note, opacity, icon, tint_on, tint, text_on, text) = match space {
        Some(space) => (
            format!("Editing \u{201c}{}\u{201d}.", space.title),
            opacity_percent(space.opacity),
            icon_size_index(space.icon_size),
            space.tint.is_some(),
            space.tint.unwrap_or_else(|| palette.tint.with_alpha(255)),
            space.text.is_some(),
            space.text.unwrap_or(palette.title_text),
        ),
        None => (
            EMPTY_NOTE.to_string(),
            100,
            icon_size_index(None),
            false,
            palette.tint.with_alpha(255),
            false,
            palette.title_text,
        ),
    };
    let enabled = space.is_some();
    let icon_labels: Vec<&str> = SPACE_ICON_SIZES.iter().map(|(label, _)| *label).collect();
    SettingsPage {
        title: "Space".into(),
        fields: vec![
            Field::note(field::SPACE_NOTE, note),
            Field::slider(field::SPACE_OPACITY, "Opacity", (20, 100), opacity).enabled(enabled),
            Field::choice(field::SPACE_ICON_SIZE, "Icon size", &icon_labels, icon).enabled(enabled),
            Field::toggle(field::SPACE_TINT_TOGGLE, "Custom tint", tint_on).enabled(enabled),
            Field::color(field::SPACE_TINT, "Tint color", tint).enabled(enabled && tint_on),
            Field::toggle(field::SPACE_TEXT_TOGGLE, "Custom text color", text_on).enabled(enabled),
            Field::color(field::SPACE_TEXT, "Text color", text).enabled(enabled && text_on),
            Field::button(field::SPACE_RESET, "", "Reset to global").enabled(enabled),
        ],
    }
}

/// What an edit changed, so the caller knows what to refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    /// The theme changed; restyle and repaint every space.
    Appearance,
    /// The grid changed; re-flow every space and repaint.
    Layout,
    /// Only pending (unapplied) settings changed.
    Pending,
}

pub fn apply_field(
    config: &mut Config,
    session: &mut Session,
    id: u32,
    value: FieldValue,
    system_dark: bool,
) -> Effect {
    let effect = match (id, value) {
        (field::MODE, FieldValue::Choice(index)) => {
            config.theme.mode = MODES.get(index).map_or(config.theme.mode, |entry| entry.0);
            Effect::Appearance
        }
        (field::BACKDROP, FieldValue::Choice(index)) => {
            config.theme.backdrop = BACKDROPS
                .get(index)
                .map_or(config.theme.backdrop, |entry| entry.0);
            Effect::Appearance
        }
        (field::CORNERS, FieldValue::Slider(radius)) => {
            config.theme.corner_radius = radius.clamp(0, corner::MAX.into()) as u8;
            Effect::Appearance
        }
        (field::TINT_OPACITY, FieldValue::Slider(value)) => {
            config.theme.tint_opacity = value.clamp(0, 100) as u8;
            Effect::Appearance
        }
        (field::TITLE_SHADING, FieldValue::Slider(value)) => {
            config.theme.title_shading = value.clamp(0, 100) as u8;
            Effect::Appearance
        }
        (field::BORDER, FieldValue::Toggle(on)) => {
            config.theme.show_border = on;
            Effect::Appearance
        }
        (field::MATCH_EXPLORER, FieldValue::Toggle(on)) => {
            config.theme.match_explorer = on;
            // Mirroring also imports Explorer's icon size and item spacing, so
            // the grid has to re-flow, not just restyle.
            Effect::Layout
        }
        (field::LABEL_LINES, FieldValue::Choice(index)) => {
            config.theme.label_lines = LABEL_LINES_OPTIONS
                .get(index)
                .map_or(config.theme.label_lines, |entry| entry.0);
            Effect::Appearance
        }
        (field::LABEL_SIZE, FieldValue::Slider(value)) => {
            config.theme.label_size =
                value.clamp(LABEL_SIZE_RANGE.0.into(), LABEL_SIZE_RANGE.1.into()) as u8;
            Effect::Appearance
        }
        (field::LABEL_SHADOW, FieldValue::Choice(index)) => {
            config.theme.label_shadow = LABEL_SHADOW_OPTIONS
                .get(index)
                .map_or(config.theme.label_shadow, |entry| entry.0);
            Effect::Appearance
        }
        (field::ICON_SIZE, FieldValue::Slider(value)) => {
            config.theme.icon_size =
                value.clamp(ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into()) as u8;
            Effect::Layout
        }
        (field::OVERLAY_SIZE, FieldValue::Slider(value)) => {
            config.theme.overlay_size =
                value.clamp(OVERLAY_SIZE_RANGE.0.into(), OVERLAY_SIZE_RANGE.1.into()) as u8;
            Effect::Appearance
        }
        (field::GRID_GAP, FieldValue::Slider(value)) => {
            config.desktop.layout.gap = value.clamp(GAP_RANGE.0, GAP_RANGE.1);
            Effect::Layout
        }
        (field::GRID_PADDING, FieldValue::Slider(value)) => {
            config.desktop.layout.padding = value.clamp(GAP_RANGE.0, GAP_RANGE.1);
            Effect::Layout
        }
        (field::CUSTOM_COLORS, FieldValue::Toggle(on)) => {
            if on && !config.theme.custom_colors {
                // Start from what is on screen rather than a stale color.
                let palette = config.theme.palette(system_dark);
                config.theme.tint = palette.tint.with_alpha(255);
                config.theme.text = palette.title_text;
            }
            config.theme.custom_colors = on;
            Effect::Appearance
        }
        (field::TINT, FieldValue::Color(color)) => {
            config.theme.tint = color.with_alpha(255);
            Effect::Appearance
        }
        (field::TEXT, FieldValue::Color(color)) => {
            config.theme.text = color.with_alpha(255);
            Effect::Appearance
        }
        (field::USE_ACCENT, FieldValue::Toggle(on)) => {
            config.theme.use_accent = on;
            Effect::Appearance
        }
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
        (field::HOTKEY, FieldValue::Text(text)) => {
            session.hotkey_text = text;
            session.hotkey_error = None;
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
        _ => Effect::None,
    };
    if effect != Effect::None {
        session.dirty = true;
    }
    effect
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
    fn match_explorer_toggle_applies_live() {
        let (mut config, mut session) = setup();
        assert!(config.theme.match_explorer);
        let effect = apply_field(
            &mut config,
            &mut session,
            field::MATCH_EXPLORER,
            FieldValue::Toggle(false),
            true,
        );
        assert_eq!(effect, Effect::Layout);
        assert!(!config.theme.match_explorer);
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

        let mut match_explorer = config.clone();
        match_explorer.theme.match_explorer = !config.theme.match_explorer;
        assert!(needs_relayout(&session, &match_explorer));

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
        assert_eq!(session.hotkey_text, "Ctrl+Shift+F");
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(form.apply_enabled);
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
        session.hotkey_error = Some("bad".into());
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(form.field(field::HOTKEY_NOTE).unwrap().label, "bad");
    }

    use deskspace_core::Rect;

    fn config_with_space(title: &str, opacity: u8) -> (Config, SpaceId) {
        let mut config = Config::new();
        let id = config.desktop.add_space(title, Rect::new(0, 0, 200, 200));
        config.desktop.space_mut(id).unwrap().opacity = opacity;
        (config, id)
    }

    fn space_labels(form: &SettingsForm) -> Vec<String> {
        form.nav.as_ref().unwrap().groups[1]
            .entries
            .iter()
            .map(|entry| entry.label.clone())
            .collect()
    }

    #[test]
    fn rail_maps_entries_to_pages_and_marks_customized_spaces() {
        let (config, _) = config_with_space("Work", 200);
        let session = Session::new(&config);
        let form = build_form(&config, &session, true, Path::new("x"));
        let nav = form.nav.as_ref().expect("the rail is set");
        assert_eq!(nav.groups[0].title, "Settings");
        assert_eq!(nav.groups[0].entries.len(), 7);
        assert_eq!(nav.groups[1].title, "Spaces");
        let space_page = nav.groups[0].entries.len();
        for (index, entry) in nav.groups[0].entries.iter().enumerate() {
            assert_eq!(entry.page, index);
        }
        let space_entry = &nav.groups[1].entries[0];
        assert_eq!(space_entry.label, "Work");
        assert!(space_entry.marked);
        assert_eq!(space_entry.page, space_page);
        assert_eq!(form.pages.len(), space_page + 1);
        assert_eq!(nav.selected, 0);

        // Selecting the space entry fills and enables the Space page.
        let mut selected = Session::new(&config);
        selected.nav_selected = space_page;
        let form = build_form(&config, &selected, true, Path::new("x"));
        assert!(form.field(field::SPACE_OPACITY).unwrap().enabled);
        assert_eq!(
            form.field(field::SPACE_NOTE).unwrap().label,
            "Editing \u{201c}Work\u{201d}."
        );
        match &form.field(field::SPACE_OPACITY).unwrap().kind {
            FieldKind::Slider { value, .. } => assert_eq!(*value, 78),
            other => panic!("expected a slider, got {other:?}"),
        }
    }

    #[test]
    fn rail_lists_only_spaces_with_overrides() {
        let mut config = Config::new();
        let _plain = config.desktop.add_space("Plain", Rect::new(0, 0, 200, 200));
        let tinted = config
            .desktop
            .add_space("Tinted", Rect::new(0, 0, 200, 200));
        config.desktop.space_mut(tinted).unwrap().tint = Some(Color::rgb(1, 2, 3));
        let session = Session::new(&config);
        assert_eq!(session.listed_spaces, vec![tinted]);
        let saved = session
            .saved_spaces
            .iter()
            .find(|saved| saved.id == tinted)
            .unwrap();
        assert_eq!(saved.tint, Some(Color::rgb(1, 2, 3)));
        assert_eq!(saved.opacity, 255);

        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(space_labels(&form), ["Tinted"]);
    }

    #[test]
    fn rail_drops_deleted_spaces_but_keeps_the_listing() {
        let mut config = Config::new();
        let kept = config.desktop.add_space("Kept", Rect::new(0, 0, 200, 200));
        config.desktop.space_mut(kept).unwrap().icon_size = Some(150);
        let removed = config
            .desktop
            .add_space("Removed", Rect::new(0, 0, 200, 200));
        config.desktop.space_mut(removed).unwrap().opacity = 100;
        let session = Session::new(&config);
        assert_eq!(session.listed_spaces, vec![kept, removed]);
        config.desktop.remove_space(removed);

        // The deleted space is dropped from the rail.
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(space_labels(&form), ["Kept"]);

        // Resetting the last override does not pull the entry out (sticky),
        // but the entry is no longer marked.
        config.desktop.space_mut(kept).unwrap().icon_size = None;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(space_labels(&form), ["Kept"]);
        assert!(!form.nav.as_ref().unwrap().groups[1].entries[0].marked);
    }

    #[test]
    fn space_page_is_disabled_with_no_selection() {
        let (config, session) = setup();
        let form = build_form(&config, &session, true, Path::new("x"));
        for id in [
            field::SPACE_OPACITY,
            field::SPACE_ICON_SIZE,
            field::SPACE_TINT_TOGGLE,
            field::SPACE_TINT,
            field::SPACE_TEXT_TOGGLE,
            field::SPACE_TEXT,
            field::SPACE_RESET,
        ] {
            assert!(!form.field(id).unwrap().enabled, "field {id} should be off");
        }
        assert_eq!(
            form.field(field::SPACE_NOTE).unwrap().label,
            "No spaces are customized yet. Right-click a space and choose Appearance."
        );
    }

    #[test]
    fn opacity_percent_round_trips_through_bytes() {
        for percent in [20, 40, 60, 80, 100] {
            assert_eq!(opacity_percent(opacity_byte(percent)), percent);
        }
        assert_eq!(opacity_byte(100), 255);
        assert_eq!(opacity_byte(20), 51);
    }

    #[test]
    fn space_page_ids_are_recognized() {
        assert!(is_space_field(field::SPACE_OPACITY));
        assert!(is_space_field(field::SPACE_RESET));
        assert!(!is_space_field(field::ICON_SIZE));
        assert!(!is_space_field(field::RESET));
    }

    #[test]
    fn global_page_count_matches_the_settings_group() {
        let (config, session) = setup();
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(
            form.nav.as_ref().unwrap().groups[0].entries.len(),
            GLOBAL_PAGE_COUNT
        );
    }

    #[test]
    fn selected_space_follows_the_rail_selection() {
        let (config, id) = config_with_space("Work", 200);
        let mut session = Session::new(&config);
        assert_eq!(selected_space(&config, &session), None);
        session.nav_selected = GLOBAL_PAGE_COUNT;
        assert_eq!(selected_space(&config, &session), Some(id));
        session.nav_selected = GLOBAL_PAGE_COUNT + 1;
        assert_eq!(selected_space(&config, &session), None);
    }

    #[test]
    fn space_field_command_maps_every_field() {
        let mut config = Config::new();
        let id = config.desktop.add_space("Work", Rect::new(0, 0, 200, 200));
        let palette = Theme::default().palette(false);
        let space = config.desktop.space(id).unwrap();

        assert_eq!(
            space_field_command(
                space,
                field::SPACE_OPACITY,
                FieldValue::Slider(100),
                &palette
            ),
            Some(Command::SetSpaceOpacity { id, opacity: 255 })
        );
        assert_eq!(
            space_field_command(
                space,
                field::SPACE_OPACITY,
                FieldValue::Slider(20),
                &palette
            ),
            Some(Command::SetSpaceOpacity { id, opacity: 51 })
        );
        assert_eq!(
            space_field_command(
                space,
                field::SPACE_ICON_SIZE,
                FieldValue::Choice(3),
                &palette
            ),
            Some(Command::SetSpaceIconSize {
                id,
                icon_size: Some(150)
            })
        );
        assert_eq!(
            space_field_command(
                space,
                field::SPACE_ICON_SIZE,
                FieldValue::Choice(0),
                &palette
            ),
            Some(Command::SetSpaceIconSize {
                id,
                icon_size: None
            })
        );
        assert_eq!(
            space_field_command(
                space,
                field::SPACE_TINT_TOGGLE,
                FieldValue::Toggle(true),
                &palette
            ),
            Some(Command::SetSpaceTint {
                id,
                tint: Some(palette.tint.with_alpha(255))
            })
        );
        assert_eq!(
            space_field_command(
                space,
                field::SPACE_TINT_TOGGLE,
                FieldValue::Toggle(false),
                &palette
            ),
            Some(Command::SetSpaceTint { id, tint: None })
        );
        let tint = Color::rgb(1, 2, 3);
        assert_eq!(
            space_field_command(space, field::SPACE_TINT, FieldValue::Color(tint), &palette),
            Some(Command::SetSpaceTint {
                id,
                tint: Some(tint)
            })
        );
        assert_eq!(
            space_field_command(
                space,
                field::SPACE_TEXT_TOGGLE,
                FieldValue::Toggle(true),
                &palette
            ),
            Some(Command::SetSpaceText {
                id,
                text: Some(palette.title_text)
            })
        );
        assert_eq!(
            space_field_command(
                space,
                field::SPACE_TEXT_TOGGLE,
                FieldValue::Toggle(false),
                &palette
            ),
            Some(Command::SetSpaceText { id, text: None })
        );
        let text = Color::rgb(4, 5, 6);
        assert_eq!(
            space_field_command(space, field::SPACE_TEXT, FieldValue::Color(text), &palette),
            Some(Command::SetSpaceText {
                id,
                text: Some(text)
            })
        );
        assert_eq!(
            space_field_command(space, field::SPACE_NOTE, FieldValue::Slider(0), &palette),
            None
        );
    }
}
