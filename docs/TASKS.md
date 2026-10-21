# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

Only open work lives here. Finished tasks are moved, verbatim, to
`archive/TASKS-done.md` (history only; its file and line pointers predate the
R1 and K splits). Built-but-unconfirmed work is listed in section V.

**Pick a task:** these are ready for any contributor:
- section B (code health);
- C10a-C10c (theme files; C10a is pure and needs no screen);
- Q2-Q6;
- section U, in order (U1 and U2 are pure and need no screen);
- section P, in order within each group (P1, P4, P5, P8, and P11 are pure and
  need no screen);

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
| A model mutation | `deskspace-core/src/command/` (`Command` and the dispatch in `mod.rs`; one `apply_<variant>` method each in `space_ops.rs`, `item_ops.rs`, `lens_ops.rs`, `layer_ops.rs`) |
| A lens's folder view and entries | `deskspace-core/src/source.rs` (`SpaceSource`, `FolderView`, `FolderEntry`) |
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
| The debug window (`--debug`) | `deskspace-platform/src/win32/debug_window/` (`mod.rs`, `build.rs`, `layout.rs`, `pages.rs`, `console.rs`); the console's pure model in `deskspace-platform/src/debug_console.rs`; the log capture in `deskspace-app/src/debug_log.rs`; the panel and actions in `deskspace-app/src/app/debug/` |
| The color picker popup | `deskspace-platform/src/win32/color_picker/` (`mod.rs` the window, `state.rs` its input, `format.rs` the value box's text, `draw.rs` the settings swatch), `render/color_picker.rs` (layout), `render/d2d/color_picker/` (`mod.rs` the painter, whose ignored `render_preview` test writes PNGs; `pen.rs` its drawing kit) |
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

- [ ] **C10a. Theme files and folder discovery (config, pure; ROADMAP D15,
  F-58).** Distinct from C6: C6 is named *item* presets; C10 is a file
  holding a whole look (and optional global animations), so people can share
  themes without forking. Do C10a, C10b, and C10c in order, one commit each.
  - Create `crates/deskspace-config/src/theme_file.rs` and re-export
    `CURRENT_THEME_FORMAT`, `THEME_EXTENSION`, `ThemeEntry`, and `ThemeFile`
    from `crates/deskspace-config/src/lib.rs`:
    1. `pub const CURRENT_THEME_FORMAT: u32 = 1;` and
       `pub const THEME_EXTENSION: &str = "deskspace-theme.toml";`.
    2. `pub struct ThemeFile { pub format: u32, pub name: String, pub author:
       Option<String>, pub look: SpaceLook, pub animations:
       Option<Animations> }` with `#[derive(Debug, Clone, PartialEq, Eq,
       Serialize, Deserialize)]` and `#[serde(default)]`. `Default` sets
       `format: CURRENT_THEME_FORMAT`, `name: "Untitled".into()`, `author:
       None`, `look: SpaceLook::default()`, and `animations: None`.
    3. `pub fn clamped(mut self) -> Self`: calls `self.look =
       self.look.clamped()` and `self.animations =
       self.animations.map(Animations::clamped)`.
    4. `pub fn load(path: &Path) -> Result<Self, ConfigError>`: reads the
       file as UTF-8, parses it with `toml::from_str`, logs a `tracing::warn!`
       when `format > CURRENT_THEME_FORMAT` (unknown keys are ignored by
       serde, as `Config` does), and returns `Ok(theme.clamped())`.
    5. `pub fn save(&self, path: &Path) -> Result<(), ConfigError>`: creates
       the parent directory with `fs::create_dir_all`, writes to a sibling
       `.tmp` file, and renames it into `path` (copy the atomic write in
       `ConfigStore::save` in `store.rs`).
    6. `pub struct ThemeEntry { pub name: String, pub path: PathBuf }` and
       `pub fn list_themes(dir: &Path) -> Vec<ThemeEntry>`: when `dir` does
       not exist, returns `Vec::new()`. Otherwise reads `dir` and collects
       every `*.deskspace-theme.toml` file and every `<dir>/<slug>/manifest.toml`
       file that loads with `ThemeFile::load` (logging a `tracing::warn!` and
       skipping unreadable files). If a loaded file's `name` is empty or
       `"Untitled"`, fall back to the file stem or `<slug>` folder name. Sort
       the entries case-insensitively by `name`.
  - In `crates/deskspace-config/src/store.rs` on `impl ConfigStore`, add
    three directory getters right below `storage_dir()`:
    - `pub fn themes_dir(&self) -> PathBuf` (`<config_dir>/themes`),
    - `pub fn animations_dir(&self) -> PathBuf` (`<config_dir>/animations`),
    - `pub fn scripts_dir(&self) -> PathBuf` (`<config_dir>/scripts`).
    Because they sit beside `self.path`, a scratch `--config` automatically
    gets its own folders.
  - Tests in `theme_file.rs`:
    - `theme_file_round_trips_look_and_animations`,
    - `missing_fields_default_and_unknown_keys_are_ignored`,
    - `out_of_range_values_are_clamped_on_load`,
    - `newer_format_still_loads`,
    - `list_themes_finds_single_files_and_pack_manifests_sorted`.
  - Done when: fmt, clippy, and tests pass.

- [ ] **C10b. Single-file Open and Save dialogs on `Ui` (platform; ROADMAP
  D15, F-58).** Needs C10a.
  - `crates/deskspace-platform/src/win32/dialog.rs` already has
    `pick_save_path(owner, title, default_name, filter)` (built in X3).
    Extend `dialog.rs`:
    1. Add `initial_dir: Option<&Path>` to `pick_save_path`. When `initial_dir`
       is `Some(dir)` and `dir.is_dir()`, pass it to `picker.SetFolder` using
       `SHCreateItemFromParsingName` (`windows::Win32::UI::Shell::SHCreateItemFromParsingName`).
       Update its existing caller in `win32/debug_window/console.rs` to pass
       `None`.
    2. Add `pub(crate) unsafe fn pick_file(owner: HWND, title: &str,
       initial_dir: Option<&Path>, filter: &[(&str, &str)]) ->
       Option<PathBuf>` right below `pick_files`. Copy `pick_files`, but:
       - omit `FOS_ALLOWMULTISELECT` from `SetOptions`;
       - build `COMDLG_FILTERSPEC` from `filter` and call
         `picker.SetFileTypes(&specs)` the same way `pick_save_path` does;
       - if `initial_dir` is `Some(dir)` and `dir.is_dir()`, set the folder
         with `SHCreateItemFromParsingName`, else fall back to
         `FOLDERID_Desktop`;
       - read the chosen file with `picker.GetResult()` (like
         `pick_save_path`) instead of `GetResults()`.
  - In `crates/deskspace-platform/src/win32/runtime/ui_chrome.rs` on `impl Ui`,
    right below `pick_files`:
    1. Add `pub fn pick_file(&mut self, title: &str, initial_dir:
       Option<&Path>, filter: &[(&str, &str)]) -> Option<PathBuf>`. Read
       `owner` with `with_runtime(|runtime| runtime.settings.unwrap_or(runtime.controller))`,
       release the borrow, and call `dialog::pick_file`.
    2. Add `pub fn pick_save_path(&mut self, title: &str, initial_dir:
       Option<&Path>, default_name: &str, filter: &[(&str, &str)]) ->
       Option<PathBuf>`, calling `dialog::pick_save_path` the same way.
  - Done when: fmt, clippy, and tests pass. Checked on screen in C10c.

- [ ] **C10c. The Themes settings page: apply, save, and load themes (app;
  ROADMAP D15, F-58).** Needs C10a and C10b. **[needs human]**
  - Add a sixth look page, **Themes**, so it appears for Global and for each
    space without making the **Colors** page too tall for a 1080-pixel
    display:
    1. `crates/deskspace-app/src/settings/ids.rs`:
       - In `mod page`, insert `pub const THEMES: usize = 5;` after
         `ADVANCED` (4), and add 1 to `GENERAL`, `NEW_SPACES`, `HOTKEYS`,
         `ABOUT`, `SHORTCUTS`, and `ANIMATIONS`.
       - In `crates/deskspace-app/src/settings/mod.rs`, update
         `pub const LOOK_PAGE_COUNT: usize = 6;`.
       - In `ids.rs` `mod field`, add the next free ids: `THEMES_HEADING`,
         `THEME_CHOICE`, `SAVE_THEME`, `LOAD_THEME`, `OPEN_THEMES_FOLDER`,
         and `THEMES_NOTE`.
    2. `crates/deskspace-app/src/settings/session.rs`:
       - Add `pub themes: Vec<ThemeEntry>` and `pub selected_theme: usize`
         (`0` means `"Current"`, `1 + i` means `themes[i]`) to `Session`.
       - In `needs_restyle`, also return `true` when
         `config.window.animations != saved.saved_animations`, and in
         `App::cancel_settings` (`app/settings_session.rs`) restore
         `saved_animations` so Cancel undoes a Global theme's animation
         changes too.
    3. `crates/deskspace-app/src/settings/pages.rs`:
       - In `look_pages`, build and append `themes_page(session)` with:
         - `Field::heading(field::THEMES_HEADING, "Themes")`,
         - `Field::choice(field::THEME_CHOICE, "Installed themes", ..)` with
           `"Current"` followed by each entry in `session.themes` (enabled
           when `!session.themes.is_empty()`),
         - `Field::button(field::SAVE_THEME, "", "Save theme\u{2026}")`,
         - `Field::button(field::LOAD_THEME, "", "Load theme\u{2026}")`,
         - `Field::button(field::OPEN_THEMES_FOLDER, "", "Open themes folder")`,
         - `Field::note(field::THEMES_NOTE, "Themes in the themes folder appear above. Applying a theme on Global also applies its animations, if it includes any.")`.
    4. `crates/deskspace-app/src/settings/apply.rs`:
       - Add `pub fn apply_theme_to_global(config: &mut Config, theme:
         &ThemeFile)`: copies `theme.look.theme` into `config.theme` while
         keeping `config.theme.ui_mode` unchanged, sets
         `config.desktop.layout.gap` and `padding` from `theme.look`, and when
         `theme.animations` is `Some(animations)` sets
         `config.window.animations = animations`.
       - Handle `field::THEME_CHOICE` in `apply_field` (for index `> 0`, call
         `ThemeFile::load(&session.themes[index - 1].path)` and
         `apply_theme_to_global`, returning `Effect::Layout`) and in
         `space_edit` (returning `Command::SetSpaceLook { id, custom: true,
         look: theme.look }`).
    5. `crates/deskspace-app/src/settings/mod.rs` and
       `crates/deskspace-app/src/app/settings_session.rs`:
       - Add `ButtonAction::{SaveTheme, LoadTheme, OpenThemesFolder}`, map
         them in `button_action`, and handle them in `App::on_settings_button`.
       - When opening the settings window (`open_settings`), populate
         `session.themes = list_themes(&self.store.themes_dir())`.
       - `SaveTheme`: build a `ThemeFile` from the current scope (Global
         includes `Some(self.config.window.animations)`, a space sets
         `animations: None`), ask for a path with `ui.pick_save_path("Save
         theme", Some(&self.store.themes_dir()), "my-theme.deskspace-theme.toml",
         &[("DeskSpace theme", "*.deskspace-theme.toml")])`, call
         `theme.save(&path)` (alerting with `ui.alert` on error), and refresh
         `session.themes`.
       - `LoadTheme`: ask for a path with `ui.pick_file("Load theme",
         Some(&self.store.themes_dir()), &[("DeskSpace theme",
         "*.deskspace-theme.toml;manifest.toml")])`, load it with
         `ThemeFile::load` (alerting on error), and apply it to Global or the
         selected space just like `THEME_CHOICE`.
       - `OpenThemesFolder`: create `self.store.themes_dir()` with
         `std::fs::create_dir_all` and open it with `ui.open_path`.
  - Docs: update `CONTEXT.md` section 5 (`themes_dir`) and section 8 (the
    Themes page), the file map above (`theme_file.rs`), and the `ROADMAP.md`
    F-58 row.
  - Tests in `settings/`:
    - `apply_theme_to_global_preserves_ui_mode_and_updates_look_and_animations`,
    - `selecting_an_installed_theme_on_a_space_emits_set_space_look`,
    - `cancel_after_global_theme_load_restores_look_and_animations`,
    - and all four `every_*` guard tests in `settings/mod.rs`.
  - **[needs human]** check: save a customized space's look as a theme, load
    it on Global and on another space from the dropdown and with "Load
    theme…", verify Cancel undoes both look and animation changes, and verify
    "Open themes folder" opens the `themes` folder in Explorer.

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
blur steps on and off. T12 (done) moved spaces to composition, and T13
(done) made the blur follow every effect. One commit each, in order
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
    backdrop_spike`. It takes Ctrl+Alt+Shift+F, G, M, D, T, and Q, because
    a never-activated window gets no keys. Found while building it
    (Windows 11 build 26300): `CompositionMaskBrush` refuses a host
    backdrop brush as its source ("Unsupported source brush type"), so G
    sweeps with 80 strips of backdrop at graded opacities instead of a
    gradient mask. M sweeps the same edge the way T13 chose: an effect
    brush whose graph is D2D's AlphaMask, the backdrop its source and a
    linear gradient its mask. The compositor accepts that graph (no error
    at startup); whether it renders is part of the check below.
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
  - Findings so far (2026-10-18, maintainer, Windows 11 build 26300): the
    blur shows at once on a window that is never activated; the rounded
    clip is smooth; the window stays after Win+D; idle CPU reads 0.00 %.
    With "Transparency effects" off, the blur stays; so with the switch
    off at startup, spaces keep the accent blur (decided in D12, built in
    T12). The demos "never fully disappeared" and Dissolve "got stuck
    pixelated": the spike held the gone state for one frame only, and
    left the backdrop split into cells after Dissolve, whose seams look
    tiled (one more mark against strips and cells). Both are fixed (a hold
    at gone, and the strips come back after each demo), and the doc
    comment now says the tint stays in G, M, and D.
  - Still to check (maintainer): with the fixed spike, F, G, M, and D take
    the blur itself fully away and back, leaving the background sharp
    under the tint; M's edge is smooth, with no bands; Dissolve's cells
    show seams even at full. Energy Saver on, a 150 % monitor, and
    Windows 10. Beside an Acrylic space: how close are the two materials,
    and do they differ with "Transparency effects" off? Idle GPU.
  - Done when: the findings are recorded here (replace this bullet). If
    the blur does not render on a never-activated window on Windows 11,
    record that in D12, delete T12-T14, and delete the example.
- [ ] **T14. Black hole (platform).** **[needs decision]**: after T13. A
  dark disk opens on the desktop beside the space (a layered, click-through
  window, as `drag_ghost.rs` is) and the space spirals into it: a
  composition visual can rotate and scale with its blur, which the accent
  path cannot. Decide where the hole opens and what Windows 10 plays
  instead, then it is one `EFFECTS` row plus its frame math and drawing.

T19-T21 add the effects D12 plans (amended 2026-10-18). They animate
today's windows and do not wait for T1 or T12. Each one adds an effect the
same way:

1. `deskspace-core/src/animation.rs`: an `Effect` variant with a `///`
   doc, and an `EFFECTS` row naming its key and options. The key is saved
   in the config, so it is never renamed. A kind lists it only where the
   task says (`KINDS`; Startup and Delete take `EVERY_EFFECT`, so they get
   it with no edit).
2. `deskspace-app/src/settings/tables.rs`: a row in `EFFECT_LABELS` (grow
   the array's length by one).
3. `deskspace-platform/src/render/transition.rs`: the effect's arm in the
   frame `match` (the compiler asks for it), next to the effect it builds
   on. Pure frame math goes in its own `render/<effect>.rs` when it is
   more than a few lines, as `render/burn.rs` is.
4. Tests beside the pure math. Then add the effect to the T8 row in
   section V ("Each Startup effect") rather than adding a new row, unless
   the task asks for its own check.

- [ ] **T19. Bounce (core, platform).** **[needs human]**
  - Row: `effect(Effect::Bounce, "bounce", &[Direction])`. Not in Fold.
  - Frame math: the rect moves as Fly's does (from beyond `edge` of
    `work_area`, clipped to it; Automatic is the top, not the nearest
    edge), but its progress goes through a pure `bounce_ease(t: f32) ->
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
- [ ] **T20. Scroll (core, platform).** **[needs human]**
  - Row: `effect(Effect::Scroll, "scroll", &[Direction])`, and add
    `Effect::Scroll` to Fold's `effects` list in `KINDS`.
  - Rect: Roll's (it collapses into `edge`; Automatic is the top), made
    taller by the cylinder's radius toward the moving edge and clamped to
    the space's full rect, so the cylinder fits inside the window.
  - Pure math in `render/scroll.rs`: `scroll_radius(rolled_px, scale) ->
    f32`, growing with the square root of the rolled length between
    `SCROLL_RADIUS_MIN` (6 px) and `SCROLL_RADIUS_MAX` (28 px) at 96 DPI,
    times `scale`; and `scroll_strips(progress, body, radius, out: &mut
    [Strip; SCROLL_STRIPS])`, with `SCROLL_STRIPS` = 24. The cylinder's
    front half is cut into that many bands by angle θ from -π/2 to π/2.
    A band's destination is centred on the moving edge at `radius ·
    sin θ`, its source is the content row at arc length `radius · (θ +
    π/2)` past the edge, and its `shade` is `cos θ` (dark at the rims,
    full at the middle). `Strip { src: (f32, f32), dst: (f32, f32), shade:
    f32 }` along the rolling axis only.
  - Painter (`render/d2d/effects.rs`): the flat part draws as Roll's
    does. The cylinder draws each strip from the snapshot the animation
    already makes at its start (`DrawBitmap` from the source band to the
    destination band), then fills the band with black at `(1 - shade) ·
    0.6` alpha. No allocation per frame: the strips live in a fixed array
    in the painter.
  - Tests: the bands are contiguous and in order, lie within `radius` of
    the edge, start their sources at the edge's row, and their shade
    peaks in the middle band; at progress 0 nothing is rolled (radius at
    its minimum, no source rows used); the radius grows with the rolled
    length and stays within its range.
  - **[needs human]** check: Scroll on Fold (menu and title
    double-click), Startup, and Delete, from each edge, on every backdrop
    at 100 % and 150 %: the body curls into a round, shaded roll that
    grows as it winds; the items stay legible on the roll; reversing
    mid-way unrolls from where it is; folded, only the title bar is left.
- [ ] **T21. Magic lamp (core, platform).** **[needs decision]**: where
  the space pours to. Proposed: the DeskSpace tray icon when it is on the
  space's monitor (`Shell_NotifyIconGetRect`), else the middle of the
  bottom edge of the space's monitor work area. Once decided, remove this
  marker; the task is then **[needs human]**:
  - Row: `effect(Effect::MagicLamp, "magic_lamp", &[])`. Not in Fold.
  - Pure mesh in `render/lamp.rs`: `lamp_slices(progress, rect, target,
    out: &mut [Slice; LAMP_SLICES])`, `LAMP_SLICES` = 48 horizontal slices
    of the space. Phase one (progress 0 to 0.5) narrows the slices toward
    the target, the narrowing growing toward the target's side, with the
    sides following a smoothstep curve, so the outline bends like a lamp's
    smoke. Phase two (0.5 to 1) slides the slices along that outline into
    the target, each shrinking to nothing as it arrives. `Slice { src_y:
    (f32, f32), dst: Rect }`.
  - Window: `win32/lamp_overlay.rs`, a click-through, layered,
    never-activated overlay over the space's monitor work area, copied
    from `win32/effect_overlay.rs`. While Magic lamp plays, the space's own
    window is hidden and the overlay draws each slice from the start
    snapshot, then `UpdateLayeredWindow`. At the end of an in the window
    shows again and the overlay goes; at the end of an out the overlay
    goes (and the window is destroyed for a Delete). There is no blur
    while it plays.
  - Tests: at progress 0 the slices tile the space's rect exactly; at 1
    every slice is empty and at the target; widths only shrink as
    progress grows; slices never overlap and never leave the monitor's
    work area.
  - **[needs human]** check: Magic lamp on Startup and Delete, on every
    backdrop at 100 % and 150 %, with the space above, below, and beside
    the target: the space pours smoothly into the point and back out,
    with no rectangle of blur or tint left behind; reversing mid-way
    turns it around; no overlay window is left after it ends, after a
    delete, or after quitting mid-way.

## X. The debug window (ROADMAP D14, F-61)

Every planned task here is done (`archive/TASKS-done.md`); what remains
are their on-screen checks in section V. D14 is the spec. These rules hold
for any new debug control:

1. **One way to change things.** An action calls the `App` method or sends
   the `Command` that the menus, settings, or drops already use. If none
   fits, stop and ask. Never write to `self.config.desktop` or move a file
   any other way.
2. **Session only.** Nothing here adds a config field or saves a debug
   toggle. Every toggle starts off, or reads the live state, on each run.
3. **No undo entries.** Debug actions record none (D14), even after
   section U lands.
4. **A control is three edits**: a `DebugId` variant
   (`app/debug/ids.rs`, the next free value, plus its `ALL` entry), a
   `DEBUG_CONTROLS` row (`app/debug/table.rs`: id, section, label), and an
   arm in both `App::debug_control` (what it shows now) and
   `App::on_debug_control` (what it does). The compiler and
   `every_control_has_one_row` point at whatever is missing.
5. **Ask before anything destructive** with `self.debug_confirm(ui,
   question)`. It adds the config path and the real-config warning, so a
   handler never writes those lines itself.
6. **Log what an action did** with one `tracing::info!` line starting
   `debug:` (for example `debug: deleted 4 spaces`), so the console shows
   it.
7. **[needs human] checks** run with `./scripts/dev.ps1 -Fresh --debug`
   (a scratch config) unless the check says otherwise, and add a row to
   section V.
8. **One file per section** in `app/debug/` holds its actions; `mod.rs`
   keeps only the framework and the two routing matches.

## P. Extensibility: textures, custom animations, verbs, and scripts (ROADMAP D15, F-21, F-58, F-59, F-62)

Users extend DeskSpace by dropping files or pack folders into three folders
beside `deskspace.toml`: `themes/`, `animations/`, and `scripts/`. No native
DLLs are loaded; custom animations and scripts run in a sandboxed `rhai`
engine inside a new `#![forbid(unsafe_code)]` crate `deskspace-ext`. Every
change to the model still flows through `Command::apply` or existing `App`
methods. Do the tasks in order within each track (`P1`-`P3` after `C10c`,
`P4`-`P7`, `P8`-`P10`, then `P11`-`P13`).

- [ ] **P1. Texture layer schema in theme manifests (core, config; pure).**
  Needs C10a.
  - Step 1 (`crates/deskspace-core/src/theme.rs`): add
    - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)] #[serde(rename_all = "snake_case")] pub enum TextureMode { #[default] Tile, Cover, NineSlice { left: u16, top: u16, right: u16, bottom: u16 } }`,
    - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct TextureLayer { pub file: String, #[serde(default)] pub mode: TextureMode, #[serde(default = "default_texture_opacity")] pub opacity: u8 }` (`default_texture_opacity() -> u8 { 100 }`), with `pub fn clamped(self) -> Self` clamping `opacity` to `0..=100`, and
    - `#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)] #[serde(default)] pub struct SpaceTextures { pub body: Option<TextureLayer>, pub title_bar: Option<TextureLayer>, pub frame: Option<TextureLayer> }` with `pub fn is_empty(&self) -> bool` and `pub fn clamped(self) -> Self`.
  - Step 2 (`crates/deskspace-core/src/theme.rs`): add `#[serde(default,
    skip_serializing_if = "SpaceTextures::is_empty")] pub textures:
    SpaceTextures` to `ThemeManifest`, and clamp it inside
    `ThemeManifest::clamped`. Re-export `SpaceTextures`, `TextureLayer`, and
    `TextureMode` from `deskspace-core/src/lib.rs`.
  - Step 3 (`crates/deskspace-config/src/theme_file.rs`): add `pub fn
    resolve_texture_path(pack_dir: &Path, relative: &str) -> Result<PathBuf,
    ConfigError>`. Reject empty strings, absolute paths, `Prefix` or `RootDir`
    components, and `ParentDir` (`..`) components so a theme pack can only
    reference image files inside its own folder. When `discover_themes` loads
    a pack folder `themes/<slug>/manifest.toml`, validate every present layer's
    `file` with `resolve_texture_path` and check `path.is_file()`; if any check
    fails, log a `tracing::warn!` and clear that layer (`None`) so the rest of
    the theme still loads. On a single `.deskspace-theme.toml` file, ignore
    `textures` (set `textures = SpaceTextures::default()`) with a warning if
    non-empty, because single files have no pack folder.
  - Tests: TOML round-trip for `SpaceTextures` with `tile`, `cover`, and
    `nine_slice`; `clamped` caps `opacity` at 100; `resolve_texture_path`
    accepts `"bg.png"` and `"assets/frame.png"` and rejects `"../secret.png"`,
    `"/abs.png"`, and `"C:\\abs.png"`.

- [ ] **P2. Load image files with WIC (platform).** Needs P1.
  - Step 1 (`crates/deskspace-platform/src/render/d2d/bitmap.rs`): add
    `pub(crate) fn decode_file(wic: &IWICImagingFactory, path: &Path) ->
    Option<Icon>`. Call `wic.CreateDecoderFromFilename` with a null-terminated
    UTF-16 path (`GENERIC_READ`, `WICDecodeMetadataCacheOnDemand`), grab frame
    0 with `GetFrame(0)`, convert it to `GUID_WICPixelFormat32bppPBGRA` via
    `CreateFormatConverter` (the same format `to_premultiplied_bgra` in
    `bitmap.rs` uses), copy the pixels into a `Vec<u8>` with `CopyPixels`, and
    return `Icon::from_bgra(width, height, pixels)`. Return `None` (and log a
    `tracing::warn!`) if any step fails or if `width` or `height` exceeds 4096
    px.
  - Step 2 (`crates/deskspace-platform/src/render/scene.rs`): add
    - `#[derive(Debug, Clone, PartialEq)] pub struct SceneTexture { pub bitmap: Icon, pub mode: TextureMode, pub opacity: f32 }`, and
    - `#[derive(Debug, Clone, Default, PartialEq)] pub struct SceneTextures { pub body: Option<SceneTexture>, pub title_bar: Option<SceneTexture>, pub frame: Option<SceneTexture> }`,
    and add `pub textures: SceneTextures` to `SpaceScene` (defaulting to
    `SceneTextures::default()`). Re-export `SceneTexture` and `SceneTextures`
    from `render/mod.rs` and `deskspace-platform/src/lib.rs`.
  - Step 3 (`crates/deskspace-platform/src/win32/runtime/mod.rs`): add a small
    texture cache `textures: HashMap<(PathBuf, Option<SystemTime>), Icon>` on
    `Runtime` and expose `pub fn load_texture(&mut self, path: &Path) ->
    Option<Icon>` on `Ui` (`ui_chrome.rs`). It stats `path` for its `modified`
    timestamp, returns a clone of the cached `Icon` on a hit, and otherwise
    decodes via `decode_file(&self.rt.graphics.wic, path)` and caches the
    result. Add `pub fn clear_texture_cache(&mut self)` on `Ui` and call it
    alongside `clear_icon_cache`.
  - Tests: in `bitmap.rs`, write a tiny 2×2 PNG to `std::env::temp_dir()` (a
    hardcoded valid PNG byte slice), decode it with `decode_file`, and check
    `width == 2`, `height == 2`, and `pixels.len() == 16`; verify a missing
    path returns `None`.

- [ ] **P3. Paint space textures: tile, cover, and nine-slice (platform, app).**
  Needs P2 and C10c. **[needs human]**
  - Step 1 (`crates/deskspace-platform/src/render/d2d/geometry.rs`): add two
    pure geometry helpers so they can be unit-tested without Direct2D:
    - `pub(super) fn cover_source_rect(src_w: f32, src_h: f32, dst_w: f32,
      dst_h: f32) -> D2D_RECT_F`: returns the centered crop rectangle in
      source-bitmap coordinates that matches `dst_w / dst_h` aspect ratio.
    - `pub(super) fn nine_slice_patches(src_w: f32, src_h: f32, dst: Rect,
      insets: (u16, u16, u16, u16)) -> [(D2D_RECT_F, D2D_RECT_F); 9]`: returns
      the nine `(source_rect, dest_rect)` pairs (4 corners unscaled, 4 edges
      stretched along one axis, center stretched along both; clamp insets so
      `left + right <= src_w` and `dst.w`, and `top + bottom <= src_h` and
      `dst.h`).
  - Step 2 (`crates/deskspace-platform/src/render/d2d/mod.rs`): add
    `Painter::draw_texture(&mut self, dc: &ID2D1DeviceContext, tex:
    &SceneTexture, dst: Rect, clip_radius: f32)`. Obtain the GPU bitmap via
    `self.icon_bitmap(dc, &tex.bitmap)` (which already caches `ID2D1Bitmap` by
    `Icon::id`). Push a rounded-rect layer or axis-aligned clip when
    `clip_radius > 0.0`, then draw:
    - `TextureMode::Cover`: one `dc.DrawBitmap` call using `cover_source_rect`
      and `tex.opacity`.
    - `TextureMode::NineSlice { left, top, right, bottom }`: loop over
      `nine_slice_patches` and call `dc.DrawBitmap` for each patch with
      non-empty destination width and height.
    - `TextureMode::Tile`: create an `ID2D1BitmapBrush1` with
      `D2D1_EXTEND_MODE_WRAP` on X and Y and `tex.opacity`, translated to
      `(dst.x, dst.y)`, and call `dc.FillRectangle`.
    In `draw_scene`, paint `scene.textures.body` right after the body tint fill,
    `scene.textures.title_bar` right after the title-bar band fill (clipped to
    the title-bar rect), and `scene.textures.frame` right after the border
    stroke.
  - Step 3 (`crates/deskspace-app/src/scene.rs` and `settings/apply.rs`): store
    an optional `active_theme_pack: Option<(PathBuf, SpaceTextures)>` on `App`
    (set when applying a pack theme in `C10c`, cleared when switching to a
    built-in preset or single-file theme). When building a `SpaceScene`, resolve
    each present `TextureLayer` through `ui.load_texture` into `SceneTextures`.
  - Tests: `cover_source_rect` crops wide and tall sources symmetrically;
    `nine_slice_patches` keeps corner sizes exact and clamps oversized insets.
  - **[needs human]** check: drop a test theme pack with a tiled body PNG, a
    cover title-bar PNG, and a nine-slice frame PNG into `themes/`; apply it
    from Settings > Themes; resize and fold the space at 100 % and 150 % DPI
    and check that corners stay crisp and textures follow the rounded window.

- [ ] **P4. Create the `deskspace-ext` crate and sandboxed Rhai engine (ext).**
  - Step 1 (`Cargo.toml`, `deny.toml`): add `crates/deskspace-ext` to the
    workspace members (`deskspace-ext = { path = "crates/deskspace-ext" }`) and
    add `rhai` to `[workspace.dependencies]` with default features disabled and
    only `std` and `f32_float` enabled (pure Rust, MIT/Apache-2.0). Run
    `cargo deny check all` to confirm license and ban gates pass.
  - Step 2 (`crates/deskspace-ext/Cargo.toml` and `src/lib.rs`): start
    `src/lib.rs` with `#![forbid(unsafe_code)]`. Depend on `deskspace-core`,
    `rhai`, `serde`, `toml`, and `tracing`. Update `docs/ENGINEERING.md`
    section 3's dependency line (`app -> {config, platform, ext, update} ->
    core` and `platform -> ext`), `docs/CONTEXT.md` section 2's crate table,
    and the file map in `docs/TASKS.md`.
  - Step 3 (`crates/deskspace-ext/src/engine.rs`): build `pub struct Sandbox {
    engine: rhai::Engine }`. In `Sandbox::new()`:
    - Call `engine.set_max_operations(50_000)` (a tighter per-frame limit of
      `10_000` is used for animation `frame`/`slices` calls via a second
      engine or `on_progress`), `engine.set_max_call_levels(16)`,
      `engine.set_max_expr_depths(32, 16)`, `engine.set_max_string_size(4096)`,
      `engine.set_max_array_size(256)`, and `engine.set_max_map_size(64)`.
    - Disable `eval` (`engine.disable_symbol("eval")`).
    - Register pure `f32` math helpers so scripts and shaders stay concise:
      `lerp(a, b, t)`, `clamp(x, lo, hi)`, `smoothstep(edge0, edge1, x)`,
      `ease_out_cubic(t)`, `ease_out_back(t)`, `ease_out_bounce(t)`, `sin(x)`,
      `cos(x)`, `abs(x)`, `min(a, b)`, `max(a, b)`, and `hash(x, y, seed)` (a
      deterministic 0.0..1.0 float hash for procedural noise).
  - Step 4 (`crates/deskspace-ext/src/diagnostic.rs`): add `#[derive(Debug,
    Clone, PartialEq, Eq)] pub struct ExtDiagnostic { pub slug: String, pub
    path: PathBuf, pub line: Option<usize>, pub message: String }` so load and
    runtime errors have one shape across animations and scripts.
  - Tests: a script with `loop {}` stops with a max-operations error; `eval` is
    rejected at compile time; `lerp(0.0, 10.0, 0.25)` returns `2.5`; `hash(3,
    4, 1)` is deterministic and in `0.0..=1.0`.

- [ ] **P5. Animation pack manifest and Rhai `frame` / `field` / `slices`
  evaluator (ext; pure).** Needs P4.
  - Step 1 (`crates/deskspace-ext/src/animation.rs`): define
    - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct
      AnimationManifest { pub name: String, #[serde(default)] pub author:
      String, #[serde(default)] pub version: String, #[serde(default =
      "default_duration_ms")] pub default_duration_ms: u16 }` (`220`),
    - `#[derive(Debug, Clone, Copy, PartialEq)] pub struct FramePose { pub
      opacity: f32, pub offset_x: f32, pub offset_y: f32, pub scale_x: f32, pub
      scale_y: f32, pub clip_top: f32, pub clip_bottom: f32, pub clip_left:
      f32, pub clip_right: f32, pub threshold: f32, pub edge_width: f32, pub
      edge_r: u8, pub edge_g: u8, pub edge_b: u8 }` (defaults: `opacity = 1.0`,
      `scale_x = 1.0`, `scale_y = 1.0`, clips `0.0..1.0`, `threshold = u`),
    - `#[derive(Debug, Clone, Copy, PartialEq)] pub struct SlicePose { pub
      top: f32, pub bottom: f32, pub offset_x: f32, pub offset_y: f32, pub
      opacity: f32 }`, and
    - `#[derive(Debug, Clone)] pub struct CustomAnimation { pub slug: String,
      pub manifest: AnimationManifest, ast: Arc<rhai::AST>, has_field: bool,
      has_slices: bool }`.
  - Step 2 (`crates/deskspace-ext/src/animation.rs`): implement compilation and
    evaluation on `CustomAnimation`:
    - `pub fn compile(sandbox: &Sandbox, slug: String, manifest:
      AnimationManifest, source: &str, path: &Path) -> Result<Self,
      ExtDiagnostic>`: compiles `source` into an `AST`, checks that at least one
      of `fn frame(u, ctx)`, `fn field(x, y, ctx)`, or `fn slices(u, ctx)`
      exists, and records `has_field` and `has_slices`.
    - `pub fn eval_frame(&self, sandbox: &Sandbox, u: f32, w: f32, h: f32,
      way_in: bool, detail: u8) -> Result<FramePose, String>`: calls `frame(u,
      ctx)` (with a 10,000-op cap) where `ctx` is a Rhai map holding `w`, `h`,
      `way` (`"in"` or `"out"`), and `detail`, clamps all returned floats (`0..=1`
      for opacity and clips, `-4096..=4096` for offsets, `0.01..=4.0` for
      scales), and returns the default `FramePose` (with `threshold = u`) when
      `fn frame` is omitted.
    - `pub fn eval_field(&self, sandbox: &Sandbox, cols: usize, rows: usize,
      detail: u8) -> Result<Vec<u8>, String>`: runs once when an animation
      starts, calling `field(x, y, ctx)` for each cell `(col + 0.5) / cols`,
      `(row + 0.5) / rows` (capped at `64 × 64` cells and `200_000` total ops)
      and quantizing each `0.0..=1.0` float to `1..=254` `u8` (matching
      `NoiseMask`'s cell format in `render/d2d/effects.rs`).
    - `pub fn eval_slices(&self, sandbox: &Sandbox, u: f32, w: f32, h: f32,
      way_in: bool, detail: u8) -> Result<Vec<SlicePose>, String>`: calls
      `slices(u, ctx)` and returns up to 64 clamped horizontal strips.
  - Step 3 (`crates/deskspace-ext/src/animation.rs`): add `pub fn
    discover_animations(sandbox: &Sandbox, dirs: &[PathBuf]) ->
    (Vec<CustomAnimation>, Vec<ExtDiagnostic>)`, scanning `<dir>/<slug>/manifest.toml`
    + `effect.rhai` in alphabetical slug order (first directory wins on
    duplicate slugs so user `animations/` overrides bundled theme animations).
  - Tests: compile and evaluate a bounce `frame(u, ctx)` script, a radial
    `field(x, y, ctx)` script (check the center cell vs corner cell), and a
    4-strip `slices(u, ctx)` script; verify missing entry points and syntax
    errors return an `ExtDiagnostic` with a line number.

- [ ] **P6. Wire custom animations into `EffectChoice`, `Play`, and the D2D
  painter (core, platform, app).** Needs P5.
  - Step 1 (`crates/deskspace-core/src/settings.rs`): change `EffectChoice` so
    it can hold a custom slug alongside the built-in effects: add `Custom(String)`
    to `EffectChoice` (or serialize `"custom:<slug>"` via custom `Serialize` /
    `Deserialize` impls so existing `"roll"`, `"fade"`, `"same_as_startup"`, etc.
    strings in `deskspace.toml` are 100 % unchanged). Add `pub fn
    clamped_with(self, known_custom: &[String]) -> Self` on `Animations`: if a
    `Custom(slug)` is not in `known_custom`, fall back to that `AnimKind`'s
    default effect (`Roll` for Startup/Fold, `SameAsStartup` for Delete).
  - Step 2 (`crates/deskspace-platform/src/render/transition.rs` and
    `render/d2d/effects.rs`): add `PlayEffect::Custom(Arc<CustomAnimation>)` (depend
    on `deskspace-ext` in `deskspace-platform`). When `SpaceAnim::start` begins
    a `PlayEffect::Custom(anim)`:
    - If `anim.has_field()`, evaluate `anim.eval_field` once at start and upload
      the resulting `Vec<u8>` grid into a `CellMask` bitmap (the same helper
      `NoiseMask` and `BurnMask` use in `effects.rs`).
    - On each 15 ms tick in `win32/space_animation.rs`, evaluate
      `anim.eval_frame` (and `anim.eval_slices` if `anim.has_slices()`) to build
      the `Transition` frame: apply `offset_x`/`offset_y`, `scale_x`/`scale_y`,
      `opacity`, `clip_*`, threshold the precomputed `CellMask` with `threshold`
      (plus an optional colored edge band when `edge_width > 0.0`), or draw the
      horizontal strips from `SlicePose`. If a script errors at runtime, log one
      `tracing::warn!` and finish the transition cleanly at `u = 1.0`.
  - Step 3 (`crates/deskspace-app/src/app/animation.rs`): in `play_for`, resolve
    `EffectChoice::Custom(slug)` from the loaded custom animations list into
    `PlayEffect::Custom(Arc::clone(anim))`, falling back to `Roll`/`Fade` if
    missing.
  - Tests: `EffectChoice` round-trips `"roll"`, `"same_as_startup"`, and
    `"custom:jelly"` through TOML; `clamped_with` keeps `"custom:jelly"` when
    present and resets it to the kind's default when absent.

- [ ] **P7. List custom animations in Settings and Debug, with Reload (config,
  platform, app).** Needs P6. **[needs human]**
  - Step 1 (`crates/deskspace-config/src/lib.rs`): add `pub fn
    animations_dir(&self) -> PathBuf` (`self.path.parent().unwrap().join("animations")`)
    on `ConfigStore`.
  - Step 2 (`crates/deskspace-app/src/app/extensions.rs`, new module): hold the
    `Sandbox`, the loaded `Vec<Arc<CustomAnimation>>`, and `Vec<ExtDiagnostic>`
    on `App`. Add `App::reload_extensions(&mut self, ui: &mut Ui)` which scans
    `store.themes_dir()`, `store.animations_dir()`, and any `animations/`
    subfolders inside discovered theme packs (`themes/<theme>/animations/`),
    clamps `config.window.animations` with `clamped_with`, and logs the counts.
    Call it at startup and whenever the Settings window opens.
  - Step 3 (`crates/deskspace-app/src/settings/pages.rs` and `apply.rs`): append
    each loaded custom animation (`"Custom: <manifest.name>"`) to the Effect
    dropdowns on the Startup, Delete, and Fold pages (`FieldId::StartupEffect`,
    `DeleteEffect`, `FoldEffect`). When a custom animation is chosen, grey out
    built-in-only rows (`Direction`, `Edge`, `StartFrom`, `Detail` unless the
    script uses `field`), keep `Duration` (seeded to `default_duration_ms` when
    first picked), and add an `"Open animations folder"` button
    (`ButtonAction::OpenAnimationsFolder`) at the bottom of the Animations pages
    (creating `animations/` first if missing, then calling `ui.open_path`).
  - Step 4 (`crates/deskspace-app/src/app/debug/`): add a `"Reload extensions"`
    button (`DebugId::ReloadExtensions`) in `DebugSectionId::Config` and include
    custom animations in `X18`'s Debug → Animations preview.
  - Tests: `effect_options` includes loaded custom animations after the built-in
    choices and maps their dropdown index back to `EffectChoice::Custom(slug)`.
  - **[needs human]** check: drop a folder `animations/jelly/` with a
    `manifest.toml` and `effect.rhai` into the scratch config's `animations/`
    folder; click "Reload extensions" in Debug (or reopen Settings); select
    "Custom: Jelly" on Startup and play it on a space from Debug → Animations
    and by hiding/showing spaces; delete the folder and reload, and check that
    Startup falls back cleanly without crashing.

- [ ] **P8. The `Verb` enum and `--verb` CLI flag (app; pure).**
  - Step 1 (`crates/deskspace-app/src/verb.rs`, new module): define
    `#[derive(Debug, Clone, PartialEq, Eq)] pub enum Verb { Show, Hide, Toggle,
    Settings, NewSpace, Reload, ApplyTheme(String), RunScript(String) }`.
    Implement:
    - `pub fn to_wire(&self) -> String`: encodes the verb as a single line
      (`"show"`, `"hide"`, `"toggle"`, `"settings"`, `"new-space"`, `"reload"`,
      `"apply-theme <name>"`, `"run-script <slug>"`).
    - `pub fn from_wire(s: &str) -> Result<Self, String>`: parses that line,
      rejecting empty arguments, control characters (`c.is_control()`), and
      strings longer than 256 bytes.
  - Step 2 (`crates/deskspace-app/src/cli.rs`): add `pub verb: Option<Verb>` to
    `Cli`. Parse `--verb <command>` (where `<command>` is either a single token
    like `toggle` or a verb with its argument like `--verb apply-theme
    "Nord Dark"` or `--verb "apply-theme Nord Dark"`). Keep `--show` and
    `--settings` working as shorthand aliases that set `verb = Some(Verb::Show)`
    and `verb = Some(Verb::Settings)`. Update `--help` and `docs/CONTEXT.md`
    section 8's CLI flags list.
  - Tests: every `Verb` variant round-trips through `to_wire` and `from_wire`;
    `parse_args` parses `--verb toggle`, `--verb apply-theme Nord`, `--show`,
    and `--settings`, and rejects unknown verbs or missing arguments with a
    clear error message.

- [ ] **P9. Send and receive verbs over `WM_COPYDATA` (platform).** Needs P8.
  - Step 1 (`crates/deskspace-platform/src/input.rs`): add `Verb(String)` to
    `Event`.
  - Step 2 (`crates/deskspace-platform/src/win32/controller.rs`): define a
    constant `COPYDATA_VERB_TAG: usize = 0x4453_5642` (`"DSVB"`). Add `pub fn
    send_verb(wire: &str) -> bool` (re-exported from `deskspace-platform/src/lib.rs`):
    find the running `DeskSpaceController` window with `FindWindowW`, build a
    `COPYDATASTRUCT { dwData: COPYDATA_VERB_TAG, cbData, lpData }` pointing to
    the UTF-8 bytes of `wire`, and send it with `SendMessageTimeoutW`
    (`SMTO_ABORTIFHUNG | SMTO_BLOCK`, 2000 ms timeout). Keep `wake_existing_instance`
    calling `send_verb("show")` (or `send_verb("settings")`) so there is only
    one IPC path.
  - Step 3 (`crates/deskspace-platform/src/win32/controller.rs`): handle
    `WM_COPYDATA` in the controller's `wnd_proc`:
    - Check `cds.dwData == COPYDATA_VERB_TAG` and `cds.cbData <= 256`.
    - Trust check: if `wparam != 0`, query the sender window's process id with
      `GetWindowThreadProcessId` and verify `ProcessIdToSessionId` matches the
      current process's session id; ignore the message if it does not. (UIPI
      already blocks lower-integrity processes from sending `WM_COPYDATA` to
      DeskSpace unless explicitly allowed.)
    - Decode the UTF-8 slice into a `String`, push `Event::Verb(wire)` onto the
      runtime event queue, and return `LRESULT(1)`.
  - Tests: unit-test the UTF-8 payload validation helper (rejects > 256 bytes
    and invalid UTF-8).

