# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

Only open work lives here. Finished tasks are moved, verbatim, to
`archive/TASKS-done.md` (history only; its file and line pointers predate the
R1 and K splits). Built-but-unconfirmed work is listed in section V.

**Pick a task:** these are ready for any contributor:
- section B (code health);
- Q2-Q6;
- section U, in order (U1 and U2 are pure and need no screen).

L1 needs a maintainer's on-screen findings. Skip tasks marked **[needs
decision]** and everything in section D.

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
| Handling an `Event`; timer and menu ids; preset tables | `deskspace-app/src/app.rs` (`on_event`, `MenuId`, `*_PRESETS`) |
| Saving, dispatching a `Command` | `deskspace-app/src/app/persist.rs` |
| An `Effect` a pure decision returns, and `App::run` that carries it out (R17) | `deskspace-app/src/app/effect.rs` |
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
| Test fixtures that build the model through `Command`s | `deskspace-app/src/fixtures.rs` |
| Managed store for dropped files | `deskspace-app/src/storage.rs` |
| Events the platform raises | `deskspace-platform/src/input.rs` |
| Menu and settings-form data types | `deskspace-platform/src/ui.rs` |
| `Ui` methods, the message loop | `deskspace-platform/src/win32/runtime/` (`mod.rs`, `ui_spaces.rs`, `ui_input.rs`, `ui_shell.rs`, `ui_chrome.rs`) |
| Controller window (tray, hotkeys, timers, `WM_APP` dispatch) | `deskspace-platform/src/win32/controller.rs` |
| `WM_APP` message ids | `deskspace-platform/src/win32/messages.rs` |
| Keys (spaces are never focused) | `deskspace-platform/src/win32/keyboard.rs` (`fn watched_key`) |
| Space window procedure, DWM | `deskspace-platform/src/win32/{space_window,dwm}.rs` |
| Drawing | `deskspace-platform/src/render/scene.rs`, `render/d2d/` |
| The zoom pop-up window and its layout | `deskspace-platform/src/win32/zoom_bar.rs`, `render/zoom_bar.rs`, `render/d2d/zoom_bar.rs` |
| Settings window controls | `deskspace-platform/src/win32/settings_window/` (`mod.rs`, `build.rs`, `update.rs`, `nav.rs`, `paint.rs`, `commands.rs`, `preview.rs`) |
| The color picker popup | `deskspace-platform/src/win32/color_picker/` (`mod.rs`, `layout.rs`, `paint.rs`, `draw.rs`) |
| Shell: open, item menu, drag out | `deskspace-platform/src/win32/shell_items.rs` |
| Shortcuts from dropped shell items, custom targets | `deskspace-platform/src/win32/shortcuts.rs` |
| The installed-apps picker and its cache, the custom shortcut dialog | `deskspace-platform/src/win32/{app_picker,app_catalog,shortcut_dialog,dialog}.rs` |
| Desktop enumeration, icons | `deskspace-platform/src/win32/desktop_items.rs`, `win32/icons.rs` |
| Takeover: hide icons, watchdog | `deskspace-platform/src/win32/{desktop_icons,watchdog}.rs` |
| CI and release | `.github/workflows/{ci,release}.yml`, `crates/deskspace-app/build.rs` |

Ids (`WM_APP` messages, timers, menu ids, `field::` ids): never renumber
one. "Next free" in the recipes means the highest value in that list plus
one; read it from the code. `CONTEXT.md` section 6, "Message, timer, and id
tables", says where each list lives and which test guards it.

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
   Then an arm in `Desktop::touched_by` (`command/touched.rs`) naming what it
   changes; the compiler asks for it.
2. Tests in `command/mod.rs`: one per outcome (copy the `Reflow` tests).
3. App: call it through `self.commit(ui, command, at)` (`app/persist.rs`),
   which repaints what `touched_by` names and saves; bare `self.dispatch`
   only when the caller repaints or saves in its own way. Never mutate
   `self.config.desktop` directly.
4. Add the variant to the Commands list in `CONTEXT.md` section 4.

### Recipe: a new key

