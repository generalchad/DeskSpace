pub mod error;
pub mod input;
pub mod render;
pub mod shell;
pub mod ui;
pub mod win32;

pub use error::PlatformError;
pub use input::{Event, spaceEvent, MouseButton, SettingsEvent, TrayEvent};
pub use render::{spaceScene, spaceStyle, Icon, ItemState, ItemVisual};
pub use shell::{DesktopEntry, IconCache, enumerate_desktop_items};
pub use ui::{CursorShape, Field, FieldKind, FieldValue, MenuEntry, SettingsForm, SettingsPage};
pub use win32::Placement;
pub use win32::dpi::enable_per_monitor_awareness;
pub use win32::hotkey::{Hotkey, HotkeySpec, hotkey_id, parse as parse_hotkey};
pub use win32::system::{system_uses_dark_mode, work_area_at};
pub use win32::{EventHandler, RunOptions, Ui, run};

pub type Result<T, E = PlatformError> = std::result::Result<T, E>;
