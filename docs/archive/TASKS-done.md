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

- [x] **K9. Split `render/d2d/mod.rs`.** 804 lines, at the guideline.
  Turn it into `render/d2d/mod.rs` plus:
  - `prepared.rs`: `PreparedScene`.
  - `drag_image.rs`: `render_items`.
  - `mod.rs` keeps `Painter`, `DrawContext`, `PaintFrame`,
    `BorderStroke`, and the device-loss helpers.

- [x] **U2. The undo stack (app, pure; no callers yet).** `app/undo.rs`:
  - `UndoStack { undo, redo }` of `UndoEntry { label, spaces, files }`.
    `spaces` holds each changed space's before and after
    (`Option<(usize, Space)>` each; `None` means absent), and `files` is a
    `Vec<FileChange>` in the order made: `Moved { from, to }`,
    `Renamed { from, to }`, `Created { path }`, and
    `Recycled { original, at }` (`at` is when it was recycled).
  - Recording: `begin(label)`, then `note_space(desktop, id)` captures a
    space's before state the first time an entry sees it, `note_file(change)`
    adds a file change, and `finish(desktop)` reads every after state and
    pushes the entry, unless it changed nothing. `finish` clears `redo` and
    keeps at most `UNDO_DEPTH` (50) entries.
  - Checks: `UndoEntry::can_undo(desktop, exists)` is true while every
    listed space equals its after state and every file change can be
    reversed (`to` exists and `from` is free, as `exists` reports);
    `can_redo` is the same against the before state. `exists` is a closure,
    so tests run without a disk.
  - Tests: an entry round trip, the depth cap, a new entry clears redo, a
    changed space or a moved file makes an entry stale, and an empty action
    records nothing.

