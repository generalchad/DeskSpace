# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

Only open work lives here. Finished tasks are moved, verbatim, to
`archive/TASKS-done.md` (history only; its file and line pointers predate the
R1 and K splits). Built-but-unconfirmed work is listed in section V.

**Pick a task:** these are ready for any contributor:
- section B (code health);
- section O, in order (on-screen checks by script);
- Q2-Q6;
- section W phase 5: W26 (a pure move), W24 and W25 (with a
  reviewer), and W34 now that W14 has landed; the rest once the task each
  one names has landed;
- section S, in order: S8 and S13; S5 and S9-S12 need a screen or
  a reviewer.

L1 needs a maintainer's on-screen findings. Skip tasks marked **[needs
decision]** and everything in section D. Phase 4 is complete; its on-screen
checks are in section V.

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
   section V, and archive the task as in rule 7. Run any `scripts/onscreen/`
   check that covers part of it first, and cut what it covers from the row,
   as section O's rules say. Anything only a human can
   record (for example L1's findings) stays here until it is recorded.

## File map

Where each kind of code lives. Paths are under `crates/`.

| To change... | Look in |
|--------------|---------|
| The model (spaces, items, views, the desktop layer) | `deskspace-core/src/{space,view,item,desktop,desktop_layer}.rs` |
| Item ordering, both kinds | `deskspace-core/src/sort.rs` (`SortRule`) |
| A model mutation | `deskspace-core/src/command/` (`Command` and the dispatch in `mod.rs`; one `apply_<variant>` method each in `space_ops.rs`, `item_ops.rs`, `lens_ops.rs`, `layer_ops.rs`) |
| A lens's folder view and entries | `deskspace-core/src/source.rs` (`SpaceSource`, `FolderView`, `FolderEntry`) |
| Grid, sizes, snapping | `deskspace-core/src/layout.rs` (`GridSpec`, `arrange_space`) |
| Where each item sits in the current view, the scroll extent, and hit rects | `deskspace-core/src/view_layout.rs` (`ViewLayout`, `ViewMetrics`) |
| A look setting or color | `deskspace-core/src/theme.rs` (`Theme`, `Palette`, `*_RANGE`) |
| A non-look setting, config load/migrate | `deskspace-config/src/schema.rs` (`WindowSettings`, `Config::migrate`) |
| Config file paths and atomic save | `deskspace-config/src/store.rs` |
| CLI flags | `deskspace-app/src/cli.rs` |
| Startup before the window loop | `deskspace-app/src/bootstrap.rs` |
| Handling an `Event`; timer and menu ids; preset tables | `deskspace-app/src/app.rs` (`on_event`, `MenuId`, `*_PRESETS`, `SORT_MENU_IDS`) |
| Saving, dispatching a `Command` | `deskspace-app/src/app/persist.rs` |
| Undo and redo stack, applying entries | `deskspace-app/src/app/undo.rs` |
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
| Space window procedure, DWM, shadow window | `deskspace-platform/src/win32/{space_window,dwm,shadow_window}.rs`, `render/shadow.rs` |
| An animation effect | `deskspace-core/src/animation.rs` (the tables), `deskspace-platform/src/render/transition.rs` (frame math), `render/d2d/` (drawing), `win32/space_animation.rs` (playing it on a window), `win32/overlay.rs` (windows beside the space); the recipe is section T |
| Drawing | `deskspace-platform/src/render/scene.rs`, `render/d2d/` (`mod.rs` the `Painter`, `effects.rs` an animation's resources, `prepared.rs` the `PreparedScene`, `drag_image.rs` the drag image) |
| The zoom pop-up window and its layout | `deskspace-platform/src/win32/zoom_bar.rs`, `render/zoom_bar.rs`, `render/d2d/zoom_bar.rs` |
| Settings window controls | `deskspace-platform/src/win32/settings_window/` (`mod.rs`, `build.rs`, `update.rs`, `nav.rs`, `paint.rs`, `commands.rs`, `preview.rs`) |
| The debug window (`--debug`) | `deskspace-platform/src/win32/debug_window/` (`mod.rs`, `build.rs`, `layout.rs`, `pages.rs`, `console.rs`); the console's pure model in `deskspace-platform/src/debug_console.rs`; the log capture in `deskspace-app/src/debug_log.rs`; the panel and actions in `deskspace-app/src/app/debug/` |
| The color picker popup | `deskspace-platform/src/win32/color_picker/` (`mod.rs` the window, `state.rs` its input, `format.rs` the value box's text, `draw.rs` the settings swatch), `render/color_picker.rs` (layout), `render/d2d/color_picker/` (`mod.rs` the painter, whose ignored `render_preview` test writes PNGs; `pen.rs` its drawing kit) |
| Shell: open, item menu, drag out | `deskspace-platform/src/win32/shell_items.rs` |
| Shortcuts from dropped shell items, custom targets | `deskspace-platform/src/win32/shortcuts.rs` |
| The installed-apps picker and its cache, the custom shortcut dialog | `deskspace-platform/src/win32/{app_picker,app_catalog,shortcut_dialog,dialog}.rs` |
| Desktop enumeration, icons | `deskspace-platform/src/win32/desktop_items.rs`, `win32/icons.rs` |
| Icon-set glyphs (SVG to pixels) | `deskspace-platform/src/render/glyph.rs` (the document), `render/d2d/glyph.rs` (the rasterizer), `win32/glyphs.rs` (the cache) |
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
user-visible change is expected. Do them in any order. The exceptions are
B32-B36, from a review on 2026-10-21: they fix performance and correctness,
and B33 and B36 change behavior on purpose, as each says.

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

- [ ] **B31. Test only the crates a change affects (scripts).**
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

- [ ] **B33. Load icons off the UI thread (platform, app).**
  **[needs decision]**: the worker and event below are a proposal.
  - Why: `IconCache::icon_for` loads a missing icon synchronously while
    `space_scene` builds a scene, through the shell's image factory. A
    lens on a slow or network folder, or the first scroll through a big
    one, stalls every space until each icon is loaded. Explorer draws a
    generic icon at once and fills the real one in later.
  - Proposal: `icon_for` returns the cached icon, or a generic one for
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

All of U1-U7 are done; what still awaits a check is in section V.

## W. Lenses (ROADMAP M4 phases 2-5, D3, D17, D18, F-07)

### What you are building

A *lens* is a space that shows the contents of a real folder, live. If a
file is added to `C:\Users\me\Downloads` in Explorer, a lens on that
folder shows it a moment later. A lens is only a view: DeskSpace never
moves, copies, or deletes a lens's files on its own. Read ROADMAP D3 (the
rules for files) and M4 (the plan) before you start; both are short.

The model already exists (phase 1; `CONTEXT.md` section 4). In code a lens
is a `Space` whose `source` is `SpaceSource::Folder(FolderView)`. There is
no separate `Lens` type (D5). Four `Command`s change a lens:

- `CreateLens` makes one.
- `SetLensFolder` points it at another folder.
- `SetLensView` changes its hidden and filter settings; `SetSpaceView`,
  which every space has, changes its sort rule.
- `SyncLens { id, entries }` replaces its items with a fresh listing of
  the folder. Nothing adds items to a lens one by one: `AddItem` and
  `MoveItem` into a lens return `Rejected`.

Phases 1-4 are done: drops, pastes, shortcuts, deletes, and menus treat a
lens as D3 says (W1-W7), and the views, sort rule, status bar, and lens
settings are in (W9, W18-W23, ROADMAP D17). Phase 5 is W24-W43 (D18). New
here? Read "Where to start" below before anything else in this section.

### Words used below

- **Watch.** Windows' `ReadDirectoryChangesW` tells us that something in a
  folder changed. It does not say what the folder now holds, so after a
  change we list the folder again.
- **Debounce.** Copying 200 files fires 200 changes. We wait until the
  folder has been quiet for 150 ms and then react once. `Coalescer` in
  `win32/folder_watch.rs` already does this. It is pure and tested, so
  read it and its tests first.
- **Worker thread.** Any thread that is not the UI thread. A slow folder,
  such as a network drive or a folder of 50,000 files, must never freeze
  the spaces, so watching and listing run on a worker. A worker cannot
  touch windows. It posts a message (`PostMessageW`) to the controller
  window, and the UI thread turns that message into an `Event` for the
  app.
- **I/O completion port.** A Windows queue that many file handles report
  into, and that one thread waits on with `GetQueuedCompletionStatus`.
  Each handle is registered with a number, its *completion key*, so when a
  result arrives you know which folder it belongs to. You can also put
  your own packets on the queue with `PostQueuedCompletionStatus`. That is
  how the UI thread tells the worker to add a folder, remove one, or stop.
  Overview:
  https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
- **Shell file operation.** `IFileOperation` is the engine behind
  Explorer's copy, move, and delete. It shows Explorer's progress and
  conflict dialogs and records a step Explorer can undo.
  `win32/file_op.rs` wraps it.

### Before you start

- One commit per task ("How to work a task" above). W1-W7 went in order
  for safety: no menu could create a lens until drops and deletes were
  safe, so no user could lose a file while the work was half done. Keep
  that habit. A task that touches files lands complete, or not at all.
- To get a lens to work on, run the app with a scratch config
  (`./scripts/dev.ps1 -OnTop`; `ENGINEERING.md` section 1) and use "New
  lens…" from the space or tray menu, on a throwaway folder. Never point a
  test lens at a folder you care about: Delete in a lens recycles real
  files. The section V rows that say "hand-made" mean one written into the
  config instead. Exit the app, add this to
  `%TEMP%\deskspace-dev\deskspace.toml`, and run the app again. Use an
  `id` no other space has:

  ```toml
  [[desktop.spaces]]
  id = 90
  title = "Lens test"
  rect = { x = 100, y = 100, width = 320, height = 240 }
  source = { folder = { path = "C:/Temp/lens-test" } }
  ```

- `cargo test` cannot run a watch or open a dialog. Put every decision you
  can into a small pure function and test that, and keep the Win32 calls
  thin. Each task names its pure functions.
- Decided (2026-10-20, maintainer): one worker thread serves every watch,
  each watch has a key, and the worker lists the folder itself. Do not
  redesign this inside a task. If it seems wrong, stop and ask.
- Decided (2026-10-21, maintainer), for W3a-W4:
  - A drop on a lens moves; "Copy here" copies. Both go through the
    shell with Explorer's Replace or Skip dialog. Keep-in-place (Shift)
    means nothing on a lens, so it is refused.
  - Paste and new shortcuts on a lens put their files in the lens's
    folder, exactly as a drop does.
  - A lens item dropped or pasted on a normal space is an ordinary file
    to that space: it moves into storage like a file from Explorer.
  - There is one shell transfer function with a collision rule, not a
    second copy of it.
  - A lens whose folder is DeskSpace's storage folder, or inside it, is
    kept in the config but never watched, and shows as unavailable.

### Where a test cannot reach

`App` methods that take `&mut Ui` cannot run in `cargo test`, because a
`Ui` needs real windows. So every task below pulls its decision into a
small function that takes plain data (a `&Space`, a `&Path`, a
`DROPEFFECT`) and tests that. The method that takes the `Ui` only calls
it. If you find yourself wanting to test a `Ui` method, stop and look for
the decision inside it.

### Where to start

W1-W8 and W10-W23 are done (2026-10-21; W14-W16 on 2026-10-22, W17 on
2026-10-23, W9 and W18-W23 on 2026-10-24) and archived; their on-screen checks
are in section V. The open work is phase 5 (W24-W43).

1. Read, in this order: "How to work a task" (top of this file),
   `ENGINEERING.md` in full, `CONTEXT.md` section 3 and the lens part of
   section 4 ("Space"), then ROADMAP D3, D17, and, for a phase 5 task,
   D18.
2. Read the phase's opening rules, then the task. A phase 5 task also
   means reading "Interleaving", "Performance", and "Safety and
   security" under phase 5. They bind phase 4 too.
3. Pick a task from the table below. "Open now" means nothing it waits
   for is still open (2026-10-21). Otherwise wait until every task it
   waits for has been ticked and archived. Those are the tasks in its
   "After:" line, or, for W9-W23, the ones phase 4's first rule names. A
   ticked box means a task is done; "Open now" goes stale, so trust the
   boxes.
4. Find the code with the file map at the top of this file. For lenses
   the rows to know are "A lens's folder view and entries" (core), "A
   model mutation" (`command/lens_ops.rs`), and `app/lens.rs` (watches
   and listings), plus `app/menus.rs` for the lens menu.
5. Before you commit, run the gates. Tick the box and move the task to
   `archive/TASKS-done.md` in the same commit. For a **[needs human]**
   task, add its section V row (rule 8).

| Level | Tasks | Open now |
|-------|-------|----------|
| First task: one idea, little Win32, no threads | W26 (a pure move), W27, W29, W40 | W26 |
| Next: a feature across core, app, and a menu or page | W28, W30, W33, W34, W35, W36, W38, W39 | W34 |
| With a reviewer: layout, painting, threads, or file safety | W24, W25, W31, W32, W37, W42, W43 | W24, W25 |
| Maintainer only: an on-screen record | W41 | W41 |

"With a reviewer" means: agree the plan with a reviewer before writing
code, and have the same reviewer read the diff. W42 is the hardest task left
in the section. W24 and W25 fix code that runs today, so they go first among
the reviewer tasks.

### Phase 4: views, sort rules, and lens settings

Read ROADMAP D17 first; it is the spec. Most of phase 4 is for every
space, not only lenses. A view, a status bar, and a sort rule work the
same on both kinds. Only the folder, the title, and hidden files are
lens-only.

- One commit each. W8-W11 are small and stand apart from the views: W9
  needs nothing, W10 needs W8, and W11 needs W10. W12-W23 build the
  views on each other, so take them in order.
- Each task adds a type, variant, or field in the same commit as its
  first caller, never ahead of it with `#[expect(dead_code)]`. So
  `ViewMode` grows one variant per view task, and the task that draws a
  view adds it.
- Tables carry the design, so a new view is one row:
  - `VIEW_MODES` (`settings/tables.rs`, added in W12): one row per view,
    with its mode, its Explorer label, and its Ctrl+Shift digit. The
    View ▸ menu, the keys, the View page, and the status bar's tips all
    read it.
  - `SORT_KEYS` (W14): the Sort by keys in Explorer's order and words.
  - `DETAILS_COLUMNS` (W18) and `STATUS_BUTTONS` (W19).
- Explorer is the reference for every look and word. Where a task says
  "as Explorer", open the same folder in Explorer beside the space, at
  100 % and 150 %, and match it. Record any measured size as a named
  constant in 96-DPI pixels, with a comment saying it was measured, and
  when.
- Glyphs come from Segoe Fluent Icons on Windows 11 and Segoe MDL2
  Assets on Windows 10. Use only code points both fonts have, and write
  each one's name beside its constant:
  https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
  and https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-ui-symbol-font.

### Phase 5: lens and view extras (after v0.3)

Read ROADMAP D18 first; it is the spec, and it builds on D17. Phase 5
ships after v0.3, so no task here blocks the lens release. The phase 4
rules above (first callers, the tables, Explorer as the reference, the
glyph fonts) hold here too.

- W24-W26 fix or tidy code that exists today. Take them any time, even
  before phase 4; W24 and W25 are worth doing first.
- Every other task names the phase 4 task it needs ("After: W18").
  Within those limits, take them in any order, but read "Interleaving"
  below before running two at once.

#### Interleaving

Two kinds. **Work interleaving**: tasks that edit the same code, so
running two at once makes them collide. **Runtime interleaving**: events
that can arrive in an order the code does not expect.

Work that collides when run in parallel:

- **`SpaceView` fields.** W12, W14, W15, W19, W28, W30, W33, and W36
  each add a field and a serde-default test. Two branches add fields
  side by side; resolve a merge by keeping both, in task order.
- **"Next free" ids.** Menu ids (W12, W14, W15, W19, W28, W30, W33, W38),
  `field::` ids (W10, W11, W20, W23, W28, W33, W35, W38, W40), and page
  ids. Two branches that each take the "next free" value end up with the
  same id, and git merges that without a conflict. The uniqueness tests
  (`field_ids_are_unique`, the `MenuId::ALL` test) catch it after a
  rebase; take a new id then. Never renumber an id that has landed.
- **`view_layout.rs`** (W16) grows in W17, W18, W21, W22, W33 (compact),
  and W37 (groups). Run those one at a time; each changes the metrics
  every view shares.
- **`DETAILS_COLUMNS`** is created in W18 and changed in W29-W32. Run
  them in order.
- **Label text.** W26 merges the four copies of the extension rule.
  W28 then changes that one place. Taking W28 before W26 means editing
  four places, so don't.
- **The lens's folder.** After W42, file operations read the folder a
  lens *shows*, through one `App::lens_folder`. A task that lands after
  W42 and reads `FolderView.path` must say which one it means. W10 and
  W11 mean the lens's own folder on purpose.
- **Creation signatures.** W19 and W23 change `CreateLens`, and W40
  changes `CreateSpace`. Rebase whichever lands second.
- **Other sections:**
  - J (tabs): J2 pages through the icon grid. After W16 it must page
    through the view layout instead; if J2 starts first, stop and ask.
  - U (undo): U3 lists the actions to record. If U3 has landed, wrap
    each new action in `begin`/`finish` (`SetSpaceView`, `SetLensTitle`,
    a column resize on release, not per move). If it has not, add the
    new actions to U3's list. Browsing and folding a group are session
    state and record nothing.
  - B14 and B26 rework `settings_window/build.rs`, where W8 adds a font.
    Do B14 first, or keep W8's edit inside one function.
  - R14 (table-driven settings fields) would rewrite the pages W10, W20,
    W23, W35, and W38 build. Do not start R14 while those are open.
  - Section K: `win32/space_window.rs` is already over the 800-line
    guideline. Put new hit tests and paint code in `app/pointer.rs` and
    `render/d2d/`, not there.
  - T (animations): the title icon (W9), status bar (W19), headers (W18,
    W37), and Back button (W43) paint through the normal paint path, so
    animations snapshot them for free. Each task's check includes one
    Startup effect.

Runtime orders the code must survive:

- **A listing for a folder the lens no longer shows.** The watch worker
  posts a listing as a message, so it can arrive after Change folder…
  or (after W42) a step into a subfolder. W24 makes a listing carry the
  folder it read, and the app drops a stale one. Until W24 lands, a
  stale listing can briefly list the old folder's files in the lens.
- **Message pumps inside a call.** `transfer_files`, `pick_folder`, and
  `alert` pump messages. A listing, a settings event, or a delete can run
  inside them. Hold no runtime borrow across them (`ENGINEERING.md`
  section 6, Re-entrancy). Afterwards, look the space up again: it may
  have been deleted, or (after W42) it may show another folder. Change
  folder… from the Lens page is the case to test.
- **An item or space that vanishes between two events.** A listing can
  remove the item under a rename, a drag, a hover tip, or a column drag.
  Anything that holds an `ItemId`, a group, or a column between events
  checks that it still exists and cancels quietly if it does not.
- **Settings window and menu at once.** A menu edit made while the
  settings window is open goes into the window's snapshot (`CONTEXT.md`
  section 6, Native UI). Every new `SpaceView` field follows that rule
  (W20 sets it up), so Cancel never undoes a menu edit.
