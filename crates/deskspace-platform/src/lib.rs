pub mod edit;
pub mod error;
pub mod input;
pub mod render;
pub mod shell;
pub mod ui;
pub mod win32;

pub use edit::{EditKey, EditOutcome, MoveDir, TextEdit};
pub use error::PlatformError;
pub use input::{
    ClipboardAction, Event, MouseButton, SelectionKey, SettingsEvent, SpaceEvent, TrayEvent,
};
pub use render::{
    ITEM_ICON_SIZE, Icon, ItemMetrics, ItemState, ItemVisual, SpaceScene, SpaceStyle,
    item_check_box, item_check_box_hit,
};
pub use shell::{
    DesktopEntry, IconCache, desktop_directory, enumerate_desktop_items, notify_moved,
};
pub use ui::{
    CursorShape, Field, FieldKind, FieldValue, MenuEntry, SettingsForm, SettingsPage, SliderMark,
};
pub use win32::Placement;
pub use win32::desktop_entry::{set_desktop_show_entry, signal_show_spaces};
pub use win32::display_settings::{CheckBoxMode, ShellDisplay, desktop_display};
pub use win32::dpi::enable_per_monitor_awareness;
pub use win32::drag_image::DragImage;
pub use win32::hotkey::{Hotkey, HotkeySpec, hotkey_id, parse as parse_hotkey};
pub use win32::shell_items::{DragEffect, DragResult, ItemMenuChoice};
pub use win32::system::{drag_threshold, system_accent_color, system_uses_dark_mode, work_area_at};
pub use win32::write_icon;
pub use win32::{ClipboardFiles, EventHandler, RunOptions, Ui, run, space_dpi};

pub type Result<T, E = PlatformError> = std::result::Result<T, E>;