- [x] **U3. Undo and redo layout changes (app).**
  - `App::dispatch` calls `note_space` for every space in `touched_by`
    while an entry is open. Wrap each user action in `begin`/`finish` at its
    entry point (menu choice, drag end, resize end, rename commit, new
    space, delete space): move, resize, reorder, sort, arrange, snap all,
    fit to contents, lock, fold, rename a space, create a space, and move
    items between spaces. A drag records once, on release, not per mouse
    move.
  - `App::undo` and `App::redo` (in `app/undo.rs`): check the top entry; if
    it is stale, drop it and tell the user with `ui.alert` ("DeskSpace can't
    undo “Move 3 items”, because the space changed since."); otherwise
    dispatch `RestoreSpaces`, move the entry across, refresh what it names,
    and save. File changes are reversed in U5-U7; until then an entry that
    holds any is never recorded.
  - Menus (the menu-entry recipe): Undo and Redo, named for the entry's
    label ("Undo Move 3 items"), at the top of the space menu and the tray,
    greyed when there is nothing to do.
  - Tests: through `fixtures.rs`, each listed action records one entry that
    undoes and redoes; a settings-window edit records none.

- [x] **U4. Ctrl+Z and Ctrl+Y (the key recipe).** Two `SelectionKey`
  variants under `WATCH_PASTE`, the flag Ctrl+V uses, so they act only
  while a space is current and pass through to Explorer otherwise. Move
  Ctrl+Z out of `other_keys_pass_through_to_type_to_select_or_the_app`.
  **[needs human]** check: Ctrl+Z after a space move puts it back, Ctrl+Y
  moves it again; with no space current, Ctrl+Z on the desktop still undoes
  an Explorer rename.

- [x] **U5. Undo file moves (app).** Record `Moved` where DeskSpace moves a
  file: `place_dropped` (into the store, including drops from the taken-over
  desktop), `release_item` (back to the desktop on a space delete), and a
  paste's external moves; record `Renamed` for an item rename
  (`rename.rs`). Reverse in reverse order with the same mechanism that made
  the change (`ItemStore` for store moves, `ui.transfer_files`, and
  `ui.rename_path`), after checking every change first, so a stale entry
  touches no file. Then a space delete undoes: its files go back into the
  store and the space comes back with its items. **[needs human]** check:
  drop a file into a space and undo (it is back where it was); delete a
  space with stored files, undo, then redo; rename an item and undo.

- [x] **U6. Restore from the Recycle Bin (platform).**
  `Ui::restore_recycled(original: &Path, since: SystemTime) -> Option<PathBuf>`:
  find the Recycle Bin item whose original location is `original` and that
  was deleted at or after `since` (the newest if several), restore it with
  the shell's "undelete" verb, and return the restored path. `None` when it
  is gone (emptied, or restored by hand). Cite the Microsoft Learn pages for
  the Recycle Bin folder and its properties beside the code. **[needs
  human]** check: recycle a file in Explorer, restore it through a test
  harness or `--verbose` log, and confirm it is back under its old name.

- [x] **U7. Undo deletes, copies, and new shortcuts (app).** Record
  `Recycled` (with the time taken just before the call) for a Recycle-Bin
  delete in `delete_selected_items` and `delete_space_item`; an item that
  was only unlisted is a model change and needs no file record. Record
  `Created` for copies (`copy_dropped`, a paste's copies), shortcuts
  (`add_shortcuts`), and "Add files…". Undoing `Recycled` restores with
  U6; undoing `Created` recycles the file and turns the record into
  `Recycled` so redo can restore it. Shift+Delete records nothing.
  **[needs human]** check: delete two items and undo (both return to the
  space); paste a copy, undo (it goes to the Recycle Bin), then redo; empty
  the Recycle Bin between a delete and its undo: the undo says it can't and
  changes nothing.
- [x] **W1. Keyed folder watches (platform, app).** The hardest task here;
  ask a maintainer to review it.
  - Why: today `win32/folder_watch.rs` runs one thread per watch, and
    `Ui::watch_folder` keeps a single watch, which the Desktop folder
    already uses. Lenses need many watches at once, and the app must know
    which folder changed.
  - `input.rs`: add `pub enum WatchKey { Desktop, Space(SpaceId) }`, and
    give `Event::FolderChanged` a field: `FolderChanged { key: WatchKey }`.
  - `folder_watch.rs`: replace the one-folder worker with one worker for
    all folders, built on an I/O completion port.
    - Adding a folder opens the directory exactly as `run` does today,
      registers it with the port (`CreateIoCompletionPort`, with the
      folder's slot number as the completion key), and starts its first
      `ReadDirectoryChangesW`.
    - Add, remove, and stop are sent with `PostQueuedCompletionStatus`,
      using reserved keys that no folder slot uses. Make the reserved keys
      named constants.
    - The worker loop waits on the port. A folder's completion marks that
      folder's `Coalescer` and starts its next read. A timeout posts
      `FOLDER_CHANGE` for every folder whose burst is due. The wait's
      timeout is the smallest `wait_ms` of all folders.
    - A message carries only two numbers, so pass the folder's slot in
      `wParam`. The runtime keeps the table from slot to `WatchKey`.
    - Removing a folder must cancel its pending read and wait for it to
      finish before closing the handle, as `run` does today. Otherwise
      Windows writes into freed memory.
  - `runtime`: the worker starts with the first watch and stops on
    teardown. `Ui::watch_folder(key, path)` adds the key's watch, replaces
    it when the path changed, and does nothing when the path is the same.
    `Ui::unwatch_folder(key)` removes it, and does nothing for a key it
    does not know.
  - App: `watch_desktop_folder` and `apply_takeover` pass
    `WatchKey::Desktop`. `Event::FolderChanged { key: WatchKey::Desktop }`
    calls `resync_desktop_layer`, as today.
  - Tests (pure parts): two folders debounce independently; the soonest
    timeout wins; removing an unknown key changes nothing.
  - Done when: the gates pass and the desktop layer behaves exactly as
    before. **[needs human]**: the I8 check in section V again.

- [x] **W2. Lenses follow their folders (platform, app).**
  - Why: after W1 the app hears "this folder changed". Now the worker also
    reads the folder, and the app shows what it read.
  - New file `win32/folder_list.rs` with
    `list_folder(path) -> Result<Vec<FolderEntry>, PlatformError>`.
    - Call `FindFirstFileExW` with `FindExInfoBasic` and
      `FIND_FIRST_EX_LARGE_FETCH` (the fast settings), then `FindNextFileW`
      until there are no more files, and always `FindClose`. Skip the `.`
      and `..` entries.
    - Put the conversion in a pure function, `entry_from(name,
      attributes, size, last_write)`, and test it. It sets `kind` with
      `ItemKind::classify`, and `hidden` when the file has
      `FILE_ATTRIBUTE_HIDDEN` or `FILE_ATTRIBUTE_SYSTEM`. `size` is `None`
      for folders. `modified` is in seconds since 1970; a `FILETIME`
      counts 100 ns steps since 1601. `label` stays empty, which shows the
      file name without its extension, as normal spaces do.
  - The worker calls `list_folder` once when a watch starts and after each
    debounced burst. The event becomes
    `Event::FolderChanged { key, listing: FolderListing }`, where
    `FolderListing` is `Listed(Vec<FolderEntry>)` or `Unavailable`. A list
    does not fit in a message's two numbers, so the worker leaves it in a
    shared slot (an `Arc<Mutex<..>>` per watch) and the UI thread takes it
    when the message arrives.
  - `Unavailable` covers a folder that does not exist, cannot be opened,
    or disappears while watched (its read then fails). The worker tries
    that folder again every `RETRY_MS` (5 s, a named constant). The app
    sets no timer for it.
  - The Desktop watch ignores its listing. The desktop layer also needs
    each icon's position, and only the shell knows those.
  - App: a new method, `App::sync_watches`. It builds the set of watches
    the model wants: the Desktop while `layer_active()`, and each lens as
    `(WatchKey::Space(id), its folder)`. It compares that with what is
    watched now and calls `watch_folder` or `unwatch_folder` for each
    difference. Call it at startup and in place of each direct
    `watch_desktop_folder` or `unwatch_folder` call, then delete those
    calls. W6 and W7 call it too. Test the set it builds, with a model
    built by `fixtures.rs`.
  - App, the event: `Listed(entries)` dispatches
    `Command::SyncLens { id, entries }` and refreshes that space.
    `Unavailable` dispatches `SyncLens` with no entries and adds the id to
    a new session field, `unavailable_lenses: HashSet<SpaceId>`, which W5
    draws. `Listed` removes the id again. A key whose space no longer
    exists is ignored.
  - Icons: the icon cache keeps an icon until it is told to drop it
    (`CONTEXT.md`, "The icon cache is invalidated only after..."). Before
    the sync, compare each entry's `modified` with its item's, and
    invalidate the icon of every one that changed. Then update that
    `CONTEXT.md` note.
  - Done when: the gates pass, and `CONTEXT.md` describes the watch and
    the listing. **[needs human]**: with the hand-made lens, create,
    rename, and delete files in its folder in Explorer; the lens follows
    within about 150 ms. Rename the folder away: the lens empties. Rename
    it back: it fills again within 5 s.
- [x] **W3a. A lens never points into storage (app).**
  - Why: every space shares one storage folder, and deleting a normal
    space moves its stored files out of it. A lens on that folder would
    show those files as its own, and a delete or a drop there could move
    files that other spaces list. So such a lens must never be watched.
    W3b, W6, and W7 call this check too.
  - `storage.rs`: the tests module has `fn is_inside(path, root)`. Move
    it, unchanged with its doc, out of `mod tests` to module level as
    `pub(crate) fn is_inside`. Its test and
    `user_data_is_outside_the_install_root` keep using it.
  - `storage.rs`: add `ItemStore::allows_lens(&self, folder: &Path) ->
    bool`. It is false when `folder` is the store's root or lies inside
    it, and true otherwise, including for a folder that contains the
    store. Windows paths can differ by case and by short names
    (`TIMOTH~1`), so first canonicalize both paths. Use the canonical
    forms only when both calls succeed, else the paths as given; this is
    the rule `same_path` follows, so read it first.
  - `app/lens.rs`: add `fn refused_lenses(&self) -> Vec<SpaceId>`, the
    lenses whose folder `self.storage.allows_lens` refuses. In
    `desired_watches`, skip those lenses. In `sync_watches`, add each
    refused id to `self.unavailable_lenses`. When `insert` returns true
    (it was not there yet), log one `tracing::warn!` naming the space and
    the folder. Then the warning appears once, not on every sync.
  - Tests, in `storage.rs`: `allows_lens` is false for the root, a child,
    a grandchild, and the root spelled in another case; it is true for a
    sibling whose name starts the same (`storage-old`), an unrelated
    folder, and the root's parent. In `app/lens.rs` (copy
    `every_lens_wants_its_folder_and_normal_spaces_want_none`): a lens on
    `app.storage.root()` is in `refused_lenses` and has no watch in
    `desired_watches`.
  - Docs: in the lens part of `CONTEXT.md` section 4, one sentence: a lens
    on the storage folder is kept but never watched.
  - Done when: the gates pass. No screen check is needed.
- [x] **W3b. Dropping onto a lens moves or copies into its folder
  (platform, app).**
  - Why: today a file dropped on a lens is first moved into storage by
    `place_dropped`, and then `AddItem` is rejected, so the file is stuck
    in storage where the user never sees it. D3 says a drop on a lens
    moves the file into the lens's folder instead, with Explorer's own
    dialogs.
  - Platform, `win32/file_op.rs`. Read `transfer` first.
    1. Add `pub enum OnCollision { Rename, Ask }` with a `///` doc on
       each variant. `Rename` gives a clashing name " (2)"; storage uses
       it because every space shares the folder. `Ask` shows Explorer's
       "Replace or Skip" dialog; a user's real folder wants that.
    2. Add a pure `fn operation_flags(on_collision: OnCollision)` that
       returns the flags `transfer` passes to `SetOperationFlags` today
       for `Rename`, and the same flags without `FOF_RENAMEONCOLLISION`
       for `Ask`. Look up the type `SetOperationFlags` takes in the
       `windows` crate docs. Both keep `FOF_ALLOWUNDO`, so Ctrl+Z in
       Explorer undoes the move.
    3. `transfer` takes an `on_collision: OnCollision` argument and uses
       `operation_flags`. Update the module's `//!` doc, which says name
       clashes are always renamed.
    4. Export `OnCollision` from the crate root (`lib.rs`, beside
       `DropAction`). `Ui::transfer_files` (`win32/runtime/ui_shell.rs`)
       takes it and passes it on.
    5. Test `operation_flags`: `Rename` has `FOF_RENAMEONCOLLISION` and
       `Ask` does not; both have `FOF_ALLOWUNDO`.
  - App, the existing callers: `copy_dropped` (`app/files.rs`) and
    `paste_clipboard` (`app/clipboard.rs`, two calls) pass
    `OnCollision::Rename`. They put files in storage, so nothing changes
    for them.
  - App, `app/files.rs`, a pure function `fn paths_to_transfer(paths:
    &[PathBuf], folder: &Path) -> Vec<PathBuf>`. It keeps the paths in
    order and leaves out three kinds:
    - a path already directly in `folder` (its parent is `folder`, by
      `storage::same_path`), because moving it there changes nothing;
    - `folder` itself;
    - any folder that contains `folder` (`storage::is_inside(folder,
      path)`), because a folder cannot move into itself.

    Test each rule, then a mixed list, which keeps only the valid paths
    in their order.
  - App, `app/files.rs`, a new method `fn drop_into_lens(&mut self, ui:
    &mut Ui, folder: PathBuf, paths: Vec<PathBuf>, action: DropAction)`:
    1. If `!folder.is_dir()`, log a warning and return. W5 shows the
       user why.
    2. If `!self.storage.allows_lens(&folder)` (W3a), return.
    3. Before moving anything, note which normal spaces list the dropped
       files: for each path, `find_item_by_file`, then
       `space_containing`. Skip lens spaces; their own watch follows the
       move.
    4. `let paths = paths_to_transfer(&paths, &folder);` and return if it
       is empty.
    5. `match action`: `Move` calls `ui.transfer_files(&paths, &folder,
       true, OnCollision::Ask)`, and `Copy` the same with `false`.
       `KeepInPlace` cannot apply to a folder: log at `debug` and return.
       After W3c the platform never sends it to a lens; keep the arm and
       say so in a comment.
    6. Call `self.prune_missing(ui, id)` for each space noted in step 3.
       It drops the items whose files just moved away.
  - App, `add_dropped`: as its first statement, if the target space is a
    lens (`space.folder_view()`), clone the folder path, call
    `drop_into_lens`, and return. Nothing after that line runs for a
    lens: no `adopt`, `AddItem`, or `MoveItem`. The lens's watch shows the
    new files a moment later. "Add files…" (`menus.rs` `add_files`)
    already goes through `add_dropped`, so it is covered.
  - Re-entrancy: the shell's dialog runs its own message loop. The app
    holds no runtime borrow, and `Ui::transfer_files` takes its own only
    for a moment, so nothing more is needed. Do not add a `with_runtime`
    call around it (`ENGINEERING.md` section 6).
  - Done when: `add_dropped` never reaches `adopt`, `AddItem`, or
    `MoveItem` for a lens, and the gates pass. **[needs human]**: drop
    files from Explorer and from a normal space onto a lens; they land
    in its folder, and the normal space no longer lists its file. Drop a
    file whose name is already there: Explorer's Replace or Skip dialog
    appears. Ctrl+Z in Explorer undoes the move. A right drop, "Copy
    here": the original stays. "Add files…" on a lens moves the files
    into its folder. Regression: a clashing drop on a normal space still
    gets " (2)" and no dialog.
- [x] **W3c. Shift and the right-drop menu on a lens (platform, app).**
  - Why: on a normal space, Shift means "show here without moving". A
    lens shows only its folder, so it cannot show a file kept elsewhere.
    The drop must be refused, with the "no" cursor, before the user lets
    go. Also, a source that offers only `COPY` should copy into a lens;
    today `drop_plan` turns `COPY` into keep-in-place.
  - The drop target lives in the platform and does not know which spaces
    are lenses. The app tells it through the scene, as it tells it
    everything else (`ENGINEERING.md` section 3, "Rendering and UI are
    data-driven"). Follow "Recipe: a new visual in a space", steps 1-2:
    1. `render/scene.rs`: `pub enum DropInto { Space, Folder }`, deriving
       `Default` with `Space` as the default, and a `///` doc for each.
       Add a `SpaceScene::drop_into: DropInto` field and export the enum
       beside `SpaceScene`.
    2. `deskspace-app/src/scene.rs`, `space_scene`: `DropInto::Folder`
       when the space is a lens. Test it: a lens's scene says `Folder`, a
       normal space and the desktop layer say `Space`.
  - Platform, `win32/space_window.rs`: `pub(crate) fn drop_into(window:
    HWND) -> DropInto`, which reads the stored scene. Copy
    `space_animation::is_leaving`, which reads window state the same way.
    `PreparedScene` keeps its `SpaceScene` in a private field; add a
    `pub(crate) fn scene(&self) -> &SpaceScene` accessor if none exists.
    A window with no scene yet answers `DropInto::Space`.
  - Platform, `win32/drop_target.rs`. All of these are pure, with tests
    at the bottom of the file:
    - `drop_plan(kind, keys, allowed, into: DropInto)`. For `Folder`
      and `DragKind::Files`: with Shift held, `(DropPlan::None,
      DROPEFFECT_NONE)`; otherwise `MOVE` when allowed, else `COPY`
      gives `DropPlan::Files(DropAction::Copy)`, not `KeepInPlace`. For
      `Space`, nothing changes. Shell items and Alt-links are the same
      for both (W3f handles them).
    - `drag_effect` takes `into` and passes it on.
    - `right_drop_entries(allowed, into)`: "Show here without moving" is
      greyed for `Folder`, as "Create shortcuts here" is greyed by
      `allowed`.
    - The `IDropTarget` methods read `drop_into(self.window)` and pass it
      on. Read it once per call, not in a loop.
    - Tests: copy the existing `drop_plan` and `drag_effect` tests and
      add the `Folder` cases (Shift refuses; `COPY` only gives `Copy`;
      a plain drop moves). Add a `right_drop_entries` case: for `Folder`,
      `DROP_KEEP` is disabled. Every existing test passes `DropInto::Space`
      and keeps its expectation.
  - Done when: the gates pass. **[needs human]**: Shift-drag a file from
    Explorer onto a lens: the cursor shows "no" and nothing moves. Right
    drag onto a lens: "Show here without moving" is grey. On a normal
    space, Shift and the right-drop menu behave as before.
- [x] **W3d. A lens item dropped on a normal space (app).**
  - Why: dragging an item from a lens onto a normal space does nothing
    today. `add_dropped` finds the item by its file and sends `MoveItem`,
    which core rejects for a lens item. Decided (2026-10-21): to a
    normal space, a lens's file is an ordinary file, so it moves into
    storage like a file dropped from Explorer.
  - `app/hit.rs`, `find_item_by_file`: skip items that belong to a lens.
    The method answers "which item that DeskSpace owns uses this file";
    say that in its `///` doc. A lens's items belong to its folder.
    Callers then take their "not found" path. `add_dropped` adopts the
    file into storage and adds a new item, and a paste's `plan_paste`
    treats it as an outside move. The lens's watch drops the old item at
    its next sync.
  - Test, in `app/hit.rs` (the module has `App::new` tests to copy): put
    an item for `C:/x/a.txt` in a normal space, and the same path in a
    lens through `fixtures::apply` with `Command::SyncLens`.
    `find_item_by_file` returns the normal space's item. With only the
    lens listing it, it returns `None`.
  - Done when: the gates pass. **[needs human]**: drag an item from a
    lens onto a normal space: the file leaves the lens's folder, the
    space lists it, and the lens no longer shows it. Delete that space:
    the file goes to the desktop, not back to the folder (D3).
- [x] **W3e. Pasting into a lens (app).**
  - Why: `paste_clipboard` (`app/clipboard.rs`) moves or copies the
    clipboard's files into storage and then sends `AddItem`, which a lens
    rejects. The files are stuck in storage, as with drops before W3b.
  - At the top of `paste_clipboard`, once `clipboard` is read and not
    empty: if the target is a lens, call `self.drop_into_lens(ui, folder,
    clipboard.paths, action)` (W3b), where `action` is `DropAction::Move`
    for a cut and `DropAction::Copy` otherwise. After a cut, call
    `ui.clear_clipboard()`, as the end of the method already does. Then
    return. Copy `add_dropped`'s lens check from W3b so both read the
    same way.
  - Nothing new is pure here; W3b already tests `paths_to_transfer`.
  - Done when: `paste_clipboard` never reaches `transfer_files` into
    storage or `AddItem` for a lens, and the gates pass. **[needs
    human]**: in Explorer, copy a file and paste it on a lens (click the
    lens, then Ctrl+V): a copy lands in the folder. Cut and paste: it
    moves. Cut an item from a normal space and paste it on a lens: it
    moves into the folder and leaves the space. A paste on a normal space
    works as before.
- [x] **W3f. Shortcuts on a lens (app).**
  - Why: `add_shortcuts` (`app/files.rs`) writes every new `.lnk` or
    `.url` into storage, then sends `AddItem`, which a lens rejects. A
    Start-menu app dropped on a lens, "Paste shortcut", the custom
    shortcut dialog, and the installed-apps picker all end there.
    Explorer writes such a shortcut into the folder it was dropped on.
  - `storage.rs`: make `unique_destination` `pub(crate)`.
    `ItemStore::unique_path` keeps calling it.
  - `app/files.rs`, `write_shortcut`: take the destination folder as an
    argument and use `storage::unique_destination(folder, name)` in
    place of `self.storage.unique_path(name)` (three places).
  - `add_shortcuts`: work out the destination first. For a lens it is
    its folder, after the W3a `allows_lens` check and an `is_dir` check
    (log and return when either fails). Otherwise it is
    `self.storage.root()`, created as today. Pass it to `write_shortcut`.
    Send `AddItem` only for a normal space; a lens's watch lists the new
    file.
  - Test, in `app/files.rs` (no COM needed): `write_shortcut` with a
    `ShortcutSource::Shell` whose `path` is a `.lnk` file in a temp
    folder copies it into a second temp folder. A second call names the
    copy " (2)". Copy the temp-folder setup from
    `stored_paths_lists_stored_files_and_skips_items_in_place`.
  - Done when: `add_shortcuts` never sends `AddItem` for a lens, and the
    gates pass. **[needs human]**: drop a Start-menu app on a lens: a
    working `.lnk` is in the folder and the lens shows it. "New shortcut
    ▸ Paste shortcut" and the custom shortcut dialog on a lens do the
    same. On a normal space, shortcuts work as before.
- [x] **W3g. Deleting an item from a lens (app).**
  - Why: D3 says removing a lens item recycles its file. The Delete key
    already does that for any item (`delete_selected_items` recycles
    every selected file). The X on the settings window's Shortcuts page
    (`delete_space_item`) recycles only stored files and merely unlists
    the rest. On a lens the next sync would bring the item back, so the
    X seems to do nothing.
  - `app/files.rs`: a pure `fn recycles_on_delete(space: &Space, store:
    &ItemStore, path: &Path) -> bool`. It is true for a lens item and for
    a stored file, and false for an item listed in place.
  - `delete_space_item`: use `recycles_on_delete` where it uses
    `stored` today (the question and the recycle step). Rename
    `delete_item_question`'s `stored` parameter to `recycles`; its text
    already says "moved to the Recycle Bin".
  - Tests: `recycles_on_delete` for each case (build the spaces with
    `fixtures`; a lens item through `Command::SyncLens`). The existing
    `deleting_an_item_says_where_its_file_goes` keeps passing.
  - Done when: the gates pass. **[needs human]**: select a lens item and
    press Delete: the file is in the Recycle Bin and the lens drops it.
    On the settings window's Shortcuts page, the X on a lens item says
    the file goes to the Recycle Bin, and it does.
- [x] **W4. Deleting a lens never touches its folder (app).**
  - Why: deleting a normal space moves its stored files back to the
    desktop. For a lens, D3 says the folder and its files stay where they
    are. After W3a no lens points into storage, so the code already
    leaves a lens's files alone by accident. This task makes it so on
    purpose, and fixes the question the user is asked.
  - `app/files.rs`, the two places that touch a deleted space's files:
    - `stored_paths`: return an empty list for a lens. The debug
      window's "Delete all spaces" recycles what this returns.
    - `release_item`: never move a lens item's file. Only unlist it.
  - `delete_space_now`: after `DeleteSpace` is applied, also remove the
    id from `self.unavailable_lenses` and call `self.sync_watches(ui)`,
    so a deleted lens's folder stops being watched. Do it for every
    deleted space, with no `if`; for a normal space it changes nothing.
  - `app.rs`, the question before deleting:
    - `delete_needs_confirmation(space)`: a lens always asks, even when
      it shows no items. The space menu's "Delete space…" ellipsis
      follows it with no further change (`menus.rs`).
    - `delete_consequence`: take `space: &Space` and `stored: usize` in
      place of `count`. For a lens, say `It shows the folder
      “{path}”. The folder and its files stay where they are.` Normal
      spaces keep today's text. Update the caller in `delete_space`.
  - Tests: `stored_paths` of a lens whose folder is the store's root
    returns nothing, even with files there (copy
    `stored_paths_lists_stored_files_and_skips_items_in_place`). An
    empty lens needs confirmation. A lens's consequence names the folder
    and says the files stay. Today's `delete_consequence` tests still
    pass with a normal space.
  - Done when: the gates pass. No screen check is needed.
- [x] **W5. "Folder not available" (app, platform).**
  - Why: after W2 a lens whose folder is missing is simply empty, which
    looks like a bug.
  - Follow "Recipe: a new visual in a space". Add
    `SpaceScene::placeholder: Option<String>`, `None` by default. In
    `space_scene`, a lens in `unavailable_lenses` gets
    `"Folder not available\n<path>"`. Draw it centered in the item area,
    in the label color at about 60 % opacity. Build the text layout in
    `PreparedScene`, not while painting, so a paint allocates nothing.
  - Test: an unavailable lens's scene carries the text; a normal space
    and an available lens carry `None`.
  - **[needs human]**: rename a lens's folder away; the text shows.
    Rename it back; the items return within 5 s.
- [x] **W6. The lens menu (platform, app).**
  - Platform: `Ui::pick_folder(title) -> Option<PathBuf>`, a "Select
    folder" dialog. Copy `pick_files` (`win32/dialog.rs`) and set
    `FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM` in place of its options.
  - App: follow "Recipe: a new menu entry". Right-clicking a lens shows
    these beside the usual space entries:
    - Open folder: `ui.open_path` on the folder.
    - Change folder...: `pick_folder`, the W3a guard
      (`ItemStore::allows_lens`; when it fails, say why with `ui.alert`
      and stop), `SetLensFolder`, then `sync_watches`.
    - Sort by: the same keys as a normal space's sort menu (`SORT_PRESETS`
      in `app.rs`), the current one checked. A choice dispatches
      `SetLensView`. On a lens the order is kept as files come and go,
      while a normal space sorts once. Do not change normal spaces here;
      that is F-31, in phase 4. Both menus use the one table.
    - Folders first, and Show hidden files: checkable entries that
      dispatch `SetLensView`.
  - Tests: a lens's menu has these entries with the right checks and
    commands; a normal space's menu is unchanged.
  - **[needs human]**: each entry on a hand-made lens; after a restart the
    sort, folders-first, and hidden-files settings are still set.
- [x] **W7. New lens... (app).**
  - "New lens..." right under "New space" in the space menu, the tray
    menu, and the desktop layer's menu. It runs `pick_folder`, then the
    W3a guard, `ItemStore::allows_lens` (when it fails, say why with `ui.alert` and stop), then
    `CreateLens` sized and placed as `create_space` places a new space and
    titled with the folder's name, then `sync_watches`.
  - Update the ROADMAP: F-07's row in the feature index, and M4 phases 2
    and 3 marked done. Add the new entries to the menu lists in
    `CONTEXT.md` section 8 and in `README.md`.
  - **[needs human]**: create a lens from each of the three menus.
- [x] **W8. A glyph for each kind in the scopes pane (app, platform).**
  **[needs human]**
  - `ui.rs` `SettingsScope`: add `glyph: Option<char>` with a `///` doc
    (a code point drawn before the label; `None` draws nothing and keeps
    the label where it is).
  - `settings/mod.rs`: `enum ScopeKind { Global, Space, Lens }`, with
    `ScopeKind::of(Option<&Space>)` and `ScopeKind::glyph()`. `build_form`
    fills each scope's glyph from it. W10 reuses `ScopeKind` for the page
    list, so keep it beside `listed_pages`.
  - Glyphs: Global `U+E713` (Setting), a lens `U+E8B7` (Folder), and a
    space a grid glyph that both fonts have (choose one; the links are
    above).
  - `win32/settings_window/paint.rs` `on_draw_scope`: draw the glyph in a
    square as tall as the label, before it, in the row's text color, so
    a dimmed row dims its glyph too. `CreateFontW` quietly substitutes a
    missing face, so test for Segoe Fluent Icons with
    `EnumFontFamiliesExW` and fall back to Segoe MDL2 Assets. Create the
    font once with the window's other fonts in `build.rs`, scaled for
    DPI, and free it with them.
  - Tests: `ScopeKind::of` for each kind; extend
    `global_lists_every_space_dimmed_until_customized` with the glyphs.
  - **[needs human]** check: in light and dark mode, at 100 % and 150 %,
    on Windows 11 and 10: every row shows its kind's glyph, the labels
    line up, a dimmed row's glyph is dimmed, and the selection covers the
    glyph.
- [x] **W10. The Lens page (app).**
  - `listed_pages` takes a `ScopeKind` (W8) instead of an
    `Option<SpaceId>`. A lens lists a space's pages, then a new
    `page::LENS`. Read how `page::SHORTCUTS` was added first. If a new
    page cannot go there without renumbering the animation pages, stop
    and ask.
  - `pages.rs` `lens_page`: a "Folder" heading; a note with the full
    path (or "Folder not available" and the path); the buttons "Open
    folder" and "Change folder…"; a "Contents" heading; and a "Show
    hidden files" toggle. Each button is a `ButtonAction` variant whose
    arm in `App::on_settings_button` calls the method the lens menu
    already calls. The toggle becomes `SetLensView`, the way a look edit
    becomes `SetSpaceLook`. The page is built for every scope, so the
    form keeps its shape (`the_form_shape_is_stable_across_scopes`); a
    normal space just never lists it.
  - Cancel: `SpaceSnapshot` gains the lens's `FolderView`, and
    `App::cancel_settings` restores it. Change folder… commits at once, so
    it writes the new path into the snapshot too, as a menu edit made
    while the window is open does (`CONTEXT.md` section 6, Native UI).
  - Tests: a lens lists the Lens page and a normal space does not;
    the toggle becomes `SetLensView`; Cancel restores `show_hidden` and
    keeps a changed folder; `every_button_has_an_action` covers the two
    buttons.
  - Done when: also `CONTEXT.md` section 6 (Native UI, "The app's form")
    names the page.
- [x] **W11. A lens's title follows its folder (core, platform, app).**
  - `source.rs`: `enum LensTitle { FolderName, ProfilePath, FullPath,
    Custom }` (`snake_case`), and `FolderView.title: LensTitle`. The
    serde default is `Custom`, so a lens saved before this task keeps
    its title (D15). New lenses start at `FolderName`.
  - A pure `lens_title(view: &FolderView, current: &str, profile: &Path)
    -> String` in `source.rs`:
    - `FolderName`: the last part of the path, or the whole path for a
      drive root (`D:\`) or a share root (`\\server\share`);
    - `ProfilePath`: the path below `profile`, or the profile folder's
      own name for the profile itself, or the full path when the folder
      is outside it. Compare without case, and treat `/` and `\` alike
      (commit 60c0528 fixed the same trap for icons);
    - `FullPath`: the path with `\` separators;
    - `Custom`: `current`, unchanged.

    It replaces `lens_title` in `app/menus.rs`; delete that one.
  - `Space.title` stays the text every reader shows (the title bar, the
    scopes pane, the delete question), so no reader changes. The app
    computes the text and the commands carry it:
    - `SetLensFolder` gains `title: String`;
    - a new `SetLensTitle { id, title: LensTitle, text: String }`
      (the Command recipe);
    - `RenameSpace` on a lens sets its `title` to `Custom`, in core.
  - Platform: `profile_directory()` beside `desktop_directory`
    (`SHGetKnownFolderPath(FOLDERID_Profile)`). The app reads it once at
    startup.
  - Lens page (W10): a "Title" heading, a "Show as" choice (Folder name,
    Path from your user folder, Full path, Custom), and a "Custom title"
    text field, enabled only for Custom. Copy how the New spaces page's
    default title field edits.
  - Tests in core: each mode; a drive root; a share root; a profile
    prefix in another case and with forward slashes; a folder outside the
    profile; the profile itself; `RenameSpace` makes a lens custom; a
    lens with no `title` key loads as `Custom`.
  - **[needs human]** check: on a lens in the profile, each "Show as"
    entry retitles it at once, and Cancel puts it back. Change folder…
    retitles it under Folder name and keeps a custom title. F2 on the
    title, then the Lens page shows Custom.
- [x] **W12. Views, part 1: the icon sizes (core, app).** **[needs
  human]**
  - Core: `SpaceView { mode: ViewMode }` (`#[serde(default)]`, in a new
    `view.rs`) and `Space.view` (`#[serde(default)]`). `ViewMode` starts
    with `Icons` (the default: today's look-driven size), `ExtraLargeIcons`,
    `LargeIcons`, and `MediumIcons`. `ViewMode::icon_px()` returns the
    fixed size (256, 96, 48 px at 96 DPI), or `None` for `Icons`. Small
    icons put the label beside the icon, so they come with List's cell in
    W16.
  - `Command::SetSpaceView { id, view }` (the Command recipe). It works
    on both kinds; there is no lens branch.
  - App: where a space's icon size and grid cell are worked out (from
    `Theme.icon_size` today, in `app/view.rs`), use `icon_px()` when it is
    `Some`. `app/zoom.rs` leaves a space with a fixed size alone, as D9
    leaves a customized Custom look alone.
  - Tables: `VIEW_MODES` in `settings/tables.rs` (mode, label, digit;
    `Icons` is "Settings icon size" and has no digit), and
    `VIEW_PRESETS` in `app.rs`, pairing each mode with its `MenuId` as
    `SORT_PRESETS` does. A guard test checks that every row of one has
    exactly one row in the other.
  - Menu (the menu-entry recipe): View ▸ above Sort by in the space
    menu of both kinds, with one radio entry per row, checking the
    space's mode.
  - Tests: serde default; `SetSpaceView` outcomes; a space in
    `LargeIcons` gets a 96 px cell at 100 %; zoom leaves it alone; the
    guard test.
  - **[needs human]** check: each View ▸ entry resizes only that space,
    on a normal space and a lens. "Settings icon size" gives back the
    look's size. Ctrl+wheel does not move a space with a fixed size. The
    view survives a restart.

- [x] **B35. Save the config durably (config).**
  - Why: `ConfigStore::save` writes `deskspace.toml.tmp` and renames it
    over the config, but never flushes it. NTFS can make the rename
    durable before the data, so a power cut right after a save can leave
    an empty or partial config, which then loads as defaults and loses
    the user's layout (`ENGINEERING.md` rule 3).
  - `store.rs` `save`: write the temp file through a `File`, call
    `File::sync_all` before closing it, then rename as today. This is
    plain `std`, so the crate stays `forbid(unsafe_code)`.
  - Test: `save_then_load_round_trips` still passes; add one that a
    failed rename leaves the old config in place and the temp file
    behind (rename onto a path whose parent is a file).
  - Done when: the gates pass.
- [x] **B32. Index paths instead of comparing them pairwise (app).**
  - Why: `storage::same_path` canonicalizes both paths, two filesystem
    calls, whenever the strings differ. `app/view.rs`
    `desktop_layer_items` calls it for every desktop entry against every
    space item, and `app/hit.rs` `find_item_by_file` runs once per dropped
    or pasted path. One layer sync is about 2 × entries × items calls on
    the UI thread: about 120,000 for 200 entries and 300 items (review,
    2026-10-21).
  - `storage.rs`: a pure-ish `pub(crate) fn path_key(path: &Path) ->
    PathBuf`: the canonical path when `canonicalize` succeeds, else the
    path with `/` turned into `\` and case-folded. Two paths name the
    same file when their keys are equal. `same_path` becomes
    `path_key(a) == path_key(b)` after its fast `a == b` check.
  - `app/hit.rs`: `fn item_index(&self) -> HashMap<PathBuf, ItemId>`
    over every non-lens space's items, keyed by `path_key`.
    `find_item_by_file` keeps its signature for single lookups. Callers
    that look up many paths (`add_dropped`, `plan_paste`,
    `drop_into_lens`) build the index once and look each path up.
  - `desktop_layer_items`: build a `HashSet` of the space items' keys
    once, then test each entry's key against it.
  - Tests: `path_key` folds case and slashes for a missing file, and two
    spellings of an existing temp file share a key; `item_index` skips
    lens items (copy `a_lens_item_is_not_owned_by_deskspace`).
  - Done when: no loop calls `same_path` against every item, and the
    gates pass. W25 later moves the remaining canonicalize calls for
    lens folders off the UI thread; this task does not wait for it.
- [x] **B36. Compare file names the way Windows does (platform, app).**
  - Why: `storage::same_path` falls back to `eq_ignore_ascii_case`, and
    `storage::is_inside` compares components the same way, so `É.txt`
    and `é.txt` never match, although Windows treats them as one name.
  - Platform: `pub fn same_name_ignoring_case(a: &OsStr, b: &OsStr) ->
    bool` over `CompareStringOrdinal` with `bIgnoreCase` set, the
    function Microsoft documents for file names (link it beside the
    call); export it from the crate root.
  - App: `same_path`'s fallback and `is_inside`'s component compare call
    it. If B32 has landed, `path_key`'s case fold uses the same rule
    (`LCMapStringEx` with `LCMAP_UPPERCASE` on the invariant locale), so
    keys and comparisons agree.
  - Tests: `É` and `é` match; `ß` and `SS` do not (ordinal compares one
    character at a time); the existing `is_inside_compares_whole_components`
    passes.
  - Done when: no `eq_ignore_ascii_case` is left on a path in `app`, and
    the gates pass.
- [x] **B34. An O(1) least-recently-used icon cache (platform, pure).**
  - Why: once `IconCache` is full (1,024 icons), `IconCache::insert`
    scans every entry with `min_by_key` to find the oldest, on every new
    icon. Scrolling a large lens pays that for each icon it loads.
  - `win32/icons.rs`: keep a `BTreeMap<u64, Key>` from `last_used` to
    key beside `entries`. A hit moves its key to the new clock value; an
    insert into a full cache pops the first (oldest) entry. That is
    O(log n) with no new dependency. `invalidate` removes every variant
    of a path from both maps.
  - Tests: the existing `evicts_the_least_recently_used_icon` and
    `invalidate_forgets_every_variant_of_a_path` pass unchanged; add one
    that a hit protects an icon from the next eviction.
  - Done when: no method scans `entries`, and the gates pass.
- [x] **W13. Ctrl+Shift+1..8 (the key recipe).** **[needs human]**
  - Explorer's view keys: 1 Extra large icons, 2 Large icons, 3 Medium
    icons, 4 Small icons, 5 List, 6 Details, 7 Tiles, 8 Content. Watch all
    eight under `WATCH_PASTE` now, so later views need no keyboard
    change.
  - `win32/keyboard.rs`: eight `SEL_VIEW_*` codes decode to one
    `SelectionKey::View(u8)`, carrying the digit. The recipe gives a key
    with data its own `Event`, because most data does not fit in the
    code; a digit does, so one variant is enough (decided for D15).
  - `App::on_selection_key`: find the `VIEW_MODES` row for the digit and
    dispatch `SetSpaceView` on the current space. A digit with no row yet
    does nothing.
  - Docs: the `ENGINEERING.md` Keys table, `CONTEXT.md` section 8, and
    `README.md` Controls.
  - Tests: a `keys_for_*` row and the `selection_key` mapping; the app
    maps 2 to `LargeIcons`.
  - **[needs human]** check: click a space, then Ctrl+Shift+1, 2, and 3
    change it. With Explorer in front, Explorer's own Ctrl+Shift+1 still
    works.
- [x] **W14. One sort rule, with Explorer's Sort by (core, app).**
  - Core: `SortRule { key: SortKey, descending, folders_first }` beside
    `SortKey`, defaulting to name ascending, and `SpaceView.sort`.
    `FolderView` loses `sort`, `descending`, and `folders_first` (lenses
    are unreleased, D15), and `SyncLens` sorts by `view.sort`. The normal
    space's one-off sort (the command `App::sort_space` dispatches) takes
    a `SortRule` and stores it in `view.sort`. If there are two sort
    comparators today, merge them into `SortRule::compare`, keeping the
    natural name order for ties.
  - Menu: Sort by ▸ becomes Explorer's folder menu. First the keys from
    `SORT_KEYS` as radio items (Name, Date modified, Type, Size), then a
    separator, then Ascending and Descending, then a separator, then
    Folders first. A key keeps the direction, and a direction keeps the
    key. On a normal space each choice sorts once with the new rule, as
    today; a lens keeps it. Retire the seven `SORT_PRESETS` menu ids and
    note them as never reused in `MenuId`'s doc, as `ids.rs` does for
    fields 25 and 26.
  - Tests: `SortRule::compare` for each key, both ways, and with folders
    first; the one-off sort stores its rule; a lens re-sorts by it; the
    menu checks the rule's key and direction.
  - Done when: also the W6 row in section V says "Sort by" in the new
    shape.

- [x] **W15. Keep sorted (core, app).**
  - Core: `SpaceView.keep_sorted: bool`, off by default.
    `Space::keeps_sorted()` is true for a lens or when it is set; it is the
    one place that knows a lens is always sorted.
  - A space that `keeps_sorted()` re-sorts after every change to its items:
    `AddItem`, `MoveItem` into it, and an item rename. Do it in core,
    through one helper each of those `apply_` methods calls, not in the
    app.
  - `ReorderItem` and `ReorderItems` on such a space are `Rejected` (D15).
    `app/drag.rs` shows no rearranging insertion there; a drag out still
    works.
  - Menu: Keep sorted, last in Sort by ▸, checked and grey on a lens.
  - Tests: each re-sort trigger; reordering is rejected; a lens
    `keeps_sorted()`; serde default.
  - **[needs human]** check: with Keep sorted on, a dropped file lands in
    order, a renamed item moves to its place, and a drag inside does not
    rearrange (a drag out still moves the file). Turned off, the space
    rearranges freely again and keeps the order it had.

- [x] **W16. Views, part 2: the view layout and Small icons (core, app,
  platform).** **[needs human]** The largest task in the phase; pair with
  a reviewer before starting.
  - Today every item sits on the icon grid (`GridSpec`, `arrange_space`,
    `Item.position`, `Space::item_at`). The views from here on place
    items by their order instead.
  - Core, a new `view_layout.rs`: a pure layout that takes the mode, the
    content size, the item count, and the cell metrics. It returns each
    item's rect in content coordinates, the content extent, and the axis
    that scrolls. The icon views hand off to the grid as today. Small
    icons uses an *inline cell* (the icon left and one label line beside
    it, ellipsized), placed in rows that wrap at the width, scrolling
    down.
  - Every reader of item geometry moves onto it: the item rects in
    `scene.rs`, `Space::item_at`, the marquee, `next_in_direction` (give
    it the rects), the drag-to-rearrange drop index,
    `content_extent`/`max_scroll`, and the scroll indicator. These views
    never write `Item.position` (D15). If a reader cannot use the layout
    without a lens-or-space branch, stop and ask.
  - Paint: the inline cell at 16 px, as Explorer's Small icons; the
    selection fills the cell, and the check box sits left of the icon.
  - Add `ViewMode::SmallIcons` (digit 4) and its `VIEW_MODES` row.
  - Tests: the rects tile in order with no overlap; rows wrap at the
    width; the extent; the center of each rect hits its item; arrow keys
    move by row and column.
  - **[needs human]** check: Small icons on a space and on a lens, at
    100 % and 150 %: click, Ctrl and Shift click, the marquee, the arrow
    keys, type-to-select, F2 (the box sits over the label), drag out, and
    a drag to rearrange (normal space, not kept sorted). Back in Medium
    icons, the hand arrangement is as it was.

- [x] **W17. List (core, app, platform).** **[needs human]**
  - `ViewMode::List` (digit 5): Explorer's List. Inline cells (W16) fill
    each column top to bottom, the columns run left to right, and the
    space scrolls sideways; the wheel scrolls sideways too, as in
    Explorer. A column is `LIST_COLUMN_WIDTH` wide (measure Explorer's),
    and longer labels are ellipsized. Measuring each label is for later
    (add a **Found while working** line).
  - The layout reports the horizontal axis. Scroll state, the wheel,
    Home, End, PageUp, PageDown, and the scroll indicator follow the axis
    it reports, never the mode.
  - Tests: column breaks; the horizontal extent; the arrow keys (Down
    within a column, Right to the next column).
  - **[needs human]** check: the W16 list again in List view, plus the
    wheel and the keys scrolling sideways, and the indicator along the
    bottom.

- [x] **W18. Details (core, app, platform).** **[needs human]**
  - `ViewMode::Details` (digit 6): one row per item at full width, under
    a header row that stays put while the rows scroll.
  - `DETAILS_COLUMNS` in `settings/tables.rs`: Name (it takes the width
    left over, down to a minimum), Date modified, Type, and Size
    (right-aligned), each with its `SortKey` and an Explorer-measured
    default width.
  - A click on a header sorts by its column, and a second click on the
    same one reverses, through the W14 path. The sorted column shows
    Explorer's arrow.
  - Text, made when the scene is built, never while painting:
    - Platform helpers beside `item_facts`: `type_name(path, is_folder)`
      (`SHGetFileInfoW` with `SHGFI_TYPENAME | SHGFI_USEFILEATTRIBUTES`,
      cached by extension, with no disk access), `format_date(secs)` (the
      user's short date and time, `GetDateFormatEx` and
      `GetTimeFormatEx`, as Explorer shows them), and `format_kb(bytes)`
      (`StrFormatKBSize`, "1,234 KB"). A folder's size is blank.
    - The app formats and passes the text into `space_scene`, which stays
      pure. `ItemVisual` gains a `///` field for the column texts, empty
      outside Details.
  - Hover and selection fill the whole row. F2's box sits in the Name
    column.
  - Tests: row layout under the header; a header hit maps to its column;
    a second click reverses; the type-name cache key.
  - **[needs human]** check: a folder in Details beside the same folder
    in Explorer's Details: the same columns, values, alignment, date, and
    size format; header clicks sort and reverse; 100 % and 150 %; light
    and dark.

- [x] **W9. A lens's folder icon in its title bar (the "new visual"
  recipe).** **[needs human]**
  - `render/scene.rs` `SpaceScene`: `title_icon: Option<Icon>`, `None` by
    default.
  - `scene.rs` `space_scene`: a lens gets its folder's shell icon at the
    small size (16 px at 96 DPI, scaled), fetched the way item icons are.
    An unavailable folder gets the generic folder icon. Ask the shell
    for it with `SHGFI_USEFILEATTRIBUTES` and `FILE_ATTRIBUTE_DIRECTORY`,
    so a missing folder is never touched. A normal space gets `None`.
  - `render/d2d/`: draw the icon left of the title, centered in the title
    bar, and move the title text right by the icon and a gap. An inline
    title rename moves the same way. If the app turns a click in the
    title into a caret position, shift that too.
  - Test: copy `the_settings_highlight_reaches_the_scene`. A lens's scene
    has a title icon, and a normal space's has none.
  - **[needs human]** check: lenses on Downloads, on Documents, and on a
    plain folder show each folder's own icon. Rename the folder away, and
    the generic folder icon shows. F2 on the title starts the rename box
    after the icon. The same folded, at 100 % and 150 %, on every
    backdrop. A normal space's title bar is unchanged.


- [x] **W19. The status bar (core, app, platform).** **[needs human]**
  - Core: `SpaceView.status_bar: bool`, off by default.
    `Space::content_rect` leaves `STATUS_BAR_HEIGHT` (measure Explorer's;
    scale it as `title_height` is) at the bottom while it is on and the
    space is unfolded. Layout, scrolling, and hit tests then shrink with
    no other change.
  - `CreateLens` gains `view: SpaceView`. The app passes a view with the
    status bar on (D15), and W23 turns that into a setting. New normal
    spaces keep the default.
  - `SpaceScene.status: Option<StatusBar>`, with the left `segments` and
    the right `buttons` (rect, glyph, checked, hovered). The app builds
    it:
    - A pure `status_segments(count, selected, selected_bytes:
      Option<u64>) -> Vec<String>`: "1 item" or "12 items", then "1 item
      selected" or "3 items selected", then the size. The size shows only
      when every selected item is a file with a known size, as in
      Explorer. The size comes from a platform `format_bytes`
      (`StrFormatByteSizeEx`, "1.20 MB"). Draw the segments with Windows
      11 Explorer's separators.
    - `STATUS_BUTTONS`: Details and Large icons, each a mode, a glyph,
      and Explorer's tip: "Displays information about each item in the
      window." and "Displays items by using large thumbnails." Build the
      shortcut part of each tip, "(Ctrl+Shift+6)", from the mode's
      `VIEW_MODES` digit.
    - An unavailable lens shows no segments, but keeps its buttons.
  - `app/pointer.rs`: a click on a button dispatches `SetSpaceView`. The
    hit test is pure and uses the same rects the scene carries. Hover
    highlights a button and shows its tip through `info_tip`. The current
    mode's button is drawn checked.
  - Menu: View ▸ "Status bar", a check entry under the modes.
  - Tests: the segments for 0, 1, and many items, with and without a
    selection; the size only when all selected are files; `content_rect`
    shrinks; the button hit test; serde default; `CreateLens` keeps its
    view.
  - **[needs human]** check: a new lens has the bar, and a new space does
    not. The menu toggles it. The counts follow a click, Ctrl+A, and the
    marquee; the size shows for files only. Both buttons switch the view
    and show which is on, and their tips read as Explorer's. Folded, the
    bar is gone. Check every backdrop (the text is as legible as the
    labels), at 100 % and 150 %, in light and dark.


- [x] **W20. The View page (app).**
  - Every space's scope lists `page::VIEW`, always enabled, like
    Shortcuts. It has a View choice (`VIEW_MODES`) and a Status bar
    toggle. Under a "Sort" heading come Sort by (`SORT_KEYS`), Order
    (Ascending, Descending), Folders first, and Keep sorted (on and grey
    for a lens). Each edit becomes `SetSpaceView` and previews.
  - `SpaceSnapshot` gains the view, and Cancel restores it.
  - Built for Global too, so the form keeps its shape, but not listed
    there.
  - Tests: `every_editable_space_field_has_a_handler` covers the page;
    Cancel restores the view; Keep sorted is grey for a lens.
  - Done when: also `CONTEXT.md` section 6 names the page.

- [x] **W21. Tiles (app, platform).** **[needs human]**
  - `ViewMode::Tiles` (digit 7): tiles in rows that wrap, each a 48 px
    icon at the left and three lines beside it (name, type, size; the
    second and third dimmed, as Explorer draws them). The text comes from
    W18's helpers, and the layout from W16's. The tile size is measured
    from Explorer.
  - Tests: the layout's rects and extent.
  - **[needs human]** check: beside Explorer's Tiles on the same folder,
    at 100 % and 150 %, in light and dark.

- [x] **W22. Content (app, platform).** **[needs human]**
  - `ViewMode::Content` (digit 8): one row per item, with Explorer's
    separator line between rows. The icon is on the left, with the name
    and the type beside it; "Date modified: …" and "Size: …" are on the
    right. The text and layout come as for W21.
  - Tests and check: as W21, against Explorer's Content.

- [x] **W23. The Lenses page: defaults for new lenses (config, app).**
  - `deskspace-config` `WindowSettings.new_lens: NewLens { view:
    SpaceView, title: LensTitle, show_hidden: bool }`, with
    `#[serde(default)]`. Its default has the status bar on, `FolderName`,
    and the default sort.
  - Global lists `page::LENSES` after New spaces. Its fields are View,
    Status bar, Sort by, Order, Folders first, Show hidden files, and Show
    as. Build the shared fields with the View page's builder (one
    function given a `&SpaceView`, with the Lenses page's own field ids),
    not a copy.
  - `App::create_lens` reads `new_lens`, replacing W19's fixed view and
    W11's fixed `FolderName`.
  - Tests: the config's round trip and missing-field default; a new lens
    gets the defaults.
  - **[needs human]** check: change each default, and "New lens…" uses
    it; existing lenses do not change.


- [x] **W44. Undo for lens and view actions (app).** The undo work (U1-U7,
  D11) and the lens work (D17, D18) met late, so no lens or view action
  records an entry yet: `SetSpaceView` (the View menu, Ctrl+Shift+1..8, the
  status bar's buttons), `SetLensView` (Sort by, Show hidden files, the Lens
  page), `SetLensFolder` (Change folder…), `SetLensTitle`, and `CreateLens`
  (New lens…) all reach the model through a bare `commit`.
  - Record each as one entry through `App::record_effects` (the "new model
    change" recipe), labelled as its menu names it ("Change view", "Sort
    lens", "Change folder", "New lens"), and restart the watch after an
    applied change as the menu already does. `CreateLens` in
    `App::show_new_space` opens its entry; check it labels it "New lens" and
    finishes it on failure.
  - A restored lens must list its folder again: after an undo or redo that
    restored one, `App::sync_watches`. The command itself is
    `RestoreSpaces`, so nothing new is needed in core.
  - Tests in `app/undo.rs` (copy the layout tests): each action records one
    entry that undoes and redoes; a restored lens gets a watch; a change
    that does not apply records nothing.
  - **[needs human]** check: change a view, sort a lens, toggle its hidden
    files, change its folder, and create a lens; each undoes and redoes from
    Ctrl+Z / Ctrl+Y and both menus, and a lens brought back lists its folder
    again.
- [x] **S1. The model (core).**
  - `deskspace-core/src/icon.rs`: `IconRef { set: String, name: String }`
    and `SpaceIcon { None (default), Automatic, Chosen(IconRef) }`, both
    `Debug, Clone, PartialEq, Eq`. `IconRef::parse("fluent:games")` accepts
    a prefix and a name of lowercase ASCII letters, digits, and hyphens (the
    Iconify name rule), split at the one `:`; `Display` writes it back.
  - `SpaceIcon` saves as a string, via `#[serde(from = "String", into =
    "String")]`: `"automatic"`, or `"<set>:<name>"`. Any other string loads
    as `None` instead of failing the file (ENGINEERING rule 3). Document
    that on the type.
  - `Space.icon: SpaceIcon`, `#[serde(default, skip_serializing_if =
    "SpaceIcon::is_none")]`, `pub(crate)` with an `icon()` accessor, like
    `locked`. Missing means `None`, so every existing space keeps no icon.
  - `Command::SetSpaceIcon { id, icon }`: `Applied`, or `NotFound`. A locked
    space still takes it, as `RenameSpace` does. Its `touched_by` arm joins
    `RenameSpace`'s.
  - `Command::CreateSpace` gains `icon: SpaceIcon`, set on the new space.
    Every caller passes `SpaceIcon::None` for now (S8 passes the New spaces
    default); the compiler lists them.
  - Tests: core, `parse` and `Display` round trip, bad strings rejected,
    and `SetSpaceIcon`'s two outcomes (copy the `RenameSpace` tests).
    Config, in `schema.rs`'s tests (core has no format crate): a space with
    each `SpaceIcon` survives a TOML save and load, a space without the key
    loads as `None` and writes none, and `icon = "?"` loads as `None`.
  - `CONTEXT.md` section 4, "Space": one sentence saying a space stores a
    reference to its icon, not the picture, resolved when it is drawn (S7).

- [x] **S2. Icon sets and matching (core, pure).**
  - `deskspace-core/src/icon_set.rs`: the subset of the Iconify JSON
    format DeskSpace reads
    (<https://iconify.design/docs/types/iconify-json.html>; cite it on the
    type), `Serialize` and `Deserialize`, camelCase:
    `IconSet { prefix, info: IconSetInfo, icons: BTreeMap<String,
    IconData>, aliases: BTreeMap<String, IconAlias>, width, height }`, where
    `IconData { body, width, height }` (both optional, falling back to the
    set's, which default to 16 as the format says), `IconAlias { parent,
    rotate, h_flip, v_flip }`, and `IconSetInfo { name, author: { name, url
    }, license: { title, spdx, url } }`, all `#[serde(default)]`. Unknown
    keys are ignored.
  - `Glyph { key: String, body: String, width: u32, height: u32 }` (`key`
    is `"<prefix>:<name>"`) and `IconSet::glyph(name) -> Option<Glyph>`. It
    follows aliases to their icon, at most 8 steps so a cycle ends, and
    refuses an alias with a rotation or a flip, which DeskSpace does not
    draw. `IconSet::names()` lists icons and plain aliases.
  - `deskspace-core/src/icon_match.rs`, the D16 matching. Words are the
    title in lowercase, split on every character that is not alphanumeric,
    dropping words shorter than 3 letters and those in `STOP_WORDS` ("the",
    "and", "for", "new", "space", "stuff", "misc", "other").
    Each word also tries a singular form: "-ies" to "-y", "-es" after s, x,
    ch, or sh, else a trailing "s" on a word longer than 3 letters.
    - `best_icon<'a>(title, set: &'a IconSet, words: &'a [(&str, &str)])
      -> Option<&'a str>` scores each word, as written and singular: 100 a
      curated `words` row for the word, 95 for its singular, 80 an icon or
      plain alias named exactly the word, 75 its singular, 60 an icon whose
      first hyphen-separated part is the word or its singular. A curated
      row whose icon the set lacks is skipped. The highest score wins; ties
      go to the earlier word in the title, then the shorter name, then
      alphabetical order. No candidate, no icon. Aliases resolve to their
      icon's name.
    - `search<'a>(query, set, words) -> Vec<&'a str>` for the picker. The
      query is split as above, but with no stop words or minimum length.
      An icon matches when every query word is a prefix of one of its
      hyphen-separated name parts or an alias's, or a curated row maps the
      word to it. Curated hits come first, then icons with a part equal to
      a query word, then the rest, each group alphabetical, with no
      duplicates. An empty query lists every icon alphabetically.
  - Tests, on small sets built in the test: aliases resolve, a cycle and a
    transformed alias give `None`, sizes fall back to the set's; each score
    level wins over the one below it; ties break as stated; stop words and
    short words never match; "Libraries", "Boxes", and "Games" reach
    "library", "box", and "game"; "New space" gives `None`; every `search`
    ordering rule.

- [x] **S3. Icon sets, the cache, and the trimmer (config).** Needs network
  once, to fetch the upstream set; stop and ask if you have none.
  - `cargo-add` `serde_json` to `deskspace-config` (it is already in
    `Cargo.lock` through the updater, so `cargo deny` sees nothing new).
  - `deskspace-config/src/icons.rs`:
    - `IconSetSpec { prefix, label, author, license, upstream,
      upstream_version, keep_suffix, revision, words }`, all `&'static`
      except `revision: u32`, and `pub const ICON_SETS: &[IconSetSpec]`
      with one row: prefix `fluent`, label "Fluent UI System Icons", author
      "Microsoft Corporation", license "MIT", upstream
      `@iconify-json/fluent` at the newest version on npm today
      (`https://registry.npmjs.org/@iconify-json/fluent/latest` says which),
      `keep_suffix` `-20-regular` (one-color outlines, as Windows 11's own
      title-bar glyphs are), revision 1, and `FLUENT_WORDS`. Add
      `icon_set(prefix)`, `default_set()` (the first row), `file_name()`
      (`fluent-r1.json`), and `release_tag()` (`icons-fluent-r1`).
    - `FLUENT_WORDS: &[(&str, &str)]`, English words to icon names, for
      common space titles. Start from: apps, games, work, office, school,
      study, homework, photos, pictures, videos, movies, music, podcasts,
      documents, downloads, projects, code, tools, utilities, design, art,
      notes, books, finance, money, bills, taxes, shopping, travel, health,
      fitness, recipes, home, family, kids, social, chat, mail, calendar,
      web, links, favorites, cloud, backup, archive, temp, important,
      todo, ideas, research, science, sports, weather, maps, news,
      security, network, server, database, terminal, scripts, writing,
      presentations, spreadsheets, clients, meetings. Pick each target by
      searching the names file. Leave out a word the set already names
      exactly, which matching finds on its own.
    - `IconStore`, made by `ConfigStore::icons_dir()`, which is
      `%LOCALAPPDATA%\DeskSpace\DeskSpace\data\icons`, or an `icons` folder
      beside an explicit config, as `storage_dir()` does, so a scratch
      config has its own cache. `load(spec) -> Result<Option<IconSet>,
      ConfigError>`: `None` when the file is missing; a file that does not
      parse, or whose `prefix` is not the row's, is deleted and returned as
      an error, so the next need fetches it again. `save(spec, bytes)`
      parses first, writes a sibling `.tmp` and renames it (as
      `ConfigStore::save` does), then deletes this prefix's other revisions.
      The cache is trusted once written: only the download is verified
      (S4).
    - `trim(upstream: IconSet, spec) -> IconSet`: keeps the icons whose name
      ends with `keep_suffix`, with the suffix removed, and the aliases that
      end with it, have no transform, and point at a kept icon. It keeps
      `info` and sets `prefix`. A width and height every kept icon shares
      move to the set level. `missing_names(set, shipped: &str) ->
      Vec<String>` lists the shipped names the set lacks.
  - `crates/deskspace-config/examples/icon_set.rs`, the glue:
    - `trim <prefix> <upstream icons.json> <out dir>` writes
      `<out dir>/<file_name>` as compact JSON. `BTreeMap` keeps the order,
      so the same input gives the same bytes. It exits non-zero, listing
      them, if any name in `assets/icons/<prefix>-names.txt` is missing,
      then rewrites that file: every name, sorted, one per line.
    - `spec <prefix>` prints `tag=`, `file=`, and `upstream=<package>@
      <version>` lines, for the workflow (S5).
  - Fetch the upstream package from `https://registry.npmjs.org/
    @iconify-json/fluent/-/fluent-<version>.tgz`, extract `package/
    icons.json` into your scratchpad, run `trim`, and commit the new
    `assets/icons/fluent-names.txt` (not the set itself). Put the icon and
    alias counts in the commit body.
  - Tests: a small Iconify file parses, unknown keys and all; `trim` keeps,
    renames, and drops exactly as stated; `missing_names`; `IconStore` round
    trip in a temp dir, a corrupt file deleted, older revisions pruned, and
    a scratch config's `icons_dir()` beside it. For every row: its names
    file is sorted with no duplicates, every `words` target is in it, its
    words are single lowercase words with no duplicates, and prefixes are
    unique.
  - `CONTEXT.md` section 5: one bullet on `icons_dir()` and that the cache
    is trusted once written.

- [x] **S4. Signed downloads (update).**
  - `deskspace-update/src/lib.rs`: `pub fn fetch_signed(tag, file) ->
    Result<Vec<u8>, UpdateError>`. It downloads `{ASSETS_URL}/{tag}/{file}`
    and `{file}.sig`, where `ASSETS_URL` is
    `https://github.com/generalchad/DeskSpace/releases/download`, then
    verifies them against `UPDATE_PUBKEY`. Without a key it is
    `NotConfigured`, as `check` is. Use a blocking `reqwest` client with a
    30 s timeout and a `DeskSpace/<version>` user agent. Refuse a body over
    `MAX_ASSET_BYTES` (32 MiB), by its `Content-Length` and while reading,
    so a bad server cannot fill memory. Blocking: the doc says to call it
    on a worker thread.
  - `fn verify(data, signature, pubkey) -> Result<(), UpdateError>`:
    base64-decode the key and the `.sig` text, then `minisign-verify`'s
    `PublicKey::decode`, `Signature::decode`, and `verify(.., true)`. That is
    exactly what `cargo-packager-updater` 0.2.3 does in its
    `verify_signature`; cite it (crates.io source, `src/lib.rs`), so one key
    and one `cargo packager signer sign` serve both. New `UpdateError`
    variants `Fetch(String)` and `Signature(String)`.
  - `cargo-add` `reqwest` (no default features; `blocking` and
    `native-tls`, as the updater uses), `minisign-verify`, and `base64` at
    their locked versions. `cargo-deny-check` must pass with no new crates.
  - Update the crate doc: the app also fetches icon sets, only when the
    user has agreed (D16).
  - Tests, with no network: generate a throwaway key with
    `cargo packager signer generate` (no password), sign a small file, and
    commit the public key, the file, and its `.sig` under
    `crates/deskspace-update/tests/fixtures/` (never the private key). A good
    signature verifies; a changed byte, another key, and garbage base64
    each fail with an error, not a panic.


- [x] **O1. The harness (scripts).**
  - `scripts/onscreen/OnScreen.psm1`, one module every check imports:
    - the P/Invoke class from `zorder.ps1`, moved here, plus
      `GetDpiForWindow`, `GetDC`/`BitBlt` (`CAPTUREBLT`), and
      `CreateDIBSection`; `zorder.ps1` imports it, and its output is
      unchanged.
    - `Start-DeskSpace -Config <toml>`: builds with `./scripts/dev.ps1
      -NoRun`, writes the TOML to a fresh folder under `%TEMP%`, starts
      `deskspace.exe --config <it> --on-top --no-icons`, and returns once
      every space window is visible and its rect has held still for 500 ms.
      Configs set Startup to None, so nothing animates.
    - `Stop-DeskSpace`: ends that process and deletes the folder.
    - `Show-Backdrop -Color <rgb>`: a borderless, non-topmost window of one
      color over the primary monitor, under the `--on-top` spaces.
    - `Get-DeskSpaceWindows`: each window of the process with its class,
      handle, rect, extended style, DPI, and z-order index.
    - `Get-ScreenPixels -Rect <r>`: the composed screen there, as BGR.
    - `Assert-Check <name> <bool> <detail>` and `Complete-Checks`: print a
      PASS / FAIL / NOT COVERED table and exit non-zero on any FAIL.
  - `scripts/onscreen/smoke.ps1`: one Solid space with an opaque red tint
    over a white backdrop. The capture reads red at the space's center and
    white well away from it.
  - **Stop and ask** if the smoke check cannot read the space's pixels
    with `BitBlt` (composition content missing from the capture). The
    fallback, `Windows.Graphics.Capture`, is a design change.
  - `ENGINEERING.md` section 1 lists `scripts/onscreen/`, and `TASKS.md`
    rule 8 says to run any `onscreen` script that covers a check before
    handing what is left to a human.
  - Done when `smoke.ps1` exits 0 here and `zorder.ps1` prints what it did.


- [x] **O2. Shadows: which windows, where (D15).**
  `scripts/onscreen/shadows.ps1`, structure only, no pixels. It reads
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\EnableTransparency`
  and runs the matching half of the D15 row. Cases: Acrylic, Blur, Solid,
  and Clear at radius 0, 4, 8, and 16; `--placement desktop-bottom`; and
  `--takeover-layer`. For each:
  - Exactly one visible `DeskSpaceShadow` per space that should cast one
    (the row's rules), and none for Clear, desktop placement, and the
    layer; with `show_shadow = false`, none at all.
  - The shadow is the next window below its space
    (`GetWindow(space, GW_HWNDNEXT)`).
  - Its extended style has `WS_EX_LAYERED`, `WS_EX_TRANSPARENT`,
    `WS_EX_NOACTIVATE`, and `WS_EX_TOOLWINDOW`.
  - Its rect is `ShadowLook::rect` of the space's rect (`render/shadow.rs`),
    with the reach scaled to the window's DPI the way
    `space_window::sync_shadow` scales it. Read both; do not guess.
  - `WindowFromPoint` at the middle of the band left of the space, and of
    the band below it, reaches the backdrop, not the shadow.
  - Two spaces, one customized with `shadow_size = 40`: each shadow's
    reach matches its own theme.
  - Done when it exits 0 here with every case of this machine's half
    covered.
- [x] **O3. Shadows: the pixels (D15).** `shadows.ps1 -Pixels` adds, over
  a white backdrop, alpha read as `1 - R / 255` on lines out from the
  middle of the left edge, the bottom edge, and the caster's bottom-left
  corner:
  - Beside the space a straight edge shows half the blur, so alpha is
    within 0.09 of half `shadow_opacity`; the drop uncovers a band below,
    where it is within 0.10 of three quarters. No pixel outside the space
    is darker than the setting (`shadow_size` 16 and 48; opacity 0, 40,
    and 100; at 125 %).
  - Alpha never rises going outward, and is 0 by the reach.
  - The band below is wider than the band above by the drop
    (`ShadowLook::drop`).
  - A 16 px corner thins the shadow where a square one does not: on the
    diagonal from the caster's bottom-left corner, at `r * (1 - 1/sqrt(2))`
    out, the 16 px alpha is at most half the square corner's.
  - Inside a Solid space the pixels equal its tint: the cut-out leaves
    the space undarkened.
  - Opacity 0 hides the shadow window and leaves the backdrop untouched.
  - Done 2026-10-27: exits 0 here. Section V keeps the animations, the
    Settings page, the debug toggles, other DPIs, Windows 10, and the
    desktop-hosted placement. Amended then, by the maintainer's decision:
    the setting is the black's own alpha, not the darkest pixel, so the
    theme field and D15 say so.


- [x] **S6. Drawing a glyph (platform).**
  - `render/glyph.rs` (pure): `svg_document(glyph: &Glyph, color: Color) ->
    String` wraps the body in `<svg xmlns=".." viewBox="0 0 w h">`, writes
    `color` as `#rrggbb` in place of every `currentColor` (Direct2D's SVG
    support has no `currentColor`), and adds an `opacity` for a color that
    is not opaque.
  - `render/d2d/glyph.rs`: `rasterize(glyph, px, color) -> Result<Icon,
    PlatformError>`. On the shared device (`create_device`), draw into a
    `px` square target bitmap with `ID2D1DeviceContext5::CreateSvgDocument`
    (from `SHCreateMemStream`) and `DrawSvgDocument`. Copy the result to a
    CPU-readable bitmap, map it, and build an `Icon` (premultiplied BGRA,
    as `Icon::new` wants). Without `ID2D1DeviceContext5` (Windows 10 before
    1703) it fails, and callers show no icon. Cite the Microsoft Learn
    pages for `CreateSvgDocument` and `DrawSvgDocument` beside the calls.
  - `win32/glyphs.rs`: `GlyphCache`, copied from `IconCache`
    (least-recently-used, 256 entries), keyed by `(glyph key, px, color)`.
    `icon(&mut self, key, px, color, glyph: impl FnOnce() -> Option<Glyph>)
    -> Option<Icon>` builds the glyph only on a miss. It logs a failure
    once per key at `debug`. Re-export `GlyphCache` and `Glyph` from
    `lib.rs`.
  - Tests: `svg_document`'s wrapping, color, and opacity. `rasterize`
    draws `M2 2h16v16H2z` in a 20-wide glyph at 20 px in red: the middle
    pixel is opaque red and a corner is clear. It runs on WARP in CI, as
    `create_device` falls back to it. The cache evicts the oldest entry
    and keys on color.
  - `CONTEXT.md` section 6, "Rendering": one bullet on glyphs. Add the new
    modules to the module map.
  - Done 2026-10-27. The document also sets the root's `fill` to the
    color, so a body that names no paint still draws in it; the tests'
    bodies are `<path d="..."/>` elements, since a body is SVG content,
    not path data. The cache tests draw real glyphs through WARP.


- [x] **S7. Icons in the title bar (app, platform).** **[needs human]**
  - `render/scene.rs`: `SpaceScene.title_icon: Option<TitleIcon>`, with
    `TitleIcon { rect: Rect, image: Option<Icon> }` in window-local pixels;
    default `None` (the visual-in-a-space recipe).
  - `deskspace-app/src/scene.rs`: pure `title_icon_rect(title_height) ->
    Rect`, a square `TITLE_ICON_SIZE` (16) by `TITLE_ICON_INSET` (10) at 96
    DPI and a 28 px title, scaled by `title_height / DEFAULT_TITLE_HEIGHT`,
    centered vertically. `space_scene` fills `title_icon` when the space
    resolves to a picture; the desktop layer never does.
  - `deskspace-app/src/app/space_icons.rs`, a new `app/` module (add it to
    `CONTEXT.md` section 3's table): `SpaceIcons` on `App`, with per-prefix
    state (`Loading`, `Ready(Arc<IconSet>)`, `Missing`, `Failed(String)`),
    a `GlyphCache`, and a memo of automatic matches by title, cleared when
    a set changes.
    - On `Event::Started`, a worker thread runs `IconStore::load` for the
      sets in use (`Automatic` uses `default_set()`), as the update checks
      do. It wakes the app with `ICONS_WAKE` (the next free wake id beside
      `UPDATE_WAKE`), which refreshes the spaces showing that set. No
      network here; S8 adds the download.
    - `App::space_glyph(space) -> Option<Glyph>`: `Chosen` gives its name
      in its set; `Automatic` gives `best_icon` in `default_set()` with the
      row's words; then `IconSet::glyph`, when that set is `Ready`.
      `App::space_icon(space, color, title_height) -> Option<Icon>` puts it
      through the cache, at the rect's size, in the space's `title_text`
      color. `refresh_space` passes that to the scene.
  - Painter: move `draw_title` and `LOCK_GLYPH` into a new
    `render/d2d/title.rs`, so `d2d/mod.rs` stays under 800 lines. Draw the
    image at its rect with `bitmap_for`, and start the title text (and the
    rename box) `TITLE_ICON_GAP` (6 px, scaled the same way) after the
    rect. The per-target bitmap cache is pruned to the scene's icons, so
    count the title icon among them.
  - Tests: `title_icon_rect` at 28 and 42 px; a space with
    `Chosen`, with `Automatic` and a matching title, and with `None` gives
    the scene a rect or none (copy
    `the_settings_highlight_reaches_the_scene`); the layer's scene has
    none.
  - **[needs human]** check: with a scratch config, make the cache by hand
    (S3's example: `trim fluent <upstream icons.json>
    %TEMP%\deskspace-dev\icons`). Then set `icon = "fluent:games"` on one
    space and `icon = "automatic"` on spaces titled "Games", "Photos", and
    "Stuff". The first three show their icons left of the title, in the
    title's color, and follow a dark and light switch. "Stuff" and spaces
    without the key show none and look as before. Hovering or clicking the
    icon does what the title bar does. Check at 100 % and 150 %, on every
    backdrop, and with a lock. With the cache deleted, nothing shows and
    nothing fails.
  - Done 2026-10-27. `TitleIcon` carries the rect in window-local pixels,
    so the painter no longer centers a lens icon itself; `d2d/title.rs`
    holds the title drawing and its layout tests. The set load is one
    worker per set, so a chosen set and the default one load in
    parallel. The app tests cover `title_icon_rect`, the no-icon scene,
    and the glyph resolution (chosen, automatic, none, missing); the
    scene-to-painter wiring is what the on-screen check above proves,
    since the app cannot build an `Icon` without the shell or COM.


- [x] **S13. Credits (app, docs).**
  - `settings/pages.rs` `about_page`: one `Field::note` (`ABOUT_ICONS`, the
    next free id) with a line per `ICON_SETS` row: "Icons: `label` by
    `author`, `license` License." One field whatever the number of sets,
    so a pack is never a new id.
  - `NOTICE`: the Fluent UI System Icons copyright line and MIT license
    text, under a heading naming the set and its upstream URL.
  - Tests: the About page lists every row; `NOTICE` names every row's label
    (read it as `deskspace-update`'s tests read `release.yml`).
  - Done 2026-10-27. The note is one `Field::note` with one line per
    `ICON_SETS` row, so a new set never takes a new id; `NOTICE` has
    the Fluent UI System Icons heading, upstream URL, copyright, and
    MIT text. The About note and `NOTICE` are tested against
    `ICON_SETS`; the note's on-screen height is a glance for the next
    pass through Settings > About.


- [x] **S8. Consent, the download, and new spaces (config, app).**
  **[needs human]**
  - `deskspace-config/src/schema.rs`: `WindowSettings.icon_downloads:
    IconDownloads { Ask (default), Allowed, Declined }` and
    `new_space_icon: NewSpaceIcon { Automatic (default), None }`, with
    `NewSpaceIcon::space_icon()`. Both snake_case enums.
  - `app/space_icons.rs`: a pure `next_step(state, consent, asked) ->
    IconStep { Nothing, Load, Ask, Fetch, Unavailable }`, where `asked`
    means the prompt already showed this session. It is the one place that
    decides to download, and only `Allowed` gives `Fetch` (S rule 2).
    `App::need_icon_set(ui, spec)` acts on it.
    - `Fetch`: a worker runs `fetch_signed(release_tag, file_name)`, then
      `IconStore::save`, and wakes `ICONS_WAKE`. A failure becomes
      `Failed`, logged at `warn`, and is tried again only at the next
      start.
    - `Ask`: arms `Timer::IconConsent` (the next free timer, 300 ms, so a
      new space's animation starts and an OLE drop returns first). Its
      handler shows `ui.choose` titled "Show icons on spaces?". The text
      names the set's label, author, and license, says it is downloaded
      once from DeskSpace's releases on GitHub and that nothing about you
      or your files is sent, and points at Settings > General. The buttons
      are "Download icons" and "No thanks"; save `Allowed` or `Declined`,
      and Esc counts as "No thanks".
  - Call `need_icon_set` where an icon is first needed: when a load finds
    no file and a space uses the set, and after `open_new_space` makes an
    `Automatic` space. `open_new_space` passes
    `window.new_space_icon.space_icon()` to `CreateSpace`. `seed_space`
    keeps `SpaceIcon::None`, so the first launch never asks.
  - Settings (the new-global-setting recipe): an "Icon" choice on the New
    spaces page (Automatic, No icon), and "Icon downloads" on the General
    page (Ask first, Allowed, Never), with a note naming what is downloaded
    and from where. Option tables go in `tables.rs`. In
    `App::apply_settings`, a change to `Allowed` calls `need_icon_set` for
    the sets in use.
  - README: the privacy paragraph names the one-time icon download, which
    asks first, beside the update check. Add the two settings to
    `CONTEXT.md` section 5.
  - Tests: `next_step` for every state, consent, and `asked` combination,
    `Fetch` only with `Allowed`; the settings' serde defaults and old files
    without the keys; each `apply_field` arm; a new space gets the
    configured icon (through `fixtures.rs`); the seed space gets none.
  - **[needs human]** check, with a scratch config, before the key exists:
    New space opens a "Games"-titled space. The prompt appears once.
    "Download icons" logs a `NotConfigured` failure and shows no icon, and
    nothing else breaks. With S5 published and the key set: the same steps
    download once, then every automatic space shows its icon. The next
    start reads the cache with no network (check with `-Verbosity 1`).
    "No thanks" never asks again until General is set back to "Ask first".
    `-Fresh` first launch: no prompt.
  - Done 2026-10-27. `next_step` is the one download decision, with a
    5-by-3-by-2 test matrix in `space_icons.rs`. A load or download
    result wakes the app; missing sets then ask or fetch, and one
    shared channel carries both. The consent question waits 300 ms
    (`Timer::IconConsent`), saves Allowed or Declined, and Esc counts
    as No thanks. `create_new_space` builds its command in
    `new_space_command`, tested through `fixtures.rs`; the seed space
    asserts `None`. The settings gain "Icon" and "Icon downloads"
    with their tables and arms. The published set and key are what the
    on-screen check above still needs.


- [x] **W26. One place decides whether a label shows its extension
  (app).** Any time; before W28.
  - `self.surface_match_explorer(surface) &&
    self.system.shell().show_extensions` is written out in
    `app/rename.rs`, `app/selection.rs`, `app/settings_session.rs`, and
    `app/view.rs`. Move it into one `App::shows_extensions(surface)` in
    `app/view.rs` and call that from all four. A pure move.
  - Done when: `rg "show_extensions" crates/deskspace-app/src/app` finds
    only the new method and its callers' use of its result.
  - Done 2026-10-27: `shows_extensions` is in `app/view.rs`; the
    rename, selection, settings-preview, and scene builders call it.

- [x] **W24. A listing carries the folder it read (platform, app).**
  Any time.
  - Why: `FolderListing::Listed` has no path, and `WatchKey::Space(id)`
    keeps its key when a lens changes folder. So a listing posted for the
    old folder, still in the queue when Change folder… runs, is applied
    to the new one. W42 makes this happen on every step into a
    subfolder.
  - `input.rs`: `Event::FolderChanged` gains `folder: PathBuf`, the path
    the worker listed (or found unavailable). `win32/folder_watch.rs`
    fills it from the watch's own record.
  - `app/lens.rs`: a pure `listing_is_current(listed: &Path, shown:
    &Path) -> bool` (without case, `/` and `\` alike). A stale listing is
    dropped with a `debug!` line. The desktop layer's listing gets the
    same check against the Desktop folder.
  - Tests: the pure check (same path, another case, other slashes,
    another folder); the controller decode keeps the path.
  - Done when: also `CONTEXT.md` section 6 (Desktop shell) says a
    listing names its folder.
  - Done 2026-10-29: `Event::FolderChanged` carries `folder`, the path
    of the slot that listed it. The stale listing came through the
    runtime's pending queue: a change decoded during `pick_folder`'s pump
    waited there until after Change folder… had re-pointed the watch.
    `on_lens_listing` drops a listing whose folder the lens no longer
    shows (`lens_shows`, tested without a `Ui`). `on_desktop_listing`
    re-points the watch instead, since a mismatch there means the Desktop
    moved. The controller's decode has no test: it needs a live runtime.

- [x] **W25. The storage check runs off the UI thread (platform, app).**
  Any time.
  - Why: `ItemStore::allows_lens` calls `std::fs::canonicalize` on the
    lens's folder and on the store, on the UI thread. `sync_watches`
    calls it twice for every lens, through `desired_watches` and
    `refused_lenses`, each time the watches are reconciled. On an
    unreachable network share, `canonicalize` can block until the SMB
    timeout, freezing every space. It is also the only junction guard,
    so it must not simply be dropped.
  - Canonicalize the store once, at startup (`ItemStore::new`), and keep
    it.
  - `win32/folder_watch.rs`: when the worker opens a watched folder's
    handle, it asks for the handle's final path
    (`GetFinalPathNameByHandleW`, Microsoft Learn link beside it) and
    refuses a folder that is the store or inside it. It posts the refusal
    as a new `FolderListing::Refused`. That costs no extra IO, since the
    handle is already open. The store root is passed in when the watch
    starts.
  - `app/lens.rs`: `desired_watches` and `refused_lenses` stop calling
    `allows_lens`; a `Refused` listing marks the lens unavailable, as
    `refused_lenses` does today. `allows_lens` stays only where a user
    has just picked a folder (New lens…, Change folder…), and it
    compares against the stored root.
  - Tests: a pure `inside_store(final_path, root)` (without case, with
    the `\\?\` prefix `GetFinalPathNameByHandleW` adds); the app turns
    `Refused` into an unavailable lens. One platform test makes a
    junction into a temp "store" with `cmd /c mklink /J`, which needs no
    admin rights, and expects `Refused`.
  - **[needs human]** check: a lens on a share whose server is off
    leaves the other spaces responsive at startup; a lens on a junction
    into the store shows "Folder not available".
  - Done 2026-10-29. Settled with the maintainer before coding: the store
    travels with each watch (`Ui::watch_folder(key, path, store)`); the
    drop, paste, and New shortcut checks keep `allows_lens`, since each runs
    once per user action and dropping them would let a drop through a
    junction into the store; a store not created yet resolves through its
    parent; a refused folder retries on the 5 s cadence like a missing one.
    `is_inside` moved to the platform (`win32/names.rs`) and compares
    `\\?\` and plain drives and shares alike; the task's `inside_store` is
    that function. A folder whose final path cannot be read is checked by
    the path as given, as `canonicalize`'s fallback always did.

- [x] **W42. Browse into a subfolder (app).** **[needs human]** After:
  W24, W25.
  - `App` keeps, per lens, the folder shown below its own one: session
    state, a relative path, empty by default. `App::lens_folder(id)`
    joins the two. A pure `browse_into(root, shown, entry) ->
    Option<PathBuf>` refuses anything that is not a directory entry of
    the folder shown, refuses a shortcut, and refuses a result that does
    not stay inside `root` (D18).
  - Every app read of `FolderView.path` that means "where the files go"
    switches to `lens_folder`: `drop_into_lens`, paste, the shortcut
    writer's lens path, delete, Open folder, and `desired_watches`.
    List each switched site in the commit body; leave W10 and W11's
    reads alone.
  - Double-clicking a directory in a lens browses into it. That empties
    the selection, re-points the lens's watch (W24 drops the old
    listing), and scrolls to the top. The title bar shows `Root › Sub ›
    Sub2`, clipped from the left when too long, from a pure
    `browse_title`.
  - Change folder…, a restart, and deleting the lens forget the folder
    shown.
  - Tests: `browse_into` (a directory, a file, a `.lnk`, a name with
    `..` in it, a different case); `browse_title`; each switched site
    reads the folder shown.
  - **[needs human]** check: browse two levels down in a Downloads
    lens; drop, paste, New shortcut, and Delete there all act on the
    subfolder; deleting the subfolder in Explorer while it is shown shows
    "Folder not available"; a junction to DeskSpace's store inside the
    lens shows unavailable when browsed into (W25).
  - Done 2026-10-29. Settled with the maintainer before coding: the
    title is cut from its start by the painter (`title_keeps_end`, measured
    once per title and width), a small platform change beyond "(app)"; and
    Enter on a folder browses too, sharing `App::open_item` with
    double-click. `App::browsed` keeps the lens folder each browse was made
    under, so a changed folder ignores it at once and `sync_watches` drops
    it; that covers Change folder…, its undo, and deleting the lens without
    a call at each, and Change folder… also drops it for the same folder.
    `lens_folder` moved to `app/lens.rs`. After review: the title icon
    follows the folder shown, so it and the unavailable flag agree; a
    browse keeps the lens current, so the keys still reach it; the cut
    measures DirectWrite clusters once per title, so it splits no
    character and a resize recuts without layout.

- [x] **W43. Back and Up (app, platform).** **[needs human]** After:
  W42.
  - A Back button at the left of the title bar, before the folder icon,
    only while a lens shows a subfolder (a `SpaceScene` field, the "new
    visual" recipe). Clicking it shows the parent. So do Alt+Left,
    Backspace, Alt+Up (the key recipe, under `WATCH_PASTE`, so they act
    only while a space is current), and the mouse's back button
    (`XBUTTON1`, a new `SpaceEvent` field). At the lens's own folder they
    do nothing, and Backspace passes on to type-to-select as today.
  - Docs: the `ENGINEERING.md` Keys table, `CONTEXT.md` section 8, and
    `README.md` Controls.
  - Tests: the parent of each depth; nothing above the root; the keys'
    `keys_for_*` rows.
  - **[needs human]** check: each way back at 100 % and 150 %; the
    button is gone at the root; Backspace in a rename edits the text and
    does not go back.
  - Done 2026-10-29. Settled before coding, as the maintainer asked:
    the hook takes the keys only under a new `WATCH_BACK` flag (the current
    space is browsed), since it decides before the app is asked; at a
    lens's own folder Backspace passes through as before, and the Alt
    arrows pass through instead of moving the selection, as in Explorer.
    Back and Up both show the parent (D18 keeps no forward history). A
    double-click on the button goes up twice rather than folding the
    space. The mouse's back button acts on the lens under the pointer.
    The button's size is provisional and it has no hover highlight.
    After review: the press on the button is remembered, so the
    double-click that follows never folds the space even when the press
    reached the lens's own folder; a resize edge under the button wins; a
    click on it makes the lens current; the mouse's back button finishes a
    rename first; and Alt leaves the scroll keys as they were.

- [x] **W34. Lens filters: the matcher (core).** After: W14.
  - `deskspace-core/src/wildcard.rs`: `fn matches(pattern: &str, name:
    &str) -> bool` for `*` and `?`, without case, in time linear in the
    two lengths (the greedy matcher that remembers the last `*`; cite
    its source beside it). Compare by `char`, folding with
    `to_lowercase` per char; no allocation per call.
  - `FolderView.include` and `exclude` (already reserved): caps
    `MAX_PATTERNS` (32) and `MAX_PATTERN_LEN` (260 chars), applied by
    `FolderView::clamped` on load and on edit. An empty pattern is
    dropped.
  - `SyncLens` applies them (D18): "Show only" to files, "Hide" to every
    entry, both by name only, never by full path.
  - Tests: the matcher on the usual cases and on hostile ones (thousands
    of `*`, a 260-char pattern against a 260-char name, which must stay
    fast in a debug test), non-ASCII names, case; `SyncLens` with each
    list; the caps. Time `SyncLens` with 32 patterns on 50,000 entries
    (see Performance).
  - Done 2026-10-29. The matcher's bound is pattern x name steps, not
    linear as written above; the caps keep it to about 68,000. It checks
    the text after the last star against the name's end first, and
    compares ASCII without Unicode folding. Patterns are also trimmed and
    deduplicated. Release timings of `SyncLens` on 50,000 entries: 44-46
    ms with no filters (already over the 8 ms budget before W34), 62-65 ms
    with 32 typical patterns, 309 ms with 32 whose end matches and middle
    fails. Taken to the maintainer before merging, as the Performance
    rule asks.

- [x] **W35. Lens filters on the Lens page (app).** **[needs human]**
  After: W10, W34.
  - A "Filter" heading with two text fields, "Show only" and "Hide",
    each a `;`-separated list (`*.pdf; *.docx`). A pure
    `parse_patterns(text) -> Vec<String>` trims spaces and drops
    empties; `format_patterns` writes it back. Edits preview and Cancel
    undoes them (W10's snapshot).
  - A lens whose filters hide every entry shows "No items match the
    filter" in `SpaceScene::placeholder`, so it never looks broken.
  - Changing a filter re-filters the listing the app already has; it
    does not re-list the folder.
  - Tests: `parse_patterns` round trip; the placeholder.
  - **[needs human]** check: filter a Downloads lens to `*.pdf`; Hide
    `*.tmp`; a filter that matches nothing shows the message; typing
    quickly in the field stays smooth on a 50,000-file lens.
  - Done 2026-10-29. The window rewrites a text field whose value
    differs from the form, so the fields show the text as typed
    (`Session::filter_texts`, as `hotkey_texts` does), and the lens's
    list only when the page is built. `App::listings` keeps each lens's
    last listing with its folder, and a re-filter uses it only while the
    lens still shows that folder. Show hidden files still re-lists, as
    before. Every keystroke re-filters on the UI thread; B37 covers the
    cost on big folders.

- [x] **W46. An empty title follows the folder again (app).** **[urgent]**
  **[needs human]** After: W11.
  - Why: renaming a lens's title (F2 with nothing selected, or a
    double-click on the title) quietly switches its "Show as" to Custom,
    so the title stops following the folder, and committing an empty
    title is ignored. The only way back is the Lens page. Raised by the
    maintainer, 2026-10-29: F2 on the title stays a nickname, never a
    folder rename, but the nickname must be easy to undo.
  - `app/rename.rs`, where a title rename commits: on a lens, an empty
    (or all-space) title becomes `SetLensTitle` with the Lenses defaults
    page's "Show as" (`WindowSettings.new_lens.title`, W23; Folder name
    unless changed), or Folder name when that default is Custom, and the
    title text `lens_title` gives for it. One undo entry, as a rename is
    now. A normal space keeps ignoring an empty title.
  - Decide it in a pure function, for example `title_commit(space,
    typed, default) -> Option<Command>`, so the cases are tested without
    a `Ui`.
  - Tests: a lens with an empty title, an all-space title, and a typed
    title; a normal space with an empty title; the default Custom case.
  - Docs: `CONTEXT.md` section 8 (the F2 and title-rename lines) and
    `README.md` Controls say that clearing a lens's title returns it to
    following the folder.
  - **[needs human]** check: F2 on a lens's title, clear it, Enter: the
    title shows the folder's name and the Lens page shows "Folder name";
    Ctrl+Z gives the old nickname back.
  - Done 2026-10-29. An empty title on a lens that already follows its
    folder is ignored too, so clearing it never swaps one "Show as" for
    another.

- [x] **W45. A lens follows its folder when it is renamed or moved
  (platform, app).** **[urgent]** **[needs human]** ROADMAP D19 is the
  spec. After: W24, W25, W42.
  - Why: renaming or moving a lens's folder in Explorer makes the lens
    "Folder not available" within 5 s, though the watch's open handle
    follows the folder to its new path. A lens that follows its folder
    is also the safe way to offer renaming it: rename in Explorer, or
    through an explicit "Rename folder…" (a later task), never through
    F2 on the title (see W46).
  - Platform: in `check_paths`, when an open folder's path no longer
    names it, the worker reads the handle's final path (`final_path`,
    W25) and posts a new `FolderListing::Moved { to }` instead of
    `Unavailable` when `to` is a real folder, outside the Recycle Bin (a
    `$Recycle.Bin` component, without case), outside the store (W25), and
    on the same volume. Decide that in a pure function.
  - App: re-point the lens with `SetLensFolder` (its title follows when
    "Show as" follows the folder), as one undo entry, and the watch moves
    with it. A browsed lens (W42) whose subfolder moved returns to its
    own folder. Log the move with one `tracing::info!` line.
  - The notice: a general space notice, a translucent bar over the
    space's content showing "Folder moved:" and the new path, hidden
    about 3 s later. Draw and time it as the zoom pop-up is (`app/zoom.rs`,
    `render/zoom_bar.rs`), as a `SpaceScene` field the platform paints,
    so later messages reuse it. If no extension point fits, stop and ask.
  - Tests: the pure "should the lens follow" check (a rename, a move on
    the same volume, into the Recycle Bin, into the store, onto another
    volume); a platform test that renames a watched scratch folder and
    expects `Moved`; the app turns `Moved` into the new folder and a
    notice, and a browsed lens returns to its own folder.
  - **[needs human]** check: rename a lens's folder in Explorer: within
    5 s the lens shows the new path, keeps its items, and shows "Folder
    moved:" with the path for about 3 s; delete it in Explorer: the lens
    says "Folder not available", and never lists the Recycle Bin; Ctrl+Z
    points it back.
  - Done 2026-10-29. The notice is `SpaceScene::notice`, a bar two title
    bars high along the bottom of the item area in the title bar's colors
    (`notice_rect`), set from `App::notices` and cleared by
    `Timer::Notice`. A lens browsed into a subfolder sees only the
    subfolder's watch, so when its own folder is the one renamed it
    returns there and shows "Folder not available" rather than following.

- [x] **W47. A view change keeps the sort rule (core).** **[needs human]**
  - Why: `SpaceView::with_mode` gives the default sort rule, so a View
    menu choice or a Ctrl+Shift digit resets a space's sort rule and its
    Keep sorted choice, though D17 says a space keeps its last rule
    (confirmed by the maintainer, 2026-10-29).
  - Build the new view from the current one and change only the mode.
    Every caller of `with_mode` gets this; if one needs the default rule,
    stop and ask.
  - Tests: a space sorted by date, descending, with Keep sorted off,
    keeps all three across a mode change, by the View menu's path and by
    the Ctrl+Shift path.
  - **[needs human]** check: sort a space by Date, descending, then press
    Ctrl+Shift+6 and Ctrl+Shift+2: Sort by ▸ still shows Date and
    Descending.
  - Done 2026-10-29. `SpaceView::with_mode` is now a method on the
    current view, so the View menu (`view_after`), the Ctrl+Shift keys
    (`view_by_digit`), and the status bar's view buttons all keep the
    sort rule, Keep sorted, and the status bar. The status bar was reset
    too, so its own view buttons used to hide it.

- [x] **W29. Date created (core, platform, app).** After: W18.
  - `FolderEntry.created` and `ItemFacts.created` (seconds since the
    epoch, `Option`). `folder_list.rs` reads `ftCreationTime` from the
    record it already has; `item_facts` reads it from the metadata call it
    already makes. No new IO.
  - `SortKey::Created` (saved as `created`), a `SORT_KEYS` row after
    Size, and a `DETAILS_COLUMNS` row, hidden by default; W30 lets the
    user show it.
  - Tests: the conversion (copy `unix_seconds`'s tests); sorting by it.
  - Done 2026-10-29. `Item`, `ItemFacts`, and `FolderEntry` carry
    `created`; a lens's sync refreshes it with the other facts. The
    Details row is hidden through a `shown` flag on `DetailsColumn`, and
    the header, the cells, and the sort arrow read
    `shown_details_columns`, so W30 replaces that one function with the
    space's own list. Sort by ▸ Date created is menu id 74.

- [x] **W27. The lens's path as a tip on its title (app).** **[needs
  human]** After: W9.
  - Hovering a lens's title shows its full path through the existing hover
    tip (`app/info_tip.rs`). An unavailable lens adds "Folder not
    available". After W42, it shows the folder on display. The text
    comes from the model; no disk access.
  - A pure `title_tip(space, unavailable) -> Option<String>`; `None` for
    a normal space. Hovering the title of a normal space shows no tip, as
    today.
  - Tests: the tip for a lens, an unavailable lens, and a normal space.
  - **[needs human]** check: the tip appears after the usual delay and
    leaves with the pointer; it does not cover the title while you are
    renaming.
  - Done 2026-10-30. The pure function is `scene::title_tip(space, shown,
    unavailable)`: it takes the folder the lens displays as an argument,
    because since W42 that is session state (`App::lens_folder`) the
    space cannot know. `PointerState::title_hover` arms the same
    `Timer::InfoTip`; `update_hover` does not arm it while its space is
    being renamed, and `show_info_tip` holds back an already-armed tip for
    the same reason, since F2 starts a rename with no pointer move to
    re-check the hover. `begin_rename` also dismisses a tip already shown
    when F2 starts the rename. The item tip has the same hole on an item
    rename; noted under **Found while working**.

- [x] **W40. The default view for new spaces (config, app).** After:
  W23.
  - `WindowSettings.new_space_view: SpaceView`, defaulting to
    `SpaceView::default()`, so nothing changes until the user changes it.
    `CreateSpace` takes a `view`, as `CreateLens` does since W19. Fields
    on the New spaces page, through W23's shared builder.
  - Tests: the config's missing-field default; a new space gets the
    setting.
  - Done 2026-10-30. The New spaces page's view and sort fields are the
    shared builder's third user, so `ViewIds`, the three pages' id sets,
    and `view_fields` moved to `settings/view_fields.rs`; `pages.rs` had
    reached the 800-line guideline (K's rule, not an `ALLOWED` entry).
    `apply_create_space` takes the rect instead of `at` and `size`, as
    `apply_create_lens` does, to stay under `too_many_arguments`. The
    seed space keeps `SpaceView::default()`, as it keeps its own title
    and size. The C8 row in section V now covers the view.

- [x] **W39. Ctrl+wheel through the views (app).** **[needs human]**
  After: W22.
  - A pure `step_view(mode, steps: i32) -> ViewMode`. It walks
    `VIEW_MODES` in menu order, stops at the ends, and returns
    `ViewMode::Icons` unchanged (D18). `app/zoom.rs` calls it first for a
    space whose mode is not `Icons`, and on a step dispatches
    `SetSpaceView` and shows no pop-up. Ctrl+Plus and Ctrl+Minus step the
    same way; Ctrl+0 sets `Icons`.
  - Tests: steps both ways, the ends, `Icons` unchanged, a big wheel
    delta that is more than one notch.
  - **[needs human]** check: Ctrl+wheel on a space in Large icons walks
    to Content and back and stops; a space on "Settings icon size" zooms
    as before (the F13 row still passes).
  - Done 2026-10-30. `step_view` lives beside `view_for_digit` in
    `settings/tables.rs`. Positive steps walk forward in menu order, so
    zooming in — wheel up, Ctrl+Plus — runs against it, toward the bigger
    views, as Explorer's Ctrl+wheel does; the wheel's notches are negated
    at `adjust_icon_size`. A step goes through `set_space_view`, so it is
    one "Change view" undo entry like the View menu's; an end-stop or a
    partial notch changes nothing and never zooms instead, since a fixed
    view never zooms. The keys read `selection.surface` at handle time,
    and the wheel takes the surface the event named.

- [x] **W28. File name extensions per space (core, app).** **[needs
  human]** After: W20, W26.
  - `SpaceView.extensions: Extensions { Automatic, Show, Hide }`;
    `Automatic` is the default (D18). `App::shows_extensions` (W26) reads
    it first and falls back to today's rule for `Automatic`.
  - View ▸ "File name extensions": a check entry showing what the space
    shows now. Choosing it sets `Show` or `Hide`. The View page offers all
    three in a choice.
  - Rename already selects the name without its extension when one
    shows; check that it still does.
  - Tests: each value under both presets; the menu check follows the
    effective value.
  - **[needs human]** check: Show and Hide on a normal space and a lens,
    Automatic under Match Explorer with Explorer's setting on and off;
    F2 selects only the name part.
  - Done 2026-10-30. `Extensions` lives beside `SpaceView` in core
    `view.rs`, serialized as `automatic` / `show` / `hide` and left out
    when default, like `SortRule`. `space_view_menu` takes the effective
    answer from `App::shows_extensions` so the check needs no system state
    of its own; the menu id is 75 and the View-page field id 152. Rename
    and the scene's labels flow through `shows_extensions`, so both follow
    the choice with no further change.

- [x] **W30. Choose the Details columns (core, app).** **[needs
  human]** After: W29.
  - `SpaceView.columns: Vec<ColumnSpec { id: Column, width:
    Option<u16> }>`, in display order; the default is Name, Date
    modified, Type, Size. `Column` has `#[serde(other)] Unknown`. A pure
    `ColumnSpec::sanitize(list)` runs on load and after every edit: it
    drops unknown and repeated ids, keeps Name first, caps the list at
    `Column::ALL.len()`, and clamps widths to `COLUMN_WIDTH_RANGE`.
  - A right-click on the header opens Explorer's header menu: a check
    entry per column, with Name grey and on. Explorer's "Size all columns
    to fit" needs text measurement, so leave it out and add a **Found
    while working** line.
  - Tests: `sanitize` on hostile lists (unknown ids, Name missing or
    last, repeats, zero and huge widths, a thousand entries); the menu
    checks the columns shown.
  - **[needs human]** check: show and hide each column on a lens and a
    space; the choice survives a restart; Name cannot be hidden.
  - Done 2026-10-31. `SpaceView` holds a `Vec`, so it is no longer `Copy`,
    and `Space::view()` returns a reference. A hostile list cannot fail the
    load: `lenient_columns` drops an entry that makes no sense and treats a
    field that is no list as the default, and a width of any whole number
    reaches `sanitize`. The layout reads a stored width already, ahead of
    W31's drag. Menu ids 76-80. A right press on the header leaves the
    selection, since a row can be scrolled under it. A scratch-config run
    of the real app drove the menu by posted messages: a hostile list
    loaded, the checks and the grey Name were right, and a choice saved.

- [x] **W48. Fit to contents in the views that place by order (core,
  app).** **[needs human]**
  - Why: on a space in Small icons or List, Fit to contents changes
    nothing, because those views keep the space's size. Decided by the
    maintainer, 2026-10-29: fit the content extent.
  - Small icons and the other row-placed views keep the width and set the
    height to fit the rows; List keeps the height and sets the width to
    fit the columns. The result stays within the work area, as for the
    icon views. Compute the size from `ViewLayout`'s extent in a pure
    function.
  - Tests: Small icons with a few items shrinks the height, with many it
    stops at the work area; List grows the width to its columns.
  - **[needs human]** check: Fit to contents on a space in Small icons,
    List, and Details hugs its items.
  - Done 2026-10-31. `FitToContents` carries the `ViewMetrics` the space is
    drawn with, since core cannot know the display's scale, and `fit_size`
    in `view_layout.rs` is the pure function. It sizes once: `arrange_space`
    still leaves an order-placed view's size alone. So the menu and the
    Layout page's button also offer the fit on a space the user has not
    sized (`Space::can_fit`), or a fit could not be repeated after adding
    items. An empty space keeps room for one line, and a folded one fits as
    it would show. A scratch-config run of the real app fitted Small icons,
    Details, and List through the menu: width kept and height shrunk, or
    the reverse for List.

- [x] **W31. Resize the Details columns (app).** **[needs human]**
  After: W30.
  - Dragging a header divider sets that column's `width` (Name keeps the
    rest). The pointer shows the resize cursor over a divider. Each move
    re-lays out the visible rows (arithmetic, see Performance); the
    `SetSpaceView` is committed and saved once, on release. Esc during
    the drag puts the width back.
  - A pure `divider_at(x, columns, scale) -> Option<usize>` with a hit
    slop scaled for DPI.
  - Tests: `divider_at` at each edge and in between; widths clamp.
  - **[needs human]** check: resize each column at 100 % and 150 %;
    smooth on a 50,000-file lens; Esc cancels; a listing that arrives
    mid-drag does not end it.
  - Done 2026-10-31; the maintainer reviewed the plan and chose how a
    divider behaves. Name takes the rest, so widening a column would leave
    its divider behind the pointer. Name is held at its width for the drag
    and takes the rest again on release, so the columns shift flush then;
    the divider right of Name resizes the next column instead. Each move
    updates the model with a bare dispatch, like Move and Resize space, which
    differs from the task's "committed once": one undo entry and one save
    still come on release. Name's width exists only during a drag, and a file
    never brings one back. A scratch-config run of the real app dragged
    dividers by posted messages, with the results read from the saved file.
    One of four runs failed with a stray sort and a wrong width, and three
    clean runs followed; real mouse input over the topmost window is the
    likely cause, not proven.

- [x] **W32. Reorder the Details columns (app).** **[needs human]**
  After: W31.
  - Dragging a header (not its divider) moves the column, with an
    insertion mark between headers. Name stays first (D18). Committed on
    release, as W31.
  - A pure `column_drop_index(x, columns) -> usize`.
  - Tests: drop indices, and that Name never moves.
  - **[needs human]** check: reorder at 100 % and 150 %; Esc cancels; a
    click without a drag still sorts.
  - Done 2026-10-31; the maintainer reviewed the plan. A press on a header
    now waits for its release, so a drag does not sort, which moves a
    header sort from press to release. During a drag only the insertion mark
    changes (`DetailsView.drop_mark`); the columns change once, on release.
    Dragging Name shows no mark and does not sort. A release that changes
    nothing records nothing. A scratch-config run of the real app dragged
    headers by posted messages, with the real pointer checked to be off the
    window: every check passed.

- [x] **W33. Compact view (core, app).** **[needs human]** After: W22.
  - `SpaceView.compact: bool`, off by default. The order-placed views
    (Small icons, List, Details, Tiles, Content) take their row and cell
    padding from one metrics struct in `view_layout.rs`, which has a
    compact variant measured from Explorer's Compact view. The icon
    views are unchanged (D18).
  - View ▸ "Compact view" (a check entry) and a View page toggle.
  - Tests: compact rows are shorter in each order-placed view and the
    same in the icon views.
  - **[needs human]** check: against Explorer's Compact view in each
    view, at 100 % and 150 %.
  - Done 2026-10-31. The maintainer chose provisional sizes over measuring
    Explorer now, so `COMPACT_CELLS` in `view_layout.rs` is not yet "measured
    from Explorer's Compact view": section V carries the comparison. `NORMAL_CELLS`
    holds the old values unchanged, also never measured, so a space not in
    compact looks as before. Menu id 81, field id 153. The toggle is on the
    View page only, not the two defaults pages, as the task asks.

- [x] **W36. Group by: the groups (core, app).** After: W15, W18.
  - Core: `SpaceView.group: Option<GroupBy { key: SortKey, descending
    }>`. `Space::order_is_fixed()` replaces W15's reorder check: it is
    true while the space keeps sorted *or* is grouped, and
    `ReorderItem` reads it (D18).
  - App, pure: `groups(items, group_by, facts) -> Vec<Group { name,
    range }>` over items already sorted by the sort rule, with each
    group's items kept together. Group names follow Explorer, and the
    implementer checks each one on screen:
    - Name: letter ranges (for example "A - H", "I - P", "Q - Z") and
      "0 - 9";
    - Type: the W18 type name;
    - Size: Explorer's buckets, from Empty and Tiny through Gigantic;
    - Date modified: Today, Yesterday, Earlier this week, Last week,
      Earlier this month, Last month, Earlier this year, A long time ago.
      Pass the local date in, never read the clock inside.
  - Tests: each key's groups and order, both directions; date buckets
    across a week, month, and year boundary; an empty space has no
    groups; `order_is_fixed`.

- [x] **W37. Group by: headers and folding (core, app, platform).**
  **[needs human]** After: W36, W17.
  - `view_layout.rs`: a header row before each group, in every view
    (an icon view places grouped items by order, as D18 says). Prefix
    sums and binary search keep a scroll window O(log groups)
    (Performance).
  - Painting: Explorer's header (name, count, a chevron, and a rule to
    the edge).
  - A click on a header folds or unfolds it. The fold state is per space
    in `ViewCache` (session only). Folding drops the group's items from
    the selection; Ctrl+A, the marquee, and the arrow keys skip folded
    items (D18).
  - Tests: layout with folded and open groups; the visible-window lookup
    against a brute-force walk on random group sizes; folding clears the
    selection; Ctrl+A skips folded items.
  - **[needs human]** check: group a 50,000-file lens by Type and scroll
    fast; fold and unfold; Ctrl+A then Delete after folding a group:
    the question counts only visible items.
  - Done 2026-10-31, as one commit, since W36's code has no caller before
    W37; the maintainer reviewed the plan and chose where the order lives.
    The stored item order stays as it is, and the app derives a display order
    (`Grouping`, core; `crate::grouping`, app), cached per space behind a digest
    of what it reads, so ungrouping restores a hand arrangement and undo is not
    involved. Only `ViewLayout` maps between stored index and display position,
    so its readers still speak stored indices. Within a group items keep their
    stored order, which for a lens is its sort rule. A grouped List stacks
    downward as rows. The chevron sits at the left and the rule runs along the
    bottom of the row. Release build, 2026-10-31, 50,000 items, on the UI thread:
    grouping 1.9-4.6 ms by key, and the digest each repaint pays about 1.9 ms
    (SipHash took 4.8 ms, so it is a word-at-a-time hash).

- [x] **W38. Group by: the menu and the settings (app).** After: W37,
  W23.
  - Group by ▸ beside Sort by ▸, as Explorer: (None), then the keys from
    `SORT_KEYS`, then Ascending and Descending. Rows on the View page
    and the Lenses defaults page, through the shared builder.
  - Tests: the menu checks the current grouping; the page fields have
    handlers.
  - Done 2026-10-31. Menu ids 82-89; field ids 154-157. The direction's field
    is grey until a key is chosen. The New spaces page has no group rows, as
    the task lists only the View and Lenses pages, though it shares the
    builder.

- [x] **W49. Picking a lens's own folder keeps its items (core, app).**
  After: W42.
  - Why: Change folder… on the folder a lens already shows (and is not
    browsed below) leaves it empty. `SetLensFolder` clears the items, and
    the watch for that path is already running, so it posts no listing.
  - Fix: `apply_set_lens_folder` keeps the items when the path is the one
    the lens shows, since they belong to that folder. Change folder… on a
    browsed lens still returns home by clearing the subfolder's items.
  - Done when: on an idle lens, picking its own folder changes nothing but
    the title; on a browsed lens, it returns home and lists the folder's
    own items; picking another folder still empties the lens until its
    listing arrives.
  - Done 2026-10-31. `change_lens_folder` returns a browsed lens home
    through `show_subfolder`, so the subfolder's items and selection go as
    they do on Backspace. A core test covers the same-folder pick.

- [x] **W41. Close F-40: the desktop's view modes (docs).** **[needs
  human]** After: W12.
  - Check (maintainer): on Windows 10 and 11, the desktop's View menu
    offers only Large, Medium, and Small icons. In Small icons, are the
    desktop's labels below or beside the icon? Record it beside
    `display_settings.rs`'s `FVM_SMALLICON` branch.
  - If the labels sit below, nothing changes: the default view already
    follows the desktop's size. Close F-40 in ROADMAP. If they sit
    beside, add a task to show a matching space as Small icons under
    Match Explorer.
  - Done when: the finding is recorded and F-40 is closed or has its
    task.
  - Done 2026-10-31. Windows 11 (maintainer): the desktop's View menu lists
    Large icons, Medium icons, and Small icons, plus Auto arrange icons,
    Align icons to grid, and Show desktop icons; it offers no List,
    Details, Tiles, or Content. Small icons keep their labels below the
    icon, so the default view already matches and F-40 closes with no
    view-mode mapping. No Windows 10 machine was available; section V
    keeps that half of the check open.

