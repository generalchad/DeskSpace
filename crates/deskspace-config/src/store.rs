use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::error::ConfigError;
use crate::schema::Config;

const QUALIFIER: &str = "";
const ORGANIZATION: &str = "DeskSpace";
const APPLICATION: &str = "DeskSpace";
const FILE_NAME: &str = "deskspace.toml";

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn discover() -> Result<Self, ConfigError> {
        let dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .ok_or(ConfigError::NoConfigDirectory)?;
        Ok(Self {
            path: dirs.config_dir().join(FILE_NAME),
        })
    }

    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    pub fn load(&self) -> Result<Config, ConfigError> {
        let text = std::fs::read_to_string(&self.path)
            .map_err(|source| ConfigError::io(self.path.clone(), source))?;
        let mut config: Config = toml::from_str(&text)?;
        config.migrate();
        Ok(config)
    }

    pub fn load_or_default(&self) -> Config {
        match self.load() {
            Ok(config) => config,
            Err(error) => {
                if self.exists() {
                    tracing::warn!(%error, path = %self.path.display(), "using default configuration");
                }
                Config::new()
            }
        }
    }

    pub fn save(&self, config: &Config) -> Result<(), ConfigError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ConfigError::io(parent, source))?;
        }
        let text = toml::to_string_pretty(config)?;
        let temp = self.path.with_extension("toml.tmp");
        std::fs::write(&temp, text.as_bytes())
            .map_err(|source| ConfigError::io(temp.clone(), source))?;
        std::fs::rename(&temp, &self.path)
            .map_err(|source| ConfigError::io(self.path.clone(), source))?;
        Ok(())
    }
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::discover().unwrap_or_else(|_| Self::with_path(FILE_NAME))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::with_path(dir.path().join("deskspace.toml"));
        let mut config = Config::new();
        config
            .desktop
            .add_space("F", deskspace_core::Rect::new(0, 0, 100, 100));
        store.save(&config).unwrap();
        assert!(store.exists());
        let loaded = store.load().unwrap();
        assert_eq!(loaded.desktop.spaces[0].title, "F");
    }
}
