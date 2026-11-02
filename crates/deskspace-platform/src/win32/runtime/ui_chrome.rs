//! Ui methods for DeskSpace's own chrome: the color picker, popup
//! menus, dark mode, the settings and debug windows, the zoom pop-up, the
//! tray tooltip, and quitting.

use std::time::Duration;

use windows::Win32::UI::WindowsAndMessaging::{KillTimer, MB_SETFOREGROUND, MESSAGEBOX_STYLE};

use super::*;
use crate::input::ShellItem;
use crate::render::ZoomBar;
use crate::ui::{CustomShortcut, LogLine};
use crate::win32::debug_info::WindowRow;

/// Tells the user something went wrong when there is no `Ui` to ask, such as
/// after the message loop has ended. Blocks until the box is dismissed.
pub fn alert(title: &str, text: &str) {
    // With no window of its own, the box would open behind the active app.
    warning_box(None, title, text, MB_SETFOREGROUND);
}

fn warning_box(owner: Option<HWND>, title: &str, text: &str, extra: MESSAGEBOX_STYLE) {
    let title = wide(title);
    let text = wide(text);
    // SAFETY: both strings are NUL-terminated and outlive the modal call.
    unsafe {
        MessageBoxW(
            owner,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONWARNING | extra,
        );
    }
}

impl Ui {
    /// Opens DeskSpace's color picker and returns the chosen color. Blocks in a
    /// modal loop; other events are queued meanwhile.
    pub fn pick_color(
        &mut self,
        initial: deskspace_core::theme::Color,
    ) -> Option<deskspace_core::theme::Color> {
        let (owner, dark) = with_runtime(|runtime| {
            (
                runtime.settings.unwrap_or(runtime.controller),
                runtime.ui_dark,
            )
        })?;
        pick_color(owner, initial, dark)
    }

    /// Sets the color picker's recent colors, most recent first, such as the
    /// ones saved last session. Picks update them and raise
    /// `Event::RecentColors`.
    pub fn set_recent_colors(&mut self, colors: Vec<deskspace_core::theme::Color>) {
        with_runtime(|runtime| runtime.recent_colors = colors);
    }

    /// Opens the installed-apps picker titled `title` and returns the apps the
    /// user checked, or an empty list when they cancelled. Blocks in a modal
    /// loop; other events are queued meanwhile.
    pub fn pick_apps(&mut self, title: &str) -> Vec<ShellItem> {
        let Some((owner, dark)) = with_runtime(|runtime| {
            (
                runtime.settings.unwrap_or(runtime.controller),
                runtime.ui_dark,
            )
        }) else {
            return Vec::new();
        };
        // SAFETY: `owner` is a live window owned by this thread, and the
        // picker is modal for the duration of the call.
        unsafe { crate::win32::app_picker::pick_apps(owner, title, dark) }
    }

    /// Lists the installed apps and renders their icons on a background
    /// thread, so the installed-apps picker opens at once later. Call once at
    /// startup; the picker refreshes the list itself after each use.
    pub fn prefetch_apps(&mut self) {
        crate::win32::app_catalog::refresh_in_background();
    }

    /// Opens the "Custom shortcut" dialog titled `title` and returns the
    /// shortcut the user described, or `None` when they cancelled. A path
    /// target is already resolved to something that exists. Blocks in a modal
    /// loop; other events are queued meanwhile.
    pub fn custom_shortcut(&mut self, title: &str) -> Option<CustomShortcut> {
        let (owner, dark) = with_runtime(|runtime| {
            (
                runtime.settings.unwrap_or(runtime.controller),
                runtime.ui_dark,
            )
        })?;
        // SAFETY: `owner` is a live window owned by this thread, and the
        // dialog is modal for the duration of the call.
        unsafe { crate::win32::shortcut_dialog::custom_shortcut(owner, title, dark) }
    }