- [x] **C11. Resize a space to its view's rows and columns (core, app).**
  Decided by the maintainer, 2026-10-31 (ROADMAP D22).
  - Why: `GridSpec::fit_to_grid` rounds every resize to the icon grid, which
    Details, List, Tiles, and Content do not use, so their rows can still be
    cut in half. The height fit ignores the status bar, and the resize dead
    zone is half an icon cell in every view.
  - Fix: `Command::ResizeSpace` carries the space's `ViewMetrics`, as
    `FitToContents` does, so core can fit against the cells the space is
    drawn with. With `fit_to_grid` on, fit the dragged axes to that view's
    cells; Details and Content fit height only (`ViewMode::rows_fill_width`)
    and keep the width at the `snap` step. Include the status bar and
    Details' header in the height fit. Make the dead zone half an item's
    shorter side, never less than half `snap`.
  - Done when: resizing each view lands on whole rows and columns of that
    view; Details and Content leave the width free; a space's status bar is
    never clipped by the height fit; icon views keep today's result;
    `fit_to_grid` off still keeps the dragged size. Tests cover each view
    family, the status bar, and the dead zone. `CONTEXT.md` section 4 and
    the F-03 row say the new rule.
  - Done 2026-10-31. `ViewMetrics::fit_rect` fits the view's cells, and the
    status bar and Details' header are chrome. Details and Content keep the
    `snap`-stepped width. The dead zone is half the shorter real cell side,
    or half the row height in a row view, and never less than half `snap`.
    Tests: the three fit tests in `view_layout.rs`; Details, List, keeping
    the fit in bounds, and the status bar in `command/mod.rs`; and the
    dead-zone tests in `pointer.rs`.