- **The current space.** The keyboard hook decides to consume a key, and
  the app handles it later; the current space can change in between. The
  app looks the current space up when it handles the key, never earlier.
- **Bursts.** Column-resize drags, Ctrl+wheel, and typing in a filter
  field fire many events. Re-lay out per event, but save once, through
  the existing debounced `schedule_save`.

#### Performance

The bar is bare-metal: a 50,000-file lens scrolls, selects, and resizes
with no hitch. `ENGINEERING.md` sections 2-4 have the general rules;
these are the ones phase 4 and 5 code most often breaks.

- **Layout is arithmetic.** An order-placed view finds the rect for index
  `i`, and the indices visible in a scroll window, by arithmetic, not by
  walking every item. Groups (W37) keep prefix sums of group sizes and
  binary-search them. Nothing costs O(items) per mouse move or wheel
  notch.
- **Text is made once.** Type names (cached by extension), dates, sizes,
  and group names are formatted when a scene is built, and only for the
  items the scene carries (the visible ones). Painting allocates nothing;
  `PreparedScene` already encodes strings to UTF-16 once.
- **No IO the listing did not already do.** Dates, sizes, and attributes
  come from the `FindFirstFileExW` record the worker reads. Hover tips,
  titles, and status text read the model. Nothing calls the disk on the
  UI thread per item or per event; W25 removes the one place that calls
  it per lens.
- **Measure what could be slow.** A task that touches `SyncLens`,
  sorting, filtering, or grouping times it on 50,000 entries with a
  release build and records the dated result in its commit body. Over
  8 ms (half a 60 Hz frame) on the UI thread means stop and ask before
  merging.

#### Safety and security

`SECURITY.md` names file handling and parsing as the areas of most
interest. In phase 4 and 5 terms:

- **No file outside the user's intent.** A file operation acts only on
  items the user can see and selected: filtered-out and folded items
  are never selected (D18). It goes into the folder the lens shows,
  inside its own folder, which passes the storage check on its final
  path (W25).
- **Never follow a shortcut while browsing.** Only an entry the listing
  marks a directory is browsable. A `.lnk` to a folder opens as today.
- **No regular expressions.** A pattern from the config cannot make
  matching slow: the matcher (W34) is linear, its inputs are capped, and
  there is no regex crate to audit.
- **Every config value is untrusted.** Hand-edited files reach every
  new field, so clamp ranges, cap list lengths, drop duplicates, and ignore
  unknown names (`#[serde(other)]` on an unknown variant), so one bad
  value never fails the whole config load. Each new field gets a test
  that loads a hostile value.
- **No new dependency** without `cargo deny check all`, and none is
  expected in this phase.

#### Phase 5 tasks

- [ ] **W24. A listing carries the folder it read (platform, app).**
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

- [ ] **W25. The storage check runs off the UI thread (platform, app).**
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

- [ ] **W26. One place decides whether a label shows its extension
  (app).** Any time; before W28.
  - `self.surface_match_explorer(surface) &&
    self.system.shell().show_extensions` is written out in
    `app/rename.rs`, `app/selection.rs`, `app/settings_session.rs`, and
    `app/view.rs`. Move it into one `App::shows_extensions(surface)` in
    `app/view.rs` and call that from all four. A pure move.
  - Done when: `rg "show_extensions" crates/deskspace-app/src/app` finds
    only the new method and its callers' use of its result.

- [ ] **W27. The lens's path as a tip on its title (app).** **[needs
  human]** After: W9.
  - Hovering a lens's title shows its full path through the existing
    hover tip (`app/info_tip.rs`). An unavailable lens adds "Folder not
    available". After W42, it shows the folder on display. The text
    comes from the model; no disk access.
  - A pure `title_tip(space, unavailable) -> Option<String>`; `None` for
    a normal space. Hovering the title of a normal space shows no tip, as
    today.
  - Tests: the tip for a lens, an unavailable lens, and a normal space.
  - **[needs human]** check: the tip appears after the usual delay and
    leaves with the pointer; it does not cover the title while you are
    renaming.