- [ ] **P10. Handle `Event::Verb` in `bootstrap.rs` and `App` (app).** Needs
  P8, P9, C10c, and P7. **[needs human]**
  - Step 1 (`crates/deskspace-app/src/bootstrap.rs`): when the single-instance
    mutex is already held, send `cli.verb.as_ref().unwrap_or(&Verb::Show).to_wire()`
    via `deskspace_platform::send_verb` and exit `Ok(())`. When starting as the
    primary instance and `cli.verb` is `Some(verb)` (other than `Show`), queue
    `Event::Verb(verb.to_wire())` after `Event::Started`.
  - Step 2 (`crates/deskspace-app/src/app/extensions.rs` and `app.rs`): route
    `Event::Verb(wire)` through `Verb::from_wire(&wire)` and dispatch onto the
    existing `App` methods (never a second code path):
    - `Verb::Show` → `self.set_visible(ui, true)`
    - `Verb::Hide` → `self.set_visible(ui, false)`
    - `Verb::Toggle` → `self.set_visible(ui, !self.visible)`
    - `Verb::Settings` → `self.open_settings(ui, SettingsScope::Global)`
    - `Verb::NewSpace` → `self.create_space(ui, ui.cursor_pos())`
    - `Verb::Reload` → `self.reload_extensions(ui)`
    - `Verb::ApplyTheme(name)` → look up `name` (case-insensitive match on theme
      name or slug) and call the same theme-apply method `C10c` uses; log a
      `tracing::warn!` if not found.
    - `Verb::RunScript(slug)` → log a `tracing::info!` placeholder until `P12`
      wires script execution.
  - Tests: dispatching `Event::Verb("toggle".into())` flips `app.visible`;
    `"new-space"` adds a space through `Command::CreateSpace`.
  - **[needs human]** check: with DeskSpace running, run `deskspace --verb hide`,
    `deskspace --verb show`, `deskspace --verb toggle`, `deskspace --verb
    new-space`, `deskspace --verb settings`, and `deskspace --verb apply-theme
    "<theme>"` from a second terminal and check that each acts on the running
    instance at once.