- [x] **C12. Fit to contents on an item's menu (app, core).**
  Decided by the maintainer, 2026-10-31 (ROADMAP D23).
  - Why: the space menu already offers Fit to contents, but an item's
    right-click menu does not; an icon view can also be fitted larger than
    the work area, so part of it clips.
  - Fix: `App::show_item_menu` adds "Fit to contents" for an item in a
    space, enabled from `Space::can_fit`, and fits that space when chosen.
    `apply_fit_to_contents` caps the width and height to `bounds` in every
    view, not only the order-placed ones.
  - Done when: right-clicking an item in a space shows the entry (greyed on
    a locked or already-fitting space), choosing it fits the space and
    records one "Fit to contents" undo entry, a desktop item's menu does not
    show it, and no fitted space exceeds the work area. A test covers the
    cap; the menu itself is on-screen checked.
  - Done 2026-10-31. `show_item_menu` adds the entry for an item in a space,
    enabled from `Space::can_fit`, and chooses the same
    `fit_space_to_contents` path as the space menu, so it is one undo
    entry. `apply_fit_to_contents` caps the width and height to `bounds` in
    every view; an icon view reflows into whole columns and then scrolls. A
    core test covers the cap. The menu's placement, greyed state, and
    desktop absence are in section V.

- [x] **B31. Test only the crates a change affects (scripts).**
  - Why: `cargo test --workspace` runs every crate's tests. A change to
    `deskspace-config` can only break `config` and `app`, the crates that
    depend on it, so while working the rest can be skipped. Cargo has no
    built-in way to do this. `ENGINEERING.md` section 3 already names
    the switch this task adds.
  - `scripts/dev.ps1`: add a `-Changed` switch. Without `-Check` it does
    nothing. With `-Check`, the test step runs `cargo test --locked -p
    <crate> ...` for the affected crates only; fmt and clippy are
    unchanged. Add an example to the script's comment-based help.
  - Which crates are affected. Write each step as its own small
    PowerShell function, so each can be read and tried alone:
    1. **Changed files:** `git diff --name-only main...HEAD`, plus `git
       diff --name-only HEAD` (uncommitted), plus untracked files from
       `git ls-files --others --exclude-standard`. Add a `-Base` parameter
       for the branch to compare with, default `main`.
    2. **Changed crates:** a file under `crates/<dir>/` belongs to the
       package in that folder. Read the package names from `cargo
       metadata --format-version 1 --no-deps --locked` (each package's
       `manifest_path`), never from a list in the script, so a new crate
       needs no edit. A change to `Cargo.toml`, `Cargo.lock`,
       `rust-toolchain.toml`, or anything under `.cargo/` affects every
       crate. Any other file (docs, scripts, workflows) affects none.
    3. **Dependents:** from the same metadata, a crate's dependencies
       are its `dependencies` entries that have a `path`. Repeat until
       nothing new is added: every crate that depends on an affected
       crate is affected too.
    4. If nothing is affected, print "No crate changed; no tests to
       run." and skip the test step.
  - Print the affected crates before testing, so the user sees what was
    skipped.
  - `ENGINEERING.md` section 1: add the switch to the list after `-Check`.
  - Done when: these runs print the expected crates (try each, then
    undo the change):
    - edit a comment in `deskspace-app`: `deskspace-app` only;
    - in `deskspace-config`: `deskspace-config`, `deskspace-app`;
    - in `deskspace-core`: all five;
    - in `docs/TASKS.md` only: none;
    - in `Cargo.toml`: all five.

    No unit tests: the repository has no PowerShell test framework, and
    adding one is out of scope.

