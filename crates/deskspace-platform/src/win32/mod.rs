mod console;
pub mod desktop_shell;
pub mod dpi;
pub mod hotkey;
pub mod window;

pub use desktop_shell::Placement;
pub use window::{DesktopWindow, EventHandler, WindowConfig, WindowContext};
