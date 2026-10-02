use deskspace_core::{Desktop, GridSpec, Theme};
use serde::{Deserialize, Serialize};

pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub desktop: Desktop,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub window: WindowSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
