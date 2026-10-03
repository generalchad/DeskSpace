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

use deskspace_core::geometry::{Point, Rect};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Controls::{
    ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, ICC_TAB_CLASSES, INITCOMMONCONTROLSEX,
    InitCommonControlsEx,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, DispatchMessageW, GetMessageW, IsDialogMessageW, IsWindow, MSG, PostMessageW,
    SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow, TranslateMessage, WM_CLOSE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNO, MessageBoxW, SetForegroundWindow,
};
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::input::Event;
use crate::render::{spaceScene, spaceStyle, d2d};
use crate::ui::{CursorShape, MenuEntry, SettingsForm};
use crate::win32::hotkey::{self, Hotkey, HotkeySpec};
use crate::win32::system::wide;
use crate::win32::{console, controller, desktop_shell, space_window, menu, settings_window, tray};

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
    tray_tooltip: String,
    close_dispatched: bool,
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
    let windows = with_runtime(|runtime| {
        let mut windows: Vec<HWND> = runtime.spaces.drain().map(|(_, hwnd)| hwnd).collect();
        windows.extend(runtime.settings.take());
        windows
    })
    .unwrap_or_default();
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
        style: spaceStyle,
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
        let options = space_window::spaceWindowOptions {
            id,
            rect,
            style,
            host,
            click_through,
            on_top,
            visible,
        };
        let hwnd = space_window::create(options)?;
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

    pub fn set_space_scene(&mut self, id: u64, scene: spaceScene) {
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

    pub fn set_space_style(&mut self, id: u64, style: spaceStyle) {
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

    /// Shows a popup menu at `at` (screen coordinates) and returns the chosen
    /// entry id. Blocks in a modal loop; other events are queued meanwhile.
    pub fn popup_menu(&mut self, entries: &[MenuEntry], at: Point) -> Option<u32> {
        let owner = with_runtime(|runtime| runtime.controller)?;
        menu::track(owner, entries, at)
    }

    /// Opens the settings window, or brings it to the front and refreshes it.
    pub fn open_settings(&mut self, form: &SettingsForm) {
        if let Some(existing) = settings_hwnd() {
            settings_window::update(existing, form);
            settings_window::bring_to_front(existing);
            return;
        }
        match settings_window::create(form) {
            Ok(hwnd) => {
                with_runtime(|runtime| runtime.settings = Some(hwnd));
            }
            Err(error) => tracing::error!(%error, "could not open the settings window"),
        }
    }

    pub fn update_settings(&mut self, form: &SettingsForm) {
        if let Some(hwnd) = settings_hwnd() {
            settings_window::update(hwnd, form);
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

fn space_hwnd(id: u64) -> Option<HWND> {
    with_runtime(|runtime| runtime.spaces.get(&id).copied()).flatten()
}

fn settings_hwnd() -> Option<HWND> {
    with_runtime(|runtime| runtime.settings).flatten()
}

/// Creates the controller window, delivers `Event::Started`, and pumps
/// messages until `Ui::quit`, console close, or session end.
pub fn run(options: RunOptions, handler: Box<dyn EventHandler>) -> Result<(), PlatformError> {
    // COM is needed by WIC (icon conversion) and the shell. Apartment-threaded
    // because every COM object here is used only on the UI thread.
    // SAFETY: called once on the UI thread, balanced below.
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
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
            tray_tooltip: options.tray_tooltip.clone(),
            close_dispatched: false,
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
    if com.is_ok() {
        // SAFETY: balances the successful CoInitializeEx above, after every
        // COM object owned by this thread has been released.
        unsafe { CoUninitialize() };
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
