use deskspace_core::SpaceId;
use deskspace_core::source::FolderEntry;

use crate::ui::{FieldValue, LogLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A direction for keyboard item navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDir {
    Left,
    Right,
    Up,
    Down,
}

/// A keyboard navigation key pressed while a space is current (see
/// `Ui::watch_selection_keys`). `extend` is Shift (extend the selection from
/// the anchor); `focus` is Ctrl without Shift (move the focus only, leaving
/// the selection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavKey {
    pub dir: NavDir,
    pub extend: bool,
    pub focus: bool,
}

/// A shell item dropped to become a shortcut: a Start-menu or taskbar app
/// (which often has no filesystem path at all), or any item dragged with Alt
/// held. A space creates a shortcut to it, as Explorer does when one is
/// dropped on a folder (ROADMAP F-50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellItem {
    /// Explorer's display name, used for the shortcut's file name.
    pub name: String,
    /// The item's filesystem path, or `None` for a virtual item such as an
    /// app in the Applications folder. A dropped `.lnk` or `.url` is copied
    /// rather than linked to, as Explorer does.
    pub path: Option<std::path::PathBuf>,
    /// The item's absolute shell ID list, as the raw bytes of an
    /// `ITEMIDLIST` (the `ILGetSize` bytes of the `ILCombine`'d IDs). Rebuilt
    /// into a PIDL by `create_shortcut`.
    pub id_list: Vec<u8>,
}

/// What a drop does with the dragged files. A left drop picks this from the
/// modifier keys; a right drop lets the user choose it from a menu (G5b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropAction {
    /// Move the files into the space storage (the default).
    Move,
    /// Leave the files where they are; the space only lists them. What Shift
    /// does on a left drop, and Explorer's "Show here without moving".
    KeepInPlace,
    /// Put a copy in the space storage and leave the original, copied through
    /// the shell's file operation.
    Copy,
}

/// Which folder a watch belongs to. The worker reports the folder's slot;
/// the runtime maps it back to this key before the app sees the event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchKey {
    /// The user's Desktop folder, watched for the desktop layer.
    Desktop,
    /// A lens's bound folder.
    Space(SpaceId),
}

/// What a watched folder held when its last change settled. The worker lists
/// the folder itself, so the app never does IO for a watch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderListing {
    /// The folder's entries, as the worker just read them.
    Listed(Vec<FolderEntry>),
    /// The folder is missing or cannot be read; the worker keeps retrying.
    Unavailable,
    /// The folder resolves to DeskSpace's storage folder or inside it, for
    /// example through a junction, so it is not watched. The worker retries
    /// it like a missing folder.
    Refused,
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
    /// `ctrl` is true while Ctrl is held (Ctrl+wheel changes the icon size).
    Wheel {
        x: i32,
        y: i32,
        delta: i32,
        ctrl: bool,
    },
    /// Files or folders dragged from Explorer or the desktop were dropped on
    /// the space.
    FilesDropped {
        x: i32,
        y: i32,
        paths: Vec<std::path::PathBuf>,
        /// What to do with the files: move them into the store, list them
        /// where they are (Shift), or copy them into the store.
        action: DropAction,
    },
    /// Virtual shell items (a Start-menu tile, a taskbar app) were dropped on
    /// the space. There is no file to move; the app writes a shortcut for each.
    ShellItemsDropped {
        x: i32,
        y: i32,
        items: Vec<ShellItem>,
    },
    /// Another window took the mouse capture mid-gesture (for example on
    /// Alt+Tab), so no matching `MouseUp` will arrive. Cancel any drag.
    CaptureLost,
    /// The space was moved to a monitor with a different DPI, so its grid and
    /// icon sizes must be reflowed. `dpi` is the new value (96 is 100 %).
    DpiChanged {
        dpi: u32,
    },
    /// The mouse's back button (`XBUTTON1`) was released over the space.
    BackButton,
}

/// A clipboard shortcut pressed while an item is selected or a space is
/// current (see `Ui::watch_selection_keys`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardAction {
    Copy,
    Cut,
    Paste,
}

/// A key that acts on the selection or the current space, watched by the
/// low-level keyboard hook (see `Ui::watch_selection_keys`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKey {
    /// Enter: open the focused item.
    Open,
    /// Escape: clear the selection.
    Clear,
    /// Shift+Delete: delete the focused items without the Recycle Bin.
    PermanentDelete,
    /// F5: re-read Explorer's display options and repaint the spaces.
    Refresh,
    /// Menu key or Shift+F10: open the focused item's context menu, or the
    /// space's when no item is selected.
    Menu,
    /// Home: scroll the current space to the top.
    Home,
    /// End: scroll the current space to the bottom.
    End,
    /// PageUp: scroll the current space up by one page.
    PageUp,
    /// PageDown: scroll the current space down by one page.
    PageDown,
    /// Ctrl+Plus (either Plus key): larger icons, as Ctrl+wheel up.
    ZoomIn,
    /// Ctrl+Minus (either Minus key): smaller icons, as Ctrl+wheel down.
    ZoomOut,
    /// Ctrl+0 (either 0 key): icons back to 100 %.
    ZoomReset,
    /// Ctrl+Z: undo the last action.
    Undo,
    /// Ctrl+Y: redo the last undone action.
    Redo,
    /// Ctrl+Shift+1 through 8: switch the current space to Explorer's view
    /// with that digit (`VIEW_MODES` in the settings tables). A digit with no
    /// view yet does nothing.
    View(u8),
    /// Backspace, Alt+Left, or Alt+Up: show the parent of the subfolder a
    /// lens is browsed into (D18). Sent only while there is one.
    Back,
}

