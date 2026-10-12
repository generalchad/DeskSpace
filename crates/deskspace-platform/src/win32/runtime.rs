//! The UI-thread runtime: one `EventHandler` shared by every DeskSpace window
//! (spaces, the settings window, and the hidden controller that owns the tray
//! icon and hotkeys), plus the message loop.
//!
//! Re-entrancy: Win32 calls made by the handler (SetWindowPos, ShowWindow,
//! DestroyWindow, TrackPopupMenu, ChooseColor) pump or send messages that
//! re-enter window procedures synchronously. The handler is therefore moved
//! out of the runtime while it runs; events raised meanwhile are queued and
//! delivered by the outermost `dispatch` once the handler returns. Runtime
//! state is only ever borrowed for short, non-re-entrant sections
//! (`with_runtime`), never across a Win32 call that can send messages.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use deskspace_core::geometry::{Point, Rect};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::MapWindowPoints;
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::UI::Controls::{
    ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, ICC_TAB_CLASSES, INITCOMMONCONTROLSEX,
    InitCommonControlsEx,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, DispatchMessageW, GetMessageW, IsDialogMessageW, IsWindow, KillTimer, MSG,
    PostMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SetTimer, ShowWindow, TranslateMessage, WM_CLOSE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNO, MessageBoxW, SetForegroundWindow,
};
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::input::Event;
use crate::render::{SpaceScene, SpaceStyle, d2d};
use crate::ui::{CursorShape, MenuEntry, SettingsForm};
use crate::win32::clipboard::ClipboardFiles;
use crate::win32::drag_ghost::DragGhost;
use crate::win32::drag_image::DragImage;
use crate::win32::hotkey::{self, Hotkey, HotkeySpec};
use crate::win32::info_tip::{self, InfoTip};
use crate::win32::shell_items::{self, DragResult, ItemMenuChoice};
use crate::win32::system::wide;
use crate::win32::{
    clipboard, console, controller, dark_mode, desktop_icons, desktop_shell, drop_target, file_op,
    folder_watch, menu, outside_click, settings_window, shell_selection, shell_view, space_window,
    tray, watchdog,
};

pub trait EventHandler {
    fn on_event(&mut self, event: Event, ui: &mut Ui);
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub placement: desktop_shell::Placement,
    pub click_through: bool,
    /// Development aid: keep spaces above other windows so they can be
    /// inspected without uncovering the desktop.
    pub on_top: bool,
    pub tray_tooltip: String,
}

struct Runtime {
    /// `None` while `dispatch` has the handler checked out.
    handler: Option<Box<dyn EventHandler>>,
    pending: VecDeque<Event>,
    controller: HWND,
    host: Option<HWND>,
    click_through: bool,
    on_top: bool,
    spaces_visible: bool,
    spaces: HashMap<u64, HWND>,
    settings: Option<HWND>,
    /// The desktop layer window (ROADMAP D7), chrome-less and behind the
    /// spaces. Created only when the app asks for it (`--takeover-layer`).
    layer: Option<HWND>,
    /// Dark menus and settings window; set by the app (`Ui::set_ui_dark`).
    ui_dark: bool,
    /// The menu theme last flushed, so an unchanged mode skips the flush.
    menus_dark: Option<bool>,
    tray_tooltip: String,
    close_dispatched: bool,
    /// Created on the first in-space item drag and kept for reuse.
    drag_ghost: Option<DragGhost>,
    /// Created on the first item hover and reused for every info tip.
    info_tip: Option<InfoTip>,
    /// The folder under watch and its worker, when `Ui::watch_folder` is on.
    /// Dropped on teardown, which stops the worker.
    folder_watch: Option<(PathBuf, folder_watch::FolderWatch)>,
}

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

/// Runs `f` with a short-lived borrow of the runtime. `f` must not call
/// anything that can re-enter a window procedure.
fn with_runtime<R>(f: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    RUNTIME.with(|cell| match cell.try_borrow_mut() {
        Ok(mut slot) => slot.as_mut().map(f),
        Err(_) => {
            tracing::error!("UI runtime re-entered while borrowed; event dropped");
            None
        }
    })
}

