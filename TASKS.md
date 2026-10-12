# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

Ticked tasks are kept for the record. Their file and line pointers describe
the code when they landed, often before `app.rs` was split into `app/`
modules (R1); use the file map below, not those pointers, to find code today.
Unticked tasks were re-checked against the code on 2026-10-11.

**Pick a task:** start with section A (comment and small correctness fixes),
then B (cleanups) and K (file splits). These are the safest and make every
later task easier. Tasks marked **[needs decision]** or in section D are not
ready; skip them.

## How to work a task

1. Read `AGENTS.md` once (especially "Design priorities"), then only the
   `CONTEXT.md` sections a task names. Use the file map below to find code.
2. Do one task per commit. Do not combine tasks or fix "nearby" things; note
   them under **Found while working** at the end of this file instead.
3. Line numbers are approximate and drift. Find code by the function or type
   name given (search for `fn name`); the line is only a hint.
4. If the code does not match the task's description, or the task needs a
   design decision it does not state, **stop and ask**. Do not improvise.
5. Copy the pattern the task points at. Prefer the recipes below over
   inventing a new structure.
6. Leave the code more readable than you found it, within the task's scope:
   descriptive names, small functions, named constants, and a `///` doc on any
   new item whose behavior is not obvious. Do not add a feature by special-
   casing an existing function; if no extension point fits, stop and ask.
7. Done means all of:
   - `cargo fmt --all`
   - `cargo clippy --workspace --all-targets -- -D warnings` (no new
     `#[allow]`)
   - `cargo test --workspace`, with new tests for new logic; then update the
     test count in `CONTEXT.md` section 2
   - `CONTEXT.md` updated where the task changes behavior, and the
     `ROADMAP.md` feature index (section 7) updated if a feature's status
     changed
   - the task's own "Done when" list
8. Commit with the repo's style: `feat: ...`, `fix: ...`, `refactor: ...`,
   `docs: ...`, `test: ...`, `perf: ...`, `ci: ...`, imperative and
   lowercase, with the task id in parentheses (`refactor: split d2d.rs (K2)`).
   Tick the task's box here in the same commit.
9. Tasks marked **[needs human]** need an on-screen check you cannot do.
   Implement, then list exactly what to check in the commit body and leave the
   box unticked with "(awaiting check)".
10. Tasks in section D are **not** for routine work. Skip them.

## File map

Where each kind of code lives. Paths are under `crates/`.

| To change... | Look in |
|--------------|---------|
| The model (spaces, items, the desktop layer) | `deskspace-core/src/{space,item,desktop,desktop_layer}.rs` |
| A model mutation | `deskspace-core/src/command.rs` (`Command`, `Desktop::apply_with_layout`) |
| Grid, sizes, snapping | `deskspace-core/src/layout.rs` (`GridSpec`, `arrange_space`) |
| A look setting or color | `deskspace-core/src/theme.rs` (`Theme`, `Palette`, `*_RANGE`) |
| A non-look setting, config load/migrate | `deskspace-config/src/schema.rs` (`WindowSettings`, `Config::migrate`) |
| Config file paths and atomic save | `deskspace-config/src/store.rs` |
| CLI flags | `deskspace-app/src/cli.rs` |
| Startup before the window loop | `deskspace-app/src/bootstrap.rs` |
| Handling an `Event`; timer and menu ids; preset tables | `deskspace-app/src/app.rs` (`on_event`, `mod menu`, `*_PRESETS`) |
| Saving, dispatching a `Command` | `deskspace-app/src/app/persist.rs` |
| What a space looks like, refresh/re-flow, takeover | `deskspace-app/src/app/view.rs` |
| Surface (space or desktop layer) lookups | `deskspace-app/src/app/hit.rs` |
| Mouse, drag, resize, hover | `deskspace-app/src/app/pointer.rs` |
| Selection, arrow keys, type-to-select | `deskspace-app/src/app/selection.rs` |
| Inline rename | `deskspace-app/src/app/rename.rs` |
| Drops, delete, prune | `deskspace-app/src/app/files.rs` |
| Cut / copy / paste | `deskspace-app/src/app/clipboard.rs` |
| Space, item, and tray menus | `deskspace-app/src/app/menus.rs` |
| Settings window behavior (open, preview, Apply, Cancel) | `deskspace-app/src/app/settings_session.rs` |
| Settings pages, fields, `field::` ids (pure) | `deskspace-app/src/settings.rs` |
| Show/hide, desktop verb, start with Windows | `deskspace-app/src/app/visibility.rs` |
| Model → `SpaceScene` (pure) | `deskspace-app/src/scene.rs` |
| Managed store for dropped files | `deskspace-app/src/storage.rs` |
| Events the platform raises | `deskspace-platform/src/input.rs` |
| Menu and settings-form data types | `deskspace-platform/src/ui.rs` |
| `Ui` methods, the message loop | `deskspace-platform/src/win32/runtime.rs` |
| Controller window (tray, hotkeys, timers, `WM_APP` dispatch) | `deskspace-platform/src/win32/controller.rs` |
| Keys (spaces are never focused) | `deskspace-platform/src/win32/keyboard.rs` (`fn handle`) |
| Space window procedure, DWM | `deskspace-platform/src/win32/{space_window,dwm}.rs` |
| Drawing | `deskspace-platform/src/render/{scene,d2d}.rs` |
| Settings window controls | `deskspace-platform/src/win32/settings_window.rs` |
| Shell: open, item menu, drag out | `deskspace-platform/src/win32/shell_items.rs` |
| Desktop enumeration, icons | `deskspace-platform/src/shell.rs` |
| Takeover: hide icons, watchdog | `deskspace-platform/src/win32/{desktop_icons,watchdog}.rs` |
| CI and release | `.github/workflows/{ci,release}.yml`, `crates/deskspace-app/build.rs` |