- [x] **B33. Load icons off the UI thread (platform, app).** The design
  below was approved by the maintainer, 2026-10-29.
  - Why: `IconCache::icon_for` loads a missing icon synchronously while
    `space_scene` builds a scene, through the shell's image factory. A
    lens on a slow or network folder, or the first scroll through a big
    one, stalls every space until each icon is loaded. Explorer draws a
    generic icon at once and fills the real one in later.
  - Design: `icon_for` returns the cached icon, or a generic one for
    the item's `ItemKind` and queues the path. One worker thread (COM
    initialized as STA, as the image factory needs) loads queued icons
    and posts a `WM_APP` message (next free id) that becomes
    `Event::IconsLoaded { paths }`. The app refreshes only the spaces
    that list those paths. The queue drops paths that scrolled out of
    view before the worker reaches them.
  - Pure parts to test: the queue (dedupes, keeps the newest request per
    path, drops a path that is no longer wanted) and which spaces a
    loaded path refreshes.
  - **[needs human]** check: a lens on a folder of a few thousand images
    scrolls without stalls, and its icons fill in.

- [x] **B38. Check for rewritten files without comparing every pair (app).**
  Found by review, 2026-10-31.
  - Why: `App::take_listing` compared each listed entry with each of the
    lens's items on the UI thread, and an unchanged file never ended the
    search early. Every listing after a lens's first stalled every space: 95
    ms at 1,000 files, 2.4 s at 5,000, and 9.7 s at 10,000 (release build,
    2026-10-31). The timing test for B37 covers only `SyncLens`, so it did
    not show.
  - `rewritten_paths(items, entries)` in `app/lens.rs` indexes the items by
    path once and looks each entry up. Pure, and tested on plain data.
  - Done when: the scan is gone and the gates pass. On 50,000 entries it takes
    21 ms, where the old loop scaled to about 248 s; that is still over the
    Performance rule's 8 ms, and B37 moves the work off the UI thread.

