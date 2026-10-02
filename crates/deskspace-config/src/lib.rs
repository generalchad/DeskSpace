#![forbid(unsafe_code)]

pub mod error;
pub mod schema;
pub mod store;

pub use error::ConfigError;
pub use schema::{CURRENT_VERSION, Config, WindowSettings};
pub use store::ConfigStore;

pub type Result<T, E = ConfigError> = std::result::Result<T, E>;