Ids (`WM_APP` messages, timers, menu ids, `field::` ids) and their next free
values are tabled in `CONTEXT.md` section 6, "Message, timer, and id tables".
Never renumber an id; take the next free one and update that table.

### Recipe: a new global setting

The pattern used by `Theme.icon_size` / `Theme.overlay_size`:

1. `deskspace-core/src/theme.rs` (or `deskspace-config/src/schema.rs` for a
   `WindowSettings` field): add the field with a `///` doc, give it a value in
   `Default`, and rely on the struct-level `#[serde(default)]`. If it is a
   range, add `pub const NAME_RANGE`/`NAME_DEFAULT` like `OVERLAY_SIZE_RANGE`,
   and clamp it in `Theme::clamped` so a hand-edited file loads in range.
2. `deskspace-app/src/settings.rs`: add a `field::` id (the next free number;
   see the id tables) and a `Field` on the right page: a look setting goes in
   `look_pages` (fed the theme being edited, which is a space's own while it
   is customized), a Global-only one in `general_page` (or the page it
   belongs on). Then add the match arm that clamps and returns the right
   `Effect` (`None`, `Appearance` to restyle, `Layout` to re-flow, `Pending`
   for apply-on-OK): in `apply_look_field` for a look setting, which makes it
   per space with no further work, or in `apply_field`'s Global-only arms
   otherwise.
3. If Cancel must undo a re-flow or restyle, extend `settings::needs_relayout`
   or `needs_restyle` (`App::cancel_settings` in `app/settings_session.rs`
   calls them).
4. If Apply must do something outside the config (registry, hotkeys,
   takeover), add it to `App::apply_settings` (`app/settings_session.rs`)
   next to `sync_start_with_windows`.
5. Tests: a serde round-trip / missing-field default test in core or config,
   and an `apply_field` test in `settings.rs` (copy
   `start_with_windows_is_pending`).

### Recipe: a new model change (`Command`)

1. `deskspace-core/src/command.rs`: add a variant with a `///` doc. Add its
   arm in `apply_with_layout`; return `NotFound` for a missing id,
   `Rejected` when the change is not allowed (for example a locked space),
   else `Applied`. Keep the arm short: put real logic in a method on
   `Space`/`Desktop` or a `layout.rs` function.
2. Tests in `command.rs`: one per outcome (copy the `Reflow` tests).
3. App: call it through `self.dispatch(...)` or, for a change to one space
   that should repaint and save, `self.apply_to_space(ui, id, command, at)`
   (`app/persist.rs`). Never mutate `self.config.desktop` directly.
4. Add the variant to the Commands list in `CONTEXT.md` section 4.

### Recipe: a new key

1. `deskspace-platform/src/input.rs`: add a `SelectionKey` variant (no data)
   with a `///` doc. Only a key that carries data gets its own `Event`
   variant and `WM_APP` id.
2. `win32/keyboard.rs`: add a `SEL_*` code, then a branch in `fn handle`
   under the right flag (see the AGENTS.md Gotchas table) that posts
   `SELECTION_KEY` with it. Map the code to the variant where `SELECTION_KEY`
   is dispatched in `win32/controller.rs`.
3. `deskspace-app/src/app.rs`: handle the variant in `App::on_selection_key`,
   calling a method in the `app/` module that owns the behavior.
4. Update the key lists in `AGENTS.md` (Gotchas), `CONTEXT.md` section 8,
   and `README.md` Controls.

### Recipe: a new menu entry

1. `deskspace-app/src/app.rs`: add an id to `mod menu` (next free; see the id
   tables) and to the `other` list in the `menu_ids_are_unique` test. If the
   entry is one of a family (opacity, icon size, sort), add a row to the
   preset table instead.
2. `deskspace-app/src/app/menus.rs`: add a `MenuEntry` in the builder
   (`show_space_menu`, `show_tray_menu`, `show_item_menu`, or the submenu
   helper) and an arm that handles the chosen id. Entries shared by the tray
   and the space menu go through `run_common_menu`.
3. Update the menu lists in `CONTEXT.md` section 8 and `README.md` Controls.

### Recipe: a new visual in a space

1. `deskspace-platform/src/render/scene.rs`: add a `SpaceScene` field with a
   `///` doc and a neutral default (copy `highlight`).