- [ ] **P11. Script manifest, read/action context, and sandboxed runner (ext;
  pure).** Needs P4.
  - Step 1 (`crates/deskspace-ext/src/script.rs`): define
    - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct
      ScriptManifest { pub name: String, #[serde(default)] pub description:
      String, #[serde(default)] pub author: String, #[serde(default)] pub
      version: String, #[serde(default)] pub on_desktop_item: bool,
      #[serde(default)] pub on_action: bool }`,
    - `#[derive(Debug, Clone, PartialEq, Eq)] pub struct ScriptItem { pub id:
      u64, pub name: String, pub ext: String, pub kind: String, pub path:
      PathBuf, pub target_ext: String, pub size_bytes: u64, pub age_days: u64,
      pub space_title: Option<String> }` (where `ext` is lowercase without the
      leading dot, `kind` is `"file"`, `"folder"`, `"shortcut"`, or `"url"`, and
      `target_ext` is the shortcut target's lowercase extension when `kind ==
      "shortcut"`),
    - `#[derive(Debug, Clone, PartialEq, Eq)] pub struct ScriptSpace { pub id:
      u64, pub title: String, pub item_count: usize, pub folded: bool, pub
      locked: bool }`, and
    - `#[derive(Debug, Clone, PartialEq, Eq)] pub enum ScriptOp { RouteToSpace {
      space_title: String, create_if_missing: bool }, MoveItemToSpace { item_id:
      u64, space_title: String, create_if_missing: bool }, CreateSpace { title:
      String }, RenameSpace { from: String, to: String }, SetFolded {
      space_title: String, folded: bool }, SetLocked { space_title: String,
      locked: bool }, SortSpace { space_title: String, by: SortBy, ascending:
      bool }, FitSpace { space_title: String }, ApplyTheme { name: String },
      DeleteSpace { space_title: String }, RecycleItem { item_id: u64 } }`, with
      `pub fn is_destructive(&self) -> bool` returning `true` for `DeleteSpace`
      and `RecycleItem`.
  - Step 2 (`crates/deskspace-ext/src/script.rs`): implement `CustomScript`:
    - `pub fn compile(sandbox: &Sandbox, slug: String, manifest: ScriptManifest,
      source: &str, path: &Path) -> Result<Self, ExtDiagnostic>`.
    - Register read getters on `item` (`item.name`, `item.ext`, `item.kind`,
      `item.target_ext`, `item.size_bytes`, `item.age_days`, `item.space`) and
      action methods on `ctx` (`ctx.spaces()`, `ctx.items(space_title)`,
      `ctx.route_to(title)`, `ctx.route_to_existing(title)`,
      `ctx.move_item(id, title)`, `ctx.create_space(title)`,
      `ctx.rename_space(from, to)`, `ctx.set_folded(title, bool)`,
      `ctx.set_locked(title, bool)`, `ctx.sort_space(title, by, asc)`,
      `ctx.fit_space(title)`, `ctx.apply_theme(name)`,
      `ctx.delete_space(title)`, `ctx.recycle_item(id)`, and `ctx.log(msg)`).
      Each method pushes a `ScriptOp` into a per-call vec (capped at 64 ops per
      run) rather than mutating anything directly.
    - `pub fn run_on_desktop_item(&self, sandbox: &Sandbox, item: &ScriptItem,
      spaces: &[ScriptSpace]) -> Result<Vec<ScriptOp>, ExtDiagnostic>` and
      `pub fn run_on_action(&self, sandbox: &Sandbox, spaces: &[ScriptSpace],
      items: &[ScriptItem]) -> Result<Vec<ScriptOp>, ExtDiagnostic>`.
    - `pub fn discover_scripts(sandbox: &Sandbox, dir: &Path) ->
      (Vec<CustomScript>, Vec<ExtDiagnostic>)` scanning `scripts/<slug>/manifest.toml`
      + `script.rhai`.
  - Tests: a rule script that routes `.pdf` and `.docx` items to `"Documents"`
    and `.lnk` game shortcuts to `"Games"` returns the expected `RouteToSpace`
    ops; an action script that sorts every space by name and folds empty spaces
    returns `SortSpace` and `SetFolded` ops; a script calling `delete_space`
    marks `is_destructive() == true`.

- [ ] **P12. Execute `ScriptOp`s in the app with destructive confirmation and
  single undo (config, app).** Needs P10 and P11. **[needs human]**
  - Step 1 (`crates/deskspace-config/src/lib.rs`): add `pub fn scripts_dir(&self)
    -> PathBuf` (`self.path.parent().unwrap().join("scripts")`) on
    `ConfigStore`. Load scripts inside `App::reload_extensions` (`P7`).
  - Step 2 (`crates/deskspace-app/src/app/extensions.rs`): implement
    `App::apply_script_ops(&mut self, ui: &mut Ui, script_name: &str,
    default_item: Option<&Path>, ops: Vec<ScriptOp>)`:
    - If any op in `ops` has `op.is_destructive()`, build a human-readable
      summary listing the spaces to delete and files to send to the Recycle Bin
      and call `ui.confirm("DeskSpace Script", &question)` once before running
      any destructive op; if the user cancels, skip all destructive ops and log
      `tracing::info!`.
    - Execute each op through the existing `App` and `Command` methods:
      `RouteToSpace` / `MoveItemToSpace` finds the space by case-insensitive
      title (or creates it via `create_space` when `create_if_missing` is true)
      and moves the desktop file into that space with `import_files_to_space`;
      `CreateSpace`, `RenameSpace`, `SetFolded`, `SetLocked`, `SortSpace`, and
      `FitSpace` commit the corresponding `Command`; `ApplyTheme` calls `C10c`'s
      theme apply; `DeleteSpace` calls `delete_space_now(ui, id,
      StoredFiles::ToDesktop)`; `RecycleItem` calls `delete_items`. When section
      `U` (`U3`) is present, group the non-destructive `Command`s from one
      script run into a single undo entry.
  - Step 3 (`crates/deskspace-app/src/app/files.rs`, `menus.rs`, and
    `extensions.rs`):
    - When `resync_desktop_layer` (or `Event::FolderChanged`) sees a newly
      added loose desktop item, run enabled `on_desktop_item` scripts in slug
      order until the first `RouteToSpace` op matches, and apply it.
    - Add a `"Scripts ▸"` submenu on the space context menu and tray menu
      whenever at least one `on_action` script is enabled, plus a `"Run all
      desktop rules now"` item, and wire `Verb::RunScript(slug)` from `P10` to
      run that script's `on_action` (or run its `on_desktop_item` across current
      loose desktop items).
  - Tests: `apply_script_ops` with `CreateSpace`, `SortSpace`, and `SetFolded`
    updates `app.desktop` through `Command::apply`; destructive ops are skipped
    when a confirmation flag is false.
  - **[needs human]** check: drop a rule script into `scripts/auto-docs/` that
    routes `.txt` files to `"Docs"`; with takeover on, create a new `.txt` file
    on the Desktop and check that `"Docs"` is created (if missing) and receives
    the file; trigger an action script from the tray menu's `Scripts ▸` submenu
    and via `deskspace --verb run-script <slug>`.

