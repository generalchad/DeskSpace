use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;

use crate::error::ConfigError;
use crate::schema::Config;

const QUALIFIER: &str = "";
const ORGANIZATION: &str = "DeskSpace";
const APPLICATION: &str = "DeskSpace";
const FILE_NAME: &str = "deskspace.toml";
const STORAGE_DIR: &str = "storage";

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
    storage: PathBuf,
}

impl ConfigStore {
    /// The per-user configuration in roaming AppData. Files moved into
    /// spaces go to local AppData instead, so they do not roam with the
    /// profile.
    pub fn discover() -> Result<Self, ConfigError> {
        let dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .ok_or(ConfigError::NoConfigDirectory)?;
        Ok(Self {
            path: dirs.config_dir().join(FILE_NAME),
            storage: dirs.data_local_dir().join(STORAGE_DIR),
        })
    }

    /// An explicit configuration file. Its space storage sits beside it, so
    /// a scratch configuration never touches the real storage folder.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let storage = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(STORAGE_DIR);
        Self { path, storage }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where files dropped into spaces are moved (ROADMAP D3).
    pub fn storage_dir(&self) -> &Path {
        &self.storage
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

    /// Loads the configuration, falling back to defaults when there is no file.
    ///
    /// A file that cannot be parsed, or that was written by a newer version, is
    /// copied aside first: the app saves on exit, and without the copy that save
    /// would silently replace the user's layout with defaults or drop unknown
    /// fields.
    pub fn load_or_default(&self) -> Config {
        if !self.exists() {
            return Config::new();
        }
        match self.load() {
            Ok(config) => {
                if config.is_from_newer_version() {
                    tracing::warn!(
                        version = config.version,
                        path = %self.path.display(),
                        "configuration was written by a newer DeskSpace"
                    );
                    self.backup_logged("newer");
                }
                config
            }
            Err(error) => {
                tracing::warn!(%error, path = %self.path.display(), "using default configuration");
                self.backup_logged("unreadable");
                Config::new()
            }
        }
    }

    /// Copies the current file to `<file>.<reason>-<unix millis>.bak` beside it.
    pub fn backup(&self, reason: &str) -> Result<PathBuf, ConfigError> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or_default();
        let file_name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| FILE_NAME.to_string());
        let target = self
            .path
            .with_file_name(format!("{file_name}.{reason}-{millis}.bak"));
        std::fs::copy(&self.path, &target)
            .map_err(|source| ConfigError::io(target.clone(), source))?;
        Ok(target)
    }

    fn backup_logged(&self, reason: &str) {
        match self.backup(reason) {
            Ok(target) => {
                tracing::warn!(backup = %target.display(), "previous configuration backed up")
            }
            Err(error) => tracing::error!(%error, "could not back up previous configuration"),
        }
    }

    /// `config` as the TOML text `save` writes.
    pub fn serialize(config: &Config) -> Result<String, ConfigError> {
        Ok(toml::to_string_pretty(config)?)
    }

    pub fn save(&self, config: &Config) -> Result<(), ConfigError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ConfigError::io(parent, source))?;
        }
        let text = Self::serialize(config)?;
        let temp = self.path.with_extension("toml.tmp");
        let mut file =
            std::fs::File::create(&temp).map_err(|source| ConfigError::io(temp.clone(), source))?;
        file.write_all(text.as_bytes())
            .map_err(|source| ConfigError::io(temp.clone(), source))?;
        // A rename can reach the disk before the data it points at, so without
        // the flush a power cut after a save could leave an empty config.
        file.sync_all()
            .map_err(|source| ConfigError::io(temp.clone(), source))?;
        drop(file);
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

    fn backups_in(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "bak"))
            .collect()
    }

    #[test]
    fn explicit_configs_keep_storage_beside_them() {
        let store = ConfigStore::with_path("C:/scratch/deskspace.toml");
        assert_eq!(store.storage_dir(), Path::new("C:/scratch/storage"));
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::with_path(dir.path().join("deskspace.toml"));
        let mut config = Config::new();
        let create = deskspace_core::Command::CreateSpace {
            title: "F".to_owned(),
            at: deskspace_core::Point::ZERO,
            size: deskspace_core::Size::new(100, 100),
            locked: false,
        };
        let bounds = deskspace_core::Rect::new(0, 0, 1920, 1040);
        assert!(config.desktop.apply(create, bounds).is_applied());
        store.save(&config).unwrap();
        assert!(store.exists());
        let loaded = store.load().unwrap();
        assert_eq!(loaded.desktop.spaces()[0].title(), "F");
    }

    /// A destination that cannot be replaced (a read-only file, as `attrib +r`
    /// leaves it) fails the rename: the old file stays and the temp is left for
    /// inspection.
    #[test]
    fn a_failed_rename_keeps_the_old_config_and_leaves_the_temp() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::with_path(dir.path().join("deskspace.toml"));
        store.save(&Config::new()).unwrap();
        let before = std::fs::read(store.path()).unwrap();

        let mut permissions = std::fs::metadata(store.path()).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(store.path(), permissions).unwrap();

        let mut config = Config::new();
        config.version = 99;
        assert!(store.save(&config).is_err());

        assert_eq!(std::fs::read(store.path()).unwrap(), before);
        assert!(store.load().is_ok());
        assert!(dir.path().join("deskspace.toml.tmp").is_file());
    }

    #[test]
    fn missing_file_loads_defaults_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::with_path(dir.path().join("deskspace.toml"));
        assert_eq!(store.load_or_default(), Config::new());
        assert!(backups_in(dir.path()).is_empty());
    }

    #[test]
    fn unreadable_file_is_backed_up_before_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deskspace.toml");
        std::fs::write(&path, "this is = = not toml").unwrap();
        let store = ConfigStore::with_path(&path);

        assert_eq!(store.load_or_default(), Config::new());

        let backups = backups_in(dir.path());
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&backups[0]).unwrap(),
            "this is = = not toml"
        );
    }

    #[test]
    fn newer_file_is_backed_up_but_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deskspace.toml");
        std::fs::write(&path, "version = 99\nfuture_field = true\n").unwrap();
        let store = ConfigStore::with_path(&path);

        assert_eq!(store.load_or_default().version, 99);
        assert_eq!(backups_in(dir.path()).len(), 1);
    }
}
