# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

Only open work lives here. Finished tasks are moved, verbatim, to
`archive/TASKS-done.md` (history only; its file and line pointers predate the
R1 and K splits). Built-but-unconfirmed work is listed in section V.

**Pick a task:** section B (code health; B27-B30 in order), K7, and
section N (in order) are ready for any contributor; G5c is ready. L1 needs a maintainer's on-screen
findings. Skip tasks marked **[needs decision]** and everything in section D.

## How to work a task

1. Read `ENGINEERING.md` once (especially "Design priorities"), then only the
   `CONTEXT.md` sections a task names. Use the file map below to find code.
2. One task per commit. Do not fix "nearby" things; note them under **Found
   while working** at the end of this file instead.
3. Line numbers drift; find code by the function or type name given.
4. If the code does not match the task, or the task needs a design decision
   it does not state, **stop and ask**. Do not improvise.
5. Copy the pattern the task points at, preferring the recipes below. Leave
   the code more readable than you found it, within the task's scope. If no
   extension point fits, stop and ask rather than special-casing.
6. Done means: fmt, clippy `-D warnings` (no new `#[allow]`), and tests pass
   (`ENGINEERING.md` has the commands), with new tests for new logic;
   `CONTEXT.md` updated where
   behavior changed, and the `ROADMAP.md` feature index (section 7) where a
   feature's status changed; and the task's own "Done when" list.
7. Commit in the `ENGINEERING.md` style with the task id in parentheses
   (`refactor: split d2d.rs (K2)`). In the same commit, tick the box and move
   the task, verbatim, to the end of `archive/TASKS-done.md`.
