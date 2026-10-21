# TASKS archive

Finished tasks, moved here verbatim from `docs/TASKS.md` (first batch on
2026-10-13). This file is history: it is not kept up to date, and its file and
line pointers describe the code when each task landed. `docs/CONTEXT.md` is
authoritative for current behavior. Open on-screen checks for these tasks are
listed in `docs/TASKS.md` section V.

New entries are appended at the end, under the letter of their section.

## A. Fix the bad examples (do these first)

- [x] **A1. Do not hold the runtime borrow across info-tip calls.**
  - File: `crates/deskspace-platform/src/win32/runtime.rs`,
    `Ui::show_info_tip` and `Ui::hide_info_tip` (~line 482-514).
  - Problem: `tip.show(...)` / `tip.hide()` send window messages while
    `with_runtime` holds the borrow, which the module header forbids.
  - Fix: copy the check-out pattern in `show_drag_ghost` and `with_ghost`
    right above: `take()` the `InfoTip` out of the runtime, call it, put it
    back. Add a small `with_info_tip(&mut self, f)` helper like `with_ghost`.
  - Done when: no `with_runtime` closure in `runtime.rs` calls a method that
    sends messages. Behavior unchanged.

- [x] **A2. Add the missing `// SAFETY:` comments.**
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

- [x] **A3. Fix stale and misleading comments.** Comments only.
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

- [x] **A4. Explain the magic constants AGENTS.md calls out.**
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

- [x] **A5. Route space re-flow through a `Command`.**
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

- [x] **A6. Cancel after toggling "Match Explorer" must re-flow.**
  - Bug: `field::MATCH_EXPLORER` returns `Effect::Layout`, but
    `App::cancel_settings` only re-flows when `layout` or `icon_size`
    changed, so cancelling a match toggle leaves spaces on the wrong grid.
  - Fix: add `|| session.saved_theme.match_explorer !=
    self.config.theme.match_explorer` to the `relayout` condition.
  - Prefer a small pure helper `fn needs_relayout(saved: &Session, config:
    &Config) -> bool` in `settings.rs` with a test, used by `cancel_settings`.

- [x] **A7. Bring the keyboard comments up to date.** Comments only.
  - `crates/deskspace-platform/src/win32/keyboard.rs`:
    - the module header (lines ~1-14) lists an old, narrow key set. Rewrite
      it to say the hook has an edit mode and two watch flags, and point to
      `fn handle` for the keys, instead of listing them (a list goes stale);
    - the `WATCH_SELECTION` / `WATCH_PASTE` comments in `thread_local!`
      (~116-119) say "Delete, Ctrl+C, and Ctrl+X" and "Ctrl+V". Reword to
      "an item is selected" and "a space is current (the name is
      historical)";
    - the doc of `watch_selection` (~138) repeats the old key list; reword it
      the same way;
    - `RENAME_KEY` (~48) says "while an item is selected"; F2 also fires with
      only a current space.
  - `crates/deskspace-platform/src/win32/runtime.rs`, the doc of
    `Ui::watch_selection_keys` (~439-443): same fix.
  - Use the flag-by-key table in `AGENTS.md` (Gotchas) as the source of
    truth, and do not copy the table into the code.
  - Done when: no comment in either file lists keys that `fn handle` does not
    match. Then delete the "(`TASKS.md` A7)" sentence from the AGENTS.md
    Gotchas entry.

- [x] **A8. Keep the check-box mode when the desktop view cannot be read.**
  - Bug: `crates/deskspace-platform/src/win32/display_settings.rs`,
    `desktop_display` starts from `global_display()`, whose `check_boxes` is
    `CheckBoxMode::Hidden`, and only overrides it when the view read
    succeeds. So a failed read (the view is briefly unavailable, for example
    while Explorer restarts) hides the boxes, while the comment in
    `desktop_display` claims it does not. The app then stores that snapshot
    (`App::refresh_shell_display`, `app/view.rs`) and, because the mode left
    `Independent`, clears the independent ticks.
  - Fix:
    1. Add `pub fn desktop_display_since(previous: CheckBoxMode) ->
       ShellDisplay` beside `desktop_display`. It does the same reads, but
       when the view is unavailable or `view_check_boxes` returns `None` it
       uses `previous`. Have `desktop_display()` call it with
       `CheckBoxMode::Hidden`, so its behavior is unchanged for
       `shell.rs` (`enumerate_desktop_items`, which reads only the hidden-file
       flags).
    2. Put the choice in a pure function `fn check_box_mode(read:
       Option<CheckBoxMode>, previous: CheckBoxMode) -> CheckBoxMode` with a
       test for both cases.
    3. Re-export it from `win32/mod.rs` and `lib.rs` next to
       `desktop_display`. In the app, the two callers (`app.rs` in the
       `Started` arm and `App::refresh_shell_display` in `app/view.rs`) call
       `desktop_display_since(self.shell_display.check_boxes)`.
    4. Move the "a failed read must not hide check boxes" comment onto
       `desktop_display_since`, where it is true.
  - Leave `icon_size` and `icon_spacing` as they are (their `None` also means
    "not in icon mode", so they need a separate design).
  - Then delete the "(`TASKS.md` A8)" sentence in `CONTEXT.md` section 6 and
    describe the new behavior there.
  - Done when: the new test passes and both app callers pass the stored mode.

- [x] **A9. Clamp the grid and new-space size when the config loads.**
  - Today `Config::migrate` (`crates/deskspace-config/src/schema.rs`, ~109)
    clamps the theme and every space look, but not `desktop.layout.gap`,
    `padding`, or `snap`, nor `window.new_space_width` / `new_space_height`.
    They are clamped only where used, so a hand-edited value is wrong in the
    settings window until it is edited.
  - Add `pub fn clamped(self) -> Self` to `GridSpec`
    (`crates/deskspace-core/src/layout.rs`): `gap` and `padding` to
    `GAP_RANGE`, `snap` to `SNAP_RANGE`. Copy the shape of `Theme::clamped`.
  - Add `pub fn clamped(self) -> Self` to `WindowSettings` (`schema.rs`):
    width and height to `NEW_SPACE_SIZE_RANGE`.
  - Call both in `Config::migrate`, next to the theme clamp.
  - Tests: one per `clamped` (out-of-range in, in-range out), and one
    `Config` load test with an out-of-range `gap` in the TOML.
  - Then delete "(`TASKS.md` A9)" from `CONTEXT.md` section 5 and say the
    grid and new-space size are clamped on load too.

- [x] **A10. Make the menu-id test cover every id.**
  - `crates/deskspace-app/src/app.rs`, test `menu_ids_are_unique`: the
    `other` list omits `menu::SPACE_SETTINGS`. Add it.
  - Done when: every `pub const` in `mod menu` appears either in `other` or
    in a preset table. Test-only change (`test: ...`).

- [x] **A11. Say "spaces", not the old container name, in `scripts/dev.ps1`.**
  - The help text and parameter comments still used the old container name
    (examples at ~17 and ~21, parameters at ~39 and ~43). Replace it with
    "spaces". Comments and help text only; no behavior change.

## B. Small cleanups

Each is independent. None changes behavior.

- [x] **B1. Remove clearly dead code.** For each item, `grep` the whole
  workspace first; delete only if nothing outside its own tests uses it.
  - `deskspace-core/src/id.rs` `IdGenerator` (and its tests).
  - `deskspace-core/src/space.rs` `Space::required_size` and
    `Space::resize_to_fit` (duplicates of `arrange_space`).
  - `deskspace-core/src/desktop.rs` `Desktop::item_with_path` (the app uses
    `App::find_item_by_file`).
  - `deskspace-platform/src/error.rs` `PlatformError::NoDesktopWindow`.
  - Keep `Command::{MoveSpace, AutoArrange, SnapAll}`: the M3 Arrange
    submenu will use them.

- [x] **B2. Name the repeated clamp ranges.**
  - The ranges `50..=200` (icon size), `60..=160` (label size), `0..=24`
    (grid gap/padding), and `0..=64` (snap) are repeated in
    `deskspace-app/src/settings.rs` (`build_form`, `apply_field`),
    `deskspace-app/src/app.rs` (`icon_scale`), `deskspace-app/src/scene.rs`,
    and `render/d2d.rs`.
  - Add `ICON_SIZE_RANGE` and `LABEL_SIZE_RANGE` next to
    `OVERLAY_SIZE_RANGE` in `theme.rs`, and `GAP_RANGE` / `SNAP_RANGE` in
    `layout.rs`. Use them everywhere.

- [x] **B3. Drive the opacity and sort menus from one table each.**
  - `deskspace-app/src/app.rs`: the opacity values appear both in
    `space_appearance_menu`'s `OPACITIES` and in `show_space_menu`'s match;
    the sort ids are mapped both in `space_sort_menu` and in the match.
  - Move both tables to module-level `const`s. Build the menus from them, and
    in `show_space_menu` look the chosen id up in the table instead of
    matching each id.
  - Test: every table id is unique and does not collide with other `menu::`
    ids.

- [x] **B4. Merge `pick_space_tint` and `pick_space_text`.**
  - One `fn pick_space_color(&mut self, ui, id, at, which: SpaceColor)` with
    a small private `enum SpaceColor { Tint, Text }` choosing the current
    value and the `Command`.

- [x] **B5. One helper for "dispatch, repaint, schedule a save".**
  - `fn apply_to_space(&mut self, ui: &mut Ui, id: SpaceId, command: Command,
    at: Point) -> bool` in `app.rs`. Use it in `set_space_opacity`,
    `sort_space`, the `FIT` and `LOCK` arms, `toggle_rollup`, and wherever
    else the three-line pattern appears. Keep any extra step (like
    `self.scroll.remove(&id)` for `FIT`) at the call site.

- [x] **B6. Use the shared `wide` helper.**
  - `win32/info_tip.rs` ~69 re-implements UTF-16 conversion; use
    `crate::win32::system::wide`. Replace fully qualified
    `crate::win32::system::wide(...)` calls with a `use` at the top of each
    file.

- [x] **B7. Simplify the keyboard hook's pass-through.**
  - `win32/keyboard.rs` `hook_proc` calls `CallNextHookEx` in seven places.
    Move the body into `fn handle(code, wparam, event) -> bool` (true =
    consumed) and have `hook_proc` call `CallNextHookEx` once when it returns
    false. No behavior change; keep the comments that explain each branch.

- [x] **B8. Remove the settings window's unused tab layout.**
  - `SettingsForm.nav` (`crates/deskspace-platform/src/ui.rs`) is an
    `Option<SettingsNav>`; `None` builds the old top-tab layout. The app
    always passes `Some` (`build_form` in `deskspace-app/src/settings.rs`),
    so the tab code is dead.
  - Make the field `pub nav: SettingsNav` and update its doc. Fix the
    constructors and tests in `ui.rs` (a test that used `nav: None` can use a
    small `SettingsNav` with one group, like `nav_entry` already builds).
  - In `crates/deskspace-platform/src/win32/settings_window.rs`, delete what
    only the tab layout used: `ID_TAB`, `TAB_SUBCLASS_ID`, `tab_proc`,
    `paint_tab`, `on_tab_changed`, the `None` branch in `build` (it creates a
    `WC_TABCONTROLW`), and any `match`/`if let` on `nav` that only existed for
    `None`. Search for each name after deleting to be sure nothing still uses
    it.
  - In `win32/dark_mode.rs`, reword the `UiColors` field comments that
    describe tabs (~128-135) to describe the panes.
  - Done when: `rg -i "tab_proc|paint_tab|WC_TABCONTROL|ID_TAB" crates`
    finds nothing, and the settings window still opens (check with
    `cargo run -p deskspace-app -- --config <scratch.toml> --settings`;
    remove the scratch config afterwards). Update `CONTEXT.md` section 6
    (the `SettingsForm.nav` and dark-mode bullets mention B8).

- [x] **B9. Remove stale `too_many_arguments` allows.**
  - `#[allow(clippy::too_many_arguments)]` sits on functions in
    `deskspace-app/src/scene.rs` (`space_scene`, `desktop_layer_scene`,
    `desktop_layer_drag_scene`, `drag_scene`), `render/d2d.rs` (`draw_item`,
    `draw_check_box`, `draw_text`, `draw_edit_buffer`), and
    `win32/settings_window.rs` (one `Builder` method). `space_scene` takes 7
    arguments, which clippy allows, so at least that one is stale.
  - Remove each allow, one at a time, and run
    `cargo clippy --workspace --all-targets -- -D warnings`. Put it back only
    if clippy then fails for that function.
  - Do not restructure arguments in this task. List the allows that had to
    stay under **Found while working** with their argument counts, so a later
    task can group them into a struct (as `ViewState` does for scenes).

## C. Features (ROADMAP M3, remaining items 1-4)

Do them in order. Each is one commit plus a `docs:` commit if the docs change
is large.

- [x] **C1. Per-space icon size (rest of F-25).** Superseded: a space's icon
  size is now part of its look (section H), and this field is folded into it
  on load. Kept for the record.
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
  setting". (awaiting check)
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
  a new global setting". (awaiting check)
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
  setting". (awaiting check)
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

- [ ] **C5. The item preset dropdown (F-48).** (awaiting check)
  - Core: `Theme.match_explorer: bool` became `Theme.preset: Preset`
    (`Custom | MatchExplorer`), with `Theme::match_explorer()` and a
    `match_explorer` serde alias so older files load. `preset` is a look
    setting, so it is per space through `SpaceLook` too.
  - Settings: the Appearance page's "Match Explorer's display" toggle became
    the "Item preset" choice (`field::PRESET`, options `PRESETS`); a chosen
    preset returns `Effect::Layout`. The Layout page's icon-size,
    icon-spacing, and edge-padding fields are greyed under `MatchExplorer`.
  - Tests: core `match_explorer()`; config default, snake_case, and the legacy
    bool; `app::view` resolution parity; the settings choice and the disabled
    fields.
  - **[needs human]** check: the dropdown shows "Match Explorer's display" /
    "Custom"; "Custom" enables the Layout fields and the icon size takes
    effect; "Match Explorer's display" greys them.

- [ ] **C7. The Hotkeys page (F-18, M3 remaining item 1).** (awaiting check)
  - Config: `WindowSettings.new_space_hotkey` and `settings_hotkey` strings
    (default empty = unbound), `#[serde(default)]`.
  - `settings.rs`: a `HOTKEY_BINDINGS` table (field, note field, label, index)
    drives both the page and the app's registration. Move the existing
    `HOTKEY` field here; a `hotkey_texts` / `hotkey_errors` pair on `Session`
    stages each binding until Apply.
  - App: register every binding on `Started` and on Apply (empty unregisters),
    unregister all on `CloseRequested`; `NEW_SPACE_HOTKEY` creates a space at
    `cursor_pos()`, `SETTINGS_HOTKEY` opens settings. A test keeps the app's
    indices in step with the table.
  - **[needs human]** check: bind each hotkey, confirm it fires; a conflicting
    binding is reported next to its field and the window stays open; clearing
    a binding stops it firing.
  - Landed as `feat: add the Hotkeys settings page (C7)`. The platform gained
    `cursor_pos()` (`win32/system.rs`).

- [ ] **C8. The New spaces page (F-17, M3 remaining item 1).** (awaiting check)
  - Config: `WindowSettings.new_space_title` (default "New space"),
    `new_space_width` / `new_space_height` (default 240x120, clamped to
    `NEW_SPACE_SIZE_RANGE`), and `new_space_locked` (default false), all
    `#[serde(default)]`.
  - Core: `Command::CreateSpace` carries `size` and `locked`; the app supplies
    the configured defaults in `App::create_space`, clamped, with a fallback
    title when the configured one is blank.
  - Settings: a "New spaces" page with the title, the width and height
    (`editable` sliders), and the lock toggle, all `Effect::Pending` (they
    affect future spaces only).
  - **[needs human]** check: create a space from the space menu, the tray, and
    the hotkey; it uses the configured title, size, and lock state, and a
    locked new space keeps its size and scrolls.
   - Landed as `feat: add the New spaces settings page (C8)`.

## D. Reserved (resolved items)

- **R1. Split `deskspace-app/src/app.rs`** (4,483 lines as of 2026-10-11) into
  `app/` submodules, each with its own `impl App` block: `persist`, `view`,
  `hit`, `selection`, `visibility`, `rename`, `pointer` (drag), `info_tip`,
  `files`, `clipboard`, `menus`, `settings_session`. Move `app::run` into
  `bootstrap.rs`. Pure moves, no behavior change. The scaffold (the `app/`
  module tree plus the `run` move) landed first; each group then moves into
  place in its own commit. Landed: every group is in place, and `app.rs` is
  down to about 775 lines (about 600 without tests). The resulting layout is
  in `CONTEXT.md` section 3 and the file map above.
- **R2. Extract `Selection`, `PointerState`, and `EditState` from `App`.**
  After R1. Collapses the five copies of "select only this item" and the
  three copies of "forget this item everywhere". Landed: each owns its state
  behind its own methods, and the pointer, hover, marquee, and selection
  writes go through them.
- **R5. M2 desktop takeover design.** Resolved: ROADMAP D7 (model) and D8
  (placement and safety). The work is I1-I8.
- **R8. Auto-update design.** Resolved: ROADMAP D6. The policy uses
  `cargo-packager-updater` (FULL-package updates only, no deltas). The work
  is E8-E11; E5 is the installer, already done.
- **R9. Installer design.** Resolved: ROADMAP D6. The work is E5-E7
  (cargo-packager/NSIS), done and awaiting R12.

## E. Release engineering (ROADMAP M8)

E1-E4 have landed (awaiting check), as have E12 and E13. E5-E7 (the
cargo-packager installer) have landed, awaiting the R12 dry run; E8-E11
remain and have their own dependencies, stated in each task. Code signing
(R10), the first dry run (R12), and winget (R13) are maintainer work.