2. `deskspace-app/src/scene.rs`: fill it in `space_scene` (and
   `desktop_layer_scene` if the layer needs it). Add a scene test (copy
   `the_settings_highlight_reaches_the_scene`).
3. `render/d2d.rs`: draw it in `Painter`'s paint path. Do not allocate there;
   pre-compute in `PreparedScene` if needed.

### Recipe: behavior on both spaces and the desktop layer

Take a `Surface`, not a `SpaceId`, and read through the `surface_*`
accessors in `app/hit.rs` (`surface_items`, `surface_item_at`,
`surface_layout`, `surface_palette`, ...). If an accessor is missing, add it
there for both variants rather than matching on `Surface` at the call site.

### Per-space settings

There is no per-space override recipe any more. A space's look is a whole
copy of the look settings (`SpaceLook`: the `Theme` plus grid spacing), so
every look setting added through the recipe above is per space too. A look
setting outside `Theme` and `GridSpec` would go into `SpaceLook` (with
`from_global`, `clamped`, and `grid` kept in step). The app reads a space's
values through `theme_for(id)`, `grid_for(id)`, and `palette_for(id)`, never
`config.theme` directly.

---

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

- [ ] **A11. Say "spaces", not "spaces", in `scripts/dev.ps1`.**
  - The help text and parameter comments still say "spaces" (examples at
    ~17 and ~21, parameters at ~39 and ~43). Replace with "spaces". Comments
    and help text only; no behavior change.

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

- [ ] **B8. Remove the settings window's unused tab layout.**
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

- [ ] **B9. Remove stale `too_many_arguments` allows.**
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

- [ ] **C6. More item presets, and preset import/export (F-48).** Later; needs
  design first.
  - The "Item preset" dropdown is the seed of a preset system: add named
    presets (compact and spacious variants, for example) that set several item
    settings at once, and import/export (save/open) a preset as a file so it
    can be shared. "Match Explorer's display" and "Custom" are the first two.
  - Decide whether a preset snapshots a whole `Theme`/`SpaceLook` or only the
    item fields, how it interacts with per-space looks, and the file format.
    Keep `Preset` forward-compatible (`#[serde(default)]`).
  - The preset grows to cover the remaining Explorer-equivalent item settings
    as they land (hidden/protected files once F-22 exists).

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

## D. Reserved (strong model or maintainer; do not start)

Listed so they are not attempted piecemeal. Each needs design work first.
Ids are `R1`-`R13` so they do not collide with the ROADMAP decision ids
(`D1`-`D9`).

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
- **R3. Platform module layout.** The mechanical splits are now ready-to-do
  tasks in section K (`shell.rs`, `settings_window.rs`, `render/d2d.rs`,
  `runtime.rs`). What remains needs design:
  - one icon decoder instead of two: `shell::decode_icon_masked` (GDI
    `GetDIBits`, for HICONs) and `d2d::decode_bitmap` (WIC, for HBITMAPs);
  - a `win32/messages.rs` owning every `WM_APP + n` (today in
    `controller.rs`, `keyboard.rs`, and `outside_click.rs`; see the table in
    `CONTEXT.md` section 6), with a test that the values are unique;
  - generic helpers for `GWLP_USERDATA` window state (copies in
    `settings_window.rs` and `space_window.rs`) and for window-class
    registration (`controller.rs`, `drag_ghost.rs`, `settings_window.rs`,
    `space_window.rs`);
  - make internal modules private: `win32/mod.rs` has 12 `pub mod`s, though
    the app uses only the `lib.rs` re-exports.
