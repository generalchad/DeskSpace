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

    /// Watches `path` for changes under `key` and delivers
    /// `Event::FolderChanged { key }` once per debounced burst. Calling it
    /// again with the same key and path does nothing; another path replaces
    /// that key's watch. Call `unwatch_folder` to stop.
    pub fn watch_folder(&mut self, key: WatchKey, path: &Path) {
        let Some(controller) = with_runtime(|runtime| runtime.controller) else {
            return;
        };
        let result = with_runtime(|runtime| {
            if runtime.folder_watches.is_none() {
                match folder_watch::FolderWatch::start(controller, messages::FOLDER_CHANGE) {
                    Ok(watches) => runtime.folder_watches = Some(watches),
                    Err(error) => return Err(error),
                }
            }
            match runtime.folder_watches.as_mut() {
                Some(watches) => watches.watch(key, path),
                None => Ok(()),
            }
        });
        if let Some(Err(error)) = result {
            tracing::warn!(%error, path = %path.display(), "could not watch the folder");
        }
    }

    /// Stops the folder watch for `key`, if it has one.
    pub fn unwatch_folder(&mut self, key: WatchKey) {
        with_runtime(|runtime| {
            if let Some(watches) = runtime.folder_watches.as_mut() {
                watches.unwatch(key);
            }
        });
    }

    /// The folder watches the app has now, so it can bring them in line with
    /// the model (`App::sync_watches`).
    pub fn watched_folders(&self) -> Vec<WatchKey> {
        with_runtime(|runtime| {
            runtime
                .folder_watches
                .as_ref()
                .map(folder_watch::FolderWatch::watched_keys)
                .unwrap_or_default()
        })
        .unwrap_or_default()
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

    /// A `Send + Sync` handle a worker thread can use to wake the UI thread
    /// with `Event::Wake { id }`.
    pub fn waker(&self) -> Waker {
        let controller = with_runtime(|runtime| runtime.controller).unwrap_or_default();
        Waker {
            controller: controller.0 as isize,
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
