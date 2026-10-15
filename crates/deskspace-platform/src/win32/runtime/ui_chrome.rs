//! Ui methods for DeskSpace's own chrome: the color picker, popup
//! menus, dark mode, the settings window, the tray tooltip, and quitting.

use super::*;
use crate::input::ShellItem;
use crate::ui::CustomShortcut;

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
        // SAFETY: `owner` is a live window owned by this thread, and the picker
        // is modal for the duration of the call.
        unsafe { crate::win32::color_picker::pick(owner, initial, dark) }
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
    /// on and the dialog starts on the Desktop folder. Blocks in a modal loop;
    /// other events are queued meanwhile.
    pub fn pick_files(&mut self, title: &str) -> Vec<std::path::PathBuf> {
        let Some(owner) = with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller))
        else {
            return Vec::new();
        };
        // SAFETY: `owner` is a live window owned by this thread, and the dialog
        // is modal for the duration of the call.
        unsafe { crate::win32::dialog::pick_files(owner, title) }
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
