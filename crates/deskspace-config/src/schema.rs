use deskspace_core::{Desktop, Theme};
use serde::{Deserialize, Serialize};

/// Version 2 replaced the color-key theme with the Windows 11 theme model and
/// made item positions relative to their space.
pub const CURRENT_VERSION: u32 = 2;

/// The inclusive range of a new space's default width and height, in pixels.
pub const NEW_SPACE_SIZE_RANGE: (i32, i32) = (120, 1200);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// A missing version deserializes as 0 so `migrate` treats the file as legacy.
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub desktop: Desktop,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub window: WindowSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    /// Shows or hides the spaces. Empty means unbound.
    pub toggle_hotkey: String,
    /// Creates a space on the pointer's monitor. Empty means unbound.
    pub new_space_hotkey: String,
    /// Opens the settings window. Empty means unbound.
    pub settings_hotkey: String,
    /// Start with the spaces hidden (`--show` overrides it).
    pub start_hidden: bool,
    /// Run DeskSpace at sign-in through the per-user `Run` key.
    pub start_with_windows: bool,
    /// Hide Explorer's desktop icons and draw the loose items on DeskSpace's
    /// own desktop layer (M2 takeover). Off by default.
    pub take_over_desktop: bool,
    /// The title a space created from a menu or hotkey starts with.
    pub new_space_title: String,
    /// The size a new space starts with, in pixels, clamped to
    /// `NEW_SPACE_SIZE_RANGE`.
    pub new_space_width: i32,
    /// See `new_space_width`.
    pub new_space_height: i32,
    /// Whether a new space starts locked (kept at its size, scrolling instead
    /// of growing).
    pub new_space_locked: bool,
    /// Check for a newer release in the background, about once a day. Off by
    /// default: no network traffic happens unless the user turns this on or
    /// starts a check from the tray.
    pub check_for_updates: bool,
    /// Install an update the automatic check finds without asking: download
    /// it in the background, then install it while the user is away. Off by
    /// default; it only takes effect with `check_for_updates` on, in the
    /// installed build.
    pub install_updates: bool,
    /// Unix seconds of the last completed update check, so an automatic check
    /// runs at most once a day.
    pub last_update_check: Option<u64>,
    /// A version the user chose to skip; an automatic check does not offer it
    /// again.
    pub skipped_version: Option<String>,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            toggle_hotkey: "Ctrl+Alt+D".to_string(),
            new_space_hotkey: String::new(),
            settings_hotkey: String::new(),
            start_hidden: false,
            start_with_windows: false,
            take_over_desktop: false,
            new_space_title: "New space".to_string(),
            new_space_width: 240,
            new_space_height: 120,
            new_space_locked: false,
            check_for_updates: false,
            install_updates: false,
            last_update_check: None,
            skipped_version: None,
        }
    }
}