/// A button on the zoom pop-up (ROADMAP F-57), which changes the icon size
/// as Ctrl+wheel does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomAction {
    /// Smaller icons, one Ctrl+wheel notch.
    Out,
    /// Larger icons, one Ctrl+wheel notch.
    In,
    /// Back to 100 %.
    Reset,
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
    /// The navigation rail moved to `entry`, an index into the flattened
    /// `SettingsNav` entries (`SettingsNav::page_of` gives its page).
    NavChanged {
        entry: usize,
    },
    /// The scopes pane moved to `scope`, an index into `SettingsNav::scopes`.
    ScopeChanged {
        scope: usize,
    },
    /// The header switch (`SettingsHeader::switch`) was turned on or off.
    HeaderSwitch {
        on: bool,
    },
    /// The header button (`SettingsHeader::button`) was pressed.
    HeaderButton,
    /// The X on item `index` of the space preview `field` was clicked.
    PreviewRemove {
        field: u32,
        index: usize,
    },
    /// The + tile of the space preview `field` was clicked. `x`, `y` is the
    /// screen point at the tile's bottom-left corner, where a menu can open.
    PreviewAdd {
        field: u32,
        x: i32,
        y: i32,
    },
    Ok,
    /// Cancel button, Escape, or the window's close button.
    Cancel,
    Apply,
}

/// Input from the debug window (ROADMAP D14). Each `id` is a
/// `DebugControl::id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugEvent {
    /// A `Button` was pressed.
    Pressed { id: u32 },
    /// A `Toggle` was turned on or off.
    Toggled { id: u32, on: bool },
    /// A `Choice` moved to option `index`.
    Chose { id: u32, index: usize },
    /// The console's Level box moved to `level`.
    ConsoleLevel { level: LogLevel },
    /// The window was closed; DeskSpace keeps running.
    Closed,
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
    /// Input from the debug window (`Ui::open_debug`).
    Debug(DebugEvent),
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
    /// F2 was pressed while an item is selected or a space is current: rename
    /// the focused item, or the space's title when no item is selected.
    RenameSelection,
    /// A `SelectionKey` was pressed (Enter to open the focused item, and so on).
    SelectionKey(SelectionKey),
    /// Alt+Enter was pressed while an item is selected or a space is current:
    /// show the focused item's Properties, or DeskSpace Settings when no item
    /// is selected.
    ShowProperties,
    /// Ctrl+A was pressed while a space is current: select all its items.
    SelectAll,
    /// A printable character was typed while a space is current (type-to-select,
    /// see `Ui::watch_selection_keys`). The key is consumed by the hook.
    Typed(char),
    /// An arrow key was pressed while a space is current: move the selection,
    /// extend it (Shift), or move the focus only (Ctrl).
    Navigate(NavKey),
    /// A timer armed with `Ui::set_timer` elapsed. `id` is the caller's own
    /// timer id, so the app can tell timers apart.
    Timer {
        id: usize,
    },
    /// A worker thread asked the UI thread to wake up, tagging the reason with
    /// its own `id` (see `Ui::waker`). Used to hand an async result back.
    Wake {
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
    /// Another launch asked this instance to show its spaces (the desktop
    /// right-click "Show spaces" entry, or a shortcut).
    ShowSpaces,
    /// A folder watched with `Ui::watch_folder` changed. The app re-reads the
    /// folder and reconciles what it draws. M2 watches the Desktop folder for
    /// the desktop layer; M4 reuses it for a lens's bound folder.
    FolderChanged {
        key: WatchKey,
        /// The folder the worker listed. A listing can wait in the queue
        /// while the key's watch moves to another folder (Change folder…),
        /// so the app checks it against the folder it shows now.
        folder: std::path::PathBuf,
        listing: FolderListing,
    },
    /// The user switched Windows between light and dark mode.
    SystemThemeChanged,
    /// DWM's color changed, which can mean a new accent color. It can come
    /// in bursts and without any visible change.
    SystemColorChanged,
    /// Windows' "Animation effects" switch changed (`system_animations`).
    SystemAnimationsChanged,
    /// A zoom pop-up button was clicked (see `Ui::show_zoom_bar`).
    Zoom(ZoomAction),
    /// The color picker's recent colors changed after a pick, most recent
    /// first (see `Ui::set_recent_colors`). Save them.
    RecentColors(Vec<deskspace_core::theme::Color>),
    /// Delivered once, before the runtime tears windows down: on quit, console
    /// Ctrl+C or close, and at logoff/shutdown. Save here.
    CloseRequested,
}
