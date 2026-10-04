use crate::ui::FieldValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Input on a space window. Coordinates are desktop coordinates (the
/// coordinate system of `Space::rect`), not client coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpaceEvent {
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        x: i32,
        y: i32,
        button: MouseButton,
        /// Ctrl was held: add to or remove from the selection, as Explorer.
        ctrl: bool,
        /// Shift was held: extend the selection from the anchor.
        shift: bool,
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
        /// Shift was held at the drop: leave the files where they are.
        keep_in_place: bool,
    },
    /// Another window took the mouse capture mid-gesture (for example on
    /// Alt+Tab), so no matching `MouseUp` will arrive. Cancel any drag.
    CaptureLost,
}

/// A clipboard shortcut pressed while an item is selected or a space is
/// current (see `Ui::watch_selection_keys`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardAction {
    Copy,
    Cut,
    Paste,
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
    Space {
        id: u64,
        event: SpaceEvent,
    },
    Hotkey {
        id: i32,
    },
    Tray(TrayEvent),
    Settings(SettingsEvent),
    /// A key decoded while the app is capturing text for an inline rename (see
    /// `Ui::capture_text_input`).
    EditKey(crate::edit::EditKey),
    /// Delete was pressed while an item was selected (see
    /// `Ui::watch_selection_keys`).
    DeleteSelection,
    /// Ctrl+C, Ctrl+X, or Ctrl+V was pressed while an item was selected or a
    /// space was current.
    Clipboard(ClipboardAction),
    /// The clipboard changed, so any cut ghosting may no longer apply.
    ClipboardChanged,
    /// F2 was pressed while an item was selected.
    RenameSelection,
    /// Ctrl+A was pressed while a space is current: select all its items.
    SelectAll,
    /// A timer armed with `Ui::set_timer` elapsed. `id` is the caller's own
    /// timer id, so the app can tell timers apart.
    Timer {
        id: usize,
    },
    /// A mouse button went down outside every space (on the desktop, another
    /// app, or the taskbar). Only reported while `Ui::watch_outside_clicks`
    /// is on.
    PointerDownOutside {
        /// Whether the press landed on the desktop background, which clears the
        /// item selection. A press on another app keeps it, as Explorer does.
        on_desktop: bool,
    },
    /// The user switched Windows between light and dark mode.
    SystemThemeChanged,
    /// Delivered once, before the runtime tears windows down: on quit, console
    /// Ctrl+C or close, and at logoff/shutdown. Save here.
    CloseRequested,
}
