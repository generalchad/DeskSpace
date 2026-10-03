pub mod error;
pub mod input;
pub mod render;
pub mod shell;
pub mod ui;
pub mod win32;

pub use error::PlatformError;
pub use input::{Event, MouseButton, SettingsEvent, SpaceEvent, TrayEvent};
pub use render::{Icon, ItemState, ItemVisual, SpaceScene, SpaceStyle};
pub use shell::{DesktopEntry, IconCache, desktop_directory, enumerate_desktop_items};
pub use ui::{
    CursorShape, Field, FieldKind, FieldValue, MenuEntry, SettingsForm, SettingsPage, SliderMark,
};
pub use win32::Placement;
pub use win32::dpi::enable_per_monitor_awareness;
pub use win32::hotkey::{Hotkey, HotkeySpec, hotkey_id, parse as parse_hotkey};
pub use win32::shell_items::{DragEffect, DragResult, ItemMenuChoice};
pub use win32::system::{drag_threshold, system_uses_dark_mode, work_area_at};
pub use win32::{EventHandler, RunOptions, Ui, run};

pub type Result<T, E = PlatformError> = std::result::Result<T, E>;
