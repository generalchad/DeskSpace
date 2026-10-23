//! Ui methods for space windows and the desktop layer.

use super::*;

impl Ui {
    /// Creates space `id`'s window, hidden: `show_space` brings it in once
    /// its scene is pushed, so its first frame is never empty.
    pub fn create_space(
        &mut self,
        id: u64,
        rect: Rect,
        style: SpaceStyle,
    ) -> Result<(), PlatformError> {
        let Some((host, click_through, on_top, composition, exists)) = with_runtime(|runtime| {
            (
                runtime.host,
                runtime.click_through,
                runtime.on_top,
                runtime.composition,
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
            visible: false,
            composition,
        };
        let hwnd = space_window::create(options)?;
        with_runtime(|runtime| runtime.spaces.insert(id, hwnd));
        Ok(())
    }

    /// Plays `out` on space `id`'s window and then `back`, leaving it shown:
    /// the debug window's effect preview. Pressed again mid-way, it turns
    /// around. Does nothing while the spaces are hidden.
    pub fn preview_play(&mut self, id: u64, out: Play, back: Play) {
        if let Some(hwnd) = space_hwnd(id) {
            // SAFETY: a live space window, and no runtime borrow is held.
            unsafe { space_animation::preview(hwnd, out, back) };
        }
    }

    /// Moves every space above other windows, or back to the bottom, as
    /// `--on-top` places them; spaces created later follow too.
    pub fn set_on_top(&mut self, on_top: bool) {
        let Some(windows) = with_runtime(|runtime| {
            runtime.on_top = on_top;
            runtime.spaces.values().copied().collect::<Vec<_>>()
        }) else {
            return;
        };
        for hwnd in windows {
            space_window::set_on_top(hwnd, on_top);
        }
    }

    pub fn is_on_top(&self) -> bool {
        with_runtime(|runtime| runtime.on_top).unwrap_or(false)
    }

    /// Brings a new space's window in with `play` (ROADMAP D12), unless the
    /// spaces are hidden; then it shows with the rest.
    pub fn show_space(&mut self, id: u64, play: Play) {
        let visible = with_runtime(|runtime| runtime.spaces_visible).unwrap_or(false);
        if let (true, Some(hwnd)) = (visible, space_hwnd(id)) {
            // SAFETY: a live space window, and no runtime borrow is held.
            unsafe { space_animation::set_visible(hwnd, true, play) };
        }
    }

    /// Destroys space `id`'s window after it plays `play` (ROADMAP D12).
    /// The space is gone at once to every lookup, hiding included; the window
    /// stays only for the animation, taking no input or drops, and quitting
    /// destroys it with the rest.
    pub fn destroy_space(&mut self, id: u64, play: Play) {
        let Some(hwnd) = with_runtime(|runtime| {
            let hwnd = runtime.spaces.remove(&id)?;
            runtime.dying.push(hwnd);
            Some(hwnd)
        })
        .flatten() else {
            return;
        };
        // SAFETY: a live space window, and no runtime borrow is held.
        unsafe { space_animation::dismiss(hwnd, play) };
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

    /// Shows or hides every space with `play` (ROADMAP D12); an animation
    /// already running the other way turns around from where it is.
    pub fn set_spaces_visible(&mut self, visible: bool, play: Play) {
        let windows = with_runtime(|runtime| {
            runtime.spaces_visible = visible;
            runtime.spaces.values().copied().collect::<Vec<_>>()
        })
        .unwrap_or_default();
        for hwnd in windows {
            // SAFETY: space windows are owned by this runtime, and no
            // runtime borrow is held.
            unsafe { space_animation::set_visible(hwnd, visible, play) };
        }
    }

    /// Folds space `id` to `rect` (its title bar) or unfolds it to `rect`,
    /// playing `play` (ROADMAP D12). The model has already changed; the
    /// `set_space_rect` that follows with the same rect leaves the roll be.
    pub fn fold_space(&mut self, id: u64, rect: Rect, play: Play) {
        if let Some(hwnd) = space_hwnd(id) {
            // SAFETY: a live space window, and no runtime borrow is held.
            unsafe { space_animation::fold(hwnd, rect, play) };
        }
    }

    /// Creates the chrome-less desktop layer window spanning `rect`, reusing
    /// the space window and painter. Idempotent: calling it again does nothing.
    /// It is a `WS_CHILD` of the desktop host when one is in use and top-level
    /// otherwise, so it sits behind the spaces (a top-level space always
    /// places itself directly above it). It is never topmost, even with
    /// `--on-top` (`SpaceWindowOptions::desktop_layer`).
    pub fn create_desktop_layer(
        &mut self,
        rect: Rect,
        style: SpaceStyle,
    ) -> Result<(), PlatformError> {
        let Some((host, click_through, exists)) =
            with_runtime(|runtime| (runtime.host, runtime.click_through, runtime.layer.is_some()))
        else {
            return Err(PlatformError::NotRunning);
        };
        if exists {
            return Ok(());
        }
        let options =
            space_window::SpaceWindowOptions::desktop_layer(rect, style, host, click_through);
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

    fn with_ghost(&mut self, f: impl FnOnce(&DragGhost)) {
        if let Some(ghost) = with_runtime(|runtime| runtime.drag_ghost.take()).flatten() {
            f(&ghost);
            with_runtime(|runtime| runtime.drag_ghost = Some(ghost));
        }
    }
}
