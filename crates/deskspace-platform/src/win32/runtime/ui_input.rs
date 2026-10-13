//! Ui methods for input sources: mouse hooks, folder and keyboard
//! watches, timers, and hotkeys.

use super::*;

impl Ui {
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
}