/// Delivers `event` to the handler, or queues it if the handler is already
/// running further up the stack.
pub(crate) fn dispatch(event: Event) {
    let checked_out = with_runtime(|runtime| {
        runtime.pending.push_back(event);
        runtime.handler.take()
    })
    .flatten();
    let Some(mut handler) = checked_out else {
        return;
    };
    let mut ui = Ui {
        _ui_thread_only: PhantomData,
    };
    while let Some(event) = with_runtime(|runtime| runtime.pending.pop_front()).flatten() {
        handler.on_event(event, &mut ui);
    }
    with_runtime(|runtime| runtime.handler = Some(handler));
}

/// Sends `CloseRequested` the first time it is called; later calls (for
/// example WM_ENDSESSION followed by WM_DESTROY) are ignored.
pub(crate) fn dispatch_close_once() {
    let first = with_runtime(|runtime| !std::mem::replace(&mut runtime.close_dispatched, true))
        .unwrap_or(false);
    if first {
        dispatch(Event::CloseRequested);
    }
}

pub(crate) fn tray_tooltip() -> String {
    with_runtime(|runtime| runtime.tray_tooltip.clone()).unwrap_or_default()
}

/// Whether `window` is one of this runtime's space windows.
pub(crate) fn is_space_window(window: HWND) -> bool {
    with_runtime(|runtime| runtime.spaces.values().any(|space| *space == window)).unwrap_or(false)
}

/// The DPI scale factor of a space window (`1.0` at 96 DPI), for per-monitor
/// layout. Falls back to `1.0` before the window exists or outside the UI
/// thread.
pub fn space_dpi(id: u64) -> f32 {
    with_runtime(|runtime| {
        runtime
            .spaces
            .get(&id)
            .copied()
            .map(|hwnd| crate::win32::dpi::scale_for(crate::win32::dpi::window_dpi(hwnd)))
    })
    .flatten()
    .unwrap_or(1.0)
}

pub(crate) fn settings_closed(window: HWND) {
    with_runtime(|runtime| {
        if runtime.settings == Some(window) {
            runtime.settings = None;
        }
    });
}

