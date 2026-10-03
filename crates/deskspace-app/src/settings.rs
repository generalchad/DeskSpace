//! The settings window's content and how edits map onto the configuration.
//! Pure functions over `Config`, so the form logic is testable without Win32.

use std::path::Path;

use deskspace_config::{Config, WindowSettings};
use deskspace_core::theme::corner;
use deskspace_core::{Backdrop, GridSpec, LabelShadow, Theme, ThemeMode};
use deskspace_platform::{Field, FieldValue, SettingsForm, SettingsPage, SliderMark};

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
        }
    }

    /// Marks the current config as the new baseline after a successful save.
    pub fn commit(&mut self, config: &Config) {
        self.saved_theme = config.theme.clone();
        self.saved_window = config.window.clone();
        self.saved_layout = config.desktop.layout;
        self.hotkey_error = None;
        self.dirty = false;
    }

    pub fn revert(&self, config: &mut Config) {
        config.theme = self.saved_theme.clone();
        config.window = self.saved_window.clone();
        config.desktop.layout = self.saved_layout;
    }
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

pub fn build_form(
    config: &Config,
    session: &Session,
    system_dark: bool,
    config_path: &Path,
) -> SettingsForm {
    let theme = &config.theme;
    let palette = theme.palette(system_dark);
    let solid = theme.backdrop == Backdrop::Solid;

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
                (60, 160),
                theme.label_size.clamp(60, 160) as i32,
            ),
            Field::choice(
                field::LABEL_SHADOW,
                "Label shadow",
                &labels(&LABEL_SHADOW_OPTIONS),
                index_of(&LABEL_SHADOW_OPTIONS, theme.label_shadow),
            ),
            Field::heading(field::LAYOUT_HEADING, "Layout"),
            Field::slider(
                field::GRID_GAP,
                "Icon spacing",
                (0, 24),
                config.desktop.layout.gap.clamp(0, 24),
            )
            .suffix(" px"),
            Field::slider(
                field::GRID_PADDING,
                "Edge padding",
                (0, 24),
                config.desktop.layout.padding.clamp(0, 24),
            )
            .suffix(" px"),
            Field::heading(field::COLORS_HEADING, "Colors"),
            Field::toggle(
                field::CUSTOM_COLORS,
                "Use custom colors instead of the color mode's",
                theme.custom_colors,
            ),
            Field::color(field::TINT, "Background tint", palette.tint.with_alpha(255))
                .enabled(theme.custom_colors),
            Field::color(field::TEXT, "Text", palette.title_text).enabled(theme.custom_colors),
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
                (0, 64),
                config.desktop.layout.snap.clamp(0, 64),
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

    SettingsForm {
        title: "DeskSpace Settings".into(),
        pages: vec![appearance, general, about],
        apply_enabled: session.dirty,
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
        (field::LABEL_LINES, FieldValue::Choice(index)) => {
            config.theme.label_lines = LABEL_LINES_OPTIONS
                .get(index)
                .map_or(config.theme.label_lines, |entry| entry.0);
            Effect::Appearance
        }
        (field::LABEL_SIZE, FieldValue::Slider(value)) => {
            config.theme.label_size = value.clamp(60, 160) as u8;
            Effect::Appearance
        }
        (field::LABEL_SHADOW, FieldValue::Choice(index)) => {
            config.theme.label_shadow = LABEL_SHADOW_OPTIONS
                .get(index)
                .map_or(config.theme.label_shadow, |entry| entry.0);
            Effect::Appearance
        }
        (field::GRID_GAP, FieldValue::Slider(value)) => {
            config.desktop.layout.gap = value.clamp(0, 24);
            Effect::Layout
        }
        (field::GRID_PADDING, FieldValue::Slider(value)) => {
            config.desktop.layout.padding = value.clamp(0, 24);
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
            config.desktop.layout.snap = value.clamp(0, 64);
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
    fn appearance_edits_apply_live_and_mark_dirty() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::BACKDROP,
            FieldValue::Choice(1),
            true,
        );
        assert_eq!(effect, Effect::Appearance);
        assert_eq!(config.theme.backdrop, Backdrop::Blur);
        assert!(session.dirty);
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
}
