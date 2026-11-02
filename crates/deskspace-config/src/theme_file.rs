//! Theme files (ROADMAP F-58): a whole `SpaceLook` in one TOML file, so a
//! look can be saved, loaded, and shared without forking the configuration.
//!
//! The format is `format = 1`, a `name`, an optional `author`, and a `[look]`
//! table holding a `SpaceLook`. Missing fields default, out-of-range values
//! clamp on load, and unknown keys are ignored. A newer `format` still loads,
//! with a warning, as the configuration does.

use std::path::Path;

use deskspace_core::SpaceLook;
use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

/// The theme-file format this build writes.
pub const FORMAT: u32 = 1;

/// One shared look. `look` is the whole theme plus the grid spacing and
/// padding, the same unit a customized space stores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeFile {
    pub format: u32,
    pub name: String,
    pub author: Option<String>,
    pub look: SpaceLook,
}

impl Default for ThemeFile {
    fn default() -> Self {
        Self {
            format: FORMAT,
            name: String::new(),
            author: None,
            look: SpaceLook::default(),
        }
    }
}

impl ThemeFile {
    /// Reads and clamps the theme at `path`. A newer format loads with a
    /// warning rather than failing, so an older build still shows the look.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::io(path, source))?;
        let mut theme: Self = toml::from_str(&text)?;
        if theme.format > FORMAT {
            tracing::warn!(
                path = %path.display(),
                format = theme.format,
                supported = FORMAT,
                "theme file is newer than this build"
            );
        }
        theme.look = theme.look.clamped();
        Ok(theme)
    }

    /// Writes the theme at `path`, creating its folder.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ConfigError::io(parent, source))?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text).map_err(|source| ConfigError::io(path, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::layout::GAP_RANGE;

    fn theme_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("sunset.deskspace-theme.toml")
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = theme_path(&dir);
        let theme = ThemeFile {
            name: "Sunset".to_owned(),
            author: Some("me".to_owned()),
            ..ThemeFile::default()
        };
        theme.save(&path).unwrap();
        assert_eq!(ThemeFile::load(&path).unwrap(), theme);
    }

    #[test]
    fn missing_fields_default_and_unknown_keys_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = theme_path(&dir);
        std::fs::write(&path, "name = \"Bare\"\nextra = 3\n[look]\nextra = true\n").unwrap();
        let theme = ThemeFile::load(&path).unwrap();
        assert_eq!(theme.format, FORMAT);
        assert_eq!(theme.name, "Bare");
        assert_eq!(theme.author, None);
        assert_eq!(theme.look, SpaceLook::default());
    }

    #[test]
    fn out_of_range_values_clamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = theme_path(&dir);
        std::fs::write(&path, "[look]\ngap = 100000\npadding = -5\n").unwrap();
        let theme = ThemeFile::load(&path).unwrap();
        assert_eq!(theme.look.gap, GAP_RANGE.1);
        assert_eq!(theme.look.padding, GAP_RANGE.0);
    }

    #[test]
    fn a_newer_format_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = theme_path(&dir);
        std::fs::write(&path, "format = 99\nname = \"Future\"\n").unwrap();
        let theme = ThemeFile::load(&path).unwrap();
        assert_eq!(theme.format, 99);
        assert_eq!(theme.name, "Future");
    }
}
