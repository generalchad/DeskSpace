use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not determine a per-user configuration directory")]
    NoConfigDirectory,
    #[error("i/o error at {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("failed to parse configuration: {0}")]
    Parse(#[from] toml::de::Error),
}

impl ConfigError {
    pub fn io(path: impl Into<std::path::PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
