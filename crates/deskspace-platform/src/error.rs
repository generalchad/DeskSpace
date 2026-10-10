use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("windows API call failed: {0}")]
    Win32(#[from] windows::core::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("a registry operation failed (error {0})")]
    Registry(u32),
    #[error("window class registration failed")]
    ClassRegistration,
    #[error("the format of {0} is not valid")]
    InvalidFormat(&'static str),
    #[error("the UI runtime is not running on this thread")]
    NotRunning,
}