`.github/workflows/release.yml` runs on a `v*` tag. Its steps, in order:
checkout; `rustup toolchain install` (installs the pinned version from
`rust-toolchain.toml`) and `rustc -Vv`; "Check tag matches workspace
version" (compares `v<version>` of package `deskspace-app` from `cargo
metadata` with `GITHUB_REF_NAME`); "Test" (`cargo test --workspace
--locked`); "Build" (`cargo build --release --locked -p deskspace-app`, which
builds both `deskspace.exe` and `deskspace-watchdog.exe`); "Installer"
(`cargo install cargo-packager --locked --version 0.11.8`, then `cargo
packager -p deskspace-app --release --formats nsis`, producing
`deskspace_<version>_x64-setup.exe`); "Package" (stages `deskspace.exe`,
`deskspace-watchdog.exe`, `LICENSE`, `NOTICE`, `README.md` into
`deskspace-<tag>-x86_64-pc-windows-msvc/`, zips it, writes a lowercase
SHA-256); "Publish" (`gh release create
<tag> <zip> <zip>.sha256 <setup.exe> --title "DeskSpace <tag>"
--generate-notes`, plus `--prerelease` when the tag contains `-`). Read it
before starting any E task.

- [ ] **E12. Ship the watchdog in the release zip.** (awaiting check)
  - The takeover's crash restore needs `deskspace-watchdog.exe` beside
    `deskspace.exe` (`win32/watchdog.rs`, `arm_watchdog`), but the "Package"
    step copies only `deskspace.exe`, `LICENSE`, `NOTICE`, and `README.md`.
  - In `.github/workflows/release.yml`, "Package" step: add
    `target/release/deskspace-watchdog.exe` to the files copied into the
    staging folder, exactly like `deskspace.exe`. Change nothing else.
  - Done when: the workflow passes `actionlint` (or a careful read if it is
    not installed), and `CONTEXT.md` section 10 (the takeover and "No
    installer" bullets) and `ROADMAP.md` (M8 item 5, F-45, D8) no longer say
    the watchdog is missing. The real check is R12. **[needs human]**

- [ ] **E1. Commit an `.ico` asset and embed it, with version info, in the
  executable (ROADMAP M8 item 2).** (awaiting check) Landed as
  `feat: embed the app icon and version info (E1)`; `winresource` passed
  `cargo deny check`.
  - Asset: `assets/deskspace.ico` at the repository root. Generate it once
    from the existing drawing in `win32/icon.rs`, using `write_icon`'s ICO
    layout at sizes 16, 20, 24, 32, 40, 48, 64, and 256. Use a throwaway
    local change (for example a temporarily widened `FILE_SIZES` and a
    scratch test). Commit only the `.ico`, not the scratch code. From now on
    the `.ico` is the source of truth.
  - Build: add `winresource` as a build-dependency of `deskspace-app`
    (`rust_cargo-add` with `dependency_type = "build"`, so `Cargo.lock` stays
    current). In `crates/deskspace-app/build.rs`, inside the existing
    `windows` + `msvc` branch, compile a resource with
    `set_icon_with_id("../../assets/deskspace.ico", "1")`. `winresource`
    fills `FileVersion`, `ProductVersion`, and `ProductName` from Cargo
    metadata. Set `FileDescription` to "DeskSpace" and `LegalCopyright` to the
    line in the About page (`settings.rs`, `field::ABOUT_LICENSE`). Keep the
    manifest link args as they are. Add
    `cargo:rerun-if-changed=../../assets/deskspace.ico`.
  - If `rc.exe` is not found locally, or `cargo deny check` rejects
    `winresource` or its dependencies, **stop and ask**.
  - Do not change any runtime code; E2 does that.
  - Done when: `cargo build --release` produces a `deskspace.exe` that shows
    the icon in Explorer, and Properties > Details shows the workspace
    version. `cargo deny check` passes. **[needs human]** check: the icon at
    Explorer's small, medium, and extra-large views.

- [ ] **E2. Use the embedded icon at runtime and delete the drawn one.**
  (awaiting check) After E1. Landed as
  `refactor: use the embedded icon at runtime and delete the drawn one (E2)`;
  the desktop verb's icon and one leftover cleanup moved into
  `app/visibility.rs` (the entry is no longer in `app.rs`).
  - `win32/tray.rs`: build `tray_icon` / `shared_icon` from the resource
    with `LoadImageW(GetModuleHandleW(None), MAKEINTRESOURCE(1), IMAGE_ICON,
    cx, cy, LR_DEFAULTCOLOR)`. Use `GetSystemMetrics(SM_CXSMICON)` for the
    tray size. Keep the existing destroy-once rule (see `tray.rs` ~94).
    `settings_window.rs` ~247 already uses `shared_icon`; leave it unchanged.
  - `deskspace-app/src/app.rs` (the desktop "Show spaces" entry, ~620): drop
    the `write_icon` call and pass `"{exe}",0` as the icon. Delete a
    `deskspace.ico` left over beside the storage folder by older versions,
    ignoring errors.
  - Delete `win32/icon.rs`, the `write_icon` re-exports in `win32/mod.rs` and
    `lib.rs`, and the `TODO(deskspace)` there.
  - Update `CONTEXT.md` section 6 (icon) and section 10 (the "Show spaces"
    verb note that mentions `icon::write_icon`).
  - Done when: `grep -r write_icon crates` finds nothing. **[needs human]**
    check: the tray, the settings title bar, and the Shift+right-click desktop
    menu entry show the icon at 100 % and 150 % scaling.

- [x] **E13. Pin the Rust toolchain (ROADMAP D9).** Before E3.
  - `rust-toolchain.toml`: change `channel = "stable"` to the exact current
    stable version, written as `"1.MINOR.PATCH"` (run `rustc -V` with the
    current stable; at the time of writing it was 1.98.1). Keep the
    `components`, `targets`, and `profile` lines.
  - `.github/workflows/ci.yml` and `release.yml`: replace each
    `uses: dtolnay/rust-toolchain@stable` step with a step that installs the
    toolchain the file names, so the file stays the single source of truth:
    `run: rustup toolchain install` (with no argument, rustup 1.28 and newer
    installs the toolchain from `rust-toolchain.toml`, with its components
    and targets), followed by a `run: rustc -Vv` step so the log records the
    version. If the runner's rustup is older and the command fails, use
    `rustup show` instead, which installs it on older versions.
  - Do not change `rust-version` in `Cargo.toml`; it is the
    minimum supported version, a separate fact. (It moved from 1.85 to 1.88
    when the updater's `time` dependency needed a patched release, `F-45`.)
  - Run `cargo clippy --workspace --all-targets -- -D warnings` with the
    pinned version; it must pass unchanged.
  - Docs: `AGENTS.md` Prerequisites (say the toolchain is pinned in
    `rust-toolchain.toml` and installed automatically by rustup) and add a
    short "Bumping the toolchain" note there: change the version in
    `rust-toolchain.toml` alone, fix any new clippy lints in the same
    commit, and commit as `ci: bump the Rust toolchain to 1.x.y`.
    `README.md` Requirements: the pinned version is installed automatically
    by rustup.
  - Done when: CI runs show the pinned `rustc -Vv`, and
    `rg "rust-toolchain@stable" .github` finds nothing.

- [x] **E3. Reproducible release build (ROADMAP M8 item 5).** After E13,
  which pins the toolchain, so a rebuild uses the same rustc.
  - `.github/workflows/release.yml`: on the Build step, set
    `RUSTFLAGS: --remap-path-prefix=${{ github.workspace }}=. --remap-path-prefix=C:\Users\runneradmin\.cargo=cargo`.
    Then confirm with `strings`/`Select-String` that no runner path is left
    in the exe.
  - Make the linker deterministic: in `crates/deskspace-app/build.rs`, inside
    the msvc branch, add `cargo:rustc-link-arg-bins=/Brepro`. It zeroes the
    PE timestamp and replaces it with a content hash.
  - Check locally: build twice, from two clones in different directories,
    with the same `--remap-path-prefix` (relative to each clone), and compare
    `Get-FileHash` of both `deskspace.exe`. If they differ, compare the two
    `dumpbin /headers` outputs, record what differs under **Found while
    working**, and **stop and ask**.
  - Done when: the two hashes match. Add one line on how to reproduce a
    release build to the "No installer, no code signing, no auto-update" note
    in `CONTEXT.md` section 10.

- [x] **E4. Release checklist and versioning policy (ROADMAP M8 item 5).**
  - New `RELEASING.md` at the repository root. Link it from `AGENTS.md`
    (extend the existing "Release" row of the Commands table), from the
    `README.md` Documentation table, and from `ROADMAP.md` M8 item 5. Add
    `release:` to the commit types in rule 8 above and in `AGENTS.md`
    (Commits). Contents:
    - Versioning: SemVer on `workspace.package.version`. While below 1.0, the
      minor version follows ROADMAP section 6 (v0.2, v0.3, ...) and patch
      versions are fixes. Pre-releases use `-alpha.N` / `-beta.N` / `-rc.N`,
      which `release.yml` marks as pre-releases. The config schema version
      (`CURRENT_VERSION`) is independent of the app version.
    - Checklist: `main` is green in CI; bump the version in `Cargo.toml`
      and refresh `Cargo.lock` (`cargo check --workspace`); move the
      finished items in `ROADMAP.md` / `CONTEXT.md`; commit as
      `release: vX.Y.Z`; `git tag vX.Y.Z && git push origin vX.Y.Z`; watch
      the run with `gh run watch`; download the zip, check the `.sha256`, and
      smoke-test it with a scratch `--config`; edit the generated notes on
      the release page.
    - Recovering from a bad release: delete the release and the tag
      (`gh release delete vX.Y.Z --cleanup-tag`), fix, and publish the next
      patch version. Never re-use a published version number.
  - Done when: the checklist matches `release.yml` step for step (the tag
    check, `--locked`, the asset names; the step list is at the top of this
    section).

### Installer and auto-update (ROADMAP D6, F-45)

Read ROADMAP D6 before any of E5-E11; it is the spec, and these tasks only
say where things go. Confirm every API named below on docs.rs for the pinned
crate version: `https://docs.rs/cargo-packager-updater` for the updater and
`https://github.com/crabnebula-dev/cargo-packager` for the packager; if one
differs, **stop and ask**.

- [x] **E5. Configure cargo-packager (installer + uninstaller).**
  - In `crates/deskspace-app/Cargo.toml`, add a `[package.metadata.packager]`
    table: `productName = "DeskSpace"`, `identifier =
    "com.generalchad.despacespace"`, a publisher, description, and homepage,
    `formats = ["nsis"]`, an explicit `binaries` list with `deskspace` marked
    main and `deskspace-watchdog` (I6), and `resources = ["../../LICENSE",
    "../../NOTICE", "../../README.md"]`.
  - Add a `[package.metadata.packager.nsis]` table:
    `installMode = "currentUser"`, `installerIcon =
    "../../assets/deskspace.ico"`, and
    `template = "../../packaging/nsis/installer.nsi"`. These paths are
    `../../`-prefixed because cargo-packager changes into the package
    directory before packaging, so they are relative to
    `crates/deskspace-app`, not the repository root.
  - The `deskspace-update` crate is not needed for the installer:
    `cargo-packager` is a CI-only tool (installed in E6), never a workspace
    dependency, so no dependency is added here and the dependency graph stays
    unchanged. (E10 creates the update crate.)
  - Vendor the custom NSIS template at `packaging/nsis/installer.nsi` (from
    `cargo-packager` 0.11.8). It overrides the NSIS default install root, so
    the installer writes to `%LocalAppData%\DeskSpace.App` instead of
    `%LocalAppData%\DeskSpace`, the parent of the user's stored files (E7).
  - Run from the repository root: `cargo build --release -p deskspace-app`,
    then `cargo packager -p deskspace-app --release --formats nsis`. The
    result is `deskspace_<version>_x64-setup.exe`, published beside the
    portable zip in the same GitHub Release (E6).
  - Done when: the command produces the installer and it installs per user
    with no admin prompt. The real check is R12. **[needs human]**

- [x] **E6. Build the installer in `release.yml`.** After the Build step:
  `cargo install cargo-packager --locked --version 0.11.8` (a CI-only tool;
  the version is pinned to match the vendored template), then `cargo packager
  -p deskspace-app --release --formats nsis`. Find the built
  `target/release/*-setup.exe` (`deskspace_<version>_x64-setup.exe`) and add
  it to the `gh release create` assets alongside the portable zip and its
  `.sha256`.
  - Done when: the workflow passes `actionlint` (or a careful read if it is
    not installed). The real check is R12. **[needs human]**