- [x] **S14. Do not offer a download that cannot work (app).** **[needs
  decision]** Found 2026-10-31: `next_step` (`app/space_icons.rs`) never asks
  `deskspace_update::is_configured`. So a build with an empty `UPDATE_PUBKEY`
  still asks "Show icons on spaces?" for a new space with an automatic icon,
  then `fetch_signed` returns `NotConfigured` and no icon shows. Setting the
  key alone does not cure it: the download also fails until S5 has published
  the set. To decide: whether a build without a key skips the question (the
  update controls already grey out this way, and a declined consent must not
  be saved for it), or whether the key and S5 are simply required before any
  release. Tests, if the code changes: `next_step` for a build with no key.
  - Decided (maintainer, 2026-10-31): a build without a key skips the question.

- [x] **S5. Publishing an icon set (ci).** **[needs human]**
  - `.github/workflows/icons.yml`, `workflow_dispatch` with a `prefix`
    input (default `fluent`), on `windows-latest`, copying `release.yml`'s
    toolchain and `cargo install cargo-packager` steps:
    1. `cargo run -p deskspace-config --example icon_set -- spec <prefix>`
       into `$GITHUB_OUTPUT`.
    2. Fail if `gh release view <tag>` finds the release. A published set is
       never replaced, because shipped builds pin its revision.
    3. Download and extract the upstream package, run `trim`, then `git diff
       --exit-code assets/icons`. The commit that bumped the row must
       already carry its names file.
    4. Sign the file with `cargo packager signer sign`, using
       `CARGO_PACKAGER_SIGN_PRIVATE_KEY` from `DESKSPACE_MINISIGN_KEY`. Stop
       with a clear error while the secret is empty.
    5. `gh release create <tag> <file> <file>.sig --prerelease
       --latest=false`, titled "Icon set: `label` r`revision`", with notes
       naming the upstream package and saying it is not an app release.
       It must be a pre-release: the update feed reads
       `releases/latest/download/latest.json`, which a newer full release
       would take over.
  - `RELEASING.md`: a short "Publishing an icon set" section. To add or
    update a set, bump or add its row, run the example's `trim`, commit the
    row and names file, then run the workflow. Never delete an `icons-*`
    release. `actionlint` must pass.
  - **[needs human]** check (maintainer, once `DESKSPACE_MINISIGN_KEY` and
    `UPDATE_PUBKEY` are set): run the workflow for `fluent`. The release is
    a pre-release named `icons-fluent-r1`, with the `.json` and `.sig`. The
    repository's "Latest" badge still points at the app. A second run fails
    at step 2.

