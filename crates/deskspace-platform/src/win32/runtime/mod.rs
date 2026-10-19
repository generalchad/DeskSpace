//! The UI-thread runtime: one `EventHandler` shared by every DeskSpace window
//! (spaces, the settings window, and the hidden controller that owns the tray
//! icon and hotkeys), plus the message loop.
//!
//! Re-entrancy: Win32 calls made by the handler (SetWindowPos, ShowWindow,
//! DestroyWindow, TrackPopupMenu, the color picker) pump or send messages that
//! re-enter window procedures synchronously. The handler is therefore moved
//! out of the runtime while it runs; events raised meanwhile are queued and
//! delivered by the outermost `dispatch` once the handler returns. Runtime
//! state is only ever borrowed for short, non-re-entrant sections
//! (`with_runtime`), never across a Win32 call that can send messages.

mod ui_chrome;
mod ui_input;
mod ui_shell;
mod ui_spaces;

pub use ui_chrome::alert;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use deskspace_core::geometry::{Point, Rect};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, MapWindowPoints};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::UI::Controls::{
    ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, ICC_TAB_CLASSES, INITCOMMONCONTROLSEX,
    InitCommonControlsEx,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, DispatchMessageW, GetMessageW, IsDialogMessageW, IsWindow, KillTimer, MSG,
    PostMessageW, SetTimer, TranslateMessage, WM_CLOSE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_OK, MB_YESNO, MessageBoxW, SetForegroundWindow,
};
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::input::{Event, ShellItem};
use crate::render::{Play, SpaceScene, SpaceStyle, d2d};
use crate::ui::{CursorShape, DebugPanel, MenuEntry, SettingsForm};
use crate::win32::clipboard::ClipboardFiles;
use crate::win32::drag_ghost::DragGhost;
use crate::win32::drag_image::DragImage;
use crate::win32::hotkey::{self, Hotkey, HotkeySpec};
use crate::win32::info_tip::{self, InfoTip};
use crate::win32::shell_items::{self, DragResult, ItemMenuChoice};
use crate::win32::system::wide;
use crate::win32::{
    clipboard, composition, console, controller, dark_mode, debug_window, desktop_icons,
    desktop_shell, drop_target, file_op, folder_watch, menu, messages, outside_click,
    settings_window, shadow_window, shell_selection, shell_view, space_animation, space_window,
    tray, watchdog, zoom_bar,
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
    /// Top-level spaces get their blur through composition, as decided at
    /// startup (`composition::start`).
    composition: bool,
    spaces_visible: bool,
    spaces: HashMap<u64, HWND>,
    /// Windows of deleted spaces still playing their Delete animation
    /// (ROADMAP D12); each destroys itself at the end, and teardown destroys
    /// any left.
    dying: Vec<HWND>,
    settings: Option<HWND>,
    /// The debug window (`--debug`, ROADMAP D14), while it is open.
    debug: Option<HWND>,
    /// The desktop layer window (ROADMAP D7), chrome-less and behind the
    /// spaces. Created only when the app asks for it (`--takeover-layer`).
    layer: Option<HWND>,
    /// Dark menus and settings window; set by the app (`Ui::set_ui_dark`).
    ui_dark: bool,
    /// The menu theme last flushed, so an unchanged mode skips the flush.
    menus_dark: Option<bool>,
    tray_tooltip: String,
    close_dispatched: bool,
    /// Set by `Ui::quit_after` while the spaces play their way out before
    /// the teardown (ROADMAP D13).
    leaving: bool,
    /// Created on the first in-space item drag and kept for reuse.
    drag_ghost: Option<DragGhost>,
    /// Created on the first item hover and reused for every info tip.
    info_tip: Option<InfoTip>,
    /// The zoom pop-up (ROADMAP F-57), created when it is first shown.
    zoom_bar: Option<HWND>,
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

/// Posts the close that tears the runtime down (`Ui::quit`).
pub(crate) fn post_close() {
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

/// While DeskSpace is leaving (`Ui::quit_after`), closes once no space is
/// still playing its way out. Called whenever a space's animation ends.
pub(crate) fn space_left() {
    let Some((true, windows)) = with_runtime(|runtime| {
        (
            runtime.leaving,
            runtime.spaces.values().copied().collect::<Vec<_>>(),
        )
    }) else {
        return;
    };
    if !windows.into_iter().any(space_animation::is_leaving) {
        post_close();
    }
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

/// Whether `window` is one of this runtime's space windows or the desktop
/// layer: a surface that handles its own presses.
pub(crate) fn is_surface_window(window: HWND) -> bool {
    with_runtime(|runtime| {
        runtime.layer == Some(window) || runtime.spaces.values().any(|space| *space == window)
    })
    .unwrap_or(false)
}

/// The desktop layer's window, while it exists.
pub(crate) fn desktop_layer_window() -> Option<HWND> {
    with_runtime(|runtime| runtime.layer).flatten()
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

pub(crate) fn debug_closed(window: HWND) {
    with_runtime(|runtime| {
        if runtime.debug == Some(window) {
            runtime.debug = None;
        }
    });
}

/// Paints every space window again, the dying ones too: their composition
/// surfaces lost their content with the device that drew it.
pub(crate) fn invalidate_spaces() {
    let windows = with_runtime(|runtime| {
        let mut windows: Vec<HWND> = runtime.spaces.values().copied().collect();
        windows.extend(&runtime.dying);
        windows
    })
    .unwrap_or_default();
    for window in windows {
        // SAFETY: a live window of this runtime; invalidating sends nothing.
        unsafe {
            let _ = InvalidateRect(Some(window), None, false);
        }
    }
}

/// Forgets a dying window that has destroyed itself (`space_animation`).
pub(crate) fn forget_dying(window: HWND) {
    with_runtime(|runtime| runtime.dying.retain(|dying| *dying != window));
}

/// Destroys every space window, the settings window, and the debug window. Called from the
/// controller's WM_DESTROY, after `CloseRequested` has been delivered.
pub(crate) fn destroy_ui_windows() {
    // Stop the folder watch first: its worker posts to the controller, which
    // could otherwise re-create the layer window while windows are tearing
    // down. Dropped outside the borrow, because stopping joins the worker.
    let watch = with_runtime(|runtime| runtime.folder_watch.take()).flatten();
    drop(watch);
    let (windows, ghost, info_tip) = with_runtime(|runtime| {
        let mut windows: Vec<HWND> = runtime.spaces.drain().map(|(_, hwnd)| hwnd).collect();
        windows.append(&mut runtime.dying);
        windows.extend(runtime.settings.take());
        windows.extend(runtime.debug.take());
        windows.extend(runtime.layer.take());
        windows.extend(runtime.zoom_bar.take());
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

/// A `Send + Sync` handle that wakes the UI thread, delivering
/// `Event::Wake { id }`. Worker threads (the update check and download) hold one
/// to hand their result back. The controller is stored as an `isize` because
/// `HWND` is not `Send`.
#[derive(Clone)]
pub struct Waker {
    controller: isize,
}

impl Waker {
    /// Asks the UI thread to deliver `Event::Wake { id }`. A wake after the
    /// runtime is gone just fails silently.
    pub fn wake(&self, id: usize) {
        // SAFETY: posting to our own window; a post to a destroyed window just
        // fails and is ignored.
        unsafe {
            let _ = PostMessageW(
                Some(HWND(self.controller as *mut core::ffi::c_void)),
                messages::WAKE,
                WPARAM(id),
                LPARAM(0),
            );
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

fn debug_hwnd() -> Option<HWND> {
    with_runtime(|runtime| runtime.debug).flatten()
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

    let composition = composition::start(host.is_some(), options.click_through);
    let controller = controller::create()?;
    RUNTIME.with(|cell| {
        *cell.borrow_mut() = Some(Runtime {
            handler: Some(handler),
            pending: VecDeque::new(),
            controller,
            host,
            click_through: options.click_through,
            on_top: options.on_top,
            composition,
            spaces_visible: true,
            spaces: HashMap::new(),
            dying: Vec::new(),
            settings: None,
            debug: None,
            layer: None,
            ui_dark: false,
            menus_dark: None,
            tray_tooltip: options.tray_tooltip.clone(),
            close_dispatched: false,
            leaving: false,
            drag_ghost: None,
            info_tip: None,
            zoom_bar: None,
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
    composition::release();
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
        // IsDialogMessage gives the settings and debug windows Tab/Shift+Tab
        // focus movement, and the settings window Enter for OK and Escape
        // for Cancel, like a dialog box.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-isdialogmessagew
        let is_dialog_message = |dialog: HWND| {
            // SAFETY: `dialog` is our live window and `message` is filled in.
            unsafe { IsDialogMessageW(dialog, &message) }.as_bool()
        };
        if [settings_hwnd(), debug_hwnd()]
            .into_iter()
            .flatten()
            .any(is_dialog_message)
        {
            continue;
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
