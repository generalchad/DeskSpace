use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("space {0} was not found")]
    SpaceNotFound(u64),
    #[error("item {0} was not found")]
    ItemNotFound(u64),
    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),
}

pub type Result<T, E = CoreError> = std::result::Result<T, E>;