- **R4. Keep IO out of core.** `deskspace-core/src/item.rs` reads the file
  system: `ItemKind::classify` (`path.is_dir()`), `read_metadata`
  (`std::fs::metadata`), and `file_flags` (Windows attributes), all from
  `Item::new`, which `Command::AddItem { space, path }` calls inside core.
  Move the reads to the app or platform, add an `ItemFacts { kind, size,
  modified, compressed, encrypted }` (name to be decided) that the caller
  fills, and pass it into `AddItem` and `Item::new`. Callers:
  `app/files.rs` (`add_dropped`) and `bootstrap.rs` (seeding). Needs a
  decision on where the reads live (the app's `storage.rs` is a candidate).
- **R5. M2 desktop takeover design.** Resolved: ROADMAP D7 (model) and D8
  (placement and safety). The work is I1-I8.
- **R6. Undo design.** Needs an answer to ROADMAP open question 3.
- **R7. On-screen verification pass** (maintainer). The list is in
  `CONTEXT.md` section 2, "Pending on-screen checks".
- **R8. Auto-update design.** Resolved: ROADMAP D6. The work is E5 and
  E8-E11.
- **R9. Installer design.** Resolved: ROADMAP D6. The work is E5-E7.
- **R10. Code signing (maintainer, ROADMAP M8 item 3, D6).** Apply to
  SignPath Foundation (the project must meet their OSS terms: an OSI
  license, which Apache-2.0 is, and a public repository and build). Then add
  their GitHub Actions integration to `release.yml`: sign `deskspace.exe`
  before both the zip and `vpk pack`, and sign the Velopack `Setup.exe`
  (see `vpk pack -h` for the signing options). Store the credentials as
  Actions secrets. This must land before or with the first release that has
  E9 (auto-update), because unsigned updates trigger SmartScreen on every
  version.
- **R11. Windows 11 top-level context menu (ROADMAP M8 item 4).** Needs R10,
  and M2 may make it unnecessary.
- **R12. First release dry run (maintainer).** After E1-E6. Follow
  `RELEASING.md` with a pre-release version (for example `0.2.0-alpha.1`).
  Check:
  - the zip contains `deskspace-watchdog.exe` beside `deskspace.exe` (E12);
  - the workflow publishes a pre-release with the zip, the `.sha256`, and
    the Velopack assets (`DeskSpace.App-win-Setup.exe`, the full `.nupkg`,
    `releases.win.json`);
  - on a clean Windows 10 VM and a clean Windows 11 VM: the extracted exe
    runs, shows its icon, and lists the version under Properties > Details;
    `Setup.exe` installs into `%LocalAppData%\DeskSpace.App` with no admin
    prompt and creates a Start menu shortcut;
  - after E7: uninstalling removes the install root, the Run value, and the
    desktop verb, and leaves the storage folder and the config untouched;
  - after E9: install `alpha.1`, publish `alpha.2`, and confirm that the
    manual tray check offers it and the update restarts into `alpha.2` with
    the same spaces;
  - what SmartScreen shows.

  Record the findings in `CONTEXT.md` section 10.
- **R13. winget (maintainer, ROADMAP M8 item 8).** After the first stable
  (non-pre-release) version that has the installer.
  - Submit the first manifest to `microsoft/winget-pkgs` by hand (`wingetcreate
    new` with the `Setup.exe` URL), with package identifier
    `generalchad.DeskSpace`.
  - Then add a `release.yml` job that runs only for non-pre-release tags and
    updates the manifest (`wingetcreate update --submit`, or the
    `winget-releaser` action), using a fine-grained PAT stored as an Actions
    secret.

## E. Release engineering (ROADMAP M8)

E1-E2 have landed (awaiting check). Do E12 first (it is small and fixes a
shipped gap), then E13, then E3-E4 in order. E5-E11 have their own dependencies,
stated in each task. Code signing (R10), the first dry run (R12), and winget
(R13) are maintainer work.

`.github/workflows/release.yml` runs on a `v*` tag. Its steps, in order:
checkout; `dtolnay/rust-toolchain@stable` (E13 replaces it); "Check tag matches workspace
version" (compares `v<version>` of package `deskspace-app` from `cargo
metadata` with `GITHUB_REF_NAME`); "Test" (`cargo test --workspace
--locked`); "Build" (`cargo build --release --locked -p deskspace-app`, which
builds both `deskspace.exe` and `deskspace-watchdog.exe`); "Package" (stages
`deskspace.exe`, `LICENSE`, `NOTICE`, `README.md` into
`deskspace-<tag>-x86_64-pc-windows-msvc/`, zips it, writes a lowercase
SHA-256); "Publish" (`gh release create <tag> <zip> <zip>.sha256 --title
"DeskSpace <tag>" --generate-notes`, plus `--prerelease` when the tag
contains `-`). Read it before starting any E task.

- [ ] **E12. Ship the watchdog in the release zip.**
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

- [ ] **E13. Pin the Rust toolchain (ROADMAP D9).** Before E3.
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
  - Do not change `rust-version = "1.85"` in `Cargo.toml`; it is the
    minimum supported version, a separate fact.
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

- [ ] **E3. Reproducible release build (ROADMAP M8 item 5).** After E13,
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

- [ ] **E4. Release checklist and versioning policy (ROADMAP M8 item 5).**
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
say where things go. The Velopack Rust docs are thin. Confirm every API
named below on docs.rs for the pinned crate version and at
https://docs.velopack.io; if one differs, **stop and ask**.

- [ ] **E5. Add the `deskspace-update` crate and the Velopack startup hook.**
  - New library crate `crates/deskspace-update` with
    `#![forbid(unsafe_code)]`, `thiserror` errors, and `tracing`. It depends
    on `velopack` (`rust_cargo-add`) and on nothing in the workspace. Add it
    to `[workspace.dependencies]`, and make `deskspace-app` depend on it.
  - `pub const PACK_ID: &str = "DeskSpace.App";`, documented as "must match
    `--packId` in `release.yml`; never `DeskSpace`, see ROADMAP D6".
  - `pub fn startup()` wraps `velopack::VelopackApp::build().run()`. Call it
    as the **first** statement of `main` in
    `crates/deskspace-app/src/main.rs`, before `Cli::parse()`. Velopack
    starts the exe with its own arguments during install, update, and
    uninstall, and may exit the process; clap would reject those arguments.
    Add a comment saying so. Do not call it from `watchdog_main.rs` (the
    `deskspace-watchdog` binary): only the main exe is Velopack's.
  - `pub enum InstallKind { Installed, Portable }` and `pub fn
    install_kind() -> InstallKind`: `Installed` when
    `velopack::UpdateManager::new(...)` succeeds for this process, else
    `Portable`. `cargo run` and the zip build are `Portable`. Log the kind at
    `debug` on startup.
  - Update `AGENTS.md` (workspace layout, dependency direction
    `app -> {config, platform, update} -> core`; `update` depends on no
    workspace crate) and `CONTEXT.md` section 3.
  - If `cargo deny check` rejects `velopack` or anything it pulls in,
    **stop and ask**; do not edit `deny.toml` on your own. Expect this:
    `deny.toml` allows only `Apache-2.0`, `MIT`, `MPL-2.0`, and
    `Unicode-3.0`, and an HTTP/TLS stack often needs more (ISC,
    BSD-3-Clause, ...). Run `cargo deny check licenses` right after
    `cargo add` and report the result before writing any code.
  - `members = ["crates/*"]` picks the crate up; add
    `deskspace-update = { path = "crates/deskspace-update" }` to
    `[workspace.dependencies]` in the root `Cargo.toml`.
  - Done when: `cargo run -p deskspace-app -- --no-icons` still starts
    normally, and a test asserts `install_kind()` is `Portable` under
    `cargo test`.

- [ ] **E6. Package with Velopack in `release.yml`.** After E1 and E5.
  - After the Build step: `actions/setup-dotnet` (8.x), then `dotnet tool
    install -g vpk --version <the velopack crate version in Cargo.lock>`.
    The tool and the crate versions must match; read the crate version from
    `cargo metadata` in the step instead of hard-coding it.
  - `vpk download github --repoUrl <repo> --token ${{ github.token }}
    --outputDir releases`, so `vpk pack` can build deltas. It must not fail
    the job when there is no previous Velopack release (the first one).
  - Stage `deskspace.exe`, `deskspace-watchdog.exe` (the I6 takeover
    helper), `LICENSE`, and `NOTICE` into a directory and run
    `vpk pack --packId DeskSpace.App --packVersion <version> --packDir
    <stage> --mainExe deskspace.exe --packTitle DeskSpace --packAuthors
    "Timothy W. Brown" --icon assets/deskspace.ico --noPortable --outputDir
    releases`. `--noPortable` is required: D6 says the portable build is our
    own zip, which only notifies and never self-updates.
  - Keep the existing `gh release create` (zip and `.sha256`), then add the
    Velopack assets to that same release with `vpk upload github --repoUrl
    <repo> --token ${{ github.token }} --tag <tag> --merge`, plus `--pre` for
    pre-release tags.
  - Check every flag against `vpk pack -h` / `vpk upload github -h` for the
    pinned version, and **stop and ask** if one is missing or renamed.
  - Update `RELEASING.md` (the asset list and the pack id warning) and the
    `CONTEXT.md` section 10 note.
  - Done when: the workflow passes `actionlint` (or a careful read if it is
    not installed). The real check is R12. **[needs human]**

- [ ] **E7. Uninstall cleanup and an install-root safety test.** After E5;
  the Run-key part needs C4.
  - Find the argument Velopack passes the main exe on uninstall (the
    "hooks" / fast-callback docs and the crate's `app.rs`; the C# SDK calls
    it `--veloapp-uninstall`). In `main.rs`, before `startup()`, if that
    argument is present: call `deskspace_platform::set_desktop_show_entry(
    false, "", "", "")`, `deskspace_platform::set_start_with_windows(false)`
    (C4 has landed), and `deskspace_platform::restore_desktop_icons()` (so an
    uninstall during takeover never leaves the desktop blank; ROADMAP M8
    item 1), log any failures at `warn`, and return. Do not touch the config or the
    storage folder (D6). If the SDK exposes a proper uninstall hook instead,
    use that. If neither exists, **stop and ask**.
  - Put the argument check in a pure, tested function (`fn
    is_uninstall_hook(args: &[String]) -> bool`).
  - Safety test in `deskspace-app` (it sees both `config` and `update`):
    using `ConfigStore::discover()`, assert that neither the config path nor
    the storage path is inside `%LOCALAPPDATA%\<PACK_ID>`. Add a pure helper
    `fn is_inside(path, root) -> bool` with its own tests (case-insensitive,
    and `DeskSpace.App` is not inside `DeskSpace`).
  - Done when: both tests pass. **[needs human]** check during R12: uninstall
    removes the Run value and the verb and leaves `%LOCALAPPDATA%\DeskSpace`
    intact.

- [ ] **E8. Cross-thread wake-up for the app (platform).** Independent of
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

- [ ] **E9. A three-button question dialog (platform).** Independent.
  - `Ui::choose(&mut self, title: &str, text: &str, buttons: &[&str],
    default: usize) -> Option<usize>`: a modal `TaskDialogIndirect` owned by
    the same window as `Ui::confirm` (the settings window when it is open,
    else the controller), with custom buttons (`TDF_USE_COMMAND_LINKS` off).
    It returns the index of the button pressed, or `None` if the dialog was
    closed. The Common Controls v6 manifest in `build.rs` makes
    `TaskDialogIndirect` available.
  - Follow `Ui::confirm` (`runtime.rs` ~726) for ownership, foreground
    handling, and not holding a runtime borrow across the modal call: it
    reads the owner inside `with_runtime`, releases the borrow, then calls
    `SetForegroundWindow` and the modal API. `Win32_UI_Controls` is already a
    `windows` feature of the platform crate.
  - Link the `TaskDialogIndirect` page on Microsoft Learn beside the call.
  - Done when: it compiles and clippy is clean. **[needs human]** check: a
    temporary call shows three buttons and returns the right index (remove
    the temporary call before committing).

- [ ] **E10. Update settings, schedule, check, and prompt (F-45).** After
  E5, E8, and E9.
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
      UpdateError>`. Installed: `UpdateManager` with
      `sources::GithubSource::new(env!("CARGO_PKG_REPOSITORY"), None,
      prerelease)`, then `check_for_updates()`. Portable: query
      `https://api.github.com/repos/<owner>/<repo>/releases` with the HTTP
      client Velopack already pulls in (`ureq`, same major version, so there
      is no second copy in `Cargo.lock`), and pick the newest release
      newer than `CARGO_PKG_VERSION`.
    - Keep the choice in pure, tested functions: `fn newest(releases:
      &[Release], current: &Version, prerelease: bool) -> Option<&Release>`,
      parsed from fixture JSON in the test.
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
        url: &str)` in `runtime.rs` with a plain `ShellExecuteW` "open" in
        `win32/shell_items.rs`, keeping the `AllowSetForegroundWindow` step
        `open` already does.
      - "Update and restart" calls the E11 entry point. Until E11 lands,
        open the release page there and leave a `// TODO(deskspace): E11`.
  - Done when: the tests above pass and nothing contacts the network with
    the default config (check by reading the code path, then note it in the
    commit body). **[needs human]** check: a manual tray check on a portable
    build against a repo with a newer pre-release, with "Include
    pre-releases" on.