8. **[needs human]** tasks need an on-screen check you cannot do. Implement,
   list exactly what to check in the commit body, add a one-row summary to
   section V, and archive the task as in rule 7. Anything only a human can
   record (for example L1's findings) stays here until it is recorded.

## File map

Where each kind of code lives. Paths are under `crates/`.

| To change... | Look in |
|--------------|---------|
| The model (spaces, items, the desktop layer) | `deskspace-core/src/{space,item,desktop,desktop_layer}.rs` |
| A model mutation | `deskspace-core/src/command/` (`Command` and the dispatch in `mod.rs`; one `apply_<variant>` method each in `space_ops.rs`, `item_ops.rs`, `layer_ops.rs`) |
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
| Mouse, resize, hover | `deskspace-app/src/app/pointer.rs` |
| Ctrl+wheel, zoom, the zoom pop-up's behavior | `deskspace-app/src/app/zoom.rs` |
| Drag, drag image, drag out | `deskspace-app/src/app/drag.rs` |
| Selection, arrow keys, type-to-select | `deskspace-app/src/app/selection.rs` |
| Inline rename | `deskspace-app/src/app/rename.rs` |
| Drops, delete, prune | `deskspace-app/src/app/files.rs` |
| Cut / copy / paste | `deskspace-app/src/app/clipboard.rs` |
| Space, item, and tray menus | `deskspace-app/src/app/menus.rs` |
| Settings window behavior (open, preview, Apply, Cancel) | `deskspace-app/src/app/settings_session.rs` |
| Settings pages, fields, `field::` ids (pure) | `deskspace-app/src/settings/` (`mod.rs`, `ids.rs`, `tables.rs`, `pages.rs`, `apply.rs`, `session.rs`) |
| Show/hide, desktop verb, start with Windows | `deskspace-app/src/app/visibility.rs` |
| Update checks, downloads, automatic installs | `deskspace-app/src/app/update.rs`, `deskspace-update/src/lib.rs` |
| Model → `SpaceScene` (pure) | `deskspace-app/src/scene.rs` |
| Managed store for dropped files | `deskspace-app/src/storage.rs` |
| Events the platform raises | `deskspace-platform/src/input.rs` |
| Menu and settings-form data types | `deskspace-platform/src/ui.rs` |
| `Ui` methods, the message loop | `deskspace-platform/src/win32/runtime/` (`mod.rs`, `ui_spaces.rs`, `ui_input.rs`, `ui_shell.rs`, `ui_chrome.rs`) |
| Controller window (tray, hotkeys, timers, `WM_APP` dispatch) | `deskspace-platform/src/win32/controller.rs` |
| `WM_APP` message ids | `deskspace-platform/src/win32/messages.rs` |
| Keys (spaces are never focused) | `deskspace-platform/src/win32/keyboard.rs` (`fn handle`) |
| Space window procedure, DWM | `deskspace-platform/src/win32/{space_window,dwm}.rs` |
| Drawing | `deskspace-platform/src/render/scene.rs`, `render/d2d/` |
| The zoom pop-up window and its layout | `deskspace-platform/src/win32/zoom_bar.rs`, `render/zoom_bar.rs`, `render/d2d/zoom_bar.rs` |
| Settings window controls | `deskspace-platform/src/win32/settings_window/` (`mod.rs`, `build.rs`, `update.rs`, `nav.rs`, `paint.rs`, `commands.rs`, `preview.rs`) |
| The color picker popup | `deskspace-platform/src/win32/color_picker.rs` |
| Shell: open, item menu, drag out | `deskspace-platform/src/win32/shell_items.rs` |
| Shortcuts from dropped shell items, custom targets | `deskspace-platform/src/win32/shortcuts.rs` |
| The installed-apps picker and its cache, the custom shortcut dialog | `deskspace-platform/src/win32/{app_picker,app_catalog,shortcut_dialog,dialog}.rs` |
| Desktop enumeration, icons | `deskspace-platform/src/win32/desktop_items.rs`, `win32/icons.rs` |
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
2. `deskspace-app/src/settings/`: a `field::` id in `ids.rs` (next free), and
   a `Field` in `pages.rs`: a look setting in `look_pages` (fed the theme
   being edited, so it is per space with no further work), a Global-only one
   in `general_page` or the page it belongs on; option tables go in
   `tables.rs`. Then the match arm that clamps and returns the `Effect`
   (`None`, `Appearance` to restyle, `Layout` to re-flow, `Pending` for
   apply-on-OK) in `apply.rs`: `apply_look_field` for a look setting,
   `apply_field`'s Global-only arms otherwise. A button instead gets a
   `ButtonAction` variant (`settings/mod.rs`), a line in `button_action`,
   and an arm in `App::on_settings_button`. The tests `field_ids_are_unique`,
   `every_editable_global_field_has_a_handler`,
   `every_editable_space_field_has_a_handler`, and `every_button_has_an_action`
   fail until the field is wired up.
3. If Cancel must undo a re-flow or restyle, extend `settings::needs_relayout`
   or `needs_restyle` (called from `App::cancel_settings`).
4. If Apply must act outside the config (registry, hotkeys, takeover), add it
   to `App::apply_settings` (`app/settings_session.rs`) next to
   `sync_start_with_windows`.
5. Tests: a serde round-trip / missing-field default test in core or config,
   and an `apply_field` test in `settings/apply.rs` (copy
   `start_with_windows_is_pending`).

A look setting outside `Theme` and `GridSpec` goes into `SpaceLook` (keep
`from_global`, `clamped`, and `grid` in step). The app reads a space's values
through `theme_for(id)`, `grid_for(id)`, and `palette_for(id)`, never
`config.theme` directly.

### Recipe: a new model change (`Command`)

1. `deskspace-core/src/command/mod.rs`: a variant with a `///` doc and a
   one-line arm in `apply_with_layout` calling `self.apply_<variant>(..)`,
   written in `space_ops.rs`, `item_ops.rs`, or `layer_ops.rs` (using
   `with_space` for a single space). Return `NotFound` for a missing id,
   `Rejected` when not allowed (for example a locked space), else `Applied`.
2. Tests in `command/mod.rs`: one per outcome (copy the `Reflow` tests).
3. App: call it through `self.dispatch(...)` or, for a change to one space
   that should repaint and save, `self.apply_to_space(ui, id, command, at)`
   (`app/persist.rs`). Never mutate `self.config.desktop` directly.
4. Add the variant to the Commands list in `CONTEXT.md` section 4.

### Recipe: a new key

1. `deskspace-platform/src/input.rs`: a `SelectionKey` variant (no data) with
   a `///` doc. Only a key that carries data gets its own `Event` variant and
   `WM_APP` id (in `win32/messages.rs`, decoded in `event_for` in
   `win32/controller.rs`).
2. `win32/keyboard.rs`: a `SEL_*` code and a branch in `fn handle` under the
   right flag (the `ENGINEERING.md` Keys table) that posts `SELECTION_KEY`; map
   the code to the variant in `fn selection_key` (and its test).
3. `deskspace-app/src/app.rs`: handle it in `App::on_selection_key`, calling
   a method in the `app/` module that owns the behavior.
4. Add the key to the `ENGINEERING.md` Keys table (the one flag-by-key
   list), what it does to `CONTEXT.md` section 8, and a user-facing line to
   `README.md` Controls.

### Recipe: a new global hotkey

1. `deskspace-config/src/schema.rs`: a `WindowSettings` string field for the
   binding (default empty, so it starts unbound).
2. `settings/ids.rs`: two `field::` ids (the text field and its note).
3. `settings/tables.rs`: a `HotkeyAction` variant and a `HOTKEY_BINDINGS` row
   (action, ids, label, setting); the Hotkeys page, staging, Apply, and
   registration all read the table.
4. `app.rs`: the action's arm in `App::on_hotkey` (the compiler asks for it).
   Extend `each_action_has_one_binding` and
   `each_binding_stores_its_own_setting` in `tables.rs`.

### Recipe: a new menu entry

1. `app.rs`: an id in `mod menu` (next free) and in the `other` list of the
   `menu_ids_are_unique` test. An entry in a family (opacity, icon size,
   sort) is a row in the preset table instead.
2. `app/menus.rs`: a `MenuEntry` in the builder (`show_space_menu`,
   `show_tray_menu`, `show_item_menu`, or the submenu helper) and an arm for
   the chosen id. Entries shared by the tray and the space menu go through
   `run_common_menu`.
3. Update the menu lists in `CONTEXT.md` section 8 and `README.md` Controls.

### Recipe: a new visual in a space

1. `render/scene.rs`: a `SpaceScene` field with a `///` doc and a neutral
   default (copy `highlight`).
2. `deskspace-app/src/scene.rs`: fill it in `space_scene` (and
   `desktop_layer_scene` if the layer needs it), with a scene test (copy
   `the_settings_highlight_reaches_the_scene`).
3. `render/d2d/`: draw it in `Painter`'s paint path without allocating;
   pre-compute in `PreparedScene` if needed.

### Recipe: behavior on both spaces and the desktop layer

Take a `Surface`, not a `SpaceId`, and read through the `surface_*` accessors
in `app/hit.rs`. If one is missing, add it there for both variants rather than
matching on `Surface` at the call site.

---

## B. Code health (any contributor)

Each is a behavior-preserving refactor: the test count only grows, and no
user-visible change is expected. Do them in any order.

- [ ] **B13. One child-control helper.** `win32/dialog.rs` `control` and
  `win32/settings_window/build.rs` `Builder::control` are near-copies, both
  allowed `too_many_arguments` (8).
  - Add `pub(crate) struct ControlSpec<'a> { class: PCWSTR, text: &'a str,
    style: u32, ex_style: WINDOW_EX_STYLE, bounds: (i32, i32, i32, i32), id:
    usize }` in `dialog.rs`; `control(chrome, parent, spec)` creates the
    window and sets the font. `Builder::control` calls it with its own font.
  - Done when: both allows are gone and **Found while working** no longer
    lists `too_many_arguments`. **[needs human]** check: the settings window
    and the custom shortcut dialog look as before.

- [ ] **B14. Split `Builder::field` by field kind.**
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

- [ ] **B18. Menu ids as an enum.** The model is B15 (commit `3096a47`,
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
  - Docs: `CONTEXT.md` section 6 "Other ids" names `MenuId`; the recipe "a
    new menu entry" step 1 becomes "a `MenuId` variant (next free value) and
    its `ALL` entry".
  - Done when: `mod menu` is gone and no `u32` menu id literal is left in the
    app.

- [ ] **B19. Forbid `unsafe` in `deskspace-app`.** The app crate has no
  `unsafe` today, but nothing enforces "all `unsafe` lives in the platform"
  there. Add `#![forbid(unsafe_code)]` at the top of both binary roots,
  `src/main.rs` and `src/watchdog_main.rs` (after the existing
  `windows_subsystem` attribute and its comment). Leave `build.rs` alone; it
  is its own crate. Update the three places that list which crates forbid it:
  `ENGINEERING.md` section 3, `CONTEXT.md` section 3 ("Crate
  responsibilities"), and `CONTRIBUTING.md` "First 15 minutes" rule 2.
  - Done when: the gates pass and the docs say four crates forbid `unsafe`.

- [ ] **B21. Seed the first space through `Command`s.** `bootstrap.rs`
  builds the initial "Desktop" space by calling `Desktop::add_item` and
  `arrange_space` on `desktop.space_mut(id)` directly, the one place outside
  tests that changes the model without a `Command` (`ENGINEERING.md`
  section 3). Seed it with `Command::CreateSpace`, one `Command::AddItem` per
  entry (the `ItemFacts` it already reads), then `Command::AutoArrange`,
  applied through `Desktop::apply` with the work area `bootstrap` already
  uses. Same result, so the existing seeding tests pass unchanged; add one if
  none covers "a fresh config gets one arranged Desktop space".
  - Done when: `bootstrap.rs` calls no `*_mut` model accessor.

- [ ] **B22. List the process-wide state.** The platform keeps about sixteen
  `static`s and `thread_local!`s, each justified (hooks and Win32 callbacks
  get no context pointer), but nowhere listed, which makes the runtime the
  hardest part to reason about. Add a subsection "Process-wide state" to
  `CONTEXT.md` section 6, after "Runtime": a table of every one with its file,
  name, what it holds, why it cannot be passed instead, and who resets it.
  Find them with `grep -rn "^static\|thread_local!" crates/*/src` (the
  current set is in `render/d2d/graphics.rs`, `app_catalog.rs`,
  `color_picker.rs`, `console.rs`, `controller.rs`, `desktop_icons.rs`,
  `desktop_shell.rs`, `drop_target.rs`, `keyboard.rs`, `outside_click.rs`,
  `runtime/mod.rs`, `shell_items.rs`, `shell_view.rs`, `tray.rs`, and
  `watchdog.rs`). Docs only; no code changes. If one has no reason you can
  state, list it under **Found while working** instead of guessing.

- [ ] **B23. Make the keyboard watch a pure, tested table.**
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

- [ ] **B24. Split `App::on_space_event`.** `app/pointer.rs`
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

- [ ] **B25. Name `App::on_event`'s long arms.** `app.rs` `on_event` is a
  good dispatch `match` except for `Event::Started` (about 40 lines),
  `Event::SystemThemeChanged`, and `Event::CloseRequested`. Move each into
  `on_started`, `on_system_theme_changed`, and `on_close_requested`, in the
  module that owns most of what they touch (`app/view.rs` for the theme,
  `app/persist.rs` or `app.rs` for start and close; follow `CONTEXT.md`
  section 7's runtime-flow steps 3 and 6, which describe them). Pure move.
  Done when: every arm of `on_event` is one call.

- [ ] **B26. Split `settings_window::build` and `update`.**
  `win32/settings_window/build.rs` `build` (about 210 lines) and
  `update.rs` `update` (about 125 lines) each build or diff the panes, the
  header, and the pages in one body. Split each into `*_panes`, `*_header`,
  and `*_pages` helpers in the same file, in the order the body already runs
  them. Do it after B14 (same file). Pure move. Done when: no function in
  either file is over about 80 lines. **[needs human]** check: the settings
  window opens, switches scopes and pages, and previews as before.

Not listed here on purpose: `settings/pages.rs` `look_pages` (about 205
lines), `settings/apply.rs` `apply_look_field` (about 110), and
`settings/mod.rs` `build_form` (about 105). They are long because a field is
wired in three places, and R14 (table-driven settings fields) removes that
cause; splitting them first would be undone by R14.

### Make the compiler enforce the rules (B27-B30)

`ENGINEERING.md` section 2, rules 1 and 4: a rule the compiler can check
should not depend on review. Do the tasks in the order listed. Each is one
commit. Each step says exactly what to type; if the code does not match what
a step describes, stop and ask (rule 4).

**B27: the model can change only through `Command`.** Today `Space`'s fields
and `Desktop::spaces` are `pub`, and so are `Desktop::space_mut`,
`layer_mut`, `add_space`, `add_item`, and `remove_item`, so any crate can
bypass `Command`. B27a-B27f close that, after B21.

- [ ] **B27a. Move `Config::migrate`'s two space loops into core.**
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

- [ ] **B27b. Add read accessors (core only).**
  - In `deskspace-core/src/space.rs` `impl Space`, add one-line public
    getters: `title(&self) -> &str`, `rect(&self) -> Rect`,
    `items(&self) -> &[Item]`, `locked(&self) -> bool`,
    `rolled_up(&self) -> bool`, `custom(&self) -> bool`,
    `look(&self) -> Option<&SpaceLook>`, `user_sized(&self) -> bool`.
  - In `deskspace-core/src/desktop.rs`, add `spaces(&self) -> &[Space]`.
  - Leave `id` public: it never changes after creation.
  - If a method with one of these names already exists, stop and ask.
  - Change no caller in this commit. Done when: the gates pass.

- [ ] **B27c. Read through the accessors outside core.**
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

- [ ] **B27d. Write test fixtures through `Command`s.**
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

- [ ] **B27e. Make `Space`'s fields private.** Change the eight fields to
  `pub(crate)` for real. `serde` still reads and writes them. Done when: the
  gates pass with no other change.

- [ ] **B27f. Make the `Desktop` mutators crate-private.**
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

**B28: one commit path that refreshes and saves.** Today each caller of
`App::dispatch` has to remember to call `refresh_space` and `schedule_save`
afterwards, and `dispatch` returns a `bool`, so `NotFound` and `Rejected`
look the same. Do B28a-B28d in order.

- [ ] **B28a. `App::dispatch` returns the `CommandOutcome`.**
  - In `app/persist.rs`, change `dispatch` to return `CommandOutcome`
    instead of `bool`. Before returning, when the outcome is not applied,
    log it once: `tracing::debug!(?outcome, "command not applied")`.
  - Each caller that used the `bool` adds `.is_applied()`. Find the callers
    with `rg -n "self\.dispatch\(" crates/deskspace-app/src`.
  - Pure refactor. Done when: the gates pass.

- [ ] **B28b. Core: `Desktop::touched_by` says what a command will change.**
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

- [ ] **B28c. `App::commit` replaces `apply_to_space`.**
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

- [ ] **B28d. Move the bare `dispatch` callers to `commit`.** Make one
  commit per file: `app/menus.rs`, `app/clipboard.rs`, `app/files.rs`,
  `app/drag.rs`, `app/rename.rs`, and `app/settings_session.rs`. For each
  `self.dispatch(` call:
  - If it is followed by `refresh_space` or `schedule_save` for the spaces
    it changed, replace the call with `commit` and delete those follow-ups.
  - If it is one of several commands followed by a single refresh, or it is
    followed by `flush_save` (file moves save at once), keep `dispatch`. Add
    a one-line comment above it saying which case it is.
  - Done when: every `self.dispatch(` outside `commit` has such a comment.

- [ ] **B29. Tell the user when the config cannot be saved.** Today
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

- [ ] **B30. Lay a command out on the grid of the space it targets.**
  Today `App::dispatch` picks the grid from `space_at(near)`, the topmost
  space under the point. When spaces overlap, that can be a different space
  from the one the command changes. After B28b:
  - In `dispatch`, take `touched_by(&command).spaces.first()`. If it is a
    space that exists, use `self.layout_for(id)`. Otherwise keep today's
    `space_at(near)` fallback.
  - Done when: the gates pass. **[needs human]** check: overlap two spaces
    with different icon sizes, then sort or arrange the lower one. Its icons
    land on its own grid.

## L. Adding items to spaces (ROADMAP F-50, F-53..F-55)

Everything except L1 has landed (L2-L7 archived). Every way of adding a
shortcut must end in `App::add_shortcuts` (`app/files.rs`, with
`ShortcutSource { Shell, Custom }`); never add a second shortcut writer. The
drop side is `win32/drop_target.rs` (`DragKind`, `drag_kind`, `DropPlan`,
`drop_plan`, `shell_items`, `log_formats`).

- [ ] **L1. Record what each drag source offers.** **[needs human]**
  - The log line has landed (`fix: log every drag a space sees (L1)`).
  - Findings so far (maintainer, 2026-10-14, before the log line): a Start
    app (Element) showed the "Move" cursor, so Start offered `MOVE`, and its
    `CF_HDROP` held the app's AppUserModelID
    (`com.squirrel.element-desktop.Element`), not a path; other Start apps
    showed no "Link" cursor, so the effects vary by item. L2 was amended for
    this and has landed.
  - Check (maintainer): with DeskSpace **not elevated** (UIPI silently blocks
    drops from Start or Explorer into an elevated window), run
    `./scripts/dev.ps1 -Verbosity 1` and drag onto a space: an app from
    Start's "Pinned" grid, one from "All apps", a Store app (for example
    Calculator), a taskbar-pinned app, and a plain file from Explorer. Record
    per source the allowed effects, the formats, the cursor, and the result,
    here (replace this bullet).
  - Done when: the findings are recorded.

## C. Features (ROADMAP M3)

- [ ] **C6. More item presets, and preset import/export (F-48).** Later;
  **[needs decision]**.
  - Add named presets (compact and spacious, for example) that set several
    item settings at once, after "Match Explorer's display" and "Custom", and
    import/export a preset as a file.
  - Decide whether a preset snapshots a whole `Theme`/`SpaceLook` or only the
    item fields, how it interacts with per-space looks, and the file format.
    Keep `Preset` forward-compatible (`#[serde(default)]`). It grows with the
    remaining Explorer item settings (hidden/protected files once F-22
    exists).

- [ ] **C9. Spaces entries on the desktop background menu (F-52).**
  **[needs decision]** on which entries are always shown versus hidden-only.
  Today `win32/desktop_entry.rs` writes only "Show DeskSpace spaces", while
  spaces are hidden (`App::sync_desktop_show_entry`).
  - Platform, `win32/desktop_entry.rs`: replace
    `set_desktop_show_entry(enabled, label, command, icon)` with
    `set_desktop_verbs(verbs: &[DesktopVerb])`, where `DesktopVerb { key,
    label, command, icon }` are `&'static str` and each `key` is a child of
    `DesktopBackground\Shell` under a common `DeskSpace` prefix. Delete each
    known key (`RegDeleteTreeW`), then write them; keep the `create` /
    `set_string` helpers.
  - Each verb re-launches the executable with a flag the running instance
    handles, as `--show` does: `--settings` exists; add `--new-space` (a
    `cli.rs` field; `bootstrap.rs` posts a new `controller::NEW_SPACE`, the
    next free `WM_APP` id, and exits; the controller raises an event the app
    handles by creating a space at the pointer).
  - App, `app/visibility.rs`: `App::sync_desktop_verbs` writes "New space",
    "DeskSpace Settings", and, only while hidden, "Show DeskSpace spaces";
    `remove_desktop_show_entry` becomes `remove_desktop_verbs`. Call it on
    `Started` and on every visibility change.
  - Docs: `CONTEXT.md` sections 6 (the `WM_APP` table) and 8 (CLI flags), and
    the `ROADMAP.md` F-52 row.
  - Done when: CI is green (registry writes stay behind `cfg(test)`) and the
    flag/event round trip is unit-tested. **[needs human]** check: right-click
    the desktop and use "New space" and "DeskSpace Settings"; the entries are
    gone after a clean exit.

- [ ] **C10. Theme files: save, load, and share a look (F-58).**
  **[needs decision]**: the maintainer approves or amends the proposal below,
  then removes this marker and the task is ready. Distinct from C6: C6 is
  named *item* presets; C10 is a file holding a whole look, so people can
  share themes without forking.
  - Proposal (recommended by review, 2026-10-16):
    - A theme file is a `SpaceLook` (the whole `Theme` plus grid spacing and
      padding), because `SpaceLook` is already the unit of "a look": it
      round-trips through serde, defaults missing fields, and clamps on load.
      `ui_mode` is ignored on apply, as it is for a space. Open point: should
      `preset` (Match Explorer / Custom) travel with a theme? Proposed: yes,
      since it decides icon size and spacing, which are part of how a theme
      looks.
    - Format: TOML, extension `.deskspace-theme.toml`, with `format = 1`,
      `name`, an optional `author`, and a `[look]` table. A file with a newer
      `format` loads with a warning, unknown keys ignored, as `Config` does.
    - Location: a `themes` folder beside the config file (so a scratch
      `--config` has its own), listed in Settings; "Save theme…" and "Load
      theme…" also take any path through the standard file dialogs.
    - Applying: in the Global scope it writes `config.theme` and the grid's
      `gap` / `padding`; in a space's scope it is a `SetSpaceLook { custom:
      true }`. Either way it is a previewed edit that Cancel undoes, like any
      other settings change.
  - Once approved, three commits:
    - **C10a (config, pure):** `deskspace-config/src/theme_file.rs` with
      `ThemeFile { format, name, author, look: SpaceLook }`, `load(path)` and
      `save(path)` returning `ConfigError`, and `themes_dir()` on
      `ConfigStore`. Tests: round trip, missing fields default, out-of-range
      values clamp, unknown keys ignored, a newer `format` loads.
    - **C10b (platform):** `Ui::pick_save_path(filter, default_name)` over
      `IFileSaveDialog`, beside `Ui::pick_files`, and a filter argument on
      the open side if it has none.
    - **C10c (app):** two buttons on the Colors page ("Save theme…", "Load
      theme…"): two `ButtonAction` variants and their arms in
      `App::on_settings_button` (the "new global setting" recipe's button
      path), and a "Themes" `Choice` listing `themes_dir()`. **[needs human]**
      check: save a customized space's look, load it on Global and on another
      space, Cancel undoes the load.

## F. Ad-hoc features

## G. More Explorer-standard controls

- [ ] **G5c. "Create shortcuts here" on a right drop (ROADMAP D9).** G5a and
  G5b (the right-drag menus) have landed and await their checks (section V).
  Add the entry between "Copy here" and "Show here without moving".
  Choosing it takes L2's `DropPlan::Shortcuts` path: read the items with
  `shell_items(data)`, raise `ShellItemsDropped`, and return `LINK`, so
  `App::add_shortcuts` writes the shortcuts (or copies a dropped `.lnk`). No
  new `DropAction` variant and no second shortcut writer.
- [ ] **G6. Ctrl+Z undo.** After R6 settles the design (ROADMAP section 8,
  open question 3).

## J. Tabs in spaces (ROADMAP F-49)

Off by default. When a space's tabs are on, its items are paginated, one page
shown at a time, with a tab strip on a chosen edge (size set per space). Basic
settings now (enable, orientation, size); advanced ones later. Each task still
needs design work; the split is a starting point, not a spec. Do them in
order, after B11 and B12 (the strip adds scene and paint arguments).

- [ ] **J1. Model and command (core).** `Space.tabs: TabSettings`
  (`#[serde(default)]`, disabled): `enabled: bool`, `size: u8`, and
  `orientation: TabOrientation { Top (default) | Bottom | Left | Right }`.
  `Command::SetSpaceTabs { id, tabs }` (size clamped). Tabs are space chrome,
  so they live in `Space`, not `SpaceLook`. A disabled space lays out exactly
  as today. Tests: serde defaults, clamped size, the command.
- [ ] **J2. Pagination (core).** `page_count` and `page_items(page)` from the
  content area and grid. Decide whether the current page is session state
  (like scroll) or persisted. Pages are a view over the same `items`, so
  `ItemId`s and drag semantics are unchanged. Tests.
- [ ] **J3. Render the strip.** `SpaceScene`/`SpaceStyle` carry the strip
  (edge, size, page count, current page, labels); the painter draws it and
  the window hit-tests a click to a page. Only the current page's cells are
  drawn and hit-tested.
- [ ] **J4. Per-space Tabs settings section (app).** Enable, orientation, and
  size, through the per-space settings path.
- [ ] **J5. Space menu.** Enable/disable tabs, previous/next page, and a page
  list.

## K. Split the large files (readability)

`ENGINEERING.md` asks for files under about 800 lines of non-test code. When one
grows past it, add a K task naming what stays in `mod.rs` and what moves where
(the archived K tasks are the model). Each K task is a **pure move**: no
renames, no logic changes, no signature changes beyond visibility.

Rules for every K task:

- Turn `foo.rs` into `foo/mod.rs` plus new files declared with `mod name;`.
  Keep public paths the same (re-export from `mod.rs`), so no caller outside
  the module changes.
- Widen visibility only as far as needed (`pub(super)` for items siblings
  share; children already see their parent's private items).
- Move each test with the function it tests, into a `#[cfg(test)] mod tests`
  at the bottom of its new file, bodies unchanged.
- Give each new file a one-line `//!` header.
- Commit as `refactor: split <file> (Kn)`. If a function does not clearly
  belong to one group, leave it in `mod.rs`.
- Done when: fmt, clippy, and tests pass with the same test count;
  `git diff --stat` shows mostly moved lines; and `CONTEXT.md` section 2
  (code health) and the file map above are updated.

- [ ] **K7. Split `win32/color_picker.rs`** (about 1,400 non-test lines).
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

## N. Narrow `App` (state ownership)

`App` has 25 fields and about 300 methods spread over the `app/` modules,
and every module can reach every field. The split by file is mostly sound:
the large shared fields are already owned structs (`Selection`,
`PointerState`, `EditState`, `UpdateState`), and `config` is the model, so
reading it everywhere is expected. What is left is loose fields that belong
to one concept. Measured on 2026-10-16 (field: the `app/` modules that use
it, outside tests):

| Field | Used in |
|-------|---------|
| `type_buffer` | `app.rs` (timer), `selection` |
| `watching_outside` | `selection` |
| `settings`, `pending_preview`, `highlighted` | `settings_session` (and one read each in `view`, `menus`, `update`) |
| `takeover_layer`, `takeover`, `desktop_entries` | `view` |
| `hotkeys` | `app.rs`, `settings_session` |
| `system_dark`, `system_accent`, `shell_display` | `app.rs`, `view`, `hit`, `selection`, `zoom`, `rename`, `settings_session` |
| `scroll`, `icon_scales` | `hit`, `selection`, `pointer`, `files`, `menus`, `settings_session`; `view` |

Rules for every N task:

- Each task moves loose fields into one owned struct, in the module that
  already owns their behavior, with the methods that touch only those
  fields. Copy how `Selection` (`app/selection.rs`) is declared and held.
- Behavior-preserving: same test count or more, no user-visible change, no
  logic edits beyond `self.x` becoming `self.group.x` or a method call.
- A use outside the owning module goes through a small method (`is_open()`,
  `layer_active()`), not a `pub(super)` field. If a use needs more than a
  one-line accessor, stop and ask (rule 4).
- Update `CONTEXT.md` section 3's module table if a module's "Owns" column
  changes. Commit as `refactor: <what> (Nn)`.

- [ ] **N1. Move `type_buffer` and `watching_outside` into `Selection`.**
  Both are selection state (type-to-select's prefix, and whether outside
  clicks are watched while something is selected). `Timer::Type` in
  `App::on_timer` clears the buffer through a `Selection` method.

- [ ] **N2. A `SettingsUi` for the open settings window.** In
  `app/settings_session.rs`: `struct SettingsUi { session: Option<Session>,
  pending_preview: Preview, highlighted: Option<SpaceId> }`, held as
  `App::settings_ui`. The reads in `view.rs`, `menus.rs`, and `update.rs`
  become `self.settings_ui.is_open()` (or the accessor the read needs).

- [ ] **N3. A `LayerState` for the desktop layer.** In `app/view.rs`:
  `struct LayerState { layer_flag: bool, takeover_flag: bool, entries:
  Vec<DesktopEntry> }` from `takeover_layer`, `takeover`, and
  `desktop_entries`. `App::layer_active` and `App::desktop_takeover` read the
  config's `take_over_desktop` too, so they stay on `App` and call the
  struct's flag accessors. `bootstrap.rs` sets
  the two flags through a constructor. Check `desktop_entries`' doc comment
  ("until then they are only diagnostics"); if the layer now consumes them,
  fix the comment in the same commit.

- [ ] **N4. A `Hotkeys` for the registered bindings.** In `app.rs` (or a new
  `app/hotkeys.rs` if it passes about 60 lines): `struct Hotkeys([Option<Hotkey>;
  HOTKEY_COUNT])` with `register_all(ui, &WindowSettings)` and
  `unregister_all(ui)`, the bodies the start, Apply, and close paths share
  today. `App::on_hotkey` keeps the action `match`.

- [ ] **N5. A `SystemLook` for what Windows reports.** In `app/view.rs`:
  `struct SystemLook { dark: bool, accent: Option<Color>, shell: ShellDisplay
  }` with `refresh(&mut self)` (the reads `Event::SystemThemeChanged` and a
  space click do now). Do it after N3, and after B25 if `on_event`'s theme arm
  has moved. Mechanical, but it touches seven modules: one commit, and run
  the gates before it.

- [ ] **N6. A `ViewCache` for per-space view state.** `scroll` and
  `icon_scales` are per-space, not persisted, and keyed by `SpaceId`. Put them
  in `struct ViewCache` in `app/view.rs` with `scroll_of(id)`,
  `set_scroll(id, px)`, `forget(id)` (call it where a space is deleted; today
  both maps keep a deleted space's entry, so check and note it under **Found
  while working** if that is still true), and the `icon_scales` reads. After
  N5.

After N1-N6, `App` holds the model and its stores, the owned groups, and a few
flags. Splitting `Ui` so `App` can be tested against a fake is R15.

## D. Reserved (strong model or maintainer; do not start)

Each needs design work first. Ids are `R1`-`R17` so they do not collide with
the ROADMAP decision ids (`D1`-`D9`).

- **R6. Undo design.** Needs an answer to ROADMAP open question 3.
- **R7. On-screen verification pass** (maintainer). The list is section V.
- **R10. Code signing (maintainer, ROADMAP M8 item 3, D6).** Apply to
  SignPath Foundation (an OSI license, which Apache-2.0 is, and a public
  repository and build). Add their GitHub Actions integration to
  `release.yml`: sign `deskspace.exe` before both the zip and the installer,
  and the installer through cargo-packager's `windows.signCommand` (a shell
  command with `%1` for the path); credentials as Actions secrets. Sign
  before turning on the update feed (which waits for the
  `DESKSPACE_MINISIGN_KEY` secret and `UPDATE_PUBKEY` in
  `deskspace-update/src/lib.rs`), because unsigned updates trigger
  SmartScreen on every version.
- **R11. Windows 11 top-level context menu (ROADMAP M8 item 4).** Needs R10;
  M2 may make it unnecessary.
- **R12. Release dry run on clean machines (maintainer).** Do it before the
  next release, following `RELEASING.md` with a pre-release version (for
  example `0.3.0-alpha.1`). Check:
  - the zip contains `deskspace-watchdog.exe` beside `deskspace.exe` (E12);
  - the workflow publishes a pre-release with the zip, the `.sha256`, and
    `deskspace_<version>_x64-setup.exe`;
  - on clean Windows 10 and 11 VMs: the extracted exe runs, shows its icon,
    and lists the version under Properties > Details; the installer installs
    into `%LocalAppData%\DeskSpace.App` with no admin prompt and creates a
    Start-menu shortcut;
  - uninstalling removes the install root, the Run value, and the desktop
    verb, and leaves the storage folder and the config (E7);
  - install `alpha.1`, publish `alpha.2`: the manual tray check offers it and
    the update restarts into `alpha.2` with the same spaces (E10/E11);
  - what SmartScreen shows.

  Record the findings in `CONTEXT.md` section 10.
- **R13. winget (maintainer, ROADMAP M8 item 8).** After the first stable
  version with the installer: submit the first manifest to
  `microsoft/winget-pkgs` by hand (`wingetcreate new`, package identifier
  `generalchad.DeskSpace`), then add a `release.yml` job for non-pre-release
  tags that updates it (`wingetcreate update --submit` or the
  `winget-releaser` action) with a fine-grained PAT as an Actions secret.
- **R14. Table-driven settings fields.** A field is wired in three places
  (an id in `ids.rs`, a `Field` in `pages.rs`, an arm in `apply.rs`), held
  together by the handler tests. Design a row per field (id, page, kind,
  read, write-and-clamp, `Effect`), as `HOTKEY_BINDINGS` does for hotkeys, so
  the setting recipe becomes one row. Decide how option tables, look versus
  Global scope, and greyed-out rules (`MatchExplorer`) fit. After B10.
- **R15. Split `Ui` so `App` can be tested against a fake.** `App`'s loose
  state is section N. What remains is `Ui`: about 60 public methods (already
  filed by area in `runtime/ui_spaces.rs`, `ui_input.rs`, `ui_shell.rs`, and
  `ui_chrome.rs`), taken as one concrete type, so no `App` behavior that
  touches the screen can be unit-tested; the app's tests reach only the pure
  modules. Design focused traits or handles along those four files, a
  recording fake for tests, and how `EventHandler::on_event` receives them,
  keeping `runtime::dispatch`'s re-entrancy rules. After N1-N6. Weigh R17
  first: it may make the fake unnecessary.
- **R17. Pure update, then effects (an alternative to R15).** Make the app's
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

  After N1-N6 and B28; B28's `Touched` is the first effect.
- **R16. Scripting verbs: an out-of-process extension point (F-59).**
  Extensions today mean editing the source. Design a small, stable verb set
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

## V. Awaiting on-screen check

Built and unit-tested, but not yet confirmed on screen (the R7 list). Check
with a scratch `--config` (`./scripts/dev.ps1`), with `--on-top` over a
high-contrast window for anything visual, in light and dark mode where it
matters. Delete a row when it passes (say so in the commit); when it fails,
replace it with a task or a **Found while working** line. The task text for
each id is in `archive/TASKS-done.md`.

| Id | What to check |
|----|---------------|
| C2 (F-29) | With "Use Windows accent" on, selection and border take the accent and follow it live when it changes in Windows Settings (watch for the `WM_DWMCOLORIZATIONCOLORCHANGED` note under **Found while working**). |
| C3 (F-30) | Switching "Settings and menus" between Dark and Light repaints the open settings window and the next popup menu. |
| C4 (F-34) | "Start DeskSpace when you sign in" adds and removes the `Run` value, and DeskSpace starts at sign-in. |
| C5 (F-48) | The "Item preset" dropdown shows both entries; "Custom" enables the Layout fields and the icon size takes effect; "Match Explorer's display" greys them. |
| C7 (F-18) | Each hotkey fires once bound; a conflicting binding is reported beside its field and the window stays open; clearing a binding stops it. |
| C8 (F-17) | A space created from the space menu, the tray, and the hotkey uses the configured title, size, and lock state; a locked new space keeps its size and scrolls. |
| E1, E2 | `deskspace.exe` shows the icon in Explorer's small, medium, and extra-large views and the version under Properties > Details; the tray, settings title bar, and Shift+right-click desktop verb show the icon at 100 % and 150 %. |
| E5-E7, E10-E12 | Covered by the R12 dry run (installer, uninstall, the watchdog in the zip, update from one pre-release to the next). |
| F-45 automatic install | Installed build with a feed: turn on both update toggles; after a check finds a newer release, the tray shows "Restart to update to X"; 10 minutes without input (no full-screen app) installs silently and DeskSpace comes back with the same spaces, with takeover on the desktop icons are never left blank; the tray entry installs at once with a progress window; a portable copy greys the install toggle. |
| F1-F5 | F2 with no item selected renames the space title, with one selected the file; Alt+Enter opens Properties, or Settings with nothing selected; Enter opens the item; Esc clears the selection and still closes an open menu; Shift+Delete bypasses the Recycle Bin. |
| F9 | With "Snap resized spaces to whole columns and rows" off, a resize keeps the dragged size (still snapped to `snap`). The on state passed 2026-10-12. |
| F14 (F-51) | The color picker is opaque, rounded, and bordered; palette selection and double-click confirm; "Custom colors…" shows a flicker-free live preview; the Colors-page swatch opens it; OK and Cancel, Enter and Esc behave; the scale is right at 100 % and 150 %; the settings window does not hang. |
| G1, G3 | On the desktop shell: typing selects by prefix and the buffer expires; Home/End/PageUp/PageDown scroll; arrows move the selection, Shift extends, Ctrl moves only the dotted focus rectangle and scrolls it into view. |
| G5a | Right-drag an item out of a space onto the desktop and into an Explorer folder: the shell's Move / Copy / Create shortcut menu appears and each choice works; a right click without moving still opens the item menu. |
| G5b | Right-drag a file from Explorer onto a space: DeskSpace's Move here / Copy here / Show here without moving / Cancel menu appears; each entry does what it says; the source file is left alone on Copy, Show here, and Cancel; a left drag still moves and Shift still keeps in place. |
| H5, H4 (F-46) | Panes, hairlines, dimmed rows, and keyboard movement in both modes; a space is greyed until customized, then edits apply to it alone and Cancel restores; the switch off and on restores custom values; a menu edit with the window open survives Cancel; an old config with per-space `icon_size`/`tint` opens customized; the highlight outline is visible on Clear and Solid (Acrylic passed). |
| I8 | With takeover on, creating, renaming, and deleting a desktop file updates the layer within about 150 ms. |
| L2 (F-50) | Not elevated, `./scripts/dev.ps1 -Verbosity 1`: a Start "Pinned" app, an "All apps" app, a Store app, and a taskbar app each land in a space as a working shortcut and stay in Start; Alt-dragging an Explorer file makes a shortcut and leaves the file; dropping a `.lnk` with Alt copies it; a plain drag still moves a file and Shift keeps it in place. |
| L5 (F-54) | Copy a file and a folder in Explorer; "New shortcut ▸ Paste shortcut" in a space (and from the Shortcuts page's + tile) makes two shortcuts and the originals stay; with nothing on the clipboard the entry is grey. |
| L6 (F-55) | "Add files…" on a space lists the standard Open dialog on the Desktop; picking two files moves them into that space (as a drop does), and Cancel adds nothing. |
| Drag and drop (F-36, F-37) | The move/copy cursor; no `- Copy` left on the desktop after a drag out; the drag image over Explorer and over spaces; the in-space ghost. |
| Selection (F-41, F-42) | A space click focuses the desktop and clears its selection; check boxes in both Explorer modes; rubber-band and Ctrl+A. |
| Clipboard and rename (F-38, F-02) | Cut, copy, and paste, with cut ghosting and its clearing when another app takes the clipboard; F2 item rename. |
| Display (F-25, F-43) | Compressed/encrypted label colors; info tips; jumbo icons; per-monitor DPI on a mixed-DPI setup. |
| Mouse modifiers | Ctrl+wheel reaching a space with Ctrl set was confirmed from a log on 2026-10-16. Ctrl+click and Shift+click in a space toggle and extend the selection without first using the settings window or a menu. (Ctrl and Shift now come from the mouse message, not `GetKeyState`.) |
| F13 (F-25) | Under the default preset, Ctrl+wheel stops at 16 and 256 px; under Custom it changes the spaces (and, with takeover, the layer's icons) but not Explorer's; a space with its own Custom look keeps its size either way. Default preset with takeover on (the layer's icons resize and re-space with the spaces, both ways) passed 2026-10-16. |
| F-57 zoom pop-up | Still to check: the look in light mode and on the Blur, Solid, and Clear backdrops (Clear shows as Acrylic); the scale at 150 %, where the percentage should read 100 % at "Medium icons". Passed 2026-10-16 at 100 % in dark mode on Acrylic: Ctrl+wheel, the buttons, Ctrl+Plus/Minus/0, the hide timer and hover hold, and no focus steal. |
| F15 | Still to check: the icon-size slider and the Match Explorer preset leave every space's size alone; "Fit to contents" then fits the space to its items; Cancel after a slider preview leaves a never-resized space growing with new items again; moving a space to a monitor with another DPI still scales it (title bar included). Passed 2026-10-16: zooming keeps a never-resized space's size, re-flows its icons, and leaves its title bar and title text alone. |
| R3 | Nothing should change: shortcut arrows and other overlays look as before at every icon size; hover, click, double-click, wheel, and right-click in a space and on the desktop layer; the tray icon's click and menu; the settings window's controls and dark-mode colors; the color picker's swatches, sliders, hover, OK, Cancel, and Custom toggle. |

## Found while working

One line per issue noticed but not fixed: file, symptom, suggested task.

- `win32/controller.rs` treats `WM_SETTINGCHANGE` "ImmersiveColorSet" as
  `Event::SystemThemeChanged`, which refreshes the accent. Windows also
  broadcasts `WM_DWMCOLORIZATIONCOLORCHANGED`; if "ImmersiveColorSet" is not
  sent for every accent change, the palette is stale until the next theme
  change. Watch for it during the F-29 check.
- `app/files.rs` / `app.rs`: `Event::FolderChanged` re-syncs the desktop
  layer but never calls `IconCache::invalidate`, so a desktop file whose icon
  changes in place keeps its old icon until restart. Suggested task:
  invalidate the paths whose `modified` changed during `resync_desktop_layer`.
- `win32/runtime/mod.rs`: `is_space_window` checks only the spaces map, so a
  press on the desktop layer is reported as `PointerDownOutside` as well as
  the layer's own `MouseDown`. Check whether that clears a layer selection
  before a Ctrl+click adds to it, and decide whether the layer counts as
  inside.
- `app/files.rs`: Delete and Shift+Delete do nothing for loose items on the
  desktop layer, although Explorer would delete them; with takeover on, this
  matters for every user who opts in. Suggested task: route layer deletes
  through the same `delete_files` path.
- `win32/dark_mode.rs`: the settings window's selection bar uses a fixed
  `UiColors.accent`, not the Windows accent the spaces' check boxes use.
  Consider `system_accent_color()` for consistency.
- `win32/settings_window/preview.rs`: the Shortcuts page's space preview has
  no keyboard support (not a Tab stop; its X and + are mouse-only).
  Suggested task: arrow-key focus, Delete for the X, and Enter on the +
  tile (ROADMAP M7 accessibility).
- `app/pointer.rs` `right_drag_to` (G5a): a right drag calls
  `drag_item_out` without setting `pointer.drag_image`, so the shell
  substitutes its own oversized image. Suggested task: factor the image build
  out of `begin_drag_visual` and use it for right drags too.
- `win32/desktop_items.rs`: each `DesktopEntry` is stat'ed twice, once for
  its `kind` during enumeration and again by `item_facts` when `view.rs` /
  `bootstrap.rs` build its `Item` (the desktop layer re-reads them on every
  sync). Suggested task: give `DesktopEntry` an `ItemFacts` and drop its
  `kind` field.
