//! Ui methods that call into the shell: opening, properties, menus,
//! drag out, clipboard, file operations, and info tips.

use super::*;

impl Ui {
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

    /// Runs `f` with the info tip checked out of the runtime, so the calls it
    /// makes (which send window messages) do not run under the borrow.
    fn with_info_tip(&mut self, f: impl FnOnce(&InfoTip)) {
        if let Some(tip) = with_runtime(|runtime| runtime.info_tip.take()).flatten() {
            f(&tip);
            with_runtime(|runtime| runtime.info_tip = Some(tip));
        }
    }
}