- [ ] **E11. Download, apply, and restart (installed builds).** After E10.
  - `deskspace-update`: `pub fn download(&Pending) -> Result<(), _>` (call
    it on a worker thread) and `pub fn apply_and_restart(&Pending) -> !`
    (or a `Result`, if the SDK returns one), wrapping `download_updates` and
    `apply_updates_and_restart`. `Pending` holds the Velopack `UpdateInfo`
    from the E10 check, so the app does not check twice.
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
      teardown (`runtime.rs`, end of `run`) that normally does both;
    - call `apply_and_restart`.
    Put the first three steps (flush, hotkeys, verb) in one `fn
    release_for_exit(&mut self, ui)` in `app/persist.rs` and call it from the
    `CloseRequested` arm too, so the two exit paths cannot drift. Keep the
    icon restore and the watchdog signal in the update path only; on a normal
    close the platform teardown does them.
    On failure, restore the tooltip and report the error with `ui.choose`.
  - Keep Velopack's default `set_auto_apply_on_startup(true)`: an update that
    finished downloading but did not restart is applied on the next launch,
    which matches D6 ("apply on restart").
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

- [ ] **F8. Shift and Caps Lock decode predictably in inline rename.**
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
- [ ] **F10. "Fit to contents" in the space settings.** (awaiting check) The
  space menu already offered "Fit to contents" (user-sized, unlocked spaces);
  the settings window now offers the same as a button on a space's Layout page.
  It is grey unless the space is resized and unlocked, and runs
  `Command::FitToContents` (immediate, like the menu; Cancel does not undo it).
  **[needs human]** check: select a resized space in Settings, open Layout,
  press "Fit to contents", and confirm the space grows to fit its items with
  even padding.
