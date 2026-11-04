//! Theme files (ROADMAP F-58): a whole `SpaceLook` in one TOML file, so a
//! look can be saved, loaded, and shared without forking the configuration.
//!
//! The format is `format = 1`, a `name`, an optional `author`, and a `[look]`
//! table holding a `SpaceLook`. Missing fields default, out-of-range values
//! clamp on load, and unknown keys are ignored. A newer `format` still loads,
//! with a warning, as the configuration does.

use std::path::{Path, PathBuf};

use deskspace_core::SpaceLook;
use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

/// The theme-file format this build writes.
pub const FORMAT: u32 = 1;

/// The file name a theme is saved under: `sunset.deskspace-theme.toml`.
pub const FILE_SUFFIX: &str = ".deskspace-theme.toml";

/// The largest theme file `load` reads: 64 KiB. The `full` reference file is
/// 799 bytes (2026-11-03), so real files have a wide margin.
pub const MAX_THEME_BYTES: u64 = 64 * 1024;

/// The most theme files the Colors page lists.
pub const MAX_THEME_FILES: usize = 100;

/// The longest `name` or `author` `load` keeps, in characters.
const MAX_TEXT_CHARS: usize = 100;

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
    /// A file over [`MAX_THEME_BYTES`] is refused before it is parsed, and a
    /// long `name` or `author` is cut rather than rejected.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let size = std::fs::metadata(path)
            .map_err(|source| ConfigError::io(path, source))?
            .len();
        if size > MAX_THEME_BYTES {
            return Err(ConfigError::TooLarge {
                path: path.to_owned(),
                size,
                limit: MAX_THEME_BYTES,
            });
        }
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::io(path, source))?;
        let mut theme: Self = toml::from_str(&text).map_err(|error| ConfigError::Theme {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;
        if theme.format > FORMAT {
            tracing::warn!(
                path = %path.display(),
                format = theme.format,
                supported = FORMAT,
                "theme file is newer than this build"
            );
        }
        theme.name = cut_to_limit(&theme.name);
        theme.author = theme.author.as_deref().map(cut_to_limit);
        theme.look = theme.look.clamped();
        Ok(theme)
    }

    /// Writes the theme at `path`, creating its folder.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self).map_err(|error| ConfigError::Theme {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;
        crate::store::write_atomic(path, &text)
    }

    /// Every theme file directly in `dir`, sorted by name. A missing folder is
    /// empty, and a file that does not load is skipped with a warning, so one
    /// bad file never hides the others.
    pub fn list(dir: &Path) -> Vec<Self> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                is_theme_file(&path).then_some(path)
            })
            .collect();
        // Cap by file name before reading any, so a huge folder stays cheap.
        paths.sort_by_cached_key(|path| display_key(path));
        paths.truncate(MAX_THEME_FILES);
        let mut themes: Vec<Self> = paths
            .into_iter()
            .filter_map(|path| match Self::load(&path) {
                Ok(mut theme) => {
                    if theme.name.is_empty() {
                        theme.name = Self::name_from_path(&path);
                    }
                    Some(theme)
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        path = %path.display(),
                        "could not load a theme file"
                    );
                    None
                }
            })
            .collect();
        themes.sort_by_cached_key(|theme| theme.name.to_lowercase());
        themes
    }

    /// The name a saved theme takes from `path`: its file name without the
    /// theme suffix, or without `.toml`, or the whole name.
    pub fn name_from_path(path: &Path) -> String {
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            return String::new();
        };
        strip_ascii_suffix(file, FILE_SUFFIX)
            .or_else(|| strip_ascii_suffix(file, ".toml"))
            .unwrap_or(file)
            .to_owned()
    }

    /// `path` with the theme suffix, so a name typed without one still saves
    /// as a theme the Colors page lists. A trailing `.toml` is replaced; any
    /// other dot in the name stays.
    pub fn with_suffix(path: &Path) -> PathBuf {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return path.to_owned();
        };
        if strip_ascii_suffix(name, FILE_SUFFIX).is_some() {
            return path.to_owned();
        }
        let stem = strip_ascii_suffix(name, ".toml").unwrap_or(name);
        path.with_file_name(format!("{stem}{FILE_SUFFIX}"))
    }
}

/// Whether `path` is named like a theme file.
fn is_theme_file(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| strip_ascii_suffix(name, FILE_SUFFIX).is_some())
}

/// `text` without `suffix`, ignoring case, or `None`.
fn strip_ascii_suffix<'a>(text: &'a str, suffix: &str) -> Option<&'a str> {
    let end = text.len().checked_sub(suffix.len())?;
    text.get(end..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
        .then(|| &text[..end])
}

