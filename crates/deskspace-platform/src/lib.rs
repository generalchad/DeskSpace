pub mod error;
pub mod input;
pub mod render;
pub mod shell;
pub mod win32;

pub use error::PlatformError;
pub use input::{MouseButton, WindowEvent};
pub use render::{spaceVisual, ItemVisual, Painter, Scene};
pub use shell::{DesktopEntry, IconCache, enumerate_desktop_items};
pub use win32::Placement;
pub use win32::dpi::enable_per_monitor_awareness;
pub use win32::hotkey::{
    Hotkey, HotkeySpec, parse as parse_hotkey, register as register_hotkey,
    unregister as unregister_hotkey,
};
pub use win32::window::{DesktopWindow, EventHandler, WindowConfig, WindowContext, hotkey_id};

pub type Result<T, E = PlatformError> = std::result::Result<T, E>;