- [ ] **W28. File name extensions per space (core, app).** **[needs
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

- [ ] **W29. Date created (core, platform, app).** After: W18.
  - `FolderEntry.created` and `ItemFacts.created` (seconds since the
    epoch, `Option`). `folder_list.rs` reads `ftCreationTime` from the
    record it already has; `item_facts` reads it from the metadata call it
    already makes. No new IO.
  - `SortKey::Created` (saved as `created`), a `SORT_KEYS` row after
    Size, and a `DETAILS_COLUMNS` row, hidden by default; W30 lets the
    user show it.
  - Tests: the conversion (copy `unix_seconds`'s tests); sorting by it.

- [ ] **W30. Choose the Details columns (core, app).** **[needs
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

- [ ] **W31. Resize the Details columns (app).** **[needs human]**
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

- [ ] **W32. Reorder the Details columns (app).** **[needs human]**
  After: W31.
  - Dragging a header (not its divider) moves the column, with an
    insertion mark between headers. Name stays first (D18). Committed on
    release, as W31.
  - A pure `column_drop_index(x, columns) -> usize`.
  - Tests: drop indices, and that Name never moves.
  - **[needs human]** check: reorder at 100 % and 150 %; Esc cancels; a
    click without a drag still sorts.

- [ ] **W33. Compact view (core, app).** **[needs human]** After: W22.
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

- [ ] **W34. Lens filters: the matcher (core).** After: W14.
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

- [ ] **W35. Lens filters on the Lens page (app).** **[needs human]**
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

- [ ] **W36. Group by: the groups (core, app).** After: W15, W18.
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

- [ ] **W37. Group by: headers and folding (core, app, platform).**
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

- [ ] **W38. Group by: the menu and the settings (app).** After: W37,
  W23.
  - Group by ▸ beside Sort by ▸, as Explorer: (None), then the keys from
    `SORT_KEYS`, then Ascending and Descending. Rows on the View page
    and the Lenses defaults page, through the shared builder.
  - Tests: the menu checks the current grouping; the page fields have
    handlers.

- [ ] **W39. Ctrl+wheel through the views (app).** **[needs human]**
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

- [ ] **W40. The default view for new spaces (config, app).** After:
  W23.
  - `WindowSettings.new_space_view: SpaceView`, defaulting to
    `SpaceView::default()`, so nothing changes until the user changes it.
    `CreateSpace` takes a `view`, as `CreateLens` does since W19. Fields
    on the New spaces page, through W23's shared builder.
  - Tests: the config's missing-field default; a new space gets the
    setting.

- [ ] **W41. Close F-40: the desktop's view modes (docs).** **[needs
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

- [ ] **W42. Browse into a subfolder (app).** **[needs human]** After:
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

- [ ] **W43. Back and Up (app, platform).** **[needs human]** After:
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

## S. Space icons (ROADMAP D16, F-62)

Read D16 first, amendments included; it is the spec. Do the tasks in order,
one commit each. M5 item 7 says to build this with or after F-28 (title bar
style), which has no tasks yet. These tasks build on today's title bar, and
`title_icon_rect` (S7) is the one place the icon's geometry lives, so
whichever lands second keeps the other working.

Rules for every S task:

1. **Packs are rows.** Every icon set is one `IconSetSpec` row in
   `ICON_SETS` (S3). The cache, the download, the picker, and the saved
   names are keyed by the row's `prefix`. No code outside the Fluent row and
   its tests names Fluent; reach a set through `ICON_SETS`,
   `icon_set(prefix)`, or `default_set()`.
2. **The network only after consent.** The one way to download a set is
   `deskspace_update::fetch_signed` (S4), started from the app's
   `IconStep::Fetch` (S8), which only an `Allowed` consent produces. Never
   call the network from anywhere else.
3. **Saved names never change.** A user's config stores `<prefix>:<name>`.
   `assets/icons/<prefix>-names.txt` lists every name a set has shipped; the
   trimmer (S3) refuses to drop one, and tests check the curated words
   against it.
4. **Display only, outside a rename.** The icon has no hit test, hover,
   cursor, or click except while its space's title is being renamed, on the
   desktop (S10) or in the settings header (S12).
5. **Glyphs are rasterized off the paint path.** A scene carries a finished
   `Icon`, as items do (S7); the painter only draws bitmaps. Rasterize
   through the `GlyphCache` S6 added, never per frame.
6. Unless a task says otherwise, a name in `code` that does not exist yet
   is one the task creates.
7. **One undo entry.** Changing an icon (S10, S11) records one entry
   through `App::record_effects`, labelled "Change icon", so Ctrl+Z puts
   the old icon back; `SetSpaceIcon` is already in `touched_by`.

- [ ] **S5. Publishing an icon set (ci).** **[needs human]**
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

- [ ] **S8. Consent, the download, and new spaces (config, app).**
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

- [ ] **S9. The icon picker (platform).** Lands in the same commit as S10,
  its first caller, which holds the on-screen check.
  - `win32/icon_picker.rs`, a modal window copied from `app_picker.rs`
    (owner, disabled-owner loop, dark mode, DPI). At the top, a set
    dropdown, shown only when the request has more than one set (D16's
    packs scaffold), and a search box (cue "Search icons") that filters as
    you type through `deskspace_core::icon_match::search`. Below them, a
    note line (`request.note` or the chosen set's), and an `LVS_ICON` list
    view with `LVS_OWNERDATA`, so a set of thousands opens at once. It
    labels each cell with the name, hyphens as spaces, and fills images
    on demand (`LVN_GETDISPINFOW`) from a `GlyphCache` at 32 px, scaled, in
    the window's text color. At the bottom, "Automatic", "No icon",
    Cancel, and OK. OK is the default and enabled while a cell is
    selected; Enter or a double-click on a cell is OK.
  - `Ui::pick_icon(&IconPickerRequest) -> Option<IconChoice>` in
    `runtime/ui_chrome.rs`, beside `pick_apps`: `IconPickerRequest<'a> {
    title, sets: Vec<PickerSet<'a>>, current: Option<&IconRef> }`, with
    `PickerSet { prefix, label, set: Option<&IconSet>, words, note:
    Option<&str> }` (a set not ready has `None` and a note saying why), and
    `IconChoice { Automatic, None, Icon(IconRef) }`. Its owner is the
    settings window when open, else the controller, as `pick_apps`'s is.
    It opens on `current`, selected and scrolled to.
  - `win32/outside_click.rs` `is_outside`: a press on the picker is not
    outside, as one on a popup menu is not, so a rename behind it stays
    open (S10).
  - Tests, pure: the cell label from a name; which cell the request opens
    on; the dropdown shown only for two or more sets.

- [ ] **S10. Changing the icon while renaming (app, platform).** Includes
  S9; one commit, tick and archive both. **[needs human]**
  - `TitleIcon` gains `button: bool` and `hovered: bool`. While the space's
    title is being renamed, `space_scene` always sets `title_icon`, with
    `button: true`. The image is the space's icon or `None`, which the
    painter draws as a placeholder: a rounded outline in the title color
    at reduced alpha, with Segoe Fluent's "Add" glyph (`E710`). A hovered
    button gets the item hover fill behind it. Outside a rename nothing
    changes (S rule 4).
  - `CursorShape::Hand` (`IDC_HAND`; map it beside the others), shown
    while the pointer is over the button during a rename.
    `PointerState` tracks the hover so the scene can draw it.
  - `app/pointer.rs` `on_space_event`, before the click commits the rename:
    a left press inside the button of the title being renamed opens the
    picker and returns, leaving the rename open. Add the check as one
    named method (`title_icon_pressed`), so B24 can still move the preamble.
  - `App::pick_space_icon(ui, id)` (in `space_icons.rs`): turn key capture
    off (`ui.capture_text_input(false)`) so the picker's search box gets
    keys. Build the request: every `ICON_SETS` row, with `need_icon_set`'s
    state as each set's note ("Downloading the iconsΓÇª", "Icon downloads
    are off. Turn them on in Settings > General.", or the failure). Then
    `ui.pick_icon`, then capture back on. A choice commits `SetSpaceIcon`
    at once (`self.commit`) and refreshes the space; the rename goes on,
    and Esc later cancels only the text. If the set is not ready and
    consent is `Allowed` or `Ask`, call `need_icon_set` first (it decides through `next_step`, S rule 2); when the
    download is still running, remember the space (`pending_picker`) and
    open the picker from `ICONS_WAKE` if that rename is still open.
  - `CONTEXT.md` section 8 (rename) and README Controls: during a rename,
    click the icon to change it.
  - Tests: the scene shows the button only while renaming, and the
    placeholder for a space without an icon; `title_icon_pressed` hits only
    the button of the space being renamed; the request's notes for each
    state.
  - **[needs human]** check, on the desktop at 100 % and 150 %: outside a
    rename, the icon shows no hand and a click does what the title bar
    does. F2 on a space, then hover the icon: hand and hover fill. A click
    opens the picker in front, with the rename still showing behind it.
    Search "pho", pick an icon, then OK: the icon changes at once, and you
    can type more of the name and press Enter. Esc in the picker changes
    nothing. "Automatic" and "No icon" work; a space without an icon shows
    the placeholder. Clicking in the picker never ends the rename. With
    downloads declined, the picker opens with the note and its two
    buttons.

- [ ] **S11. Icons in the settings window (platform, app).** **[needs
  human]**
  - `ui.rs`: `SettingsScope.icon: Option<Glyph>` and `SettingsHeader.icon:
    Option<Glyph>`, owned so the form stays plain data. The settings window
    rasterizes them through a `GlyphCache` in its state, in its own text
    color and DPI. `settings/mod.rs` `build_form` takes the glyphs by space
    (a `&BTreeMap<SpaceId, Glyph>` that `settings_session.rs` builds from
    `App::space_glyph`, the S7 lookup before rasterizing), so `settings/`
    stays pure.
  - Scopes pane (`settings_window/paint.rs` `on_draw_scope`): the icon at
    s(16), left of the label, after the indent, dimmed with dimmed rows.
  - Header: put the new code in a new `settings_window/header.rs`, so it
    does not collide with B14 or B26. An owner-drawn `BUTTON` of s(20)
    square at `title_x` draws the icon. It stays disabled, and so takes no
    click or Tab stop, until S12. The title moves right by s(20) + s(8)
    while there is an icon, and `apply_header` shows, hides, and moves
    both. Global has no icon.
  - Tests: `build_form` gives each space scope and the header the glyph it
    was handed, and Global none.
  - **[needs human]** check: in light and dark, at 100 % and 150 %, spaces
    with icons show them in the scopes pane and the header, aligned with
    the text; spaces without show nothing, laid out as before; a dimmed
    scope's icon is dimmed; the header icon takes no click.

- [ ] **S12. Renaming in the settings header (platform, app).** **[needs
  human]** After S10 and S11.
  - `SettingsHeader.renamable: bool` (true for a space scope). In
    `settings_window/header.rs`, double-clicking the title (`SS_NOTIFY`,
    `STN_DBLCLK`) puts an edit over it with the text selected and focused.
    The edit answers `WM_GETDLGCODE` with `DLGC_WANTALLKEYS`, so Enter and
    Esc reach it rather than the window's OK and Cancel. Enter, or focus
    leaving for anything but the icon button or the picker, raises
    `SettingsEvent::HeaderRenamed { title }`; Esc ends the edit and
    changes nothing.
  - While the edit is open, the header's icon button is enabled and shown,
    with the placeholder when the space has none (S10's look). A click
    raises `SettingsEvent::HeaderIconPressed`. `apply_header` updates the
    icon and the label without closing an open edit.
  - App, `on_settings_event`: `HeaderRenamed` commits `RenameSpace` at once,
    through the same helper the desktop rename's commit uses (extract it
    from `finish_rename` if it is inline), ignoring an empty or unchanged
    title. `HeaderIconPressed` calls `pick_space_icon` (S10); the picker is
    owned by the settings window. Neither goes into `SpaceSnapshot`, so
    Cancel leaves both (D16), and add them to `CONTEXT.md`'s "these also
    commit at once" list.
  - Tests: both events reach their `Command`; an empty or unchanged title
    commits nothing; a rename made this way survives settings Cancel.
  - **[needs human]** check: on a space's page, double-click the name: an
    edit opens with the name selected. Type, then Enter: the header, the
    scopes pane, and the space on the desktop show the new name, and the
    window stays open. Esc in the edit keeps the old name and the window
    open. During the edit, click the icon: the picker opens over the
    settings window, and a choice shows at once in the header, the scopes
    pane, and on the desktop. Cancel afterwards keeps both changes.
    Global's header does not edit.

- [ ] **S13. Credits (app, docs).**
  - `settings/pages.rs` `about_page`: one `Field::note` (`ABOUT_ICONS`, the
    next free id) with a line per `ICON_SETS` row: "Icons: `label` by
    `author`, `license` License." One field whatever the number of sets,
    so a pack is never a new id.
  - `NOTICE`: the Fluent UI System Icons copyright line and MIT license
    text, under a heading naming the set and its upstream URL.
  - Tests: the About page lists every row; `NOTICE` names every row's label
    (read it as `deskspace-update`'s tests read `release.yml`).

## T. Animations (ROADMAP D12, D13, F-33)

Read D12 first; it is the spec. T1 is a spike only a human can judge, and
T2-T11 do not wait for it: they animate today's windows, where the accent
blur steps on and off. T12 (done) moved spaces to composition, and T13
(done) made the blur follow every effect. One commit each, in order
(T1 can run alongside).

Two tables carry the design, so a new effect or kind is one row: the
effects (what each one uses) and the kinds (which effects and controls each
offers). The app turns them into a `Play` per window; the platform plays it
with pure frame math, one frame per display refresh
(`win32/frame_clock.rs`), and never calls back into the app per frame.
Paint code applies a frame without allocating; whatever an animation needs
(an offscreen bitmap, a mask) is made when it starts. The window side is
`win32/space_animation.rs`.

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
  dark disk opens on the desktop beside the space (an `Overlay`, as
  Burn's flames are) and the space spirals into it: a
  composition visual can rotate and scale with its blur, which the accent
  path cannot. Decide where the hole opens and what Windows 10 plays
  instead, then it is one `EFFECTS` row plus its frame math and drawing.

T19-T21 add the effects D12 plans (amended 2026-10-18). They animate
today's windows and do not wait for T1 or T12. Each one adds an effect the
same way:

1. `deskspace-core/src/animation.rs`: an `Effect` variant with a `///`
   doc, and an `EFFECTS` row naming its key and options. The key is saved
   in the config, so it is never renamed. With `Direction`, Automatic is
   the top unless the row adds `.auto_edge(...)`. A kind lists it only
   where the task says (`KINDS`; Startup and Delete offer `Offers::Every`,
   so they get it with no edit). The compiler then points at the core
   test's `every_effect`.
2. `deskspace-app/src/settings/tables.rs`: a row in `EFFECT_LABELS` (grow
   the array's length by one).
3. `deskspace-platform/src/render/transition.rs`: the effect's arm in the
   frame `match` (the compiler asks for it), next to the effect it builds
   on. Pure frame math goes in its own `render/<effect>.rs` when it is
   more than a few lines, as `render/burn.rs` is. If it draws the
   contents a new way, that is a `Content` variant, drawn in the
   painter's `match` on it (`render/d2d/`); if it draws beyond the
   space's window, an `Overlay` variant in `win32/overlay.rs`. Both
   `match`es ask for every case; `overlay::kind_for` asks every effect.
4. Tests beside the pure math. Then add the effect to the T8 row in
   section V ("Each Startup effect") rather than adding a new row, unless
   the task asks for its own check.

- [ ] **T19. Bounce (core, platform).** **[needs human]**
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
- [ ] **T20. Scroll (core, platform).** **[needs human]**
  - Row: `effect(Effect::Scroll, "scroll", &[Direction])`, and add
    `Effect::Scroll` to Fold's `Offers::Only` list in `KINDS`.
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
  - Frame: a `Content::Strips` variant carrying the radius and the
    progress, which the painter turns into its strips.
  - Painter (`render/d2d/`, its `match` on `Content`): the flat part
    draws as Roll's does. The cylinder draws each strip from the snapshot the animation
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
  - Window: an `Overlay::Lamp` variant (`win32/overlay.rs`, and
    `kind_for` gives it to `Effect::MagicLamp`), on `win32/lamp_overlay.rs`:
    a click-through, layered, never-activated window over the space's
    monitor work area, built as `win32/flame_overlay.rs` is. While Magic
    lamp plays, the space's own
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

- [ ] **T22. Own-blur spike (platform).** **[needs human]**: does a blur
  DeskSpace draws itself look and run well enough to replace Windows'? A
  throwaway example, never shipped, as T1's is; the answer may amend D12's
  "The blur" (asked 2026-10-18, maintainer, after T13).
  - Why: Windows' blur is a host backdrop DWM samples, so its pixels are
    never ours; T13 controls only where it shows. Spaces sit at the bottom
    of the z-order, so what is behind one is mostly the wallpaper. Drawn
    by the painter, the blur would be ordinary pixels: every effect, any
    corner radius, and Windows 10 the same, at almost no idle cost.
  - What it is: `crates/deskspace-platform/examples/own_blur_spike.rs`, a
    window styled as a space beside a real Acrylic space. It reads the
    wallpaper and its placement for its monitor (`IDesktopWallpaper`:
    `GetWallpaper`, `GetPosition`, `GetBackgroundColor`), blurs it with
    Direct2D's Gaussian blur, adds a tint, luminosity, and noise as acrylic
    does, and draws the part under the window; it redraws when moved and
    on `WM_SETTINGCHANGE` with `SPI_SETDESKWALLPAPER`. Hotkeys as in T1.
  - Not covered on purpose: anything else behind a space (desktop icons,
    another space, windows under `--on-top`) is not in its blur; capturing
    the screen instead (Windows.Graphics.Capture) costs too much and lags.
  - Check (maintainer): beside the Acrylic space, at 100 % and 150 %, how
    close the two look in light and dark; a fit, fill, stretch, tile,
    span, and solid-color wallpaper, a slideshow, and Spotlight; two
    monitors with different wallpapers; dragging it shows no lag or seam;
    idle CPU and GPU.
  - Done when: the findings are recorded here, and a decision in D12
    either keeps Windows' blur (delete the example) or plans the move
    (new tasks, then delete this one).

### Burn, upgraded (D12, amended 2026-10-22)

T24-T33 make Burn read as fire rather than an orange wipe: it takes its
time and kindles, spreads unevenly, smoulders, throws turbulent flames and
ash, and looks the same at every DPI. One commit each, in order.

- **When to start.** Blocked: T24-T33 build on three things that are on
  `feat/color-picker-redesign` (as of `c1ad0f3`) but not in `main`, because
  only that branch's color picker was ported: `render/noise.rs`
  (`RisingField`, `fractal`, `random`), `win32/effect_overlay.rs`
  (`EffectOverlay`, `Art::Burn`, `Margins`), and `Play::seed`. Port that
  groundwork as its own task first, or stop and ask; do not copy it across
  by hand. Then T24 starts.
- **Units.** "px" means pixels at 96 DPI. From T26 on, multiply every px
  constant by the window's scale (`dpi as f32 / 96.0`).
- **Reversible.** Every frame stays a pure function of the progress, the
  seed, and the clock. The clock only drives flicker and never moves the
  front, so reversing mid-burn still retraces the same edge.
- **On-screen checks.** Each task marked **[needs human]** appends its
  check to the Burn row in section V (`T17, T18 (D12)`) and adds its id
  to that row's first column, instead of adding a new row.
- **Burn's smoke.** If Burn's smoke (`render/burn_smoke.rs`) has landed,
  keep the overlay's drawing order: smoke, then flames, then ash.

- [ ] **T24. Each effect sets its own pace (core, app, platform).**
  **[needs human]**
  - Why: D12 (amended 2026-10-22). At the default 250 ms, Burn is over
    before its flames read. Smoothstep, the one curve every effect uses
    (`ease` in `render/transition.rs`), slows at both ends, while a fire
    catches slowly and then races. An effect's pace belongs in its
    `EFFECTS` row, so Burn needs no `if`.
  - `deskspace-core/src/animation.rs`:
    - `pub enum Curve { Smooth, Kindle }`, with a `///` doc on each, and
      `pub fn apply(self, u: f32) -> f32`. `Smooth` is today's smoothstep;
      move `ease` and its comment here. `Kindle` is `1 - (1 - u)²`, where
      `u` is how much of the space is shown. An out (u from 1 to 0)
      starts slowly and ends fast; an in starts fast and settles.
    - `pub struct Pace { pub stretch_percent: u16, pub curve: Curve }`,
      `pub const PACE_NORMAL: Pace` (100, `Smooth`), `pub const
      PACE_STRETCH_RANGE: (u16, u16) = (50, 300)`, and `pub fn
      stretch(self, duration: Duration) -> Duration`.
    - `EffectSpec` gains `pace: Pace`, and `effect()` takes it as a fourth
      argument. Every row gets `PACE_NORMAL` except Burn, which gets
      `Pace { stretch_percent: 200, curve: Curve::Kindle }`.
  - `render/transition.rs`: `Transition::frame` applies
    `self.play.effect.spec().pace.curve` where it now calls `ease`.
    Delete `ease`.
  - `deskspace-app/src/app/animation.rs`: `play_for` stretches the chosen
    duration (`spec().pace.stretch`) before `tweak.slow`. The exit
    stagger (`exit_delays`) already uses `play.duration`, so it then
    allows for the stretch with no further change.
  - The settings page does not change: the Duration control still shows
    the chosen time.
  - Tests:
    - core: every row's stretch is within `PACE_STRETCH_RANGE`; every
      curve is 0 at 0 and 1 at 1, and never decreases over 101 even
      samples; `Kindle.apply(0.9)` is above 0.98 (a burn barely moves in
      its first tenth).
    - app: `play_for` gives Burn at 250 ms a 500 ms play and Roll at
      250 ms a 250 ms one. With Burn on Startup at 1000 ms and ten
      spaces, the last one still ends within `EXIT_LIMIT` (copy the
      existing exit test).
  - `CONTEXT.md` section 4, "Animations": one sentence saying an effect's
    row also sets its pace.
  - **[needs human]** check: Burn at 250 ms plays for about half a
    second, catching slowly and then racing to the end; un-burning starts
    fast and settles; every other effect looks and times as before.

- [ ] **T25. One seeded Burn field per play (platform).**
  **[needs human]**
  - Why: every burn has the same shape, because the noise has no seed.
    At exit, same-sized spaces burn identically one after another. The
    painter (`Effects::burn` in `render/d2d/effects.rs`) and the overlay
    (`Art::Burn`) also each build a `BurnField`, and they agree only
    because both happen to compute the same columns and rows.
  - `render/noise.rs`: `fractal` and `RisingField::new` take `seed: u32`.
    Mix it into the lattice hash as the octave is mixed in:
    `^ seed.wrapping_mul(0x27D4_EB2F)`. Seed 0 must give exactly today's
    values; callers other than Burn pass 0.
  - `render/burn.rs`: `BurnField::for_space(size: (f32, f32), detail: u8,
    seed: u32) -> Self` becomes the only public constructor (`new` goes
    private). It is the one place the cell grid is worked out from a size
    in pixels. The painter passes its `size`, and the overlay passes
    `(rect.width as f32, rect.height as f32)`.
  - `Mask::Burn` gains `seed: u32`, filled from `play.seed` in the Burn
    arm of `frame` (as `Mask::Smoke` does). The painter's rebuild check
    (`fits` in `Effects::burn`) compares the seed too. `Art::Burn` takes
    the seed from the `Play` it is made for.
  - Tests:
    - noise: seed 0 gives the values it gave before; pin three values
      in a test before changing the hash.
    - burn: two seeds give different fields; one seed twice gives equal
      fields; `for_space((301.0, 199.0), 5, 1)` has the cols and rows the
      painter and the overlay used to compute.
  - **[needs human]** check: at exit, two spaces of the same size burn
    along different edges; one space burns differently each time it is
    hidden; reversing mid-burn still un-burns along the same edge.

- [ ] **T26. Burn scales with DPI (platform).** **[needs human]**
  - Why: `burn::CELL` (3), the overlay `Margins`, and the flame sizes
    (`HEIGHT_RANGE`, 18 to 60) are physical pixels. At 150 % the flames
    are two-thirds the size and the edge looks finer.
  - `Geometry` (`render/transition.rs`) gains `dpi: u32`, and
    `for_visibility` and `for_fold` take it. Every call in
    `win32/space_animation.rs` passes `window_dpi(window)`
    (`win32/dpi.rs`). `Mask::Burn` gains `dpi: u32`, filled from
    `geometry.dpi`.
  - `render/burn.rs` stays free of Win32. Add `pub fn cell_px(dpi: u32) ->
    u32`, which is `(CELL * dpi + 48) / 96` and at least 1. `for_space`
    takes `dpi` and uses it for the grid.
  - `render/flames.rs`: `pub fn margins(dpi: u32) -> (u32, u32)` (side,
    top) replaces the `MARGIN_SIDE` and `MARGIN_TOP` constants; the
    overlay builds its `Margins` from it. `tongues` takes `scale: f32` and
    multiplies `HEIGHT_RANGE`, the sway, and the ember sizes by it.
  - Tests: `cell_px` is 3, 5 (rounded from 4.5), and 6 at 96, 144, and 192
    DPI; a 600 × 400 px field at 192 DPI has half the columns it has at
    96; tongues at scale 2 are twice as tall for the same bases (scale
    the bases' rows too); `margins(144)` is 1.5 times `margins(96)`,
    rounded.
  - **[needs human]** check: on a 150 % and a 200 % monitor, Burn looks
    the same as at 100 %, only bigger: the edge is as ragged relative to
    the space, and the flames are as tall relative to the title bar.

- [ ] **T27. The field owns its bands (platform; refactor).**
  - Why: `GLOW`, `CHAR`, and `SCORCH` are global field units, so their
    width in pixels depends on the noise and on the space's height. T28
    gives each field its own bands; this task moves the code first, with
    no visible change.
  - `Mask::Burn` carries `gone: f32` (0 to 1) instead of `front`. The
    free `front(gone)` and `stage(above)` become methods:
    `BurnField::front(&self, gone)` and `BurnField::stage(&self, col, row,
    front)`. The field stores `bands: Bands { glow, char, scorch }` (field
    units), set by `for_space` to today's constants, which then go.
  - `TransitionFrame`'s checks against the front (`front <= -burn::BANDS`
    in `covers_window`, and `front >= 1.0` in the tests) become
    `gone <= 0.0` and `gone >= 1.0`. `fill_burn`, `flame_bases`, and
    `Art::Burn` take the front from the field.
  - Tests: today's `burn.rs` tests, rewritten against a field, pass
    unchanged in meaning. No new behavior, so no on-screen check; say so
    in the commit body.

- [ ] **T28. Burn spreads like fire (platform).** **[needs human]**
  - Why: a threshold sweeping through noise moves at one speed along a
    blobby edge. Fire catches unevenly, climbs faster than it spreads,
    runs up in fingers, and burns holes that join the main front.
  - `render/burn.rs`: a cell's value becomes when the fire reaches it,
    scaled to 0 to 1. Leave the stages, `cell_pixels`, and the painter as
    they are.
    - **Coarse grid.** Solve on a grid of `cols.div_ceil(4)` by
      `rows.div_ceil(4)` nodes, so a full-HD space stays cheap.
    - **Speed** of a node: `SPEED_MIN + (SPEED_MAX - SPEED_MIN) *
      fractal(..)`, with the seeded `noise::fractal` at today's lattice
      for the detail. `SPEED_MIN` is 0.35 and `SPEED_MAX` is 1.0.
    - **Steps.** A step to each of the 8 neighbours costs
      `length / (speed(to) * climb)`. `length` is 1 for the four sides
      and √2 for the diagonals. `climb` is `CLIMB_UP` (1.8) toward
      row 0, 1.0 level, and `CLIMB_DOWN` (0.45) away from it.
    - **Sources.** Each bottom-row node starts at `BOTTOM_STAGGER`
      (0.2) × node rows × `noise::random(seed, col)`. Spot fires add
      `(nodes / SPOT_AREA).min(SPOTS_MAX)` sources (`SPOT_AREA` 600
      nodes, `SPOTS_MAX` 3). They sit at seeded nodes in the lower 60 %
      of rows and the middle 80 % of columns, and each starts at
      `SPOT_DELAY` (0.3) × node rows.
    - **Solve** with Dijkstra's algorithm: `std::collections::BinaryHeap`
      over `Reverse` of an `f32` wrapper ordered by `total_cmp`. Cite
      https://en.wikipedia.org/wiki/Dijkstra%27s_algorithm beside it.
    - **Cell values.** A cell takes the coarse arrival at its position
      (bilinear, as `RisingField::value_at` samples), divided by the
      largest arrival. Then add `EDGE_RAG` (0.015) × (fine
      `noise::fractal` − 0.5), so the edge stays ragged like paper, and
      clamp the result to 0 to 1.
    - **Bands from pixels.** The field keeps `units_per_cell` (1 / the
      largest arrival, divided by 4 for the coarse grid). `for_space`
      sets each band to `px / cell_px(dpi) * units_per_cell` from
      `GLOW_PX` 3, `CHAR_PX` 5, and `SCORCH_PX` 12.
  - `for_space` logs how long it took at `debug`. Both the painter and
    the overlay build the field, so in a release build a 1920 × 1080
    space at 100 % must build in under 3 ms. Put the timing in the
    commit body.
  - Tests: values stay within 0 to 1; the row means rise from the bottom
    up (keep the test); a pure `step_cost(length, speed, climb)` costs
    less up than level, and less level than down; a 300 × 300-node field with
    seed 1 has a column where a higher cell burns before a lower one (a
    hole); the median glow thickness, in pixels, of a 100 px and a
    400 px tall field differs by less than a factor of two.
  - **[needs human]** check: the bottom catches unevenly; the fire runs
    up in fingers; on a large space, a hole or two opens in the middle
    and joins the main front; the glowing edge is a similarly thin line
    on small and large spaces; Detail still coarsens it; startup and
    exit with many spaces do not stutter.

- [ ] **T29. Turbulent flames (platform).** **[needs human]**
  - Why: today's flames are identical teardrops every `SPACING` cells,
    swaying on one sine, and overlapping fire keeps the brighter channel
    (`brightest`) instead of adding up.
  - `render/flames.rs`:
    - **Sizes.** `tongues` takes the seed. Each tongue's height is
      multiplied by `0.6 + 0.7 * noise::random(seed, col)`.
    - **Turbulence.** In `paint_tongue`, multiply the heat by
      `0.45 + 1.1 * n` and clamp it to 0 to 1. `n` is one octave of the
      noise in `render/noise.rs` (make `value_noise` `pub(crate)`),
      sampled at `px / (tongue.width * 0.6)` across and `(py + time *
      RISE_PX_PER_S * scale) / (tongue.height * 0.3)` down, so the
      pattern scrolls upward. `RISE_PX_PER_S` is 120. Keep the 0.02 cut.
    - **Additive light.** Replace `brightest` with `add`: each byte of
      the two premultiplied pixels added and capped at 255. This keeps
      the pixels premultiplied (each channel ≤ alpha stays true).
    - **Sparks** take their phase from the seed instead of the index
      alone.
  - In a release build, a frame of flames for a 1200 × 900 space shades
    in under 4 ms. Measure it, log it at `trace`, and put the timing in
    the commit body.
  - Tests: keep today's; the same seed and time give identical pixels,
    and two times differ; `add` of two half-alpha pixels is brighter
    than either and stays premultiplied; two seeds give different tongue
    heights for the same bases.
  - **[needs human]** check: the flames lick and break up at the tips;
    neighbours differ in size; where flames cross they glow brighter; no
    regular comb of tongues; still no box around them.

- [ ] **T30. A glow that lives (platform).** **[needs human]**
  - Why: the glowing edge is a fixed colour ramp, the char is flat, and
    the paper ahead gets no light from the fire.
  - Clock: `frame` (`render/transition.rs`) takes `time: f32`, the
    animation clock in seconds that the flames already use
    (`now().as_secs_f32()` in `space_animation.rs`). `Mask::Burn` carries
    it. Only Burn reads it. Flames and glow then flicker together.
  - `render/burn.rs`:
    - `cell_pixels(stage)` becomes `cell_pixels(stage, shimmer)`.
      `shimmer` is `0.5 + 0.5 * sin(time * rate + phase)`, where `phase`
      comes from `noise::random(seed, cell index)` × 2π. Work it out only
      for cells in a band; Whole and Gone cells skip it.
    - Burning: brightness × `(0.82 + 0.18 * shimmer)`, rate 13.
    - `Stage::Charred` gains `along` (0 at the glow, 1 at the scorch).
      Ember cells, those with `noise::random(seed ^ EMBER_SALT, index)`
      below 0.1, lay ember colour (255, 110, 30) over the char at alpha
      `(1 - along)^1.5 * (0.4 + 0.6 * shimmer)`, rate 5. `EMBER_SALT` is
      a named constant.
    - A new `Stage::Lit(near)` follows `Scorched`, `LIGHT_PX` (24) deep.
      `near` is 1 next to the scorch and 0 at untouched paper. It lays
      premultiplied (255, 150, 60) at alpha `near² * 48 * (0.75 + 0.25 *
      shimmer)`. The light band counts toward the field's bands, so at
      gone 0 nothing is lit.
  - `fill_burn` (`render/d2d/effects.rs`) passes the clock; the marks
    bitmap is already rewritten every frame, so nothing else changes in
    the painter.
  - Tests: Whole and Gone pixels are as before; at one time, ember cells
    near the glow are brighter than other char cells; Lit's alpha is at
    most 48 and 0 at its far edge; Burning pixels differ between two
    times but stay opaque; every pixel stays premultiplied.
  - **[needs human]** check: the edge shimmers; embers wink in the char
    and die out toward the scorch; warm light on the paper just ahead of
    the edge flickers with the flames; at rest after a show, no tint is
    left on the space.

- [ ] **T31. Ash (platform).** **[needs human]**
  - Why: burned-away paper just disappears. Black flakes lifting off
    the edge sell the fire.
  - Pure, in a new `render/ash.rs`:
    - **Flake cells.** `for_space` picks them once: the cells with
      `noise::random(seed ^ ASH_SALT, index)` below `ASH_SHARE` (0.008).
      They are stored on `BurnField`.
    - **`ash::flakes(field, front, time, scale, out: &mut Vec<Flake>)`.**
      A flake lives while `0 < front - value < ASH_LIFE` (0.12 field
      units). Its `life` is `(front - value) / ASH_LIFE`. Life depends
      on the front, not the clock, so un-burning brings the flakes back
      down into the paper.
    - **Position.** A flake starts at its cell's centre in overlay
      pixels and rises `life * ASH_RISE_PX` (90) × scale. It drifts
      sideways by `sin(time * 1.7 + phase) * 10 * scale * life`.
    - **Shape.** An ellipse `3 * scale` px tall and
      `3 * scale * |cos(angle)| + 0.5` px wide, so it tumbles.
      `angle = time * spin + phase`, where `spin` (−4 to 4 rad/s) and
      `phase` come from the flake's seeded random.
    - **Colour.** (22, 18, 16) at alpha `(1 - life)^0.7 * 230`. While
      `life < 0.25` it has a 1 px rim of `fire(0.6)`; make `fire`
      `pub(crate)`.
    - **`ash::paint`.** Source-over onto the premultiplied pixels:
      `out = src + dst * (1 - src.alpha)`.
  - `Art::Burn` paints the ash after the flames. Flakes that leave the
    overlay are skipped.
  - Tests: no flakes at gone 0; a flake is higher at a larger front for
    the same time; the same inputs give the same flakes; `paint` keeps
    pixels premultiplied and never writes outside the buffer.
  - **[needs human]** check: black flakes, some with glowing rims,
    tumble up off the burning edge and fade; reversing mid-burn sinks
    them back in; no flake is left on screen after the burn ends.

- [ ] **T32. Flames on the GPU on Windows 11 (platform).** **[needs
  decision]**: whether a click-through overlay can show composition
  content. Proposed below; a maintainer decides after step 1, then the
  task is **[needs human]** and goes to a junior paired with a reviewer.
  It comes after T29-T31, whose CPU look is the reference.
  - Why: the CPU shades every overlay pixel each 15 ms tick (about 4.7 MB
    for a 1200 × 900 space) and then calls `UpdateLayeredWindow`. It
    cannot afford bloom.
  - **Step 1, spike.** Make the overlay with `WS_EX_LAYERED |
    WS_EX_TRANSPARENT | WS_EX_NOREDIRECTIONBITMAP` and a composition
    target (copy the tree setup from `win32/composition.rs`). Clicks must
    pass through it: run `scripts/zorder.ps1` over a burning space. If
    they do not, record that in D12 and delete this task and T33.
  - **Step 2, draw on the GPU.** On the composition path, `Art::Burn` draws
    into a composition drawing surface with Direct2D instead of
    `flames::paint`. Each tongue is a teardrop path, filled with a
    seeded 256 × 256 noise bitmap (made once per play) through a bitmap
    brush that scrolls up at `RISE_PX_PER_S`. The heat ramp is applied
    by `CLSID_D2D1TableTransfer`, and flames blend with
    `D2D1_PRIMITIVE_BLEND_ADD`.
  - **Step 3, bloom.** The flame layer is drawn a second time, under itself,
    through `CLSID_D2D1GaussianBlur` (standard deviation 6 × scale) at
    60 % opacity.
  - **Step 4, fallback.** Windows 10 and the accent path keep today's CPU
    overlay. Both paths share `flames::tongues`, so the layout tests
    cover both.

- [ ] **T33. Heat haze (platform).** **[needs decision]**: after T32,
  and only on composition spaces, because their painter draws through an
  `ID2D1DeviceContext` (`SurfaceCanvas`). The plain `ID2D1HwndRenderTarget`
  has no effects, so other spaces would play without haze. Proposed: the
  painter draws the space's contents through `CLSID_D2D1DisplacementMap`.
  The map is a scrolling noise bitmap, strongest `HAZE_PX` (20) above the
  burning edge and gone beyond 60 px. The scale is 4 × scale px, so the
  paper shimmers above the fire. Decide whether a look that differs by
  path is acceptable, then this is **[needs human]**.

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

- [ ] **K8. Split `win32/space_window.rs`.** About 1,200 lines of non-test
  code (measured 2026-10-27), past the guideline since T12. Turn it into
  `win32/space_window/mod.rs` plus:
  - `zorder.rs`: the z-order pinning (`set_on_top`,
    `on_window_pos_changing`, `insert_after`, `window_above`) with their
    tests.
  - `mouse.rs`: the mouse handlers (`on_mouse_move`, `on_mouse_leave`,
    `on_mouse_wheel`, `on_button_down`, `on_button_up`) and their helpers
    (`to_desktop`, `Modifiers`, `modifiers`, `button_for`, the `MK_*`
    constants).
  - `frame.rs`: `Frame` and `update_region`, with their tests.
  - `mod.rs` keeps `SpaceState`, `Backing`, creation, the `set_*` and
    `show*` functions, `paint`, `with_state`, and the window procedure.
  - Done when: `mod.rs` is under about 800 lines of non-test code and the
    K rules above hold.

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

Each needs design work first. Ids are `R`-numbered so they do not collide with
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
- **R19. A space window handle instead of `unsafe fn` contracts.** About
  twenty functions in `win32/space_animation.rs` and
  `win32/space_window.rs` are `unsafe fn` only for "`window` is a live
  space window of this thread, and no state borrow is held", each call
  repeating `// SAFETY: the caller's contract.` Design a handle that only
  the runtime makes for a live space window, whose methods are safe.
  Settle: how the borrow rule is kept (a guard, or `with_state` private to
  the handle), how the desktop layer and a dying window fit, what
  `WM_NCDESTROY` invalidates, and how the handle stays on its thread.
  Then split it into one-commit tasks by module here. It touches
  `window_state.rs` and every window module, so plan it with K8.

## O. On-screen checks a script can do

Much of section V needs no eyes. Windows reports which window sits where,
its styles, and what a click reaches (`scripts/zorder.ps1` already does),
and a screen capture over a plain backdrop turns "a soft shadow" into
numbers. `scripts/onscreen/OnScreen.psm1` is that harness, and `smoke.ps1`
and `shadows.ps1 [-Pixels]` are the checks built on it (O1-O3, archived).
Add a task here to script another row. A row a script covers is cut from
section V by the V rules; what a script cannot judge (taste, Windows 10, a
DPI this machine lacks) stays there.

The harness is PowerShell with C# P/Invoke, like `zorder.ps1`, so no
`unsafe` or Win32 code lands outside `deskspace-platform` (`ENGINEERING.md`
section 3). It only reads: it changes a scratch config and its own
backdrop window, never a Windows setting. A check whose branch this
machine's settings do not select (Transparency effects on or off, a 150 %
monitor) is reported as **not covered**, never passed.

- **R20. Test the app without Win32 (`ENGINEERING.md` section 3).**
  `deskspace-platform` is about 55 % of the code (2026-10-21) and holds two
  things: the data the app hands it (`SpaceScene`, `Event`, `SettingsForm`,
  `MenuEntry`, `Transition` and the effect math, `TextEdit`), which is
  pure, and the Win32 code that turns that data into windows. The app's
  tests are nearly all pure, yet they compile the `windows` crate, and
  any platform change re-tests the app. Design the split and settle:
  - a crate below `deskspace-platform` for the pure data and math (for
    example `deskspace-ui`), and exactly what moves into it;
  - how the app reaches the platform without naming the concrete `Ui`
    struct: a trait the platform implements, or more of R17's `Effect`,
    where a pure decision returns what to do and one place runs it;
  - the free functions the app calls directly today (`item_facts`,
    `desktop_directory`, `notify_moved`, ...): which go behind the same
    seam;
  - the order of the work, so each step is a pure move (the section K
    rules) and the gates pass after each.

  Then split it into one-commit tasks here.

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
| Drawn shadows (D15) | Windows 11 with "Transparency effects" on, at 100 % and 150 % (O2 and O3 ran at 125 %), over Acrylic and Blur as well as Solid: the shadow reads as a soft shadow, not a box. With "Transparency effects" off, Solid at 16 px shows a drawn shadow while 4 and 8 px keep DWM's own; Windows 10 casts one at every radius. On a machine with a desktop host, `--placement desktop-*` casts none (O2 could not cover that here). Roll, Fly, and CRT move and crop the shadow with the space; Fade, Dissolve, Gradient, Pixelate, and Burn fade it out to nothing with no ghost box left. Settings > Appearance: "Show shadow" greys "Shadow size" and "Shadow opacity"; Size (2–48 px) and Opacity (0–100 %) preview live and Cancel restores. `--debug` → Windows: "Outline every shadow window" draws a 1 px magenta box around each one, "Hide every shadow" hides them for the session, the "Shadow settings" box edits and saves the three settings (reflected in an open Settings window, kept on Cancel), and the Windows table lists each `DeskSpaceShadow` below its space. The structure and the pixels (one per space, z-order, rect, ex-styles, click-through, Clear/desktop/layer, show_shadow off, a per-space size, half the setting beside and three quarters below, both falloffs to 0, the drop band, the corner, opacity 0, and the Solid cut-out) passed O2 and O3 on 2026-10-27 at 125 % on the composition path (`archive/ROADMAP-progress-log.md`). |
| Animation refactor | The 2026-10-19 animation and painter refactors change no pixels, so check that everything looks as before, at 100 % and 150 %, on Acrylic and Solid: every effect on hide, show, Delete, and Fold, and Gradient, Roll, and Fly from each edge (Automatic: Roll and Gradient from the top, Fly to the nearest edge); Pixelate, Dissolve, and Burn at detail 1 and 10; the debug window's preview on one space and on all, pressed twice mid-way, its Kind box named Startup, Delete, Fold, and slow motion; exit plays every space out in reverse reading order. Items, labels, check boxes, the focus rectangle, the rename box, the lock glyph, and the settings highlight sit where they did; the drag image, the debug badge on the window icon, and the zoom pop-up look as before. |
| Flames follow | With Startup and Delete set to Burn at 1000 ms (or `--debug` at ¼×), change a space's rect while it burns: move or resize it in Settings and Apply mid-hide, and change the display layout mid-show. The flames stay on the space's burning edge at its new place, never left where it was; a resized space's flames stand on its new edge. No flame window is left after the burn ends. |
| Frame clock | On a 60 Hz and a 144 Hz display, and on a space moved to a second monitor with another refresh: Roll, Fade, Fly, CRT, and Burn on hide and show at 1000 ms are smooth, with no hitch or jump; `--debug` at ⅒× speed is smooth too; hiding during the startup stagger, and the hotkey twice mid-animation, behave as before. With no animation running, Task Manager shows no "frame clock" thread busy and idle CPU is as before. Lock the session mid-animation: it finishes, and the app stays responsive after unlocking. |
| No DWM shadow | Windows 11 with "Transparency effects" on, Acrylic, Blur, and Solid at 4 px and 8 px corners, at 100 % and 150 %: a space at rest has smooth rounded corners with no shadow; Fade, Dissolve, Gradient, Pixelate, and Burn on hide, show, and Delete leave no ghost box or outline at any point, and nothing pops off at the end; Roll rolls to a 1 px line with nothing left around it; CRT and Fly look as before. With "Transparency effects" off, spaces keep DWM's corners as before. |
| Flames under apps | Without `--on-top`, with a space partly covered by an app window (an editor, a browser): Burn on hide, show, and Delete draws its flames over the space but under that app, never over it, including where the flames rise past the space's top edge; an uncovered space's flames still show in full. Switching to another app mid-burn keeps the flames under it. |
| Window icons | At 100 % and 150 %, with the taskbar shown and auto-hidden: the settings window's taskbar button and Alt+Tab entry show the DeskSpace icon, sharp, and its title bar the small one; `--debug`'s window shows the same icon with a white bug on an amber disc in the bottom-right corner, in all three places, also on the first start of `./scripts/dev.ps1 -Settings --debug`, when both open during startup. Move the debug window to a monitor with another DPI: the icon still shows. The tray icon looks as before. |
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
| C2 (F-29) | With "Use Windows accent" on, the selection takes the accent (and the border, with "Border color" set to Windows accent), and they follow it live when it changes in Windows Settings, also with "Automatically pick an accent color from my background" and a wallpaper slideshow; switching wallpapers with an unchanged accent causes no flicker. |
| C3 (F-30) | Switching "Settings and menus" between Dark and Light repaints the open settings window and the next popup menu. The selection bar and marked dots, and the color picker's selection ring, take the Windows accent (a lighter shade in dark mode, a darker one in light mode, readable in both) and follow an accent change in Windows Settings while the window is open. |
| C4 (F-34) | "Start DeskSpace when you sign in" adds and removes the `Run` value, and DeskSpace starts at sign-in. |
| C5 (F-48) | The "Item preset" dropdown shows both entries; "Custom" enables the Layout fields and the icon size takes effect; "Match Explorer's display" greys them. |
| C7 (F-18) | Each hotkey fires once bound; a conflicting binding is reported beside its field and the window stays open; clearing a binding stops it. |
| C8 (F-17) | A space created from the space menu, the tray, and the hotkey uses the configured title, size, and lock state; a locked new space keeps its size and scrolls. |
| E1, E2 | `deskspace.exe` shows the icon in Explorer's small, medium, and extra-large views and the version under Properties > Details; the tray, settings title bar, and Shift+right-click desktop verb show the icon at 100 % and 150 %. |
| E5-E7, E10-E12 | Covered by the R12 dry run (installer, uninstall, the watchdog in the zip, update from one pre-release to the next). |
| F-45 automatic install | Installed build with a feed: turn on both update toggles; after a check finds a newer release, the tray shows "Restart to update to X"; 10 minutes without input (no full-screen app) installs silently and DeskSpace comes back with the same spaces, with takeover on the desktop icons are never left blank; the tray entry installs at once with a progress window; a portable copy greys the install toggle. |
| Update install failure | Installed build with a feed and a newer release: in PowerShell set `$env:TEMP` and `$env:TMP` to a missing drive (`Z:\none`), start `%LocalAppData%\DeskSpace.App\deskspace.exe --hidden`, then "Check for updates…" → "Update and restart": DeskSpace quits, comes back hidden on the old version with the same spaces, and a "DeskSpace was not updated" box in front of other windows names the error. With takeover on, the desktop icons are never left blank. |
| F1-F5 | F2 with no item selected renames the space title, with one selected the file; Alt+Enter opens Properties, or Settings with nothing selected; Enter opens the item; Esc clears the selection and still closes an open menu; Shift+Delete bypasses the Recycle Bin. With takeover on, Delete and Shift+Delete on one and on several selected loose desktop icons do the same, and the icons leave the layer at once. |
| F9 | With "Snap resized spaces to whole columns and rows" off, a resize keeps the dragged size (still snapped to `snap`). |
| F14 (F-51) | The color picker opens from a Colors-page swatch and a space's color menu; on Windows 11 22H2+ it is acrylic, else opaque, rounded and bordered in both modes; the square, hue, and opacity sliders drag smoothly; a swatch click selects and a double-click applies; typing `#3B82F6`, `rgb(59, 130, 246)`, or `217°, 91%, 60%` switches the radio button and the color, invalid text shows a red message and is dropped on leaving the box; Ctrl+V pastes, the copy button and Ctrl+C copy; the eyedropper samples any screen pixel live, a click keeps it and Esc or a right click cancels; Current goes back; the title strip drags it; it is sharp at 100 % and 150 % and after moving to another monitor; recent colors survive a restart and a settings Cancel; Apply, Cancel, the X, Enter, and Esc behave and the settings window does not hang. |
| G1, G3 | On the desktop shell: typing selects by prefix and the buffer expires; Home/End/PageUp/PageDown scroll; arrows move the selection, Shift extends, Ctrl moves only the dotted focus rectangle and scrolls it into view. |
| G5a | Right-drag an item out of a space onto the desktop and into an Explorer folder: the drag shows the same translucent icon image a left drag does (not the shell's large opaque one), also for a multi-item selection and from the taken-over desktop; the shell's Move / Copy / Create shortcut menu appears and each choice works; a right click without moving still opens the item menu. |
| G5b | Right-drag a file from Explorer onto a space: DeskSpace's Move here / Copy here / Show here without moving / Cancel menu appears; each entry does what it says; the source file is left alone on Copy, Show here, and Cancel; a left drag still moves and Shift still keeps in place. |
| G5c | Right-drag a file and a `.lnk` from Explorer onto a space and choose "Create shortcuts here": the space gets a working shortcut to the file and a copy of the `.lnk`, and both originals stay; the entry sits between "Copy here" and "Show here without moving"; the same on the taken-over desktop opens a new space holding them. |
| Right-drop greying | Right-drag onto a space from a source that offers only copy (a file on a read-only share or a zip's contents): "Move here" is greyed, "Copy here" and "Show here without moving" work; from Explorer every entry is enabled. |
| H5, H4 (F-46) | Panes, hairlines, dimmed rows, and keyboard movement in both modes; a space is greyed until customized, then edits apply to it alone and Cancel restores; the switch off and on restores custom values; a menu edit with the window open survives Cancel; an old config with per-space `icon_size`/`tint` opens customized; the highlight outline is visible on Clear and Solid, and on Acrylic with Windows corners (border on and off, 100 % and 150 %) its corners are round, following the window's |
| I8 | With takeover on, creating, renaming, and deleting a desktop file updates the layer within about 150 ms; so does editing one in place whose icon is its content (save a new picture over a desktop `.png`, or re-point a desktop shortcut to another app), with no restart. |
| W2 (F-07) | With the hand-made lens in section W, create, rename, and delete files in its folder in Explorer: the lens follows within about 150 ms. Rename the folder away: the lens empties. Rename it back: it fills again within 5 s. |
| W3b (F-07) | Drop files from Explorer and from a normal space onto a lens (hand-made, section W): they land in its folder, and the normal space no longer lists its file. Drop a file whose name is already there: Explorer's Replace or Skip dialog appears. Ctrl+Z in Explorer undoes the move. A right drop, "Copy here": the original stays. "Add files…" on a lens moves the files into its folder. Regression: a clashing drop on a normal space still gets " (2)" and no dialog. |
| W3c (F-07) | Shift-drag a file from Explorer onto a hand-made lens (section W): the cursor shows "no" and nothing moves. Right-drag onto a lens: "Show here without moving" is grey. On a normal space, Shift and the right-drop menu behave as before. |
| W3d (F-07) | Drag an item from a hand-made lens (section W) onto a normal space: the file leaves the lens's folder, the space lists it, and the lens no longer shows it. Delete that space: the file goes to the desktop, not back to the folder (D3). |
| W3e (F-07) | In Explorer, copy a file and paste it on a hand-made lens (section W; click it, then Ctrl+V): a copy lands in the folder. Cut and paste: it moves. Cut an item from a normal space and paste it on a lens: it moves into the folder and leaves the space. A paste on a normal space works as before. |
| W3f (F-07) | Drop a Start-menu app on a hand-made lens (section W): a working `.lnk` is in the folder and the lens shows it. "New shortcut ▸ Paste shortcut" and the custom shortcut dialog on a lens do the same. On a normal space, shortcuts work as before. |
| W3g (F-07) | Select an item on a hand-made lens (section W) and press Delete: the file is in the Recycle Bin and the lens drops it. On the settings window's Shortcuts page, the X on a lens item says the file goes to the Recycle Bin, and it does. |
| W5 (F-07) | Point a lens at a throwaway folder, then rename the folder away: "Folder not available" and the path show, centered in the lens, in a faded label color; a long path wraps inside the lens. Rename it back: the items return within 5 s and the text goes. |
| W6 (F-07) | Right-click a hand-made lens (section W): Open folder opens it in Explorer; Change folder… picks another folder and the lens follows it, and picking DeskSpace's storage folder shows a message and changes nothing. Sort by has Explorer's shape (Name, Date modified, Type, Size; Ascending and Descending; Folders first) and checks the current key and direction; each choice reorders the lens at once, and Show hidden files toggles and applies at once. After a restart the sort rule (key, direction, folders first) and the hidden-files setting are still set. A normal space's Sort by looks the same, and a key choice sorts it once. |
| W7 (F-07) | Create a lens with "New lens…" from the space menu, the tray menu, and the desktop layer's menu (takeover on): each asks for a folder and opens a lens titled with the folder's name beside the top row of spaces, filled within a moment. Picking DeskSpace's storage folder shows a message and creates nothing. Cancelling creates nothing. |
| New space placement (D10) | With a few spaces in a row at the top of a monitor, New space from the space menu, the tray, the desktop menu, and the hotkey each opens to the right of the row, its top in line, with the same gutter as the others; with the row full to the screen edge it goes to the next row or under all of them; it never covers another space; on a second monitor it opens on the monitor of the click or the pointer; on an empty monitor it opens near the top-left corner; on a 150 % monitor the gutter looks the same. |
| Desktop menu (F-14) | With takeover on, right-click an empty spot of the desktop: New space, DeskSpace settings…, and Hide/Show spaces sit above Explorer's desktop menu; each DeskSpace entry works; View, Sort by, Refresh, New › Text Document, Paste, Display settings, and Personalize work, and the layer shows the result; the Menu key and Shift+F10 with nothing selected open the same menu; a right-click on a loose icon still shows its item menu, and on a space the space menu. Run elevated once too: the menu still shows New, Display settings, and Personalize below the DeskSpace entries, with masked icons and no separator dangling under Hide spaces; with `-Verbosity 1` any fallback logs a warning naming the step that failed. |
| Menu at the click | With takeover on and "Auto arrange" off, right-click an empty spot far from other icons: Paste (after copying a file) and New › Text Document each put the new icon at the click, not in the next free slot; a space's item menu commands (Open, Properties, Delete) work as before. If Explorer ignores the point, record it here. |
| New on the desktop | With takeover on, right-click an empty spot of the desktop: New › Folder, and New › Text Document with extensions shown and hidden, each make the item and open DeskSpace's rename box on it, with "New folder" (or the base name) selected; typing a name and Enter names it; Esc keeps "New folder"; a second New › Folder gives "New folder (2)" its own box. Paste and View › Refresh open no box. With takeover off (`--layer` only), Explorer's own box shows, as before. |
| Layer rename | With takeover on, a loose desktop icon renames like a space item: F2, a slow second click, and "Rename" on its right-click menu each open the rename box on the icon (the base name selected when extensions show); Enter renames the file, which stays selected and in place; Esc and a click elsewhere behave as in a space; Ctrl+Z in Explorer undoes it. |
| Layer presses | With takeover on, top-level and with `--placement desktop-*`: Ctrl+click and Shift+click on loose desktop icons add to the selection; a plain click on empty layer space clears it; a click on another app keeps it; a click on the layer during a rename commits the rename. |
| Taskbar repair | With takeover on, use the PC normally with `--debug` open for a while (full-screen video, games, Win+D, switching monitors). If the log ever says "a taskbar was under the desktop layer; sank the layer below it", the taskbar takes clicks again within 5 seconds and the spaces still take clicks and drops; record the logged foreground window in `CONTEXT.md` section 10. "sinking the layer did not help" means the repair failed: replace this row with a task for one layer per monitor. |
| Hosted layer order | With `--placement desktop-*` and takeover off, turn takeover on from Settings: the loose desktop icons appear and every hosted space stays visible above them and takes clicks; the same after a restart with takeover on. |
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
| T17, T18 (D12) | Burn on Startup (hide, show, a new space, startup, exit) and on Delete, on each backdrop at 100 % and 150 %: the space burns from the bottom up along a ragged edge that glows, chars, and scorches ahead of itself, like paper; flames lick up from the edge and past the top and sides of the frame with no box or blur around them, and never show over another application (cover the space with a window and they stay behind it), while with `--on-top` they follow the space above other windows; Detail changes how ragged the edge is; reversing mid-burn (the hotkey twice) un-burns it; no flame window is left behind after it ends, after a delete, or after quitting mid-burn; a desktop-hosted space burns without flames. |
| U3 (D11) | Space menu and tray menu list Undo and Redo at the top, greyed when empty and named for the top action ("Undo Move space", "Redo Move space"); moving, resizing, reordering, sorting, fitting, locking, folding, renaming, creating, and deleting an empty space each undo and redo from both menus. |
| U4 (D11) | Ctrl+Z after a space move puts it back, Ctrl+Y moves it again; with no space current, Ctrl+Z on the desktop still undoes an Explorer rename. |
| U5 (D11) | Drop a file into a space and undo (it is back where it was); delete a space with stored files, undo, then redo; rename an item and undo. |
| U6 (D11) | Recycle a file in Explorer, restore it through a test harness or `--verbose` log, and confirm it is back under its old name. |
| U7 (D11) | Delete two items and undo (both return to the space); paste a copy, undo (it goes to the Recycle Bin), then redo; empty the Recycle Bin between a delete and its undo: the undo says it can't and changes nothing. Shift+Delete leaves Undo as it was. |
| W44 (D11) | Change a space's view (View menu, Ctrl+Shift+1..8, the status bar's buttons), sort a lens, toggle its hidden files, change its folder, and create a lens: each is one Undo entry named for it ("Undo Change view", "Undo Sort lens", "Undo Change lens view", "Undo Change folder", "Undo New lens"), Ctrl+Z and Ctrl+Y restore and reapply it from the menus too, and a lens brought back lists its folder again. |
| W8 (D17) | In light and dark mode, at 100 % and 150 %, on Windows 11 and 10: every scopes row shows its kind's glyph (Global, a space, a lens), the labels line up, a dimmed row's glyph is dimmed, and the selection covers the glyph. |
| W9 (D17) | Lenses on Downloads, Documents, and a plain folder show each folder's own icon left of the title; rename the folder away and the generic folder icon shows; F2's rename box starts after the icon; the same folded, at 100 % and 150 %, on every backdrop; a normal space's title bar is unchanged. |
| W11 (D17) | On a lens in the user's profile: each "Show as" entry retitles it at once, and Cancel puts the title back; Change folder… retitles it under Folder name and keeps a custom title; F2 on the title, then the Lens page shows Custom. |
| W12 (D17) | Each View ▸ entry resizes only that space, on a normal space and a lens. "Settings icon size" gives back the look's size. Ctrl+wheel does not move a space with a fixed size. The view survives a restart. |
| W13 (D17) | After clicking a space, Ctrl+Shift+1, 2, and 3 change it to Extra large, Large, and Medium icons; Ctrl+Shift+4 through 8 change nothing yet; with Explorer in front, Explorer's own Ctrl+Shift+1 still works. |
| W15 (D17) | Turning Sort by ▸ Keep sorted on sorts the space at once. With it on, a dropped file lands in order and a renamed item moves to its place; a drag inside does not rearrange, while a drag out still moves the file. Turned off, the space rearranges freely again and keeps the order it had. On a lens the entry is checked and greyed, and its order holds. |
| W16 (D17) | Small icons on a space and on a lens, at 100 % and 150 %: 16 px icons with one label line beside them, rows wrapping and scrolling down; click, Ctrl and Shift click, the marquee, the arrow keys, and type-to-select all hit the inline cells; F2's box sits over the label; drag out works, and a drag to rearrange reorders a normal space (not one kept sorted). Back in Medium icons, the hand arrangement is as it was. |
| W17 (D17) | List on a space and on a lens, at 100 % and 150 %: 16 px icons with one label line beside them, columns filling top to bottom and running left to right, the wheel scrolling sideways, Home/End/PageUp/PageDown and the arrow keys moving along the columns, and the scroll indicator along the bottom; click, Ctrl and Shift click, the marquee, and type-to-select hit the inline cells; F2's box sits over the label; drag out works, and a drag to rearrange reorders a normal space (not one kept sorted). Check the W16 row again in List. |
| W18 (D17) | A folder in Details beside the same folder in Explorer's Details: the same columns (Name, Date modified, Type, Size), values, alignment, date, and size format; Name takes the width left over; header clicks sort and reverse and the arrow follows; the header stays put while rows scroll; hover and selection fill the row and F2's box sits in Name; 100 % and 150 %; light and dark. |
| W19 (D17) | A new lens has the status bar and a new space does not; View ▸ Status bar toggles it; the counts follow a click, Ctrl+A, and the marquee, and the size shows for files only; both buttons switch the view and show which is on, and their tips read as Explorer's; folded, the bar is gone; every backdrop at 100 % and 150 %, light and dark. |
| W21 (D17) | Tiles beside Explorer's Tiles on the same folder, at 100 % and 150 %, in light and dark: 48 px icons with name, type, and size beside them, tiles wrapping into rows and scrolling down; click, Ctrl and Shift click, the marquee, the arrow keys, type-to-select, F2 over the name, and drag out. |
| W22 (D17) | Content beside Explorer's Content on the same folder, at 100 % and 150 %, in light and dark: one row per item with the icon and name at the left, "Date modified: …" and "Size: …" at the right, and Explorer's separator between rows; click, Ctrl and Shift click, the marquee, the arrow keys, type-to-select, F2 over the name, and drag out. |
| W23 (D17) | Global's Lenses page sits after New spaces. Change each default (View, Status bar, Sort by, Order, Folders first, Show hidden files, Show as) and "New lens…" uses it; an existing lens keeps its own settings. |
| Space icons (D16, S7) | Make the cache by hand first (S3's `trim fluent <upstream icons.json> %TEMP%\deskspace-dev\icons`). With a scratch config, set `icon = "fluent:games"` on one space and `icon = "automatic"` on spaces titled "Games", "Photos", and "Stuff": the first three show their icon left of the title in the title's color, and follow a dark/light switch; "Stuff" and spaces without the key show none and look as before. Renaming a space keeps its icon and the box starts after it; hovering or clicking the icon does what the title bar does. Check at 100 % and 150 %, on Acrylic, Blur, Solid, and Clear, and with a lock. With the cache deleted, nothing shows and nothing fails. |

## Found while working

One line per issue noticed but not fixed: file, symptom, suggested task.

- Code comments cite task ids and dates in about 143 places, such as
  `(W3c)` or `T12` (review, 2026-10-21). Ids are decoded only through
  `TASKS.md` and its archive, and they age as tasks are archived.
  Suggested, for the maintainer to decide: keep the why in the comment
  and the id in the commit message, and stop adding ids to new comments.

- `app/undo.rs` `reverse_files`: the checks run first, but a file step can
  still fail midway (the Recycle Bin emptied of one of two deleted items),
  leaving the earlier steps applied while the entry is dropped as stale; a
  restored file then sits in the store unlisted until a prune. Suggested
  task: roll back the steps already applied when a later one fails.
- `app/menus.rs` `add_files`: U7 asked for `Created` records here, but
  "Add files…" moves (ROADMAP D3, as a drop does), so U5's `Moved` record
  already undoes it and none was added. Suggested fix: drop it from the
  task text if a U task is ever reopened.
- `win32/settings_window/preview.rs`: the Shortcuts page's space preview has
  no keyboard support (not a Tab stop; its X and + are mouse-only).
  Suggested task: arrow-key focus, Delete for the X, and Enter on the +
  tile (ROADMAP M7 accessibility).
- Code comments cite task ids and dates in about 143 places, such as
  `(W3c)` or `T12` (review, 2026-10-21). Ids are decoded only through
  `TASKS.md` and its archive, and they age as tasks are archived.
  Suggested, for the maintainer to decide: keep the why in the comment
  and the id in the commit message, and stop adding ids to new comments.
- `deskspace-platform` tests: the crate's tests take about 16 s
  (2026-10-21), while every other crate's take under 0.1 s. The time
  goes to tests that call the real shell. Run alone,
  `win32::app_catalog::tests::a_background_refresh_fills_the_cache` takes
  about 3 s more than an empty test. Suggested task: find what each slow
  test waits for, and test the decision on plain data instead, keeping
  at most one real-shell check per module.
- **Readability, W12:** the task calls `LargeIcons` "a 96 px cell", but this
  codebase's grid cell is twice the base 48 px icon (a 96 px cell at 100 %),
  so a fixed 96 px icon gets a 192 px cell. The task/word should say "icon",
  or the fixed views need their own cell rule. Noted while implementing W12.
- **API consistency:** `Space::view()` returns `SpaceView` by value while
  `source()` and `look()` return references; harmless because `SpaceView` is
  `Copy`, but surprising beside its siblings. Suggested task: return
  `&SpaceView` or document the value return.
- **app/view.rs:** `base_layout` now takes a `fixed_icon: bool` beside
  `theme`, `grid`, `scale`, and `title_scale`. The call sites are still
  readable, but a small `LayoutScale` (fixed view vs look) would make the
  fixed-vs-look choice harder to misread when W14 adds more view fields.
- **view.rs:** `SpaceView::with_mode` gives the default sort rule, so a View
  menu choice or a Ctrl+Shift digit resets a space's sort rule and its Keep
  sorted choice (W14, W15), though D17 says a space keeps its last rule.
  Suggested: build the new view from the current one and change only the
  mode, if that is what D17 means.
- **W16 Small icons:** the inline cell is 200 × 22 px at 96 DPI, with a 2 px
  gap, because those are plausible Explorer values but have not been measured
  on screen. Confirm them (and the row height at a larger label size, which
  the cell does not follow yet) during W16's check, then fix the constants in
  `view_layout.rs` and record the measurement.
- **W17 List:** `LIST_COLUMN_WIDTH` (200 px) and `LIST_CELL_HEIGHT` (22 px)
  are the same provisional values as Small icons, not measured. Confirm them
  during W17's check and record the measurement.
- **W17 List labels:** each label is ellipsized at the column's right edge, as
  W17 asks; Explorer instead sizes a column to the longest label in the folder
  (up to a limit). Measuring the labels is left for later.
- **Docs, D17 vs W16:** D17 calls Small icons one of the "four icon views"
  and says only List, Details, Tiles, and Content place items by order, while
  W16 (and W12's note) puts Small icons' labels beside the icon in inline
  cells, so it now places items by order. W16 was followed. Suggested: amend
  D17 to say the three large icon views keep the grid and Small icons is an
  order-placed inline view with a fixed 16 px icon.
- **W16 Fit to contents:** on a space in Small icons the command changes
  nothing, because an order-placed view keeps the space's size. Decide
  whether it should fit the content extent (and what width) as the view tasks
  continue.

- `deskspace-config/src/icons.rs` `trim`: Iconify marks deprecated icons
  `"hidden": true` (48 of the 2887 kept Fluent icons in 1.2.60). DeskSpace
  does not read the flag, so they are matched and listed like any icon.
  Suggested: read `hidden` in `IconData` and leave hidden icons out of
  `IconSet::names` (search and matching) while still drawing a saved one.
- `@iconify-json/fluent`'s `icons.json` has no `info` block (it is in
  `info.json`), so the trimmed set's `info` is empty. Credits come from the
  `ICON_SETS` row (S13), so nothing reads it yet.