1. `deskspace-platform/src/input.rs`: a `SelectionKey` variant (no data) with
   a `///` doc. Only a key that carries data gets its own `Event` variant and
   `WM_APP` id (in `win32/messages.rs`, decoded in `event_for` in
   `win32/controller.rs`).
2. `win32/keyboard.rs`: a `SEL_*` code and a branch in `fn watched_key` under
   the right flag (the `ENGINEERING.md` Keys table) that returns
   `SELECTION_KEY` with it, plus a row in that flag's `keys_for_*` test; map
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

1. `app.rs`: a `MenuId` variant (next free value) and its `ALL` entry. An
   entry in a family (opacity, icon size, sort) also gets a row in its preset
   table.
2. `app/menus.rs`: a `menu_item` in the builder (`show_space_menu`,
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

- [ ] **B22. Mark the process-wide state where it lives.** The platform keeps
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
    handles by creating a space on the pointer's monitor).
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

## U. Undo and redo (ROADMAP D11, M2)

Read D11 first; it is the spec. Do the tasks in order, one commit each. The
history is session state in the app; core gains only a restore `Command`
and ids that are never reused. Every undoable action records one entry, and
undo or redo either applies a whole entry or, when the entry is stale,
changes nothing and says so.

- [ ] **U2. The undo stack (app, pure; no callers yet).** `app/undo.rs`:
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
- [ ] **U3. Undo and redo layout changes (app).**
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
- [ ] **U4. Ctrl+Z and Ctrl+Y (the key recipe).** Two `SelectionKey`
  variants under `WATCH_PASTE`, the flag Ctrl+V uses, so they act only
  while a space is current and pass through to Explorer otherwise. Move
  Ctrl+Z out of `other_keys_pass_through_to_type_to_select_or_the_app`.
  **[needs human]** check: Ctrl+Z after a space move puts it back, Ctrl+Y
  moves it again; with no space current, Ctrl+Z on the desktop still undoes
  an Explorer rename.
- [ ] **U5. Undo file moves (app).** Record `Moved` where DeskSpace moves a
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
- [ ] **U6. Restore from the Recycle Bin (platform).**
  `Ui::restore_recycled(original: &Path, since: SystemTime) -> Option<PathBuf>`:
  find the Recycle Bin item whose original location is `original` and that
  was deleted at or after `since` (the newest if several), restore it with
  the shell's "undelete" verb, and return the restored path. `None` when it
  is gone (emptied, or restored by hand). Cite the Microsoft Learn pages for
  the Recycle Bin folder and its properties beside the code. **[needs
  human]** check: recycle a file in Explorer, restore it through a test
  harness or `--verbose` log, and confirm it is back under its old name.
- [ ] **U7. Undo deletes, copies, and new shortcuts (app).** Record
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

## T. Animations (ROADMAP D12, D13, F-33)

Read D12 first; it is the spec. T1 is a spike only a human can judge, and
T2-T11 do not wait for it: they animate today's windows, where the accent
blur steps on and off. T12 and T13 then move spaces to composition so the
blur follows every effect, if T1 says it can. One commit each, in order
(T1 can run alongside).

Two tables carry the design, so a new effect or kind is one row: the
effects (what each one uses) and the kinds (which effects and controls each
offers). The app turns them into a `Play` per window; the platform plays it
with a window timer and pure frame math, and never calls back into the app
per frame. Paint code applies a frame without allocating; whatever an
animation needs (an offscreen bitmap, a mask) is made when it starts.
`space_window.rs` is at the 800-line guideline, so the window side goes in
a new module.

- [ ] **T1. Composition backdrop spike.** **[needs human]**
  - The example has landed (`feat: add the composition backdrop spike
    (T1)`); run it with `cargo run -p deskspace-platform --example
    backdrop_spike`. It takes Ctrl+Alt+Shift+F, G, D, T, and Q, because a
    never-activated window gets no keys. Found while building it (Windows
    11 build 26300): `CompositionMaskBrush` refuses a host backdrop brush
    as its source ("Unsupported source brush type"), so G sweeps with 80
    strips of backdrop at graded opacities instead of a gradient mask.
  - What it is: `crates/deskspace-platform/examples/backdrop_spike.rs`, never shipped: a
    window styled as a space (`WS_POPUP`, `WS_EX_TOOLWINDOW |
    WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP`, `MA_NOACTIVATE`), with
    `DWMWA_USE_HOSTBACKDROPBRUSH` and a `Windows.UI.Composition` tree
    through `ICompositorDesktopInterop::CreateDesktopWindowTarget`: a
    sprite filled by `Compositor::CreateHostBackdropBrush`, a translucent
    tint sprite above it, and a 12 px rounded clip. Keys: F animates the
    root's opacity to 0 and back over 1 s; G masks the backdrop with an
    animated linear gradient (`CompositionMaskBrush`); D masks it with a
    noise surface drawn by Direct2D, its threshold stepped by a timer; T
    toggles topmost. Add the `windows` features it needs to
    `deskspace-platform` only; `cargo deny check all` must still pass.
  - Check (maintainer), over a high-contrast window, on Windows 11 and on
    Windows 10: the blur shows while the window has never been activated
    and while another app is focused; F, G, and D fade, sweep, and dissolve
    the blur itself, not only the tint; the rounded clip is anti-aliased;
    and what shows with "Transparency effects" off, with Energy Saver on,
    after Win+D, and on a 150 % monitor. Compare it with an Acrylic space
    beside it: how close are the two materials? Note the idle CPU and GPU.
  - Done when: the findings are recorded here (replace this bullet). If
    the blur does not render on a never-activated window on Windows 11,
    record that in D12, delete T12-T14, and delete the example.
- [ ] **T15. Three kinds: Startup, Delete, Fold (core, config, settings;
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
- [ ] **T16. Graceful exit (platform, app; D13).**
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
- [ ] **T17. Burn like paper (platform; D12 amended).** Replace Burn's
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
- [ ] **T18. Flames around the frame (platform).** A click-through,
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

- [ ] **T12. Composition device and space tree (platform).** **[needs
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
- [ ] **T13. The blur follows the frame (platform).** **[needs human]**
  - On composition spaces, each frame also sets the backdrop: its opacity
    and rect, and the frame's mask. A `CompositionMaskBrush` cannot take
    the backdrop (T1), so **[needs decision]** between an effect brush
    whose graph takes the backdrop as a source and the painter's mask as
    D2D's AlphaMask effect (it needs a Rust `IGraphicsEffect` and
    `IGraphicsEffectD2D1Interop`), and the backdrop split into strips and
    cells at graded opacities, as the spike does. The blur no longer steps
    there.
  - Update D12's "The blur" and `CONTEXT.md` section 6.
  - **[needs human]** check: T8's list on Windows 11, with no step in the
    blur; the same on Windows 10, where it still steps.
- [ ] **T14. Black hole (platform).** **[needs decision]**: after T13. A
  dark disk opens on the desktop beside the space (a layered, click-through
  window, as `drag_ghost.rs` is) and the space spirals into it: a
  composition visual can rotate and scale with its blur, which the accent
  path cannot. Decide where the hole opens and what Windows 10 plays
  instead, then it is one `EFFECTS` row plus its frame math and drawing.

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
  `git diff --stat` shows mostly moved lines; and the file map above names
  the new files.

## Q. Move the why next to the code (shrink `CONTEXT.md` section 6)

`CONTEXT.md` section 6 explains many Win32 quirks in prose far from the code
they justify. `ENGINEERING.md` section 4 (rules 5 and 8) wants that why in a
comment beside the code, and section 7 wants no doc to restate the code. Each
Q task moves one subsection. Take any order; one task per commit, as
`docs: move <subsection>'s why into the code (Qn)`.

Rules for every Q task:

1. Read the subsection one paragraph or bullet at a time. Put each sentence
   in exactly one of three buckets:
   - **(a) Restates the code**: a field list, a variant list, or what a
     function does that its name or `///` doc already says. Open the code
     and check that it really says it. If it does, delete the sentence. If
     it does not, add the missing words to the item's `///` doc first, then
     delete the sentence.
   - **(b) Explains one function or constant**: why it is written an unusual
     way, a Windows behavior it works around, a bug it fixed, or a Microsoft
     Learn link. Move it into a `//` comment (or the `///` doc) on that item,
     reworded to fit, with its link and date. Then delete the sentence.
   - **(c) Spans several files**: how parts fit together, an ordering or
     re-entrancy rule, or a decision. Keep it, shortened if you can.
2. If you are not sure which bucket a sentence is in, keep it (bucket c).
   Never delete a fact you could not find in the code.
3. Change comments only; no code changes. Every edited `unsafe` block keeps
   its `// SAFETY:` comment.
4. Keep the subsection's heading, because other docs cite it. If nothing is
   left under it, write one line naming the files it covers.
5. The commit body lists each bucket-(b) move as "`<file>`: `<item>`".
6. Done when: the gates pass, the subsection is at most half its length
   before, and `rg -n "<a distinctive phrase from each moved sentence>"
   crates` finds it in the code.

- [ ] **Q2. "Selection, display matching, keys, and clipboard"**
  (`win32/shell_selection.rs`, `win32/display_settings.rs`, `edit.rs`,
  `win32/keyboard.rs`, `win32/clipboard.rs`, `win32/file_op.rs`). The
  ENGINEERING.md Keys table stays where it is.
- [ ] **Q3. "Space windows"** (`win32/space_window.rs`, `win32/dwm.rs`). Keep
  the desktop-layer and `Surface` paragraph (bucket c).
- [ ] **Q4. "Desktop shell, shell items, DPI, hotkeys"**
  (`win32/folder_watch.rs`, `desktop_shell.rs`, `desktop_icons.rs`,
  `watchdog.rs`, `desktop_items.rs`, `icons.rs`, `shell_items.rs`, `dpi.rs`,
  `system.rs`, `info_tip.rs`, `hotkey.rs`).
- [ ] **Q5. "Drops" and "Drag out and the drag image"**
  (`win32/drop_target.rs`, `shell_items.rs`, `drag_image.rs`,
  `drag_ghost.rs`).
- [ ] **Q6. "Rendering" and "Controller"** (`render/`, `win32/controller.rs`,
  `tray.rs`, `console.rs`, `outside_click.rs`, `desktop_entry.rs`).
- [ ] **Q7. One home for the controls.** **[needs decision]**: `CONTEXT.md`
  section 8 and `README.md` "Controls" describe the same controls, one for
  contributors and one for users. Decide one of these:
  - README keeps the user prose, and section 8 keeps only the rules README
    leaves out (exact steps, ranges, and edge cases); or
  - section 8 moves into a `docs/CONTROLS.md` that both link to.

  The key recipe and the menu-entry recipe then name the one place to edit.

## D. Reserved (strong model or maintainer; do not start)

Each needs design work first. Ids are `R1`-`R18` so they do not collide with
the ROADMAP decision ids (`D1`-`D9`).

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
- **R18. Translation design (F-60, ROADMAP M9).** Answer ROADMAP open
  question 4 (the format and where catalogs live), then settle:
  - how keys are checked: a generated enum, or a test that every key the
    code uses has English text;
  - how the platform's own strings (color picker, installed-apps picker,
    custom shortcut dialog, right-drop menu, info tip) reach it without the
    platform holding catalogs;
  - the fallback chain (region, language, English) and plural rules;
  - how the settings window fits longer labels (measure or wrap; B14 names
    the layout constants first).

  Then split M9 items 2-5 into one-commit tasks here. If R14 lands first, a
  field's label belongs in its row.

## V. Awaiting on-screen check

Built and unit-tested, but not yet confirmed on screen (the R7 list). Check
with a scratch `--config` (`./scripts/dev.ps1`), with `--on-top` over a
high-contrast window for anything visual, in light and dark mode where it
matters. Delete a row when it passes (say so in the commit); when only part
passes, cut that part from the row and append it, dated, to the last section
of `archive/ROADMAP-progress-log.md`; when it fails,
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
| F9 | With "Snap resized spaces to whole columns and rows" off, a resize keeps the dragged size (still snapped to `snap`). |
| F14 (F-51) | The color picker is opaque, rounded, and bordered; palette selection and double-click confirm; "Custom colors…" shows a flicker-free live preview; the Colors-page swatch opens it; OK and Cancel, Enter and Esc behave; the scale is right at 100 % and 150 %; the settings window does not hang. |
| G1, G3 | On the desktop shell: typing selects by prefix and the buffer expires; Home/End/PageUp/PageDown scroll; arrows move the selection, Shift extends, Ctrl moves only the dotted focus rectangle and scrolls it into view. |
| G5a | Right-drag an item out of a space onto the desktop and into an Explorer folder: the shell's Move / Copy / Create shortcut menu appears and each choice works; a right click without moving still opens the item menu. |
| G5b | Right-drag a file from Explorer onto a space: DeskSpace's Move here / Copy here / Show here without moving / Cancel menu appears; each entry does what it says; the source file is left alone on Copy, Show here, and Cancel; a left drag still moves and Shift still keeps in place. |
| G5c | Right-drag a file and a `.lnk` from Explorer onto a space and choose "Create shortcuts here": the space gets a working shortcut to the file and a copy of the `.lnk`, and both originals stay; the entry sits between "Copy here" and "Show here without moving"; the same on the taken-over desktop opens a new space holding them. |
| H5, H4 (F-46) | Panes, hairlines, dimmed rows, and keyboard movement in both modes; a space is greyed until customized, then edits apply to it alone and Cancel restores; the switch off and on restores custom values; a menu edit with the window open survives Cancel; an old config with per-space `icon_size`/`tint` opens customized; the highlight outline is visible on Clear and Solid |
| I8 | With takeover on, creating, renaming, and deleting a desktop file updates the layer within about 150 ms. |
| New space placement (D10) | With a few spaces in a row at the top of a monitor, New space from the space menu, the tray, the desktop menu, and the hotkey each opens to the right of the row, its top in line, with the same gutter as the others; with the row full to the screen edge it goes to the next row or under all of them; it never covers another space; on a second monitor it opens on the monitor of the click or the pointer; on an empty monitor it opens near the top-left corner; on a 150 % monitor the gutter looks the same. |
| Desktop menu (F-14) | With takeover on, right-click an empty spot of the desktop: New space, DeskSpace settings…, and Hide/Show spaces sit above Explorer's desktop menu; each DeskSpace entry works; View, Sort by, Refresh, New › Text Document, Paste, Display settings, and Personalize work, and the layer shows the result; the Menu key and Shift+F10 with nothing selected open the same menu; a right-click on a loose icon still shows its item menu, and on a space the space menu. Run elevated once too: the menu still shows New, Display settings, and Personalize below the DeskSpace entries, with masked icons and no separator dangling under Hide spaces; with `-Verbosity 1` any fallback logs a warning naming the step that failed. |
| L2 (F-50) | Not elevated, `./scripts/dev.ps1 -Verbosity 1`: a Start "Pinned" app, an "All apps" app, a Store app, and a taskbar app each land in a space as a working shortcut and stay in Start; Alt-dragging an Explorer file makes a shortcut and leaves the file; dropping a `.lnk` with Alt copies it; a plain drag still moves a file and Shift keeps it in place. |
| L5 (F-54) | Copy a file and a folder in Explorer; "New shortcut ▸ Paste shortcut" in a space (and from the Shortcuts page's + tile) makes two shortcuts and the originals stay; with nothing on the clipboard the entry is grey. |
| L6 (F-55) | "Add files…" on a space lists the standard Open dialog on the Desktop; picking two files moves them into that space (as a drop does), and Cancel adds nothing. |
| Drag and drop (F-36, F-37) | The move/copy cursor; no `- Copy` left on the desktop after a drag out; the drag image over Explorer and over spaces; the in-space ghost. |
| Selection (F-41, F-42) | A space click focuses the desktop and clears its selection; check boxes in both Explorer modes; rubber-band and Ctrl+A. |
| Clipboard and rename (F-38, F-02) | Cut, copy, and paste, with cut ghosting and its clearing when another app takes the clipboard; F2 item rename. |
| Display (F-25, F-43) | Compressed/encrypted label colors; info tips; jumbo icons; per-monitor DPI on a mixed-DPI setup. |
| Mouse modifiers | Ctrl+click and Shift+click in a space toggle and extend the selection without first using the settings window or a menu. (Ctrl and Shift now come from the mouse message, not `GetKeyState`.) |
| F13 (F-25) | Under the default preset, Ctrl+wheel stops at 16 and 256 px; under Custom it changes the spaces (and, with takeover, the layer's icons) but not Explorer's; a space with its own Custom look keeps its size either way. |
| F-57 zoom pop-up | Still to check: the look in light mode and on the Blur, Solid, and Clear backdrops (Clear shows as Acrylic); the scale at 150 %, where the percentage should read 100 % at "Medium icons". |
| F15 | Still to check: the icon-size slider and the Match Explorer preset leave every space's size alone; "Fit to contents" then fits the space to its items; Cancel after a slider preview leaves a never-resized space growing with new items again; moving a space to a monitor with another DPI still scales it (title bar included). |
| B28c | Lock, fold, sort, and "Fit to contents" (space menu and the settings button) each repaint the space at once and survive a restart. |
| B29 | Make the scratch config read-only (`attrib +r`) and move a space: one "DeskSpace could not save" box appears; a second move shows none. Clear the attribute and move again: the file is saved. Closing DeskSpace while it is read-only shows no box. |
| B30 | Overlap two spaces with different icon sizes, then sort or arrange the lower one: its icons land on its own grid. |
| Desktop drop (D7) | With takeover on, drag onto an empty spot of the desktop: a Start "Pinned" app, a Store app, an Explorer file (plain, Shift, Alt, and a right drag), and an item out of a space. Each shows a move/copy/link cursor, not "no", and opens a new space at the drop point holding it. Drop next to a space and near the taskbar: the new space keeps a gutter from both and never overlaps. Right drag, then Cancel: no space is left behind. |
| Spaces above the layer | With takeover on, press Win+D twice, switch tasks a few times, and hide and show the spaces: each space still takes clicks, the wheel, and a drag from Explorer or Start (no "no" cursor). |
| R3 | Nothing should change: shortcut arrows and other overlays look as before at every icon size; hover, click, double-click, wheel, and right-click in a space and on the desktop layer; the tray icon's click and menu; the settings window's controls and dark-mode colors; the color picker's swatches, sliders, hover, OK, Cancel, and Custom toggle. |
| B23 | Key routing should not change: the F1-F5 and G1, G3 rows pass with the same keys, and Ctrl+Plus, Ctrl+Minus, and Ctrl+0 still zoom. |
| R17 | Nothing should change: the F13 and F-57 rows pass (Ctrl+wheel, the zoom keys, and the pop-up's buttons, under both presets; the pop-up hides about 2 s after the last zoom, not while hovered), and so does B28c's row. |
| T4 (D12) | Rechecked after the rail fix: "Animations" reads as a semibold header with its four pages indented under it; the "Windows settings…" button's caption fits; Detail shows a plain number. Then the rest: nothing clipped at 100 % and 150 %; each effect greys the options it does not use, Delete's "Same as new space" greys its options, "All at once" greys the delay; with Windows' "Animation effects" turned off while the window is open, every control greys and the note says why; the button opens the page with that switch (Windows 11 and 10); Cancel undoes an edit. |
| T6 | Nothing should change: spaces on every backdrop, the desktop layer, and a desktop-hosted space (`--placement desktop-*`) paint as before, title, items, border, highlight, and scroll thumb included (the paint path was split so animations can draw through a layer). |
| T8 (D12) | Each Hide and show effect (Roll, Fade, Pixelate, Gradient, Dissolve, Fly, CRT, Burn), from the tray, the space menu, and the hotkey, on Acrylic, Blur, Solid, and Clear spaces at 100 % and 150 %: smooth, no flash of old content when a show starts, and the blur steps on at the start of a show and off at the end of a hide; the hotkey pressed twice quickly turns it around without a jump; Fly leaves by the nearest edge and never shows on a second monitor; a file dragged onto a hiding space is refused; with Windows' "Animation effects" off, hiding and showing are instant; a desktop-hosted space (`--placement desktop-*`) still hides and shows. |
| T9 (D12) | Fold and unfold from the space menu and a title double-click on each backdrop: the body rolls up into the title bar with its items in place, and rolls down showing them; a second double-click mid-way turns it around; Fold set to None folds at once; moving a space right after folding it is not fought by the roll; hiding a space mid-fold hides it folded (or unfolded) and shows it the same way. |
| T10 (D12) | Each New space effect from the space menu, the tray, the hotkey, and a drop on the taken-over desktop: the space appears with it and no empty frame or early blur shows first; at startup with several spaces, "One after another" brings them in reading order at the set delay and "All at once" together; "Start with spaces hidden" shows nothing until shown; Fly from each edge and on a second monitor; hiding during the startup stagger hides every space, waiting ones included. |
| T11 (D12) | Each Delete effect, and "Same as new space", deleting from the space menu and from the settings header: the space plays it, then is gone, and clicks or drops on it while it plays do nothing; deleting a space that is still flying in (a drop on the taken-over desktop that adds nothing does this) turns it around; quitting mid-animation leaves no window behind; with the spaces hidden, a delete is instant. |

## Found while working

One line per issue noticed but not fixed: file, symptom, suggested task.

- `app/menus.rs` `show_desktop_menu`: New › Folder (or any New entry) from
  the takeover desktop menu starts Explorer's rename in its hidden view, so
  the rename box is invisible. Suggested task: start DeskSpace's inline
  rename on the new layer item instead.
- `win32/shell_items.rs` `track_shell_menu`: commands run with
  `CMINVOKECOMMANDINFO`, which has no `ptInvoke`, so Paste and New from the
  desktop menu may land in Explorer's next free slot rather than at the
  click. Suggested task: invoke with `CMINVOKECOMMANDINFOEX` and
  `CMIC_MASK_PTINVOKE`.

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
- `win32/drop_target.rs` `right_drop_entries` (G5c): only "Create shortcuts
  here" is greyed by the source's allowed effects; "Move here" and "Copy
  here" stay enabled even when the source refuses `MOVE` or `COPY`, so the
  menu can report an effect the source did not offer. Suggested task: grey
  each entry by its effect.
- `app/pointer.rs` `right_drag_to` (G5a): a right drag calls
  `drag_item_out` without setting `pointer.drag_image`, so the shell
  substitutes its own oversized image. Suggested task: factor the image build
  out of `begin_drag_visual` and use it for right drags too.
- `deskspace-core` `Desktop` (B27f): a few ways around `Command` are still
  public. `Desktop::layer` is a `pub` field, so another crate can call
  `desktop.layer.push_item`, `item_mut`, or `replace_items`; `remove_space`,
  `move_item`, and `sort_by_id` are `pub` but used only inside core; and the
  `layout` field is written directly by `settings/session.rs` (Cancel) and
  `Config::migrate`. Suggested task: make the first two crate-private, and
  decide whether the global grid is model (a `Command`) or settings.
- `win32/desktop_items.rs`: each `DesktopEntry` is stat'ed twice, once for
  its `kind` during enumeration and again by `item_facts` when `view.rs` /
  `bootstrap.rs` build its `Item` (the desktop layer re-reads them on every
  sync). Suggested task: give `DesktopEntry` an `ItemFacts` and drop its
  `kind` field.
- `win32/runtime/ui_spaces.rs` `create_desktop_layer`: with a desktop host
  (`--placement desktop-*`), turning takeover on from Settings creates the
  layer as the host's newest child, which goes above the hosted spaces, its
  siblings. At startup it is created before them. Suggested task: place a
  hosted layer at `HWND_BOTTOM` among its siblings.

- `app/effect.rs` and `settings/mod.rs`: the app crate has two `Effect` enums,
  R17's (what an event asks the shell to do) and the settings one (what a
  field edit owes: `None`, `Appearance`, `Layout`, `Pending`). Once R17 reaches
  the settings family, both meet in `settings_session.rs`. Suggested task:
  rename the settings one (for example `FieldEffect`) before that step.
