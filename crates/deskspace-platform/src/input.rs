use crate::ui::FieldValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Input on a space window. Coordinates are desktop coordinates (the same
/// space as `space::rect`), not client coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum spaceEvent {
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    MouseUp {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    DoubleClick {
        x: i32,
        y: i32,
    },
    MouseLeave,
    /// Mouse wheel over the space. `delta` is in `WHEEL_DELTA` (120) units
    /// per notch; positive means the wheel was rotated away from the user.
    Wheel {
        x: i32,
        y: i32,
        delta: i32,
    },
    /// Files or folders dragged from Explorer or the desktop were dropped on
    /// the space.
    FilesDropped {
        x: i32,
        y: i32,
        paths: Vec<std::path::PathBuf>,
    },
    /// Another window took the mouse capture mid-gesture (for example on
    /// Alt+Tab), so no matching `MouseUp` will arrive. Cancel any drag.
    CaptureLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// Left click or keyboard selection of the tray icon.
    Activated,
    /// Right click; the coordinates are where the menu should appear.
    MenuRequested { x: i32, y: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsEvent {
    Changed {
        field: u32,
        value: FieldValue,
    },
    Button {
        field: u32,
    },
    Ok,
    /// Cancel button, Escape, or the window's close button.
    Cancel,
    Apply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The runtime is ready; create space windows now.
    Started,
    space {
        id: u64,
        event: spaceEvent,
    },
    Hotkey {
        id: i32,
    },
    Tray(TrayEvent),
    Settings(SettingsEvent),
    /// The user switched Windows between light and dark mode.
    SystemThemeChanged,
    /// Delivered once, before the runtime tears windows down: on quit, console
    /// Ctrl+C or close, and at logoff/shutdown. Save here.
    CloseRequested,
}
