use deskspace_core::layout::position_items;
use deskspace_core::{Desktop, Theme};
use serde::{Deserialize, Serialize};

/// Version 2 replaced the color-key theme with the Windows 11 theme model and
/// made item positions relative to their space.
pub const CURRENT_VERSION: u32 = 2;

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
    /// Host spaces on the desktop (no blur) instead of top-level windows.
    /// Applied when DeskSpace starts.
    pub always_on_desktop: bool,
    pub toggle_hotkey: String,
    pub start_hidden: bool,
    /// Run DeskSpace at sign-in through the per-user `Run` key.
    pub start_with_windows: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            always_on_desktop: false,
            toggle_hotkey: "Ctrl+Alt+D".to_string(),
            start_hidden: false,
            start_with_windows: false,
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
            let layout = self.desktop.layout;
            for space in &mut self.desktop.spaces {
                // v1 stored 210 as the fill alpha, which no later setting
                // means; drop it rather than fold it into a custom look.
                space.clear_legacy_overrides();
                // v1 stored absolute item positions that went stale when the
                // space moved. Positions are now space-relative grid cells.
                position_items(space, layout);
            }
        }
        if self.version < CURRENT_VERSION {
            self.version = CURRENT_VERSION;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::theme::corner;
    use deskspace_core::{Backdrop, Point};

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let parsed: Config = toml::from_str("version = 2\n").unwrap();
        assert_eq!(parsed.version, 2);
        assert!(parsed.desktop.spaces.is_empty());
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
        assert!(!parsed.window.always_on_desktop);
        assert_eq!(parsed.desktop.layout.columns, 6);
        assert_eq!(
            parsed.desktop.layout.snap,
            deskspace_core::GridSpec::default().snap
        );
        // mica from early v2 files maps onto the blur material.
        assert_eq!(parsed.theme.backdrop, Backdrop::Blur);
        assert_eq!(parsed.theme.tint_opacity, Theme::default().tint_opacity);
        assert_eq!(parsed.desktop.spaces[0].items.len(), 1);
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
        let space = &parsed.desktop.spaces[0];
        assert!(!space.custom && space.look.is_none());
        assert_eq!(
            space.items[0].position,
            parsed.desktop.layout.grid_position(0)
        );
        assert_ne!(space.items[0].position, Point::new(412, 340));
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
        assert!(!parsed.desktop.spaces[0].custom);
        let big = &parsed.desktop.spaces[1];
        assert!(big.custom);
        assert_eq!(big.look.as_ref().unwrap().theme.icon_size, 150);

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
    fn start_with_windows_defaults_to_false() {
        let parsed: Config = toml::from_str("version = 2\n[window]\n").unwrap();
        assert!(!parsed.window.start_with_windows);
    }

    #[test]
    fn round_trips_through_toml() {
        let mut config = Config::new();
        config
            .desktop
            .add_space("Work", deskspace_core::Rect::new(10, 20, 300, 200));
        config.theme.custom_colors = true;
        let text = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed.desktop.spaces.len(), 1);
        assert_eq!(parsed.desktop.spaces[0].title, "Work");
        assert!(parsed.theme.custom_colors);
    }
}