- [x] **E7. Uninstall cleanup and an install-root safety test.** The Run-key
  part needs C4 (landed).
  - The custom NSIS template (`packaging/nsis/installer.nsi`, from E5)
    installs into `%LocalAppData%\DeskSpace.App` and, in `Section Uninstall`,
    deletes the `Run` value
    (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run\DeskSpace`, F-34) and
    the `HKCU\Software\Classes\DesktopBackground\Shell\DeskSpaceShowSpaces`
    key, and never touches the stored files or the config (D6).
  - Safety test in `deskspace-app`: using `ConfigStore::discover()`, assert
    that neither the config path nor the storage path is inside
    `%LOCALAPPDATA%\DeskSpace.App`. Add a pure helper `fn is_inside(path,
    root) -> bool` with its own tests (case-insensitive, and `DeskSpace.App`
    is not inside `DeskSpace`).
  - Done when: the test passes; R12 checks the uninstall on a clean VM.
    **[needs human]**

- [x] **E8. Cross-thread wake-up for the app (platform).** Landed:
  `Ui::waker` -> `Waker` in `runtime/mod.rs`, `controller::WAKE`
  (`WM_APP + 10`), and `Event::Wake { id }` in `input.rs`. Independent of
  E5-E7.
  - The update check (E10) runs on a worker thread and has to hand its
    result to the UI thread. Add a generic mechanism that knows nothing
    about updates:
    - `#[derive(Clone)] pub struct Waker` (`Send + Sync`), obtained from
      `Ui::waker()`. `Waker::wake(&self, id: usize)` posts `WAKE`
      (`WM_APP + 10`, already reserved by a comment in `controller.rs`; turn
      that comment into `pub(crate) const WAKE: u32 = WM_APP + 10;`) to the
      controller window with `id` in `wparam`.
    - Get the controller handle with `with_runtime(|runtime|
      runtime.controller)` in `Ui::waker`. `HWND` is not `Send`, so store it
      as an `isize`, the way `win32/folder_watch.rs` does.
    - The controller's window procedure (`controller.rs`, `wndproc`)
      dispatches it as a new `Event::Wake { id: usize }` in `input.rs`,
      documented like `Event::Timer`. Copy the `FOLDER_CHANGE =>` arm.
  - Copy the worker-thread `PostMessageW` call and its `// SAFETY:` wording
    from `win32/folder_watch.rs` ("a post to a destroyed window just
    fails"). A wake after the window is gone must just fail silently.
  - Re-export `Waker` from `win32/mod.rs` and `lib.rs`.
  - The app pairs it with a `std::sync::mpsc` channel it owns; the platform
    carries only the id.
  - Done when: `Event::Wake` is in `CONTEXT.md` section 6 (the Events list,
    and `WM_APP + 10` in the message table). No behavior change.

- [x] **E9. A three-button question dialog (platform).** Landed: `Ui::choose`
  uses `TaskDialogIndirect`. Independent.
  - `Ui::choose(&mut self, title: &str, text: &str, buttons: &[&str],
    default: usize) -> Option<usize>`: a modal `TaskDialogIndirect` owned by
    the same window as `Ui::confirm` (the settings window when it is open,
    else the controller), with custom buttons (`TDF_USE_COMMAND_LINKS` off).
    It returns the index of the button pressed, or `None` if the dialog was
    closed. The Common Controls v6 manifest in `build.rs` makes
    `TaskDialogIndirect` available.
  - Follow `Ui::confirm` (`runtime/ui_chrome.rs`) for ownership, foreground
    handling, and not holding a runtime borrow across the modal call: it
    reads the owner inside `with_runtime`, releases the borrow, then calls
    `SetForegroundWindow` and the modal API. `Win32_UI_Controls` is already a
    `windows` feature of the platform crate.
  - Link the `TaskDialogIndirect` page on Microsoft Learn beside the call.
  - Done when: it compiles and clippy is clean. **[needs human]** check: a
    temporary call shows three buttons and returns the right index (remove
    the temporary call before committing).

- [ ] **E10. Update settings, schedule, check, and prompt (F-45).** (awaiting
  check) Landed: `deskspace-update` wraps `cargo-packager-updater`; the
  `WindowSettings` fields, the General-page toggle and button, the tray entry,
  and `app/update.rs` (check, prompt via `Ui::choose`, skip). The pre-release
  channel is deferred (the GitHub "latest" feed never serves a pre-release). After
  E8 and E9.
  - New library crate `crates/deskspace-update` with
    `#![forbid(unsafe_code)]`, `thiserror` errors, and `tracing`. It depends
    on `cargo-packager-updater` (`rust_cargo-add`) and on nothing in the
    workspace. Add it to `[workspace.dependencies]` in the root `Cargo.toml`
    (`members = ["crates/*"]` picks the crate up) and make `deskspace-app`
    depend on it. Update `AGENTS.md` (workspace layout, dependency direction
    `app -> {config, platform, update} -> core`; `update` depends on no
    workspace crate) and `CONTEXT.md` section 3.
  - `cargo deny check` will need more licenses for `cargo-packager-updater`
    and its TLS stack (ISC, BSD-3-Clause, CDLA-Permissive-2.0, Zlib, ...)
    and may need an advisory ignore. Run `cargo deny check` right after
    `cargo add` and **stop and ask** before editing `deny.toml`; do not
    loosen it on your own.
  - Config, following "Recipe: a new global setting", all in
    `WindowSettings` with `#[serde(default)]`:
    `check_for_updates: bool` (false), `update_prereleases: bool` (false),
    `last_update_check: Option<u64>` (Unix seconds),
    `skipped_version: Option<String>`. Serde default tests for each.
  - Settings: on the General page, a "Check for updates automatically" toggle
    and an "Include pre-releases" toggle, both `Effect::Pending`. Under them,
    a note saying the check contacts GitHub, and only when enabled or when
    started from the tray.
  - `deskspace-update`:
    - `pub struct Available { version: String, notes: String, page_url:
      String }`.
    - `pub fn check(prerelease: bool) -> Result<Option<Available>,
      UpdateError>`, using
      `cargo_packager_updater::check_update(current_version, Config {
      endpoints, pubkey, .. })` with the feed endpoints and the minisign
      public key compiled in (R10, F-45). It is FULL-package only; there are
      no deltas.
    - Keep the version choice in a pure, tested function `fn newest(...)`
      adapted to the signed JSON feed: parse the feed in the test, pick the
      newest release newer than `CARGO_PKG_VERSION`, and honour `prerelease`
      (skip `-`-tagged versions unless it is on).
    - Blocking is fine here; the app calls it on a worker thread.
  - App. The code goes in a new `app/update.rs` (one `impl App` block, like
    the other `app/` modules; declare it in `app.rs`), except the constants
    and the `on_event` arms, which stay in `app.rs`:
    - `UPDATE_TIMER = 4` (reserved by the comment on `PREVIEW_TIMER` in
      `app.rs`; remove that note). On `Started`, when `check_for_updates` is
      on, arm it for 60 s, then hourly. When it fires and `fn check_due(last:
      Option<u64>, now: u64) -> bool` (24 h, tested) is true, start a check.
    - A check runs on `std::thread::spawn`, sends the result over the
      channel, and calls `waker.wake(UPDATE_WAKE)`. On
      `Event::Wake { id: UPDATE_WAKE }`, take the result, set
      `last_update_check`, and `schedule_save`. Keep one check in flight at
      a time (an `update_in_flight` flag).
    - Tray: a "Check for updates…" entry (new `menu::CHECK_UPDATES = 12`,
      the lowest free id; add it to `menu_ids_are_unique`) above the
      separator in `App::show_tray_menu` (`app/menus.rs`), always shown, and
      its arm there (not in `run_common_menu`, which the space menu shares).
      Follow "Recipe: a new menu entry". A
      manual check ignores `check_for_updates` and `skipped_version`, and
      reports "DeskSpace is up to date" or the error through `ui.choose`
      with one "OK" button.
    - On an available version (automatic checks skip `skipped_version`),
      show `ui.choose` with the version and the first ~600 characters of the
      notes:
      - Installed: "Update and restart" / "Later" / "Skip this version".
      - Portable: "Open release page" / "Later" / "Skip this version".
      - "Skip" stores `skipped_version`. "Later" does nothing. "Open release
        page" opens `page_url`. `Ui::open_path` cannot open a URL (it turns
        `/` into `\` and uses the item verb), so add `Ui::open_url(&mut self,
        url: &str)` in `runtime/ui_shell.rs` with a plain `ShellExecuteW` "open" in
        `win32/shell_items.rs`, keeping the `AllowSetForegroundWindow` step
        `open` already does.
      - "Update and restart" calls the E11 entry point. Until E11 lands,
        open the release page there and leave a `// TODO(deskspace): E11`.
  - Done when: the tests above pass and nothing contacts the network with
    the default config (check by reading the code path, then note it in the
    commit body). **[needs human]** check: a manual tray check on a portable
    build against a feed with a newer pre-release, with "Include
    pre-releases" on.

- [ ] **E11. Download, apply, and restart (installed builds).** (awaiting check)
  Landed as `deskspace_update::download_and_install`, run on a worker thread
  after the config is flushed; a portable build opens the release page instead.
  After E10.
  - `deskspace-update`: `pub fn download(&Pending) -> Result<(), _>` (call
    it on a worker thread) and `pub fn apply_and_restart(&Pending) -> !`
    (or a `Result`, if the API returns one), wrapping
    `cargo-packager-updater`'s download and install. Confirm the exact API
    (`Update::install()` / `download_and_install()`) on docs.rs at the pinned
    crate version, and **stop and ask** if it differs. `Pending` holds the
    update the E10 check found, so the app does not check twice.
  - App: "Update and restart" starts the download on a worker thread (same
    channel and `Waker` as E10, with a new wake id), and sets the tray
    tooltip to "Downloading update…" with `ui.set_tray_tooltip`. When the
    download finishes:
    - flush the configuration synchronously with `self.flush_save(ui)`
      (`app/persist.rs`; a pending autosave must not be lost);
    - unregister every hotkey (loop over `self.hotkeys`, as the
      `CloseRequested` arm in `app.rs` does);
    - remove the desktop verb (`remove_desktop_show_entry`);
    - call `deskspace_platform::restore_desktop_icons()` and
      `deskspace_platform::signal_watchdog_clean_exit()`, because
      `apply_and_restart` exits the process without running the platform
      teardown (`runtime/mod.rs`, end of `run`) that normally does both;
    - call `apply_and_restart`.
    Put the first three steps (flush, hotkeys, verb) in one `fn
    release_for_exit(&mut self, ui)` in `app/persist.rs` and call it from the
    `CloseRequested` arm too, so the two exit paths cannot drift. Keep the
    icon restore and the watchdog signal in the update path only; on a normal
    close the platform teardown does them.
    On failure, restore the tooltip and report the error with `ui.choose`.
  - `cargo-packager-updater` is FULL-package only (no deltas), which matches
    D6 ("apply on restart"): a downloaded update is applied on the next
    launch, or at once by `apply_and_restart`.
  - Use the same channel as E10 and a new wake id (`UPDATE_DOWNLOADED_WAKE`
    next to `UPDATE_WAKE` in `app.rs`).
  - Remove the E10 `TODO`. Mark F-45 "Done; check pending" in the ROADMAP
    feature index, and update `CONTEXT.md` (sections 6, 7, and 10).
  - **[needs human]** check during R12: an `alpha.1` install updates to
    `alpha.2` and comes back with the same spaces and settings.

## F. Ad-hoc features

Small requests, one commit each.

- [x] **F1. F2 renames the space title when no item is selected.** The
  selection-key watch (`win32/keyboard.rs`) now watches F2 whenever a space is
  current, and `App::begin_f2_rename` renames the focused item, or the current
  space's title (`begin_rename`) when the selection is empty. **[needs human]**
  check: click a space with no item selected and press F2 (title rename); with
  an item selected, F2 still renames the file.
- [x] **F2. Alt+Enter opens item Properties or DeskSpace Settings.** Added
  `Event::ShowProperties`, `Ui::show_properties` (the shell `properties` verb
  through `ShellExecuteExW`), and the `Alt+Enter` watch. Alt+Enter opens the
  focused item's Properties, or DeskSpace Settings when no item is selected.
  **[needs human]** check: with an item selected, Alt+Enter shows its
  Properties; with a space current and nothing selected, it opens Settings.
- [x] **F3. Enter opens the focused item.** Added the `SELECTION_KEY` message
  and `SelectionKey` event (with a `keyboard::set_menu_open` guard so an open
  popup keeps its own keys). **[needs human]** check: select an item and press
  Enter; it opens.
- [x] **F4. Escape clears the selection.** **[needs human]** check: with items
  selected, Escape clears them; Escape still closes an open menu.
- [x] **F5. Shift+Delete deletes permanently.** `Ui::delete_files` and
  `file_op::delete` take a `permanent` flag that drops `FOF_ALLOWUNDO`.
  **[needs human]** check: Shift+Delete does not go to the Recycle Bin.
- [x] **F6. F5 refreshes the spaces.** Re-reads Explorer's display options and
  re-flows every space.
- [x] **F7. The Menu key or Shift+F10 opens the context menu.** Anchored at the
  focused item's cell center, or the space center with no item selected.

- [x] **F8. Shift and Caps Lock decode predictably in inline rename.**
  (awaiting check) `win32/keyboard.rs` `char_for` read Caps Lock from
  `GetAsyncKeyState`'s low bit, which is an unreliable "pressed since the last
  call" flag rather than the toggle, and only ever set modifier bits, so a
  stale `GetKeyboardState` bit (frozen, because the thread never removes
  keyboard messages) could stick and force uppercase. It now reads each
  modifier's live state and sets or clears the bit, and reads Caps Lock's
  toggle from `GetKeyState`'s low bit. **[needs human]** check: in a rename,
  type with Shift, Caps Lock on, Caps Lock off, Caps+Shift, and toggling Caps
  mid-name; Shift+digit gives the symbol; no case gets stuck.

- [ ] **F9. Fit a resized space to whole columns.** (awaiting check) A
  user-sized space kept its dragged width, so `position_items` left-aligned the
  grid and the remainder landed on the right (the "Email" space: left padding
  12, right 24). `ResizeSpace` now rounds a horizontal resize down to whole
  columns (`GridSpec::fit_width` / `fit_rect_width`; the grabbed edge stays
  put), so both side paddings match. **[needs human]** check: drag a space's
  right edge and see it settle on whole columns with even left/right padding.
  **Result (2026-10-12):** the width fits whole columns with even side padding.
  The height is now fit to whole rows too, behind the General page's "Snap
  resized spaces to whole columns and rows" (`GridSpec.fit_to_grid`, default
  on); the grid it fits to is the existing `snap` / icon-spacing /
  edge-padding settings. **[needs human]** re-check: drag a right/bottom edge
  and see even padding on all sides; turn the toggle off and confirm a resize
  keeps the dragged size (still snapped to `snap`).
- [x] **F10. "Fit to contents" in the space settings.** (awaiting check) The
  space menu already offered "Fit to contents" (user-sized, unlocked spaces);
  the settings window now offers the same as a button on a space's Layout page.
  It is grey unless the space is resized and unlocked, and runs
  `Command::FitToContents` (immediate, like the menu; Cancel does not undo it).
  **[needs human]** check: select a resized space in Settings, open Layout,
  press "Fit to contents", and confirm the space grows to fit its items with
  even padding.
- [x] **F11. A dead zone before a border drag resizes a space.** (awaiting
  check) A press on a border started a resize at once, so a click or tiny
  jitter changed the size (and a corner grab changed both axes). The resize now
  waits until the pointer leaves a half-cell dead zone around the grab
  (`pointer::resize_edges_now`), and each axis joins on its own, so a
  horizontal move on a corner resizes only the width. **[needs human]** check:
  click a border without moving (no resize); drag a side past half a cell (only
  that axis resizes); drag a corner horizontally (only the width changes).
- [x] **F12. Check box coloring matches Windows.** (awaiting check) A checked
  item check box was a white box with a black tick. It is now filled with the
  Windows accent and shows a white tick, like Explorer: `Palette.check_fill`
  (default `theme::CHECK_ACCENT`, Windows blue) is set to the system accent in
  `surface_palette` and carried on `SpaceScene.check_fill`; the unchecked box
  keeps its light fill and border. **[needs human]** check: tick a desktop item
  in a space and compare the box to Explorer's; change the Windows accent and
  confirm the box follows it.

## F. Ad-hoc features (continued)

- [ ] **F14. A modern color picker (ROADMAP F-51).** (awaiting check) Landed as
  `feat: add a modern color picker (F-51)`, then redesigned on the Windows
  color dialog: the Colors page and the space menu's Appearance submenu open
  DeskSpace's own popup (`win32/color_picker.rs`) instead of the system
  `ChooseColor` dialog. It opens on a palette (a "Recent colors" row and a
  "Windows colors" grid), with the full saturation/value square, hue and alpha
  strips behind a "Custom colors…" toggle, plus a preview, a hex readout, and
  OK/Cancel. `deskspace-core` gained `Hsv`, `Color::to_hsv`, and
  `Hsv::to_color`; `Ui::pick_color` and the settings window both route through
  it. The popup paints opaque GDI and does not extend the DWM frame into the
  client, which had made it transparent. It draws text with the system message
  font (`system::message_logfont`), and both its swatches and the settings
  window's color previews are slightly rounded boxes
  (`color_picker::draw_color_box`, `settings_window::paint::on_draw_swatch`).
  Polish pass: the color field's swatch is itself the button, with an
  eyedropper glyph (`system::glyph_font`, `system::draw_glyph`) and no separate
  "Choose…" button; the picker paints into a DIB back buffer (no flicker) and
  composites its marker circles with per-pixel anti-aliasing; the check mark is
  an anti-aliased glyph; and the modal loop pumps every message so the owner is
  not marked "not responding".
  - **[needs human]** check, over a high-contrast window (`--on-top`) and in
    light and dark mode: the popup is opaque, rounded, and bordered; the
    palette highlights the selected swatch (smooth check mark) and a
    double-click confirms; the "Custom colors…" toggle reaches the square, hue,
    and alpha controls with a live, flicker-free preview and smooth marker
    rings; the Colors page's swatch button shows the color and an eyedropper
    and opens the picker; OK returns the color (custom tint/text and the Colors
    page fields) and Cancel leaves it unchanged; Enter accepts and Esc cancels;
    the scale is right at 100 % and 150 %; opening the picker does not hang the
    settings window.

## G. More Explorer-standard controls

Remaining gaps from the controls review. Do them in order.

- [x] **G1. Type-to-select, and Home/End/Page scroll.** Typing a letter moves
  the selection to the next item whose name starts with it (a short timeout
  buffer, as Explorer uses); Home/End/PageUp/PageDown scroll the space. Extend
  the selection-key watch and add a pure prefix-match helper with tests.
  Landed as `feat: type-to-select and Home/End/Page scrolling (G1)`.
  **[needs human]** check: on the desktop shell, type a letter to jump to an
  item and watch the buffer expire and start fresh; press Home/End/PageUp/
  PageDown over a scrolled space.
- [x] **G2. Ctrl+wheel changes the icon size.** Add `ctrl` to
  `SpaceEvent::Wheel` and adjust the icon size the way Explorer's desktop does,
  clamped to `ICON_SIZE_RANGE`. Landed as
  `feat: change the icon size with Ctrl+wheel (G2)`. The standard behavior was
  chosen: it changes the global `Theme.icon_size` (10 % a notch), so spaces
  that follow Global resize together while a customized space keeps its own
  size; it re-lays out the spaces and saves. **[needs human]** check: Ctrl+wheel
  over a space resizes the icons of every non-customized space, clamped at
  50 %/200 %.
- [x] **G3. Arrow-key selection navigation.** Grid-aware Left/Right/Up/Down with
  Shift to extend and Ctrl to move focus, scroll the focused item into view, and
  draw a focus rectangle. Needs its own movement model; the largest item here.
  Landed as `feat: arrow-key selection navigation (G3)`. The hook posts
  `NAV_KEY` (yielding `Event::Navigate`); `next_in_direction` (a pure helper
  with tests) picks the nearest item by primary-axis distance, tie-broken by the
  cross axis; the app moves the selection, extends it (Shift, in item order like
  Shift+click), or moves a `focused` item (Ctrl); `SpaceScene.focused_item`
  draws a dotted focus rectangle. **[needs human]** check: with the desktop
  shell foreground and a space holding a few items, arrow around the grid,
  Shift+arrow to extend, Ctrl+arrow to move only the dotted focus rectangle and
  scroll it into view.
- [x] **G4. Slow double-click renames.** Clicking an already-selected item's
  label again starts an inline rename, as Explorer does.

## G. More Explorer-standard controls (continued)

- [ ] **G7. Accept non-file shell drags as shortcuts (F-50).** (awaiting check)
  Landed: a space's drop target accepts a `CFSTR_SHELLIDLIST` drag in addition
  to `CF_HDROP` (`win32/drop_target.rs`, `DragKind`), reads the CIDA into
  absolute ID lists with Explorer names, and raises
  `SpaceEvent::ShellItemsDropped` (`input.rs`, `ShellItem`). The app writes a
  `.lnk` per item into the space storage via the new `win32/shortcuts.rs`
  `create_shortcut` (`IShellLinkW::SetIDList` + `IPersistFile::Save`) under a
  clash-free name (`ItemStore::unique_path`) and lists it
  (`App::add_shortcuts`, `app/files.rs`). Files take precedence when a drag
  carries both formats. **[needs human]** check: drag a Start-menu tile and a
  taskbar app onto a space; each appears as a shortcut and opening it launches
  the app (including a packaged app with no file path), the cursor is not the
  red no-drop one, and dragging a normal file still moves the file.

## H. Per-space looks in the settings window (ROADMAP M3, F-46)

The settings window has three panes: the **scopes** (Global, then every
space indented under it, dimmed while it follows Global), the **pages** of the
selected scope, and the **page**, under a header with the scope's name and,
for a space, a "Customize this space" switch. A space shows only the look
pages (Appearance, Labels, Layout, Colors, Advanced), greyed with the global
values until the switch is on. CONTEXT.md sections 4 and 6 describe it.

Design rules (decided 2026-10-10, replacing the first rail design the same
day; do not re-open without asking):

- **A customized space has its own copy of every look setting** (`SpaceLook`:
  the theme plus icon spacing and edge padding), seeded from Global the first
  time. Turning the switch off keeps the copy, and turning it on again
  restores it.
- **The page shape never changes.** Every page (nine today) is always built; the app
  chooses which ones the pages pane lists and fills them with the scope's
  values, so the in-place update machinery keeps working.
- **The platform knows nothing about spaces.** It draws the scopes, pages,
  and header from `SettingsNav` and reports `ScopeChanged`, `NavChanged`, and
  `HeaderSwitch`; the app maps indices back to spaces.
- **Space edits go through `Command`s** (`SetSpaceLook`, `CustomizeSpace`);
  `settings.rs` stays pure and returns them. Global edits write `Config`
  directly, as before.
- **Context-menu edits commit immediately.** Each one customizes the space,
  starting from what it shows now. If one lands while the window is open, it
  also updates that space's Cancel snapshot, so Cancel never undoes it.
- ROADMAP M3's planned page of defaults for new spaces must be titled "New
  spaces", so it is not confused with the scopes pane.

- [x] **H1-H3. The first design (a rail with a Spaces group of customized
  spaces and one shared Space page with four overrides).** Built, then
  replaced by the three-pane design above at the user's request, because the
  Spaces entries read as more global pages. Commits `08c6ec9`..`a35ee61`,
  replaced by `283b7fb` and `38eac47`.
- [ ] **H5. Three panes, the Customize switch, and per-space looks.**
  (awaiting check) Landed as `283b7fb` (platform) and `38eac47` (model and
  app). **[needs human]** check, with two spaces and the window open on a
  high-contrast background (`--on-top`):
  - light and dark mode: the panes, the hairlines, the dimmed rows, and Tab
    and the arrow keys moving through both panes;
  - a space greyed until "Customize this space" is ticked; an edit then
    changes that space alone, live; Cancel puts it back;
  - turning the switch off and on again restores the space's custom values;
  - a menu edit (for example Opacity 60%) with the window open customizes the
    space, un-dims its row, and survives Cancel;
  - an older config with per-space `icon_size`/`tint` opens with that space
    customized and looking the same.

- [ ] **H4. Flash the selected space.** After H5. (awaiting check)
  - `render/scene.rs`: add `SpaceScene.highlight: bool` (default false).
    When it is set, `d2d/` draws a 2 px outline just inside the frame,
    following the corner radius.
  - Color: the space's `palette_for(id).selected_fill` at full alpha, so it
    follows the Windows accent when the space uses it.
  - Landed: `watch_highlight` / `clear_highlight` in
    `app/settings_session.rs`.
  - App: `highlighted: Option<SpaceId>`, and `HIGHLIGHT_TIMER = 5` (E10
    reserves 4 for `UPDATE_TIMER`; `PREVIEW_TIMER` is 6) at about 1200 ms.
    `SettingsEvent::ScopeChanged` to a space, and `open_settings_with` on a
    space, start it: set the flag and refresh that space. The timer clears it
    and refreshes again. Selecting another space moves the highlight. Cancel,
    OK, and closing the window clear it.
  - Done when: a scene test shows the flag reaching the scene (it does,
    `the_settings_highlight_reaches_the_scene`). **[needs human]** check: a
    space behind other windows is hard to see, so check with `--on-top` that
    the outline is visible on Acrylic (checked, visible), Clear, and Solid
    backdrops.

## I. M2 desktop takeover (ROADMAP M2, D7/D8)

Design is in `ROADMAP.md` D7 (model) and D8 (placement and safety); these
tasks only say where things go. Do them in order. Hiding Explorer's icons
(I5) stays behind an off-by-default setting and the hidden `--takeover` flag
until I5-I6 all land, so no partial build hides the desktop.

- [x] **I1. Read the desktop's items and positions through the shell.**
  - `crates/deskspace-platform/src/shell.rs`: `DesktopEntry` gained
    `position: Option<Point>`. `enumerate_desktop_items` now prefers the
    desktop view (`crate::win32::shell_view::desktop_shell_view()` cast to
    `IFolderView`): `ItemCount(SVGIO_ALLVIEW)`, then per index
    `IFolderView::Item(index)` for the child PIDL and
    `IFolderView::GetItemPosition(pidl)` for the point (`Item`, not
    `ItemPosition`; the PIDL comes from `Item`, so no
    `SHGetIDListFromObject`). The `IShellItem` for the label and kind is built
    with `SHCreateItemWithParent`, its path read with `SIGDN_FILESYSPATH`
    (virtual items such as This PC are skipped, per D1). Falls back to the
    existing name-sorted folder walk with `position: None` when the view is
    unavailable (for example before OLE is up). The fallback filter is the
    pure `keep_attributes`.
  - `crates/deskspace-app/src/app.rs`: `App` caches the entries in
    `desktop_entries`, refreshed on `Started` (after OLE) with a `debug` line;
    the M2 layer consumes them in I3.
  - Tests: `keep_attributes_filters_hidden_protected_and_desktop_ini`. The COM
    path is exercised at runtime (the `debug` line), since CI has no desktop.
  - Done when: every entry carries a position when the view is readable, and
    a normal config behaves as before apart from item order. Landed in
    `feat: read desktop items and positions through the shell (I1)`.

- [x] **I2. Add `DesktopLayer` to the model (core).**
  - New `crates/deskspace-core/src/desktop_layer.rs`:
    `DesktopLayer { items: Vec<Item> }`, `#[serde(default)]`, `Default`, with
    `item`, `item_at(position, cell)`, `push_item`, `remove_item`,
    `replace_items`, `item_count`, `items`. Document that a layer item's
    `position` is **absolute** (virtual-desktop coordinates), unlike a space
    item's content-relative position.
  - `crates/deskspace-core/src/desktop.rs`: add
    `#[serde(default)] pub layer: DesktopLayer`; make `next_item_id` and
    `item` cover both spaces and the layer; add `layer()` / `layer_mut()`.
    `space_containing`, `space_at`, and the space helpers stay space-only.
  - `crates/deskspace-core/src/command.rs`: add `SyncDesktop { items:
    Vec<Item> }` (reconcile by path: reuse an existing id for a path, allocate
    one for a new path, drop gone paths; order follows `items`),
    `MoveToDesktop { item: ItemId }`, and `SetDesktopItemPosition { item,
    position }`. `AddItem` / `MoveItem` (into a space) are unchanged.
  - Tests: `SyncDesktop` keeps ids by path across a reorder; serde default is
    empty; `next_item_id` spans spaces and the layer; `MoveToDesktop` moves
    from a space to the layer and returns `NotFound` for a missing id.
  - Update the Commands list in `CONTEXT.md` section 4.

- [ ] **I3. Draw the desktop layer on a chrome-less hosted window.** (awaiting check)
  - `render/scene.rs`: give `SpaceScene` / `SpaceStyle` a `chrome: bool`
    (title bar and border on or off) so the painter needs no new path; the
    desktop layer's scene has no title, no border, and no scroll.
  - Platform: `Ui::create_desktop_layer(rect)`, `set_desktop_layer_rect`,
    `set_desktop_layer_scene`, `destroy_desktop_layer`; a `WS_CHILD` of the
    host (reuse `space_window` with `chrome: false`), opaque, spanning the
    virtual desktop, behind the spaces.
  - App: build the layer scene from `config.desktop.layer` (items that
    intersect the virtual desktop, no scroll) and push it when it changes;
    rebuild after a `SyncDesktop` and on a display or DPI change.
  - Gate with a hidden `--takeover-layer` flag. Additive: Explorer still
    draws its icons, so the layer is only for an on-screen look.
  - Tests: a layer scene-builder test showing items reach the scene.
  - **[needs human]** check: `--on-top --takeover-layer` over a high-contrast
    window shows the loose icons at the right positions.
    Clean up the scratch config afterwards.

- [x] **I4. Generalize item interaction to a `Surface` (app refactor).**
  - `crates/deskspace-app/src/app.rs`: add
    `enum Surface { Space(SpaceId), Desktop }`; replace `selected_space:
    Option<SpaceId>` with `selected_surface: Option<Surface>`; route
    `item_at`, `select_at`, `update_hover`, `active_items`,
    `select_all_in_space`, the pointer / drag / drop paths, and the scene
    build through `Surface`. Resolve each surface's items, rect, layout, and
    scene through one accessor rather than `config.desktop.space(id)`
    directly.
  - Keep spaces behavior identical; this is the R2-style refactor, done as
    pure moves where possible.
  - Done when: all existing tests pass unchanged and the desktop layer
    responds to click, rubber-band, drag, drop, and open.
  - **[needs human]** check: on the layer, select and drag an icon; drop a
    loose item onto a space and confirm the file moves.

- [x] **I5. Hide and restore Explorer's icon layer (platform).**
  - New `crates/deskspace-platform/src/win32/desktop_icons.rs`: `hide()`
    finds every `SHELLDLL_DefView` (enumerate `Progman` and the wallpaper
    `WorkerW` children, plus top-level windows) and hides its `SysListView32`
    child (`ShowWindow(SW_HIDE)`); `restore()` shows them again. Track the
    handles; both are idempotent.
  - Config: a `WindowSettings.take_over_desktop: bool` (default false),
    following "Recipe: a new global setting": a General-page toggle with
    `Effect::Pending` and a serde default, applied through
    `desktop_icons::hide` / `restore`. Add the hidden `--takeover` flag that
    forces it on for development.
  - Wire: hide after the layer is drawn and the setting is on; restore before
    `OleUninitialize` (add to the `run` teardown in `runtime/mod.rs`); re-hide on
    `TaskbarCreated` (Explorer restart) and on a low-frequency health timer
    (next free timer id; E10 uses 4, HIGHLIGHT 5, PREVIEW 6).
  - Update `CONTEXT.md` sections 6 and 10 (the "No desktop icon
    integration" limitation) and the `ROADMAP.md` F-01 row.
  - Done when: clean exit restores the icons, and restarting Explorer brings
    them back.
  - **[needs human]** check: enable takeover, confirm the strip; exit and
    confirm the icons return; kill and restart Explorer and confirm the strip
    re-hides.
  - Implemented as decided during the work: `hide`/`restore` re-enumerate on
    every call instead of tracking handles, so a listview Explorer recreates is
    restored rather than a stale handle being shown; enabling takeover also
    draws the layer (no blank desktop); the health timer is the app's id 8,
    armed only while takeover is on.

- [x] **I6. Crash-safe restore (watchdog helper).** After I5.
  - A helper process started before the icons are hidden. Either
    `deskspace.exe --watchdog <pid>` handled in `main`, or a small dedicated
    bin. It waits on the main process and, if the main process exits without
    the clean-exit signal, re-shows every `SHELLDLL_DefView` listview and
    exits.
  - Clean-exit signal: the main process sets a named event (or passes an exit
    code) on a clean shutdown; the watchdog restores only when it is absent.
    Keep the check a pure, tested function (`fn is_watchdog(args) -> bool`
    and the clean/crash decision). Add a single-instance guard so two
    watchdogs do not fight.
  - The watchdog needs no workspace dependency; it may reuse
    `desktop_icons::restore` if it links the platform crate, otherwise carry
    a minimal copy.
  - Update `CONTEXT.md` section 6.
  - **[needs human]** check: enable takeover, kill `deskspace.exe` from Task
    Manager, and confirm the desktop icons come back.
  - Landed in `crates/deskspace-platform/src/win32/watchdog.rs` and the
    `deskspace-watchdog` binary (`crates/deskspace-app/src/watchdog_main.rs`):
    a separate executable with a distinct name, launched detached, so a bulk
    kill of `deskspace.exe` by image name or a closed console leaves it
    running. It re-parents itself once (`respawn_reparented`) so ending the
    app's process tree cannot reach it either. The app calls `arm_watchdog`
    (per-pid clean-exit event) just before hiding the icons, and the runtime
    teardown signals the event right after `desktop_icons::restore`. The
    watchdog polls the event once the parent exits and restores the icons only
    when it was never signaled; a per-pid named mutex keeps a stray watchdog a
    no-op. Pure `watchdog_pid` / `is_watchdog` / `should_restore` are tested;
    the process wait and restore need the on-screen check.
    - Packaging note: the release must ship `deskspace-watchdog.exe` beside
      `deskspace.exe` in both the cargo-packager installer and the portable zip
      (see E6); without it the takeover silently loses crash safety.

- [x] **I7. Win+D watcher - closed as not needed (2026-10-11).**
  - The premise did not reproduce. On Windows 11 build 26300, Show Desktop
    (`MinimizeAll`) skips `WS_EX_TOOLWINDOW` windows, and a space is
    `WS_POPUP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`, so a top-level space is
    left visible. Verified with the running app (top-level, `--on-top`,
    `--takeover`, `--takeover-layer`, and both hosted placements, on both
    monitors) and with an isolated style matrix; a normal window in the same
    run was minimized (foreground went to `Progman`). A watcher would have been
    a no-op, so no watcher and no config toggle were added. A later commit
    (`996a939`) made spaces ignore `WM_SYSCOMMAND`/`SC_MINIMIZE`
    (`space_window.rs`), so survival no longer depends on the tool-window
    exclusion.
  - Done: the finding is recorded in `ROADMAP.md` D8 and the 2026-10-11
    progress entry, and in `CONTEXT.md` sections 2, 9, and 10. No `[needs
    human]` check remains.

- [ ] **I8. Watch the Desktop folder (platform).** Shared with M4 phase 2.
  (awaiting check)
  - `ReadDirectoryChangesW` on the Desktop folder, debounced about 100-150 ms,
    off the UI thread. Post a wake to the controller (reuse E8's `Event::Wake`
    if it has landed, else a new `WM_APP` id) and, on the app side,
    re-enumerate and `SyncDesktop`.
  - Build it so lenses can reuse it (M4 phase 2 watches a bound folder the
    same way). Note the reuse in `CONTEXT.md` section 6.
  - Tests: the debounce / coalesce helper as a pure function.
  - Done when: creating or deleting a file on the desktop updates the layer
    within about 150 ms.
  - Landed in `crates/deskspace-platform/src/win32/folder_watch.rs`: a
    reusable `FolderWatch` over `ReadDirectoryChangesW` (overlapped, one
    worker thread), with the debounce in a pure `Coalescer`; it posts
    `WM_APP + 14` (`controller::FOLDER_CHANGE`), which dispatches
    `Event::FolderChanged`. The app watches the Desktop folder while the
    layer is drawn (`App::watch_desktop_folder`) and re-runs
    `resync_desktop_layer` on the event. `Win32_System_IO` joined the
    `windows` features. It took its own message id (14), so E8's `Event::Wake`
    is not needed for it.
  - **[needs human]** check: with takeover on, create, rename, and delete a
    file on the desktop from Explorer or a terminal; the layer follows within
    about 150 ms.

## K. Split the large files (finished splits)

- [x] **K1. Split `win32/settings_window.rs` (~2,600 lines).** Do B8 first;
  it deletes the tab code, so there is less to move.
  - `settings_window/mod.rs` keeps: the constants, `FieldControls`,
    `SettingsState` and its impls, `create`, `set_dark`, `theme_controls`,
    `bring_to_front`, `window_style`, `ensure_classes`, `state_of`,
    `with_state`, `settings_event`, `wndproc`, and `page_wndproc`.
  - `build.rs`: `Builder` and its impl, `build`, `rail_list`, `fill_list`,
    `scaled`, `create_fonts`.
  - `update.rs`: `Update`, `update`, `plan_field`, `set_swatch`,
    `apply_header`, `apply_enabled`, `sync_selection`.
  - `nav.rs`: `NavRow` and its impl, `nav_rows`, `row_entry`, `entry_row`,
    `on_nav_changed`, `on_scope_changed`, `on_header_switch`.
  - `paint.rs`: `on_paint`, `erase_body`, `on_draw_scope`, `on_draw_item`,
    `on_draw_header_button`, `blend`.
  - `commands.rs`: `on_field_command`, `pick_color`, `on_scroll`,
    `show_page`, `window_text`, `colorref`, `from_colorref`.
  - `runtime.rs` calls `settings_window::{create, update, set_dark,
    bring_to_front, pick_color}`; keep those paths working.

- [x] **K2. Split `render/d2d.rs` (~1,475 lines).**
  - `d2d/mod.rs` keeps: the drawing constants, `PreparedScene`, `Painter`
    and its impl, `PaintFrame`, `release_graphics`, `render_items`.
  - `graphics.rs`: `Graphics` and its impl, `graphics`,
    `character_trimming`, `make_format`, `first_installed`, and the
    thread-local that holds `Graphics`.
  - `item.rs`: `draw_item`, `draw_check_box`, `draw_marquee`.
  - `edit.rs`: `draw_edit_title`, the `EDIT_*` constants, `EditLine`,
    `edit_lines`, `draw_edit_buffer`, `edit_field`, `draw_edit_text`,
    `utf16_index`.
  - `bitmap.rs`: `decode_bitmap`, `to_icon`.
  - `geometry.rs`: `fill_rounded`, `draw_text`, `color_f`, `rect_f`,
    `to_rect_f`, `inset`.
  - Callers outside the module use `d2d::{Painter, PreparedScene,
    PaintFrame, render_items, decode_bitmap, release_graphics}`; keep them.

- [x] **K3. Split `deskspace-app/src/settings.rs` (~1,820 lines, ~1,060
  without tests).** It is pure, so this is the easiest split to verify.
  - `settings/mod.rs` keeps: `Effect`, `build_form`, `scope_index`,
    `scope_at`, `scope_space`, `index_of`, `labels`, `LOOK_PAGE_COUNT`, and
    the `pub use` lines below.
  - `ids.rs`: `pub mod field` and `pub mod page` (re-exported from
    `mod.rs` as `pub use ids::{field, page};` so `settings::field::X` still
    works).
  - `tables.rs`: `MODES`, `BACKDROPS`, `LABEL_LINES_OPTIONS`,
    `LABEL_SHADOW_OPTIONS`, `PRESETS`, `preset_index`, `corner_marks`,
    `HOTKEY_HINT`, `HotkeyBinding`, `HOTKEY_BINDINGS`, `hotkey_setting`,
    `set_hotkey_setting`, `is_hotkey_field`.
  - `pages.rs`: `look_pages`, `general_page`, `new_spaces_page`,
    `hotkeys_page`, `about_page`.
  - `apply.rs`: `apply_look_field`, `apply_field`, `space_edit`,
    `customize_space`, `match_global`, `reset_appearance`.
  - `session.rs`: `SpaceSnapshot`, `Session`, `snapshot_spaces`,
    `needs_relayout`, `needs_restyle`.
  - Re-export every item the app uses from `mod.rs` (search
    `settings::` in `crates/deskspace-app/src`), so no caller changes.
  - Update the recipe "a new global setting" above to name the new files.

- [x] **K4. Split `win32/runtime.rs` (~940 lines).** `Ui`'s methods move
  into several `impl Ui` blocks, one file per area; the fields stay
  private (child modules can read them).
  - `runtime/mod.rs` keeps: `EventHandler`, `RunOptions`, `Runtime`, the
    thread-local, `with_runtime`, `dispatch`, `dispatch_close_once`,
    `tray_tooltip`, `is_space_window`, `space_dpi`, `settings_closed`,
    `destroy_ui_windows`, the `Ui` struct, `desktop_to_screen`,
    `space_hwnd`, `settings_hwnd`, `warn_on_shape_change`, `run`,
    `message_loop`, `init_common_controls`.
  - `ui_spaces.rs`: `create_space` through `destroy_desktop_layer`
    (`destroy_space`, `space_ids`, `set_space_rect`, `set_space_scene`,
    `set_space_cursor`, `set_space_style`, `set_spaces_visible`, the four
    desktop-layer methods), plus `show_drag_ghost`, `move_drag_ghost`,
    `hide_drag_ghost`, `with_ghost`.
  - `ui_input.rs`: `watch_outside_clicks`, `watch_folder`,
    `unwatch_folder`, `set_timer`, `clear_timer`, `capture_text_input`,
    `watch_selection_keys`, `register_hotkey`, `unregister_hotkey`.
  - `ui_shell.rs`: `open_path`, `show_properties`, `item_menu`, `drag_out`,
    `set_clipboard_files`, `clipboard_files`, `clear_clipboard`,
    `transfer_files`, `delete_files`, `rename_path`, `show_info_tip`,
    `hide_info_tip`, `with_info_tip`, `on_desktop_click`.
  - `ui_chrome.rs`: `pick_color`, `popup_menu`, `set_ui_dark`,
    `open_settings`, `update_settings`, `close_settings`, `confirm`,
    `is_settings_open`, `set_tray_tooltip`, `quit`.
  - Keep each method's doc comment with it. The module header (the borrow
    rules) stays in `mod.rs`.

- [x] **K5. Move drag out of `app/pointer.rs` (~1,080 lines, ~920 without
  tests).**
  - New `app/drag.rs` (declare it in `app.rs` next to `mod pointer;`) with
    one `impl App` block holding `drag_to`, `end_drag`, `begin_drag_visual`,
    `end_drag_visual`, `reorder_item_at`, `drag_item_out`, and
    `move_desktop_item`, plus the `Drag` enum (make it `pub(super)`;
    `PointerState` keeps its field of that type).
  - `pointer.rs` keeps mouse dispatch (`on_space_event`,
    `begin_interaction`), `select_at`, hover, double-click, marquee,
    scrolling, `adjust_icon_size`, the cursor and resize helpers, and
    `PointerState`.
  - Add `drag` to the module table in `CONTEXT.md` section 3.

- [x] **K6. Move `shell.rs` under `win32/` and split it (~820 lines).** It
  holds `unsafe` Win32 code, which AGENTS.md keeps under `win32/`.
  - `win32/desktop_items.rs`: `DesktopEntry`, `enumerate_desktop_items`,
    `enumerate_via_view`, `enumerate_via_folder`, `listed_by_explorer`,
    `keep_attributes`, `filesystem_path`, `display_name`, `fallback_label`,
    `desktop_directory`, `notify_moved`.
  - `win32/icons.rs`: `IconCache`, `CachedIcon`, `load_icon`,
    `is_shortcut`, `load_classic_icon`, `load_base_icon`,
    `decode_icon_masked`, `overlay_index`, `load_overlay_image`,
    `scale_overlay_to_corner`, `system_image_list`, `resample`,
    `composite_overlay`.
  - Delete `shell.rs` and `pub mod shell` in `lib.rs`; change the `lib.rs`
    re-exports (`DesktopEntry`, `IconCache`, `desktop_directory`,
    `enumerate_desktop_items`, `notify_moved`) to point at the new modules.
    The app uses only those re-exports, so it does not change. Check with
    `rg "crate::shell" crates` that nothing in the platform crate still uses
    the old path (nothing did when this task was written).
  - `shell.rs` calls `crate::render::d2d::decode_bitmap`; keep that call
    as it is (R3 decides about the two decoders).
  - Update the module map in `CONTEXT.md` section 6 and the R3 entry above.

- [x] **L2. Decide a drop by what the source allows; a link makes a
  shortcut.** After L1's findings.
  - Rule, matching Explorer: a drop whose effect is `LINK` creates a
    shortcut, whatever formats the drag carries. A space picks `LINK` when the
    source offers neither `MOVE` nor `COPY` (Start, the taskbar), or when Alt
    is held and `LINK` is allowed (Explorer's create-shortcut modifier;
    Explorer also uses Ctrl+Shift, but Shift means keep-in-place on a space,
    so use Alt alone). Otherwise files move (Shift keeps them) as today, and a
    drag with only shell items prefers `LINK` as `shell_drop_effect` does.
  - Pure, tested helper in `drop_target.rs`, replacing the three copies of the
    `match kind { .. }` effect choice in `DragEnter`, `DragOver`, and `Drop`:

    ```rust
    /// What a drop onto a space does.
    enum DropPlan { None, MoveFiles, KeepFiles, Shortcuts }

    fn drop_plan(kind: DragKind, keys: MODIFIERKEYS_FLAGS, allowed: DROPEFFECT)
        -> (DropPlan, DROPEFFECT)
    ```

    `DragKind` keeps meaning "what the drag carries"; `drop_plan` decides.
    Tests: files with every effect allowed (move), files with Shift (keep),
    files with only `LINK` (shortcuts), files with Alt and `LINK` allowed
    (shortcuts), shell items only (shortcuts, `LINK` preferred), nothing
    allowed (none). `desired_effect` and `shell_drop_effect` either become
    private helpers of `drop_plan` or go; keep their tests passing or move
    them onto `drop_plan`.
  - `Drop`: for `DropPlan::Shortcuts`, read the items with `shell_items(data)`
    whether or not the drag carries `CF_HDROP`
    (`SHCreateShellItemArrayFromDataObject` reads either), raise
    `ShellItemsDropped`, and do **not** set "Performed DropEffect" (nothing
    was moved). `MoveFiles` / `KeepFiles` keep today's `FilesDropped` path.
  - Copy a shortcut instead of linking to it. A `.lnk` pointing at another
    `.lnk` is what Explorer avoids; it copies the link. Add
    `pub path: Option<PathBuf>` to `ShellItem` with a `///` doc (filled from
    `SIGDN_FILESYSPATH` in `shell_item`; `None` for virtual items). In
    `App::add_shortcuts`, when `path` has a `lnk` or `url` extension (a pure
    `fn is_link_file(path: &Path) -> bool`, case-insensitive, with a test),
    `std::fs::copy` it to the unique store path instead of calling
    `create_shortcut`. The source file is never moved or deleted.
  - Docs: `CONTEXT.md` section 6 (the file-drops bullet) and section 8 (the
    drop bullet), `README.md` Controls (Alt-drag creates a shortcut), and the
    F-50 row in the ROADMAP feature index.
  - Done when: the tests pass. **[needs human]** check: every source from L1
    lands in a space as a working shortcut, with the link cursor; the Start
    entry is still in Start afterwards; Alt-dragging a file from Explorer
    makes a shortcut and leaves the file; a plain drag still moves the file
    and Shift still keeps it in place.
  - Amended (maintainer, 2026-10-14, after L1's first finding): a
    `CF_HDROP` counts as files only when every entry is an absolute path
    (`is_file_list`), because the Start menu offered `MOVE` with a file list
    of AppUserModelIDs; such a drag is shell items. Landed in
    `fix: make start-menu drags into spaces shortcuts (L2)`.

- [x] **L3. A picker for apps (platform).** Independent of L1-L2.
  - New `win32/pickers.rs` (add it to the file map above and to the module
    table in `CONTEXT.md` section 6, under Shell) with
    `pub(crate) fn pick_apps(owner: HWND) -> Vec<ShellItem>`, exposed as
    `Ui::pick_apps(&mut self) -> Vec<ShellItem>` in `runtime/ui_shell.rs`. An
    empty Vec means the user cancelled.
  - Spike first: an `IFileOpenDialog` (`FileOpenDialog` CLSID) with
    `FOS_ALLNONSTORAGEITEMS | FOS_ALLOWMULTISELECT | FOS_NOVALIDATE`, titled
    "Add apps", opened on the Applications folder
    (`SHGetKnownFolderItem(&FOLDERID_AppsFolder, ..)`, then `SetFolder`), and
    `GetResults` for the chosen items. If the dialog cannot show or return
    Applications-folder items (try a Store app and a desktop app), **stop and
    ask**: the fallback is a DeskSpace list window over the Applications
    folder, which needs design.
  - A chosen `IShellItem` becomes a `ShellItem` the way `drop_target.rs`'s
    `shell_item` already does it: move `shell_item` and `display_name` into
    `win32/shortcuts.rs` as `pub(crate)` and call them from both places (a
    pure move, never a second copy).
  - Modal rules: copy `Ui::confirm` (`runtime/ui_chrome.rs`): read the owner
    (the settings window when open, else the controller) inside
    `with_runtime`, release the borrow, then show the dialog. Link the
    Microsoft Learn pages for `IFileOpenDialog` and `FILEOPENDIALOGOPTIONS`
    beside the calls; every `unsafe` block gets a `// SAFETY:` comment.
  - Done when: it compiles, clippy is clean, and a temporary call (removed
    before committing) returns names and ID lists. **[needs human]**
  - Landed differently (maintainer, 2026-10-13): no spike; the maintainer
    chose the DeskSpace list window up front (`win32/app_picker.rs`: check
    boxes, a search box, Add / Cancel), with shared modal plumbing in
    `win32/dialog.rs`. `Ui::pick_apps(title)` lives in
    `runtime/ui_chrome.rs` beside the color picker. Checked on screen
    2026-10-13 (three apps picked and added).

- [x] **L4. "Add ▸ Apps…" in the space menu (F-53).** After L3. Follow
  "Recipe: a new menu entry".
  - `app.rs` `mod menu`: `ADD_APPS = 45` (it opens a new group at 45; update
    the next-free note in `CONTEXT.md` section 6), and add it to
    `menu_ids_are_unique`.
  - `app/menus.rs` `show_space_menu`: an "Add" submenu right after "Fit to
    contents", built by one small helper `space_add_menu` like
    `space_sort_menu`. It holds "Apps…" now; L5 adds "Paste shortcut" and L6
    "Files…". Leave it enabled for a locked space: locking fixes the
    position, not the contents, and `Command::AddItem` accepts a locked space.
  - The arm: `let items = ui.pick_apps();` and, when it is not empty,
    `self.add_shortcuts(ui, id, at, items)`, with `at` the point
    `show_space_menu` already receives.
  - Docs: `CONTEXT.md` section 8 (the space-menu bullet), `README.md`
    Controls, and the F-53 row in the ROADMAP feature index.
  - Done when: tests pass. **[needs human]** check: "Add ▸ Apps…" on a space;
    pick a Store app and two desktop apps at once; three working shortcuts
    appear in that space, and Cancel adds nothing.
  - Landed differently (maintainer, 2026-10-13): the submenu is "New
    shortcut ▸" with "From installed apps…" (`NEW_SHORTCUT_APPS = 45`) and
    "Custom shortcut…" (`NEW_SHORTCUT_CUSTOM = 46`, a target / arguments /
    name dialog, `win32/shortcut_dialog.rs`), table-driven by
    `NEW_SHORTCUT_ENTRIES`. `add_shortcuts` takes `ShortcutSource`s so the
    custom shortcut ends there too. Checked on screen 2026-10-13.

- [x] **L7. A per-space Shortcuts page in the settings window (F-53).**
  Decided with the maintainer (2026-10-13).
  - A sixth page, listed only when a space is selected in the scopes pane
    (never for Global), always enabled (adding and removing items is not a
    look setting, so it ignores "Customize this space").
  - It shows a styled preview of the space: its title bar, its tint, and
    every item's real icon and label in grid order, scrolling when there are
    many. Not pixel-exact (no blur, approximate spacing).
  - Each item carries a small X in its upper-right corner. Clicking it asks
    first, then deletes the item as the Delete key does: a stored file goes
    to the Recycle Bin, an item listed in place is only unlisted (ROADMAP
    D3). It commits at once, like "Fit to contents"; Cancel does not bring it
    back.
  - A last tile shows a +; clicking it opens the same "New shortcut ▸"
    choices as the space menu (`NEW_SHORTCUT_ENTRIES`) at the tile.
  - Platform: a new `FieldKind` for the preview, drawn by an owner-drawn
    child window of the page, reporting `SettingsEvent`s for the X and the +.
  - Landed (2026-10-13): `FieldKind::SpacePreview` in
    `settings_window/preview.rs`, `SettingsEvent::PreviewRemove` /
    `PreviewAdd`, `page::SHORTCUTS` (9) with `settings::listed_pages`,
    `App::delete_space_item` and `App::offer_new_shortcut`. The
    installed-apps list is now cached at startup (`win32/app_catalog.rs`,
    `Ui::prefetch_apps`) after the maintainer found the picker slow to open.
    Checked on screen 2026-10-13 (hover, X with confirmation, +, and a fast
    picker).

- [x] **L5. "Paste shortcut" in the New shortcut submenu (F-54).** After
  F-53. Explorer's "Paste shortcut": copy anything in Explorer, then make a
  shortcut to it in a space.
  - Platform: `Ui::clipboard_shell_items(&mut self) -> Vec<ShellItem>` in
    `runtime/ui_shell.rs`, implemented in `win32/clipboard.rs`:
    `OleGetClipboard`, then the same path as drops (make
    `drop_target::shell_items` `pub(crate)`, or move it beside `shell_item`
    in `shortcuts.rs`; do not copy it). Reuse the retry the clipboard file
    reader already has for a clipboard another process holds.
  - App: `menu::PASTE_SHORTCUT = 47` in the "New shortcut" submenu after a
    separator (a row in `NEW_SHORTCUT_ENTRIES` with a new `NewShortcut`
    variant), greyed when `ui.clipboard_shell_items()` is empty (read it once
    while building the menu). It calls `add_shortcuts` with
    `ShortcutSource::Shell`; L2's copy-a-`.lnk` rule applies because it lives
    there. Paste shortcut never moves or
    deletes the clipboard's files and leaves the clipboard and any cut
    ghosting alone.
  - No key: Explorer has no default key for it, and Ctrl+V keeps meaning
    paste.
  - Docs: `CONTEXT.md` section 8 (the New shortcut bullet), `README.md`
    Controls, and the F-54 row.
  - Done when: tests pass. **[needs human]** check: copy a file and a folder
    in Explorer; "Paste shortcut" in a space makes two shortcuts and the
    originals stay; with nothing on the clipboard the entry is grey.

- [x] **L6. "Add files…" in the space menu (F-55).** Decided with the
  maintainer (2026-10-14): a picked file moves into the space, exactly as a
  drop does.
  - Platform: `Ui::pick_files`, an `IFileOpenDialog` with
    `FOS_ALLOWMULTISELECT | FOS_FORCEFILESYSTEM`, opened on the Desktop
    folder (`win32/dialog.rs::pick_files`, from
    `shortcut_dialog::browse_for_file`). One dialog cannot pick
    files and folders together, so folders stay drag-only for now.
  - App: `menu::ADD_FILES = 48`, an entry after "New shortcut ▸" (it adds
    files, not shortcuts); the arm calls
    `add_dropped(ui, id, at, paths, false)`, so it
    shares the drop path exactly.
  - Docs: `CONTEXT.md` section 8, `README.md` Controls, and the F-55 row.
  - Done when: tests pass. **[needs human]** check: pick two files; they land
    in the space as decided, and Cancel adds nothing.

- [x] **G5a. A right drag starts a shell drag.**
  - Today a right press only selects (`on_space_event`, `app/pointer.rs`,
    the `MouseButton::Right` arm calls `select_at`) and a right release opens
    the item or space menu.
  - App, `app/pointer.rs`:
    1. Add a field to `PointerState`: `right_press: Option<(Surface, ItemId,
       Point)>`, with a `///` doc ("an item pressed with the right button,
       not yet dragged").
    2. In the right `MouseDown` arm, after `select_at`, set it when the
       press hit an item (`self.surface_item_at(surface, point)`).
    3. In the `MouseMove` arm, before `drag_to`: if `right_press` is set and
       the pointer moved past `deskspace_platform::drag_threshold()` from
       the press point (copy how the left-button drag decides it has
       started), take `right_press` and call `self.drag_item_out(ui,
       surface, item)` with the right button (step 5). Then return.
    4. Add a second field, `right_dragged: bool` ("a right drag ran since
       the last right press"); step 3 sets it. In the right `MouseUp` arm:
       clear `right_press`; if `right_dragged` is set, clear it and open no
       menu; otherwise open the item or space menu exactly as today. Clear
       both fields on `CaptureLost` and in the right `MouseDown` arm.
    5. `drag_item_out` itself needs no new argument: the platform does not
       take a button.
  - Platform: probably no change. `win32/shell_items.rs` `drag_out` calls
    `SHDoDragDrop` with no drop source (`None`), and the shell's default
    drop source follows whichever mouse button is held when the drag
    starts, so a drag started while the right button is down ends on its
    release and the target shows its menu. Two things to confirm while
    testing, and fix only if they fail:
    - `drag_out` calls `ReleaseCapture()` first; the right button must
      still be down at that point (it is, since the drag starts on a
      move);
    - the right button's release is consumed by the drag loop, so the space
      may never get its `MouseUp`; then `right_dragged` stays set until the
      next right press clears it (step 4), which is harmless.
    If the default drop source does not handle the right button, **stop and
    ask**; a custom `IDropSource` is a larger change.
  - Pure helper with a test: `fn right_drag_started(press: Point, now:
    Point, threshold: Size) -> bool` in `app/pointer.rs`.
  - Done when: tests pass. **[needs human]** check: right-drag an item from
    a space onto the desktop and into an Explorer folder; the shell's
    Move / Copy / Create shortcut menu appears and each choice works; a
    right click without moving still opens the item menu.

- [x] **G5b. DeskSpace's drop menu on a space.** After G5a.
  - The menu is shown by the drop target itself, inside `Drop`, because
    `Drop` must return the chosen effect to the source synchronously. The
    app then receives an ordinary `FilesDropped` describing the choice, so
    no new re-entrancy is introduced.
  - Model the choice explicitly. In `input.rs`, replace
    `FilesDropped.keep_in_place: bool` with `action: DropAction`, a new
    `pub enum DropAction { Move, KeepInPlace, Copy }` with a `///` doc per
    variant (`Move`: into the store, the default; `KeepInPlace`: list the
    file where it is, what Shift does today; `Copy`: put a copy in the
    store). Re-export it from `lib.rs`. Update the one app caller (the
    `FilesDropped` arm in `app/pointer.rs`) to map `Move` /
    `KeepInPlace` onto today's `add_dropped(.., keep_in_place)`; this
    first step is a pure refactor and can be its own commit.
  - Platform, `win32/drop_target.rs`:
    1. In `DragEnter`, remember whether `MK_RBUTTON` was down (a
       `right_drag: Cell<bool>` on the drop target).
    2. `desired_effect` (or `drop_plan`, once L2 has replaced it): for a
       right drag, return `MOVE` (the menu decides the rest), so the cursor
       does not flicker with the modifier keys.
    3. In `Drop`, for a right drag, show a popup menu at the drop point
       (the `POINTL` `Drop` receives is already in screen coordinates) with
       `menu::track(owner, &entries, at)`, the function `Ui::popup_menu`
       uses, owned by the space window; read no runtime state across the
       call (AGENTS.md Gotchas). Entries:
       "Move here", "Copy here", "Show here without moving", a separator,
       and "Cancel". Map the choice with a pure, tested
       `fn right_drop_choice(id: u32) -> Option<DropAction>` (`None` for
       Cancel or a dismissed menu). On `None`, return `DROPEFFECT_NONE`
       and raise no event. Otherwise raise `FilesDropped` with that action
       and return `COPY` for `Copy` and `KeepInPlace`, `MOVE` for `Move`.
       Keep the "Performed DropEffect" marking exactly as the left-drop
       path does.
  - App, `add_dropped` (`app/files.rs`): add the `Copy` case, copying the
    files into the store through `ui.transfer_files` the way a Ctrl+C /
    Ctrl+V paste does (`app/clipboard.rs`), then listing them. Change
    `add_dropped` to take the `DropAction` instead of a bool.
  - Done when: `right_drop_choice` has a test per entry. **[needs human]**
    check: right-drag a file from Explorer onto a space; the menu appears;
    each entry does what it says, the source file is left alone on Copy,
    Show here, and Cancel, and a left drag still moves (Shift still keeps in
    place).

- **R4. Keep IO out of core.** `deskspace-core/src/item.rs` reads the file
  system (`ItemKind::classify`, `read_metadata`, `file_flags`) from
  `Item::new`, which `Command::AddItem` calls inside core. Move the reads out,
  add an `ItemFacts { kind, size, modified, compressed, encrypted }` (name
  to be decided) that the caller fills, and pass it into `AddItem` and
  `Item::new`. Callers: `app/files.rs` (`add_dropped`, `add_shortcuts`),
  `app/clipboard.rs` (paste), and `bootstrap.rs` (seeding). Needs a decision
  on where the reads live (`storage.rs` is a candidate).
  Resolved: the reads live in the platform, `deskspace_platform::item_facts`
  (`win32/desktop_items.rs`, beside the attribute reads it already did), not
  `storage.rs`. `ItemFacts` carries `is_folder` rather than a `kind`, so
  `ItemKind::classify(path, is_folder)` stays the one place that decides a
  kind. `view.rs` (the desktop layer) also creates items and reads facts.

- **R3. Platform module layout.** What remains after the K splits:
  - one icon decoder instead of two: `icons::decode_icon_masked` (GDI
    `GetDIBits`, for HICONs) and `d2d::decode_bitmap` (WIC, for HBITMAPs);
  - a `win32/messages.rs` owning every `WM_APP + n` (today in
    `controller.rs`, `keyboard.rs`, and `outside_click.rs`), with a test
    that the values are unique (compare B15 for timers);
  - make internal modules private: `win32/mod.rs` has 12 `pub mod`s, though
    the app uses only the `lib.rs` re-exports;
  - long window procedures (`space_window.rs` about 180 lines,
    `controller.rs`, `color_picker.rs`, `settings_window/mod.rs` about 120-150
    each): one named handler per message, keeping `runtime::dispatch` the
    last statement of each branch (`AGENTS.md` Gotchas).
  Resolved, in four commits:
  - The fourteen ids live in `win32/messages.rs`, values unchanged, with
    tests that they are unique and inside the `WM_APP` range. Payload codes
    (`SEL_*`, `NAV_*`, `ACTION_*`) stay in `keyboard.rs` with their encoders
    and decoders.
  - Every top-level module of the platform crate is private, not only the
    `win32` ones. A `win32` module is `pub(crate)` only when code outside
    `win32` (`lib.rs`, `render`) uses it. No dead code surfaced.
  - The window procedures (`color_picker.rs`, `settings_window/mod.rs` and
    `space_window.rs`) get one named handler per multi-line message; one-line
    arms and the arms whose length is their why-comment stay inline. The
    controller instead gets a pure `event_for` that decodes every posted
    message, and the picker's pointer logic became tested `PickerState`
    methods.
  - The GDI decoder is gone. `d2d::decode_icon` (WIC `CreateBitmapFromHICON`)
    matched it byte for byte on a legacy masked icon, a 32 bpp icon, shell
    file icons and every overlay at each image-list size, before it replaced
    it. Two tests pin the mask and premultiply behavior.

- [x] **F13. Ctrl+wheel under "Match Explorer's display" drives the
  desktop's icon size (ROADMAP D9).** The reason Ctrl+wheel did nothing
  under any preset is fixed (`fix: read mouse modifiers from the message`;
  section V); confirm the `Custom` branch on screen before relying on it.
  - Today `App::adjust_icon_size` (`app/pointer.rs`) changes
    `Theme.icon_size`, which the default `MatchExplorer` preset replaces with
    Explorer's size (`App::icon_scale` / `space_scale`, `app/view.rs`). D9
    says it should change the desktop's own icon size instead.
  - Platform, `win32/display_settings.rs`:
    1. `pub const DESKTOP_ICON_RANGE: (i32, i32) = (16, 256);` and
       `pub const DESKTOP_ICON_STEP: i32 = 8;` (pixels; `///` doc each).
    2. A pure `pub fn next_desktop_icon_size(current: i32, notches: i32) ->
       i32` (`current + notches * DESKTOP_ICON_STEP`, clamped). Tests: one
       notch each way, both clamps.
    3. `pub fn set_desktop_icon_size(pixels: i32) -> bool`: from
       `shell_view::desktop_shell_view()`, cast to `IFolderView2`, read the
       mode with `GetViewModeAndIconSize`, call `SetViewModeAndIconSize(mode,
       pixels)`; `false` with a `tracing::debug!` on failure. Copy the shape
       and `// SAFETY:` wording of `view_icon_size`, and link the Microsoft
       Learn page for `IFolderView2::SetViewModeAndIconSize`.
    4. Re-export all four from `win32/mod.rs` and `lib.rs` next to
       `desktop_display`.
  - App, `App::adjust_icon_size`: convert `delta` to notches (`delta /
    WHEEL_DELTA`; return on 0). If the *global* preset matches Explorer and
    `self.shell_display.icon_size` is `Some(current)`, call
    `set_desktop_icon_size(next_desktop_icon_size(current, notches))` and, on
    `true`, `self.refresh_shell_display(ui)`. Otherwise keep today's
    behavior. Update the doc comment for both branches. A space with its own
    `Custom` look is unaffected either way.
  - Docs: `CONTEXT.md` section 8 (Ctrl+wheel), `README.md` Controls, and the
    F-25 row in the ROADMAP feature index (drop "open (F13)").
  - Done when: the tests pass. **[needs human]** check: under the default
    preset, Ctrl+wheel over a space changes the desktop's icons and the
    spaces together, both ways, and stops at both ends; under Custom it
    changes only the spaces.

- [x] **F15. An icon-size change keeps every space's size (Explorer
  parity).** **[needs decision]** on what an auto-sized space's `columns`
  means afterwards. Explorer never resizes a window when its icon size
  changes: the icons re-flow into fewer or more columns and a scrollbar
  appears when they no longer fit. Today `arrange_space` (`layout.rs`) keeps
  a user-sized space's rect but recomputes an auto-sized one from
  `GridSpec.columns`, so Ctrl+wheel and the icon-size slider grow and shrink
  every space the user never resized (maintainer, 2026-10-16: keep every
  space's size, as Explorer does).
  - Decide: does an icon-size change pin an auto-sized space (set
    `user_sized`, through a `Command`), or does an auto-sized space derive its
    columns from its current width once it has one, and keep growing only in
    height as items are added? Decided 2026-10-16: pin it (`KeepSize`).
  - Both Ctrl+wheel (`App::adjust_icon_size`, both branches) and the
    Settings icon-size slider (and a Match Explorer size change read in
    `refresh_shell_display`) follow the same rule; no special case for one
    path.
  - Done when: an icon-size change leaves every space's rect unchanged, with
    a core test per kind of space. **[needs human]** check: Ctrl+wheel over
    an auto-sized and a resized space; neither changes size, both re-flow and
    scroll.

- [x] **B10. Named ranges for tint opacity and title shading.**
  `settings/apply.rs` (`apply_look_field`) clamps `tint_opacity` and
  `title_shading` with a literal `0, 100`, `settings/pages.rs` passes
  `(0, 100)` for both sliders, and `Theme::clamped` uses `.min(100)`. This
  breaks the "use the constant, never a literal range" rule (`CONTEXT.md`
  section 4).
  - `theme.rs`: add `pub const TINT_OPACITY_RANGE: (u8, u8) = (0, 100);` and
    `TITLE_SHADING_RANGE`, with a `///` doc, beside `OVERLAY_SIZE_RANGE`. Use
    them in `Theme::clamped`, `apply_look_field`, and `look_pages`, as
    `ICON_SIZE_RANGE` is used.
  - Done when: `grep -n '0, 100' crates/deskspace-app/src/settings` finds no
    range for these two fields.

- [x] **B11. Group the scene-builder arguments.** `scene.rs`
  `desktop_layer_scene` (8 arguments), `desktop_layer_drag_scene` (9), and
  `drag_scene` (8) carry `#[allow(clippy::too_many_arguments)]`. Each takes
  the same `theme, palette, layout, metrics` quartet.
  - Add `pub struct SurfaceLook<'a> { theme: &'a Theme, palette: &'a
    Palette, layout: GridSpec, metrics: ItemMetrics }` (`Copy`, `///` doc) in
    `scene.rs` and pass it in place of the four arguments to all four
    builders, `space_scene` included. Build it at the call sites in `app/`
    (`view.rs`, `drag.rs`) from the existing `*_for` / `surface_*` accessors.
  - Done when: the three allows are gone and scene tests pass unchanged
    apart from the argument.

- [x] **B12. Group the d2d draw-helper arguments.** `render/d2d/item.rs`
  `draw_item` (8) and `draw_check_box` (8) and `render/d2d/edit.rs`
  `draw_edit_buffer` (9) are allowed. Each starts with the same
  `graphics, target, brush` (or `target, brush`).
  - Add `pub(super) struct DrawContext<'a> { graphics: &'a Graphics, target:
    &'a ID2D1RenderTarget, brush: &'a ID2D1SolidColorBrush }` in
    `render/d2d/mod.rs`, build it once per paint and in `render_items`, and
    pass it. `draw_check_box` also takes its four box numbers as one
    `CheckBoxRect` (or the existing check-box geometry type in
    `render/mod.rs`, if one fits).
  - No allocation in the paint path (`AGENTS.md`).
  - Done when: the three allows are gone.

- [x] **B15. Timer ids as an enum.** The app's timer ids are eight loose
  `const`s in `app.rs` (`AUTOSAVE_TIMER` ... `HEALTH_TIMER`), declared out of
  order, and `on_event` matches them with eight `if id == ...` guards.
  - Add `#[repr(usize)] enum Timer { Autosave = 1, Caret = 2, InfoTip = 3,
    Update = 4, Highlight = 5, Preview = 6, Type = 7, Health = 8 }` (the
    existing values; never renumber) with `fn from_id(id: usize) ->
    Option<Self>`, and small `App` wrappers `set_timer(ui, Timer, ms)` /
    `clear_timer(ui, Timer)` over `Ui::set_timer` / `clear_timer`. Keep each
    `*_MS` delay constant beside the module that uses it.
  - `on_event`: one `Event::Timer { id }` arm that matches
    `Timer::from_id(id)`.
  - Test: `from_id` round-trips every variant and rejects 0 and 9.
  - Docs: the timer table in `CONTEXT.md` section 6 names the variants.
  - Done with all ten timers: `UpdateIdle = 9` and `ZoomBar = 10` landed
    after this task was written, so the test rejects 0 and 11 (maintainer,
    2026-10-16).

- [x] **B16. One home for the key list.** The full key list is kept in three
  places (`ENGINEERING.md` Keys, `CONTEXT.md` section 8, `README.md`
  Controls), so the key recipe edits all three.
  - Keep the flag-by-key table in `ENGINEERING.md` only. `CONTEXT.md` section 8
    links to it instead of restating which flag a key needs. `README.md`
    keeps its user-facing prose (it is for users, not contributors).
  - Update the key recipe's step 4.

- [x] **B17. Fix stale statements in CONTEXT and ROADMAP.**
  - `CONTEXT.md` section 2, "Code health": no non-test file is over 800
    lines is false until K7 lands; reword it to name `color_picker.rs`.
  - `ROADMAP.md` section 4, "Next, in order": item 1 (section L) has only
    L1's findings left and item 4 lists G5a-G5b, which have landed; reduce
    them to L1 and G5c.
  - Done when: the two statements match this file.

- [x] **B20. Stop hand-counting tests in the docs.** `CONTEXT.md` section 2
  states the test count ("403 tests") and `How to work a task` rule 6 asks
  every task to update it, so it drifts. `cargo test` already reports the
  number.
  - `CONTEXT.md` section 2: drop the number and the "Update this count"
    sentence.
  - This file: rule 6 drops "and the test count in `CONTEXT.md` section 2
    updated"; section K's "Done when" keeps "with the same test count" (it
    is checked from `cargo test` output, not the docs); section B's intro is
    unchanged.
  - Done when: `grep -rn "403\|test count" docs/*.md` finds only the K rule
    and the section B intro.

- [x] **B21. Seed the first space through `Command`s.** `bootstrap.rs`
  builds the initial "Desktop" space by calling `Desktop::add_item` and
  `arrange_space` on `desktop.space_mut(id)` directly, the one place outside
  tests that changes the model without a `Command` (`ENGINEERING.md`
  section 3). Seed it with `Command::CreateSpace`, one `Command::AddItem` per
  entry (the `ItemFacts` it already reads), then `Command::AutoArrange`,
  applied through `Desktop::apply` with the work area `bootstrap` already
  uses. Same result, so the existing seeding tests pass unchanged; add one if
  none covers "a fresh config gets one arranged Desktop space".
  - Done when: `bootstrap.rs` calls no `*_mut` model accessor.

- [x] **B27a. Move `Config::migrate`'s two space loops into core.**
  - In `deskspace-core/src/desktop.rs`, add two methods to `impl Desktop`:
    - `pub fn reset_v1_spaces(&mut self)`: the body of the v1 loop in
      `deskspace-config/src/schema.rs` `Config::migrate` (each space:
      `clear_legacy_overrides()`, then `position_items(space,
      self.layout)`).
    - `pub fn clamp_looks(&mut self)`: `for space in &mut self.spaces {
      space.look = space.look.take().map(SpaceLook::clamped); }`.
    - Move each loop's comment into the new method's `///` doc.
  - In `Config::migrate`, replace the two loops with
    `self.desktop.reset_v1_spaces();` and `self.desktop.clamp_looks();`.
  - Tests: the config migrate tests pass unchanged. Add one core test,
    `clamp_looks_clamps_every_space`: a space with an out-of-range
    `look` comes back in range (copy an existing `SpaceLook::clamped` test
    for the values).
  - Done when: `rg -n "desktop.spaces" crates/deskspace-config/src` finds
    nothing outside `#[cfg(test)]` modules.

- [x] **B27b. Add read accessors (core only).**
  - In `deskspace-core/src/space.rs` `impl Space`, add one-line public
    getters: `title(&self) -> &str`, `rect(&self) -> Rect`,
    `items(&self) -> &[Item]`, `locked(&self) -> bool`,
    `rolled_up(&self) -> bool`, `custom(&self) -> bool`,
    `look(&self) -> Option<&SpaceLook>`, `user_sized(&self) -> bool`.
  - In `deskspace-core/src/desktop.rs`, add `spaces(&self) -> &[Space]`.
  - Leave `id` public: it never changes after creation.
  - If a method with one of these names already exists, stop and ask.
  - Change no caller in this commit. Done when: the gates pass.

- [x] **B27c. Read through the accessors outside core.**
  1. Temporarily change the eight `Space` fields above and
     `Desktop::spaces` from `pub` to `pub(crate)`. Do not commit this.
  2. Run `cargo check --workspace --all-targets`.
  3. Fix every error that is a **read**: `space.title` becomes
     `space.title()`, `&space.items` becomes `space.items()`,
     `desktop.spaces.iter()` becomes `desktop.spaces().iter()`, and so on.
     Leave every error that is a **write** (`=`, `push`, `&mut`) alone; it
     belongs to B27d.
  4. Change the fields back to `pub`, run the gates, and commit only the
     call-site changes.
  - Done when: step 1 again leaves only write errors.

- [x] **B27d. Write test fixtures through `Command`s.**
  - Find the writes with step 1 of B27c (again uncommitted). They are in
    tests: `scene.rs`, `settings/mod.rs`, `settings/apply.rs`,
    `settings/session.rs`, `app.rs`, `app/hit.rs`, `app/menus.rs`,
    `app/selection.rs`, and the config tests.
  - Replace each with the command in this table, applied with
    `desktop.apply(command, bounds)`. Read each variant's fields in
    `deskspace-core/src/command/mod.rs`, and copy `bounds` from an existing
    test in that file.

    | Write | Command |
    |-------|---------|
    | `custom = true` and `look = Some(x)` | `SetSpaceLook { custom: true, look: x, .. }` |
    | `look = Some(x)` alone | `SetSpaceLook { custom: false, look: x, .. }` |
    | `custom = true` alone | `CustomizeSpace { on: true, .. }` |
    | `locked = true` | `ToggleLock` (on a new, unlocked space) |
    | `rolled_up = true` | `ToggleRollup` |
    | `user_sized = true` | `KeepSize` |
    | `items.push(item)` / `add_item(id, item)` | `AddItem` |

  - Assert each fixture command `is_applied()`, so a broken fixture fails
    loudly.
  - If a write has no row in the table, stop and ask.
  - Change the fields back to `pub`, and commit only the test changes.
  - Done when: step 1 of B27c compiles with no errors outside
    `deskspace-core`.

- [x] **B27e. Make `Space`'s fields private.** Change the eight fields to
  `pub(crate)` for real. `serde` still reads and writes them. Done when: the
  gates pass with no other change.

- [x] **B27f. Make the `Desktop` mutators crate-private.**
  - Change `Desktop::spaces` (the field), `space_mut`, `layer_mut`,
    `add_space`, `add_item`, and `remove_item` to `pub(crate)`, as well as
    `Space::remove_item` and `DesktopLayer::remove_item`.
  - Fix each error the same way as B27d. A test that needs a new space's id
    reads `desktop.next_space_id()` before applying `Command::CreateSpace`.
  - Change the B27 note in `ENGINEERING.md` section 3 to say that the
    compiler enforces it. In `CONTRIBUTING.md` "Keep three rules", say the
    same.
  - Done when: `rg -n "space_mut|layer_mut|add_space|\.add_item\("
    crates/deskspace-app crates/deskspace-config` finds nothing.

- [x] **B28a. `App::dispatch` returns the `CommandOutcome`.**
  - In `app/persist.rs`, change `dispatch` to return `CommandOutcome`
    instead of `bool`. Before returning, when the outcome is not applied,
    log it once: `tracing::debug!(?outcome, "command not applied")`.
  - Each caller that used the `bool` adds `.is_applied()`. Find the callers
    with `rg -n "self\.dispatch\(" crates/deskspace-app/src`.
  - Pure refactor. Done when: the gates pass.

- [x] **B28b. Core: `Desktop::touched_by` says what a command will change.**
  - In `deskspace-core/src/command/mod.rs`, add `pub struct Touched {
    pub spaces: Vec<SpaceId>, pub layer: bool }` with a `///` doc. Also add
    `pub fn touched_by(&self, command: &Command) -> Touched` on `Desktop`.
    It is called **before** the command is applied, so it can still find
    an item's current space.
  - Write it as one `match` over every variant with **no `_` arm**, so a new
    variant cannot compile until someone decides what it touches.
  - The rules:
    - A variant with a space `id` touches that space.
    - An item variant touches the space that holds the item now.
    - A move touches both spaces. **List the destination first**: its grid
      lays out the result.
    - `AutoArrange` and `SnapAll` touch every space.
    - `SyncDesktop` and `SetDesktopItemPosition` touch only the layer.
    - `MoveToDesktop` touches its source space and the layer.
    - If a variant fits none of these rules, stop and ask.
  - Tests: one per rule above, in `command/mod.rs`.

- [x] **B28c. `App::commit` replaces `apply_to_space`.**
  - In `app/persist.rs`, add `pub(super) fn commit(&mut self, ui: &mut Ui,
    command: Command, at: Point) -> CommandOutcome`. In this order, it:
    1. reads `self.config.desktop.touched_by(&command)`;
    2. calls `self.dispatch(command, at)`;
    3. if the command applied, calls `refresh_space` for each touched space
       that still exists;
    4. if `touched.layer` is set, redraws the desktop layer. Use the method
       `app/view.rs` already calls for that; if it takes more than one call,
       stop and ask;
    5. calls `schedule_save`.
  - Replace each `apply_to_space(ui, id, command, at)` call with
    `commit(ui, command, at)`. The callers are in `app.rs`, `app/menus.rs`,
    and `app/settings_session.rs`. Then delete `apply_to_space`.
  - Done when: `rg -n apply_to_space crates` finds nothing. **[needs
    human]** check: lock, roll up, sort, and "Fit to contents" each repaint
    the space at once and survive a restart.

- [x] **B28d. Move the bare `dispatch` callers to `commit`.** Make one
  commit per file: `app/menus.rs`, `app/clipboard.rs`, `app/files.rs`,
  `app/drag.rs`, `app/rename.rs`, and `app/settings_session.rs`. For each
  `self.dispatch(` call:
  - If it is followed by `refresh_space` or `schedule_save` for the spaces
    it changed, replace the call with `commit` and delete those follow-ups.
  - If it is one of several commands followed by a single refresh, or it is
    followed by `flush_save` (file moves save at once), keep `dispatch`. Add
    a one-line comment above it saying which case it is.
  - Done when: every `self.dispatch(` outside `commit` has such a comment.

- [x] **N1. Move `type_buffer` and `watching_outside` into `Selection`.**
  Both are selection state (type-to-select's prefix, and whether outside
  clicks are watched while something is selected). `Timer::Type` in
  `App::on_timer` clears the buffer through a `Selection` method.

- [x] **N2. A `SettingsUi` for the open settings window.** In
  `app/settings_session.rs`: `struct SettingsUi { session: Option<Session>,
  pending_preview: Preview, highlighted: Option<SpaceId> }`, held as
  `App::settings_ui`. The reads in `view.rs`, `menus.rs`, and `update.rs`
  become `self.settings_ui.is_open()` (or the accessor the read needs).

- [x] **N3. A `LayerState` for the desktop layer.** In `app/view.rs`:
  `struct LayerState { layer_flag: bool, takeover_flag: bool, entries:
  Vec<DesktopEntry> }` from `takeover_layer`, `takeover`, and
  `desktop_entries`. `App::layer_active` and `App::desktop_takeover` read the
  config's `take_over_desktop` too, so they stay on `App` and call the
  struct's flag accessors. `bootstrap.rs` sets
  the two flags through a constructor. Check `desktop_entries`' doc comment
  ("until then they are only diagnostics"); if the layer now consumes them,
  fix the comment in the same commit.

- [x] **N4. A `Hotkeys` for the registered bindings.** In `app.rs` (or a new
  `app/hotkeys.rs` if it passes about 60 lines): `struct Hotkeys([Option<Hotkey>;
  HOTKEY_COUNT])` with `register_all(ui, &WindowSettings)` and
  `unregister_all(ui)`, the bodies the start, Apply, and close paths share
  today. `App::on_hotkey` keeps the action `match`.

- [x] **N5. A `SystemLook` for what Windows reports.** In `app/view.rs`:
  `struct SystemLook { dark: bool, accent: Option<Color>, shell: ShellDisplay
  }` with `refresh(&mut self)` (the reads `Event::SystemThemeChanged` and a
  space click do now). Do it after N3, and after B25 if `on_event`'s theme arm
  has moved. Mechanical, but it touches seven modules: one commit, and run
  the gates before it.

- [x] **N6. A `ViewCache` for per-space view state.** `scroll` and
  `icon_scales` are per-space, not persisted, and keyed by `SpaceId`. Put them
  in `struct ViewCache` in `app/view.rs` with `scroll_of(id)`,
  `set_scroll(id, px)`, `forget(id)` (call it where a space is deleted; today
  both maps keep a deleted space's entry, so check and note it under **Found
  while working** if that is still true), and the `icon_scales` reads. After
  N5.

- [x] **B29. Tell the user when the config cannot be saved.** Today
  `App::save` (`app/persist.rs`) only logs the error, and release builds
  have no console, so a failed save goes unseen until changes are lost.
  - Platform, `win32/runtime/ui_chrome.rs`: add `pub fn alert(&mut self,
    title: &str, text: &str)` right below `confirm`. Copy `confirm`'s body,
    use `MB_OK | MB_ICONWARNING`, and return nothing.
  - App, `app.rs`: add `reported_failure: bool` to `struct Autosave`.
  - App, `app/persist.rs`:
    - Add a pure `fn save_failure_text(path: &Path, error: &str) -> String`.
      It names the path, gives the error, and says that changes may be lost
      when DeskSpace closes.
    - Change `save` to `save(&mut self, ui: &mut Ui)`. On an error, log as
      now; then, if `reported_failure` is false, set it and call
      `ui.alert("DeskSpace could not save", &save_failure_text(..))`. On
      success, set it back to false.
    - Add `flush_save_on_close(&mut self, ui: &mut Ui)`. It does what
      `flush_save` does but never alerts, because a modal box must not
      block a shutdown. Call it from `Event::CloseRequested` instead of
      `flush_save`.
  - Test: `save_failure_text` contains the path and the error.
  - **[needs human]** check: make the scratch config read-only (`attrib +r`)
    and move a space. One box appears; a second move shows none. Clear the
    attribute and move again; the file is saved.

- [x] **B30. Lay a command out on the grid of the space it targets.**
  Today `App::dispatch` picks the grid from `space_at(near)`, the topmost
  space under the point. When spaces overlap, that can be a different space
  from the one the command changes. After B28b:
  - In `dispatch`, take `touched_by(&command).spaces.first()`. If it is a
    space that exists, use `self.layout_for(id)`. Otherwise keep today's
    `space_at(near)` fallback.
  - Done when: the gates pass. **[needs human]** check: overlap two spaces
    with different icon sizes, then sort or arrange the lower one. Its icons
    land on its own grid.

- [x] **B18. Menu ids as an enum.** The model is B15 (commit `3096a47`,
  `Timer` in `app.rs`). `app.rs`'s `mod menu` holds 34 loose `u32` consts,
  and `menu_ids_are_unique` only works because its hand-kept `other` list
  names every id the preset tables do not.
  - Replace `mod menu` with `#[repr(u32)] enum MenuId` beside `Timer`: one
    variant per const, CamelCase (`ROLL` -> `Roll`, `OPACITY_100` ->
    `Opacity100`), **the same values** (never renumber; 9 stays unused), plus
    `const ALL: [Self; N]` and `fn from_id(id: u32) -> Option<Self>`, as
    `Timer` has.
  - The preset tables (`NEW_SHORTCUT_ENTRIES`, `OPACITY_PRESETS`,
    `ICON_SIZE_PRESETS`, `SORT_PRESETS`) hold `MenuId` instead of `u32`.
    `MenuEntry::item` still takes a `u32` (the platform knows no app ids), so
    pass `id as u32`; add a small `fn menu_item(id: MenuId, label: &str) ->
    MenuEntry` in `app/menus.rs` if more than a few call sites need it.
  - Where a chosen id is matched (`app/menus.rs`: `show_space_menu`,
    `show_item_menu` with `ItemMenuChoice::App`, the tray menu,
    `run_common_menu`, the submenu helpers), convert once with
    `MenuId::from_id` and match on the enum.
  - Tests: replace `menu_ids_are_unique` with `menu_ids_round_trip` (every
    `ALL` entry survives `from_id`, `from_id(9)` and `from_id(0)` are `None`)
    and `menu_ids_stay_below_the_shell_range` (every value is below
    `0x8000`). `ALL` makes the hand-kept list unnecessary.
  - Docs: in `CONTEXT.md` section 6, "Message, timer, and id tables", the
    "Menu ids" row names `enum MenuId` and its guard tests. The recipe "a
    new menu entry" step 1 becomes "a `MenuId` variant (next free value) and
    its `ALL` entry". Move the `mod menu` doc comment onto the enum.
  - Done when: `mod menu` is gone and no `u32` menu id literal is left in the
    app.

- [x] **B19. Forbid `unsafe` in `deskspace-app`.** The app crate has no
  `unsafe` today, but nothing enforces "all `unsafe` lives in the platform"
  there. Add `#![forbid(unsafe_code)]` at the top of both binary roots,
  `src/main.rs` and `src/watchdog_main.rs` (after the existing
  `windows_subsystem` attribute and its comment). Leave `build.rs` alone; it
  is its own crate. Update the three places that list which crates forbid it:
  `ENGINEERING.md` section 3, `CONTEXT.md` section 3 ("Crate
  responsibilities"), and `CONTRIBUTING.md` "First 15 minutes" rule 2.
  - Done when: the gates pass and the docs say four crates forbid `unsafe`.

- [x] **G5c. "Create shortcuts here" on a right drop (ROADMAP D9).** G5a and
  G5b (the right-drag menus) have landed and await their checks (section V).
  Add the entry between "Copy here" and "Show here without moving".
  Choosing it takes L2's `DropPlan::Shortcuts` path: read the items with
  `shell_items(data)`, raise `ShellItemsDropped`, and return `LINK`, so
  `App::add_shortcuts` writes the shortcuts (or copies a dropped `.lnk`). No
  new `DropAction` variant and no second shortcut writer.

- **R15. Split `Ui` so `App` can be tested against a fake.** Superseded:
  the maintainer chose R17 (pure update, then effects) on 2026-10-17, which
  tests the app's decisions without a `Ui` fake. Kept for the record.
  `App`'s loose
  state is grouped into owned structs (N1-N6, archived). What remains is `Ui`: about 60 public methods (already
  filed by area in `runtime/ui_spaces.rs`, `ui_input.rs`, `ui_shell.rs`, and
  `ui_chrome.rs`), taken as one concrete type, so no `App` behavior that
  touches the screen can be unit-tested; the app's tests reach only the pure
  modules. Design focused traits or handles along those four files, a
  recording fake for tests, and how `EventHandler::on_event` receives them,
  keeping `runtime::dispatch`'s re-entrancy rules. Weigh R17
  first: it may make the fake unnecessary.

- [x] **B23. Make the keyboard watch a pure, tested table.**
  `win32/keyboard.rs` `fn handle` is about 160 lines of `if` chains that both
  decide which key means what and post the message, so none of the key
  routing is unit-tested, and the `ENGINEERING.md` Keys table is kept in step
  by hand.
  - Extract `fn watched_key(vk: u32, mods: Mods, watch: Watch) ->
    Option<(u32, usize)>` (the message and its `wparam`) as a pure function,
    where `Mods { ctrl, shift, alt }` and `Watch { selection, paste }` are
    small `Copy` structs. `handle` reads the modifiers, the flags, and
    `desktop_is_foreground` (still checked first), calls it, and posts. Keep
    the editing path (`EDITING`) as it is, in its own function.
  - Tests: one per row of the `ENGINEERING.md` Keys table, each asserting the
    key is routed under its flag and ignored without it (for example Ctrl+V
    with only `selection` set is `None`).
  - Done when: `handle` is under about 50 lines and every key in the Keys
    table has a test. **[needs human]** check: the F1-F5, G1, and G3 rows of
    section V still pass.

- [x] **K7. Split `win32/color_picker.rs`** (about 1,400 non-test lines).
  - `color_picker/mod.rs` keeps: the module doc, the sizing constants,
    `WINDOWS_COLORS`, the recent colors (`RECENT_LIMIT`, `remember`),
    `PickerState`, `pick`, `picker_position`, `dpi_of`, `style_window`,
    `ensure_class`, `with_state`, `mouse_point`, `confirm`, and `wndproc`.
  - `layout.rs` (pure): `Panel`, `Region`, `PickerLayout`, `swatch_color`,
    `saturation_value`, `fraction_x`, with their tests.
  - `paint.rs`: `Buffer`, `paint`, `ensure_buffer`, `paint_markers`,
    `composite_marker`, `coverage`, `paint_content`, `paint_palette`,
    `paint_custom`, `scaled_marker`, `blend`, `checker_color`.
  - `draw.rs`: the GDI primitives `rect_to_win`, `fill_solid`, `frame`,
    `draw_color_box` (re-exported from `mod.rs`: `settings_window/paint.rs`
    calls `color_picker::draw_color_box`), `draw_ring`, `draw_swatch`,
    `draw_check`, `draw_label`, `draw_hex`, `draw_button`, `fill_image`.
  - `pick` keeps its path, used by `runtime/ui_chrome.rs` and
    `settings_window/commands.rs`.

- [x] **Q1. "Native UI"** (`ui.rs`, `win32/menu.rs`,
  `win32/settings_window/`, `win32/color_picker/`, `win32/dialog.rs`,
  `win32/dark_mode.rs`).

- **R17. Pure update, then effects (chosen over R15, archived).** Make the app's
  decisions a pure function, `fn update(state: &mut AppState, event: Event)
  -> Vec<Effect>`. `Effect` is a plain enum, such as `RefreshSpace(id)`,
  `ScheduleSave`, `ShowMenu(..)`, or `SetTimer(..)`. A thin `run(effect, ui)`
  is then the only code that touches `Ui`, and tests assert the effects that
  an event produces, with no Win32 and no fake. Settle three things:
  - modal calls that return an answer (`popup_menu`, `confirm`,
    `pick_color`): split the flow into a request effect and a follow-up
    event, or keep those few as direct calls;
  - how this keeps `runtime::dispatch`'s re-entrancy rules;
  - the first event family to move as a pilot (timers or zoom).

  Its prerequisites (N1-N6, B28) have landed; B28's `Touched` is the first
  effect.

  Approved as proposed below (maintainer, 2026-10-17).
  - Why R17, not R15: a recording `Ui` fake (R15) tests that calls happened
    in some order, so its tests break whenever the call sequence changes.
    Effects are plain data, so a test asserts what an event means
    (`[RelayoutAll, ScheduleSave, ShowZoomBar(..)]`) and survives refactors.
    It also needs no traits over the 60 `Ui` methods.
  - Modal calls: keep `popup_menu`, `confirm`, and `pick_color` as direct
    calls in the thin shell, and make what follows them pure. The menu
    builders are already pure, so `show_space_menu` becomes: build entries
    (pure), `ui.popup_menu` (shell), then `space_menu_choice(state, id,
    choice) -> Vec<Effect>` (pure, tested). No request/response event pairs;
    they would add a message round trip to every menu for no test gain.
  - Queries such as `ui.is_zoom_bar_hovered()` are read by the shell before
    `update` and passed in as an argument, so `update` stays pure.
  - Re-entrancy: `run(effects, ui)` executes the list in order, inside the
    same `on_event` call that produced it, so the handler stays checked out
    exactly as today. An effect that can destroy a window or re-enter
    (`SyncWindows`, a modal) runs where its direct call runs today; nothing
    is deferred to a later message.
  - Pilot: zoom (`app/zoom.rs`). `Effect` starts with only what zoom needs:
    `RelayoutAll`, `ScheduleSave`, `RefreshSettings`, `RefreshShellDisplay`,
    `ShowZoomBar(ZoomBar)`, `HideZoomBar`, `SetTimer(Timer, u32)`, and
    `SetDesktopIconSize(i32)`. `App::commit` then becomes the first
    non-pilot user (`Refresh(Touched)` plus `ScheduleSave`). Grow the enum
    one event family at a time, each its own task, and stop if the shell
    grows instead of shrinking.
  - Once approved, three commits:
    - **R17a:** `app/effect.rs` with `enum Effect` and `App::run`, and no
      callers yet; the gates pass.
    - **R17b:** zoom's decisions as `fn zoom_effects(state, action) ->
      Vec<Effect>`, with `App::zoom` reduced to compute, then `run`. Tests
      assert the effects of each `ZoomAction` at the range ends and at
      the default (copy `zooming_stops_at_both_ends_of_its_range`).
    - **R17c:** `App::commit` returns its effects through the same `run`.
      **[needs human]** check: the F13 and F-57 rows of section V still pass.

- **R6. Undo design.** Superseded 2026-10-17: the maintainer answered
  ROADMAP open question 3 with D11, and section U holds the work. Was:
  Needs an answer to ROADMAP open question 3.

- [ ] **G6. Ctrl+Z undo.** Superseded 2026-10-17 by section U (U4 is the
  key). Was: After R6 settles the design (ROADMAP section 8, open question
  3).

- [x] **U1. Ids are never reused; a restore command (core).**
  - `Desktop::next_space_id` and `next_item_id` take the highest id ever
    handed out this session, not the highest present, so an id freed by a
    delete is not given to a newer space or item. Keep the high-water marks
    in `#[serde(skip)]` fields, seeded from the loaded ids, so the config
    file does not change.
  - `Command::RestoreSpaces { spaces: Vec<(SpaceId, Option<(usize, Space)>)> }`:
    for each id, `Some` puts that exact `Space` back at that index in the
    space order (replacing a space with the same id), and `None` removes the
    space. `Rejected` if a restored item id is held by another space or by
    the desktop layer. Its `touched_by` arm names every listed id.
  - Tests: a delete then a create gets a new id; a restore round-trips a
    deleted space with its items, look, and place in the order; a clashing
    item id is rejected; `None` removes.

- [x] **T2. Call roll-up "fold" (all crates; no behavior change).**
  - Rename in code: `Space::rolled_up` to `folded` (the field keeps
    `#[serde(rename = "rolled_up")]`, so the key on disk is unchanged),
    `Command::ToggleRollup` to `ToggleFold`, `SpaceScene::rolled_up` to
    `folded`, the app's `toggle_rollup`, and any `MenuId` (its value
    stays). Menu labels: "Fold" and "Unfold".
  - Docs: every current mention in `CONTEXT.md`, `ROADMAP.md`, `TASKS.md`,
    and `README.md` (not `archive/`).
  - Tests: a config holding `rolled_up = true` loads folded and saves
    `rolled_up` again. Done when `rg -i "roll.?up" crates docs README.md
    --glob '!docs/archive/**'` finds only D12, this task, and that test.

- [x] **T3. The animation model (core, config; pure).**
  - `deskspace-core/src/animation.rs`:
    - `Effect { None, Roll, Fade, Pixelate, Gradient, Dissolve, Fly, Crt,
      Burn }` (`snake_case` serde) and `EFFECTS`, one row per effect: its
      label and the options it uses (`EffectOption::{Detail,
      Direction}`).
    - `Kind { HideShow, NewSpace, Delete, Fold }` and `KINDS`, one row per
      kind: its label, the effects it offers, whether it offers "Same as
      new space" (Delete), and whether it has startup controls (New
      space).
    - `Edge { Top, Bottom, Left, Right, Nearest }`, the ranges
      (`DURATION_MS_RANGE` 100 to 1000, `DETAIL_RANGE`, `STAGGER_MS_RANGE`)
      and defaults.
  - Config: `WindowSettings.animations: Animations`, one
    `AnimationSettings { effect, duration_ms, detail, direction }` per kind
    (Delete's effect also has `SameAsNewSpace`), plus `startup:
    StartupOrder { AllAtOnce, OneAfterAnother }` and `stagger_ms`; the
    defaults are D12's, and values clamp on load (an effect a kind does
    not offer falls back to its default).
  - Tests: a config without `animations` loads D12's defaults; values
    clamp; every kind offers None and its default; every `EFFECTS` row's
    options match a test table.

- [x] **T4. The Animations page (app, platform).** **[needs human]**
  - Platform: `deskspace_platform::system_animations() -> bool`
    (`SPI_GETCLIENTAREAANIMATION`) and `Event::SystemAnimationsChanged`,
    raised by the controller for a `WM_SETTINGCHANGE` whose `wParam` is
    `SPI_SETCLIENTAREAANIMATION`.
  - App: `SystemLook` holds the switch, read at start and on the event,
    which refreshes an open form. A new Global-only page (at the end of
    `mod page`) built from `KINDS` and `EFFECTS`: per kind a heading,
    Effect, Duration, Detail, and Direction, each greyed when the chosen
    effect does not use it, and for New space the startup order and the
    delay (greyed for "All at once"). With Windows' switch off, every
    control is greyed and a note at the top says why, beside an "Open
    Windows animation settings…" button (`ButtonAction`) that opens D12's
    URI for the running Windows version with `Ui::open_url`. The field ids
    come from the tables, so `field_ids_are_unique` and the handler tests
    cover them.
  - Tests: the page per effect (greying), the switch off greys all, the
    button's URI per version, `apply_field` for each control.
    **[needs human]** check: the page at 100 % and 150 %, and flipping
    "Animation effects" in Windows while it is open.

- [x] **T5. Frame math (platform, pure).** `render/transition.rs`, beside
  `zoom_bar.rs`, with no Win32:
  - `Play { effect, way: In | Out, duration, delay, detail, edge }`, the
    data the app sends, and `Transition`, a running `Play`:
    `frame(now) -> TransitionFrame` (the window rect relative to the
    space's rect, the opacity, the mask, the block size, the glow) and
    `reverse(now)`, which turns it around so the next frame equals the
    current one. Reversing works at any frame, including during the delay.
  - Rects: Roll and Fold keep the top edge; Fly moves toward its edge and
    returns the part inside the work area, with the content offset the
    painter applies; CRT opens from the middle line, then closes to a dot.
  - Tests: per effect, in and out at 0 and at the end, monotonic between;
    `reverse` continuous at many points for every effect; None done at
    once; a delay holds frame 0; Fly never leaves the work area.

- [x] **T6. The painter draws a frame: Fade, Gradient, Dissolve, Pixelate
  (platform; no callers yet).** `Painter::paint` takes
  `Option<&TransitionFrame>` and applies it to the tint and the contents
  after the scene is drawn: Fade as a layer at the frame's opacity;
  Gradient as a layer whose opacity brush is a linear gradient; Dissolve
  as an opacity mask from a noise bitmap made once per window size and
  thresholded per frame; Pixelate by drawing the scene into a compatible
  render target (made when the animation starts) at 1 / block size, then
  back with nearest-neighbor scaling. Done when `None` paints exactly as
  today and the masks' pure parts (the noise, the gradient stops) have
  tests.

- [x] **T7. The painter draws CRT and Burn (platform; no callers yet).**
  CRT's bright line and fading glow over the opening rect; Burn as
  Dissolve plus a glowing band at the threshold, its width from Detail.
  The same done-when as T6.

- [x] **T8. Play hide and show (platform, app).** **[needs human]**
  - Platform: `win32/space_animation.rs` keeps a window's `Transition` in
    its `SpaceState` and a per-window timer, about 16 ms, that advances it,
    sets the window rect, and repaints with the frame.
    `set_spaces_visible(visible, play)`: an In paints frame 0 before
    `SW_SHOWNOACTIVATE`; an Out plays to the end, then `SW_HIDE` and puts
    the rect back. A request that finds a transition running reverses it.
    A hiding space refuses drops (`DROPEFFECT_NONE`). `set_rect` during a
    transition moves its target; `destroy_space` ends it.
  - App: `app/animation.rs` (pure) builds each `Play` from the settings
    and `SystemLook` (None while Windows' switch is off).
  - Docs: `CONTEXT.md` section 6 (Space windows and Rendering), section 8,
    `README.md`.
  - Tests: the `Play`s for each setting. **[needs human]** check: each
    effect hides and shows on Acrylic, Blur, Solid, and Clear spaces at
    100 % and 150 % (the blur steps at the start of a show and the end of
    a hide); the hotkey pressed twice quickly turns it around without a
    jump; a drop on a hiding space is refused; with Windows' "Animation
    effects" off, hiding is instant.

- [x] **T9. Play fold and unfold (platform, app).** **[needs human]**
  - `Ui::fold_space(id, rect, play)` plays a Roll from the window's rect
    to `rect`. While it runs, `space_window::set_rect` compares a new rect
    with its target, so the refresh after the command does not cut it
    short; a different rect (a move) ends it there.
  - App: an `Effect::FoldSpace(id)` (`app/effect.rs`) that calls
    `ui.fold_space` with the space's `visible_rect`; `App::toggle_fold`
    runs it before `commit`'s refresh. Hit tests read the model, which
    changes at once, as today.
  - Tests: `toggle_fold`'s effects. **[needs human]** check: fold and
    unfold from the menu and a title double-click at each backdrop; a
    second double-click mid-way turns it around.

- [x] **T10. Play new spaces and startup (platform, app).** **[needs
  human]**
  - `Ui::create_space` takes an `Option<Play>`: the window is created
    hidden and starts the In. The app passes New space's `Play` for every
    space that comes into being (menu, hotkey, a drop on the desktop,
    undo) and, at startup, one per space with `delay` from the startup
    order (pure: `startup_plays(spaces, order, stagger)`, sorted by
    position). "Start with spaces hidden" passes none.
  - Tests: `startup_plays` order and delays. **[needs human]** check:
    every New space effect from the menu and at startup, both startup
    orders, Fly from each edge and on a second monitor.

- [x] **T11. Play delete (platform, app).** **[needs human]**
  - `Ui::destroy_space(id, Option<Play>)` takes the window out of the
    runtime's spaces map at once (so it is gone to every lookup and to
    hiding), plays the Out, refuses drops and ignores input meanwhile, and
    destroys the window at the end. "Same as new space" plays New space's
    effect. Teardown destroys a dying window at once.
  - **[needs human]** check: each effect on delete, from the menu and the
    settings header; deleting a space still flying in turns it around;
    quitting mid-animation leaves nothing behind.

- [x] **T15. Three kinds: Startup, Delete, Fold (core, config, settings;
  D12 amended 2026-10-18).** Merge Hide and show and New space into
  Startup.
  - Core: `Kind::{Startup, Delete, Fold}`; Startup has the startup
    controls, and Delete follows Startup (`"same_as_startup"`). The kind
    rows' keys stay stored names.
  - Config: `Animations.startup_exit` holds Startup's settings, reading
    the old `hide_show` table through a serde alias; the old `new_space`
    table is ignored. `startup` (the order) and `stagger_ms` keep their
    keys, so a config written since T3 still loads. A stored
    `"same_as_new_space"` becomes Delete's default, Same as startup.
  - App: hiding, showing, and a new space play Startup with no stagger;
    startup staggers as before. The pages: Startup, Delete, Fold.
  - Tests: an old config with `hide_show` and `new_space` tables loads
    into Startup; Delete follows Startup; the pages and their greying.
    **[needs human]** check: the three pages; each Startup effect on hide,
    show, a new space, and startup.

- [x] **T16. Graceful exit (platform, app; D13).**
  - Platform: `Ui::quit_after(plays: Vec<(u64, Play)>, limit: Duration)`
    plays each space's Out (a hide, `space_animation`) and posts the usual
    `WM_CLOSE` to the controller when the last one ends, or at `limit`
    through a timer of its own (a `TIMERPROC`, so it takes no app `Timer`
    id). Called again while leaving, or with nothing to play, it quits at
    once. `WM_ENDSESSION` and console close keep closing at once.
  - App: `App::exit` builds the plays (Startup's Out, staggered in reverse
    reading order, the stagger shrunk so the last space ends within
    `EXIT_LIMIT`, 2 s) and is what the Exit menu entries and the update
    install call instead of `ui.quit`.
  - Tests: the exit plays: reverse order, the stagger shrunk to fit,
    nothing for hidden spaces, instant while Windows' switch is off.
    **[needs human]** check: Exit from the tray and a space menu with
    several spaces; with takeover on the icons come back only after the
    spaces have left; a second Exit mid-way quits at once; signing out
    does not wait.

- [x] **T17. Burn like paper (platform; D12 amended).** Replace Burn's
  cell dissolve in the painter:
  - A burn field made once per animation: smooth noise (a few octaves of
    value noise, bilinearly upsampled from a coarse grid) plus a bias that
    burns from the bottom up, so the front is ragged and organic. Detail
    sets the noise's scale.
  - Per frame, from the field and the progress, three bands ahead of the
    front: a soft scorch (browning), a char edge (near black), and the
    glowing edge (yellow through orange to red), with everything behind
    the front gone. Written into a coarse premultiplied bitmap drawn with
    linear interpolation: one as the content's opacity mask, one as the
    color laid over it.
  - Pure parts tested: the field's range and its bottom-up bias, and the
    bands at a given threshold. Dissolve keeps the cell mask.

- [x] **T18. Flames around the frame (platform).** A click-through,
  layered, never-activated overlay window (as `drag_ghost.rs`), sized to
  the space plus a margin above and beside it, placed directly above the
  space in z-order while Burn plays and destroyed after. Each frame draws
  flame tongues rising from the burning front and up the frame's edges:
  tapered gradients (yellow core, orange, red, transparent) whose heights
  flicker over time, with a few embers drifting up, then
  `UpdateLayeredWindow`. The flame layout (where tongues stand and how
  high, per progress and time) is pure and tested. **[needs human]**
  check (with T17): Burn on Startup and Delete looks like burning paper
  on each backdrop, the flames rise past the frame without a box around
  them, reversing mid-burn un-burns, and nothing is left behind.

- [x] **X1. The debug data types (platform; data only, no window).**
  - `deskspace-platform/src/ui.rs`, each with a `///` doc:
    - `enum LogLevel { Error, Warn, Info, Debug, Trace }` (`Copy`,
      `PartialEq`, `Eq`, `Debug`), with `pub const ALL: [Self; 5]` in that
      order and `pub fn label(self) -> &'static str` ("Error", "Warn",
      "Info", "Debug", "Trace").
    - `struct LogLine { seq: u64, millis: u64, level: LogLevel, target:
      String, message: String }`, all `pub` (`Clone`, `Debug`,
      `PartialEq`). `seq` counts from 1 and never repeats; `millis` is the
      time since the process started.
    - `struct DebugPanel { sections: Vec<DebugSection>, console_level:
      LogLevel }`, `struct DebugSection { title: String, controls:
      Vec<DebugControl> }`, `struct DebugControl { id: u32, label: String,
      kind: DebugControlKind }`, and `enum DebugControlKind { Button {
      enabled: bool }, Toggle { on: bool }, Choice { options: Vec<String>,
      selected: usize }, Readout { text: String }, Table { columns:
      Vec<String>, rows: Vec<Vec<String>> } }`. All `Clone`, `Debug`,
      `PartialEq`.
    - `DebugPanel::same_shape(&self, other) -> bool`: the same section
      titles, and in each the same control ids, labels, and kind variants
      (values may differ).
  - `deskspace-platform/src/input.rs`: `enum DebugEvent { Pressed { id:
    u32 }, Toggled { id: u32, on: bool }, Chose { id: u32, index: usize },
    ConsoleLevel { level: LogLevel }, Closed }`, and the variant
    `Event::Debug(DebugEvent)`, both with `///` docs.
  - `lib.rs`: re-export the new types beside the other `ui` and `input`
    ones.
  - `deskspace-app/src/app.rs` `on_event`: the arm `Event::Debug(_) => {}`
    with `// TODO(X4): the debug window's events.` above it.
  - Tests in `ui.rs`: `LogLevel::ALL` labels are unique, and `same_shape`
    is true for a changed value and false for a changed id, label, kind,
    or section title.
  - Done when: the gates pass. No docs change yet.

- [x] **X2. The debug window and its panel (platform).** Needs X1.
  - New module `win32/debug_window/` (add `mod debug_window;` in
    `win32/mod.rs`), with a one-line `//!` header on each file:
    - `layout.rs` (pure): `fn debug_layout(width: i32, height: i32, scale:
      f32) -> DebugLayout`, where `DebugLayout { rail: Rect, panel: Rect,
      console: Rect }`. Constants at 96 DPI, scaled: `RAIL_WIDTH` 168,
      `EDGE` 12, `MIN_WIDTH` 760, `MIN_HEIGHT` 560, `CONSOLE_MIN` 200. The
      rail is the left column; the panel fills the rest of the top half;
      the console takes the bottom half of the client (at least
      `CONSOLE_MIN`) across the full width. Tests: no rect overlaps
      another, all fit the client, and the console grows with the height.
    - `build.rs`: creates the controls. The rail is a list box of section
      titles. Every section is built at once into a panel child window
      (`WS_VSCROLL`, scrolled with the wheel and the bar when the section
      is taller than the panel); only the chosen section's controls are
      shown. One row per control: a label on the left (`LABEL_WIDTH` 220
      at 96 DPI) and the control right of it, except a `Button`, which is
      a push button captioned with its label. `Toggle` is a check box,
      `Choice` a drop-down list combo box, `Readout` a read-only multi-line
      edit (as tall as its lines, at most 8, then it scrolls), and `Table`
      a report-view list view with the given columns, 8 rows tall.
      Control ids are `CONTROL_BASE + DebugControl::id`, with
      `CONTROL_BASE` = 1000.
    - `mod.rs`: the window class (`window_class` and `window_state`, as
      every window does), `WS_OVERLAPPEDWINDOW`, titled "DeskSpace
      Debug", minimum size from `layout.rs` (`WM_GETMINMAXINFO`),
      re-laid out on `WM_SIZE` and `WM_DPICHANGED`. Colors, fonts, and
      brushes come from `dialog::DialogChrome`, and controls are themed
      with `dark_mode` as `shortcut_dialog.rs` does. A button press,
      toggle, or choice raises the matching `DebugEvent` through
      `runtime::dispatch` (the last statement in its branch;
      `ENGINEERING.md` "Re-entrancy"). Closing destroys the window and
      raises `DebugEvent::Closed`.
  - `win32/runtime/`: a `debug: Option<HWND>` field on `Runtime`, beside
    `settings`. In `ui_chrome.rs`: `Ui::open_debug(&DebugPanel)` (create,
    or bring the open one to front), `Ui::update_debug(&DebugPanel)`
    (when `same_shape`, set only the changed values; otherwise log a
    `warn` and rebuild, as `warn_on_shape_change` does for settings),
    `Ui::close_debug()`, and `Ui::is_debug_open() -> bool`. The message
    loop passes the debug window to `IsDialogMessageW`, as it does the
    settings window, so Tab moves between controls. The teardown destroys
    it with the settings window.
  - Leave the console rect empty; X3 fills it.
  - Docs: `CONTEXT.md` section 6 module map, the "Windows" row: add
    `debug_window`. The file map in this file: a row "The debug window
    (`--debug`)" naming `win32/debug_window/`.
  - Done when: the gates pass and the layout tests pass. Its on-screen
    check is X5's.

- [x] **X3. The console (platform).** Needs X2.
  - Pure model, `deskspace-platform/src/debug_console.rs` (crate root, no
    `unsafe`, beside `edit.rs`): `ConsoleModel` with `CONSOLE_CAPACITY` =
    10,000 lines (the oldest is dropped), `push(&[LogLine])`,
    `set_filter(&str)` (case-insensitive, matches the target or the
    message; empty shows all), `set_paused(bool)` (while paused, pushed
    lines are held and appear on resume), `clear()`, `shown_len()`,
    `shown(index) -> &LogLine`, and `fn format_line(&LogLine) -> String`
    as `mm:ss.mmm LEVEL target message`. Tests for each, including the
    capacity drop and a filter applied to held lines on resume.
  - `win32/debug_window/console.rs`: a toolbar row (a Level combo box
    listing `LogLevel::ALL`, a Filter edit, Pause and Auto-scroll check
    boxes with Auto-scroll on, and Clear, Copy, and "Save log…" buttons)
    above a virtual report list view (`LVS_OWNERDATA`; columns Time,
    Level, Target, Message) that reads rows from the `ConsoleModel`. Text
    color by level through `NM_CUSTOMDRAW`: Error red, Warn amber, Debug
    and Trace the dimmed text color of `UiColors`, Info the normal one,
    with the colors as named constants. After new lines, set the item
    count (`LVM_SETITEMCOUNT` with `LVSICF_NOSCROLL`) and, with
    Auto-scroll on, show the last row (`LVM_ENSUREVISIBLE`).
  - The Level combo box shows `DebugPanel::console_level` and raises
    `DebugEvent::ConsoleLevel`. Filter, Pause, Auto-scroll, and Clear stay
    inside the platform; they are view state, like scrolling.
  - Copy puts every shown line (`format_line`, one per line) on the
    clipboard as `CF_UNICODETEXT` through a new `pub(crate) fn
    set_text(owner, &str)` in `win32/clipboard.rs`; also expose it as
    `Ui::set_clipboard_text(&str)` for X12. "Save log…" asks for a path
    with a new `dialog::pick_save_path(owner, title, default_name,
    filter)` over `IFileSaveDialog` beside `pick_files` (C10b reuses it;
    say so in its `///` doc) and writes the shown lines as UTF-8.
  - `Ui::append_debug_log(&[LogLine])` pushes into the open window's
    model and does nothing when it is closed.
  - Never log from the console's own code paths (a log line there would
    feed itself); the model has no `tracing` calls.
  - Done when: the gates pass and the model tests pass. Its on-screen
    check is X4's.

- [x] **X4. Capture the log, and `--debug` (app).** Needs X3.
  **[needs human]**
  - `cli.rs`: `pub debug: bool` with `#[arg(long, hide = true)]` and a
    `///` doc in the style of `on_top`'s ("Development aid: open the
    debug window, with a live log console and test tools (ROADMAP
    D14).").
  - New `deskspace-app/src/debug_log.rs` (declare it in `main.rs`):
    - `LogBuffer`, a `Clone` handle over `Arc<Mutex<..>>` holding at most
      `CAPACITY` (10,000) `LogLine`s and the next `seq`: `push(level,
      target, message)` stamps `seq` and `millis` (from a
      `static START: OnceLock<Instant>` set on first use) and drops the
      oldest past the capacity; `since(seq) -> Vec<LogLine>` returns the
      lines with a higher `seq`.
    - `ConsoleLayer { buffer: LogBuffer }`, implementing
      `tracing_subscriber::Layer<S>`. `on_event` reads the fields with a
      `LineVisitor` (`tracing::field::Visit`): the `message` field first,
      then a space and `key=value` for each other field, in order.
    - `ConsoleLevel`, wrapping a `tracing_subscriber::reload::Handle` to
      a `LevelFilter`, with `set(LogLevel)` that logs a `warn` if the
      reload fails.
    - `pub struct DebugLog { pub buffer: LogBuffer, pub level:
      ConsoleLevel }`.
    - Tests: the ring drops the oldest and `seq` keeps counting; `since`
      returns only newer lines; under
      `tracing::subscriber::with_default`, `info!(count = 2, "hello")`
      records level Info, this module's target, and the message `hello
      count=2`.
  - `main.rs`:
    - `init_tracing(verbosity, debug: bool) -> Option<DebugLog>`. Build
      `tracing_subscriber::registry()` with the existing `fmt` layer
      (`with_target(false)`, under the same `EnvFilter`, as a per-layer
      filter), plus, only when `debug` is set, `ConsoleLayer` under a
      `reload::Layer` of `LevelFilter::DEBUG`. `RUST_LOG` and `-v` still
      decide stderr alone. If the locked `tracing-subscriber` will not
      accept a reloadable per-layer filter, stop and ask; do not change
      the dependency.
    - Right after `init_tracing`, in every build, with or without
      `--debug`: a panic hook (`std::panic::set_hook`) that logs
      `tracing::error!(%info, "panic")` and then calls the previous hook
      (taken with `std::panic::take_hook`).
    - Pass the `Option<DebugLog>` to `bootstrap::run`.
  - `bootstrap.rs`: after `App::new`, set `app.debug =
    debug_log.map(|log| DebugTools::new(log, cli.config.is_none()))`, as
    `app.layer` is set.
  - New `app/debug.rs` (`mod debug;` in `app.rs`): `pub(super) struct
    DebugTools { log: DebugLog, sent: u64, real_config: bool, started:
    Instant }` and an `App` field `debug: Option<DebugTools>` (`None` in
    `App::new`).
    - `App::open_debug(ui)`: sets `sent` to 0 (so a reopened window gets
      the whole ring again), `ui.open_debug(&self.debug_panel())`, and
      starts the timer. `debug_panel` returns an empty `sections` list for
      now, with `console_level` as last set (start at `Debug`).
    - A `Timer::DebugLog` variant (the next free value; add it to
      `Timer::ALL` and an arm in `App::on_timer`), re-armed every
      `DEBUG_LOG_MS` (100) while `ui.is_debug_open()`: push
      `log.buffer.since(sent)` with `ui.append_debug_log` and move `sent`.
      A timer, not a waker, so a line logged while draining cannot feed
      itself.
    - `App::on_debug(ui, DebugEvent)`: `ConsoleLevel` calls
      `log.level.set`, `Closed` clears the timer, the rest is ignored
      until X5. Replace X1's TODO arm with `Event::Debug(event) =>
      self.on_debug(ui, event)`.
    - On `Started`, when `self.debug` is set, call `open_debug` last.
  - Docs: `CONTEXT.md` section 3, the `app/` table: a `debug` row ("the
    debug window's panel and actions, and the log console's feed (ROADMAP
    D14)"). Section 7 step 1: one sentence that `--debug` adds a console
    layer with its own level and every build logs panics. Section 8's CLI
    flags: add `--debug` to the hidden development flags. The file map
    row from X2 also names `debug_log.rs` and `app/debug.rs`.
  - **[needs human]** check: the window opens with the console already
    holding the startup lines; moving a space adds lines live; Level
    Trace shows trace lines and Info stops new debug ones; Filter, Pause
    and resume, Auto-scroll, Clear, Copy (paste into Notepad), and "Save
    log…" work; a `-Release` build shows the log with no console window;
    without `--debug` nothing opens and stderr logs as before.

- [x] **X5. The panel framework and the tray entry (app).** Needs X4.
  **[needs human]**
  - Turn `app/debug.rs` into `app/debug/mod.rs` (pure move first, in the
    same commit) and add:
    - `ids.rs`: `#[repr(u32)] enum DebugId` with `ALL`, `fn id(self) ->
      u32`, and `fn from_id(u32) -> Option<Self>`, copying `MenuId`'s
      pattern; first variant `SpacesVisible = 1`. Tests
      `debug_ids_round_trip` and `debug_ids_are_unique`.
    - `table.rs`: `enum DebugSectionId { Spaces, Config, Diagnostics,
      Windows, System, Animations, Takeover, Simulate }` with `ALL` and
      `title()` (those words), and `DEBUG_CONTROLS: &[DebugRow]`, where
      `DebugRow { id: DebugId, section: DebugSectionId, label: &'static
      str }`. First row: `SpacesVisible`, `Windows`, "Show spaces". Test
      `every_control_has_one_row` (each `DebugId::ALL` entry appears
      exactly once).
    - `mod.rs`: `debug_panel` builds one `DebugSection` per
      `DebugSectionId` that has rows, in `ALL` order, each control from
      its row and `self.debug_control(id) -> DebugControlKind`, an
      exhaustive `match` on `DebugId` (`SpacesVisible` is `Toggle { on:
      self.visible }`). `on_debug` maps `Pressed`, `Toggled`, and `Chose`
      through `DebugId::from_id` to `self.on_debug_control(ui, id,
      input)`, where `enum DebugInput { Press, Toggle(bool),
      Choose(usize) }`, an exhaustive `match` (`SpacesVisible` calls
      `self.set_visible(ui, on)`). After any control, push the panel
      again.
    - `debug_confirm(ui, question) -> bool`: `ui.confirm("DeskSpace
      Debug", ..)` with the question, a blank line, `Configuration:
      <path>`, and, when `real_config`, a blank line and "This is your
      real configuration, not a scratch --config." Test the text through a
      pure `fn debug_question(question, path, real) -> String`.
    - A `Timer::DebugRefresh` (next free) every `DEBUG_REFRESH_MS` (1000)
      while the window is open: build the panel and call
      `ui.update_debug` only when it differs from the last one pushed
      (keep it in `DebugTools`).
  - Tray (the menu-entry recipe): `MenuId::Debug` (next free) with
    "Debug…", after "Settings…" in `show_tray_menu`, only when
    `self.debug.is_some()`; its arm calls `self.open_debug(ui)`.
  - Docs: `CONTEXT.md` section 8's tray line: "and, with `--debug`,
    Debug…".
  - **[needs human]** check: the rail lists "Windows" with a "Show
    spaces" check box that hides and shows the spaces, and follows the
    tray's Hide/Show within a second; close the window, and the tray's
    "Debug…" brings it back with the earlier log; without `--debug` the
    tray has no "Debug…"; the window themes dark and light, resizes
    (the console grows), and stops at its minimum size; Tab moves
    between controls.

- [x] **X6. Split the confirmation out of `delete_space` (app; pure
  refactor).** In `app/files.rs`:
  - `pub(super) enum StoredFiles { ToDesktop, ToRecycleBin }` with a
    `///` doc on each.
  - `delete_space(ui, id)` keeps the confirmation and then calls the new
    `pub(super) fn delete_space_now(&mut self, ui, id, stored:
    StoredFiles)`, which holds the rest of today's body unchanged. With
    `ToRecycleBin` it first sends the space's stored files that still
    exist (a pure `fn stored_paths(space: &Space, storage: &ItemStore) ->
    Vec<PathBuf>`) to the Recycle Bin with `ui.delete_files(&paths,
    false)`; if that returns false, it logs an `error`, refreshes and
    saves the space, and returns, keeping it. Then the existing release
    loop runs: `release_item` skips files that no longer exist, so it only
    removes the items.
  - Every existing caller keeps `ToDesktop` (through `delete_space`), so
    nothing a user sees changes.
  - Tests: `stored_paths` lists a stored item and skips one listed in
    place (copy the fixtures in `files.rs`'s tests).
  - Done when: the gates pass, and the test count only grows.

- [x] **X7. Delete all spaces (app).** Needs X5 and X6.
  **[needs human]**
  - Control: `DeleteAllSpaces`, section Spaces, "Delete all spaces", a
    `Button` enabled when there is at least one space.
  - First, in `app/debug/mod.rs` (moved here from X5, which had no caller
    for it, so it would have been dead code):
    - `debug_confirm(ui, question) -> bool`: `ui.confirm("DeskSpace
      Debug", ..)` with the question, a blank line, `Configuration:
      <path>`, and, when `real_config`, a blank line and "This is your
      real configuration, not a scratch --config." Test the text through a
      pure `fn debug_question(question, path, real) -> String`.
  - Handler: count the spaces, their items, and their stored files that
    exist; ask with `debug_confirm` and the question from a pure `fn
    delete_all_question(spaces, items, stored) -> String`: "Delete all
    {spaces} spaces?" then a blank line and "They hold {items} items.
    {stored} stored files go to the Recycle Bin; items shown in place
    are only removed from their spaces." Then collect the ids first and
    call `delete_space_now(ui, id, StoredFiles::ToRecycleBin)` for each.
    Log how many were deleted and how many were kept.
  - Tests: `delete_all_question`, including one space and no stored
    files.
  - **[needs human]** check: with three spaces holding dropped files and
    one Shift-dropped file, "Delete all spaces" asks once with the right
    counts and the scratch config's path; Cancel changes nothing; OK
    plays each space's Delete animation; the dropped files are in the
    Recycle Bin, the Shift-dropped file is where it was, and nothing new
    is on the desktop. Then start without `--config` and check that the
    warning line shows. Press Cancel there.

- [x] **X8. Create test spaces (app).** Needs X5. **[needs human]**
  - `menus.rs`: `create_space` returns the `Option<SpaceId>` that
    `open_new_space` already gives; the menu callers ignore it.
  - `lib.rs` of the platform: re-export `win32::shortcuts::resolve_path`
    (it is `pub(crate)` today; make it `pub`).
  - New `app/debug/test_spaces.rs`: `struct TestSpace { title: &'static
    str, shortcuts: &'static [TestShortcut] }`, `struct TestShortcut {
    name: &'static str, target: &'static str, arguments: &'static str }`,
    and `TEST_SPACES`:

    | Title | Name | Target | Arguments |
    |-------|------|--------|-----------|
    | Test: Apps | Notepad | `%WINDIR%\System32\notepad.exe` | |
    | | Calculator | `%WINDIR%\System32\calc.exe` | |
    | | Character Map | `%WINDIR%\System32\charmap.exe` | |
    | Test: Tools | Command Prompt | `%WINDIR%\System32\cmd.exe` | `/k echo DeskSpace test shortcut` |
    | | PowerShell | `%WINDIR%\System32\WindowsPowerShell\v1.0\powershell.exe` | `-NoExit -Command "Write-Output 'DeskSpace test shortcut'"` |
    | | Task Manager | `%WINDIR%\System32\Taskmgr.exe` | |
    | Test: Places | Documents | `%USERPROFILE%\Documents` | |
    | | Downloads | `%USERPROFILE%\Downloads` | |
    | | Windows | `%WINDIR%` | |
    | | Temp | `%TEMP%` | |
    | Test: Web | DeskSpace on GitHub | `https://github.com/generalchad/DeskSpace` | |
    | | Example | `https://example.com` | |

  - Control: `CreateTestSpaces`, section Spaces, "Create test spaces", a
    `Button`, always enabled.
  - Handler: for each `TestSpace`, `create_space(ui, cursor_pos())`
    (New space's own placement, ROADMAP D10), then `commit`
    `Command::RenameSpace` with its title. Turn each shortcut into a
    `ShortcutSource::Custom(CustomShortcut { .. })`: `ShortcutTarget::parse`
    the target; a `Path` goes through `resolve_path`, and one that does
    not resolve is logged at `warn` and skipped. Hand them all to
    `add_shortcuts(ui, id, at, sources)`. Log the count made.
  - Tests: every `TEST_SPACES` target parses (`ShortcutTarget::parse` is
    `Some`), the web targets parse as `Url` and the rest as `Path`, and
    titles are unique.
  - **[needs human]** check: four spaces open in a row beside the
    existing ones, titled as above; every shortcut opens what it names
    (the Tools ones show the test text); a second press makes four more
    without overlapping; then "Delete all spaces" removes them and leaves
    nothing on the desktop.

- [x] **X9. The rest of Spaces (app).** Needs X5. **[needs human]** Each
  is a `Button` in section Spaces, enabled when there is a space:
  - `UnfoldAll`, "Unfold every space": `commit` `Command::ToggleFold` for
    each folded space.
  - `UnlockAll`, "Unlock every space": `commit` `Command::ToggleLock` for
    each locked space.
  - `RefreshAll`, "Refresh every space": `self.refresh_spaces(ui)` (what
    F5 does).
  - `PruneNow`, "Prune missing items": `self.prune_all_missing(ui)`.
  - `StressSize`, "Stress space size", a `Choice` of 50, 200, and 1000
    (a `STRESS_SIZES` table), kept in `DebugTools`, and `CreateStress`,
    "Create a stress space": one space (as X8 does) titled "Test: Stress
    {n}", with `n` shortcuts to `%WINDIR%\System32\notepad.exe` named
    `Item 0001` upward, through one `add_shortcuts` call.
  - Tests: through `fixtures.rs`, the commands the first two send leave
    every space unfolded and unlocked; `STRESS_SIZES` is ascending.
  - **[needs human]** check: each button does what it says; a 1000-item
    stress space opens and scrolls smoothly, and "Delete all spaces"
    removes it.

- [x] **X10. Config (app).** Needs X5. **[needs human]** Section Config:
  - `ConfigPath`, "Configuration", a `Readout`: the config file path and
    "(scratch)" or "(real)"; `StoragePath`, "Storage", a `Readout`: the
    storage folder.
  - `DumpConfig`, "Write the configuration to the log": serialize
    `self.config` with the same TOML writer `ConfigStore::save` uses and
    log it at `info`, one line per TOML line.
  - `OpenConfigFolder` and `OpenStorageFolder`: `ui.open_path` on the
    config file's folder and on the storage folder (create the storage
    folder first if it is missing, as `add_shortcuts` does).
  - **[needs human]** check: the readouts match `--config`; the dump shows
    every space in the console; both folders open in Explorer.

- [x] **X11. Process and cache facts (platform).** New module
  `win32/debug_info.rs` (one `//!` line; `mod` in `win32/mod.rs`):
  - `pub fn process_stats() -> ProcessStats { working_set: u64,
    gdi_objects: u32, user_objects: u32 }` over `GetProcessMemoryInfo`
    and `GetGuiResources` (cite their Microsoft Learn pages). If a
    `windows` crate feature is needed, add it to `deskspace-platform`
    only, and `cargo deny check all` must still pass.
  - `IconCache::len(&self) -> usize` in `win32/icons.rs`.
  - Re-export `process_stats` and `ProcessStats` from `lib.rs`.
  - Test: `process_stats()` reports a nonzero working set and at least
    one USER object once a window exists (create a message-only window in
    the test, or skip the USER part if that is not possible and say so).

- [x] **X12. Diagnostics (app).** Needs X5 and X11. **[needs human]**
  Section Diagnostics:
  - `Build`, a `Readout`: the version (`CARGO_PKG_VERSION`), "debug" or
    "release" (`cfg!(debug_assertions)`), and the executable's path.
  - `Model`, a `Readout`: the number of spaces, items, stored files, and
    desktop-layer items.
  - `Resources`, a `Readout`: uptime (from `DebugTools::started`), the
    working set in MB, GDI and USER objects, and icon-cache entries.
    `Timer::DebugRefresh` keeps it current.
  - `Updates`, a `Readout`: `deskspace_update::is_configured()`,
    `is_installed()`, and the ready version or "none".
  - `CopyDiagnostics`, "Copy diagnostics": these readouts, X16's System
    readout once it exists, and the last 200 log lines (`format_line`
    style, from `LogBuffer::since`), joined into one text and put on the
    clipboard with `ui.set_clipboard_text`.
  - Tests: a pure `fn diagnostics_text(sections: &[(&str, &str)], lines:
    &[LogLine]) -> String` puts a heading before each readout and the
    lines last.
  - **[needs human]** check: the values look right and Resources ticks
    once a second; creating and deleting spaces moves the counts; Copy
    diagnostics pastes into Notepad as a readable report.

- [x] **X13. Window and monitor tables, live on-top (platform).** Needs
  X11.
  - In `win32/debug_info.rs`: `pub struct WindowRow { class: String,
    title: String, space: Option<u64>, rect: Rect, dpi: u32, visible:
    bool }`, and `Ui::debug_windows() -> Vec<WindowRow>`: every top-level
    window of this process in z-order, top first (`EnumWindows` filtered
    by `GetWindowThreadProcessId`), then, with a desktop host, the host's
    child windows that are spaces. `space` is the id when the window is
    in the runtime's spaces map. `scripts/zorder.ps1` does the same from
    outside; follow it.
  - `pub struct MonitorRow { rect: Rect, work: Rect, dpi: u32, primary:
    bool }` and `pub fn monitors() -> Vec<MonitorRow>` over
    `EnumDisplayMonitors`.
  - `Ui::set_on_top(bool)`: sets `Runtime::on_top` and moves every space
    window to where a space created with that setting goes. Read every
    place `on_top` is read today and use the same call; do not add a
    second placement rule.
  - Tests: none possible without windows beyond `monitors()` returning at
    least one primary monitor.

- [x] **X14. Windows (app).** Needs X5 and X13. **[needs human]** Section
  Windows, after X5's "Show spaces":
  - `OnTop`, "Keep spaces above other windows", a `Toggle`: starts as
    `--on-top` and calls `ui.set_on_top(on)`.
  - `WindowTable`, "Windows", a `Table` with columns Class, Title,
    Space, Rect, DPI, Visible, from `ui.debug_windows()`.
  - `MonitorTable`, "Monitors", a `Table` with columns Rect, Work area,
    DPI, Primary, from `monitors()`.
  - Both tables refresh with `Timer::DebugRefresh`.
  - Tests: a pure `fn rect_text(Rect) -> String` ("x, y  w × h").
  - **[needs human]** check: on-top lifts the spaces above Notepad and
    off drops them back to the desktop; the window table lists every
    space, the tray's controller window, and the debug and settings
    windows, in z-order, and follows a space moved to another monitor;
    the monitor table matches Windows' display settings.

- [x] **X15. System facts (platform).** In `win32/debug_info.rs`:
  - `pub fn transparency_enabled() -> Option<bool>`: the `EnableTransparency`
    DWORD under
    `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`.
  - `Ui::hook_state() -> HookState { editing: bool, watch_selection: bool,
    watch_paste: bool, outside_clicks: bool }`, read from the flags
    `win32/keyboard.rs` and `win32/outside_click.rs` already keep. If one
    is not kept anywhere, report `false` and add a **Found while
    working** line rather than adding state.
  - `pub fn watchdog_running() -> Option<bool>`: whether the watchdog
    process `arm_watchdog` launched is still alive. If `watchdog.rs`
    keeps no handle or id for it, return `None` and add a **Found while
    working** line.
  - Re-export them from `lib.rs`. Tests: none beyond compiling.

- [x] **X16. System (app).** Needs X5 and X15. **[needs human]** Section
  System, all `Readout`s refreshed by `Timer::DebugRefresh`:
  - `Theme`: Windows dark or light (`system_uses_dark_mode`), the accent
    (`system_accent_color` as `#RRGGBB`, or "unknown"), Windows'
    "Animation effects" (the value `Event::SystemAnimationsChanged`
    keeps), and transparency effects (`transparency_enabled`).
  - `Explorer`: the app's `SystemLook`, printed with `{:#?}` (derive
    `Debug` on it if it lacks one).
  - `Hotkeys`: one line per `HOTKEY_BINDINGS` row: its label, its binding
    (or "unbound"), and whether it registered, if `app/hotkeys.rs` keeps
    that; otherwise the binding alone.
  - `Hooks`: `ui.hook_state()`, one line per flag.
  - **[needs human]** check: switching Windows between dark and light,
    changing the accent, and turning off transparency each show within a
    second; selecting an item turns on `watch_selection`; a rename turns
    on `editing`.

- [x] **X17. Slow motion and ignoring Windows' switch (app; pure).**
  Needs X5.
  - `app/animation.rs`: `#[derive(Default)] pub(super) struct
    AnimationTweak { speed: u32, ignore_windows: bool }`, where 0 and 1
    both mean normal speed, as an `App` field (not inside `DebugTools`, so
    `play` needs no `Option`). `play_for` takes it: `ignore_windows`
    plays as if Windows' switch were on, and `speed` multiplies the
    duration and the delay. Exit still ends within `EXIT_LIMIT`.
  - Controls in section Animations: `Speed`, "Speed", a `Choice` from a
    `SPEEDS` table (`(1, "Normal")`, `(2, "½×")`, `(4, "¼×")`, `(10,
    "⅒×")`), and `IgnoreWindows`, "Animate even when Windows' Animation
    effects is off", a `Toggle`.
  - Tests beside `the_settings_become_the_play`: speed 4 makes the
    duration and delay four times longer; `ignore_windows` plays the
    effect with Windows' switch off (copy
    `nothing_plays_while_windows_animations_are_off`); the default tweak
    changes nothing.
  - Add the two controls to the T8 row in section V ("checked at ¼×
    too") rather than a new row.

- [x] **X18. Play an effect on a space (platform, app).** Needs X17.
  **[needs human]**
  - Platform, `win32/space_animation.rs`: `Ui::preview_play(id: u64,
    out: Play, back: Play)` plays `out` on that space's window and, when
    it ends, `back`, leaving the window shown. Use the machinery
    `show_space` and `destroy_space` use, so it can turn around mid-way
    like any animation. It never touches the model.
  - App, section Animations: `PreviewSpace`, "Space", a `Choice` of the
    space titles in model order (keep the ids in `DebugTools`; a deleted
    space falls back to the first); `PreviewKind`, "Kind", a `Choice` of
    Startup and Delete (Fold is checked from its own menu); and
    `PreviewPlay`, "Play on the space", a `Button` that calls
    `ui.preview_play(id, self.play(kind, Way::Out), self.play(kind,
    Way::In))`, so the current settings and X17's tweak apply.
  - Tests: through a pure helper, the chosen index maps to the right id
    and a stale index falls back to the first space.
  - **[needs human]** check: every effect, played on a space at ¼×,
    leaves and comes back in place; pressing Play again mid-way turns it
    around; the other spaces and the model do not change.

- [x] **X19. Takeover (app).** Needs X5 and X15. **[needs human]** Section
  Takeover:
  - `Takeover`, "Take over the desktop (this session)", a `Toggle`
    showing whether takeover is active now. Off restores Explorer's icons
    and removes the layer; on takes over again. It must not change
    `config.window.take_over_desktop` or save. If `apply_takeover`
    (`app/view.rs`) reads only that config field, add a session override
    to `LayerState` that it honours; if that needs more than a few lines,
    stop and ask.
  - `ResyncLayer`, "Re-sync the desktop layer", a `Button` enabled while
    takeover is active: `self.resync_desktop_layer(ui)`.
  - `TakeoverState`, a `Readout`: active or not, the layer's item count,
    and the watchdog (`watchdog_running`: "running", "not running", or
    "unknown").
  - **[needs human]** check: with takeover on, the toggle off brings
    Explorer's icons back at once and they stay back past the 5-second
    health timer; on hides them and draws the layer; Re-sync picks up a
    file added to the Desktop folder; the saved config's
    `take_over_desktop` is unchanged afterwards.

- [x] **X20. Simulate (platform, app).** Needs X5. **[needs human]**
  Section Simulate:
  - Platform: `Ui::simulate_taskbar_created()` posts the registered
    `TaskbarCreated` message to the controller, the same value the
    controller already registers and handles.
  - `ThemeChanged`, "Windows theme changed", a `Button`:
    `self.on_event(ui, Event::SystemThemeChanged)`, the event Windows
    sends.
  - `ExplorerRestarted`, "Explorer restarted", a `Button`:
    `ui.simulate_taskbar_created()`.
  - `ReadOnlyConfig`, "Make the configuration file read-only", a `Toggle`
    (it asks first when turning on): sets or clears the read-only
    attribute of the config file with `std::fs::set_permissions`. On
    `CloseRequested`, clear it again if this toggle set it, before
    `flush_save`, so a session never leaves the file read-only.
  - `CrashNow`, "Crash now", a `Button` that asks first, then
    `panic!("crash requested from the debug window")`. The panic hook
    (X4) logs it; the process then ends where the panic leaves the window
    procedure, as a real bug would.
  - **[needs human]** check: "Windows theme changed" repaints the spaces
    and the settings window; "Explorer restarted" re-adds the tray icon
    and, with takeover on, re-hides Explorer's icons; with the config
    read-only, moving a space shows B29's one "could not save" box, and
    after Exit the file is writable again; "Crash now" with takeover on
    ends DeskSpace and the watchdog brings Explorer's icons back within a
    few seconds, and a debug build's console shows the panic line.
- [x] **T12. Composition device and space tree (platform).** **[needs
  decision]** until T1's findings are recorded, then **[needs human]**.
  - `win32/composition.rs`: one per UI thread, released in the runtime
    teardown before `OleUninitialize`, as `Graphics` is: a
    `DispatcherQueueController`, the `Compositor`, a Direct3D 11 and
    Direct2D device, and a `CompositionGraphicsDevice`, recreated on device
    loss.
  - A top-level space window gets `WS_EX_NOREDIRECTIONBITMAP` and a
    desktop window target: a root visual (the corner clip), a backdrop
    sprite (the host backdrop brush for Acrylic and Blur, nothing for Clear
    and Solid), and a content sprite whose `CompositionDrawingSurface` the
    painter draws into. Split the painter from `HwndCanvas` first if the
    drawing code needs it, as its own commit.
  - The accent policy stays for Windows 10, desktop-hosted spaces, the
    desktop layer, and any failure to start composition, chosen once at
    startup and logged.
  - **[needs human]** check: the R3 list passes unchanged on Windows 11
    with each backdrop, and on Windows 10 nothing changes at all.

- [x] **T13. The blur follows the frame (platform).** **[needs human]**
  - On composition spaces, each frame also sets the backdrop: its opacity
    and rect, and the frame's mask. A `CompositionMaskBrush` cannot take
    the backdrop (T1), so the backdrop sprite's brush is an effect brush
    whose graph is D2D's AlphaMask, with the backdrop as its source and the
    painter's mask as its mask (decided 2026-10-18, maintainer, over
    splitting the backdrop into strips and cells). The spike's `AlphaMask`
    (`IGraphicsEffect` and `IGraphicsEffectD2D1Interop`) is the pattern; if
    T1's M key does not render, stop and ask. The blur no longer steps
    there.
  - Update D12's "The blur" and `CONTEXT.md` section 6.
  - **[needs human]** check: T8's list on Windows 11, with no step in the
    blur; the same on Windows 10, where it still steps.

- [x] **R16. Scripting verbs: an out-of-process extension point (F-59).**
  Resolved by `ROADMAP.md` D15 and split into tasks `P8`-`P10` (`WM_COPYDATA`
  transport to `DeskSpaceController`, same-session trust check, shared `Verb`
  enum in `deskspace-app`, and in-process sandboxed Rhai extensions in
  `deskspace-ext` instead of native DLL plugins).
  - Extensions today mean editing the source. Design a small, stable verb set
    (`new-space`, `show`, `hide`, `settings`, `apply-theme <file>`, ...) that
    `deskspace.exe --verb ...` hands to the running instance, so scripts, the
    desktop menu (C9), and Explorer's "Send to" (F-56) share one path. Settle:
    the transport (today `--show` posts one `WM_APP` id; arguments need
    `WM_COPYDATA` or a named pipe), one `Verb` enum shared by `cli.rs` and the
    controller, mapping verbs onto existing app methods (never a second way to
    change the model), and the trust boundary (same user and session only;
    never accept a verb from an elevated or remote caller). C9's `--new-space`
    should be built as the first verb if this lands first. No in-process plugin
    loading: native plugins in a shell-integrated app cost more stability and
    security than they are worth now.

- [x] **T22. Smoke (core, platform).** **[needs human]**
  - Row: `effect(Effect::Smoke, "smoke", &[Detail])`, added to
    `EVERY_EFFECT` so Startup and Delete offer it; Fold keeps its list.
  - Frame math: `render/smoke.rs`, pure. `SmokeField::new(width, height,
    detail, seed)` lays 3 to 6 plumes along the lower edge from `seed`.
    A cell's value is low at the bottom plus noise, so the front eats
    upward along a ragged edge; the smoke's density comes from those
    seeded columns, leaning and spreading as they rise. `front(gone)`
    moves from below the field to above it, and `cover` fades the window
    across a soft edge. `Mask::Smoke { front, detail, seed }` carries it;
    the painter writes `cover` into a smooth cell bitmap.
  - Seed: `Play::seed` is set once when a play starts (`space_animation`
    mixes a counter with the clock), so each play and space differs while
    a reverse, which keeps the seed, retraces the same wisps.
  - Snapshot: `render/d2d/snapshot.rs` renders the `PreparedScene` through
    a WIC render target at the space's size once, at the start; the
    overlay samples it through the plume so the title, items, and tint
    are carried away. A failed snapshot is logged and the smoke falls back
    to the tint.
  - Overlay: `win32/effect_overlay.rs` gains `Art::Smoke`: `smoke::paint`
    shades the plume into the DIB at 3 to 8 px cells and upsamples it
    bilinearly, tinted by the space; it only draws where the front has
    reached, so the smoke starts low and climbs. Margins are 18 % of the
    width and 50 % of the height, clamped.
  - Tests: the field stays in range and is higher at the top than the
    bottom; the front covers everything at 0 and nothing at 1; sampling
    matches the cells; the same seed and progress draw the same smoke and
    a different seed draws different smoke; the snapshot's red reaches a
    blue tint's plume; pixels stay premultiplied; transition tests cover
    the new mask.
  - **[needs human]** check: Smoke on Startup (hide, show, a new space,
    startup, exit) and Delete, on Acrylic, Blur, Solid, and Clear at 100 %
    and 150 %: the space fades into tinted plumes that rise past its frame
    with no box or blur around them; its title, icons, and labels visibly
    drift into the smoke; Detail changes the number and scale of plumes;
    each play looks different, but reversing mid-way retraces the same
    smoke; a show condenses it back; no overlay is left after it ends,
    after a delete, or after quitting mid-way; a desktop-hosted space
    smokes without the overlay; CPU stays smooth with several spaces at
    once and on a full-screen space.

- [x] **T23. Burn's smoke (core, app, platform).** **[needs human]**
  - Option: `EffectOption::Smoke`, used by Burn alone; `AnimationSettings.smoke`
    (on by default, so an old file loads smoking), `Resolved.smoke`, and
    `Play.smoke`. Startup and Delete offer it; Fold does not.
  - Settings: a Smoke toggle on Startup and Delete, greyed unless Burn is
    chosen, and Off/On in the debug window's Animation settings box.
  - Painter: `render/burn_smoke.rs`, pure, shades a neutral warm gray plume
    rising from the burn front into the overlay; `flames::paint_over` draws
    the fire over it. With smoke on, Burn's overlay uses `smoke::margins`
    for both, so the flames stand in the larger window as before.
  - Tests: no smoke at either end of the burn; smoke is gray and
    premultiplied; an old config loads with smoke on; the page and the debug
    box edit it.
  - **[needs human]** check: Burn with Smoke on and off on Startup and
    Delete, on each backdrop at 100 % and 150 %: on, a gray plume rises from
    the glowing edge behind the flames, thickest at the edge and thinning
    upward, with no box or tint; off, Burn looks exactly as it did; the
    toggle persists and the debug box changes it.