/// Destroys every space window and the settings window. Called from the
/// controller's WM_DESTROY, after `CloseRequested` has been delivered.
pub(crate) fn destroy_ui_windows() {
    // Stop the folder watch first: its worker posts to the controller, which
    // could otherwise re-create the layer window while windows are tearing
    // down. Dropped outside the borrow, because stopping joins the worker.
    let watch = with_runtime(|runtime| runtime.folder_watch.take()).flatten();
    drop(watch);
    let (windows, ghost, info_tip) = with_runtime(|runtime| {
        let mut windows: Vec<HWND> = runtime.spaces.drain().map(|(_, hwnd)| hwnd).collect();
        windows.extend(runtime.settings.take());
        windows.extend(runtime.layer.take());
        (windows, runtime.drag_ghost.take(), runtime.info_tip.take())
    })
    .unwrap_or_default();
    // Dropped outside the borrow: destroying a window sends messages.
    drop(ghost);
    drop(info_tip);
    for hwnd in windows {
        // SAFETY: each HWND was created by this runtime on this thread.
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}

/// Handle given to `EventHandler::on_event` for driving the UI.
pub struct Ui {
    _ui_thread_only: PhantomData<*const ()>,
}

impl Ui {
    pub fn create_space(
        &mut self,
        id: u64,
        rect: Rect,
        style: SpaceStyle,
    ) -> Result<(), PlatformError> {
        let Some((host, click_through, on_top, visible, exists)) = with_runtime(|runtime| {
            (
                runtime.host,
                runtime.click_through,
                runtime.on_top,
                runtime.spaces_visible,
                runtime.spaces.contains_key(&id),
            )
        }) else {
            return Err(PlatformError::NotRunning);
        };
        if exists {
            return Ok(());
        }
        let options = space_window::SpaceWindowOptions {
            id,
            rect,
            style,
            host,
            click_through,
            on_top,
            visible,
            register_drop: true,
        };
        let hwnd = space_window::create(options)?;
        // A top-level space and the (top-level) desktop layer both pin to the
        // bottom of the z-order. Lower the layer again after the space exists
        // so the full-screen layer never ends up covering a space.
        if host.is_none()
            && let Some(layer) = with_runtime(|runtime| runtime.layer).flatten()
        {
            space_window::lower(layer);
        }
        with_runtime(|runtime| runtime.spaces.insert(id, hwnd));
        Ok(())
    }

    pub fn destroy_space(&mut self, id: u64) {
        if let Some(hwnd) = with_runtime(|runtime| runtime.spaces.remove(&id)).flatten() {
            // SAFETY: the window was created by this runtime.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }

    pub fn space_ids(&self) -> Vec<u64> {
        with_runtime(|runtime| runtime.spaces.keys().copied().collect()).unwrap_or_default()
    }

    pub fn set_space_rect(&mut self, id: u64, rect: Rect) {
        if let Some(hwnd) = space_hwnd(id) {
            space_window::set_rect(hwnd, rect);
        }
    }

    pub fn set_space_scene(&mut self, id: u64, scene: SpaceScene) {
        if let Some(hwnd) = space_hwnd(id) {
            space_window::set_scene(hwnd, scene);
        }
    }

    /// Sets the pointer shape over a space. Call it while handling a mouse
    /// event on that space; it lasts until changed or the mouse leaves.
    pub fn set_space_cursor(&mut self, id: u64, shape: CursorShape) {
        if let Some(hwnd) = space_hwnd(id) {
            space_window::set_cursor(hwnd, shape);
        }
    }

    pub fn set_space_style(&mut self, id: u64, style: SpaceStyle) {
        if let Some(hwnd) = space_hwnd(id) {
            space_window::set_style(hwnd, style);
        }
    }

    pub fn set_spaces_visible(&mut self, visible: bool) {
        let windows = with_runtime(|runtime| {
            runtime.spaces_visible = visible;
            runtime.spaces.values().copied().collect::<Vec<_>>()
        })
        .unwrap_or_default();
        let command = if visible { SW_SHOWNOACTIVATE } else { SW_HIDE };
        for hwnd in windows {
            // SAFETY: space windows are owned by this runtime.
            unsafe {
                let _ = ShowWindow(hwnd, command);
            }
        }
    }

    /// Creates the chrome-less desktop layer window spanning `rect`, reusing
    /// the space window and painter. Idempotent: calling it again does nothing.
    /// It is a `WS_CHILD` of the desktop host when one is in use and top-level
    /// otherwise, so it sits behind the spaces (`create_space` lowers it again
    /// after each top-level space, which shares the bottom of the z-order).
    pub fn create_desktop_layer(
        &mut self,
        rect: Rect,
        style: SpaceStyle,
    ) -> Result<(), PlatformError> {
        let Some((host, click_through, on_top, exists)) = with_runtime(|runtime| {
            (
                runtime.host,
                runtime.click_through,
                runtime.on_top,
                runtime.layer.is_some(),
            )
        }) else {
            return Err(PlatformError::NotRunning);
        };
        if exists {
            return Ok(());
        }
        let options = space_window::SpaceWindowOptions {
            id: 0,
            rect,
            style,
            host,
            click_through,
            on_top,
            visible: true,
            register_drop: false,
        };
        let hwnd = space_window::create(options)?;
        with_runtime(|runtime| runtime.layer = Some(hwnd));
        Ok(())
    }

    pub fn set_desktop_layer_rect(&mut self, rect: Rect) {
        if let Some(hwnd) = with_runtime(|runtime| runtime.layer).flatten() {
            space_window::set_rect(hwnd, rect);
        }
    }

    pub fn set_desktop_layer_scene(&mut self, scene: SpaceScene) {
        if let Some(hwnd) = with_runtime(|runtime| runtime.layer).flatten() {
            space_window::set_scene(hwnd, scene);
        }
    }

    pub fn destroy_desktop_layer(&mut self) {
        if let Some(hwnd) = with_runtime(|runtime| runtime.layer.take()).flatten() {
            // SAFETY: the layer window was created by this runtime.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }

    /// Reports `Event::PointerDownOutside` for presses outside every space
    /// while enabled. It watches every mouse event in the session, so keep
    /// it on only while something (such as a selection) depends on it.
    pub fn watch_outside_clicks(&mut self, enabled: bool) {
        if let Some(controller) = with_runtime(|runtime| runtime.controller) {
            outside_click::watch(controller, enabled);
        }
    }

    /// Watches `path` for changes and delivers `Event::FolderChanged` once per
    /// debounced burst. Idempotent for the path already watched; another path
    /// replaces the watch. Call `unwatch_folder` to stop.
    pub fn watch_folder(&mut self, path: &Path) {
        let Some(controller) = with_runtime(|runtime| runtime.controller) else {
            return;
        };
        let current = with_runtime(|runtime| {
            runtime
                .folder_watch
                .as_ref()
                .map(|(watched, _)| watched.clone())
        })
        .flatten();
        if current.as_deref() == Some(path) {
            return;
        }
        // Dropped outside the borrow: stopping joins the worker thread.
        let previous = with_runtime(|runtime| runtime.folder_watch.take()).flatten();
        drop(previous);
        match folder_watch::FolderWatch::start(path, controller, controller::FOLDER_CHANGE) {
            Ok(watch) => {
                with_runtime(|runtime| runtime.folder_watch = Some((path.to_path_buf(), watch)));
            }
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "could not watch the folder");
            }
        }
    }

    /// Stops the folder watch started with `watch_folder`.
    pub fn unwatch_folder(&mut self) {
        // Dropped outside the borrow: stopping joins the worker thread.
        let watch = with_runtime(|runtime| runtime.folder_watch.take()).flatten();
        drop(watch);
    }

    /// Arms a timer that delivers `Event::Timer` with `id` every `millis`
    /// milliseconds until it is cleared. Calling it again with the same id
    /// restarts it, which is how a caller debounces a burst of changes; clear
    /// it in the handler to stop it.
    pub fn set_timer(&mut self, id: usize, millis: u32) {
        if let Some(controller) = with_runtime(|runtime| runtime.controller) {
            // SAFETY: the controller is a live window owned by this thread.
            unsafe {
                SetTimer(Some(controller), id, millis, None);
            }
        }
    }

    /// Cancels a timer armed with `set_timer`.
    pub fn clear_timer(&mut self, id: usize) {
        if let Some(controller) = with_runtime(|runtime| runtime.controller) {
            // SAFETY: as in `set_timer`.
            unsafe {
                let _ = KillTimer(Some(controller), id);
            }
        }
    }

    /// Captures keystrokes (delivered as `Event::EditKey`) while the app is
    /// editing a space title inline. The keys are consumed, so they do not
    /// reach the focused app; call with `false` when editing ends.
    pub fn capture_text_input(&mut self, enabled: bool) {
        if let Some(controller) = with_runtime(|runtime| runtime.controller) {
            crate::win32::keyboard::capture(controller, enabled);
        }
    }

    /// Watches the selection keys while `selection` is true, and the keys that
    /// need a current space while `paste` is true, since spaces are never
    /// focused. Call with `false` when nothing applies; only the watched keys
    /// are consumed.
    pub fn watch_selection_keys(&mut self, selection: bool, paste: bool) {
        if let Some(controller) = with_runtime(|runtime| runtime.controller) {
            crate::win32::keyboard::watch_selection(controller, selection, paste);
        }
    }

    /// Opens the system color picker and returns the chosen color. Blocks in a
    /// modal loop; other events are queued meanwhile.
    pub fn pick_color(
        &mut self,
        initial: deskspace_core::theme::Color,
    ) -> Option<deskspace_core::theme::Color> {
        let owner = with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller))?;
        // SAFETY: `owner` is a live window owned by this thread, and the picker
        // is modal for the duration of the call.
        unsafe { settings_window::pick_color(owner, initial) }
    }

    /// Shows a popup menu at `at` (screen coordinates) and returns the chosen
    /// entry id. Blocks in a modal loop; other events are queued meanwhile.
    pub fn popup_menu(&mut self, entries: &[MenuEntry], at: Point) -> Option<u32> {
        let owner = with_runtime(|runtime| runtime.controller)?;
        menu::track(owner, entries, at)
    }

    /// Opens `path` with its default verb, as a double-click in Explorer does.
    pub fn open_path(&mut self, path: &Path) {
        let owner = with_runtime(|runtime| runtime.controller);
        match shell_items::open(owner, path) {
            Ok(()) => tracing::debug!(path = %path.display(), "opened item"),
            Err(error) => tracing::warn!(%error, path = %path.display(), "could not open item"),
        }
    }

    /// Shows the shell's Properties dialog for `path`, as Alt+Enter does in
    /// Explorer.
    pub fn show_properties(&mut self, path: &Path) {
        let owner = with_runtime(|runtime| runtime.controller);
        match shell_items::properties(owner, path) {
            Ok(()) => tracing::debug!(path = %path.display(), "showing item properties"),
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "could not show item properties")
            }
        }
    }

    /// Shows `entries` followed by the shell's context menu for `path`.
    /// Blocks in a modal loop like `popup_menu`. Entry ids must be below
    /// 0x8000.
    pub fn item_menu(
        &mut self,
        path: &Path,
        entries: &[MenuEntry],
        at: Point,
    ) -> Option<ItemMenuChoice> {
        let owner = with_runtime(|runtime| runtime.controller)?;
        shell_items::track_item_menu(owner, path, entries, at)
    }

    /// Drags `paths` out like Explorer does and blocks until it is dropped,
    /// showing `image` under the pointer (the shell's default otherwise).
    /// `positions` are the items' cells in desktop coordinates, in the same
    /// order, so a group keeps its arrangement when dropped on the desktop;
    /// pass none to let the target lay them out. Drops on spaces arrive
    /// afterwards as `SpaceEvent::FilesDropped`.
    pub fn drag_out(
        &mut self,
        paths: &[PathBuf],
        positions: &[Point],
        image: Option<&DragImage>,
    ) -> Option<DragResult> {
        let owner = with_runtime(|runtime| runtime.controller);
        let screen: Vec<Point> = positions.iter().copied().map(desktop_to_screen).collect();
        match shell_items::drag_out(owner, paths, &screen, image) {
            Ok(result) => Some(result),
            Err(error) => {
                tracing::warn!(%error, "could not start drag");
                None
            }
        }
    }

    /// Replaces the clipboard with `paths`, marked cut (moved on paste) or
    /// copied, using the shell's `CF_HDROP` format. Returns whether it worked.
    pub fn set_clipboard_files(&mut self, paths: &[PathBuf], cut: bool) -> bool {
        match clipboard::set_file_list(paths, cut) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, "could not write the clipboard");
                false
            }
        }
    }

    /// The files on the clipboard and whether they were cut, or `None` when it
    /// holds no files.
    pub fn clipboard_files(&mut self) -> Option<ClipboardFiles> {
        clipboard::file_list()
    }

    /// Empties the clipboard after a cut has been pasted.
    pub fn clear_clipboard(&mut self) {
        if let Err(error) = clipboard::clear() {
            tracing::warn!(%error, "could not clear the clipboard");
        }
    }

    /// Moves or copies `paths` into `destination` with the shell's file
    /// operation, returning where the files landed. Blocks while the shell
    /// shows its progress UI; other events are queued meanwhile.
    pub fn transfer_files(
        &mut self,
        paths: &[PathBuf],
        destination: &Path,
        move_items: bool,
    ) -> Option<Vec<PathBuf>> {
        let owner = with_runtime(|runtime| runtime.controller);
        match file_op::transfer(owner, paths, destination, move_items) {
            Ok(created) => Some(created),
            Err(error) => {
                tracing::warn!(%error, dest = %destination.display(), "file operation failed");
                None
            }
        }
    }

    /// Deletes `paths` with the shell's file operation. Unless `permanent` is
    /// set they go to the Recycle Bin (undoable, no prompt). Blocks while the
    /// shell works; other events are queued.
    pub fn delete_files(&mut self, paths: &[PathBuf], permanent: bool) -> bool {
        let owner = with_runtime(|runtime| runtime.controller);
        match file_op::delete(owner, paths, permanent) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, "delete failed");
                false
            }
        }
    }

    /// Renames `path` to `new_name` (a file name, not a path) with the shell's
    /// file operation, so the change is undoable. Blocks while the shell works.
    pub fn rename_path(&mut self, path: &Path, new_name: &str) -> bool {
        let owner = with_runtime(|runtime| runtime.controller);
        match file_op::rename(owner, path, new_name) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "rename failed");
                false
            }
        }
    }

    /// Shows `image` floating with its hotspot at `at` (desktop coordinates)
    /// while the app drags an item inside a space.
    pub fn show_drag_ghost(&mut self, image: &DragImage, at: Point) {
        // Checked out of the runtime: showing the window sends messages.
        let ghost = with_runtime(|runtime| runtime.drag_ghost.take()).flatten();
        let ghost = match ghost {
            Some(ghost) => Some(ghost),
            None => DragGhost::create()
                .map_err(|error| tracing::warn!(%error, "no drag ghost window"))
                .ok(),
        };
        if let Some(mut ghost) = ghost {
            ghost.show(image, desktop_to_screen(at));
            with_runtime(|runtime| runtime.drag_ghost = Some(ghost));
        }
    }

    /// Moves the image shown by `show_drag_ghost`.
    pub fn move_drag_ghost(&mut self, at: Point) {
        let at = desktop_to_screen(at);
        self.with_ghost(|ghost| ghost.move_to(at));
    }

    pub fn hide_drag_ghost(&mut self) {
        self.with_ghost(DragGhost::hide);
    }

    /// Shows Explorer's info tip for `path` near `at` (desktop coordinates).
    /// Reused across hovers; call `hide_info_tip` when the pointer leaves.
    pub fn show_info_tip(&mut self, path: &Path, at: Point) {
        let Some(owner) = with_runtime(|runtime| runtime.controller) else {
            return;
        };
        if with_runtime(|runtime| runtime.info_tip.is_none()).unwrap_or(false) {
            match InfoTip::create(owner) {
                Ok(tip) => {
                    with_runtime(|runtime| runtime.info_tip = Some(tip));
                }
                Err(error) => {
                    tracing::debug!(%error, "could not create the info tip window");
                    return;
                }
            }
        }
        let text = info_tip::describe(path);
        let at = desktop_to_screen(at);
        let point = POINT { x: at.x, y: at.y };
        self.with_info_tip(|tip| tip.show(&text, point));
    }

    /// Hides the info tip, if one is shown.
    pub fn hide_info_tip(&mut self) {
        self.with_info_tip(InfoTip::hide);
    }

    /// Makes a click on a space behave like a click on the desktop: the
    /// desktop becomes the active window (deactivating whatever was in front)
    /// and its icon selection clears. A selection inside an Explorer window is
    /// left alone, as native Windows would.
    pub fn on_desktop_click(&mut self) {
        shell_selection::on_desktop_click();
    }

    fn with_ghost(&mut self, f: impl FnOnce(&DragGhost)) {
        if let Some(ghost) = with_runtime(|runtime| runtime.drag_ghost.take()).flatten() {
            f(&ghost);
            with_runtime(|runtime| runtime.drag_ghost = Some(ghost));
        }
    }

    /// Runs `f` with the info tip checked out of the runtime, so the calls it
    /// makes (which send window messages) do not run under the borrow.
    fn with_info_tip(&mut self, f: impl FnOnce(&InfoTip)) {
        if let Some(tip) = with_runtime(|runtime| runtime.info_tip.take()).flatten() {
            f(&tip);
            with_runtime(|runtime| runtime.info_tip = Some(tip));
        }
    }

    /// Switches DeskSpace's own UI (popup menus, including the shell's item
    /// menus, and the settings window) between dark and light. The open
    /// settings window switches at once.
    pub fn set_ui_dark(&mut self, dark: bool) {
        let flush = with_runtime(|runtime| {
            runtime.ui_dark = dark;
            runtime.menus_dark.replace(dark) != Some(dark)
        });
        // Every appearance edit in the settings window re-asserts the mode;
        // flushing the menu themes each time would be wasted work.
        if flush == Some(true) {
            dark_mode::set_menus_dark(dark);
        }
        if let Some(settings) = settings_hwnd() {
            settings_window::set_dark(settings, dark);
        }
    }

    /// Opens the settings window, or brings it to the front and refreshes it.
    pub fn open_settings(&mut self, form: &SettingsForm) {
        if let Some(existing) = settings_hwnd() {
            warn_on_shape_change(existing, form);
            settings_window::bring_to_front(existing);
            return;
        }
        let dark = with_runtime(|runtime| runtime.ui_dark).unwrap_or(false);
        match settings_window::create(form, dark) {
            Ok(hwnd) => {
                with_runtime(|runtime| runtime.settings = Some(hwnd));
            }
            Err(error) => tracing::error!(%error, "could not open the settings window"),
        }
    }

    pub fn update_settings(&mut self, form: &SettingsForm) {
        if let Some(hwnd) = settings_hwnd() {
            warn_on_shape_change(hwnd, form);
        }
    }

    pub fn close_settings(&mut self) {
        if let Some(hwnd) = with_runtime(|runtime| runtime.settings.take()).flatten() {
            // SAFETY: the settings window was created by this runtime.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }

    /// Asks a yes/no question in a modal message box; "No" is the default.
    pub fn confirm(&mut self, title: &str, text: &str) -> bool {
        let owner = with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller));
        let title = wide(title);
        let text = wide(text);
        // SAFETY: both strings are NUL-terminated and outlive the modal call.
        let answer = unsafe {
            if let Some(owner) = owner {
                // A message box owned by a background window opens behind the
                // active app unless its owner is made foreground first.
                let _ = SetForegroundWindow(owner);
            }
            MessageBoxW(
                owner,
                PCWSTR(text.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
            )
        };
        answer == IDYES
    }

    pub fn is_settings_open(&self) -> bool {
        settings_hwnd().is_some()
    }

    pub fn register_hotkey(
        &mut self,
        index: i32,
        spec: HotkeySpec,
    ) -> Result<Hotkey, PlatformError> {
        let controller =
            with_runtime(|runtime| runtime.controller).ok_or(PlatformError::NotRunning)?;
        hotkey::register(controller, hotkey::hotkey_id(index), spec)
    }

    pub fn unregister_hotkey(&mut self, hotkey: Hotkey) {
        if let Some(controller) = with_runtime(|runtime| runtime.controller) {
            hotkey::unregister(controller, hotkey);
        }
    }

    pub fn set_tray_tooltip(&mut self, text: &str) {
        let controller = with_runtime(|runtime| {
            runtime.tray_tooltip = text.to_string();
            runtime.controller
        });
        if let Some(controller) = controller {
            tray::set_tooltip(controller, text);
        }
    }

    /// Ends the message loop after the current event. `CloseRequested` is
    /// delivered first.
    pub fn quit(&mut self) {
        if let Some(controller) = with_runtime(|runtime| runtime.controller) {
            // SAFETY: posting to our own live window.
            unsafe {
                let _ = PostMessageW(
                    Some(controller),
                    WM_CLOSE,
                    Default::default(),
                    Default::default(),
                );
            }
        }
    }
}

