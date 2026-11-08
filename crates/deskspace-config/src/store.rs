use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;

use crate::error::ConfigError;
use crate::schema::Config;

const QUALIFIER: &str = "";
const ORGANIZATION: &str = "DeskSpace";
const APPLICATION: &str = "DeskSpace";
const FILE_NAME: &str = "deskspace.toml";
const STORAGE_DIR: &str = "storage";
const ICONS_DIR: &str = "icons";

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
    storage: PathBuf,
    icons: PathBuf,
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
            icons: dirs.data_local_dir().join(ICONS_DIR),
        })
    }

    /// An explicit configuration file. Its space storage and icon cache sit
    /// beside it, so a scratch configuration never touches the real ones.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let storage = parent.join(STORAGE_DIR);
        let icons = parent.join(ICONS_DIR);
        Self {
            path,
            storage,
            icons,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where files dropped into spaces are moved (ROADMAP D3).
    pub fn storage_dir(&self) -> &Path {
        &self.storage
    }

    /// Where downloaded icon sets are cached (`IconStore`). Local AppData, as
    /// the storage is: a cache does not roam.
    pub fn icons_dir(&self) -> &Path {
        &self.icons
    }

    /// Where theme files live: a `themes` folder beside the config, so a
    /// scratch `--config` has its own and a real one keeps the user's.
    pub fn themes_dir(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("themes")
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    pub fn load(&self) -> Result<Config, ConfigError> {
        let bytes = std::fs::read(&self.path)
            .map_err(|source| ConfigError::io(self.path.clone(), source))?;
        self.parse(bytes)
    }

    fn parse(&self, bytes: Vec<u8>) -> Result<Config, ConfigError> {
        let text = String::from_utf8(bytes).map_err(|error| {
            ConfigError::io(
                self.path.clone(),
                std::io::Error::new(std::io::ErrorKind::InvalidData, error),
            )
        })?;
        let mut config: Config = toml::from_str(&text)?;
        config.migrate();
        Ok(config)
    }

    /// Loads the configuration, falling back to defaults when there is no file.
    ///
    /// A file that cannot be parsed, or that was written by a newer version, is
    /// copied aside first: the app saves on exit, and without the copy that save
    /// would silently replace the user's layout with defaults or drop unknown
    /// fields. A file that exists but cannot be read, or an unparsable one whose
    /// copy fails, is an error: starting on defaults would let the next save
    /// replace the only copy of the user's layout.
    pub fn load_or_default(&self) -> Result<Config, ConfigError> {
        if !self.exists() {
            return Ok(Config::new());
        }
        let bytes = read_with_retry(&self.path)
            .map_err(|source| ConfigError::io(self.path.clone(), source))?;
        match self.parse(bytes) {
            Ok(config) => {
                if config.is_from_newer_version() {
                    tracing::warn!(
                        version = config.version,
                        path = %self.path.display(),
                        "configuration was written by a newer DeskSpace"
                    );
                    self.backup_logged("newer");
                }
                Ok(config)
            }
            Err(reason) => match self.backup("unreadable") {
                Ok(backup) => {
                    tracing::warn!(
                        %reason,
                        backup = %backup.display(),
                        "configuration could not be parsed; backed it up and using defaults"
                    );
                    Ok(Config::new())
                }
                Err(backup) => Err(ConfigError::NotBackedUp {
                    path: self.path.clone(),
                    reason: Box::new(reason),
                    backup: Box::new(backup),
                }),
            },
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
        let text = Self::serialize(config)?;
        write_atomic(&self.path, &text)
    }
}

/// How many times a load reads a file that exists, and how long it waits
/// between tries. Another program holding the file open, or a roaming profile
/// still syncing it, usually lets go within about a second.
const READ_ATTEMPTS: u32 = 5;
const READ_RETRY_DELAY: Duration = Duration::from_millis(250);

fn read_with_retry(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut attempt = 1;
    loop {
        match std::fs::read(path) {
            Ok(bytes) => return Ok(bytes),
            Err(error) if attempt >= READ_ATTEMPTS => return Err(error),
            Err(error) => {
                tracing::debug!(%error, attempt, path = %path.display(), "configuration not readable yet");
                attempt += 1;
                std::thread::sleep(READ_RETRY_DELAY);
            }
        }
    }
}

/// Writes `text` to `path` through a sibling temp file and a rename, creating
/// the folder. A rename can reach the disk before the data it points at, so
/// without the flush a power cut after a save could leave an empty file.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ConfigError::io(parent, source))?;
    }
    let mut temp_name = path.file_name().map(OsString::from).unwrap_or_default();
    temp_name.push(".tmp");
    let temp = path.with_file_name(temp_name);
    let mut file =
        std::fs::File::create(&temp).map_err(|source| ConfigError::io(temp.clone(), source))?;
    file.write_all(text.as_bytes())
        .map_err(|source| ConfigError::io(temp.clone(), source))?;
    file.sync_all()
        .map_err(|source| ConfigError::io(temp.clone(), source))?;
    drop(file);
    std::fs::rename(&temp, path).map_err(|source| ConfigError::io(path, source))?;
    Ok(())
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
    fn explicit_configs_keep_the_icon_cache_beside_them() {
        let store = ConfigStore::with_path("C:/scratch/deskspace.toml");
        assert_eq!(store.icons_dir(), Path::new("C:/scratch/icons"));
    }

    #[test]
    fn explicit_configs_keep_their_themes_beside_them() {
        let store = ConfigStore::with_path("C:/scratch/deskspace.toml");
        assert_eq!(store.themes_dir(), Path::new("C:/scratch/themes"));
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
            icon: deskspace_core::SpaceIcon::None,
            view: deskspace_core::SpaceView::default(),
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
        assert_eq!(store.load_or_default().unwrap(), Config::new());
        assert!(backups_in(dir.path()).is_empty());
    }

    /// Another program holding the file open with no sharing, as a sync
    /// client or an editor can, must not start DeskSpace on defaults.
    #[test]
    fn a_file_held_open_is_an_error_and_stays_unchanged() {
        use std::os::windows::fs::OpenOptionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deskspace.toml");
        let store = ConfigStore::with_path(&path);
        store.save(&Config::new()).unwrap();
        let before = std::fs::read(&path).unwrap();

        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let error = store.load_or_default().unwrap_err();
        drop(held);

        assert!(matches!(error, ConfigError::Io { ref path, .. } if path == store.path()));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(backups_in(dir.path()).is_empty());
    }

    /// The seam for a failing backup: a file name long enough that the
    /// backup's name, which adds `.unreadable-<millis>.bak`, passes the
    /// 255-character limit on one path component, while the file itself
    /// fits.
    #[test]
    fn an_unparsable_file_whose_backup_fails_is_an_error_and_stays_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("{}.toml", "d".repeat(240)));
        std::fs::write(&path, "this is = = not toml").unwrap();
        let store = ConfigStore::with_path(&path);

        let error = store.load_or_default().unwrap_err();

        assert!(matches!(error, ConfigError::NotBackedUp { .. }), "{error}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "this is = = not toml"
        );
        assert!(backups_in(dir.path()).is_empty());
    }

    #[test]
    fn a_file_that_is_not_text_is_backed_up_before_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deskspace.toml");
        std::fs::write(&path, [0xFF, 0xFE, 0x00]).unwrap();
        let store = ConfigStore::with_path(&path);

        assert_eq!(store.load_or_default().unwrap(), Config::new());
        assert_eq!(backups_in(dir.path()).len(), 1);
    }

    #[test]
    fn unreadable_file_is_backed_up_before_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deskspace.toml");
        std::fs::write(&path, "this is = = not toml").unwrap();
        let store = ConfigStore::with_path(&path);

        assert_eq!(store.load_or_default().unwrap(), Config::new());

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

        assert_eq!(store.load_or_default().unwrap().version, 99);
        assert_eq!(backups_in(dir.path()).len(), 1);
    }
}