- [x] **B37. Refresh a big lens off the UI thread (core, platform,
  app).** The split below was decided by the maintainer, 2026-10-29, and amended 2026-10-31.
  - Why: `Command::SyncLens` filters, reconciles, sorts, and lays out
    every entry on the UI thread. On 50,000 entries in a release build it
    took 44-46 ms with no filters and 62-65 ms with 32 typical filter
    patterns, and 309 ms with 32 patterns built so each is tried in full
    (2026-10-29, `time_a_filtered_sync_of_fifty_thousand_entries` in
    `command/lens_ops.rs`). The Performance rule in section W allows 8 ms.
    Accepted for W34 by the maintainer on that date, with this task to
    follow.
  - Measured again, 2026-10-31: those timings used names already in sorted
    order. In NTFS-like order `sort_items` alone takes about 330 ms on 50,000
    items, against 24 ms sorted, and the whole `SyncLens` 430-490 ms on a
    scattered listing. Moving only the filter and the sort cannot reach 8 ms:
    reconciling ids by path costs about 38 ms (build the map 20, look up 12,
    drop it 6), and `rewritten_paths` (B38) 21 ms. Feed the timing test
    entries in listing order.
  - Design (amended by the maintainer, 2026-10-31): the watch worker, which
    already lists the folder off the UI thread, builds the finished item list.
    It filters, sorts, reconciles ids, and finds the rewritten paths with
    pure functions from `deskspace-core`, using the view's rules passed in
    with the watch (as W25 passes the store). The UI thread only swaps the
    list in, and the old list is dropped off it. A filter or sort change sends
    the new rules to the worker, which redoes the work there.
  - To settle while building, and ask if a choice changes this design:
    - Ids. `Desktop` hands them out and they are never reused (D11), so the
      worker needs a block of reserved ids and its own map from path to id.
    - A list built under old rules, or before an undo restored the lens, must
      not be swapped in. Tag each list with what it was built from.
    - `App::filtered_out` needs to know the listing was not empty, and
      `refilter_lens`'s copy of the listing moves to the worker.
    - `App::sync_grouping` also walks every item on the UI thread for a
      grouped lens. Measure it, and move it with the rest if it is over budget.
  - Measure with the timing test above before and after; record both.
  - Done when: swapping in a 50,000-entry list stays under 8 ms on the UI
    thread with 32 patterns, and a lens of that size refreshes without a
    visible hitch (**[needs human]**).