- [ ] **P13. The Global `Scripts` settings page and `--debug` live folder
  watching (core, platform, app).** Needs P12. **[needs human]**
  - Step 1 (`crates/deskspace-core/src/settings.rs`): add `#[serde(default,
    skip_serializing_if = "Vec::is_empty")] pub disabled_scripts: Vec<String>`
    to `WindowSettings` so all discovered scripts are enabled by default and
    toggling one off persists its slug in `deskspace.toml`.
  - Step 2 (`crates/deskspace-app/src/settings/`): add a Global-only `Scripts`
    page (`page::SCRIPTS`, next index in `ALL_PAGES`, hidden under per-space
    scope just like `General` and `Hotkeys`). Show:
    - a toggle row per discovered script (`"<manifest.name> —
      <manifest.description>"`),
    - a `"Run desktop rules now"` button and `"Reload extensions"` button,
    - an `"Open scripts folder"` button (`ButtonAction::OpenScriptsFolder`,
      creating `scripts/` if missing and calling `ui.open_path`), and
    - a read-only diagnostic note at the bottom listing any `ExtDiagnostic`
      load/runtime errors with their file and line number (or `"No script or
      animation errors."` when clean).
  - Step 3 (`crates/deskspace-platform/src/win32/folder_watch.rs` and
    `deskspace-app/src/bootstrap.rs`): when `--debug` is active, start a folder
    watcher on the config directory covering `themes/`, `animations/`, and
    `scripts/` (reusing `FolderWatch` with `FILE_NOTIFY_CHANGE_FILE_NAME |
    FILE_NOTIFY_CHANGE_DIR_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE` and a 150 ms
    debounce) that posts `Event::ExtensionsChanged`, calling
    `app.reload_extensions(ui)` automatically whenever a file is saved.
  - Tests: `disabled_scripts` round-trips in `WindowSettings`; the `Scripts`
    page is present on `SettingsScope::Global` and omitted on
    `SettingsScope::Space`.
  - **[needs human]** check: open Settings > Scripts; toggle a script off and on
    and check `deskspace.toml`; click "Open scripts folder"; with `--debug` on,
    edit a theme or animation file in Notepad and save it, and verify the debug
    log prints the reload line and the space updates within 200 ms.

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
| Border settings | Settings > Appearance: "Border thickness" from 1 to 8 px thickens the border evenly on every side, at 100 % and 150 %, on Acrylic with Windows corners and on Clear at 16 px; the slider greys out with "Show window border" off. Settings > Colors: the border is white in dark mode and black in light mode, also with custom text colors. "Border color" set to Windows accent colors it with the accent; set to Custom, it starts from the border on screen and "Custom border color" changes it live. "Border opacity" works with all three, and Cancel undoes every change. "Use Windows accent color for the selection" leaves the border alone. A customized space keeps its own border while Global changes. Old configs load with a 1 px border, as before. |
| Painted border | With "Show window border" on, at 100 % and 150 %, on Acrylic, Blur, Solid, and Clear, with Square, Small, and Windows corners: the border sits on the window's edge and follows its rounded corners with no gap or cut-off corner, as close to DWM's old border as you can tell. Fade, Dissolve, Burn, and Pixelate on Startup and Delete: the border fades, dissolves, burns, and pixelates with the space and no outline is left. With the border off, none shows. Windows 10: a square border. The zoom pop-up looks as before. |
| Blank space at startup | With Startup set to Roll, "One after another", and `--debug`, start DeskSpace ten times: the debug window opens first, then the spaces roll in one after another, as they do without `--debug`, and every space shows its title, tint, and items, with none left blank (blur only) until hovered. The same with Startup set to None. Restart right after signing in and after a display or GPU driver change too: with `--debug` at Debug level, a "direct2d target lost" line is followed by a full repaint of that window. |
| F-61 extras | With `--debug`: the Spaces table follows moves, folds, locks, and renames within a second; "Clear the icon cache" redraws every icon without gaps; "Save now" writes the file at once; Input → "Log events" logs one line per click, key, wheel, and drop and none for pointer moves; Input → State follows a selection and a hover; each "Mark the log" adds the next numbered line; Simulate → "Windows Animation effects changed" re-reads the switch. |
| X20 (F-61) | Debug → Simulate: "Windows theme changed" repaints the spaces and the settings window; "Explorer restarted" re-adds the tray icon and, with takeover on, re-hides Explorer's icons; with the config read-only, moving a space shows B29's one "could not save" box, and after Exit the file is writable again; "Crash now" with takeover on ends DeskSpace and the watchdog brings Explorer's icons back within a few seconds, and a debug build's console shows the panic line. |
| X19 (F-61) | Debug → Takeover, with takeover on: the toggle off brings Explorer's icons back at once and they stay back past the 5-second health timer; on hides them and draws the layer; Re-sync picks up a file added to the Desktop folder; State shows the watchdog running while takeover is on; the saved config's `take_over_desktop` is unchanged afterwards. |
| X18 (F-61) | Debug → Animations: every effect, played on a space at ¼×, leaves and comes back in place; pressing Play again mid-way turns it around; hiding the spaces from the tray mid-preview keeps them hidden; the other spaces and the model do not change. |
| Play on all spaces (F-61) | Debug → Animations → "Play on all spaces" plays the chosen kind on every space at once at the chosen speed, each leaving and coming back in place; pressing it again mid-way turns them all around; with the spaces hidden from the tray, nothing shows. |
| X16 (F-61) | Debug → System: switching Windows between dark and light, changing the accent, and turning off transparency each show within a second; selecting an item turns on Watch selection; a rename turns on Editing; Hotkeys says which bindings registered. |
| X14 (F-61) | Debug → Windows: "Keep spaces above other windows" lifts the spaces above Notepad and off drops them back to the desktop; the Windows table lists every space, the tray's controller window, and the debug and settings windows in z-order, and follows a space moved to another monitor; the Monitors table matches Windows' display settings. |
| X12 (F-61) | Debug → Diagnostics: the Build, Model, Resources, and Updates values look right and Resources ticks once a second; creating and deleting spaces moves the counts; "Copy diagnostics" pastes into Notepad as a readable report. |
| X10 (F-61) | Debug → Config: the Configuration and Storage readouts match `--config` ("scratch", or "real" without it); "Write the configuration to the log" shows every space in the console; both Open buttons open the folder in Explorer. |
| X9 (F-61) | Debug → Spaces: Unfold, Unlock, Refresh, and Prune each do what they say on folded, locked, and stale spaces; a 1000-item stress space opens and scrolls smoothly, and "Delete all spaces" removes it. |
| Delete stress spaces (D14) | Debug → Spaces: make a 50 and a 200 stress space and rename a third; Delete stress spaces asks "Delete 2 stress spaces?" with 250 items, then removes those two with their shortcuts in the Recycle Bin and none on the desktop; the renamed one stays; the button greys once none are left. |
| X8 (F-61) | Debug → Spaces → "Create test spaces" opens four spaces in a row beside the existing ones, titled Test: Apps, Tools, Places, and Web; every shortcut opens what it names (the Tools ones show the test text); a second press makes four more without overlapping; then "Delete all spaces" removes them and leaves nothing on the desktop. |
| X7 (F-61) | With three spaces holding dropped files and one Shift-dropped file, Debug → Spaces → "Delete all spaces" asks once with the right counts and the scratch config's path; Cancel changes nothing; OK plays each space's Delete animation; the dropped files are in the Recycle Bin, the Shift-dropped file is where it was, and nothing new is on the desktop. Started without `--config`, the question shows the real-configuration warning (press Cancel). |
| X5 (F-61) | With `--debug`, the rail lists "Windows" with a "Show spaces" check box that hides and shows the spaces and follows the tray's Hide/Show within a second; closing the window and choosing the tray's "Debug…" brings it back with the earlier log; without `--debug` the tray has no "Debug…"; the window themes dark and light, resizes (the console grows), and stops at its minimum size; Tab moves between controls. |
| X4 (F-61) | With `--debug`, the window opens with the console already holding the startup lines; moving a space adds lines live; Level Trace shows trace lines and Info stops new debug ones; Filter, Pause and resume, Auto-scroll, Clear, Copy (paste into Notepad), and "Save log…" work; a `-Release` build shows the log with no console window; without `--debug` nothing opens and stderr logs as before. |
| C2 (F-29) | With "Use Windows accent" on, the selection takes the accent (and the border, with "Border color" set to Windows accent), and they follow it live when it changes in Windows Settings (watch for the `WM_DWMCOLORIZATIONCOLORCHANGED` note under **Found while working**). |
| C3 (F-30) | Switching "Settings and menus" between Dark and Light repaints the open settings window and the next popup menu. |
| C4 (F-34) | "Start DeskSpace when you sign in" adds and removes the `Run` value, and DeskSpace starts at sign-in. |
| C5 (F-48) | The "Item preset" dropdown shows both entries; "Custom" enables the Layout fields and the icon size takes effect; "Match Explorer's display" greys them. |
| C7 (F-18) | Each hotkey fires once bound; a conflicting binding is reported beside its field and the window stays open; clearing a binding stops it. |
| C8 (F-17) | A space created from the space menu, the tray, and the hotkey uses the configured title, size, and lock state; a locked new space keeps its size and scrolls. |
| E1, E2 | `deskspace.exe` shows the icon in Explorer's small, medium, and extra-large views and the version under Properties > Details; the tray, settings title bar, and Shift+right-click desktop verb show the icon at 100 % and 150 %. |
| E5-E7, E10-E12 | Covered by the R12 dry run (installer, uninstall, the watchdog in the zip, update from one pre-release to the next). |
| F-45 automatic install | Installed build with a feed: turn on both update toggles; after a check finds a newer release, the tray shows "Restart to update to X"; 10 minutes without input (no full-screen app) installs silently and DeskSpace comes back with the same spaces, with takeover on the desktop icons are never left blank; the tray entry installs at once with a progress window; a portable copy greys the install toggle. |
| Update install failure | Installed build with a feed and a newer release: in PowerShell set `$env:TEMP` and `$env:TMP` to a missing drive (`Z:\none`), start `%LocalAppData%\DeskSpace.App\deskspace.exe --hidden`, then "Check for updates…" → "Update and restart": DeskSpace quits, comes back hidden on the old version with the same spaces, and a "DeskSpace was not updated" box in front of other windows names the error. With takeover on, the desktop icons are never left blank. |
| F1-F5 | F2 with no item selected renames the space title, with one selected the file; Alt+Enter opens Properties, or Settings with nothing selected; Enter opens the item; Esc clears the selection and still closes an open menu; Shift+Delete bypasses the Recycle Bin. |
| F9 | With "Snap resized spaces to whole columns and rows" off, a resize keeps the dragged size (still snapped to `snap`). |
| F14 (F-51) | The color picker opens from a Colors-page swatch and a space's color menu; on Windows 11 22H2+ it is acrylic, else opaque, rounded and bordered in both modes; the square, hue, and opacity sliders drag smoothly; a swatch click selects and a double-click applies; typing `#3B82F6`, `rgb(59, 130, 246)`, or `217°, 91%, 60%` switches the radio button and the color, invalid text shows a red message and is dropped on leaving the box; Ctrl+V pastes, the copy button and Ctrl+C copy; the eyedropper samples any screen pixel live, a click keeps it and Esc or a right click cancels; Current goes back; the title strip drags it; it is sharp at 100 % and 150 % and after moving to another monitor; recent colors survive a restart and a settings Cancel; Apply, Cancel, the X, Enter, and Esc behave and the settings window does not hang. |
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
| T4 (D12) | Rechecked after the rail fix: "Animations" reads as a semibold header with its pages (Startup, Delete, Fold since T15) indented under it; the "Windows settings…" button's caption fits; Detail shows a plain number. Then the rest: nothing clipped at 100 % and 150 %; each effect greys the options it does not use, Delete's "Same as startup" greys its options, "All at once" greys the delay; with Windows' "Animation effects" turned off while the window is open, every control greys and the note says why; the button opens the page with that switch (Windows 11 and 10); Cancel undoes an edit. |
| T6 | Nothing should change: spaces on every backdrop, the desktop layer, and a desktop-hosted space (`--placement desktop-*`) paint as before, title, items, border, highlight, and scroll thumb included (the paint path was split so animations can draw through a layer). |
| T8 (D12) | Each Startup effect, on hiding and showing (Roll, Fade, Pixelate, Gradient, Dissolve, Fly, CRT, Burn), from the tray, the space menu, and the hotkey, on Acrylic, Blur, Solid, and Clear spaces at 100 % and 150 %: smooth, no flash of old content when a show starts, and the blur steps on at the start of a show and off at the end of a hide; the hotkey pressed twice quickly turns it around without a jump; Fly leaves by the nearest edge and never shows on a second monitor; a file dragged onto a hiding space is refused; with Windows' "Animation effects" off, hiding and showing are instant; a desktop-hosted space (`--placement desktop-*`) still hides and shows; checked at ¼× too (`--debug`, Animations → Speed), and with "Animate even when Windows' Animation effects is off" on, hiding and showing animate while Windows' switch is off (X17). |
| T9 (D12) | Fold and unfold from the space menu and a title double-click on each backdrop: the body rolls up into the title bar with its items in place, and rolls down showing them; a second double-click mid-way turns it around; Fold set to None folds at once; moving a space right after folding it is not fought by the roll; hiding a space mid-fold hides it folded (or unfolded) and shows it the same way. |
| T10 (D12) | Each Startup effect on a new space from the space menu, the tray, the hotkey, and a drop on the taken-over desktop: the space appears with it and no empty frame or early blur shows first; at startup with several spaces, "One after another" brings them in reading order at the set delay and "All at once" together; "Start with spaces hidden" shows nothing until shown; Fly from each edge and on a second monitor; hiding during the startup stagger hides every space, waiting ones included. |
| T11 (D12) | Each Delete effect, and "Same as startup", deleting from the space menu and from the settings header: the space plays it, then is gone, and clicks or drops on it while it plays do nothing; deleting a space that is still flying in (a drop on the taken-over desktop that adds nothing does this) turns it around; quitting mid-animation leaves no window behind; with the spaces hidden, a delete is instant. |
| T15 (D12) | Settings > Animations lists Startup, Delete, and Fold; a config last saved with the Hide and show page keeps that page's effect as Startup's; Startup's effect plays on hiding, showing, a new space, and startup (staggered only at startup). |
| T16 (D13) | Exit from the tray and from a space menu with several spaces: they leave one after another in reverse reading order (all together with "All at once"), then DeskSpace closes, within about 2 s; with takeover on, Explorer's icons come back only after the spaces have left; a second Exit mid-way closes at once; with the spaces hidden, Exit closes at once; signing out of Windows is not held up. |
| Debug animation settings (D14) | With `--debug`, Animations shows one "Animation settings" box below the preview controls, in light and dark mode, at 100 % and 150 %; choosing Startup, Delete, or Fold in Kind shows that kind's values at once, with the rows it or its effect does not use greyed (as on its settings page), and the window stays on Animations; scrolling the section leaves no stale pixels; every choice takes effect on the next play (Play on the space; Fold plays its effect out and back) and is in the config file at once; with the settings window open, its Animations pages show the change, and Cancel there keeps it. |
| Flat material | Fade, Pixelate, Gradient, Dissolve, and Burn on hide, show, and delete (and in the debug preview) on Acrylic, Blur, and Solid spaces with a border, with Windows' "Transparency effects" off and on: no outline stays around the fading space, and it comes back once a show ends; with the switch off, no dark box either, the dark fill coming back only once a show ends; with it on, see the T13 row; Roll, Fly, and CRT look as before. |
| T12 (D12) | Windows 11 with "Transparency effects" on: the log says "spaces use composition"; the R3 list passes unchanged with each backdrop (Acrylic and Blur blurred, Clear and Solid not), at 100 % and 150 %, corners and border as before, and the T8 row still passes, the blur stepping on and off as today. With the switch off at startup, the log names it and spaces look as before T12. Windows 10, `--placement desktop-*`, and `--click-through` change nothing. Idle CPU and GPU no higher than before. |
| T13 (D12) | Windows 11 confirmed 2026-10-18 (maintainer). Still to check: on Windows 10 the blur still steps; idle GPU no higher than before. |
| T17, T18 (D12) | Burn on Startup (hide, show, a new space, startup, exit) and on Delete, on each backdrop at 100 % and 150 %: the space burns from the bottom up along a ragged edge that glows, chars, and scorches ahead of itself, like paper; flames lick up from the edge and past the top and sides of the frame with no box or blur around them; Detail changes how ragged the edge is; reversing mid-burn (the hotkey twice) un-burns it; no flame window is left behind after it ends, after a delete, or after quitting mid-burn; a desktop-hosted space burns without flames. |

