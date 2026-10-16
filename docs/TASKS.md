# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

Only open work lives here. Finished tasks are moved, verbatim, to
`archive/TASKS-done.md` (history only; its file and line pointers predate the
R1 and K splits). Built-but-unconfirmed work is listed in section V.

**Pick a task:** section B (code health) and K7 are ready for any
contributor; G5c is ready. L1 needs a maintainer's on-screen findings, and F13
first needs the Ctrl+wheel bug under **Found while working** explained. Skip
tasks marked **[needs decision]** and everything in section D.

## How to work a task

1. Read `AGENTS.md` once (especially "Design priorities"), then only the
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
   (`AGENTS.md` has the commands), with new tests for new logic and the test
   count in `CONTEXT.md` section 2 updated; `CONTEXT.md` updated where
   behavior changed, and the `ROADMAP.md` feature index (section 7) where a
   feature's status changed; and the task's own "Done when" list.
7. Commit in the `AGENTS.md` style with the task id in parentheses
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
| Drag, drag image, drag out | `deskspace-app/src/app/drag.rs` |
| Selection, arrow keys, type-to-select | `deskspace-app/src/app/selection.rs` |
| Inline rename | `deskspace-app/src/app/rename.rs` |
| Drops, delete, prune | `deskspace-app/src/app/files.rs` |
| Cut / copy / paste | `deskspace-app/src/app/clipboard.rs` |
| Space, item, and tray menus | `deskspace-app/src/app/menus.rs` |
| Settings window behavior (open, preview, Apply, Cancel) | `deskspace-app/src/app/settings_session.rs` |
| Settings pages, fields, `field::` ids (pure) | `deskspace-app/src/settings/` (`mod.rs`, `ids.rs`, `tables.rs`, `pages.rs`, `apply.rs`, `session.rs`) |
| Show/hide, desktop verb, start with Windows | `deskspace-app/src/app/visibility.rs` |
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
   right flag (the `AGENTS.md` Gotchas table) that posts `SELECTION_KEY`; map
   the code to the variant in `fn selection_key` (and its test).
3. `deskspace-app/src/app.rs`: handle it in `App::on_selection_key`, calling
   a method in the `app/` module that owns the behavior.
4. Update the key lists in `AGENTS.md` (Gotchas), `CONTEXT.md` section 8, and
   `README.md` Controls (B16 reduces this to one place).

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

- [ ] **B10. Named ranges for tint opacity and title shading.**
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

- [ ] **B11. Group the scene-builder arguments.** `scene.rs`
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

- [ ] **B12. Group the d2d draw-helper arguments.** `render/d2d/item.rs`
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

- [ ] **B15. Timer ids as an enum.** The app's timer ids are eight loose
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

- [ ] **B16. One home for the key list.** The full key list is kept in three
  places (`AGENTS.md` Gotchas, `CONTEXT.md` section 8, `README.md`
  Controls), so the key recipe edits all three.
  - Keep the flag-by-key table in `AGENTS.md` only. `CONTEXT.md` section 8
    links to it instead of restating which flag a key needs. `README.md`
    keeps its user-facing prose (it is for users, not contributors).
  - Update the key recipe's step 4.

- [ ] **B17. Fix stale statements in CONTEXT and ROADMAP.**
  - `CONTEXT.md` section 6, "Mouse input": "Ctrl+wheel changes the global
    icon size" contradicts the known bug (section 2 and **Found while
    working**); say it is meant to, and point at F13.
  - `CONTEXT.md` section 2, "Code health": no non-test file is over 800
    lines is false until K7 lands; reword it to name `color_picker.rs`.
  - `ROADMAP.md` section 4, "Next, in order": item 1 (section L) has only
    L1's findings left and item 4 lists G5a-G5b, which have landed; reduce
    them to L1 and G5c.
  - Done when: the three statements match this file.

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

## F. Ad-hoc features

- [ ] **F13. Ctrl+wheel under "Match Explorer's display" drives the
  desktop's icon size (ROADMAP D9).** Blocked: first explain why Ctrl+wheel
  does nothing under any preset (**Found while working**).
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

`AGENTS.md` asks for files under about 800 lines of non-test code. When one
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

## D. Reserved (strong model or maintainer; do not start)

Each needs design work first. Ids are `R1`-`R15` so they do not collide with
the ROADMAP decision ids (`D1`-`D9`).

- **R3. Platform module layout.** What remains after the K splits:
  - one icon decoder instead of two: `icons::decode_icon_masked` (GDI
    `GetDIBits`, for HICONs) and `d2d::decode_bitmap` (WIC, for HBITMAPs);
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
- **R15. Narrow `App` and `Ui`.** `App` has about 31 fields, all reachable
  from every `app/` module, and `Ui` about 58 public methods. Group the
  remaining session state into owned structs (as `PointerState`, `EditState`,
  and `Selection` already are), and consider splitting `Ui` into focused
  handles (spaces, shell, chrome) so app logic can be tested against a fake.
  Needs a design that keeps `runtime::dispatch`'s re-entrancy rules.

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
| R3 (window procedures) | Nothing should change: hover, click, double-click, wheel, and right-click in a space and on the desktop layer; the tray icon's click and menu; the settings window's controls and dark-mode colors; the color picker's swatches, sliders, hover, OK, Cancel, and Custom toggle. |

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
- `app/pointer.rs` `adjust_icon_size` (`SpaceEvent::Wheel`): the maintainer
  reports (2026-10-12) that Ctrl+wheel over a space does nothing under any
  preset, so even the `Custom` branch never runs and the G2 check was
  premature. Find out why the event does not reach `adjust_icon_size`
  (before F13).
- `app/pointer.rs` `right_drag_to` (G5a): a right drag calls
  `drag_item_out` without setting `pointer.drag_image`, so the shell
  substitutes its own oversized image. Suggested task: factor the image build
  out of `begin_drag_visual` and use it for right drags too.
- `win32/desktop_items.rs`: each `DesktopEntry` is stat'ed twice, once for
  its `kind` during enumeration and again by `item_facts` when `view.rs` /
  `bootstrap.rs` build its `Item` (the desktop layer re-reads them on every
  sync). Suggested task: give `DesktopEntry` an `ItemFacts` and drop its
  `kind` field.
