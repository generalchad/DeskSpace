use deskspace_core::{Desktop, GridSpec, Theme};
use serde::{Deserialize, Serialize};

pub const CURRENT_VERSION: u32 = 1;

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
    pub snap_grid: i32,
    pub background_alpha: u8,
    pub always_on_desktop: bool,
    pub toggle_hotkey: String,
    pub start_hidden: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            snap_grid: GridSpec::default().snap,
            background_alpha: 255,
            always_on_desktop: false,
            toggle_hotkey: "Ctrl+Alt+D".to_string(),
            start_hidden: false,
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
        if self.version == 0 {
            self.version = CURRENT_VERSION;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let parsed: Config = toml::from_str("version = 1\n").unwrap();
        assert_eq!(parsed.version, 1);
        assert!(parsed.desktop.spaces.is_empty());
        assert_eq!(parsed.window.toggle_hotkey, "Ctrl+Alt+D");
    }

    #[test]
    fn partial_tables_keep_spaces_and_fill_defaults() {
        let text = r#"
            version = 1

            [window]
            toggle_hotkey = "Ctrl+Shift+F"

            [desktop.layout]
            columns = 6

            [theme]
            background_fill = { r = 1, g = 2, b = 3, a = 255 }

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
        assert_eq!(parsed.window.snap_grid, WindowSettings::default().snap_grid);
        assert_eq!(parsed.desktop.layout.columns, 6);
        assert_eq!(parsed.desktop.layout.snap, GridSpec::default().snap);
        assert_eq!(parsed.theme.title_text, Theme::default().title_text);
        assert_eq!(parsed.desktop.spaces[0].items.len(), 1);
    }

    #[test]
    fn missing_version_is_migrated() {
        let mut parsed: Config = toml::from_str("").unwrap();
        assert_eq!(parsed.version, 0);
        parsed.migrate();
        assert_eq!(parsed.version, CURRENT_VERSION);
    }

    #[test]
    fn round_trips_through_toml() {
        let mut config = Config::new();
        config
            .desktop
            .add_space("Work", deskspace_core::Rect::new(10, 20, 300, 200));
        let text = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed.desktop.spaces.len(), 1);
        assert_eq!(parsed.desktop.spaces[0].title, "Work");
    }
}