- [ ] **F11. A dead zone before a border drag resizes a space.** (awaiting
  check) A press on a border started a resize at once, so a click or tiny
  jitter changed the size (and a corner grab changed both axes). The resize now
  waits until the pointer leaves a half-cell dead zone around the grab
  (`pointer::resize_edges_now`), and each axis joins on its own, so a
  horizontal move on a corner resizes only the width. **[needs human]** check:
  click a border without moving (no resize); drag a side past half a cell (only
  that axis resizes); drag a corner horizontally (only the width changes).
- [ ] **F12. Check box coloring matches Windows.** (awaiting check) A checked
  item check box was a white box with a black tick. It is now filled with the
  Windows accent and shows a white tick, like Explorer: `Palette.check_fill`
  (default `theme::CHECK_ACCENT`, Windows blue) is set to the system accent in
  `surface_palette` and carried on `SpaceScene.check_fill`; the unchecked box
  keeps its light fill and border. **[needs human]** check: tick a desktop item
  in a space and compare the box to Explorer's; change the Windows accent and
  confirm the box follows it.

- [ ] **F13. Ctrl+wheel under "Match Explorer's display" drives the
  desktop's icon size (ROADMAP D9).**
  - Today `App::adjust_icon_size` (`app/pointer.rs`) changes the global
    `Theme.icon_size`, but under the default `MatchExplorer` preset
    Explorer's icon size replaces it (`App::icon_scale` / `space_scale`,
    `app/view.rs`), so Ctrl+wheel visibly does nothing. D9 decides it should
    change the desktop's own icon size, as Ctrl+wheel on the desktop does.
  - Platform, `crates/deskspace-platform/src/win32/display_settings.rs`:
    1. Add `pub const DESKTOP_ICON_RANGE: (i32, i32) = (16, 256);` and
       `pub const DESKTOP_ICON_STEP: i32 = 8;` with a `///` doc each (pixels;
       16 is the smallest icon view, 256 the jumbo size).
    2. Add a pure, tested `pub fn next_desktop_icon_size(current: i32,
       notches: i32) -> i32`: `current + notches * DESKTOP_ICON_STEP`,
       clamped to the range. Tests: one notch up, one down, both clamps.
    3. Add `pub fn set_desktop_icon_size(pixels: i32) -> bool`. It gets the
       view with `shell_view::desktop_shell_view()`, casts it to
       `IFolderView2`, reads the current mode with `GetViewModeAndIconSize`,
       and calls `SetViewModeAndIconSize(mode, pixels)`, keeping the mode.
       Return `false` (with a `tracing::debug!`) on any failure. Copy the
       shape and `// SAFETY:` wording of `view_icon_size` in the same file,
       and link the Microsoft Learn page for
       `IFolderView2::SetViewModeAndIconSize` beside the call.
    4. Re-export both functions and both constants from `win32/mod.rs` and
       `lib.rs` next to `desktop_display`.
  - App, `App::adjust_icon_size` (`app/pointer.rs`):
    1. Convert `delta` to notches (`delta / WHEEL_DELTA`); return if 0.
    2. If `self.config.theme.match_explorer()` and
       `self.shell_display.icon_size` is `Some(current)`: call
       `set_desktop_icon_size(next_desktop_icon_size(current, notches))`;
       if it returns true, call `self.refresh_shell_display(ui)`, which sees
       the new size and re-flows the matching spaces. Return.
    3. Otherwise keep today's behavior (change `Theme.icon_size`).
    4. Update the method's doc comment to describe both branches.
    Note the scope: the branch follows the *global* preset, as today's code
    follows the global icon size. A space with its own `Custom` look is
    unaffected either way.
  - Docs: `CONTEXT.md` section 8 (the Ctrl+wheel bullet), `README.md`
    Controls (the Ctrl+wheel sentence), and the F-25 row in the ROADMAP
    feature index (drop "open (F13)").
  - Done when: the tests pass. **[needs human]** check: with the default
    preset, Ctrl+wheel over a space changes the desktop's icons and the
    spaces together, both ways, and stops at the smallest and largest size;
    with the preset set to Custom, Ctrl+wheel changes only the spaces.

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
- **G5. Right-button drag (ROADMAP D9).** A right drag on an item always
  starts a shell drag (it never reorders inside the space). On drop,
  Explorer and the desktop show their own Move / Copy / Create shortcut
  menu; a space shows DeskSpace's. Three tasks, in order.

