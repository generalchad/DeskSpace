mod edit;
mod error;
mod input;
mod render;
mod ui;
mod win32;

pub use edit::{EditKey, EditOutcome, MoveDir, TextEdit};
pub use error::PlatformError;
pub use input::{
    ClipboardAction, DropAction, Event, MouseButton, NavDir, NavKey, SelectionKey, SettingsEvent,
    ShellItem, SpaceEvent, TrayEvent,
};
pub use render::{
    ITEM_ICON_SIZE, Icon, ItemMetrics, ItemState, ItemVisual, SpaceScene, SpaceStyle,
    item_check_box, item_check_box_hit,
};
pub use ui::{
    CursorShape, CustomShortcut, Field, FieldKind, FieldValue, MenuEntry, PreviewItem,
    SettingsButton, SettingsForm, SettingsHeader, SettingsNav, SettingsNavEntry, SettingsNavGroup,
    SettingsPage, SettingsScope, SettingsSwitch, ShortcutTarget, SliderMark, SpacePreview,
};
pub use win32::Placement;
pub use win32::autostart::set_start_with_windows;
pub use win32::desktop_entry::{set_desktop_show_entry, signal_show_spaces};
pub use win32::desktop_icons::{
    hide as hide_desktop_icons, rehide_if_active as rehide_desktop_icons,
    restore as restore_desktop_icons,
};
pub use win32::desktop_items::{
    DesktopEntry, desktop_directory, enumerate_desktop_items, item_facts, notify_moved,
};
pub use win32::display_settings::{
    CheckBoxMode, ShellDisplay, desktop_display, desktop_display_since,
};
pub use win32::dpi::enable_per_monitor_awareness;
pub use win32::drag_image::DragImage;
pub use win32::hotkey::{Hotkey, HotkeySpec, hotkey_id, parse as parse_hotkey};
pub use win32::icons::IconCache;
pub use win32::shell_items::{DragEffect, DragResult, ItemMenuChoice};
pub use win32::shortcuts::{create_custom_shortcut, create_shortcut};
pub use win32::system::{
    cursor_pos, drag_threshold, system_accent_color, system_uses_dark_mode, virtual_desktop_rect,
    work_area_at,
};
pub use win32::watchdog::{
    arm as arm_watchdog, is_watchdog, run_watchdog,
    signal_clean_exit as signal_watchdog_clean_exit, watchdog_pid,
};
pub use win32::{ClipboardFiles, EventHandler, RunOptions, Ui, Waker, run, space_dpi};

pub type Result<T, E = PlatformError> = std::result::Result<T, E>;