impl WindowSettings {
    /// The settings with `new_space_width` and `new_space_height` brought into
    /// `NEW_SPACE_SIZE_RANGE`, so a hand-edited file loads in range.
    pub fn clamped(self) -> Self {
        let size = |value: i32| value.clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1);
        Self {
            new_space_width: size(self.new_space_width),
            new_space_height: size(self.new_space_height),
            ..self
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            desktop: Desktop::default(),
            theme: Theme::default(),
            window: WindowSettings::default(),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this file was written by a newer DeskSpace. Saving it would drop
    /// fields this build does not know about.
    pub fn is_from_newer_version(&self) -> bool {
        self.version > CURRENT_VERSION
    }

    pub fn migrate(&mut self) {
        if self.version < 2 {
            // The v1 theme was opaque colors tuned for a color-keyed GDI
            // overlay; carried over, they would hide the acrylic backdrop.
            self.theme = Theme::default();
            self.desktop.reset_v1_spaces();
        }
        if self.version < CURRENT_VERSION {
            self.version = CURRENT_VERSION;
        }
        // Clamp ranged values so a hand-edited out-of-range setting cannot
        // leave the settings window showing no choice selected.
        self.theme = self.theme.clone().clamped();
        self.window = self.window.clone().clamped();
        self.desktop.layout = self.desktop.layout.clamped();
        self.desktop.clamp_looks();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::layout::{GAP_RANGE, SNAP_RANGE};
    use deskspace_core::theme::{ICON_SIZE_RANGE, OVERLAY_SIZE_RANGE, corner};
    use deskspace_core::{Backdrop, Point};

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let parsed: Config = toml::from_str("version = 2\n").unwrap();
        assert_eq!(parsed.version, 2);
        assert!(parsed.desktop.spaces().is_empty());
        assert_eq!(parsed.window.toggle_hotkey, "Ctrl+Alt+D");
        assert_eq!(parsed.theme, Theme::default());
    }

    #[test]
    fn partial_tables_keep_spaces_and_fill_defaults() {
        let text = r#"
            version = 2

            [window]
            toggle_hotkey = "Ctrl+Shift+F"

            [desktop.layout]
            columns = 6

            [theme]
            backdrop = "mica"

            [[desktop.spaces]]
            id = 1
            title = "Work"
            rect = { x = 0, y = 0, width = 100, height = 100 }

            [[desktop.spaces.items]]
            id = 1
            path = "C:/a.txt"
            label = "a"
        "#;
        let parsed: Config = toml::from_str(text).unwrap();
        assert_eq!(parsed.window.toggle_hotkey, "Ctrl+Shift+F");
        assert_eq!(parsed.desktop.layout.columns, 6);
        assert_eq!(
            parsed.desktop.layout.snap,
            deskspace_core::GridSpec::default().snap
        );
        // mica from early v2 files maps onto the blur material.
        assert_eq!(parsed.theme.backdrop, Backdrop::Blur);
        assert_eq!(parsed.theme.tint_opacity, Theme::default().tint_opacity);
        assert_eq!(parsed.desktop.spaces()[0].items().len(), 1);
    }

    #[test]
    fn theme_enums_serialize_as_snake_case() {
        let text = toml::to_string_pretty(&Config::new()).unwrap();
        assert!(text.contains("backdrop = \"acrylic\""));
        assert!(text.contains("mode = \"system\""));
        assert!(text.contains("corner_radius = 8"));
    }

    #[test]
    fn corner_radius_reads_numbers_and_old_styles() {
        let parse = |theme: &str| {
            toml::from_str::<Config>(&format!("version = 2\n[theme]\n{theme}"))
                .unwrap()
                .theme
                .corner_radius
        };
        assert_eq!(parse("corner_radius = 12"), 12);
        assert_eq!(parse("corner_radius = 99"), corner::MAX);
        assert_eq!(parse("corners = \"square\""), corner::SQUARE);
        assert_eq!(parse("corners = \"small\""), corner::SMALL);
        assert_eq!(parse("corners = \"round\""), corner::WINDOWS);
        assert_eq!(parse(""), corner::WINDOWS);
    }

    #[test]
    fn missing_version_is_migrated() {
        let mut parsed: Config = toml::from_str("").unwrap();
        assert_eq!(parsed.version, 0);
        parsed.migrate();
        assert_eq!(parsed.version, CURRENT_VERSION);
    }

    #[test]
    fn v1_files_get_the_new_theme_and_relative_items() {
        let text = r#"
            version = 1

            [theme]
            background_fill = { r = 1, g = 2, b = 3, a = 255 }
            title_text = { r = 9, g = 9, b = 9, a = 255 }

            [[desktop.spaces]]
            id = 1
            title = "Work"
            rect = { x = 400, y = 300, width = 100, height = 100 }
            opacity = 210

            [[desktop.spaces.items]]
            id = 1
            path = "C:/a.txt"
            label = "a"
            position = { x = 412, y = 340 }
        "#;
        let mut parsed: Config = toml::from_str(text).unwrap();
        parsed.migrate();
        assert_eq!(parsed.version, CURRENT_VERSION);
        assert_eq!(parsed.theme, Theme::default());
        let space = &parsed.desktop.spaces()[0];
        assert!(!space.custom() && space.look().is_none());
        assert_eq!(
            space.items()[0].position,
            parsed.desktop.layout.grid_position(0)
        );
        assert_ne!(space.items()[0].position, Point::new(412, 340));
        let theme = parsed.theme.clone();
        assert!(!parsed.desktop.fold_legacy_overrides(&theme, false));
    }

    #[test]
    fn current_files_are_not_rewritten_by_migrate() {
        let mut config = Config::new();
        config.theme.tint_opacity = 77;
        config.migrate();
        assert_eq!(config.theme.tint_opacity, 77);
    }

    #[test]
    fn migrate_clamps_out_of_range_values() {
        let text = r#"
            version = 2

            [theme]
            icon_size = 1
            tint_opacity = 250
            label_lines = 7

            [[desktop.spaces]]
            id = 1
            title = "Work"
            rect = { x = 0, y = 0, width = 100, height = 100 }

            [desktop.spaces.look]
            gap = 999
            padding = -4

            [desktop.spaces.look.theme]
            overlay_size = 5
        "#;
        let mut parsed: Config = toml::from_str(text).unwrap();
        parsed.migrate();
        assert_eq!(parsed.theme.icon_size, ICON_SIZE_RANGE.0);
        assert_eq!(parsed.theme.tint_opacity, 100);
        assert_eq!(parsed.theme.label_lines, 2);
        let look = parsed.desktop.spaces()[0].look().unwrap();
        assert_eq!(look.gap, GAP_RANGE.1);
        assert_eq!(look.padding, GAP_RANGE.0);
        assert_eq!(look.theme.overlay_size, OVERLAY_SIZE_RANGE.0);
    }

    #[test]
    fn window_settings_clamped_brings_sizes_into_range() {
        let settings = WindowSettings {
            new_space_width: 5,
            new_space_height: 9999,
            ..WindowSettings::default()
        }
        .clamped();
        assert_eq!(settings.new_space_width, NEW_SPACE_SIZE_RANGE.0);
        assert_eq!(settings.new_space_height, NEW_SPACE_SIZE_RANGE.1);
        assert_eq!(
            WindowSettings::default().clamped(),
            WindowSettings::default()
        );
    }

    #[test]
    fn migrate_clamps_the_grid_and_new_space_size() {
        let text = r#"
            version = 2

            [desktop.layout]
            gap = 999
            padding = -4
            snap = 500

            [window]
            new_space_width = 5
            new_space_height = 9999
        "#;
        let mut parsed: Config = toml::from_str(text).unwrap();
        parsed.migrate();
        assert_eq!(parsed.desktop.layout.gap, GAP_RANGE.1);
        assert_eq!(parsed.desktop.layout.padding, GAP_RANGE.0);
        assert_eq!(parsed.desktop.layout.snap, SNAP_RANGE.1);
        assert_eq!(parsed.window.new_space_width, NEW_SPACE_SIZE_RANGE.0);
        assert_eq!(parsed.window.new_space_height, NEW_SPACE_SIZE_RANGE.1);
    }

    #[test]
    fn v2_space_overrides_fold_into_a_custom_look_that_round_trips() {
        let text = r#"
            version = 2

            [[desktop.spaces]]
            id = 1
            title = "Work"
            rect = { x = 0, y = 0, width = 100, height = 100 }

            [[desktop.spaces]]
            id = 2
            title = "Big"
            rect = { x = 0, y = 0, width = 100, height = 100 }
            icon_size = 150
        "#;
        let mut parsed: Config = toml::from_str(text).unwrap();
        let theme = parsed.theme.clone();
        assert!(parsed.desktop.fold_legacy_overrides(&theme, false));
        assert!(!parsed.desktop.spaces()[0].custom());
        let big = &parsed.desktop.spaces()[1];
        assert!(big.custom());
        assert_eq!(big.look().unwrap().theme.icon_size, 150);

        let written = toml::to_string(&parsed).unwrap();
        let reread: Config = toml::from_str(&written).unwrap();
        assert_eq!(reread, parsed);
    }

    #[test]
    fn use_accent_defaults_to_false_and_parses_when_set() {
        let parsed: Config = toml::from_str("version = 2\n[theme]\n").unwrap();
        assert!(!parsed.theme.use_accent);
        let parsed: Config = toml::from_str("version = 2\n[theme]\nuse_accent = true\n").unwrap();
        assert!(parsed.theme.use_accent);
    }

    #[test]
    fn ui_mode_defaults_to_system_and_parses_when_set() {
        let parsed: Config = toml::from_str("version = 2\n[theme]\n").unwrap();
        assert_eq!(parsed.theme.ui_mode, deskspace_core::ThemeMode::System);
        let parsed: Config = toml::from_str("version = 2\n[theme]\nui_mode = \"dark\"\n").unwrap();
        assert_eq!(parsed.theme.ui_mode, deskspace_core::ThemeMode::Dark);
    }

    #[test]
    fn selection_box_defaults_and_parses_when_set() {
        let parsed: Config = toml::from_str("version = 2\n[theme]\n").unwrap();
        assert_eq!(
            parsed.theme.marquee_color,
            deskspace_core::theme::MARQUEE_COLOR_DEFAULT
        );
        assert!(parsed.theme.show_marquee_border);
        let parsed: Config = toml::from_str(
            "version = 2\n[theme]\nmarquee_color = { r = 1, g = 2, b = 3, a = 255 }\n\
             show_marquee_border = false\n",
        )
        .unwrap();
        assert_eq!(
            parsed.theme.marquee_color,
            deskspace_core::Color::rgb(1, 2, 3)
        );
        assert!(!parsed.theme.show_marquee_border);
    }

    #[test]
    fn preset_defaults_to_match_explorer_and_reads_the_legacy_flag() {
        let parsed: Config = toml::from_str("version = 2\n[theme]\n").unwrap();
        assert_eq!(parsed.theme.preset, deskspace_core::Preset::MatchExplorer);
        assert!(parsed.theme.match_explorer());
        let parsed: Config = toml::from_str("version = 2\n[theme]\npreset = \"custom\"\n").unwrap();
        assert_eq!(parsed.theme.preset, deskspace_core::Preset::Custom);
        // The older boolean setting maps onto the preset.
        let parsed: Config =
            toml::from_str("version = 2\n[theme]\nmatch_explorer = false\n").unwrap();
        assert_eq!(parsed.theme.preset, deskspace_core::Preset::Custom);
        assert!(!parsed.theme.match_explorer());
        let parsed: Config =
            toml::from_str("version = 2\n[theme]\nmatch_explorer = true\n").unwrap();
        assert_eq!(parsed.theme.preset, deskspace_core::Preset::MatchExplorer);
    }

    #[test]
    fn preset_serializes_as_snake_case_without_the_legacy_flag() {
        let mut config = Config::new();
        config.theme.preset = deskspace_core::Preset::Custom;
        let text = toml::to_string(&config).unwrap();
        assert!(text.contains("preset = \"custom\""), "{text}");
        assert!(!text.contains("match_explorer"), "{text}");
    }

    #[test]
    fn extra_hotkeys_default_to_unbound_and_parse_when_set() {
        let parsed: Config = toml::from_str("version = 2\n[window]\n").unwrap();
        assert!(parsed.window.new_space_hotkey.is_empty());
        assert!(parsed.window.settings_hotkey.is_empty());
        let parsed: Config = toml::from_str(
            "version = 2\n[window]\nnew_space_hotkey = \"Ctrl+Alt+N\"\nsettings_hotkey = \"Ctrl+Alt+S\"\n",
        )
        .unwrap();
        assert_eq!(parsed.window.new_space_hotkey, "Ctrl+Alt+N");
        assert_eq!(parsed.window.settings_hotkey, "Ctrl+Alt+S");
    }

    #[test]
    fn new_space_defaults_are_the_historic_title_and_size() {
        let parsed: Config = toml::from_str("version = 2\n[window]\n").unwrap();
        assert_eq!(parsed.window.new_space_title, "New space");
        assert_eq!(parsed.window.new_space_width, 240);
        assert_eq!(parsed.window.new_space_height, 120);
        assert!(!parsed.window.new_space_locked);
        let parsed: Config = toml::from_str(
            "version = 2\n[window]\nnew_space_title = \"Inbox\"\nnew_space_width = 320\n\
             new_space_height = 200\nnew_space_locked = true\n",
        )
        .unwrap();
        assert_eq!(parsed.window.new_space_title, "Inbox");
        assert_eq!(parsed.window.new_space_width, 320);
        assert_eq!(parsed.window.new_space_height, 200);
        assert!(parsed.window.new_space_locked);
    }

    #[test]
    fn start_with_windows_defaults_to_false() {
        let parsed: Config = toml::from_str("version = 2\n[window]\n").unwrap();
        assert!(!parsed.window.start_with_windows);
    }

    #[test]
    fn take_over_desktop_defaults_to_false_and_parses_when_set() {
        let parsed: Config = toml::from_str("version = 2\n[window]\n").unwrap();
        assert!(!parsed.window.take_over_desktop);
        let parsed: Config =
            toml::from_str("version = 2\n[window]\ntake_over_desktop = true\n").unwrap();
        assert!(parsed.window.take_over_desktop);
    }

    #[test]
    fn update_settings_default_and_parse() {
        let parsed: Config = toml::from_str("version = 2\n[window]\n").unwrap();
        assert!(!parsed.window.check_for_updates);
        assert_eq!(parsed.window.last_update_check, None);
        assert_eq!(parsed.window.skipped_version, None);
        assert!(!parsed.window.install_updates);

        let parsed: Config = toml::from_str(
            "version = 2\n[window]\ncheck_for_updates = true\ninstall_updates = true\nlast_update_check = 42\nskipped_version = \"0.3.0\"\n",
        )
        .unwrap();
        assert!(parsed.window.check_for_updates);
        assert!(parsed.window.install_updates);
        assert_eq!(parsed.window.last_update_check, Some(42));
        assert_eq!(parsed.window.skipped_version.as_deref(), Some("0.3.0"));
    }

    #[test]
    fn desktop_layer_defaults_to_empty() {
        let parsed: Config = toml::from_str("version = 2\n").unwrap();
        assert!(parsed.desktop.layer().items().is_empty());
    }

    #[test]
    fn round_trips_through_toml() {
        let mut config = Config::new();
        let create = deskspace_core::Command::CreateSpace {
            title: "Work".to_owned(),
            at: Point::new(10, 20),
            size: deskspace_core::Size::new(300, 200),
            locked: false,
        };
        let bounds = deskspace_core::Rect::new(0, 0, 1920, 1040);
        assert!(config.desktop.apply(create, bounds).is_applied());
        config.theme.custom_colors = true;
        let text = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed.desktop.spaces().len(), 1);
        assert_eq!(parsed.desktop.spaces()[0].title(), "Work");
        assert!(parsed.theme.custom_colors);
    }
}
