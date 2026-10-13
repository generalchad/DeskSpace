//! Ui methods for DeskSpace's own chrome: the color picker, popup
//! menus, dark mode, the settings window, the tray tooltip, and quitting.

use super::*;

impl Ui {
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