/// The key that sorts theme files by name without reading them.
fn display_key(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// `text` cut to [`MAX_TEXT_CHARS`] characters, never inside one.
fn cut_to_limit(text: &str) -> String {
    text.chars().take(MAX_TEXT_CHARS).collect()
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

    #[test]
    fn an_oversized_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = theme_path(&dir);
        let padding = " ".repeat(MAX_THEME_BYTES as usize);
        std::fs::write(&path, format!("name = \"Huge\"\n{padding}")).unwrap();

        let error = ThemeFile::load(&path).unwrap_err();
        match error {
            ConfigError::TooLarge { size, limit, .. } => {
                assert!(size > limit);
                assert_eq!(limit, MAX_THEME_BYTES);
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    #[test]
    fn a_long_name_and_author_are_cut() {
        let dir = tempfile::tempdir().unwrap();
        let path = theme_path(&dir);
        let long = "x".repeat(MAX_TEXT_CHARS + 50);
        std::fs::write(&path, format!("name = \"{long}\"\nauthor = \"{long}\"\n")).unwrap();

        let theme = ThemeFile::load(&path).unwrap();
        assert_eq!(theme.name, "x".repeat(MAX_TEXT_CHARS));
        assert_eq!(theme.author, Some("x".repeat(MAX_TEXT_CHARS)));
    }

    #[test]
    fn a_multibyte_name_is_cut_without_splitting_a_character() {
        let dir = tempfile::tempdir().unwrap();
        let path = theme_path(&dir);
        let long = "é".repeat(MAX_TEXT_CHARS + 50);
        std::fs::write(&path, format!("name = \"{long}\"\n")).unwrap();

        let theme = ThemeFile::load(&path).unwrap();
        assert_eq!(theme.name, "é".repeat(MAX_TEXT_CHARS));
    }

    #[test]
    fn a_listing_is_sorted_and_skips_what_does_not_load() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["Zebra", "apple"] {
            ThemeFile {
                name: name.to_owned(),
                ..ThemeFile::default()
            }
            .save(&theme_path(&dir).with_file_name(format!("{name}{FILE_SUFFIX}")))
            .unwrap();
        }
        std::fs::write(dir.path().join("broken.deskspace-theme.toml"), "not = [").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not a theme").unwrap();

        let themes = ThemeFile::list(dir.path());
        let names: Vec<&str> = themes.iter().map(|theme| theme.name.as_str()).collect();
        assert_eq!(names, ["apple", "Zebra"], "sorted without case");
    }

    #[test]
    fn a_missing_or_empty_folder_lists_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ThemeFile::list(&dir.path().join("missing")).is_empty());
        assert!(ThemeFile::list(dir.path()).is_empty());
    }

    #[test]
    fn a_nameless_theme_takes_its_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("sunset{FILE_SUFFIX}"));
        std::fs::write(&path, "[look]\ngap = 1\n").unwrap();

        let themes = ThemeFile::list(dir.path());
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "sunset");
    }

    #[test]
    fn a_name_comes_from_the_file_without_its_extension() {
        assert_eq!(
            ThemeFile::name_from_path(Path::new("C:/themes/sunset.deskspace-theme.toml")),
            "sunset"
        );
        assert_eq!(ThemeFile::name_from_path(Path::new("plain.toml")), "plain");
        assert_eq!(
            ThemeFile::name_from_path(Path::new("no-extension")),
            "no-extension"
        );
        assert_eq!(
            ThemeFile::name_from_path(Path::new("SUNSET.DESKSPACE-THEME.TOML")),
            "SUNSET"
        );
    }

    #[test]
    fn a_saved_name_gets_the_theme_suffix() {
        assert_eq!(
            ThemeFile::with_suffix(Path::new("C:/themes/sunset")),
            Path::new("C:/themes/sunset.deskspace-theme.toml")
        );
        assert_eq!(
            ThemeFile::with_suffix(Path::new("C:/themes/sunset.toml")),
            Path::new("C:/themes/sunset.deskspace-theme.toml")
        );
        assert_eq!(
            ThemeFile::with_suffix(Path::new("C:/themes/sunset.DESKSPACE-THEME.TOML")),
            Path::new("C:/themes/sunset.DESKSPACE-THEME.TOML")
        );
        assert_eq!(
            ThemeFile::with_suffix(Path::new("C:/themes/Sunset v1.2")),
            Path::new("C:/themes/Sunset v1.2.deskspace-theme.toml"),
            "a dot in the name is not an extension"
        );
    }

    #[test]
    fn a_multibyte_file_name_is_not_split() {
        let name = "ThèmesThèmesThèmesThèmes.txt";
        assert_eq!(ThemeFile::name_from_path(Path::new(name)), name);
    }

    #[test]
    fn save_creates_missing_folders() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/themes/sunset.deskspace-theme.toml");
        ThemeFile::default().save(&path).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn an_uppercase_suffix_is_listed_too() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("UPPER.DESKSPACE-THEME.TOML");
        ThemeFile {
            name: "Upper".to_owned(),
            ..ThemeFile::default()
        }
        .save(&path)
        .unwrap();

        let themes = ThemeFile::list(dir.path());
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "Upper");
    }
}
