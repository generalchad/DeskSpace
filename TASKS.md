# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

## How to work a task

1. Read `AGENTS.md` once, then only the `CONTEXT.md` sections a task names.
2. Do one task per commit. Do not combine tasks or fix "nearby" things; note
   them under **Found while working** at the end of this file instead.
3. Line numbers are approximate and drift. Find code by the function or type
   name given; the line is only a hint.
4. If the code does not match the task's description, or the task needs a
   design decision it does not state, **stop and ask**. Do not improvise.
5. Copy the pattern the task points at. Do not copy patterns listed under
   "Known bad examples" below.
6. Done means all of:
   - `cargo fmt --all`
   - `cargo clippy --workspace --all-targets -- -D warnings` (no new
     `#[allow]`)
   - `cargo test --workspace`, with new tests for new logic
   - `CONTEXT.md` updated where the task changes behavior, and the
     `ROADMAP.md` feature index (section 7) updated if a feature's status
     changed
   - the task's own "Done when" list
7. Commit with the repo's style: `feat: ...`, `fix: ...`, `refactor: ...`,
   `docs: ...`, imperative and lowercase. Then tick the task's box here in
   the same commit.
8. Tasks marked **[needs human]** need an on-screen check you cannot do.
   Implement, then list exactly what to check in the commit body and leave the
   box unticked with "(awaiting check)".
9. Tasks in section D are **not** for routine work. Skip them.

### Known bad examples (do not copy until fixed)

- `App::reflow_space` (`deskspace-app/src/app.rs`) mutates a `Space`
  directly instead of through a `Command` (fixed by A5).
- `Ui::show_info_tip` / `hide_info_tip` (`win32/runtime.rs`) hold a runtime
  borrow across a Win32 call that sends messages (fixed by A1).
- `unsafe` blocks without `// SAFETY:` (fixed by A2).
- `pick_space_tint` / `pick_space_text` and the opacity/sort `match` arms in
  `show_space_menu` are duplicated code (fixed by B3/B4).

### Recipe: a new global setting

The pattern used by `Theme.icon_size` / `Theme.overlay_size`:

1. `deskspace-core/src/theme.rs` (or `deskspace-config/src/schema.rs` for a
   `WindowSettings` field): add the field with a `///` doc, give it a value in
   `Default`, and rely on the struct-level `#[serde(default)]`. If it is a
   range, add `pub const NAME_RANGE`/`NAME_DEFAULT` like `OVERLAY_SIZE_RANGE`.
2. `deskspace-app/src/settings.rs`: add a `field::` id (next free number), add
   a `Field` to the right page in `build_form`, and a match arm in
   `apply_field` that clamps and returns the right `Effect` (`Appearance` to
   restyle, `Layout` to re-flow, `Pending` for apply-on-OK).
3. If Cancel must undo a re-flow, extend `App::cancel_settings`.
4. Tests: a serde round-trip / missing-field default test in core or config,
   and an `apply_field` test in `settings.rs` (copy the existing ones).

### Recipe: a new per-space override

The pattern used by `Space.opacity` / `Space.tint` (F-24):

1. `deskspace-core/src/space.rs`: add `#[serde(default)] pub field:
   Option<T>` (or a value with a default fn), documented.
2. `deskspace-core/src/command.rs`: add a `Command::SetSpaceX { id, x }`
   variant and an arm in `apply_with_layout` copying `SetSpaceTint`. Add a
   test next to the existing command tests.
3. `deskspace-app/src/app.rs`: add menu ids in `mod menu`, entries in
   `space_appearance_menu`, a handler like `set_space_opacity`, and include the
   field in `reset_space_appearance`.
4. `deskspace-app/src/scene.rs`: apply the override where the scene is built.

---

## A. Fix the bad examples (do these first)

- [ ] **A1. Do not hold the runtime borrow across info-tip calls.**
  - File: `crates/deskspace-platform/src/win32/runtime.rs`,
    `Ui::show_info_tip` and `Ui::hide_info_tip` (~line 482-514).
  - Problem: `tip.show(...)` / `tip.hide()` send window messages while
    `with_runtime` holds the borrow, which the module header forbids.
  - Fix: copy the check-out pattern in `show_drag_ghost` and `with_ghost`
    right above: `take()` the `InfoTip` out of the runtime, call it, put it
    back. Add a small `with_info_tip(&mut self, f)` helper like `with_ghost`.
  - Done when: no `with_runtime` closure in `runtime.rs` calls a method that
    sends messages. Behavior unchanged.