- [ ] **G5a. A right drag starts a shell drag.**
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

- [ ] **G5b. DeskSpace's drop menu on a space.** After G5a.
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
    2. `desired_effect`: for a right drag, return `MOVE` (the menu decides
       the rest), so the cursor does not flicker with the modifier keys.
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

- [ ] **G5c. "Create shortcuts here".** After G5b and the F-50 design
  (both need the same "create a `.lnk` in the store" helper, via
  `IShellLinkW` + `IPersistFile`). The entry goes between "Copy here" and
  "Show here without moving", and needs a fourth `DropAction` variant.
  **[needs decision]** until F-50's design settles where shortcuts are
  created and how packaged apps are handled.
- [ ] **G6. Ctrl+Z undo.** Design first; see ROADMAP.md section 8, open
  question 3.

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
    When it is set, `d2d.rs` draws a 2 px outline just inside the frame,
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
    `OleUninitialize` (add to the `run` teardown in `runtime.rs`); re-hide on
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
      `deskspace.exe` in both the Velopack installer and the portable zip (see
      E6); without it the takeover silently loses crash safety.

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

## J. Tabs in spaces (ROADMAP F-49)

Off by default. When a space's tabs are on, its items are paginated: split
across pages, one shown at a time, so the space holds more icons without
scrolling. A tab strip on a chosen edge switches pages, and its size is set per
space. The settings are basic now (enable, orientation, size), with room for
advanced visual and placement settings later. Each task below still needs
design work; the split is a starting point, not a spec. Do them in order.

- [ ] **J1. Model and command (core).** `Space.tabs: TabSettings`
  (`#[serde(default)]`, disabled): `enabled: bool`, `size: u8`, and
  `orientation: TabOrientation { Top (default) | Bottom | Left | Right }`.
  `Command::SetSpaceTabs { id, tabs }` (size clamped). Tabs are space chrome,
  so they live in `Space`, not `SpaceLook`. A disabled space lays out exactly
  as today. Tests: serde defaults, clamped size, the command.