    /// Opens the standard file Open dialog titled `title` and returns the files
    /// the user picked, or an empty list when they cancelled. Multi-select is
    /// on and the dialog starts on the Desktop folder; `filter` lists (name,
    /// pattern) pairs and selects the first. Blocks in a modal loop; other
    /// events are queued meanwhile.
    pub fn pick_files(&mut self, title: &str, filter: &[(&str, &str)]) -> Vec<std::path::PathBuf> {
        let Some(owner) = with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller))
        else {
            return Vec::new();
        };
        // SAFETY: `owner` is a live window owned by this thread, and the dialog
        // is modal for the duration of the call.
        unsafe { crate::win32::dialog::pick_files(owner, title, filter) }
    }

    /// Opens the standard Save dialog titled `title` and returns the path the
    /// user chose, or `None` when they cancelled. `filter` lists (name,
    /// pattern) pairs and selects the first; `default_name`'s extension is
    /// added to a name typed without one. Blocks in a modal loop; other events
    /// are queued meanwhile.
    pub fn pick_save_path(
        &mut self,
        title: &str,
        default_name: &str,
        filter: &[(&str, &str)],
    ) -> Option<std::path::PathBuf> {
        let owner = with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller))?;
        // SAFETY: `owner` is a live window owned by this thread, and the dialog
        // is modal for the duration of the call.
        unsafe { crate::win32::dialog::pick_save_path(owner, title, default_name, filter) }
    }

    /// Opens the standard "Select folder" dialog titled `title` and returns the
    /// folder the user picked, or `None` when they cancelled. Blocks in a
    /// modal loop; other events are queued meanwhile.
    pub fn pick_folder(&mut self, title: &str) -> Option<std::path::PathBuf> {
        let owner = with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller))?;
        // SAFETY: `owner` is a live window owned by this thread, and the dialog
        // is modal for the duration of the call.
        unsafe { crate::win32::dialog::pick_folder(owner, title) }
    }

    /// Shows a popup menu at `at` (screen coordinates) and returns the chosen
    /// entry id. Blocks in a modal loop; other events are queued meanwhile.
    pub fn popup_menu(&mut self, entries: &[MenuEntry], at: Point) -> Option<u32> {
        let owner = with_runtime(|runtime| runtime.controller)?;
        menu::track(owner, entries, at)
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
        if let Some(debug) = debug_hwnd() {
            debug_window::set_dark(debug, dark);
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

    /// Opens the debug window (ROADMAP D14) showing `panel`, or brings the
    /// open one to the front and shows `panel` in it. Its input arrives as
    /// `Event::Debug`.
    pub fn open_debug(&mut self, panel: &DebugPanel) {
        if let Some(existing) = debug_hwnd() {
            debug_window::update(existing, panel);
            debug_window::bring_to_front(existing);
            return;
        }
        let dark = with_runtime(|runtime| runtime.ui_dark).unwrap_or(false);
        match debug_window::create(panel, dark) {
            Ok(hwnd) => {
                with_runtime(|runtime| runtime.debug = Some(hwnd));
            }
            Err(error) => tracing::error!(%error, "could not open the debug window"),
        }
    }

    /// Shows `panel` in the open debug window: in place when only values
    /// changed, otherwise rebuilt with a warning (the panel's shape is meant
    /// to be fixed for a session). Does nothing while it is closed.
    pub fn update_debug(&mut self, panel: &DebugPanel) {
        if let Some(hwnd) = debug_hwnd() {
            debug_window::update(hwnd, panel);
        }
    }

    pub fn close_debug(&mut self) {
        if let Some(hwnd) = with_runtime(|runtime| runtime.debug.take()).flatten() {
            // SAFETY: the debug window was created by this runtime.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }

    pub fn is_debug_open(&self) -> bool {
        debug_hwnd().is_some()
    }

    /// Posts `TaskbarCreated` to the controller, as Explorer does when it
    /// restarts, so the tray icon and takeover recover as they would then.
    pub fn simulate_taskbar_created(&mut self) {
        let Some(controller) = with_runtime(|runtime| runtime.controller) else {
            return;
        };
        // SAFETY: posting to our own live window.
        unsafe {
            let _ = PostMessageW(
                Some(controller),
                controller::taskbar_created_message(),
                WPARAM(0),
                LPARAM(0),
            );
        }
    }

    /// Every taskbar's topmost flag and whether it is above the desktop
    /// layer, which would otherwise take its clicks.
    pub fn taskbars(&self) -> Vec<crate::win32::debug_info::TaskbarRow> {
        crate::win32::debug_info::taskbars(desktop_layer_window())
    }

    /// The keyboard and mouse hooks' flags, for the debug window.
    pub fn hook_state(&self) -> crate::win32::debug_info::HookState {
        crate::win32::debug_info::hook_state()
    }

    /// This process's windows for the debug window's table: top-level ones
    /// in z-order, then desktop-hosted spaces. A shadow window is listed
    /// under the space that casts it.
    pub fn debug_windows(&self) -> Vec<WindowRow> {
        let Some((host, mut spaces)) = with_runtime(|runtime| {
            let spaces: Vec<(HWND, u64)> = runtime
                .spaces
                .iter()
                .map(|(id, hwnd)| (*hwnd, *id))
                .collect();
            (runtime.host, spaces)
        }) else {
            return Vec::new();
        };
        let shadows: Vec<(HWND, u64)> = spaces
            .iter()
            .filter_map(|(hwnd, id)| space_window::shadow_of(*hwnd).map(|shadow| (shadow, *id)))
            .collect();
        spaces.extend(shadows);
        let space_of = |window: HWND| {
            spaces
                .iter()
                .find(|(hwnd, _)| *hwnd == window)
                .map(|(_, id)| *id)
        };
        crate::win32::debug_info::windows_of_this_process(host, space_of)
    }

    /// Adds `lines` to the open debug window's console. Does nothing while
    /// it is closed.
    pub fn append_debug_log(&mut self, lines: &[LogLine]) {
        if lines.is_empty() {
            return;
        }
        if let Some(hwnd) = debug_hwnd() {
            debug_window::append_log(hwnd, lines);
        }
    }

    /// Replaces the clipboard with `text`.
    pub fn set_clipboard_text(&mut self, text: &str) {
        let Some(owner) = with_runtime(|runtime| runtime.controller) else {
            return;
        };
        if let Err(error) = clipboard::set_text(owner, text) {
            tracing::warn!(%error, "could not put text on the clipboard");
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

    /// Tells the user something went wrong, in a modal message box with an
    /// OK button. Blocks until it is dismissed.
    pub fn alert(&mut self, title: &str, text: &str) {
        let owner = with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller));
        if let Some(owner) = owner {
            // A message box owned by a background window opens behind the
            // active app unless its owner is made foreground first.
            // SAFETY: `owner` is a window of this thread, or a stale handle,
            // which the call rejects.
            let _ = unsafe { SetForegroundWindow(owner) };
        }
        warning_box(owner, title, text, MESSAGEBOX_STYLE(0));
    }

    /// Asks a question with `buttons` labels, returning the index of the button
    /// the user chose, or `None` if the dialog was dismissed (Escape, the close
    /// button). `default` marks the default button. Blocks in a modal loop.
    /// https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-taskdialogindirect
    pub fn choose(
        &mut self,
        title: &str,
        text: &str,
        buttons: &[&str],
        default: usize,
    ) -> Option<usize> {
        use windows::Win32::UI::Controls::{
            TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDF_ALLOW_DIALOG_CANCELLATION,
            TDF_POSITION_RELATIVE_TO_WINDOW, TDF_SIZE_TO_CONTENT, TaskDialogIndirect,
        };
        let owner = with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller));
        let title = wide(title);
        let text = wide(text);
        let labels: Vec<Vec<u16>> = buttons.iter().map(|label| wide(label)).collect();
        let entries: Vec<TASKDIALOG_BUTTON> = labels
            .iter()
            .enumerate()
            .map(|(index, label)| TASKDIALOG_BUTTON {
                nButtonID: index as i32,
                pszButtonText: PCWSTR(label.as_ptr()),
            })
            .collect();
        let config = TASKDIALOGCONFIG {
            cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
            hwndParent: owner.unwrap_or_default(),
            dwFlags: TDF_ALLOW_DIALOG_CANCELLATION
                | TDF_POSITION_RELATIVE_TO_WINDOW
                | TDF_SIZE_TO_CONTENT,
            pszWindowTitle: PCWSTR(title.as_ptr()),
            pszMainInstruction: PCWSTR(title.as_ptr()),
            pszContent: PCWSTR(text.as_ptr()),
            cButtons: entries.len() as u32,
            pButtons: entries.as_ptr(),
            nDefaultButton: default as i32,
            ..Default::default()
        };
        let mut pressed = -1i32;
        // SAFETY: the wide strings and `entries` outlive the modal call; a
        // failure (for example comctl32 v6 missing) returns an error.
        let result = unsafe {
            if let Some(owner) = owner {
                // The settings window or controller is a background window, so
                // the dialog would otherwise open behind the active app.
                let _ = SetForegroundWindow(owner);
            }
            TaskDialogIndirect(&config, Some(&mut pressed), None, None)
        };
        if result.is_err() || pressed < 0 {
            None
        } else {
            Some(pressed as usize)
        }
    }

    /// Shows the zoom pop-up (ROADMAP F-57) with `bar`, or updates it in
    /// place, at the top center of the monitor under the pointer. Its buttons
    /// raise `Event::Zoom`; it stays until `hide_zoom_bar`.
    pub fn show_zoom_bar(&mut self, bar: ZoomBar) {
        let existing = with_runtime(|runtime| runtime.zoom_bar).flatten();
        let hwnd = match existing {
            Some(hwnd) => hwnd,
            None => match zoom_bar::create() {
                Ok(hwnd) => {
                    with_runtime(|runtime| runtime.zoom_bar = Some(hwnd));
                    hwnd
                }
                Err(error) => {
                    tracing::warn!(%error, "could not create the zoom pop-up");
                    return;
                }
            },
        };
        zoom_bar::show(hwnd, bar);
    }

    pub fn hide_zoom_bar(&mut self) {
        if let Some(hwnd) = with_runtime(|runtime| runtime.zoom_bar).flatten() {
            zoom_bar::hide(hwnd);
        }
    }

    /// Whether the zoom pop-up is showing with the pointer over it, so the app
    /// can keep it open while the user reaches for its buttons.
    pub fn is_zoom_bar_hovered(&self) -> bool {
        with_runtime(|runtime| runtime.zoom_bar)
            .flatten()
            .is_some_and(zoom_bar::is_hovered)
    }

    pub fn is_settings_open(&self) -> bool {
        settings_hwnd().is_some()
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
        post_close();
    }

    /// Quits once each space in `plays` has played its way out (ROADMAP
    /// D13), or after `limit` whatever is still playing. Asked again while
    /// leaving, or with nothing to play, it quits at once.
    pub fn quit_after(&mut self, plays: Vec<(u64, Play)>, limit: Duration) {
        let already =
            with_runtime(|runtime| std::mem::replace(&mut runtime.leaving, true)).unwrap_or(true);
        if already {
            post_close();
            return;
        }
        let mut playing = false;
        for (id, play) in plays {
            if let Some(hwnd) = space_hwnd(id) {
                // SAFETY: a live space window, and no runtime borrow is held.
                unsafe { space_animation::set_visible(hwnd, false, play) };
                playing |= space_animation::is_leaving(hwnd);
            }
        }
        if !playing {
            post_close();
            return;
        }
        let millis = u32::try_from(limit.as_millis()).unwrap_or(u32::MAX);
        // A thread timer with its own procedure, so it takes none of the
        // app's `Timer` ids on the controller.
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-settimer
        // SAFETY: `close_at_limit` matches TIMERPROC and lives forever.
        unsafe {
            let _ = SetTimer(None, 0, millis, Some(close_at_limit));
        }
    }
}

/// Closes when the exit animation outlives its limit (`Ui::quit_after`).
unsafe extern "system" fn close_at_limit(_: HWND, _: u32, id: usize, _: u32) {
    // SAFETY: killing this thread timer, which fires only once.
    unsafe {
        let _ = KillTimer(None, id);
    }
    post_close();
}