- [ ] **A2. Add the missing `// SAFETY:` comments.**
  - Locations (from a heuristic scan; re-scan, there may be a few more or
    fewer): `win32/clipboard.rs` 132, 133, 135, 143, 146, 198;
    `win32/desktop_shell.rs` 46, 68, 80; `win32/dpi.rs` 8, 16;
    `win32/drop_target.rs` 299, 301; `win32/hotkey.rs` 27, 34;
    `win32/keyboard.rs` 197, 209, 229, 239, 246, 278.
  - Each comment states why the call's invariants hold, in one or two lines,
    in the style of the existing ones (for example "the hook must pass the
    event on" or "`hwnd` is a live window owned by this thread").
  - Also: `win32/drop_target.rs` ~384 uses `/// SAFETY:` on an `unsafe fn`;
    change it to a `/// # Safety` section. `win32/drag_image.rs` ~147 refers to
    `pixels.len()`, which does not exist; reword it to the real variable.
  - Comments only. No code changes.

- [ ] **A3. Fix stale and misleading comments.** Comments only.
  - `deskspace-app/src/scene.rs` ~238: the doc for the scroll thumb ("A thin
    bar along the right edge...") sits on `auto_text_shadow`. Move it to the
    scroll-thumb function it describes, or delete it if that one already has
    a doc.
  - `deskspace-platform/src/edit.rs` line 1: says it is for renaming a space
    title; it also renames item labels.
  - `win32/info_tip.rs` ~123: `send` says "Posts"; it sends.
  - `win32/tray.rs` ~101: `shared_icon` says "A copy"; it returns the same
    handle.
  - `render/d2d.rs` ~621: mentions `render_item`; the function is
    `render_items`.
  - `win32/keyboard.rs` ~137: `stop` says "clears both modes"; it clears all
    three flags. Line ~8 lists the watched keys; add F2 and Ctrl+A. Line ~173
    `is_key`'s doc restates the code; delete it.
  - `deskspace-app/src/app.rs` ~179: the `edit` field says "The space being
    renamed inline"; it is a space title or an item label.

- [ ] **A4. Explain the magic constants AGENTS.md calls out.**
  - `win32/desktop_shell.rs` ~9 `SPAWN_WORKER_MESSAGE = 0x052C`: add a short
    comment that this undocumented Progman message makes Explorer create the
    wallpaper WorkerW, and why the timeout value is what it is.
  - `win32/keyboard.rs` ~76 `VK_A_CODE = 0x41`: replace with
    `windows::Win32::UI::Input::KeyboardAndMouse::VK_A` and `is_key`.
  - `win32/hotkey.rs`: replace raw key codes (`0x70`, `0x20`, `0x09`,
    `0x0D`, `0x1B`) with `VK_F1`, `VK_SPACE`, `VK_TAB`, `VK_RETURN`,
    `VK_ESCAPE`; document `HOTKEY_BASE_ID`.
  - `win32/data_object.rs` ~192 `0x8007000E`: use `E_OUTOFMEMORY`.
  - Done when: existing hotkey parsing tests still pass unchanged.

- [ ] **A5. Route space re-flow through a `Command`.**
  - Add `Command::Reflow { id }` in `deskspace-core/src/command.rs`. Its arm
    does exactly what `App::reflow_space` does today: for a locked space set
    `title_height` from the layout and call `position_items`; otherwise call
    `arrange_space`. Return `NotFound` for a missing id, else `Applied`.
  - In `App::reflow_space`, replace the `space_mut` block with
    `self.config.desktop.apply_with_layout(Command::Reflow { id }, layout,
    bounds)`. Get `bounds` the way `App::dispatch` does (look at it; use the
    space's center as the point).
  - Tests in `command.rs`: a locked space keeps its rect but gets the new
    title height; an unlocked auto-sized space resizes to fit.
  - Done when: `reflow_space` contains no `space_mut`. Update the Commands
    list in `CONTEXT.md` section 4.

- [ ] **A6. Cancel after toggling "Match Explorer" must re-flow.**
  - Bug: `field::MATCH_EXPLORER` returns `Effect::Layout`, but
    `App::cancel_settings` only re-flows when `layout` or `icon_size`
    changed, so cancelling a match toggle leaves spaces on the wrong grid.
  - Fix: add `|| session.saved_theme.match_explorer !=
    self.config.theme.match_explorer` to the `relayout` condition.
  - Prefer a small pure helper `fn needs_relayout(saved: &Session, config:
    &Config) -> bool` in `settings.rs` with a test, used by `cancel_settings`.

## B. Small cleanups

Each is independent. None changes behavior.

- [ ] **B1. Remove clearly dead code.** For each item, `grep` the whole
  workspace first; delete only if nothing outside its own tests uses it.
  - `deskspace-core/src/id.rs` `IdGenerator` (and its tests).
  - `deskspace-core/src/space.rs` `Space::required_size` and
    `Space::resize_to_fit` (duplicates of `arrange_space`).
  - `deskspace-core/src/desktop.rs` `Desktop::item_with_path` (the app uses
    `App::find_item_by_file`).
  - `deskspace-platform/src/error.rs` `PlatformError::NoDesktopWindow`.
  - Keep `Command::{MoveSpace, AutoArrange, SnapAll}`: the M3 Arrange
    submenu will use them.

- [ ] **B2. Name the repeated clamp ranges.**
  - The ranges `50..=200` (icon size), `60..=160` (label size), `0..=24`
    (grid gap/padding), and `0..=64` (snap) are repeated in
    `deskspace-app/src/settings.rs` (`build_form`, `apply_field`),
    `deskspace-app/src/app.rs` (`icon_scale`), `deskspace-app/src/scene.rs`,
    and `render/d2d.rs`.
  - Add `ICON_SIZE_RANGE` and `LABEL_SIZE_RANGE` next to
    `OVERLAY_SIZE_RANGE` in `theme.rs`, and `GAP_RANGE` / `SNAP_RANGE` in
    `layout.rs`. Use them everywhere.

- [ ] **B3. Drive the opacity and sort menus from one table each.**
  - `deskspace-app/src/app.rs`: the opacity values appear both in
    `space_appearance_menu`'s `OPACITIES` and in `show_space_menu`'s match;
    the sort ids are mapped both in `space_sort_menu` and in the match.
  - Move both tables to module-level `const`s. Build the menus from them, and
    in `show_space_menu` look the chosen id up in the table instead of
    matching each id.
  - Test: every table id is unique and does not collide with other `menu::`
    ids.

- [ ] **B4. Merge `pick_space_tint` and `pick_space_text`.**
  - One `fn pick_space_color(&mut self, ui, id, at, which: SpaceColor)` with
    a small private `enum SpaceColor { Tint, Text }` choosing the current
    value and the `Command`.

- [ ] **B5. One helper for "dispatch, repaint, schedule a save".**
  - `fn apply_to_space(&mut self, ui: &mut Ui, id: SpaceId, command: Command,
    at: Point) -> bool` in `app.rs`. Use it in `set_space_opacity`,
    `sort_space`, the `FIT` and `LOCK` arms, `toggle_rollup`, and wherever
    else the three-line pattern appears. Keep any extra step (like
    `self.scroll.remove(&id)` for `FIT`) at the call site.

- [ ] **B6. Use the shared `wide` helper.**
  - `win32/info_tip.rs` ~69 re-implements UTF-16 conversion; use
    `crate::win32::system::wide`. Replace fully qualified
    `crate::win32::system::wide(...)` calls with a `use` at the top of each
    file.

- [ ] **B7. Simplify the keyboard hook's pass-through.**
  - `win32/keyboard.rs` `hook_proc` calls `CallNextHookEx` in seven places.
    Move the body into `fn handle(code, wparam, event) -> bool` (true =
    consumed) and have `hook_proc` call `CallNextHookEx` once when it returns
    false. No behavior change; keep the comments that explain each branch.

## C. Features (ROADMAP M3, remaining items 1-4)

Do them in order. Each is one commit plus a `docs:` commit if the docs change
is large.

- [ ] **C1. Per-space icon size (rest of F-25).** Follow "Recipe: a new
  per-space override".
  - `Space.icon_size: Option<u8>`, a percent; `None` follows the global
    setting.
  - `Command::SetSpaceIconSize { id, icon_size: Option<u8> }`, clamped to
    `ICON_SIZE_RANGE` (from B2; if B2 is not done, use `(50, 200)` and leave a
    note).
  - Menu: an "Icon size" submenu in `space_appearance_menu` with "Default"
    (None), "Small" 75, "Medium" 100, "Large" 150, "Extra large" 200; the
    current one checked.
  - `App::space_scale(id)`: when the space has an override, return
    `percent / 100 * space_dpi(id.0)`, ignoring Explorer's icon size and the
    global setting. Leave the other paths unchanged.
  - After the command, call `reflow_space` (not just `refresh_space`) and
    `schedule_save`. `reset_space_appearance` also clears it.
  - Tests: command clamps and sets/clears; serde default is `None`.
  - **[needs human]** check: a Large space next to a Default space, both
    crisp, hit-testing correct.

- [ ] **C2. Windows accent color (F-29).** Follow "Recipe: a new global
  setting".
  - Platform: `pub fn system_accent_color() -> Option<Color>` in
    `win32/system.rs`, reading `HKCU\Software\Microsoft\Windows\DWM` value
    `AccentColor` (a DWORD laid out `0xAABBGGRR`). Return `None` on any
    failure. Copy the registry-call style from `win32/desktop_entry.rs`. Add
    a pure, tested `fn color_from_abgr(u32) -> Color`. Re-export from
    `lib.rs`.
  - Core: `Theme.use_accent: bool` (default false), and
    `Palette::with_accent(self, accent: Color) -> Palette` that replaces
    `selected_fill` and `border` with the accent at their current alphas.
    Test it.
  - App: store `system_accent: Option<Color>` on `App`, read at start and on
    `Event::SystemThemeChanged` (Windows sends it when the accent changes).
    Wherever a scene palette is built (`refresh_space`, and the drag scene),
    apply `with_accent` when `use_accent` is on and an accent is known. Add
    one helper `fn palette(&self) -> Palette` and use it at every
    `self.config.theme.palette(self.system_dark)` call site.
  - Settings: a "Use Windows accent color" toggle on the Colors page,
    `Effect::Appearance`.
  - **[needs human]** check: selection and border pick up the accent, and
    change live when the accent changes in Windows Settings.

- [ ] **C3. Theme override for settings and menus (F-30).** Follow "Recipe:
  a new global setting".
  - Core: `Theme.ui_mode: ThemeMode` (default `System`). It is separate from
    `Theme.mode`, which only colors the spaces.
  - App: `fn ui_dark(&self) -> bool` returning `self.config.theme.ui_mode
    .is_dark(...)`-style logic (reuse `Theme::is_dark`'s match; add a
    `ThemeMode::resolve(system_dark) -> bool` in core if that is cleaner, and
    make `Theme::is_dark` use it). Replace both `ui.set_ui_dark(self
    .system_dark)` calls in `on_event` with `ui.set_ui_dark(self.ui_dark())`.
  - Settings: a "Settings and menus" choice on the General page reusing the
    `MODES` table. In `on_settings_event`, after an `Effect::Appearance`, also
    call `ui.set_ui_dark(self.ui_dark())` so it previews live; Cancel must
    restore it (extend `cancel_settings`).
  - **[needs human]** check: switching to Dark/Light repaints the open
    settings window and the next popup menu.

- [ ] **C4. Start with Windows (F-34).** Follow "Recipe: a new global
  setting".
  - Config: `WindowSettings.start_with_windows: bool` (default false).
  - Platform: `pub fn set_start_with_windows(enabled: bool) -> Result<()>`
    in a new `win32/autostart.rs`, writing or deleting value `DeskSpace`
    under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. The value is
    the quoted path of `std::env::current_exe()`. Copy registry handling and
    error mapping (`PlatformError::Registry`) from `win32/desktop_entry.rs`.
    Put the command-line string in a pure, tested function. Re-export from
    `lib.rs`.
  - App: apply it in `apply_settings` when the value changed, and once at
    startup when enabled (so the stored path follows a moved executable).
    Log failures with `tracing::warn!`; do not block Apply.
  - Settings: a toggle on the General page, `Effect::Pending`.
  - Tests must not touch the registry.
  - **[needs human]** check: the value appears/disappears in the Run key and
    DeskSpace starts at sign-in.

## D. Reserved (strong model or maintainer; do not start)

Listed so they are not attempted piecemeal. Each needs design work first.

- **D1. Split `deskspace-app/src/app.rs`** (~2,900 lines) into `app/`
  submodules, each with its own `impl App` block: `persist`, `view`, `hit`,
  `selection`, `visibility`, `rename`, `pointer` (drag), `info_tip`,
  `files`, `clipboard`, `menus`, `settings_session`. Move `app::run` into
  `bootstrap.rs`. Pure moves, one commit, no behavior change.
- **D2. Extract `Selection`, `PointerState`, and `EditState` from `App`.**
  After D1. Collapses the five copies of "select only this item" and the
  three copies of "forget this item everywhere".
- **D3. Platform module layout.** Move `shell.rs` under `win32/` and split it
  (desktop items, icon loading, pixel helpers); one icon decoder instead of
  two; a `win32/messages.rs` owning every `WM_APP + n`; generic helpers for
  `GWLP_USERDATA` window state and window-class registration; split
  `settings_window.rs` and `render/d2d.rs`; make internal modules private.
- **D4. Keep IO out of core.** `deskspace-core/src/item.rs` reads file
  metadata; move it to the app/platform and pass the values into
  `Command::AddItem`.
- **D5. M2 desktop takeover design.** Needs answers to ROADMAP open questions
  1 and 2.
- **D6. Undo design.** Needs an answer to ROADMAP open question 3.
- **D7. On-screen verification pass** (maintainer). The list is in
  `CONTEXT.md` section 2, "Pending on-screen checks".

## Found while working

Add one line per issue noticed but not fixed: file, symptom, suggested task.

- `deskspace-app/src/app.rs` `reset_space_appearance`: the three dispatches
  are combined with `&&`. Harmless today (all three succeed or the space is
  missing), but reads like a bug; B5's helper would make it clearer.