- [ ] **J2. Pagination (core).** Derive the page count from the content area
  and grid; add `page_count` and `page_items(page)`. Decide whether the current
  page is session state (like scroll) or persisted per space. Pages are a view
  over the same `items` Vec, so `ItemId`s and drag semantics are unchanged.
  Tests.
- [ ] **J3. Render the strip (scene and platform).** `SpaceScene`/`SpaceStyle`
  carry the strip (edge, size, page count, current page, labels); the painter
  draws it and the window hit-tests a click to a page. Only the current page's
  cells are drawn and hit-tested.
- [ ] **J4. Per-space Tabs settings section (app).** Enable, orientation, and
  size, using the app's per-space look/space settings path. Basic now;
  advanced visual and placement settings later.
- [ ] **J5. Space menu.** Enable/disable tabs, previous/next page, and a page
  list.

## K. Split the large files (readability)

AGENTS.md asks for files under about 800 lines of non-test code. These are
over it. Each task is a **pure move**: no renames, no logic changes, no
signature changes beyond visibility. That keeps them safe for any
contributor and easy to review (the diff should be almost all moved lines).

Rules for every K task:

- Turn `foo.rs` into a directory module: `foo/mod.rs` keeps what the task
  says stays, and each new file is declared in it with `mod name;`. Keep the
  public path the same (re-export with `pub(crate) use` / `pub use` from
  `mod.rs` where callers outside the module use a moved item), so no caller
  outside the module changes.
- Widen visibility only as far as needed: `pub(super)` for items the
  sibling files share. Child modules already see their parent's private
  items, so moved code calling something that stays in `mod.rs` needs no
  change there.
- Move each test with the function it tests, into a `#[cfg(test)] mod tests`
  at the bottom of the new file. Do not change test bodies.
- Give each new file a one-line `//!` header saying what it holds.
- Do one K task per commit (`refactor: split <file> (Kn)`). If a function
  does not clearly belong to one group, leave it in `mod.rs`.
- Done when: fmt, clippy `-D warnings`, and tests pass with the same test
  count; `git diff --stat` shows mostly moved lines; and `CONTEXT.md`
  section 2 (code health) and the file map above are updated.

- [ ] **K1. Split `win32/settings_window.rs` (~2,600 lines).** Do B8 first;
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

- [ ] **K2. Split `render/d2d.rs` (~1,475 lines).**
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

- [ ] **K3. Split `deskspace-app/src/settings.rs` (~1,820 lines, ~1,060
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

- [ ] **K4. Split `win32/runtime.rs` (~940 lines).** `Ui`'s methods move
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

- [ ] **K5. Move drag out of `app/pointer.rs` (~1,080 lines, ~920 without
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

- [ ] **K6. Move `shell.rs` under `win32/` and split it (~820 lines).** It
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

## Found while working

Add one line per issue noticed but not fixed: file, symptom, suggested task.

- `deskspace-platform/src/win32/drop_target.rs` accepts `CF_HDROP` only
  (`has_files`), so a Start menu or taskbar app drag - which carries a Shell ID
  List, not files - shows the red no-drop cursor over a space. Accept
  `CFSTR_SHELLIDLIST` and create a shortcut in the space, as Explorer does
  (F-50; needs design, including packaged apps with no filesystem path).
- `deskspace-platform/src/win32/controller.rs` handles `WM_SETTINGCHANGE`
  with "ImmersiveColorSet" as `Event::SystemThemeChanged`, which is what
  refreshes the accent. Windows also broadcasts
  `WM_DWMCOLORIZATIONCOLORCHANGED` when the accent changes; if
  "ImmersiveColorSet" is not sent for every accent change, the palette is
  stale until the next theme change. Watch for it during the F-29 check.
- `deskspace-app/src/app/files.rs` / `app.rs`: `Event::FolderChanged`
  re-syncs the desktop layer but never calls `IconCache::invalidate`, so a
  desktop file whose icon changes in place keeps its old icon until restart.
  Suggested task: invalidate the paths whose `modified` changed during
  `resync_desktop_layer`.
- `deskspace-platform/src/win32/runtime.rs`: `is_space_window` checks only
  the spaces map, so a press on the desktop layer is reported as
  `PointerDownOutside` as well as the layer's own `MouseDown`. Check whether
  that clears a layer selection before a Ctrl+click adds to it, and decide
  whether the layer should count as inside.
- `deskspace-app/src/app/files.rs`: Delete and Shift+Delete do nothing for
  loose items on the desktop layer, although Explorer would delete them.
  With takeover on, Explorer's icons are hidden, so this matters for any
  user who opts in (D9 keeps takeover opt-in for v0.2). Suggested task:
  route layer deletes through the same `delete_files` path.
- `deskspace-platform/src/win32/dark_mode.rs`: the settings window's
  selection bar uses a fixed `UiColors.accent`, not the Windows accent the
  spaces' check boxes use. Consider `system_accent_color()` for consistency.
