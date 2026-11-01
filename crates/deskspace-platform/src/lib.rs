mod debug_console;
mod edit;
mod error;
mod icon_queue;
mod input;
mod render;
mod ui;
mod win32;

pub use debug_console::format_line;
pub use deskspace_core::Glyph;
pub use edit::{EditKey, EditOutcome, MoveDir, TextEdit};
pub use error::PlatformError;
pub use input::{
    ClipboardAction, DebugEvent, DropAction, Event, FolderListing, MouseButton, NavDir, NavKey,
    SelectionKey, SettingsEvent, ShellItem, SpaceEvent, TrayEvent, WatchKey, ZoomAction,
};
pub use render::{
    ColumnAlign, Content, DetailsColumnVisual, DetailsView, DropInto, Geometry, GroupHeaderVisual,
    ITEM_ICON_SIZE, Icon, ItemLayout, ItemMetrics, ItemState, ItemVisual, Mask, Play, ShadowStyle,
    Side, SpaceScene, SpaceStyle, StatusBar, StatusButtonVisual, TitleIcon, Transition,
    TransitionFrame, Way, ZoomBar, item_check_box, item_check_box_hit,
};
pub use ui::{
    CursorShape, CustomShortcut, DebugControl, DebugControlKind, DebugPanel, DebugSection, Field,
    FieldKind, FieldValue, LogLevel, LogLine, MenuEntry, PreviewItem, SettingsButton, SettingsForm,
    SettingsHeader, SettingsNav, SettingsNavEntry, SettingsNavGroup, SettingsPage, SettingsScope,
    SettingsSwitch, ShortcutTarget, SliderMark, SpacePreview,
};
pub use win32::Placement;
pub use win32::autostart::set_start_with_windows;
pub use win32::debug_info::{
    HookState, MonitorRow, ProcessStats, TaskbarRow, WindowRow, foreground_window, monitors,
    process_stats,
};
pub use win32::desktop_entry::{set_desktop_show_entry, signal_show_spaces};
pub use win32::desktop_icons::{
    hide as hide_desktop_icons, rehide_if_active as rehide_desktop_icons,
    restore as restore_desktop_icons, take_explorer_rename,
};
pub use win32::desktop_items::{
    DesktopEntry, desktop_directory, enumerate_desktop_items, format_bytes, format_date, format_kb,
    item_facts, local_today, notify_moved, profile_directory, type_name,
};
pub use win32::display_settings::{
    CheckBoxMode, DESKTOP_ICON_RANGE, DESKTOP_ICON_STEP, ShellDisplay, desktop_display,
    desktop_display_since, next_desktop_icon_size, set_desktop_icon_size,
};
pub use win32::dpi::{enable_per_monitor_awareness, monitor_dpi_at, scale_for};
pub use win32::drag_image::DragImage;
pub use win32::file_op::OnCollision;
pub use win32::glyphs::GlyphCache;
pub use win32::hotkey::{Hotkey, HotkeySpec, hotkey_id, parse as parse_hotkey};
pub use win32::icons::{IconCache, IconUser};
pub use win32::names::{is_inside, same_name_ignoring_case, upper_name};
pub use win32::shell_items::{DragEffect, DragResult, ItemMenuChoice};
pub use win32::shortcuts::{create_custom_shortcut, create_shortcut, resolve_path};
pub use win32::system::{
    cursor_pos, drag_threshold, idle_time, system_accent_color, system_animations,
    system_uses_dark_mode, transparency_enabled, user_is_presenting, virtual_desktop_rect,
    windows_build, work_area_at,
};
pub use win32::watchdog::{
    arm as arm_watchdog, is_running as watchdog_running, is_watchdog, run_watchdog,
    signal_clean_exit as signal_watchdog_clean_exit, watchdog_pid,
};
pub use win32::{ClipboardFiles, EventHandler, RunOptions, Ui, Waker, alert, run, space_dpi};

pub type Result<T, E = PlatformError> = std::result::Result<T, E>;