- [x] **T19. Bounce (core, platform).** **[needs human]**
  - Row: `effect(Effect::Bounce, "bounce", &[Direction])`. Not in Fold.
  - Frame math: the rect moves as Fly's does (from beyond `edge` of
    `work_area`, clipped to it; Automatic is the top, the row's default
    `auto_edge`), but its progress goes through a pure `bounce_ease(t: f32) ->
    f32` instead of Fly's easing: Robert Penner's "easeOutBounce" (four
    parabolic arcs; cite https://easings.net/#easeOutBounce beside it).
    The out is the in played backwards, as for every effect.
  - Tests: `bounce_ease` is 0 at 0 and 1 at 1, stays within 0..=1, and
    touches 1 at least three times (the landings); the frame at progress
    0 is wholly outside the work area on the chosen edge, at 1 it is the
    space's rect, and it never leaves the work area's span on the other
    axis (copy Fly's transition test).
  - **[needs human]** check: Bounce on Startup (show, a new space,
    startup) and Delete from each edge, on Acrylic and Solid at 100 % and
    150 %: it drops in, bounces about three times shrinking, and settles
    exactly in place; it never shows on a second monitor or over the
    taskbar.

- [x] **B22. Mark the process-wide state where it lives.** The platform keeps
  a number of `static`s and `thread_local!`s. Most are needed, because hooks
  and Win32 callbacks get no context pointer. But none says why, which makes
  the runtime the hardest part to reason about. Explain each one in a
  comment beside it, not in a doc table, which would drift (`ENGINEERING.md`
  section 7).
  1. List them: `rg -n "^\s*(pub(\(crate\))? )?static |thread_local!"
     crates/deskspace-platform/src`. Skip `static` string or number
     constants that never change; they are not state.
  2. Above each remaining one, write a comment that starts with
     `// Process state:` and gives, in at most three lines:
     - what it holds;
     - why it cannot be passed instead (for example, "a `WH_KEYBOARD_LL`
       hook procedure gets no user pointer");
     - who sets it and who clears it.

     For a `thread_local!` block, comment each entry.
  3. If you cannot say why one cannot be passed, do not guess. Write
     `// Process state: TODO(B22): no stated reason` and add a line under
     **Found while working**.
  4. In `CONTEXT.md` section 6, at the end of "Runtime", add one bullet:
     "Process-wide state: every `static` or `thread_local!` that holds state
     is marked `// Process state:` with what it holds, why it cannot be
     passed, and who clears it; `rg -n "Process state:"` lists them."
  - Comments only; no code changes. Make one commit for the platform crate.
  - Done when: every match from step 1 that holds state has the comment.

- [x] **B25. Name `App::on_event`'s long arms.** `app.rs` `on_event` is a
  good dispatch `match` except for `Event::Started` (about 40 lines),
  `Event::SystemThemeChanged`, and `Event::CloseRequested`. Move each into
  `on_started`, `on_system_theme_changed`, and `on_close_requested`, in the
  module that owns most of what they touch (`app/view.rs` for the theme,
  `app/persist.rs` or `app.rs` for start and close; follow `CONTEXT.md`
  section 7's runtime-flow steps 3 and 6, which describe them). Pure move.
  Done when: every arm of `on_event` is one call.

- [x] **B24. Split `App::on_space_event`.** `app/pointer.rs`
  `on_space_event` is about 135 lines: a preamble (the surface still exists,
  commit a rename, act like a desktop click) and a `match` over
  `SpaceEvent` whose arms run up to 25 lines.
  - Move the preamble into `fn prepare_space_event(&mut self, ui, surface,
    &event) -> bool` (false means ignore), and each arm longer than about
    five lines into a named method in the module that owns it (`pointer.rs`
    for mouse arms; the drop arms call into `files.rs`, where the logic
    already lives).
  - Pure move: no behavior change, no new `pub(super)` beyond what the moved
    methods need. Done when: `on_space_event` is under about 50 lines.

- [x] **B13. One child-control helper.** `win32/dialog.rs` `control` and
  `win32/settings_window/build.rs` `Builder::control` are near-copies, both
  allowed `too_many_arguments` (8).
  - Add `pub(crate) struct ControlSpec<'a> { class: PCWSTR, text: &'a str,
    style: u32, ex_style: WINDOW_EX_STYLE, bounds: (i32, i32, i32, i32), id:
    usize }` in `dialog.rs`; `control(chrome, parent, spec)` creates the
    window and sets the font. `Builder::control` calls it with its own font.
  - Done when: both allows are gone and **Found while working** no longer
    lists `too_many_arguments`. **[needs human]** check: the settings window
    and the custom shortcut dialog look as before.

- [x] **B14. Split `Builder::field` by field kind.**
  `win32/settings_window/build.rs` `Builder::field` is about 260 lines: one
  `match` over `FieldKind` (Heading, Note, Choice, Toggle, Slider, Color,
  Text, Button, SpacePreview) with layout literals (`self.s(184)`,
  `self.s(12)`, `self.s(28)`, `self.s(16)`).
  - Turn each arm into a `Builder` method (`heading_field`, `choice_field`,
    ...) returning its height; `field` keeps the shared setup and the
    dispatch.
  - Name the literals as `const`s at the top of `build.rs` (`LABEL_WIDTH`,
    `LABEL_GAP`, `ROW_HEIGHT`, `EDGE`, ...) in 96-DPI pixels, scaled through
    `self.s`. Same values, so the layout does not move.
  - Done when: no method in `build.rs` is over about 80 lines.
    **[needs human]** check: every settings page looks as before at 100 %
    and 150 %.

- [x] **B26. Split `settings_window::build` and `update`.**
  `win32/settings_window/build.rs` `build` (about 210 lines) and
  `update.rs` `update` (about 125 lines) each build or diff the panes, the
  header, and the pages in one body. Split each into `*_panes`, `*_header`,
  and `*_pages` helpers in the same file, in the order the body already runs
  them. Pure move. Done when: no function in either file is over about 80
  lines. **[needs human]** check: the settings window opens, switches scopes
  and pages, and previews as before.

- [x] **Q2. "Selection, display matching, keys, and clipboard"**
  (`win32/shell_selection.rs`, `win32/display_settings.rs`, `edit.rs`,
  `win32/keyboard.rs`, `win32/clipboard.rs`, `win32/file_op.rs`). The
  ENGINEERING.md Keys table stays where it is.

- [x] **Q5. "Drops" and "Drag out and the drag image"**
  (`win32/drop_target.rs`, `shell_items.rs`, `drag_image.rs`,
  `drag_ghost.rs`).

- [x] **Q6. "Rendering" and "Controller"** (`render/`, `win32/controller.rs`,
  `tray.rs`, `console.rs`, `outside_click.rs`, `desktop_entry.rs`).

- [x] **Q4. "Desktop shell, shell items, DPI, hotkeys"**
  (`win32/folder_watch.rs`, `desktop_shell.rs`, `desktop_icons.rs`,
  `watchdog.rs`, `desktop_items.rs`, `icons.rs`, `shell_items.rs`, `dpi.rs`,
  `system.rs`, `info_tip.rs`, `hotkey.rs`).