/// Desktop coordinates (`Space::rect`'s system) are the host window's client
/// coordinates for desktop-hosted spaces and screen coordinates otherwise.
fn desktop_to_screen(at: Point) -> Point {
    let Some(host) = with_runtime(|runtime| runtime.host).flatten() else {
        return at;
    };
    let mut point = [POINT { x: at.x, y: at.y }];
    // SAFETY: plain coordinate mapping from a live window to the screen.
    unsafe { MapWindowPoints(Some(host), None, &mut point) };
    Point::new(point[0].x, point[0].y)
}

fn space_hwnd(id: u64) -> Option<HWND> {
    with_runtime(|runtime| runtime.spaces.get(&id).copied()).flatten()
}

fn settings_hwnd() -> Option<HWND> {
    with_runtime(|runtime| runtime.settings).flatten()
}

/// Refreshes the settings window in place. A form whose page shape differs
/// cannot be diffed onto the existing controls, so the update is skipped. That
/// would leave the window showing stale values, so the invariant is asserted
/// in debug builds and logged in release (every page is always built, so a
/// shape change is a bug).
fn warn_on_shape_change(hwnd: HWND, form: &SettingsForm) {
    let updated = settings_window::update(hwnd, form);
    debug_assert!(updated, "the settings form must not change shape");
    if !updated {
        tracing::warn!("the settings form changed shape; the update was skipped");
    }
}