## Found while working

One line per issue noticed but not fixed: file, symptom, suggested task.

- **Open bug, needs decision:** with takeover on, the primary taskbar
  sometimes stops taking clicks; they reach the desktop layer instead
  (seen 2026-10-18). `scripts/zorder.ps1` showed `Shell_TrayWnd` with
  topmost cleared and below the layer, which spans the whole desktop;
  the secondary taskbar was fine; a restart cleared it. Hypothesis:
  Explorer's full-screen detection takes the layer, which covers the
  monitor, for a full-screen app. `App::check_taskbars` now logs a warning
  (with the taskbars and the foreground window) when it happens and Debug
  → Windows → Taskbars shows it, to find the trigger. Fix options: one
  layer per monitor sized to its work area (robust), or re-sinking the
  layer on the health tick (a 5-second band-aid).
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
- `win32/space_window.rs` is about 900 lines of non-test code after T12
  (2026-10-18), past the 800-line guideline. Suggested section K task:
  move the z-order pinning (`on_window_pos_changing`, `insert_after`,
  `window_above`, `set_on_top`) and the mouse handlers into their own
  modules.
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
- `render/d2d/mod.rs` `draw_scene`: the settings window's 2 px highlight
  outline rounds with `PaintFrame.radius`, which is 0 where DWM rounds the
  window, so its corners are square inside DWM's round ones. Suggested:
  round it with the radius the border uses.