/// Creates the controller window, delivers `Event::Started`, and pumps
/// messages until `Ui::quit`, console close, or session end.
pub fn run(options: RunOptions, handler: Box<dyn EventHandler>) -> Result<(), PlatformError> {
    // COM is needed by WIC (icon conversion) and the shell; OLE on top of it
    // by drag and drop (SHDoDragDrop fails with CO_E_NOTINITIALIZED after
    // only CoInitializeEx). OleInitialize enters a single-threaded apartment,
    // which suits every COM object here: all are used on the UI thread.
    // https://learn.microsoft.com/en-us/windows/win32/api/ole2/nf-ole2-oleinitialize
    // SAFETY: called once on the UI thread, balanced below.
    let com = unsafe { OleInitialize(None) };
    init_common_controls();

    let host = desktop_shell::find_host(options.placement);
    if options.placement.is_desktop_hosted() && host.is_none() {
        tracing::warn!("no desktop host window found; spaces will be top-level windows");
    }

    let controller = controller::create()?;
    RUNTIME.with(|cell| {
        *cell.borrow_mut() = Some(Runtime {
            handler: Some(handler),
            pending: VecDeque::new(),
            controller,
            host,
            click_through: options.click_through,
            on_top: options.on_top,
            spaces_visible: true,
            spaces: HashMap::new(),
            settings: None,
            layer: None,
            ui_dark: false,
            menus_dark: None,
            tray_tooltip: options.tray_tooltip.clone(),
            close_dispatched: false,
            drag_ghost: None,
            info_tip: None,
            folder_watch: None,
        });
    });
    tray::add(controller, &options.tray_tooltip);
    console::install(controller);

    dispatch(Event::Started);
    let result = message_loop();
    console::message_loop_finished();

    // SAFETY: IsWindow guards against destroying an already-destroyed HWND. If
    // the loop failed, this still delivers CloseRequested so the app saves.
    unsafe {
        if IsWindow(Some(controller)).as_bool() {
            let _ = DestroyWindow(controller);
        }
    }
    let runtime = RUNTIME.with(|cell| cell.borrow_mut().take());
    drop(runtime);
    d2d::release_graphics();
    drop_target::release_image_helper();
    shell_view::release();
    // Takeover: show Explorer's icons again so a clean exit never leaves the
    // desktop blank. Idempotent, so this is harmless when takeover was off.
    desktop_icons::restore();
    // Tell the takeover watchdog this was a clean exit, so it does not restore
    // the icons again (it only acts when the process died without reaching
    // here).
    watchdog::signal_clean_exit();
    if com.is_ok() {
        // SAFETY: balances the successful OleInitialize above, after every
        // COM object owned by this thread has been released.
        unsafe { OleUninitialize() };
    }
    result
}

fn message_loop() -> Result<(), PlatformError> {
    let mut message = MSG::default();
    loop {
        // SAFETY: `message` is a valid out-pointer for the whole loop.
        let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
        // GetMessageW returns -1 on error, which `as_bool` would treat as
        // "keep going" and spin forever.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessagew
        match status.0 {
            0 => return Ok(()),
            -1 => return Err(PlatformError::Win32(windows::core::Error::from_thread())),
            _ => {}
        }
        // IsDialogMessage gives the settings window Tab/Shift+Tab focus
        // movement, Enter for OK and Escape for Cancel, like a dialog box.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-isdialogmessagew
        if let Some(settings) = settings_hwnd() {
            // SAFETY: `settings` is our live window and `message` is filled in.
            if unsafe { IsDialogMessageW(settings, &message) }.as_bool() {
                continue;
            }
        }
        // SAFETY: `message` was just filled in by GetMessageW.
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn init_common_controls() {
    let controls = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES | ICC_TAB_CLASSES | ICC_BAR_CLASSES,
    };
    // SAFETY: `controls` is fully initialised.
    if !unsafe { InitCommonControlsEx(&controls) }.as_bool() {
        tracing::warn!("InitCommonControlsEx failed; settings controls may be unavailable");
    }
}
