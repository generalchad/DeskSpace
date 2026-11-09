# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

Only open work lives here. Finished tasks are moved, verbatim, to
`archive/TASKS-done.md` (history only; its file and line pointers predate the
R1 and K splits). Built-but-unconfirmed work is listed in section V.

**Urgent first:** a task marked **[urgent]** goes ahead of everything else in
its section.

**Release first:** until v0.3.0 is tagged, take section Z's tasks and the
fixes it names, and land nothing else but docs and log-only tasks
(decided 2026-11-05; ROADMAP section 4). The list below is for after the
tag, or for log-only work meanwhile.

**Pick a task:** these are ready for any contributor, now that human checks
are collected in section V instead of blocking:
- section P's "Juniors and interns" list, the best place for a first task,
  ranked most severe first, so take one from the top;
- section B's icon picker tasks: B47 and B48 next, then B49; then B51, B53,
  and B55 from "Measure and trim the UI thread";
- section G (settings window polish): the pure half of G13;
- section K (K10-K13, K15, K16, K18), each a pure move with rules that
  leave nothing to design; K18 first, since `undo.rs` is at the limit;
- section T: T24 (needs nothing now), T41a, and then T42; once T41 has
  landed, T25-T31 in order, then T44 and T46-T48;
- O5 and the O6-O9 harness abilities; Q8a-Q8c, comments only;
- anything else not marked **[needs decision]**, "still to design", or in
  section D.

Section P's "Seniors only" list is for experienced contributors.

L1 needs a maintainer's on-screen findings. Skip tasks marked **[needs
decision]**, everything in section D, and section Y's ideas, which are not
tasks yet. Phases 4 and 5 are complete; their
on-screen checks are in section V.

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
   list exactly what to check in the commit body, add a row to section V
   (the human test compendium), and archive the task as in rule 7. The check
   does not block the next task; work continues while it awaits a person.
   Run any `scripts/onscreen/` check that covers part of it first, and cut
   what it covers from the row, as section O's rules say. Anything only a
   human can record (for example L1's findings) is noted in the compendium.

## File map

Where each kind of code lives. Paths are under `crates/`.

| To change... | Look in |
|--------------|---------|
| The model (spaces, items, views, the desktop layer) | `deskspace-core/src/{space,view,item,desktop,desktop_layer}.rs` |
| A lens's list: filter, sort, ids, built on the watch worker | `deskspace-core/src/lens_build.rs` (`LensBuilder`, `LensRules`), `App::sync_watches` in `app/lens.rs` |
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
| Mouse, resize, hover | `deskspace-app/src/app/pointer/` (`mod.rs` gestures, `hover.rs` hover and cursor, `space_events.rs` the event entry) |
| The mouse wheel: zoom, Details sideways, the view's axis | `deskspace-app/src/app/wheel.rs` |
| Ctrl+wheel, zoom, the zoom pop-up's behavior | `deskspace-app/src/app/zoom.rs` |
| Drag, drag image, drag out | `deskspace-app/src/app/drag.rs` |
| Dragging a Details header or its dividers | `deskspace-app/src/app/columns.rs` |
| Selection, arrow keys, type-to-select | `deskspace-app/src/app/selection.rs` |
| Group names and which group an item is in (pure) | `deskspace-app/src/grouping.rs` |
| A grouped space's display order | `deskspace-core/src/grouping.rs` (`Grouping`), read by `ViewLayout`; `App::sync_grouping` in `app/view.rs` keeps it current |
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
| Space window procedure, DWM, shadow window | `deskspace-platform/src/win32/space_window/` (`mod.rs`, `zorder.rs`, `mouse.rs`, `frame.rs`), `dwm.rs`, `shadow_window.rs`, `render/shadow.rs` |
| An animation effect | `deskspace-core/src/animation.rs` (the tables), `deskspace-platform/src/render/transition.rs` (frame math), `render/d2d/` (drawing), `win32/space_animation.rs` (playing it on a window), `win32/overlay.rs` (windows beside the space); the recipe is section T |
| Drawing | `deskspace-platform/src/render/scene.rs`, `render/d2d/` (`mod.rs` the `Painter`, `effects.rs` an animation's resources, `prepared.rs` the `PreparedScene`, `drag_image.rs` the drag image) |
| The zoom pop-up window and its layout | `deskspace-platform/src/win32/zoom_bar.rs`, `render/zoom_bar.rs`, `render/d2d/zoom_bar.rs` |
| Settings window controls | `deskspace-platform/src/win32/settings_window/` (`mod.rs`, `build.rs`, `field.rs`, `update.rs`, `nav.rs`, `paint.rs`, `commands.rs`, `preview.rs`, `cards.rs`, `header.rs` the line above the page, `title.rs` the title's scroll, `scroll.rs` a page taller than the window); the title's pure motion is `deskspace-platform/src/title_scroll.rs`; the card grouping is `deskspace-platform/src/settings_cards.rs` |
| The debug window (`--debug`) | `deskspace-platform/src/win32/debug_window/` (`mod.rs`, `build.rs`, `layout.rs`, `pages.rs`, `console.rs`); the console's pure model in `deskspace-platform/src/debug_console.rs`; the log capture in `deskspace-app/src/debug_log.rs`; the panel and actions in `deskspace-app/src/app/debug/` |
| The color picker popup | `deskspace-platform/src/win32/color_picker/` (`mod.rs` the window, `state.rs` its input, `format.rs` the value box's text, `draw.rs` the settings swatch), `render/color_picker.rs` (layout), `render/d2d/color_picker/` (`mod.rs` the painter, whose ignored `render_preview` test writes PNGs; `pen.rs` its drawing kit) |
| Shell: open, item menu, drag out | `deskspace-platform/src/win32/shell_items.rs` |
| Shortcuts from dropped shell items, custom targets | `deskspace-platform/src/win32/shortcuts.rs` |
| The installed-apps picker and its cache, the custom shortcut dialog | `deskspace-platform/src/win32/{app_picker,app_catalog,shortcut_dialog,dialog}.rs` |
| Desktop enumeration, icons | `deskspace-platform/src/win32/desktop_items.rs`, `win32/icons/` (`mod.rs` the cache, `load.rs` the shell reads, `worker.rs` the load worker), `icon_queue.rs` (what the worker loads next) |
| Icon-set glyphs (SVG to pixels) | `deskspace-platform/src/render/glyph.rs` (the document), `render/d2d/glyph.rs` (the rasterizer), `win32/glyphs.rs` (the cache) |
| Takeover: hide icons, watchdog | `deskspace-platform/src/win32/{desktop_icons,watchdog}.rs` |
| CI and release | `.github/workflows/{ci,release,icons}.yml`, `crates/deskspace-app/build.rs` |
| The file-size limit and its exceptions | `deskspace-app/tests/file_size.rs` (`ALLOWED`) |

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
   list) and a user-facing line to `README.md` Controls. Add an exact step,
   range, or edge case to `CONTEXT.md` section 8 only if it has one.

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
3. Update the user-facing menu list in `README.md` Controls; add a rule,
   range, or edge case to `CONTEXT.md` section 8 only if the entry has one.

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

## Z. Release v0.3.0 (ROADMAP section 4, decided 2026-11-05)

The maintainer chose to release before starting more work. ROADMAP section
4, item 3, holds the gate; this section is the plan. **Until v0.3.0 is
tagged, take these first**, and land only them, the gate's fixes, docs, and
log-only tasks; every other task waits. When the tag is out, archive this
section and lift the freeze in the same commit (Z8).

v0.3.0-alpha.1 went out first, as a pre-release (maintainer, 2026-11-05),
with the gate still open. Its `CHANGELOG.md` section already has the upgrade
note, the known issues, and the entries Z2 found missing, so Z2 and Z6 now
check that section against what lands after it. It is also R12's first
install (Z7). The 0.3.0 section gathers the alpha sections into one.

The gate's fixes are in section P, marked **[urgent] (v0.3.0)**: P14b and
P14c, P35, P36, P37, P46, P49, and P50. B45 has landed; B47 and B48 (section B) join if
they are ready in time. P32 does not; Z6 lists it as a known issue.

Who takes what (split 2026-11-07, so juniors and interns can carry most of
the gate):

- **Juniors:** Z2, Z3, Z6, P14c, and the junior halves of P36 and P46
  (P36a, P46a).
- **Interns with a screen:** P14b, the Z5 batches, Z7a, and Z7c.
- **A senior:** P36b and P46b, which land the junior halves with them.
- **The maintainer:** what Z5's failures become, Z7b, and Z8.

- [ ] **Z5. Pass the marked rows (maintainer).** **[needs human]** After: Z4
  and the gate's fixes. R7, limited to the `(v0.3)` rows, scripted ones
  first (section O). Rule 8 and section V's rules hold: a passing row is
  deleted, a failing one becomes a fix task here that joins the gate, unless
  the maintainer ships it as a known issue (then Z6 lists it).
  - Split into batches, one intern and one sitting each, grouped by the
    setup they share. A batch can start once the fixes its rows depend on
    have landed; the rest of the gate need not wait. Rows for P36 and P46
    join batches Z5g and Z5d when those fixes land; P49 and P50 join Z5d.
  - Until P49 (found 2026-11-08), a space drawn through composition could
    show an old frame, another space's, or nothing after a click. Any row a
    batch passed on a build without it, where the picture matters, is run
    again.
  - Rules for every batch:
    - Run `./scripts/dev.ps1` for a scratch config, never the real one.
      Never pass `-Fresh` without asking the maintainer first: it deletes the
      scratch config and its stored files (`AGENTS.md`, Directive 1).
    - Use throwaway folders and files only. Rows that recycle or delete
      files go on a VM or a test account.
    - Do the `(script)` and `(part script)` parts by hand until the O tasks
      script them.
    - Report each row as pass, fail, or not covered (this machine lacks the
      setup), with the Windows version, the display scale, and what you saw.
      Whoever commits the results follows section V's rules; a failure goes
      to the maintainer.
  - [ ] **Z5a (intern). Lens files.** A hand-made lens on a throwaway
    folder, as section V's "First session" says: W3c, W3d, W3e, W3f, W3g,
    W7, W25, W45, W46, P20.
  - [ ] **Z5b (intern). Views and sorting.** W8, W9, W11, W12, W13, W15,
    W16, W17, W18, W19, W21, W22, W23, W28, W33, W37 and W38, W39, W47, W48.
    W18, W21, W22, and W33 need Explorer open beside the space.
  - [ ] **Z5c (intern). Details and browsing.** W27, W30, W31, W32, W35,
    W42, W43, W44.
  - [ ] **Z5d (intern). Drops, drags, and the clipboard.** F1-F5, G5a, G5b,
    G5c, Right-drop greying, L2, L5, L6, Drag and drop, Clipboard and
    rename, P25, P37.
  - [ ] **Z5e (intern). Undo.** U3, U4, U5, U6, U7, P10. After Z5d, whose
    moves these undo.
  - [ ] **Z5f (intern). The taken-over desktop.** Takeover on (Settings >
    General) in the scratch config: I8, Desktop menu, Menu at the click, New
    on the desktop, Layer rename, Desktop drop, Spaces above the layer. Turn
    takeover off afterwards.
  - [ ] **Z5g (intern). The debug window and startup.** `--debug`: X7, X8,
    X9, Delete stress spaces; then P35, which holds the scratch config open.
    X7 recycles files, so a VM or a test account.
  - The install and update rows (E5-E7 and E10-E12, F-45 automatic install,
    Update install failure) need an installed build, so they go with Z7a and
    Z7c.

- [ ] **Z7. The dry run on clean machines (maintainer).** After: Z1-Z6 and
  the gate's fixes. R12 (section D) is the checklist, with
  `0.3.0-alpha.1` (the version in `Cargo.toml` today) and then
  `0.3.0-alpha.2`. It is the first release with an update feed, so the
  alpha.1 to alpha.2 update is the check that matters most. Record the
  findings as R12 says; a failure is a fix task here.
  - **[needs decision]** Found 2026-11-05, after alpha.1 shipped: the
    updater reads `releases/latest/download/latest.json` (`UPDATE_ENDPOINT`
    constant in `deskspace-update/src/lib.rs`), and GitHub's "latest" never
    names a pre-release, so no alpha can update to another alpha. Options:
    (a) publish alpha.2 as a full release for the test, then mark it a
    pre-release again (simple, but every v0.3 install sees it meanwhile);
    (b) test the feed with the final v0.3.0 itself, and keep a v0.3.0
    install updating to a v0.3.1 as the first real check; (c) a pre-release
    channel the updater can opt into (a setting and a second feed: new work,
    after v0.3.0). Recommended: (b) now, (c) as an idea in section Y.
  - [ ] **Z7a (intern; clean VMs). Install, run, uninstall.** On clean
    Windows 10 and 11 VMs, with the latest pre-release the maintainer has
    published, every R12 check except the update itself: the zip's
    contents, the extracted exe, the installer, the uninstall, and what
    SmartScreen shows. Add the rows E5-E7, E12, and Update install failure
    as far as they go without a newer release. Record the findings as R12
    says. A first pass on `0.3.0-alpha.1` may start now, to find install
    problems early; repeat it once the gate's fixes are in a pre-release.
    alpha.1 has P49's bug, so judge nothing visual on it.
  - [ ] **Z7b (maintainer).** The **[needs decision]** above.
  - [ ] **Z7c (intern; clean VM). The update.** After Z7b, the check it
    chose. With (b): install v0.3.0 and keep the VM for v0.3.1. Also E10,
    E11, and F-45 automatic install.

- [ ] **Z8. Cut v0.3.0 (maintainer).** After: Z7. Follow `RELEASING.md`'s
  checklist, version `0.3.0`. Then, in one `docs:` commit: ROADMAP section 4
  lists v0.3.0 as released and drops item 3 from "Next"; section 7's feature
  statuses say what shipped; this section is archived, ticked, to
  `archive/TASKS-done.md`, and the freeze sentence in "Pick a task" goes.

## B. Code health (any contributor)

A B task is a behavior-preserving refactor: the test count only grows, and no
user-visible change is expected. The exceptions are performance work: B32-B36,
B39-B43 (the lens-performance costs left by B33/B37), and B44-B55 below.
Finished ones are in `archive/TASKS-done.md`.

Not listed here on purpose: `settings/pages.rs` `look_pages` (278 lines),
`settings/apply.rs` `apply_field` (197) and `apply_look_field` (136), and
`settings/mod.rs` `build_form` (130), measured 2026-11-04. They are long
because a field is wired in three places, and R14 (table-driven settings
fields) removes that cause; splitting them first would be undone by R14. They
grow with every new option (`look_pages` was about 205 lines in October), so
R14a's decision is what stops the growth.

### The icon picker and other long lists (B44-B50)

The icon picker froze for seconds on opening and on every search keystroke
(found 2026-11-05). Measured in a release build, a glyph cost about 55 ms to
draw, because each one made a new Direct3D device. B44 (archived) keeps one
device context, and a glyph now costs about 0.33 ms. A picker page of about
100 cells is still about 35 ms, over the UI thread's budget
(`ENGINEERING.md` section 3, "Performance"). These tasks take the rest, in
order.

Rules for every task here:

1. **Measure before and after**, as `ENGINEERING.md` section 3,
   "Performance" says. To time pure code, write a throwaway `#[ignore]`
   test, run it with `-- --ignored --nocapture`, and delete it before you
   commit. The real
   Fluent set is `%TEMP%\deskspace-dev\icons\fluent-r1.json` once a
   `dev.ps1` run has downloaded it; parse it with
   `serde_json::from_slice::<IconSet>`.
2. **Same results.** A faster path returns what the slow one did, in the same
   order; the existing tests are the spec, and none is deleted or loosened.
3. **The picker's Win32 code is `unsafe`.** B47 and B48 change
   `win32/icon_picker.rs` but no `unsafe` block's reasoning. Keep every
   `with_state` borrow as short as it is now (no Win32 call inside one), and
   keep each `// SAFETY:` comment true. A senior reviews B47 and B48.

- [ ] **B47. Keep the search index while the picker is open (platform).**
  After: B46. Senior review (rule 3).
  - Why: after B46, the picker still builds the index on every keystroke.
  - `PickerSpec` (`win32/icon_picker.rs`) gets `index: Option<SearchIndex<'a>>`,
    built in `PickerSpec::of` when the set is loaded. `PickerSpec::cells`
    queries it. Switching sets in the dropdown then costs nothing to build.
  - Tests: `PickerSpec::cells` with a small set from
    `deskspace_core::icon_set` (build one by hand, as the `icon_match` tests
    do) gives the same names `SearchIndex::search` gives.
  - **[needs human]** check: type "arrow", one letter at a time, then delete
    it: the grid follows each key; switch sets if there are two.

- [ ] **B48. Draw each icon once while the picker is open (platform).**
  After: B45. Senior review (rule 3).
  - Why: a `Cell` holds its own image index (`Cell.image`), and a search
    makes new cells, so every search draws its icons again and adds each one
    to the image list again. The list only grows while the window is open.
  - Replace `Cell.image` with `images: HashMap<String, i32>` on
    `PickerState`, keyed by `<prefix>:<name>` (the key `on_display_info`
    already makes), so an icon drawn once is never drawn or added again,
    in any search or set. Remove `image` from `Cell` and its tests.
  - `on_display_info` looks the key up first and draws only on a miss,
    keeping its three short borrows. A failed draw stores `-1`, as now, so it
    is not retried on every repaint.
  - Tests: a pure helper that takes the map and a key and says whether to
    draw, if you find one that reads well; otherwise say in the commit body
    that the change is covered by the human check.
  - Measure with B45's "drew icons" line: search "a", clear it, and search
    "a" again, then close. Before this task the count grows with each
    search; after it, the second "a" adds nothing.
  - **[needs human]** check: search "arrow", clear it, and search "arrow"
    again: the second time the icons are there at once. Every cell shows its
    own icon, never another's, after switching sets and searching.

- [ ] **B49. Measure the pickers on screen and decide (no code).**
  **[needs human]** After: B45-B48.
  - With `./scripts/dev.ps1 -Release -Verbosity 1`: open the icon picker,
    scroll to the end, search "a", "ar", "arrow", and clear. Then open New
    shortcut ▸ installed apps, search "mi" and clear. Record the times from
    each of B45's lines here, with the date and the machine (CPU, display
    scale).
  - If the icon picker's first page of draws, or any search, is over the
    budget, B50 is ready; if neither is, delete B50 with the numbers as the
    reason. Separately, if the app picker's fill is over the budget, add a
    seniors-only task for a virtual (`LVS_OWNERDATA`) app list, which must
    keep its check boxes working.

- [ ] **B50. Draw the picker's icons off the UI thread (platform; thread).**
  **Seniors only.** After: B49, and only if it says so.
  - Design, then bring it to the maintainer. The outline: cells show no image
    until theirs is drawn; `LVN_ODCACHEHINT` names the rows coming into view;
    a worker thread, which gets its own `Rasterizer` because B44's is
    per thread, draws them and posts each finished `Icon` to the picker
    window; the window adds it to the image list and redraws that row. A
    search or a set switch bumps a generation, and the worker drops work for
    an older one, as `icon_queue.rs` drops paths scrolled out of view. The
    window may close with work in flight: the worker must never post to a
    destroyed window's state (`ENGINEERING.md` section 6, "Window state",
    and "Never wait on something only you can complete").
  - Tests for the pure parts: the request queue and the generation rule.
  - **[needs human]** check: B48's, and a fast scroll from the top to the end
    of the grid never stalls.

### Measure and trim the UI thread (B51-B55)

Planned 2026-11-05 from a read of the code, not from a profile, so the first
tasks measure and the later ones wait for the numbers. The rules of B44-B50
hold here too; the budget and the measurement rules are `ENGINEERING.md`
section 3, "Performance".

- [ ] **B51. Log how long a space's scene takes (app, platform).** Any
  contributor; it adds log lines only.
  - Why: every scene a space gets is built whole, and nothing says what that
    costs. B52 waits for this number.
  - Copy `settings: switched page` (`win32/settings_window/nav.rs`). Add:
    - `scene: built`, timed across `App::refresh_space` (`app/view.rs`), with
      the space id, the scene's item count, and the view mode;
    - `scene: prepared`, timed across `PreparedScene`'s constructor
      (`render/d2d/prepared.rs`);
    - `scene: painted`, timed across a space window's paint, with its size.
  - These run on every hover change, so log them at `trace`. Log the same line
    at `debug` as well when it takes over 4 ms, so a `-v` log shows only the
    slow ones.
  - Done when: the gates pass and a `./scripts/dev.ps1 -Release -Verbosity 2`
    run shows each line. Say in the commit body that you saw them, or that a
    person must.

- [ ] **B52. Hover and the caret without a new scene (app, platform).**
  **Seniors only.** After: B51.
  - Why: a hover change (`App::update_hover`, `app/pointer/hover.rs`) and each
    caret blink of a rename (530 ms, `app/rename.rs`) call `refresh_space`.
    That rebuilds the whole `SpaceScene`, formats every visible Details, Tiles,
    or Content row's texts again (`view_cells`), and encodes every string to
    UTF-16 again, to change one flag.
  - First, with B51's lines: hover across a 50,000-file lens in Details at
    150 %, with every column on. If `built` plus `prepared` stays under 0.5 ms,
    delete this task with the numbers as the reason.
  - Otherwise, design and bring it to the maintainer. The outline:
    `Ui::set_space_hover(id, Option<u64>)` and `Ui::set_space_caret(id, bool)`
    patch the window's stored `PreparedScene` and invalidate only the item's or
    the title's rect. Every other change still pushes a whole scene. The
    scene's own fields stay the truth for the next full build, so a test checks
    that a patched `PreparedScene` equals one prepared from the rebuilt scene.
  - While an animation plays, a patch changes only the stored scene; the frame
    it draws keeps its snapshot.

- [ ] **B53. Shade the flames with less work per pixel (platform).** Any
  contributor; pure code.
  - Why: `flames::paint` (`render/flames.rs`) runs on the UI thread for every
    frame of a Burn. `paint_tongue` works out `below.powf(TIP_FALLOFF)` and
    `brightness` for every pixel, though both depend only on the row and the
    tongue, and `paint` clears the whole overlay (`pixels.fill(0)`), about
    4.7 MB for a 1200 × 900 space, though the tongues cover a small part.
  - Hoist the row and tongue constants out of the pixel loop. Have `paint`
    clear only the rows the previous frame drew: it takes a `&mut Rows` (a new
    pure type in `flames.rs`, the range of rows last drawn) that the overlay
    keeps, and returns the rows it drew this time.
  - Tests: copy today's `paint` into the test module as `reference_paint`. Over
    several times, sets of tongues, and a frame of other tongues before each,
    the new `paint` gives the same pixels. Keep every existing test.
  - Measure a 1200 × 900 overlay over 60 frames with a throwaway `#[ignore]`
    test, before and after.
  - Collides with T29, which rewrites `paint_tongue`: take B53 first.

- [ ] **B54. Upload only the rows the flames changed (platform; Win32).**
  After: B53 and T41b. Senior review: `unsafe` Win32.
  - Why: `UpdateLayeredWindow` copies the whole overlay every frame.
  - Call `UpdateLayeredWindowIndirect` with `prcDirty` set to the union of this
    frame's and the last frame's rows from B53, linked beside the call
    (https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindowindirect).
    The first frame, and the first after the overlay moves (`follow`), still
    send the whole window.
  - Measure the call with a `trace` line, before and after.
  - **[needs human]** check: Burn looks as before on Startup and Delete, at
    100 % and 150 %, and no flame pixel is left behind where a tongue was.

- [ ] **B55. Log the startup timeline (app, platform).** Any contributor; it
  adds log lines only.
  - Why: "fast startup" is a goal (`CONTEXT.md` section 1) that no number
    backs.
  - One `debug` line per phase, `startup: <phase>`, with the milliseconds on
    the clock the log console already keeps (`START` in `debug_log.rs`; start
    it at the top of `main` if it is not started there already): the config
    loaded, the runtime made, the desktop's items listed, the first scene
    pushed, the first space painted (a one-shot flag in the platform), and the
    icon worker first idle. Name each phase after the function that ends it.
  - Done when: a `./scripts/dev.ps1 -Release -Verbosity 1` run shows each line
    once, and the commit body records one run's numbers with the number of
    spaces and items, or says a person must.

### Readability and tooling (B56-B57)

- [ ] **B57. A coverage report for the pure crates (tooling).** **(junior)**
  Installing a tool is an install on the user's machine, so ask first, as
  `TOOLS.md` section 4 does for WinDbg.
  - Why: the pure crates carry most of the logic, and nothing says which parts
    no test reaches.
  - `scripts/coverage.ps1` runs `cargo llvm-cov` over core, config, update, and
    the app, and prints each crate's line coverage and the ten least covered
    files. It needs `cargo install cargo-llvm-cov --locked` and the
    `llvm-tools-preview` component: add the component to `rust-toolchain.toml`
    in the same commit (a `ci:` change, `ENGINEERING.md` section 1), and the
    tool to `TOOLS.md` with what it is for.
  - No CI gate and no threshold: the report informs tasks, it does not block.
  - Done when: a run prints the report; the commit body records it, and each
    pure file under 50 % becomes a **Found while working** line.

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

- [ ] **C6. More item presets (F-48).** Later. Scope decided by the
  maintainer, 2026-10-29: sharing is C10's theme files, so C6 has no file
  format of its own.
  - Add built-in named presets, Compact and Spacious, after "Match
    Explorer's display" and "Custom". A preset sets only the item fields,
    not a whole `Theme` or `SpaceLook`.
  - Still to design before it is ready: how a preset interacts with
    per-space looks. Keep `Preset` forward-compatible (`#[serde(default)]`).
    It grows with the remaining Explorer item settings (hidden/protected
    files once F-22 exists).

## Y. Ideas waiting for a decision

Brainstormed 2026-11-05: features and small delights that no ROADMAP entry
covers yet. None is ready to build. Each says what it is, where it would
plug in, how big it is, and a recommendation. Once the maintainer approves
one, it gets an `F-xx` row (and a `Dx` decision if it needs one) in
`ROADMAP.md`, and its tasks are written in the section it belongs to; then
it leaves this list. A rejected idea is deleted with the reason in the
commit.

Features:

- **Y1. Find anything.** **[needs decision]** A hotkey opens a search box
  over every space and lens; typing narrows a list of items by name, and
  Enter opens the chosen one, or Shift+Enter scrolls its space to it and
  selects it. Fit: the icon picker's `SearchIndex` (B46) is the matcher, a
  `HOTKEY_BINDINGS` row is the hotkey, and the box is a never-activated
  pop-up like the zoom pop-up's. Large. Recommended for M7 (keyboard): it is
  the biggest step toward "a full workflow without a mouse".
- **Y2. Smart lenses.** **[needs decision]** A lens that is a query: "the 20
  newest files in Downloads", "changed this week". Fit: two
  `#[serde(default)]` fields on `FolderView`, a limit and a modified-within,
  applied by `LensBuilder` after its filter and sort. Small. Recommended
  first: it reuses the most and asks the least design.
- **Y3. What's new in a lens.** **[needs decision]** Items that arrived since
  you last looked carry a small dot that fades once seen, and a folded lens's
  title says "3 new". Fit: the listing already reconciles by path (B37), so
  "new" is a path not in the last seen set, kept per lens in session state. A
  scene field draws the dot. Medium.
- **Y4. Shell-folder lenses.** **[needs decision]** Lenses on the Recycle
  Bin, This PC (drives with a free-space bar), and Quick access. Fit: a
  `SpaceSource` variant whose listing comes from `IShellFolder` on the
  worker; D3's file rules need a line for each (Delete in the Recycle Bin
  deletes for good). Large; seniors only.
- **Y5. Step aside while presenting.** **[needs decision]** When a
  full-screen app runs, or Windows reports presentation mode
  (`SHQueryUserNotificationState`), the spaces hide with Startup's Out and
  return with its In afterwards. Fit: `app/visibility.rs` and one
  `WindowSettings` switch. Small. Recommended: it removes a reason to quit
  DeskSpace before a meeting.
- **Y6. Light and dark themes.** **[needs decision]** A scope can name two
  theme files, one for light and one for dark, and switches when Windows
  does. Fit: C10's theme files and `Event::SystemThemeChanged`, which
  exists. Small.
- **Y7. Tint from the wallpaper.** **[needs decision]** A tint choice "From
  the wallpaper": the average colour of the wallpaper under the space,
  softened, so spaces sit in any desktop. Fit: T22's spike already reads the
  wallpaper and its placement (`IDesktopWallpaper`); a colour source like
  `border_source`. Medium; take it after T22 decides.
- **Y8. Colour labels.** **[needs decision]** Tag an item with a colour, drawn
  as a dot beside its label; a lens keeps its labels by path, capped. Fit: an
  `Item` field, a `Command`, a scene field, and an item-menu entry. Medium.
- **Y9. An undo history.** **[needs decision]** "Undo ▸" lists the last
  entries by label, and choosing one undoes back to it, one entry at a time,
  stopping at a stale one as undo does. Fit: `UndoStack` already labels every
  entry; one menu entry by the recipe. Small; a junior task once approved.
- **Y10. Portable mode.** **[needs decision]** A `deskspace.portable` file
  beside the exe keeps the config and the store beside it, and writes no Run
  value, desktop verb, or registry entry. Fit: `ConfigStore`'s paths and
  `bootstrap.rs`; opt-in, so no user's install changes. Small to medium.
- **Y11. Spaces per virtual desktop.** **[needs decision]** A space shows on
  every virtual desktop, as now, or on one. Fit: `IVirtualDesktopManager`
  (documented: `IsWindowOnCurrentVirtualDesktop`, `MoveWindowToDesktop`).
  Whether it works on DeskSpace's never-activated tool windows needs a spike
  first. Medium; it touches F-12 (pages), so decide them together.

Quirks:

- **Y12. Items glide.** **[needs decision]** After a sort, Arrange, or a
  lens change, items glide from their old cells to their new ones instead of
  jumping; a deleted item crumples away. Fit: a transition between two
  `PreparedScene`s keyed by item id, played by the platform from pure frame
  math as space effects are, so the app still pushes one scene. Large;
  seniors only, after section T's groundwork.
- **Y13. Shake to tidy.** **[needs decision]** Shake a space by its title
  while dragging it, and it arranges itself (the Arrange the menu runs),
  with one undo entry. Fit: a pure gesture check over the drag's recent
  points in `app/pointer/`, then the existing `arrange_space`. Small.
- **Y14. Peek into a folded space.** **[needs decision]** Holding the pointer
  over a folded space's title shows its items in a pop-up below it, without
  unfolding it or moving its neighbours. Fit: a pop-up window drawing the
  space's own scene. Medium; it overlaps F-04's unfold on hover, so decide
  them together.

## N. Packs and automation (extensions)

How people extend DeskSpace without forking it (review, 2026-11-03). There are
three tiers, safest first:

1. **Packs:** files of data the app loads, checks, and clamps. Themes (C10)
   are the first; icon sets (section S) already work this way. Colour schemes
   (N4) and animation effects (T40) are next.
2. **Verbs:** commands a script hands to the running instance (R16, N5), for
   scripts, hotkey tools, and Explorer's "Send to".
3. **Code:** native code DeskSpace has to trust. In-process plugins are a
   non-goal (R16): a crashing plugin with takeover on breaks the user's desktop.
   New behaviour of this kind is built in, through the internal tables
   (`EFFECTS`, `VIEW_MODES`, and the `Paginate` J2 proposes).

The remaining tasks are marked **[needs decision]**; each carries a
recommendation, and its parts wait for it.

Rules for every N task:

1. **A pack is data.** It never holds code, scripts, shaders, a path DeskSpace
   runs, or a URL DeskSpace fetches. Images in packs wait for a decision.
2. **A pack file is untrusted.** Check its size before parsing, clamp every
   value, cap every string and list, ignore unknown keys, and load a newer
   `format` with a warning. An enum word this build does not know falls back
   to that field's default (P34). A value of the wrong type still fails the
   file, as N1's `wrong-type` reference pins (maintainer, 2026-11-04). Each
   kind gets a hostile-file test.
3. **Packs reach the network only through `deskspace_update::fetch_signed`**,
   as icon sets do (section S, rule 2).
4. **A shared file must keep loading.** Once a format is shared, a renamed
   field breaks other people's files. N1's reference files must load forever,
   so never edit or delete one; add new ones when the format changes.
5. **A verb calls an existing `App` method.** It is never a second way to change
   the model.

- [ ] **N3. One loader for every pack kind (config).** **[needs decision]**
  After N2. This should land before any second kind (N4, T40), so the second
  kind does not copy the first.
  - Recommended:
    - `deskspace-config/src/pack.rs` holds `PackHeader { format, name, author }`,
      which each kind's file includes with `#[serde(flatten)]`.
    - `read_capped(path, max)` is N2's size check, moved there.
    - `list(dir, extension)` returns each pack's path and header, sorted by
      name. It skips a file that fails to parse, with a `warn`.
    - `PackKind` is an enum with one row per kind (folder name and file
      extension). It starts with `Theme` alone; each later kind adds its own
      variant.
    - `ConfigStore::pack_dir(kind)` replaces `themes_dir`.
  - The alternative is a loader per kind: simpler now, copied later.
  - Tests: N1's reference files still load (flattening must not change the
    format), a listing sorts and skips a broken file, and the cap holds.
  - If C10c has landed, its "Themes" choice reads `list`.

- [ ] **N4. Colour schemes.** **[needs decision]** A colour scheme changes
  colours and nothing else, so it can go on top of any theme. The review
  recommended an overlay type on 2026-11-03; option (c) below is that overlay,
  and (a) replaces it as the recommendation, because (c) copies every `Theme`
  field.
  - Options:
    - **(a) Recommended:** a `ColorScheme` type in core holding only the colour
      fields. The exact list is part of the decision; proposed: `tint`, `text`,
      `marquee_color`, `border_color`, `border_source`, and `use_accent`.
      `Theme::colors()` and `Theme::with_colors(&ColorScheme)` read and write
      them, and applying one turns `custom_colors` on.
    - **(b)** A theme file whose `[look]` holds only some keys, merged over the
      current look as TOML tables before parsing. It needs no new type, and any
      subset works. But no type says what a colour scheme is, and a misspelt key
      silently applies nothing.
    - **(c)** An overlay struct with every `Theme` field optional. It copies all
      of `Theme`'s fields, a second list to keep in step, so it is not
      recommended.
  - The parts, if (a) is chosen:
    - [ ] **N4a (junior, core).** `ColorScheme` with `#[serde(default)]`,
      `Theme::colors` and `with_colors`. Tests: a round trip through `Theme`, and
      applying a scheme leaves every other field unchanged.
    - [ ] **N4b (junior, config).** After N3. A `.deskspace-colors.toml` kind
      through N3's loader, and reference files in N1's folder.
    - [ ] **N4c (app).** After C10c. "Save colours…" and "Load colours…" on the
      Colors page, built as C10c builds its buttons. Loading is a previewed edit
      that Cancel undoes. **[needs human]** check: save the colours of one space,
      load them on another with a different theme; only the colours change.
    - [ ] **N4d (junior, docs).** README: what a colour scheme is, and where the
      files go.

- [ ] **N5. Verbs: scripting the running instance.** **[needs decision]**
  R16 (section D) holds the questions. This is a proposal that settles them,
  for the maintainer to approve:
  - **Transport:** `WM_COPYDATA` to the controller window, found by class as
    `signal_show_spaces` (`win32/desktop_entry.rs`) finds it today. Use a fixed
    `dwData` tag and one UTF-8 verb line, capped at 4 KiB, sent with
    `SendMessageTimeoutW`, so a hung instance cannot hang the script.
  - **Trust:** UIPI already blocks a sender with lower integrity. The
    controller also refuses a message whose sender window is missing, or whose
    process is in another session.
  - **One type:** a `Verb` enum in `deskspace-platform/src/input.rs`, beside
    `Event`, and an `Event::Verb(Verb)`. `cli.rs` parses `--verb <name> [args]`
    into it. A `VERBS` table (name, arguments, help) drives parsing and the
    error that lists the known verbs.
  - **Never break users:** `--show` and `--new-space` stay flags and become
    verbs inside. The `SHOW_SPACES` and `NEW_SPACE` messages stay decoded,
    since an older install's desktop verb may still send them.
  - The parts, after approval:
    - [ ] **N5a (junior, platform, pure).** `Verb`, `VERBS`,
      `Verb::parse(&[String])`, and `Verb::to_args`, starting with `Show` and
      `NewSpace`. Tests: round trips, an unknown verb, missing and extra
      arguments.
    - [ ] **N5b (senior, platform).** After N5a. `signal_verb(&Verb) -> bool`
      over `WM_COPYDATA`, and decoding in `controller.rs`, with the payload
      check in a pure `decode_verb(&[u8])`. Tests for empty, oversized,
      non-UTF-8, and unknown payloads.
    - [ ] **N5c (junior, app).** After N5b. `--show` and `--new-space` go
      through verbs, and `bootstrap.rs` hands off any verb the same way. P23's
      row in section V must still pass.
    - [ ] **N5d (junior, app).** After N5c. `hide` and `settings`, one commit
      each. `apply-theme <path>` follows after C10c. A verb is a row in `VERBS`
      plus an arm in the app calling the method the menu calls.
    - [ ] **N5e (junior, docs).** README: a "Scripting" section with examples.
  - **[needs human]** check, with N5c: a desktop verb, a shortcut with `--show`,
    and a PowerShell script all reach a running instance. With none running,
    `--show` and `--new-space` still start DeskSpace, as today.

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
- `SyncLens { id, items }` swaps in the list the watch worker built from
  the folder. Nothing adds items to a lens one by one: `AddItem` and
  `MoveItem` into a lens return `Rejected`.

Phases 1-5 are done: drops, pastes, shortcuts, deletes, and menus treat a
lens as D3 says (W1-W7), and the views, sort rule, status bar, and lens
settings are in (W9, W18-W23, ROADMAP D17). Phase 5 was W24-W49 (D18);
W41 closed F-40, and the rest of the on-screen checks are in section V.
New here? Read "Where to start" below before anything else in this section.

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

W1-W49 are done (2026-10-21 to 2026-10-31) and archived; their on-screen
checks are in section V. No task in this section is open. A new lens task
starts from "How to work a task" above and the file map, and reads ROADMAP
D3, D17, and D18 first.

### Phase 5: lens and view extras

The phase 4 rules (first callers, the tables, Explorer as the reference,
the glyph fonts) are in the archive, under "Section W's phase notes", and
hold here too.

Read ROADMAP D17 and D18 first; they are the spec.

Runtime orders:

- **A listing for a folder the lens no longer shows.** The watch worker
  posts a listing as a message, so it can arrive after Change folder…
  or (after W42) a step into a subfolder. A listing carries the folder
  it read (W24), and the app drops a stale one, so a task that changes
  which folder a lens shows keeps `on_lens_listing`'s check true.
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
with no hitch. The general rules, including the UI thread's budget, are
`ENGINEERING.md` section 3, "Performance"; these are what lens code adds.

- **Layout is arithmetic.** An order-placed view finds the rect for index
  `i`, and the indices visible in a scroll window, by arithmetic, not by
  walking every item. Groups (W37) keep prefix sums of group sizes and
  binary-search them.
- **Text is made once.** Type names (cached by extension), dates, sizes,
  and group names are formatted when a scene is built, and only for the
  items the scene carries (the visible ones). `PreparedScene` already
  encodes strings to UTF-16 once.
- **No IO the listing did not already do.** Dates, sizes, and attributes
  come from the `FindFirstFileExW` record the worker reads. Hover tips,
  titles, and status text read the model. Nothing calls the disk on the
  UI thread per item or per event; the folder watch checks the store on
  its worker (W25).
- **Measure what could be slow.** A task that touches `LensBuilder`, `SyncLens`,
  sorting, filtering, or grouping times it on 50,000 entries and records
  the dated result in its commit body. Over the budget on the UI thread
  means stop and ask before merging.

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

## S. Space icons (ROADMAP D16, F-62)

Every S task is built and archived; their checks are in section V. The rules
below stay for any task added here.

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

## G. Settings window polish (ROADMAP D26, F-68)

Read D26 first; it is the spec. There are three strands, and each can run
alongside the others:

- the highlight fades: done (G1-G3);
- cards: done (G4-G6);
- long names: done (G7, G8); G9 remains.

G13, found while building G10's page scroll, stands alone.

G1, G4, and G7 were pure code that only their partner task called, so each
landed in its partner's commit. A junior could write and test the pure half
first, and a second person could take the Win32 half on the same branch.

The header's icon and inline rename landed as S11 and S12 (archived), in
`settings_window/header.rs`; G8's scrolling title is in
`settings_window/title.rs`.

G9 crosses the junior line in section P, so it says why and names what a
senior reviews. Unless a task says otherwise, a name in `code` that does not
exist yet is one the task creates.

- [ ] **G9. Tooltips for long names in the scopes pane (platform).**
  **[needs human]** After G8. D26: scope rows do not scroll; a row that ends
  in an ellipsis (`on_draw_scope` already draws `DT_END_ELLIPSIS`) shows
  its full label in a tooltip.
  - Add a second tool to G8's tooltip, on the scopes list box
    (`TTF_IDISHWND | TTF_SUBCLASS`), with `LPSTR_TEXTCALLBACKW` text. On
    `TTN_GETDISPINFOW`, find the row under the cursor (`LB_ITEMFROMPOINT`).
    Give its label only if the label is wider than the row's text area, as
    `on_draw_scope` measures it; otherwise give no text.
  - When the pointer moves to another row, `TTM_POP` the tip so the next one
    asks again. Record the last row in `SettingsState`.
  - Put the "is it cut off" width check in one function that
    `on_draw_scope` and the tooltip share, so the two cannot disagree.
  - Senior review: `unsafe` Win32, and the tool subclasses the list box.
  - **[needs human]** check: in a scopes pane with one long-named and one
    short-named space, hovering the long one shows its full name, the short
    one shows nothing, and moving between rows updates the tip. It works in
    light and dark and at 150 %.

- [ ] **G13. Tab scrolls a long page to the focused control (platform).**
  **[needs human]** Found while building G10: on a page taller than the window
  (`settings_window/scroll.rs`), Tab onto a control scrolled out of view leaves
  it hidden.
  - Pure half (a junior may write it first, as G1's was): `PageScroll::reveal(&self,
    top: i32, bottom: i32, margin: i32) -> Option<i32>` in `page_scroll.rs`, the
    offset that brings a control's layout rows `top..bottom` into view with
    `margin` to spare, or `None` when it is in view already. A control taller than
    the view shows its top. Tests: above, below, inside, taller, and at both ends
    of the range.
  - Win32 half: after the message loop's `IsDialogMessage` moves the focus
    (`runtime/mod.rs`), and the new focus is a child of the shown page, read its
    rect in page coordinates, ask `reveal`, and scroll there through the path
    `scroll_page` already takes, so the cards repaint as for the wheel. Use
    `scaled` for the margin (8 px at 96 DPI).
  - Senior review: `unsafe` Win32, and the check runs for every message the
    settings window's loop sees, so it must cost nothing when the focus did not
    change (compare with the last focus stored in `SettingsState`).
  - **[needs human]** check: on a page taller than the window, Tab and Shift+Tab
    through every control: each one scrolls into view as it is reached, and the
    wheel, the bar, and PageUp/PageDown still work.

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
- [ ] **T14. Black hole (platform).** After T13. A dark disk opens at the
  space's own center (an `Overlay`, as Burn's flames are) and the space
  spirals into it: a composition visual can rotate and scale with its
  blur, which the accent path cannot. On Windows 10 and the accent path
  it plays Fade instead (D12, amended 2026-10-29, maintainer). It is one
  `EFFECTS` row plus its frame math and drawing.

T20-T21 add the effects D12 plans (amended 2026-10-18). They animate
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
- [ ] **T21. Magic lamp (core, app, platform).** D12's Magic lamp (amended 2026-10-29, maintainer) is the spec.
  The space pours into a spout on its own title; in Fold the spout
  settles back into the title, and on Delete and Startup's Out the title
  then flies into the tray icon. **[needs human]**:
  - Row: `effect(Effect::MagicLamp, "magic_lamp", &[])`. Fold offers it
    too (D12's Fold list).
  - The end, decided by the app and passed in as data: `LampEnd { Title,
    Fly(Rect) }`. Fold is `Title`; Delete and Startup are `Fly` to the
    tray target. The target is DeskSpace's tray icon
    (`Shell_NotifyIconGetRect`) when it is visible on the space's
    monitor; else the notification area on the primary monitor; else the
    middle of the bottom edge of the space's monitor work area. Pick it
    in a pure function over the rects, so each fallback is tested.
  - Pure mesh in `render/lamp.rs`: `lamp_slices(progress, rect, title,
    end, out: &mut [Slice; LAMP_SLICES])`, `LAMP_SLICES` = 48 horizontal
    slices of the space below its title. Pouring: the slices narrow
    toward a spout at the title's center, the narrowing growing toward
    the title, with the sides following a smoothstep curve, so the
    outline bends like a lamp's smoke; then they slide along that outline
    into the title, each shrinking to nothing as it arrives. With
    `Title`, pouring takes the whole progress and the title stays. With
    `Fly`, pouring takes the first part (start at 0.7 and tune on
    screen), then the title shrinks and flies along a gentle arc into the
    target. `Slice { src_y: (f32, f32), dst: Rect }`; the title is one
    more slice.
  - Window: an `Overlay::Lamp` variant (`win32/overlay.rs`, and
    `kind_for` gives it to `Effect::MagicLamp`), on `win32/lamp_overlay.rs`:
    a click-through, layered, never-activated window over the space's
    monitor work area, on T41b's `LayeredOverlay` (amended 2026-11-05, so
    the lamp does not copy `win32/flame_overlay.rs`; take T21 after T41b, or
    stop and ask). While Magic
    lamp plays, the space's own
    window is hidden and the overlay draws each slice from the start
    snapshot, then `UpdateLayeredWindow`. At the end of an in the window
    shows again and the overlay goes; at the end of an out the overlay
    goes (and the window is destroyed for a Delete, or shows folded for
    Fold). There is no blur while it plays. When the target is on another
    monitor (the primary's notification area), the overlay covers the
    bounding rect of both monitors' work areas, so the flight is seen
    the whole way.
  - Tests: at progress 0 the slices tile the space's rect exactly; with
    `Title`, at 1 every body slice is empty at the spout and the title is
    unchanged; with `Fly`, at 1 everything is empty at the target; widths
    only shrink as progress grows; slices never overlap; the target
    picker's three fallbacks.
  - **[needs human]** check: Magic lamp on Fold, Startup (show, hide, and
    exit), and Delete, on every backdrop at 100 % and 150 %, with the
    tray icon visible, in the overflow, and on another monitor: the body
    pours smoothly into a spout on the title; on Fold the spout settles
    into the title bar; on Delete and hide the title flies into the tray
    icon (or its fallback), and on show it comes back out; reversing
    mid-way turns it around; no rectangle of blur or tint and no overlay
    window is left after it ends, after a delete, or after quitting
    mid-way.

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

### Groundwork, a contact sheet, and smoke (T41-T48)

Planned 2026-11-05. Burn's upgrade (T25-T33 below) waits on groundwork that
was built on `feat/color-picker-redesign`, 330 commits behind `main`, and
never ported. The same branch built two kinds of smoke, Burn's plume and a
Smoke effect of its own, and left both tuned only for the debug window;
`main` has neither, and D12 does not list them. T41 ports the groundwork,
T42 lets anyone, an agent included, judge a frame without a screen. How
smoke comes back was decided on 2026-11-05 (T43, archived; D12 amended):
Burn's plume is part of Burn, always on, and Smoke is its own effect,
shown only in the debug window until its look is approved.

Rules for these, on top of section T's:

1. **Port, do not copy.** The branch predates the z-order fixes in
   `main`'s `flame_overlay.rs` (`place_above_owner`, `insert_after`, both
   2026-10-22) and the overlay extension point in `win32/overlay.rs`. Read
   the branch's code with `git show
   origin/feat/color-picker-redesign:<path>`, keep `main`'s design wherever
   the two differ, and bring over only what the task names. Never merge or
   cherry-pick the branch.
2. **Look before the human check.** From T42 on, each task that changes a
   look writes T42's sheets before and after, and the commit body says what
   changed in them. The **[needs human]** check still decides.

- [ ] **T41. Port the effect groundwork (platform).** Two parts, one commit
  each, in order. No visible change, so no section V row; say so in each
  commit body. The branch's third piece, `Play::seed`, has no reader until
  T25, so it lands there, with its first caller.
  - [ ] **T41a (junior). One noise module.** Move `fractal`, `value_noise`,
    and `lattice_value` from `render/burn.rs` into a new `render/noise.rs`,
    unchanged, and port the branch's `RisingField` (the bottom-biased field
    `BurnField::new` builds today) with its tests. `BurnField` then holds a
    `RisingField`. Before moving anything, pin three of today's field values
    in a test, so the move provably changes no burn. The branch's
    `random(seed, index)` waits for its first caller, T28.
  - [ ] **T41b (senior review; Win32). One layered window for every overlay.**
    Pull the window, its DIB section, `present` (today's `UpdateLayeredWindow`
    call), `follow`, and the z-order helpers out of `FlameOverlay` into a new
    `win32/layered_overlay.rs` (`LayeredOverlay`), keeping every `// SAFETY:`
    comment and both z-order fixes with their dated notes. `FlameOverlay` keeps
    the burn's field and flames and owns a `LayeredOverlay`. T21's lamp, T14's
    disk, and T44's smoke then use it too, instead of copying
    `flame_overlay.rs`. Move the `flame_overlay.rs` tests that test the window
    with it.
    `the_overlay_starts_directly_above_its_owner` must still pass.

- [ ] **T42. A contact sheet for Burn (platform; test tool).** **(junior)**
  After: T41a.
  - Why: no one can judge Burn without a person at the screen, and every
    look task below asks for that judgement. A picture of frames lets a
    contributor, or an agent that can read an image, compare before and after
    in seconds.
  - Move the WIC PNG writing out of the ignored `render_preview` test
    (`render/d2d/color_picker/mod.rs`) into a `#[cfg(test)]` helper both use.
  - A new `#[cfg(test)]` module, `render/effect_sheet.rs`, composes a frame on
    the CPU as the screen would: a sample space (its tint plus a few item-sized
    squares and a title band), masked by the burn's paper pixels and covered by
    its marks (bilinear between cells, as the painter draws them), with the
    flame overlay's pixels on top at its margins, over a mid-gray backdrop.
  - An ignored test, `burn_sheet`, writes one PNG per case to `%TEMP%`
    (`deskspace-burn-<case>.png`): a grid of nine frames, gone 0.1 to 0.9,
    with the clock at 0.05 s per frame. Cases: 320 × 240 and 1200 × 900 at
    96 DPI, and 320 × 240 at 144 DPI once T26 has landed. Run it with
    `cargo test -p deskspace-platform burn_sheet -- --ignored`.
  - Tests: the composer's pure parts (a fully gone cell shows the backdrop, a
    whole one shows the space).
  - Done when: the sheets are written, and the commit body describes them.
    Commit no PNG.

- [ ] **T44. Burn's smoke (platform).** **(junior, senior review)**
  **[needs human]** After: T41, T42, and T31.
  - The senior review is for the port: say in the commit body what was kept
    from the branch's version and what was left, and why.
  - Pure, in `render/burn_smoke.rs`; read the branch's version first and
    keep what worked (its emission ramp, so no smoke is left when a burn
    ends; its columns rather than a fog).
  - **Where.** Smoke rises from the fresh char just behind the burning edge,
    so it follows the fire, holes included. Its density lives on a coarse
    grid (`SMOKE_CELL_PX` 6 px times the scale), sampled bilinearly when
    shading.
  - **Motion.** It drifts up at `SMOKE_RISE_PX_PER_S` (40) times the scale,
    slower than the flames' 120, so it lags them. Low-frequency seeded noise
    scrolling with it sways each column, and a column widens as it rises.
  - **Colour.** Warm and lit from below near the edge (toward the fire's
    colour over its lowest 15 px), cooling to a neutral gray as it rises and
    thins; at most 55 % opaque, and gone by the top of the overlay.
    `flames::margins` grows its top to make room (`SMOKE_ROOM_PX` 120).
  - **Order.** Smoke, then flames, then ash, into the same buffer.
  - **Reversible.** Density is a function of the front, the seed, and the
    clock, and the clock only sways it.
  - In a release build, the smoke for a 1200 × 900 space shades in under
    2 ms a frame. Measure and put the timing in the commit body.
  - Tests: none at gone 0 or 1; the same inputs give the same pixels; two
    seeds differ; denser near the edge than near the top; every pixel stays
    premultiplied, and nothing is written outside the buffer.
  - **[needs human]** check, appended to the Burn row: a soft plume rises
    from behind the flames and leans as it climbs, warm at the bottom and gray
    at the top; it reads on a light and a dark wallpaper; there is no box, no
    tint over the space, and nothing left after the burn ends or reverses.

- [ ] **T45. Smoke, the effect (core, app, platform).** **Seniors only.**
  **[needs human]** After: T44. Still to design, and to bring to the
  maintainer first: how an `EFFECTS` row stays out of the settings
  window's Effect lists while the debug window still offers it (the
  branch's `c1ad0f3` mechanism, or a field on the row; D12, amended
  2026-11-05). Then port the
  branch's Smoke as `effect(Effect::Smoke, "smoke", &[Detail])`, by section
  T's recipe, with an `Overlay` variant on T41b's `LayeredOverlay`. Port the
  branch's later fixes too (`b86c153`, `1e80a85`, `7729dc1`, `11bcc83`): the
  plumes as tinted columns, smoke that outruns its front, the contents riding
  the current, and the hand-back without a frame's gap. Its check is the
  branch's T22 row, rewritten for `main`.

- [ ] **T46. A glow around the flames, on the CPU (platform).** **(junior)**
  **[needs human]** After: T29 and B53.
  - Why: T32 plans bloom on the GPU, and Windows 10 and the accent path will
    never have it. A cheap glow makes the CPU flames read as light, not paint.
  - In `flames::paint`, after the tongues: a quarter-size copy of the flame
    pixels, two separable box-blur passes (radius 3 cells), added back under
    the flames at `GLOW_GAIN` (0.5) with the additive `add` T29 brings. The
    scratch buffers are kept between frames, so a frame allocates nothing.
  - When T32 lands, the composition path draws its own bloom and skips this;
    both paths share `tongues`.
  - Budget: under 1.5 ms more for a 1200 × 900 overlay in release; measure.
  - Tests: an empty buffer stays empty; one bright pixel spreads evenly; every
    pixel stays premultiplied.
  - **[needs human]** check: the flames have a soft halo that warms the paper
    beside them; no hard edge where the glow stops.

- [ ] **T47. Sparks that cool (platform).** **(junior)** **[needs human]**
  After: T29.
  - Why: today's embers (`paint_ember`) are 2 × 2 squares of one colour.
  - Each spark is a short streak along its motion, as long as it moves in a
    sixtieth of a second, times the scale. Its colour runs down `FIRE_RAMP`
    as it rises, white-yellow to deep red, and its alpha follows. Each tongue
    throws 0 to 2 sparks, chosen by `noise::random(seed, col)`.
  - A spark flares when `noise::random(seed ^ FLARE_SALT, spark)` is below
    `FLARE_SHARE` (0.15). It flares once, at the height that same value
    picks along its climb: its brightness doubles, capped at 255, for
    `FLARE_PX` (6 px times the scale) of the climb. `FLARE_SALT` is a named
    constant. These numbers are a first guess (2026-11-07); tune them on
    T42's sheet and say so in the commit body.
  - Tests: the same inputs give the same pixels; a spark is cooler the higher
    it is; a flaring spark is brighter at its flare than just before it;
    pixels stay premultiplied and inside the buffer.
  - **[needs human]** check, appended to the Burn row: sparks streak up,
    cooling from yellow to red as they fade; no squares.

- [ ] **T48. The char edge lifts (platform).** **(junior)** **[needs human]**
  After: T30.
  - Why: real paper curls where it burns. A light rim on one side of the char
    and a shadow on the other make the flat band read as an edge lifting.
  - `render/burn.rs`: for a `Charred` cell, the direction the front came from
    is the field's gradient at that cell (a difference of its neighbours,
    worked out once in `for_space`). Lay a thin highlight (255, 230, 200) at
    `LIFT_ALPHA` 40 on the side away from the fire and a shadow (0, 0, 0) at
    `LIFT_ALPHA` on the side toward it, only within `LIFT_PX` (2 px) of the
    char's edge.
  - Tests: whole and gone cells are unchanged; the highlight sits on the
    unburned side for a field that burns straight upward; every pixel stays
    premultiplied.
  - **[needs human]** check, appended to the Burn row: the char's edge has a
    faint light rim on the paper side, as if it curls; at rest nothing is left.

### Burn, upgraded (D12, amended 2026-10-22)

T24-T33 make Burn read as fire rather than an orange wipe: it takes its
time and kindles, spreads unevenly, smoulders, throws turbulent flames and
ash, and looks the same at every DPI. One commit each, in order.

- **When to start.** T24 needs nothing and can start now. T25-T33 build on
  groundwork that is on `feat/color-picker-redesign` (as of `c1ad0f3`) but
  not in `main`; T41 below brings it over, so start T25 after T41. Where a
  task below names the branch's types, read `main`'s after T41:
  `EffectOverlay` and `Art::Burn` are `FlameOverlay` (the burn's state, in
  `win32/flame_overlay.rs`), and `Margins` is what `flames::margins` returns.
- **Who.** Tiers follow section P's "Who takes what": **(junior)** is pure
  pixel or model code whose design the task states, proven mostly by its
  tests, with no `unsafe`, thread, hook, or user file. **(junior, senior
  review)** is the same, with an algorithm, a budget, or a port that a
  senior reads before it lands. Calls the tasks add in `win32/` use safe
  functions that are already there; one that needs `unsafe` is a stop and
  ask (rule 4).
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
  **(junior)** **[needs human]**
  - Why: D12 (amended 2026-10-22). At the default 250 ms, Burn is over
    before its flames read. Smoothstep, the curve every effect but Bounce
    uses (`smoothstep` in `render/transition.rs`), slows at both ends, while
    a fire catches slowly and then races. An effect's pace belongs in its
    `EFFECTS` row, so Burn needs no `if` and Bounce no `match`.
  - `deskspace-core/src/animation.rs`:
    - `pub enum Curve { Smooth, Kindle, Bounce }`, with a `///` doc on each,
      and `pub fn apply(self, u: f32) -> f32`. `Smooth` is today's
      `smoothstep`; move it and its comment from `render/math.rs` here.
      `Bounce` is `bounce_ease` with its constants; move them here too. `Kindle` is
      `1 - (1 - u)²`, where `u` is how much of the space is shown. An out
      (u from 1 to 0) starts slowly and ends fast; an in starts fast and
      settles.
    - `pub struct Pace { pub stretch_percent: u16, pub curve: Curve }`,
      `pub const PACE_NORMAL: Pace` (100, `Smooth`), `pub const
      PACE_STRETCH_RANGE: (u16, u16) = (50, 300)`, and `pub fn
      stretch(self, duration: Duration) -> Duration`.
    - `EffectSpec` gains `pace: Pace` in place of its `stretch_percent`, and
      `effect()` takes it as a fourth argument. Every row gets `PACE_NORMAL`
      except Burn, which gets
      `Pace { stretch_percent: 200, curve: Curve::Kindle }`, and Bounce,
      which gets `Pace { stretch_percent: 200, curve: Curve::Bounce }`.
  - `render/transition.rs`: `Transition::frame` applies
    `self.play.effect.spec().pace.curve` in place of the `match` that
    chooses `bounce_ease` or `smoothstep`. Delete that match and both local
    curve functions.
  - `deskspace-app/src/app/animation.rs`: `play_for` already stretches the
    chosen duration before `tweak.slow`; have it read
    `spec().pace.stretch`. The exit stagger (`exit_delays`) uses
    `play.duration`, so it allows for the stretch with no further change.
  - The settings page does not change: the Duration control still shows
    the chosen time.
  - Tests:
    - core: every row's stretch is within `PACE_STRETCH_RANGE`; every
      curve is 0 at 0 and 1 at 1; `Smooth` and `Kindle` never decrease over
      101 even samples; `Bounce` stays within 0..=1 and lands at least
      three times; `Kindle.apply(0.9)` is above 0.98 (a burn barely moves
      in its first tenth).
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
  **(junior)** **[needs human]**
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
  - `Play` (`render/transition.rs`) gains `seed: u32`, its first reader being
    this task. The app leaves it 0, and the platform sets it when a play
    starts, from a counter and the clock, as the branch's `next_seed` in
    `win32/space_animation.rs` does, so spaces starting together differ.
  - `Mask::Burn` gains `seed: u32`, filled from `play.seed` in the Burn
    arm of `frame`. The painter's rebuild check
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

- [ ] **T26. Burn scales with DPI (platform).** **(junior)** **[needs human]**
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

- [ ] **T27. The field owns its bands (platform; refactor).** **(junior)**
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

- [ ] **T28. Burn spreads like fire (platform).** **(junior, senior review)**
  **[needs human]** The senior reads the solve and the timing before it lands.
  - Why: a threshold sweeping through noise moves at one speed along a
    blobby edge. Fire catches unevenly, climbs faster than it spreads,
    runs up in fingers, and burns holes that join the main front.
  - `render/noise.rs`: port the branch's `random(seed, index)` with its
    tests. T41a left it for this task, its first caller; T29-T31 and T47
    use it too.
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

- [ ] **T29. Turbulent flames (platform).** **(junior)** **[needs human]**
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

- [ ] **T30. A glow that lives (platform).** **(junior)** **[needs human]**
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

- [ ] **T31. Ash (platform).** **(junior)** **[needs human]**
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

- [ ] **T33. Heat haze (platform).** **[needs human]** After T32, and only
  on composition spaces, because their painter draws through an
  `ID2D1DeviceContext` (`SurfaceCanvas`). The plain `ID2D1HwndRenderTarget`
  has no effects, so other spaces play without haze; the maintainer
  accepted a look that differs by path (2026-10-29). The painter draws
  the space's contents through `CLSID_D2D1DisplacementMap`. The map is a
  scrolling noise bitmap, strongest `HAZE_PX` (20) above the burning edge
  and gone beyond 60 px. The scale is 4 × scale px, so the paper
  shimmers above the fire.

### Effects as data

Most effects only set a few fields of a `TransitionFrame` from how far along
they are (review, 2026-11-03), so they can be written as data: Roll moves the
rect toward the rolled one, Fade sets the opacity, Gradient sweeps the mask, and
Fly moves the rect toward an edge. As data, built-ins shrink to rows, and a user
effect becomes a pack (section N). Effects that draw their own art stay native
code: Dissolve's noise, CRT, Burn, Bounce's physics, and the planned Scroll, Magic
lamp, and Black hole. T34 is the design; T35-T39 move the built-ins one at a time
with no visible change; T40 opens it to users.

- [ ] **T34. A recipe type for simple effects (design).** **[needs decision]**
  Today `frame` (`render/transition.rs`) has one match arm per effect.
  Recommended:
  - A pure `Recipe` in `deskspace-core/src/animation.rs`, with serde, so packs
    can carry one:
    - `rect: RectMotion { Rest, Roll, Fly }`, reusing `Geometry`'s rolled rect
      and `fly_rect`;
    - `opacity: Ramp`;
    - `mask: MaskMotion { None, Gradient { feather } }`;
    - `pixelate: Option<Ramp>`.
  - A `Ramp { from, to, start, end, curve }` maps progress (0 to 1) to a value.
    It holds before `start` and after `end`, and is clamped on load.
    `curve: Curve { Linear, EaseIn, EaseOut, EaseInOut }` is a pure function
    with tests.
  - `EffectSpec` gains `recipe: Option<Recipe>`. `frame` plays the recipe when
    the row has one, and keeps its arm when the row does not.
  - To settle: whether `Curve` is the progress curve T24 (open) plans to add to
    each row, so the two land as one type, and whether `Detail` and `Direction` reach a recipe as parameters
    or stay native-only.
  - Done when: the maintainer approves a shape, and this entry is replaced by
    it.

- [ ] **T35-T39. Move one built-in effect to a recipe each.** **(junior)**
  After T34. In this order, one commit each: **T35** Fade, **T36** Roll, **T37**
  Fly, **T38** Gradient, **T39** Pixelate.
  - First copy the effect's current arm into the test module as
    `reference_frame`. Then give the row its recipe, and delete the arm.
  - Test: the recipe's frame equals the reference at progress 0, 0.1, … 1, both
    ways (in and out), for every `Direction` the effect uses, at two detail
    levels where it uses `Detail`. Compare floats within 1e-4.
  - The look must not change, so these add no section V row. If the test cannot
    pass without changing the recipe type, stop and ask; T34 missed something.
  - Commit as `refactor: play <effect> from a recipe (T3n)`.

- [ ] **T40. Effects from files.** **[needs decision]** After N3 and T35-T39.
  A `.deskspace-effect.toml` pack holds a `PackHeader` and a `Recipe`. To
  settle:
  - how a user effect is stored in a kind's settings, given that its file can
    go missing. Recommended: by file name, falling back to Fade with a
    warning, as an unknown key already does;
  - where loaded effects sit in each kind's list, and how Fold's "Offers"
    limit treats them;
  - whether DeskSpace ships example effect files.

  Then split it into tasks here. **[needs human]** check, with the last part:
  a hand-written effect plays in, plays out, and reverses mid-play.

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
  content area and grid. The current page is saved per space
  (`#[serde(default)]`, page 0) and clamped when pages shrink (decided by
  the maintainer, 2026-10-29). Pages are a view over the same `items`, so
  `ItemId`s and drag semantics are unchanged. Tests.
  - W16 has landed, so pages split the view layout (`view_layout.rs`,
    `ViewLayout`), not the old icon grid. That holds in every view, Details and
    List included.
  - **[needs decision]** (review, 2026-11-03): how a space splits into pages
    is a rule of its own, separate from how the strip looks. Recommended:
    `TabSettings.paginate: Paginate` (`#[serde(default)]`), whose first and
    only variant is `Fit` (fill a page, then start the next). Each later rule
    is a variant (J6), so the compiler points at every place to wire up.
    `page_items` takes the rule; `ViewLayout` stays rule-free.
- [ ] **J3. Render the strip.** `SpaceScene`/`SpaceStyle` carry the strip
  (edge, size, page count, current page, labels); the painter draws it and
  the window hit-tests a click to a page. Only the current page's cells are
  drawn and hit-tested.
  - The strip's colours come from the space's `Palette`, as the header and
    status bar do, so themes and colour schemes (section N) restyle it with
    no work here. Add a strip-style field to `Theme` only when a second style
    (dots, numbers) is designed, never ahead of it.
- [ ] **J4. Per-space Tabs settings section (app).** Enable, orientation, and
  size, through the per-space settings path.
- [ ] **J5. Space menu.** Enable/disable tabs, previous/next page, and a page
  list.
- [ ] **J6. Pages by group.** After J2 (with `Paginate`) and J5. `Paginate::ByGroup`:
  one page per group of the space's grouping (`deskspace-core/src/grouping.rs`),
  labelled with the group's name. A space that is not grouped falls back to
  `Fit`. Tests: pages follow the groups, and changing the grouping clamps the
  current page as J2 does.

## K. Split the large files (readability)

`ENGINEERING.md` asks for files under about 800 lines of non-test code. When one
grows past it, add a K task naming what stays in `mod.rs` and what moves where
(the archived K tasks are the model). Each K task is a **pure move**: no
renames, no logic changes, no signature changes beyond visibility. The limit is
enforced by `crates/deskspace-app/tests/file_size.rs`; its `ALLOWED` table lists
every file still over it, with the reason it is tolerated, and its K task once
one is written.

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
- Delete the split file's entry from `ALLOWED` in
  `crates/deskspace-app/tests/file_size.rs`; that test fails on a stale one.
- Done when: fmt, clippy, and tests pass with the same test count;
  `git diff --stat` shows mostly moved lines; and the file map above names
  the new files.

K10-K16 split the files `ALLOWED` listed as "none yet" (line counts are non-test
lines, measured 2026-11-03). Each lands alone, in any order, but read the
"Collides with" line first: a task there edits the same functions, so take one
at a time and rebase the other. Function names, not line numbers, say what
moves.

- [ ] **K10. Split `win32/folder_watch.rs`.** 1,278 lines. Turn it into
  `win32/folder_watch/mod.rs` plus:
  - `worker.rs`: `Worker` and its `impl` (from `run` to `drain_canceled`), with
    their tests.
  - `folder.rs`: `CanceledRead`, `Folder`, `OpenFolder`, `open_folder`,
    `follows_to`, `OpenFailure`, `final_path`, and `is_timeout`, with their tests.
  - `timing.rs`: `Bursts` and `Coalescer`, which are pure, with their tests, and
    `millis`.
  - `mod.rs` keeps the constants, `SharedListing`, `retire_listing`, `LensUpdate`,
    `FolderWatch` and its `Drop`, `PendingWatch`, and `ActiveWatch`.
  - This module runs a thread and an I/O completion port. A pure move changes
    neither, but ask a senior to review: a missed `pub(super)` that makes you
    reach for a wrapper is not a pure move any more. Stop and ask instead.
  - Done when: `mod.rs` is under about 500 lines and the K rules above hold.

- [ ] **K11. Split `app/view.rs`.** 1,253 lines. Turn it into `app/view/mod.rs`
  plus:
  - `system_look.rs`: `SystemLook` and its `impl`.
  - `cache.rs`: `ViewCache`, `CachedGrouping`, `StampHasher`, and
    `grouping_stamp`, with their tests.
  - `look.rs`: `Cells`, `LayoutScale`, and the `App` methods from `theme_for` to
    `metrics_of` (the look, scale, and layout accessors), with their tests.
  - `layer.rs`: `LayerState`, and the `App` methods from `refresh_shell_display`
    to `refresh_desktop_layer` (the desktop layer and takeover), with their tests.
  - `mod.rs` keeps `sync_windows*`, `view_for`, grouping, the refresh, restyle,
    and reflow methods, the view changes, and `changed_in_place`.
  - Collides with: P14c (`base_layout`, `Cells`) and P15f.
  - Done when: `mod.rs` is under about 600 lines and the K rules above hold.

- [ ] **K12. Split `scene.rs`.** 1,163 lines. Turn it into `scene/mod.rs` plus:
  - `details.rs`: `DETAILS_CELL_PAD`, `DetailsCellText`, and the functions from
    `details_view` to `details_cells`, plus `details_scroll_thumb`, with their
    tests.
  - `status.rs`: the `STATUS_*` constants and the functions from `status_bar` to
    `status_tip`, with their tests.
  - `chrome.rs`: the `TITLE_ICON_*`, `BACK_BUTTON_*`, and `SCROLL_THUMB_*`
    constants, `title_icon_rect` to `title_icon_request`, `title_tip`, and
    `thumb_end_margin`, `scroll_thumb`, and `thumb_span`, with their tests.
  - `mod.rs` keeps `SurfaceLook`, `ViewState`, the style functions, the space,
    layer, and drag scenes, `group_headers`, `item_layout`, the cell and label
    functions, and the text shadow helpers.
  - Many tests build a whole scene and check one part of it. Those stay in
    `mod.rs`; move only a test that calls a moved function directly.
  - Collides with: P15f, P16c.
  - Done when: `mod.rs` is under about 700 lines and the K rules above hold.

- [ ] **K13. Split `app/menus.rs`.** 1,131 lines. Turn it into `app/menus/mod.rs`
  plus:
  - `new_space.rs`: the `App` methods from `create_space` to `space_rects`, and
    the free functions `new_space_command` and `gap_at`, with their tests.
  - `entries.rs`: the free functions that build entry lists, from `menu_item` to
    `lens_view_after`, except the two above, with their tests. They take plain
    data, so their tests need no `Ui`.
  - `mod.rs` keeps the `App` methods that show menus and act on a choice.
  - Done when: `mod.rs` is under about 700 lines and the K rules above hold, and
    the "new menu entry" recipe above names the right file for each step.

- [ ] **K14. Move the ids and preset tables out of `app.rs`.**
  **[needs decision]** 1,024 lines. Its `ALLOWED` reason is "by design", because
  the recipes send you to `app.rs` for `MenuId` and the presets. Recommended:
  move `Timer`, `MenuId`, `NewShortcut`, `NEW_SHORTCUT_ENTRIES`, and every
  `*_PRESETS` and `*_MENU_IDS` table into a new `app/ids.rs`, re-exported from
  `app.rs`, which leaves about 690 lines. The alternative is to keep it whole and
  record why in `ALLOWED`.
  - If approved: `app.rs` stays `app.rs` (it already declares the `app/`
    modules), so add `mod ids;` and `pub(crate) use ids::*;`. Move `MenuId`'s
    tests with it.
  - Update the file map's `app.rs` row and the menu recipe to name `app/ids.rs`.

- [ ] **K15. Split `app/files.rs`.** 986 lines. Turn it into `app/files/mod.rs`
  plus:
  - `delete.rs`: the `App` methods from `delete_selected_items` to
    `delete_item_question`, with their tests.
  - `shortcuts.rs`: `add_shortcuts`, `create_shortcuts`, `write_shortcut`,
    `is_link_file`, `link_copy_name`, `shortcut_file_name`, and `ShortcutSource`,
    with their tests. Section L's rule holds: `add_shortcuts` stays the one way
    to add a shortcut.
  - `mod.rs` keeps `paths_to_transfer`, `StoredFiles`, and the drop handling.
  - This code moves and deletes user files. A pure move is safe, but ask a senior
    to review the diff.
  - Done when: `mod.rs` is under about 500 lines and the K rules above hold.

- [ ] **K16. Split `render/d2d/mod.rs`.** 959 lines. Add:
  - `chrome.rs`: `Painter::draw_details_header`, `draw_group_headers`, and
    `draw_status`, with `SORT_UP_GLYPH`, `SORT_DOWN_GLYPH`, and
    `DROP_MARK_WIDTH_RATIO`. Child modules can add `impl Painter` blocks, so no
    signature changes.
  - Move `draw_transition` and `draw_blur_mask` into the existing `effects.rs`
    (405 lines), which already holds an animation's resources.
  - `mod.rs` keeps the device functions, `DrawContext`, `FrameResources`,
    `Painter`'s paint path, `draw_scene`, `draw_items`, and the bitmap cache.
  - Collides with: P15f (paint clipping) and anything in section T.
  - Done when: `mod.rs` is under about 700 lines and the K rules above hold.

- [ ] **K18. Split `app/undo.rs` before it crosses the limit.** 791 lines
  (2026-11-05), so the next action that records an entry fails `file_size.rs`
  for whoever adds it, in a task about something else. Turn it into
  `app/undo/mod.rs` plus:
  - `stack.rs`: `FileChange`, `SpaceChange`, `UndoEntry`, `OpenEntry`,
    `UndoStack` and their `impl`s, `space_state`, and `same_space_state`, with
    the tests that build only a stack (`finish_caps_depth_at_undo_depth`,
    `new_entry_clears_redo_but_empty_action_does_not`, and any other that
    builds no `App`).
  - `files.rs`: the `App` methods that put files back, from `reverse_files` to
    `reverse_rename`, with `original_path` and `rollback_failed_text`.
  - `mod.rs` keeps `UNDO_DEPTH`, the label functions, `StepOutcome`,
    `Reversed`, the stale texts, `undo`, `redo`, the apply and restore steps,
    `arrange_space`, `snap_all_spaces`, and every round-trip test, which builds a
    whole `App`.
  - `files.rs` moves and restores user files. A pure move is safe, but ask a
    senior to review the diff, as K15 does. Add no `ALLOWED` entry: the file is
    under the limit today.
  - Done when: `mod.rs` is under about 400 lines and the K rules above hold.

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

- [ ] **Q8. Decode the task ids left in comments.** Decided (maintainer,
  2026-11-05): sweep them, then guard (`ENGINEERING.md` section 4 rule 10).
  About 180 comment
  lines still carry one (2026-11-05: core 22, config 5, app 96, platform 56,
  ROADMAP `Dn` ids not counted). Each sends a reader to the archive to learn
  why the code is so, and nothing stops a new one.
  - The parts, each its own commit, any order before Q8d:
    - [ ] **Q8a (junior).** Core, config, and update.
    - [ ] **Q8b (junior).** The app.
    - [ ] **Q8c (junior).** The platform. Every edited `unsafe` block keeps its
      `// SAFETY:` comment.
    - [ ] **Q8d (junior).** After Q8a-Q8c. A test beside `file_size.rs`
      (`crates/deskspace-app/tests/comment_ids.rs`) that fails on a task id in a
      `//` or `///` comment: a section letter this file uses and a number, as
      a whole word. ROADMAP's `Dn` and `F-nn` and Win32 names (`WM_`, `VK_`,
      ...) pass. The failure names the file and line.
  - For each id: look it up in this file or `archive/TASKS-done.md`, and say
    the why it stood for in the comment's own words. If the comment reads as
    well without it, delete just the id. If a comment only names the task
    ("added in W12"), delete the comment. Change comments only; Q's rules 2,
    3, and 5 hold (the commit body lists each rewritten comment's file).
  - Done when: the part's crates have no id left, and the gates pass.

## P. Pay down what we found while working

The **Found while working** list, triaged on 2026-11-02 against the code,
P28-P39 from the code review of 2026-11-04, P40-P45 from SonarCloud's
analysis of the same day, and P47-P48 from the list's triage on 2026-11-05.
"How to work a task" applies. Each
task says whether it changes behavior; a bug fix that does gets a section V
row (rule 8).

Who takes what:

- **Juniors and interns:** the change is local and the task states the design.
  A unit test or the compiler proves most of it. It touches no `unsafe` code,
  thread, hook, or user file. If the code does not match the task, stop and
  ask (rule 4); that is the expected move, not a failure.
- **Seniors only:** the change can lose a file, stall the UI, or break key
  input, or design is left to do. A senior may amend the design a task
  proposes, and records the change in the commit body.

A large task is split where a part can land alone, as C10 is: `P10a`, `P10b`,
each its own commit. Archive each part as it lands, and its parent with the last
one. A junior task that has to step over the line above says why and names a
senior to review it. A part marked **[needs decision]** waits for the maintainer,
and the parts after it wait too.

### Juniors and interns

Most severe first (ranked 2026-11-04), so start at the top. Take the first task
you can start: one marked "After" waits for the task it names, and a **[needs
human]** step waits for a person. Each lands alone unless it says "After". A new
task goes into its tier, not at the end.

#### High

A user can lose their layout, an attacker could reach every install, or the next release waits on it.

- [ ] **P14b. Measure Explorer's fixed-view cells (no code).** **(intern)** **[urgent] (v0.3.0)** **[needs human]** Part
  of P14. In Explorer, at 100 % and 150 %, measure one cell's width and height, gap
  included, in Medium, Large, and Extra large icons. Record the numbers in P14c,
  with the date.
  - How: open a folder with a dozen files, with Folder Options at its defaults (no
    compact view). For each view, take a screenshot (Win+Shift+S) and read the
    pixels in Paint, whose status bar shows the cursor's position. A cell's width
    is the distance between the left edges of two items side by side; its height
    is the distance between the tops of two rows. Change the scale in Settings >
    System > Display, and sign out and in again if Windows asks.
  - Record the Windows version beside the numbers: Windows 11 first, and Windows
    10 too if you have it.

- [ ] **P14c. Give the fixed icon views their own cell (fix).** **[urgent] (v0.3.0)** After: P14b. Part of
  P14. In `view_layout.rs`, beside `NORMAL_CELLS`, `pub fn fixed_view_cell(mode) ->
  Option<Size>`, measured in 96-DPI pixels with a comment saying so and when. If the
  150 % numbers are not 1.5 times the 100 % ones, stop and ask. `Cells::FixedView`
  carries that `Size`, which `layout_for` fills in from the space's mode. In
  `base_layout` (`app/view.rs`), give it its own arm, shaped like the `Cells::Look`
  arm: the cell times `scale.title`, which is the DPI alone, and `gap = 0`.
  - `ViewMetrics::for_view` needs no change: it hands an icon view the `GridSpec`,
    and `arrange_space` reads that too, so `base_layout` is the one place.
  - Tests: each view's cell, DPI scaling, and a look's spacing changes "Settings icon
    size" and no fixed view.
  - **[needs human]** check: three Extra large columns fit where Explorer's do.
    Update W12's row.

#### Medium

A rare loss of a user's file, memory that only grows, a broken install's shell entries, or groundwork a senior's fix needs.

#### Low

Logs, tooling, readability, docs, and checks that may close a task.

- [ ] **P18b. Make a failed capture explain itself (scripts).** Part of P18. In
  `scripts/onscreen/OnScreen.psm1`, add `Get-CaptureState`: each DeskSpace window's
  cloak state (`DwmGetWindowAttribute`, `DWMWA_CLOAKED`), the session
  (`IsWorkstationLocked` is there already; add the session id and whether it is the
  console's), and the input desktop's name. `smoke.ps1` and `shadows.ps1 -Pixels`
  print it, and `Assert-Check` a window is not cloaked, before they read pixels.
  - Never run these while the maintainer has a build running: `Start-DeskSpace`
    calls `dev.ps1`, which stops one.
  - Done when: a run prints the state, and with a space on another virtual desktop
    the check fails and names the cloak.

- [ ] **P43. Let clippy keep closures that only call a method out (lint).**
  `.and_then(|name| name.to_str())` says what `.and_then(OsStr::to_str)` says, with
  more to read. SonarCloud reports 21 of these (`rust:S1612`), and clippy's
  `redundant_closure_for_method_calls` found 36 (2026-11-04); the two lists overlap
  only in part. A lint keeps them out for good, as `ENGINEERING.md` section 2 rule 4
  wants.
  - Add `redundant_closure_for_method_calls = "warn"` to `[workspace.lints.clippy]`
    in the root `Cargo.toml`, with a one-line comment. Every crate inherits it, and
    CI's `-D warnings` makes it fail.
  - Fix what clippy reports. `cargo clippy --fix --workspace --all-targets
    --allow-dirty` applies most of them; read the diff before you commit.
  - Then open the SonarCloud list (project `generalchad_DeskSpace`, rule
    `rust:S1612`). Fix each hit that clippy did not, if the method reference
    compiles. List any that do not compile in the commit body, so the maintainer can
    mark them in SonarCloud.
  - Done when: the gates pass, and nothing but closures changed.

- [ ] **P12a. See whether a swallowed Alt shows anything (no code).**
  **[needs human]** The first step of P12. With an item selected, press
  Alt+Enter, Alt+Left, and Alt+Up, and watch for focus cues or any change on the
  desktop, on Windows 11 and on Windows 10. If nothing shows on either, delete P12
  in a commit saying so. Otherwise record what shows under P12.

- [ ] **P26a. See whether content still trails its edge (no code).**
  **[needs human]** The first step of P26. On a space with a title and items,
  play Roll, Fly, and Bounce from the debug window at 100 % and 150 %. Watch the
  edge for content lagging or a frame with no title. If nothing shows, delete P26
  in a commit saying so. Otherwise record which effect shows it under P26.

- [ ] **P16d. Measure Explorer's List column (no code).** **[needs human]** Part of
  P16. Measure Explorer's limit on a List column's width, and whether a change
  re-flows. Record it in P16e, with the date.

- [ ] **P16c. "Size all columns to fit" (feature).** After: P16a, P16b. Part of P16. In
  the Details header menu, by the menu recipe. It records one undo entry, like Choose
  columns. Take the widths from plain data in a function, and test it. **[needs
  human]** check: on a space and a 50,000-file lens, no column clips and no stall.

- [ ] **P47a. See whether a browsed lens lists too often (no code).** Part of P47.
  While a lens is browsed into a subfolder, the worker watches the lens's whole
  folder tree (`Folder::open` in `win32/folder_watch.rs` asks for the subtree when
  `root` is set), so a change anywhere under it lists the shown subfolder again.
  - Make a throwaway lens on a copy of a Rust project, browse into a quiet
    subfolder of it (`src`), and run `cargo build` in the copy while
    `./scripts/dev.ps1 -Release -Verbosity 1` logs. Count the "built a lens's
    list" lines for the shown folder per second, and note their `elapsed`.
  - Record the rate, the times, the date, and the tree's file count under P47.
    If the shown folder is listed more than once a second while nothing in it
    changes, P47b is ready. Otherwise delete P47 in the commit, with the numbers
    as the reason.

### Seniors only

These are in the order we would take them. Each lands alone.

- [ ] **P46. Bring the shell's file-operation windows to the front (fix; Win32).** **[urgent] (v0.3.0)**
  A shell file operation that needs an answer can put its window behind the
  foreground app, and DeskSpace then looks frozen: nothing answers, the user
  force-quits, and the operation is lost. `Ui::transfer_files`
  (`win32/runtime/ui_shell.rs`) gives `file_op::transfer` `runtime.controller` as
  the owner, and that window is hidden. DeskSpace never holds the foreground
  (spaces are never activated, and the drag source, Explorer, keeps it), so the
  shell creates its `OperationStatusWindow` without bringing it forward.
  - Seen 2026-11-04 on 0.3.0-alpha.1: a right drag of a clashing file onto a lens
    with "Copy here" showed no dialog. The process was `Responding`; a visible,
    ownerless `OperationStatusWindow` ("Replace or Skip Files", 463x292 at
    345,267) sat behind the foreground editor. Forcing it topmost let the check
    finish and the copy completed. The same applies to a conflict or progress
    window from any drop, paste, delete, or rename that DeskSpace starts.
  - Fix, chosen 2026-11-04: while our own shell UI may be open, find it and raise
    it.
    - `transfer`, `delete`, `rename`, and `invoke_undelete` in `win32/file_op.rs`
      are the callers; each already takes an owner.
    - Arm a Win32 timer on the controller before `PerformOperations` and kill it
      after. `PerformOperations` pumps messages, so the timer fires while the
      shell works.
    - Its handler enumerates this process's top-level windows
      (`EnumThreadWindows` on the UI thread), matches class
      `OperationStatusWindow` and the process id, and calls
      `SetWindowPos(HWND_TOPMOST, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE |
      SWP_SHOWWINDOW)` once for each. The process id keeps another app's dialog
      out of it, and `SWP_NOACTIVATE` keeps a space from taking the focus.
    - The class name is undocumented; keep it in one named constant with the
      reason and a link beside it, next to the shell calls in `file_op.rs`, not in
      the app.
  - Tests: the class match and the timer's place among the controller's ids.
  - `CONTEXT.md` section 6, file operations: a line saying the shell's own window
    is raised while the operation runs, and why. `ENGINEERING.md` section 6 gets
    the gotcha: DeskSpace never holds the foreground, so shell UI it starts has to
    be raised.
  - **[needs human]** check: with a lens on a throwaway folder and Explorer
    maximized over the spaces, drop a clashing file: Replace and Skip both show in
    front and work. The same for a long copy's progress window, a move's error
    window, and a rename collision. With `--on-top` and the spaces above
    everything, the window still shows.
  - Split by person and landed as one commit, as P36 is, on the branch
    `fix/p46-shell-dialogs`.
  - [ ] **P46a (junior; hands over to P46b).** In `win32/file_op.rs`: the
    `OPERATION_STATUS_CLASS` constant with its reason and a link, and a pure
    `is_our_operation_window(class: &str, window_pid: u32, our_pid: u32) -> bool`,
    with tests: our class and our process id match; another process's window, or
    another class, does not; the class compares exactly. Also the `CONTEXT.md`
    and `ENGINEERING.md` lines, a `CHANGELOG.md` "Fixed" line, and the section V
    row. Hand it over as P36a does. Touch nothing `unsafe`.
  - [ ] **P46b (senior).** Arm and kill the timer around `PerformOperations` in
    `transfer`, `delete`, `rename`, and `invoke_undelete`, and write the handler's
    `EnumThreadWindows`, `GetClassNameW`, `GetWindowThreadProcessId`, and
    `SetWindowPos` calls, as the task says. Review P46a's half, then commit both.
    - Amended 2026-11-07, for the senior to confirm: the controller has no timer
      ids of its own. Its `WM_TIMER` becomes `Event::Timer` for the app's `enum
      Timer`, and `SetTimer` with an id already in use on that window replaces the
      app's timer. So use a thread timer, `SetTimer(None, 0, ..)` with a
      `TIMERPROC`: it has a system-chosen id and no window, and `DispatchMessage`
      in the shell's own loop calls the procedure. Kill it by the id `SetTimer`
      returns. This replaces "the timer's place among the controller's ids" in
      the task's tests; say in the commit body which way you went.

- [ ] **P32. Keep the keyboard hook alive through a stall (fix; key hook).** Windows
  removes a low-level hook, and does not tell it, when the hook's thread does not
  answer within `LowLevelHooksTimeout` (about a second). The hook runs on the UI
  thread (`win32/keyboard.rs`). It stays installed while any space is current, and
  `App::on_event` sets the flags after every event, so that is most of the time.
  `HOOK` still holds the dead handle, so `refresh` never installs it again. After one
  stall of a second or more, Delete, F2, Ctrl+C, the arrows, and type-to-select stop
  working on spaces until every flag clears. A slow network lens, a save to a
  redirected profile (P38), or a debug build at startup can each cause such a stall.
  `outside_click.rs`'s mouse hook has the same weakness.
  - P32a, in the junior list, adds the button that reproduces it: select an item,
    press the button, then press F2.
  - Design, then bring it to the maintainer:
    - (a) Recommended: run both hooks on a thread of their own that only pumps
      messages, so a stall on the UI thread can never time them out. The flags
      become atomics that the UI thread sets. The hook already posts to the
      controller, which works from any thread. Check that `typeahead_char` reads the
      foreground thread's keyboard layout, not the hook thread's.
    - (b) Cheaper: unhook and hook again on every press on a space, so the next
      click mends it. Keys are still lost between the stall and that click.
  - Tests for whatever becomes pure. `ENGINEERING.md` section 6, Keys, gets a line
    saying which thread the hook runs on.
  - **[needs human]** check, with P32a's button: after a stall, F2 and Delete still
    work, at once with (a) or after one click with (b). The Keys-table rows in
    section V still pass.

- [ ] **P12. Mask a swallowed Alt combination, if it matters (fix; key hook).**
  Swallowing Alt+Enter, Alt+Left, or Alt+Up leaves the foreground window a lone
  Alt tap (`handle_watched`, `win32/keyboard.rs`). The original note worried
  about a menu showing. `handle_watched` only swallows while the desktop shell
  has the foreground, so an app's menu cannot show; keyboard cues on the desktop
  are all that is left.
  - After: P12a, which reproduces it and may delete this task.
  - If it shows: after a swallowed key with `mods.alt`, `SendInput` a down and up
    of an unassigned virtual key (0xE8, as AutoHotkey's `A_MenuMaskKey` does).
    Keep the choice in a pure function with a test.
  - `hook_proc` sees that injected key too and has no `LLKHF_INJECTED` check.
    Confirm it passes through `watched_key`, `typeahead_char`, and
    `handle_editing` unconsumed, and that the hook stays within its time limit.
    `ENGINEERING.md`'s Keys table gets a line.
  - **[needs human]** check: the three combinations show no cue, Alt+Tab and
    Alt+F4 still work, and a rename still cancels on Alt.

- [ ] **P13. Keep a normal space's item facts fresh (feature).** A
  normal space's item reads its dates and size once, when added. An item saved
  before W29 has no Date created and sorts as the oldest; every item's `size` and
  `modified` go stale (`CONTEXT.md` section 10). A lens re-reads them on each
  listing. P13a-P13c are in the junior list, so this task is the part that remains:
  - [ ] **P13d. Read the facts on the worker, and apply them.** After: P13a-P13c.
    At startup, B33's worker calls `read_item_facts` for each normal space's items
    and posts them in one batch per space, leaving out a missing file. A new
    `Event` hands a batch to the app, which dispatches `RefreshItemFacts` through
    `commit` with no undo entry open, or holds it until any open entry finishes.
  - B33's queue drops paths scrolled out of view. These jobs must not be dropped,
    and must yield to icon requests, so scrolling stays smooth.
  - Tests: a refresh leaves undo available.
  - `CONTEXT.md` section 10: update the sentence. **[needs human]** check: start
    on a config with a few hundred items on a slow disk: no stall, and sorting by
    Date created orders an old item correctly.

- [ ] **P14. Give the fixed icon views their own cell.** A fixed view scales the
  whole grid by icon size over 48 px (`fixed_icon_scale`, `app/view.rs`). With the
  default 96 × 104 cell, Large icons gets a 192 × 208 cell and Extra large about
  512 × 555, so three Extra large columns need about 1,500 px. W12's check does not
  mention this. The behavior is decided (ROADMAP D17, amended 2026-11-02). P14b and
  P14c are in the junior list. **Land them before the next release:** W12 is in no
  release tag (the newest is v0.2.4, checked 2026-11-02), so no saved space needs a
  migration until one ships.

- [ ] **P15. Scroll Details sideways.** Columns past the space's width are cut off
  (`scene.rs` `details_columns`), and W30 makes that easy to reach: every column on
  takes 560 px at 96 DPI. The design is ROADMAP D24. P15f is the part that remains:
  - [ ] **P15f.** After: P15b. Go through what holds an x and may not read those
    rects: the rename box, the marquee, the drag drop index, and paint clipping cells
    at the content's edge. Fix each or show it needs nothing, and scroll Name into
    view when a rename starts. Drop the note from W30's row, and judge W31's "Name
    takes the rest" past the edge.

- [ ] **P16. Measure labels.** Two Explorer behaviors wait for this: the Details
  header menu's "Size all columns to fit" (W30) and List columns sized to the longest
  label (W17 uses a fixed 200 px and an ellipsis). The design is ROADMAP D25. P16a,
  P16c, and P16d are in the junior list, so these are the parts that remain:
  - [ ] **P16b.** The `Ui` method, over `CreateTextLayout` as `d2d/title.rs` uses it,
    with a test that "WWW" measures wider than "iii".
  - [ ] **P16e.** After: P16a, P16b, P16d. List's cell width follows the longest
    label: where it is stored, and when it is measured again. A `SyncLens` is the
    likely trigger, and no scene or wheel notch may measure.

- [ ] **P17. Keyboard support for the Shortcuts preview (accessibility, M7).**
  The Shortcuts page's preview (`win32/settings_window/preview.rs`) is not a Tab
  stop, and its X and + are mouse-only. Take it with M7 item 4, not before.
  - [ ] **P17a (senior).** `WS_TABSTOP` on the field: `IsDialogMessage`
    (`runtime/mod.rs`) then gives it Tab. `WM_GETDLGCODE` asks for the arrow keys and
    Enter and leaves Tab and Esc to the dialog, so Esc still cancels. Add a focused
    target to `PreviewView`, and draw a focus rectangle only while the window has
    focus (`WM_SETFOCUS`, `WM_KILLFOCUS`).
  - [ ] **P17b (junior).** After: P17a. Move the target with a pure function on
    `PreviewLayout`, with tests, called from the arrow keys. Scroll it into view.
  - [ ] **P17c (senior).** After: P17b. Delete raises what the X's click raises, and
    Enter on the + what its click raises, through `on_click`'s events. So a lens's
    Recycle Bin question still shows. Remember the window may be gone after a dispatch
    (`ENGINEERING.md` section 6, Re-entrancy). Delete the "no keyboard support"
    sentence from the file's header.
  - **[needs human]** check, with P17c: Tab in and out; arrows, Delete, and Enter, on
    a space and a lens, at 100 % and 150 %, light and dark.

- [ ] **P48. A keyboard path to the settings header's icon and rename
  (accessibility, M7).** The header's icon and name (`settings_window/header.rs`,
  S11, S12) open the icon picker and the inline rename only on a double-click
  (`STN_DBLCLK`), so neither can be reached from the keyboard. Take it with M7
  item 4 and P17, not before.
  - Proposed, which a senior may amend: both statics get `WS_TABSTOP`, so
    `IsDialogMessage` gives them Tab, and they draw a focus rectangle only while
    focused, as P17a's field does. Enter or Space on the icon opens the picker;
    Enter or F2 on the name starts the rename. Each raises the event its
    double-click raises, so the app gains no second path.
  - Both statics live only on a space's own pages; a Global scope has no header
    to reach. Remember the window may be gone after a dispatch (`ENGINEERING.md`
    section 6, Re-entrancy).
  - **[needs human]** check: Tab reaches the icon and then the name, light and
    dark, at 100 % and 150 %; Enter opens each; Esc cancels the rename and leaves
    the focus on the name.

- [ ] **P18. Make the on-screen harness explain a wallpaper capture (scripts).**
  `scripts/onscreen/smoke.ps1`, run by an agent on the maintainer's machine on
  2026-10-31, read the wallpaper instead of the backdrop and the space, though
  both windows reported visible at their rects. O2 and O3 passed on the same
  machine on 2026-10-27, so a state or an update changed, not necessarily the
  code. `shadows.ps1 -Pixels` depends on the same capture. It did not happen
  again the same day: five runs of `smoke.ps1`, in the agent's shell and in a
  child `pwsh`, and `shadows.ps1 -Pixels` all read the right pixels (session 1,
  the console, with `Default` as the input desktop). The cause is unknown.
  - Unproven suspects, to rule out in order:
    1. The windows are cloaked (`DwmGetWindowAttribute`, `DWMWA_CLOAKED`). A
       cloaked window reports visible with a rect but is not composed, which
       fits the symptom. Another virtual desktop would do this.
    2. The session: locked (`IsWorkstationLocked` exists), disconnected, or not
       the active console.
    3. A Windows update changed what `GetDC(NULL)` with `BitBlt` and `CAPTUREBLT`
       returns.
    4. `Show-Backdrop` creates its window on PowerShell's thread, which pumps no
       messages. This does not explain the space, though.
    5. The input desktop, or a display asleep at that moment.
  - P18a and P18b are in the junior list. This task is what is left after them: if
    a capture fails again, read what P18b prints, and run it from the maintainer's
    own terminal as well as an agent's, to split the environment from the code. Until
    then there is nothing to do.
  - Never run this while the maintainer has a build running: `Start-DeskSpace`
    calls `dev.ps1`, which stops one.
  - Stop and ask before reaching for `Windows.Graphics.Capture`, as `smoke.ps1`
    says.
  - Done when: the cause is written down here with what would fix it, or the
    maintainer closes it because it has not come back.

- [ ] **P26. Stop content trailing a resizing animation's edge (fix).** After: P26a,
  which may delete this task. A composition surface of a new size reaches the screen
  a composition or two after `SetWindowPos` (`apply`, `win32/space_animation.rs`), so
  while Roll, Fly, or Bounce changes the window's size its content can trail its edge.
  Before Bounce was slowed, the first frame past the screen's edge showed a title-less
  box for one frame (captured 2026-10-31). Moving a whole-size window under a region
  would avoid it, but a window that leaves its monitor can get `WM_DPICHANGED`, which
  the app answers by reflowing the model. So that answer must tell an animation's
  move from a real one. Design first, and bring it to the maintainer.

- [ ] **P38. Write the config off the UI thread (perf; thread).** `App::save`
  serializes and writes the whole config on the UI thread 700 ms after the last
  change, and `write_atomic` waits for `sync_all`. That is quick on a local SSD. Many
  companies redirect roaming AppData to a network share, though. There, each save can
  stall the spaces for longer than the keyboard hook's timeout (P32).
  - Measure first: log each save's duration at `debug`, and `warn` above 250 ms. If
    no report or measurement shows a slow save, record that here and stop.
  - If it is needed: serialize on the UI thread, which is cheap, and hand the text to
    one writer thread that keeps only the newest. `flush_save_on_close` waits for it,
    bounded (`ENGINEERING.md` section 6), so the last save still lands before exit.
    A failed write comes back as an event, so `save`'s one alert still shows.
  - Tests: the newest-wins hand-off, as a pure type.

- [ ] **P47. List a browsed lens only when its own folder changes (perf; thread).**
  P47a, in the junior list, measures whether this is needed and may delete it.
  - [ ] **P47b.** After: P47a, and K10 (both edit `folder_watch.rs`; take K10
    first). The notify records already name each changed path, relative to the
    watched root. Before a subtree change debounces into a listing, keep it only
    when its parent is the shown folder, or when it is the shown folder itself or
    a folder on the way down to it (a rename or delete there must still reach the
    lens). Put that rule in a pure `touches_shown(root, shown, changed) -> bool`
    beside `Coalescer`, with tests: a change in the shown folder, in a sibling, in
    a deeper child, and to each folder on the path, and the same under a junction
    spelling. An overflowed read (no records) still lists, as now. Keep the
    worker's lifetime and its waits exactly as they are (`ENGINEERING.md` section
    6). **[needs human]** check: P47a's setup lists the shown folder only when it
    changes, and renaming the browsed folder from Explorer still takes the lens
    back as W42's row says.

- [ ] **P39. Catch a re-entrant `with_state` borrow at run time (fix; unsafe).**
  `window_state::with_state` hands out a `&mut T` made from a raw pointer. Only its
  `# Safety` contract stops a second borrow of the same window while the first is
  live, and a slip there is undefined behavior rather than a logged bug.
  `with_runtime` catches the same mistake for the runtime with a `RefCell` and logs
  it, but window state has no such net. R19 designs the lasting fix (a handle); this
  is the cheap net until then, and R19 can keep it.
  - A thread-local list of the windows borrowed now. `with_state` on a window already
    borrowed logs at `error` and returns `None`, as `with_runtime` does. `free` on a
    borrowed window logs and leaks the box instead of freeing it, so a slip leaks
    memory rather than use it after it is freed.
  - No allocation per call on the paint path: a list that stops growing after its
    first use, or a small fixed array.
  - Tests need a real window: create a message-only one (`HWND_MESSAGE`) in the test,
    or stop and ask. A nested `with_state` on one window returns `None` and leaves
    the outer borrow working. Nested borrows of two different windows both work.

- [ ] **P45. Split the color picker's window procedure (refactor; unsafe).**
  `wndproc` in `win32/color_picker/mod.rs` (198 lines) handles its 17 messages inline
  in one `unsafe` block. SonarCloud rates it the most complex function in the code
  (cognitive complexity 46; `rust:S3776`).
  - A pure move: each arm's body becomes a function named for its message
    (`on_mouse_move`, `on_key_down`, and so on), and `wndproc` keeps the `match`
    with one call per arm. Copy how `space_window/mod.rs` hands mouse messages to
    `space_window/mouse.rs`. If the handlers pass about 300 lines, put them in a new
    `color_picker/input.rs`.
  - The picker's state is borrowed, not owned (`window_state.rs`): every handler
    keeps its `with_state` borrows exactly as short as they are now, and
    `WM_NCDESTROY` still only detaches. Every moved `unsafe` block keeps its
    `// SAFETY:` comment, reworded only where its context changed.
  - Done when: the gates pass with the same test count, and `wndproc` is under about
    60 lines. **[needs human]** check: open the picker from Colors, drag in the
    field, type a hex value, use the arrows and Esc, and sample from the screen with
    a right-click. All behave as before.

- [ ] **P51. Report a composition draw Direct2D refused (fix; render).** After the
  v0.3.0 tag. P49 hid for days because a composition surface's
  `ICompositionDrawingSurfaceInterop::EndDraw` returns success even when Direct2D
  discarded the draw's commands (`D2DERR_WRONG_FACTORY` there), so `paint` logged
  nothing and the window kept an old frame.
  - In `DrawingSurface::end` (`render/d2d/canvas.rs`), call `Flush` on the context
    after popping the clip and before the surface's `EndDraw`, and return its error,
    so `Painter::render` sees it: a lost device is recreated as now, and anything
    else is logged as "space paint failed". Still call `EndDraw` when `Flush` fails.
  - Tests: a draw that uses a stroke style from another factory (P49's test shows
    how to make one) returns an error from `Painter::draw` on a surface canvas; or,
    if a test cannot make a composition surface, stop and ask.
  - Done when: the gates pass. **[needs human]** check: spaces still draw, and a
    `-v` log shows no new errors over a session of clicks, folds, and settings
    changes.

## D. Reserved (strong model or maintainer; do not start)

Each needs design work first. Ids are `R`-numbered so they do not collide with
the ROADMAP decision ids (`D1`-`D9`).

- **R7. On-screen verification pass** (maintainer). The list is section V.
- **R10. Code signing (maintainer, ROADMAP M8 item 3, D6).** Apply to
  SignPath Foundation (an OSI license, which Apache-2.0 is, and a public
  repository and build). Add their GitHub Actions integration to
  `release.yml`: sign `deskspace.exe` before both the zip and the installer,
  and the installer through cargo-packager's `windows.signCommand` (a shell
  command with `%1` for the path); credentials as Actions secrets. The
  update feed does not wait for this (D6, amended 2026-10-31), so until it
  lands unsigned updates trigger SmartScreen on every version.
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
  Global scope, and greyed-out rules (`MatchExplorer`) fit. B10 has landed,
  and so have the W tasks that held it back ("Section W's phase notes" in
  `archive/TASKS-done.md`), so nothing blocks it now. Every themeable or animatable option pays the
  three-place cost, so this comes before the pack kinds in section N add
  options. It is close to urgent: `settings/pages.rs` measured 797 non-test
  lines on 2026-11-05, so the next settings option fails `file_size.rs`. Until
  R14a is decided, that option's task adds an `ALLOWED` entry for `pages.rs`
  naming R14, rather than splitting what R14 rewrites. The split (review,
  2026-11-03):
  - **R14a (senior, design).** **[needs decision]** The row type, written up
    here for the maintainer, with one real field moved as the proof (an
    `icon_size`-like range field in a look page). Settle the questions above,
    plus how a row names its `Effect`, and whether a label lives in the row
    (R18 wants it to).
  - **R14b… (junior, one page per commit).** After R14a. Move one settings page
    at a time, in the order `pages.rs` builds them. `field::` ids keep their
    values (never renumber). The handler tests keep passing until R14z.
  - **R14z (junior).** Replace the handler tests with one row-table test, delete
    the arms left in `apply.rs`, and rewrite the "new global setting" recipe as
    "add a row".
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
  security than they are worth now. A proposal that settles these, and its
  split into tasks, is N5 (section N).
- **R18. Translation design (F-60, ROADMAP M9).** The format and where
  catalogs live are decided (ROADMAP D20: Fluent, embedded, and a test that
  every key the code uses has English text). Settle:
  - how the platform's own strings (color picker, installed-apps picker,
    custom shortcut dialog, right-drop menu, info tip) reach it without the
    platform holding catalogs;
  - the fallback chain (region, language, English) and plural rules;
  - how the settings window fits longer labels (measure or wrap; start from
    the layout constants in `settings_window/build.rs`, and from its `DT_CALCRECT`
    call, since D25's measuring is for what Direct2D draws).

  Then split M9 items 2-5 into one-commit tasks here. If R14 lands first, a
  field's label belongs in its row.
- **R19. A space window handle instead of `unsafe fn` contracts.** About
  twenty functions in `win32/space_animation.rs` and
  `win32/space_window/` are `unsafe fn` only for "`window` is a live
  space window of this thread, and no state borrow is held", each call
  repeating `// SAFETY: the caller's contract.` Design a handle that only
  the runtime makes for a live space window, whose methods are safe.
  Settle: how the borrow rule is kept (a guard, or `with_state` private to
  the handle), how the desktop layer and a dying window fit, what
  `WM_NCDESTROY` invalidates, and how the handle stays on its thread.
  Then split it into one-commit tasks by module here. It touches
  `window_state.rs` and every window module, so plan it after K8.

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

- [ ] **O5. Measure idle CPU and wake-ups (scripts).** **(junior)** M7 item 5
  asks for idle CPU near zero, and nothing measures it.
  - `scripts/onscreen/idle.ps1`, importing `OnScreen.psm1`: start DeskSpace on
    a scratch config with five spaces and one lens on a quiet folder
    (`Start-DeskSpace`, which runs the debug build with `--no-icons`; idle cost
    does not depend on either), wait 15 s for listings to settle, then sample
    for 60 s: the process's CPU time (`TotalProcessorTime`) and its threads'
    context switches per second (`Get-Counter` on `\Thread(deskspace*)\Context
    Switches/sec`). Print both, and the busiest thread.
  - Pass under 0.1 % CPU and 5 wake-ups a second; above either, the script
    fails and names the thread. The thresholds are named at the top of the
    script, as a first guess to tune.
  - Never run it while the maintainer has a build running: `Start-DeskSpace`
    calls `dev.ps1`, which stops one.
  - Done when: a run prints both numbers; record the first run, with the date
    and the machine, in the commit body. A failure is a **Found while working**
    line naming the thread, not a fix in this task.

- [ ] **O6. Send keys to the desktop (scripts).** **(junior)** After: O4.
  - Why: the marked rows that change a view, undo, rename, zoom, or open a
    menu begin with a key, and the hook sees keys only while the desktop shell
    has the foreground (`ENGINEERING.md` section 6, "Keys").
  - `scripts/onscreen/OnScreen.psm1`: `Send-DeskSpaceKey -Chord 'Ctrl+Shift+1'`
    over `SendInput` (`KEYBDINPUT`); accept the chord names in the app's
    hotkey parser plus `F2`, `Esc`, `Enter`, `Delete`, `Home`, `End`, `PageUp`,
    `PageDown`, the arrows, and typed text. Focus the desktop first
    (`SetForegroundWindow` on `Progman`/`WorkerW`), because a space never
    activates, and wait for the app to handle the key before asserting.
  - Rows: the marked rows whose action is a key: F1-F5, G1/G3, B23, C7, C12,
    F9, F13, F15, R17, U3-U7, W12, W13, W15-W17, W19, W25, W28, W30-W32, W35,
    W39, W42-W48, P6, P10, P20, P23, P25.
  - Done when: a throwaway check moves a space (W12), undoes it (U4), and the
    window rect and config show both; `TOOLS.md` section 3 lists the helper.

- [ ] **O7. Click, double-click, and drag (scripts).** **(junior)** After: O4.
  - Why: the marked rows that open a menu, a picker, a header drag, or the
    settings window begin with a mouse action; the harness can say what a
    point reaches but cannot press it.
  - `OnScreen.psm1`: `Invoke-DeskSpaceClick -At (x,y) [-Button Left|Right]
    [-Count 1|2] [-DragTo (x,y)] [-Steps n]` over `SendInput` (`MOUSEINPUT`),
    moving in steps for a drag and releasing every button in `finally`. Add
    `Get-DeskSpaceTrayRect` over `Shell_NotifyIconGetRect`; the tool's id is in
    `win32/controller.rs`, never guessed.
  - Rows: the marked menu, tray, settings, header, picker, and drag rows:
    S9, S10, S12, C7-C9, C11, C12, F9, F14, G5a-c, G10, H5/H4, W12, W16, W17,
    W30-W32, W42, X5, X9, X14, X19, X20, L2, L6, Layer presses, Layer rename,
    Desktop menu, Menu at the click, New on the desktop, Selection, Mouse
    modifiers, B30, P7, P10.
  - Done when: a click opens the space menu and a drag moves a space, with the
    window rect and config proving both.

- [ ] **O8. Drop a file or a shortcut (scripts).** **(junior)** After: O4.
  - Why: the file rows are the riskiest and most numerous the release waits
    on, and only a drop reaches them.
  - `OnScreen.psm1`: `Invoke-DeskSpaceDrop -Path <path> -At (x,y)
    [-Effect Move|Copy|Link]` with an OLE `IDataObject` (`CF_HDROP` for a path,
    `FileGroupDescriptorW` for a shell or Start item) driven through
    `SHDoDragDrop` at the window under the point. The drag image and cursor are
    taste and stay human.
  - Rows: W3c-W3g, W15, W42, G5a-c, L2, L5, L6, Drag and drop, Desktop drop,
    Spaces above the layer, U5, U7, X7-X9, Delete stress spaces, P25.
  - Done when: a drop moves a fixture file into a space, the config lists it,
    and a Shift-drop leaves it where it was.

- [ ] **O9. File fixtures and the Recycle Bin (scripts).** **(junior)** After:
  O4.
  - Why: the file rows need known files, a read-only config, a held-open file,
    and what the Recycle Bin holds, without touching a real user file.
  - `OnScreen.psm1`: `New-DeskSpaceFixture` (a fresh `%TEMP%` folder with named
    files, folders, a shortcut, and fixed sizes and times),
    `Set-FixtureReadOnly`, `Hold-FixtureOpen`, `Get-RecycleBinItem` (Shell
    `NameSpace(10)`, matching only the fixture's paths), and
    `Clear-FixtureRecycleBin` (never a user's bin).
  - Rows: F1-F5, U5-U7, X7-X9, Delete stress spaces, W3c-W3g, L2, L5, L6, P25,
    P10, and B29's read-only config case.
  - Done when: a file deleted through DeskSpace is found by name in the bin
    and restored, and every helper refuses a path outside its fixture.

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

## V. Human test compendium

Desired human checks for built, unit-tested work. They do not block
implementation: a task lands, its check joins this list, and work continues.
The R7 pass works through the list when a person has the screen; a release
still waits for the rows it covers. Check with a scratch `--config`
(`./scripts/dev.ps1`), with `--on-top` over a high-contrast window for
anything visual, in light and dark mode where it matters. Delete a row when
it passes (say so in the commit); when only part passes, cut that part from
the row and append it, dated, to the last section of
`archive/ROADMAP-progress-log.md`; when it fails, replace it with a task or a
**Found while working** line. The task text for each id is in
`archive/TASKS-done.md`.

**First session (Windows 11, 100 %, Transparency effects on, scratch config,
about 20-30 min).** W2, W3b-W3g, W5, W6, and W7, in that order, on section
W's hand-made lens over a throwaway folder. W27, W35, W39, W42-W45, and B37
are the second lens session; a row that needs a setup this machine lacks
(Windows 10, 150 %, transparency off) is recorded as not covered and left
for a machine that has it.

| Id | What to check |
|----|---------------|
| Drawn shadows (part script) (D15) | Windows 11 with "Transparency effects" on, at 100 % and 150 % (O2 and O3 ran at 125 %), over Acrylic and Blur as well as Solid: the shadow reads as a soft shadow, not a box. With "Transparency effects" off, Solid at 16 px shows a drawn shadow while 4 and 8 px keep DWM's own; Windows 10 casts one at every radius. On a machine with a desktop host, `--placement desktop-*` casts none (O2 could not cover that here). Roll, Fly, Bounce, and CRT move and crop the shadow with the space; Fade, Dissolve, Gradient, Pixelate, and Burn fade it out to nothing with no ghost box left. Settings > Appearance: "Show shadow" greys "Shadow size" and "Shadow opacity"; Size (2–48 px) and Opacity (0–100 %) preview live and Cancel restores. `--debug` → Windows: "Outline every shadow window" draws a 1 px magenta box around each one, "Hide every shadow" hides them for the session, the "Shadow settings" box edits and saves the three settings (reflected in an open Settings window, kept on Cancel), and the Windows table lists each `DeskSpaceShadow` below its space. The structure and the pixels (one per space, z-order, rect, ex-styles, click-through, Clear/desktop/layer, show_shadow off, a per-space size, half the setting beside and three quarters below, both falloffs to 0, the drop band, the corner, opacity 0, and the Solid cut-out) passed O2 and O3 on 2026-10-27 at 125 % on the composition path (`archive/ROADMAP-progress-log.md`). |
| Animation refactor | The 2026-10-19 animation and painter refactors change no pixels, so check that everything looks as before, at 100 % and 150 %, on Acrylic and Solid: every effect on hide, show, Delete, and Fold, and Gradient, Roll, and Fly from each edge (Automatic: Roll and Gradient from the top, Fly to the nearest edge); Pixelate, Dissolve, and Burn at detail 1 and 10; the debug window's preview on one space and on all, pressed twice mid-way, its Kind box named Startup, Delete, Fold, and slow motion; exit plays every space out in reverse reading order. Items, labels, check boxes, the focus rectangle, the rename box, the lock glyph, and the settings highlight sit where they did; the drag image, the debug badge on the window icon, and the zoom pop-up look as before. |
| G1-G3 (part script) (D26) | With `--on-top` at 100 % and 150 %: open Settings on a space, then click other spaces in the scopes pane. Each outline fades in quickly, holds about a second, and fades out, with no flash and no outline left behind. Click a second space mid-fade: the first fades out while the second fades in. Fold and unfold a space while it is highlighted, and hide all spaces mid-fade; nothing sticks. With Windows' "Animation effects" off, it switches as before. With no highlight showing, idle CPU is as before. |
| G4, G5 (D26) | In light and dark at 100 % and 150 %, on every page of Global, a space, and a lens: each heading's fields sit on one rounded card with hairlines between rows. Labels, check boxes, sliders and their marks, swatches, text boxes, and drop-downs show the card's color behind them, with no body-colored boxes. Notes under a field are grey and sit close under it. Disabled rows still grey out. Switching dark mode while the window is open repaints the cards. Tab order is as before. |
| G6 (D26) | Appearance shows Material, Border, Shadow, and Items in that order; Colors shows Colors, Border, Selection, and Themes; General shows Startup, Desktop, Look, Space icons, and Updates; New spaces opens with "Everything not set here follows the Global appearance and layout." and its locked-space note sits under "Start new spaces locked". The materials note sits under "Background material". Every setting still previews, applies, and cancels as before. |
| Page switching | In dark and light, at 100 % and 150 %: click through every page of Global and of a space, by mouse and with the arrow keys. Each page appears in one step, with no blank frame or flash of the window body and no card drawn before its controls; a page taller than the window still starts at its top. With `--debug`, the console shows a "settings: switched page" line per switch; note its elapsed_us before and after this change if you can. Open a space's Shortcuts page, switch away before its icons load, then back: the icons are there. Edit a field, switch pages, then Cancel and OK: both still work, and Settings reopens on the last page. |
| S12 (part script) (D16, D26) | On a space's page, at 100 % and 150 %, in light and dark: double-click the name: an edit opens over it with the name selected. Type, then Enter: the top line, the scopes pane, and the space on the desktop show the new name, and the window stays open. Esc in the edit keeps the old name and the window open, and neither key presses OK or Cancel. Click elsewhere in the window, or press OK, while editing: the typed name is kept. Clearing a lens's name puts its default title back. Double-click the icon, once with the edit closed and once with it open: the picker opens over the settings window each time, an open edit stays open behind it and has the focus again afterwards, and a choice shows at once on the top line, in the scopes pane, and on the desktop. While the picker is open, typing goes only to the picker, and after it closes other apps get their keys (nothing stays captured). A single click on the icon does nothing; the cursor over it is a hand and its tooltip says "Double-click to change the icon", dark in dark mode. A space with no icon: double-click the +, pick one, and it replaces the +. With an icon set still downloading, the picker opens once it arrives, while the page shows. Cancel afterwards keeps both changes, and Ctrl+Z undoes the rename. Global's top line does not edit and has no icon. |
| S11 (D16, D26) | In light and dark, at 100 % and 150 %: a space with an icon shows it in the scopes pane in place of its kind's symbol (decided 2026-11-05: the icon replaces the symbol), and on its page's top line left of its name, aligned with the text. A space without an icon keeps its kind's symbol in the scopes pane, laid out as before, and shows an outlined + on the top line. A dimmed scope's icon is dimmed. The top-line icon takes no click yet (S12). Global's top line has no icon and its title has not moved. Change a space's icon from the desktop while Settings is open: both places follow; an icon set that finishes downloading while Settings is open fills in. Switch dark mode: the icons redraw in the new text color. |
| Rail groups | At 100 % and 150 %, in light and dark: Global's pane shows Look, Fonts, New items, DeskSpace, and Animations, each title with a caret pointing down, and a space shows This space (View, Shortcuts) then Look, a lens This space (Lens, View, Shortcuts) then Look. Switching from Global's General to a space opens its first page. Click a title: its pages hide and the caret points right; click again: they return. The click never selects the title or changes the page. Collapse the group of the page shown: the page stays, and no row is selected until you pick one. Switch scopes: Look stays collapsed if it was. With the arrow keys, Up and Down step over titles in both directions (Up from Startup reaches About), and moving into a collapsed group opens it at its first entry going down and its last going up. Reopen Settings: every group is open again. |
| Dark scroll bars | In dark mode, at 100 % and 150 %: every scroll bar is dark, with no white track. Settings: a page taller than the window (G10), the scopes pane with enough spaces to scroll, and the open list of a font family drop-down. Debug window: the section pane, the section list, a long readout (Model or Diagnostics), the console, and an open family list. The installed-apps picker's list and the icon picker's set drop-down. Switch to light mode with each window open: every bar turns light again. The debug readouts' borders still look right in both modes. |
| G10 (part script) (D26) | At 100 % and 150 %, in light and dark, on Appearance, Colors, and New spaces (at 150 % on a 1080p screen they are taller than the window): a scrollbar shows while the page does not fit; dragging it, its arrows, the wheel, and PageUp/PageDown/Home/End (with a check box or button focused) reach every group; the cards and hairlines scroll with the controls, with no card or seam left behind; switching pages starts the next page at the top; a short page has no scrollbar; the OK/Cancel/Apply bar is always on screen. Scroll with the pointer passing over drop-downs and sliders: none of them changes, and an open drop-down's list still scrolls itself. With a drop-down, a text box, or a slider focused, PageUp/PageDown/Home/End still work that control as before. |
| Font pages (F-68) | At 100 % and 150 %, in light and dark: Global lists Spaces and Windows under Fonts; a space's scopes do not. The pane shows Look, Fonts, New items, DeskSpace, and Animations for Global, and Look and This space for a space or lens. Each page has a card per role with Family, Size, and Weight, and both scroll (G10) where the screen is short. Space fonts: give Space title another family, size, and weight; every space's title bar follows at once, and so do Item label, Details (Details, Tiles, Content), Placeholder, and Zoom. Cancel puts every change back; OK or Apply keeps it, and a restart still shows it (the config gains a `[typography.<role>]` table only for changed roles). Window fonts: change Body and Heading 1, press OK, reopen Settings: it uses them, as do the Debug window, the pickers, and the shortcut dialog. Each page's "Reset to defaults" resets only its own roles. Hand-edit a config to a size of 12.5 and a family that is not installed: both drop-downs show those values, and picking another entry shows the right one. With `--debug`, apply a session font in the Debug box, then change the same role in Settings: the Debug override stays in force until "Reset to saved fonts". |
| G11, G12 (F-68) | With `--debug`, the Typography section lists every role. Scroll the section with the wheel, with the pointer passing over the drop-downs: the page scrolls and no value changes; an open drop-down's list still scrolls itself. Change Body to another family and size: nothing changes until "Apply fonts", which lights up and the status line says there are unapplied changes. Press it: the Debug window takes the new font in place, still on Typography at the same scroll, with no flicker of a closed window; reopen Settings and a picker, and each follows. Arrow through an open family list: nothing applies until Apply fonts. Give Space title another family and apply: every space's title bar follows at once, and so do item labels, Details cells, and placeholders when their roles change; the label-size setting still scales item labels. Choose a size at either end of the list; text stays inside its row and window. "Reset to saved fonts" returns every role to the saved fonts, and the status line says so. Switch Language to the pseudo one: both sample lines gain accents and brackets, and Settings and the Debug window still lay out. |
| B44 (part script) | Release build, scratch config, with the icon set downloaded: F2 on a space and click its icon. The picker opens within about half a second with its first page of icons drawn; scrolling to the end of the grid and back stays smooth; a search draws its results within a moment of each key (the remaining per-key cost is B47 and B48). With five or more spaces that have icons, Settings opens with their icons in the scopes pane and header at once. At 150 % the icons are sharp, and after Windows switches between dark and light they redraw in the new color. |
| S9, S10 (part script) (D16) | On the desktop at 100 % and 150 %: outside a rename, the icon shows no hand and a click does what the title bar does. F2 on a space, then hover the icon: hand and hover fill. A click opens the picker in front, with the rename still showing behind it. Search "pho", pick an icon, then OK: the icon changes at once, and you can type more of the name and press Enter. Esc in the picker changes nothing. "Automatic" and "No icon" work; a space without an icon shows the placeholder. Clicking in the picker never ends the rename. With downloads declined, the picker opens with the note and its two buttons. |
| Flames follow | With Startup and Delete set to Burn at 1000 ms (or `--debug` at ¼×), change a space's rect while it burns: move or resize it in Settings and Apply mid-hide, and change the display layout mid-show. The flames stay on the space's burning edge at its new place, never left where it was; a resized space's flames stand on its new edge. No flame window is left after the burn ends. |
| Frame clock | On a 60 Hz and a 144 Hz display, and on a space moved to a second monitor with another refresh: Roll, Fade, Fly, Bounce, CRT, and Burn on hide and show at 1000 ms are smooth, with no hitch or jump; `--debug` at ⅒× speed is smooth too; hiding during the startup stagger, and the hotkey twice mid-animation, behave as before. With no animation running, Task Manager shows no "frame clock" thread busy and idle CPU is as before. Lock the session mid-animation: it finishes, and the app stays responsive after unlocking. |
| No DWM shadow (part script) | Windows 11 with "Transparency effects" on, Acrylic, Blur, and Solid at 4 px and 8 px corners, at 100 % and 150 %: a space at rest has smooth rounded corners with no shadow; Fade, Dissolve, Gradient, Pixelate, and Burn on hide, show, and Delete leave no ghost box or outline at any point, and nothing pops off at the end; Roll rolls to a 1 px line with nothing left around it; CRT and Fly look as before. With "Transparency effects" off, spaces keep DWM's corners as before. |
| Flames under apps (part script) | Without `--on-top`, with a space partly covered by an app window (an editor, a browser): Burn on hide, show, and Delete draws its flames over the space but under that app, never over it, including where the flames rise past the space's top edge; an uncovered space's flames still show in full. Switching to another app mid-burn keeps the flames under it. |
| Window icons (part script) | At 100 % and 150 %, with the taskbar shown and auto-hidden: the settings window's taskbar button and Alt+Tab entry show the DeskSpace icon, sharp, and its title bar the small one; `--debug`'s window shows the same icon with a white bug on an amber disc in the bottom-right corner, in all three places, also on the first start of `./scripts/dev.ps1 -Settings --debug`, when both open during startup. Move the debug window to a monitor with another DPI: the icon still shows. The tray icon looks as before. |
| Border settings (part script) | Settings > Appearance: "Border thickness" from 1 to 8 px thickens the border evenly on every side, at 100 % and 150 %, on Acrylic with Windows corners and on Clear at 16 px; the slider greys out with "Show window border" off. Settings > Colors: the border is white in dark mode and black in light mode, also with custom text colors. "Border color" set to Windows accent colors it with the accent; set to Custom, it starts from the border on screen and "Custom border color" changes it live. "Border opacity" works with all three, and Cancel undoes every change. "Use Windows accent color for the selection" leaves the border alone. A customized space keeps its own border while Global changes. Old configs load with a 1 px border, as before. |
| Painted border (part script) | With "Show window border" on, at 100 % and 150 %, on Acrylic, Blur, Solid, and Clear, with Square, Small, and Windows corners: the border sits on the window's edge and follows its rounded corners with no gap or cut-off corner, as close to DWM's old border as you can tell. Fade, Dissolve, Burn, and Pixelate on Startup and Delete: the border fades, dissolves, burns, and pixelates with the space and no outline is left. With the border off, none shows. Windows 10: a square border. The zoom pop-up looks as before. |
| Blank space at startup | With Startup set to Roll, "One after another", and `--debug`, start DeskSpace ten times: the debug window opens first, then the spaces roll in one after another, as they do without `--debug`, and every space shows its title, tint, and items, with none left blank (blur only) until hovered. The same with Startup set to None. Restart right after signing in and after a display or GPU driver change too: with `--debug` at Debug level, a "direct2d target lost" line is followed by a full repaint of that window. |
| F-61 extras (part script) | With `--debug`: the Spaces table follows moves, folds, locks, and renames within a second; "Clear the icon cache" redraws every icon without gaps; "Save now" writes the file at once; Input → "Log events" logs one line per click, key, wheel, and drop and none for pointer moves; Input → State follows a selection and a hover; each "Mark the log" adds the next numbered line; Simulate → "Windows Animation effects changed" re-reads the switch. |
| X20 (part script) (F-61) | Debug → Simulate: "Windows theme changed" repaints the spaces and the settings window; "Explorer restarted" re-adds the tray icon and, with takeover on, re-hides Explorer's icons; with the config read-only, moving a space shows B29's one "could not save" box, and after Exit the file is writable again; "Crash now" with takeover on ends DeskSpace and the watchdog brings Explorer's icons back within a few seconds, and a debug build's console shows the panic line. |
| X19 (part script) (F-61) | Debug → Takeover, with takeover on: the toggle off brings Explorer's icons back at once and they stay back past the 5-second health timer; on hides them and draws the layer; Re-sync picks up a file added to the Desktop folder; State shows the watchdog running while takeover is on; the saved config's `take_over_desktop` is unchanged afterwards. |
| X18 (F-61) | Debug → Animations: every effect, played on a space at ¼×, leaves and comes back in place; pressing Play again mid-way turns it around; hiding the spaces from the tray mid-preview keeps them hidden; the other spaces and the model do not change. |
| Play on all spaces (F-61) | Debug → Animations → "Play on all spaces" plays the chosen kind on every space at once at the chosen speed, each leaving and coming back in place; pressing it again mid-way turns them all around; with the spaces hidden from the tray, nothing shows. |
| X16 (F-61) | Debug → System: switching Windows between dark and light, changing the accent, and turning off transparency each show within a second; selecting an item turns on Watch selection; a rename turns on Editing; Hotkeys says which bindings registered. |
| X14 (part script) (F-61) | Debug → Windows: "Keep spaces above other windows" lifts the spaces above Notepad and off drops them back to the desktop; the Windows table lists every space, the tray's controller window, and the debug and settings windows in z-order, and follows a space moved to another monitor; the Monitors table matches Windows' display settings. |
| X12 (F-61) | Debug → Diagnostics: the Build, Model, Resources, and Updates values look right and Resources ticks once a second; creating and deleting spaces moves the counts; "Copy diagnostics" pastes into Notepad as a readable report. |
| X10 (part script) (F-61) | Debug → Config: the Configuration and Storage readouts match `--config` ("scratch", or "real" without it); "Write the configuration to the log" shows every space in the console; both Open buttons open the folder in Explorer. |
| X9 (v0.3) (part script) (F-61) | Debug → Spaces: Unfold, Unlock, Refresh, and Prune each do what they say on folded, locked, and stale spaces; a 1000-item stress space opens and scrolls smoothly, and "Delete all spaces" removes it. |
| Delete stress spaces (v0.3) (part script) (D14) | Debug → Spaces: make a 50 and a 200 stress space and rename a third; Delete stress spaces asks "Delete 2 stress spaces?" with 250 items, then removes those two with their shortcuts in the Recycle Bin and none on the desktop; the renamed one stays; the button greys once none are left. |
| X8 (v0.3) (part script) (F-61) | Debug → Spaces → "Create test spaces" opens four spaces in a row beside the existing ones, titled Test: Apps, Tools, Places, and Web; every shortcut opens what it names (the Tools ones show the test text); a second press makes four more without overlapping; then "Delete all spaces" removes them and leaves nothing on the desktop. |
| X7 (v0.3) (part script) (F-61) | With three spaces holding dropped files and one Shift-dropped file, Debug → Spaces → "Delete all spaces" asks once with the right counts and the scratch config's path; Cancel changes nothing; OK plays each space's Delete animation; the dropped files are in the Recycle Bin, the Shift-dropped file is where it was, and nothing new is on the desktop. Started without `--config`, the question shows the real-configuration warning (press Cancel). |
| X5 (part script) (F-61) | With `--debug`, the rail lists "Windows" with a "Show spaces" check box that hides and shows the spaces and follows the tray's Hide/Show within a second; closing the window and choosing the tray's "Debug…" brings it back with the earlier log; without `--debug` the tray has no "Debug…"; the window themes dark and light, resizes (the console grows), and stops at its minimum size; Tab moves between controls. |
| X4 (F-61) | With `--debug`, the window opens with the console already holding the startup lines; moving a space adds lines live; Level Trace shows trace lines and Info stops new debug ones; Filter, Pause and resume, Auto-scroll, Clear, Copy (paste into Notepad), and "Save log…" work; a `-Release` build shows the log with no console window; without `--debug` nothing opens and stderr logs as before. |
| C2 (F-29) | With "Use Windows accent" on, the selection takes the accent (and the border, with "Border color" set to Windows accent), and they follow it live when it changes in Windows Settings, also with "Automatically pick an accent color from my background" and a wallpaper slideshow; switching wallpapers with an unchanged accent causes no flicker. |
| C3 (F-30) | Switching "Settings and menus" between Dark and Light repaints the open settings window and the next popup menu. The selection bar and marked dots, and the color picker's selection ring, take the Windows accent (a lighter shade in dark mode, a darker one in light mode, readable in both) and follow an accent change in Windows Settings while the window is open. |
| C4 (part script) (F-34) | "Start DeskSpace when you sign in" adds and removes the `Run` value, and DeskSpace starts at sign-in. |
| C5 (F-48) | The "Item preset" dropdown shows both entries; "Custom" enables the Layout fields and the icon size takes effect; "Match Explorer's display" greys them. |
| C7 (part script) (F-18) | Each hotkey fires once bound; a conflicting binding is reported beside its field and the window stays open; clearing a binding stops it. |
| C8 (script) (F-17) | A space created from the space menu, the tray, and the hotkey uses the configured title, size, lock state, icon, and view (set the New spaces page's Show as to List with the status bar on); a locked new space keeps its size and scrolls. |
| E1, E2 (part script) | `deskspace.exe` shows the icon in Explorer's small, medium, and extra-large views and the version under Properties > Details; the tray, settings title bar, and Shift+right-click desktop verb show the icon at 100 % and 150 %. Run with `./scripts/dev.ps1 -RealConfig` so the desktop verb is the installed build's. |
| E5-E7, E10-E12 (v0.3) (part script) | Covered by the R12 dry run (installer, uninstall, the watchdog in the zip, update from one pre-release to the next). |
| F-45 automatic install (v0.3) (part script) | Installed build with a feed: turn on both update toggles; after a check finds a newer release, the tray shows "Restart to update to X"; 10 minutes without input (no full-screen app) installs silently and DeskSpace comes back with the same spaces, with takeover on the desktop icons are never left blank; the tray entry installs at once with a progress window; a portable copy greys the install toggle. |
| Update install failure (v0.3) (part script) | Installed build with a feed and a newer release: in PowerShell set `$env:TEMP` and `$env:TMP` to a missing drive (`Z:\none`), start `%LocalAppData%\DeskSpace.App\deskspace.exe --hidden`, then "Check for updates…" → "Update and restart": DeskSpace quits, comes back hidden on the old version with the same spaces, and a "DeskSpace was not updated" box in front of other windows names the error. With takeover on, the desktop icons are never left blank. |
| F1-F5 (v0.3) (part script) | F2 with no item selected renames the space title, with one selected the file; Alt+Enter opens Properties, or Settings with nothing selected; Enter opens the item; Esc clears the selection and still closes an open menu; Shift+Delete bypasses the Recycle Bin. With takeover on, Delete and Shift+Delete on one and on several selected loose desktop icons do the same, and the icons leave the layer at once. |
| F9 (script) | With "Snap resized spaces to whole columns and rows" off, a resize keeps the dragged size (still snapped to `snap`). |
| F14 (part script) (F-51) | The color picker opens from a Colors-page swatch and a space's color menu; on Windows 11 22H2+ it is acrylic, else opaque, rounded and bordered in both modes; the square, hue, and opacity sliders drag smoothly; a swatch click selects and a double-click applies; typing `#3B82F6`, `rgb(59, 130, 246)`, or `217°, 91%, 60%` switches the radio button and the color, invalid text shows a red message and is dropped on leaving the box; Ctrl+V pastes, the copy button and Ctrl+C copy; the eyedropper samples any screen pixel live, a click keeps it and Esc or a right click cancels; Current goes back; the title strip drags it; it is sharp at 100 % and 150 % and after moving to another monitor; recent colors survive a restart and a settings Cancel; Apply, Cancel, the X, Enter, and Esc behave and the settings window does not hang. |
| G1, G3 (part script) | On the desktop shell: typing selects by prefix and the buffer expires; Home/End/PageUp/PageDown scroll; arrows move the selection, Shift extends, Ctrl moves only the dotted focus rectangle and scrolls it into view. |
| G5a (v0.3) (part script) | Right-drag an item out of a space onto the desktop and into an Explorer folder: the drag shows the same translucent icon image a left drag does (not the shell's large opaque one), also for a multi-item selection and from the taken-over desktop; the shell's Move / Copy / Create shortcut menu appears and each choice works; a right click without moving still opens the item menu. |
| G5b (v0.3) (part script) | Right-drag a file from Explorer onto a space: DeskSpace's Move here / Copy here / Show here without moving / Cancel menu appears; each entry does what it says; the source file is left alone on Copy, Show here, and Cancel; a left drag still moves and Shift still keeps in place. |
| G5c (v0.3) (part script) | Right-drag a file and a `.lnk` from Explorer onto a space and choose "Create shortcuts here": the space gets a working shortcut to the file and a copy of the `.lnk`, and both originals stay; the entry sits between "Copy here" and "Show here without moving"; the same on the taken-over desktop opens a new space holding them. |
| Right-drop greying (v0.3) | Right-drag onto a space from a source that offers only copy (a file on a read-only share or a zip's contents): "Move here" is greyed, "Copy here" and "Show here without moving" work; from Explorer every entry is enabled. |
| H5, H4 (part script) (F-46) | Panes, hairlines, dimmed rows, and keyboard movement in both modes; a space is greyed until customized, then edits apply to it alone and Cancel restores; the switch off and on restores custom values; a menu edit with the window open survives Cancel; an old config with per-space `icon_size`/`tint` opens customized; the highlight outline is visible on Clear and Solid, and on Acrylic with Windows corners (border on and off, 100 % and 150 %) its corners are round, following the window's |
| I8 (v0.3) (script) | With takeover on, creating, renaming, and deleting a desktop file updates the layer within about 150 ms; so does editing one in place whose icon is its content (save a new picture over a desktop `.png`, or re-point a desktop shortcut to another app), with no restart. |
| W3c (v0.3) (part script) (F-07) | Shift-drag a file from Explorer onto a hand-made lens (section W): the cursor shows "no" and nothing moves. Right-drag onto a lens: "Show here without moving" is grey. On a normal space, Shift and the right-drop menu behave as before. |
| W3d (v0.3) (part script) (F-07) | Drag an item from a hand-made lens (section W) onto a normal space: the file leaves the lens's folder, the space lists it, and the lens no longer shows it. Delete that space: the file goes to the desktop, not back to the folder (D3). |
| W3e (v0.3) (part script) (F-07) | In Explorer, copy a file and paste it on a hand-made lens (section W; click it, then Ctrl+V): a copy lands in the folder. Cut and paste: it moves. Cut an item from a normal space and paste it on a lens: it moves into the folder and leaves the space. A paste on a normal space works as before. |
| W3f (v0.3) (part script) (F-07) | Drop a Start-menu app on a hand-made lens (section W): a working `.lnk` is in the folder and the lens shows it. "New shortcut ▸ Paste shortcut" and the custom shortcut dialog on a lens do the same. On a normal space, shortcuts work as before. |
| W3g (v0.3) (part script) (F-07) | Select an item on a hand-made lens (section W) and press Delete: the file is in the Recycle Bin and the lens drops it. On the settings window's Shortcuts page, the X on a lens item says the file goes to the Recycle Bin, and it does. |
| W7 (v0.3) (part script) (F-07) | Create a lens with "New lens…" from the space menu, the tray menu, and the desktop layer's menu (takeover on): each asks for a folder and opens a lens titled with the folder's name beside the top row of spaces, filled within a moment. Picking DeskSpace's storage folder shows a message and creates nothing. Cancelling creates nothing. |
| New space placement (script) (D10) | With a few spaces in a row at the top of a monitor, New space from the space menu, the tray, the desktop menu, and the hotkey each opens to the right of the row, its top in line, with the same gutter as the others; with the row full to the screen edge it goes to the next row or under all of them; it never covers another space; on a second monitor it opens on the monitor of the click or the pointer; on an empty monitor it opens near the top-left corner; on a 150 % monitor the gutter looks the same. |
| Desktop menu (v0.3) (part script) (F-14) | With takeover on, right-click an empty spot of the desktop: New space, DeskSpace settings…, and Hide/Show spaces sit above Explorer's desktop menu; each DeskSpace entry works; View, Sort by, Refresh, New › Text Document, Paste, Display settings, and Personalize work, and the layer shows the result; the Menu key and Shift+F10 with nothing selected open the same menu; a right-click on a loose icon still shows its item menu, and on a space the space menu. Run elevated once too: the menu still shows New, Display settings, and Personalize below the DeskSpace entries, with masked icons and no separator dangling under Hide spaces; with `-Verbosity 1` any fallback logs a warning naming the step that failed. |
| C9 (part script) (F-52) | With the app running (not elevated; `./scripts/dev.ps1 -RealConfig` so the entries are real), right-click an empty spot of the desktop: "New space" and "DeskSpace Settings" sit above Explorer's entries. "New space" opens a space on the pointer's monitor; "DeskSpace Settings" opens the running instance's window instead of starting a second copy. With the spaces hidden, "Show DeskSpace spaces" appears, and hiding/showing updates the menu. After a clean exit all three are gone. |
| P31 (part script) | With the installed DeskSpace running, run `./scripts/dev.ps1` and exit it. The desktop menu still shows the installed build's entries, and they still work. |
| P36 (v0.3) (part script) (one instance) | Start DeskSpace, hide its spaces, and start it again from the Start menu: the spaces show and no second tray icon appears; Task Manager lists one DeskSpace process. With the installed build running, `./scripts/dev.ps1` still starts beside it, and each one's Show spaces reaches only its own spaces. Installing an update over a running instance still relaunches it (with R12). |
| Menu at the click (v0.3) (part script) | With takeover on and "Auto arrange" off, right-click an empty spot far from other icons: Paste (after copying a file) and New › Text Document each put the new icon at the click, not in the next free slot; a space's item menu commands (Open, Properties, Delete) work as before. If Explorer ignores the point, record it here. |
| New on the desktop (v0.3) (part script) | With takeover on, right-click an empty spot of the desktop: New › Folder, and New › Text Document with extensions shown and hidden, each make the item and open DeskSpace's rename box on it, with "New folder" (or the base name) selected; typing a name and Enter names it; Esc keeps "New folder"; a second New › Folder gives "New folder (2)" its own box. Paste and View › Refresh open no box. With takeover off (`--layer` only), Explorer's own box shows, as before. |
| Layer rename (v0.3) (part script) | With takeover on, a loose desktop icon renames like a space item: F2, a slow second click, and "Rename" on its right-click menu each open the rename box on the icon (the base name selected when extensions show); Enter renames the file, which stays selected and in place; Esc and a click elsewhere behave as in a space; Ctrl+Z in Explorer undoes it. |
| Layer presses (part script) | With takeover on, top-level and with `--placement desktop-*`: Ctrl+click and Shift+click on loose desktop icons add to the selection; a plain click on empty layer space clears it; a click on another app keeps it; a click on the layer during a rename commits the rename. |
| Taskbar repair | With takeover on, use the PC normally with `--debug` open for a while (full-screen video, games, Win+D, switching monitors). If the log ever says "a taskbar was under the desktop layer; sank the layer below it", the taskbar takes clicks again within 5 seconds and the spaces still take clicks and drops; record the logged foreground window in `CONTEXT.md` section 10. "sinking the layer did not help" means the repair failed: replace this row with a task for one layer per monitor. |
| Hosted layer order (script) | With `--placement desktop-*` and takeover off, turn takeover on from Settings: the loose desktop icons appear and every hosted space stays visible above them and takes clicks; the same after a restart with takeover on. |
| L2 (v0.3) (part script) (F-50) | Not elevated, `./scripts/dev.ps1 -Verbosity 1`: a Start "Pinned" app, an "All apps" app, a Store app, and a taskbar app each land in a space as a working shortcut and stay in Start; Alt-dragging an Explorer file makes a shortcut and leaves the file; dropping a `.lnk` with Alt copies it; a plain drag still moves a file and Shift keeps it in place. |
| L5 (v0.3) (part script) (F-54) | Copy a file and a folder in Explorer; "New shortcut ▸ Paste shortcut" in a space (and from the Shortcuts page's + tile) makes two shortcuts and the originals stay; with nothing on the clipboard the entry is grey. |
| L6 (v0.3) (part script) (F-55) | "Add files…" on a space lists the standard Open dialog on the Desktop; picking two files moves them into that space (as a drop does), and Cancel adds nothing. |
| Drag and drop (v0.3) (part script) (F-36, F-37) | The move/copy cursor; no `- Copy` left on the desktop after a drag out; the drag image over Explorer and over spaces; the in-space ghost. |
| Selection (part script) (F-41, F-42) | A space click focuses the desktop and clears its selection; check boxes in both Explorer modes; rubber-band and Ctrl+A. |
| Clipboard and rename (v0.3) (part script) (F-38, F-02) | Cut, copy, and paste, with cut ghosting and its clearing when another app takes the clipboard; F2 item rename. |
| Display (F-25, F-43) | Compressed/encrypted label colors; info tips; jumbo icons; per-monitor DPI on a mixed-DPI setup. |
| Mouse modifiers (part script) | Ctrl+click and Shift+click in a space toggle and extend the selection without first using the settings window or a menu. (Ctrl and Shift now come from the mouse message, not `GetKeyState`.) |
| F13 (part script) (F-25) | Under the default preset, Ctrl+wheel stops at 16 and 256 px; under Custom it changes the spaces (and, with takeover, the layer's icons) but not Explorer's; a space with its own Custom look keeps its size either way. |
| F-57 zoom pop-up | Still to check: the look in light mode and on the Blur, Solid, and Clear backdrops (Clear shows as Acrylic); the scale at 150 %, where the percentage should read 100 % at "Medium icons". |
| F15 (script) | Still to check: the icon-size slider and the Match Explorer preset leave every space's size alone; "Fit to contents" then fits the space to its items; Cancel after a slider preview leaves a never-resized space growing with new items again; moving a space to a monitor with another DPI still scales it (title bar included). |
| B28c (script) | Lock, fold, sort, and "Fit to contents" (space menu and the settings button) each repaint the space at once and survive a restart. |
| B29 (script) | Make the scratch config read-only (`attrib +r`) and move a space: one "DeskSpace could not save" box appears; a second move shows none. Clear the attribute and move again: the file is saved. Closing DeskSpace while it is read-only shows no box. |
| B30 (part script) | Overlap two spaces with different icon sizes, then sort or arrange the lower one: its icons land on its own grid. |
| Desktop drop (v0.3) (part script) (D7) | With takeover on, drag onto an empty spot of the desktop: a Start "Pinned" app, a Store app, an Explorer file (plain, Shift, Alt, and a right drag), and an item out of a space. Each shows a move/copy/link cursor, not "no", and opens a new space at the drop point holding it. Drop next to a space and near the taskbar: the new space keeps a gutter from both and never overlaps. Right drag, then Cancel: no space is left behind. |
| Spaces above the layer (v0.3) (part script) | With takeover on, press Win+D twice, switch tasks a few times, and hide and show the spaces: each space still takes clicks, the wheel, and a drag from Explorer or Start (no "no" cursor). |
| R3 | Nothing should change: shortcut arrows and other overlays look as before at every icon size; hover, click, double-click, wheel, and right-click in a space and on the desktop layer; the tray icon's click and menu; the settings window's controls and dark-mode colors; the color picker's swatches, sliders, hover, OK, Cancel, and Custom toggle. |
| B23 (part script) | Key routing should not change: the F1-F5 and G1, G3 rows pass with the same keys, and Ctrl+Plus, Ctrl+Minus, and Ctrl+0 still zoom. |
| B13 | The settings window, the custom shortcut dialog, and the installed-apps picker look as before at 100 % and 150 % in light and dark mode: every control keeps its size, font, place, colors, and tab order. |
| B14 | Every settings page looks as before at 100 % and 150 % in light and dark mode: each field kind (heading, note, choice, toggle, slider with and without marks, color, text, button, space preview) keeps its size, label, spacing, and control position. |
| B26 | The settings window opens, switches scopes and pages, and previews as before; the rail, scopes, header, page controls, and command bar stay in place at 100 % and 150 %. |
| R17 (part script) | Nothing should change: the F13 and F-57 rows pass (Ctrl+wheel, the zoom keys, and the pop-up's buttons, under both presets; the pop-up hides about 2 s after the last zoom, not while hovered), and so does B28c's row. |
| T4 (D12) | Rechecked after the rail fix: "Animations" reads as a semibold header with its pages (Startup, Delete, Fold since T15) indented under it; the "Windows settings…" button's caption fits; Detail shows a plain number. Then the rest: nothing clipped at 100 % and 150 %; each effect greys the options it does not use, Delete's "Same as startup" greys its options, "All at once" greys the delay; with Windows' "Animation effects" turned off while the window is open, every control greys and the note says why; the button opens the page with that switch (Windows 11 and 10); Cancel undoes an edit. |
| T6 | Nothing should change: spaces on every backdrop, the desktop layer, and a desktop-hosted space (`--placement desktop-*`) paint as before, title, items, border, highlight, and scroll thumb included (the paint path was split so animations can draw through a layer). |
| T8 (D12) | Each Startup effect, on hiding and showing (Roll, Fade, Pixelate, Gradient, Dissolve, Fly, CRT, Burn, Bounce), from the tray, the space menu, and the hotkey, on Acrylic, Blur, Solid, and Clear spaces at 100 % and 150 %: smooth, no flash of old content when a show starts, and the blur steps on at the start of a show and off at the end of a hide; the hotkey pressed twice quickly turns it around without a jump; Fly leaves by the nearest edge and never shows on a second monitor; a file dragged onto a hiding space is refused; with Windows' "Animation effects" off, hiding and showing are instant; a desktop-hosted space (`--placement desktop-*`) still hides and shows; checked at ¼× too (`--debug`, Animations → Speed), and with "Animate even when Windows' Animation effects is off" on, hiding and showing animate while Windows' switch is off (X17). |
| T19 (D12) | Bounce on Startup (show, a new space, startup) and Delete from each edge, on Acrylic and Solid at 100 % and 150 %: at 250 ms it plays for about half a second, drops in already moving, lands three times with two clearly shrinking rebounds, and settles exactly in place; it never shows on a second monitor or over the taskbar. Fly and Bounce: nothing flashes where the space was as it leaves, no outlined box shows as it crosses an edge, and the shadow is back once the whole space is on screen. |
| T9 (D12) | Fold and unfold from the space menu and a title double-click on each backdrop: the body rolls up into the title bar with its items in place, and rolls down showing them; a second double-click mid-way turns it around; Fold set to None folds at once; moving a space right after folding it is not fought by the roll; hiding a space mid-fold hides it folded (or unfolded) and shows it the same way. |
| T10 (D12) | Each Startup effect on a new space from the space menu, the tray, the hotkey, and a drop on the taken-over desktop: the space appears with it and no empty frame or early blur shows first; at startup with several spaces, "One after another" brings them in reading order at the set delay and "All at once" together; "Start with spaces hidden" shows nothing until shown; Fly from each edge and on a second monitor; hiding during the startup stagger hides every space, waiting ones included. |
| T11 (D12) | Each Delete effect, and "Same as startup", deleting from the space menu and from the settings header: the space plays it, then is gone, and clicks or drops on it while it plays do nothing; deleting a space that is still flying in (a drop on the taken-over desktop that adds nothing does this) turns it around; quitting mid-animation leaves no window behind; with the spaces hidden, a delete is instant. |
| T15 (part script) (D12) | Settings > Animations lists Startup, Delete, and Fold; a config last saved with the Hide and show page keeps that page's effect as Startup's; Startup's effect plays on hiding, showing, a new space, and startup (staggered only at startup). |
| T16 (D13) | Exit from the tray and from a space menu with several spaces: they leave one after another in reverse reading order (all together with "All at once"), then DeskSpace closes, within about 2 s; with takeover on, Explorer's icons come back only after the spaces have left; a second Exit mid-way closes at once; with the spaces hidden, Exit closes at once; signing out of Windows is not held up. |
| Debug animation settings (part script) (D14) | With `--debug`, Animations shows one "Animation settings" box below the preview controls, in light and dark mode, at 100 % and 150 %; choosing Startup, Delete, or Fold in Kind shows that kind's values at once, with the rows it or its effect does not use greyed (as on its settings page), and the window stays on Animations; scrolling the section leaves no stale pixels; every choice takes effect on the next play (Play on the space; Fold plays its effect out and back) and is in the config file at once; with the settings window open, its Animations pages show the change, and Cancel there keeps it. |
| Flat material (part script) | Fade, Pixelate, Gradient, Dissolve, and Burn on hide, show, and delete (and in the debug preview) on Acrylic, Blur, and Solid spaces with a border, with Windows' "Transparency effects" off and on: no outline stays around the fading space, and it comes back once a show ends; with the switch off, no dark box either, the dark fill coming back only once a show ends; with it on, see the T13 row; Roll, Fly, and CRT look as before. |
| T12 (part script) (D12) | Windows 11 with "Transparency effects" on: the log says "spaces use composition"; the R3 list passes unchanged with each backdrop (Acrylic and Blur blurred, Clear and Solid not), at 100 % and 150 %, corners and border as before, and the T8 row still passes, the blur stepping on and off as today. With the switch off at startup, the log names it and spaces look as before T12. Windows 10, `--placement desktop-*`, and `--click-through` change nothing. Idle CPU and GPU no higher than before. |
| T13 (D12) | Windows 11 confirmed 2026-10-18 (maintainer). Still to check: on Windows 10 the blur still steps; idle GPU no higher than before. |
| T17, T18 (part script) (D12) | Burn on Startup (hide, show, a new space, startup, exit) and on Delete, on each backdrop at 100 % and 150 %: the space burns from the bottom up along a ragged edge that glows, chars, and scorches ahead of itself, like paper; flames lick up from the edge and past the top and sides of the frame with no box or blur around them, and never show over another application (cover the space with a window and they stay behind it), while with `--on-top` they follow the space above other windows; Detail changes how ragged the edge is; reversing mid-burn (the hotkey twice) un-burns it; no flame window is left behind after it ends, after a delete, or after quitting mid-burn; a desktop-hosted space burns without flames. |
| U3 (v0.3) (part script) (D11) | Space menu and tray menu list Undo and Redo at the top, greyed when empty and named for the top action ("Undo Move space", "Redo Move space"); moving, resizing, reordering, sorting, fitting, locking, folding, renaming, creating, and deleting an empty space each undo and redo from both menus. |
| U4 (v0.3) (script) (D11) | Ctrl+Z after a space move puts it back, Ctrl+Y moves it again; with no space current, Ctrl+Z on the desktop still undoes an Explorer rename. |
| U5 (v0.3) (part script) (D11) | Drop a file into a space and undo (it is back where it was); delete a space with stored files, undo, then redo; rename an item and undo. |
| U6 (v0.3) (script) (D11) | Recycle a file in Explorer, restore it through a test harness or `--verbose` log, and confirm it is back under its old name. |
| U7 (v0.3) (part script) (D11) | Delete two items and undo (both return to the space); paste a copy, undo (it goes to the Recycle Bin), then redo; empty the Recycle Bin between a delete and its undo: the undo says it can't and changes nothing. Shift+Delete leaves Undo as it was. |
| W44 (v0.3) (part script) (D11) | Change a space's view (View menu, Ctrl+Shift+1..8, the status bar's buttons), sort a lens, toggle its hidden files, change its folder, and create a lens: each is one Undo entry named for it ("Undo Change view", "Undo Sort lens", "Undo Change lens view", "Undo Change folder", "Undo New lens"), Ctrl+Z and Ctrl+Y restore and reapply it from the menus too, and a lens brought back lists its folder again. A file added to or deleted from a lens's folder leaves "Undo Sort lens" working, unless the lens grew or shrank to fit it (its rect changed, so the entry is stale as for any space). |
| W8 (v0.3) (D17) | In light and dark mode, at 100 % and 150 %, on Windows 11 and 10: every scopes row shows its kind's glyph (Global, a space, a lens), the labels line up, a dimmed row's glyph is dimmed, and the selection covers the glyph. |
| W9 (v0.3) (D17) | Lenses on Downloads, Documents, and a plain folder show each folder's own icon left of the title; rename the folder away and the generic folder icon shows; F2's rename box starts after the icon; the same folded, at 100 % and 150 %, on every backdrop; a normal space's title bar is unchanged. |
| W11 (v0.3) (part script) (D17) | On a lens in the user's profile: each "Show as" entry retitles it at once, and Cancel puts the title back; Change folder… retitles it under Folder name and keeps a custom title; F2 on the title, then the Lens page shows Custom. |
| W12 (v0.3) (script) (D17) | Each View ▸ entry resizes only that space, on a normal space and a lens. "Settings icon size" gives back the look's size. Ctrl+wheel does not move a space with a fixed size. The view survives a restart. |
| W13 (v0.3) (script) (D17) | After clicking a space, Ctrl+Shift+1, 2, and 3 change it to Extra large, Large, and Medium icons; Ctrl+Shift+4 through 8 change nothing yet; with Explorer in front, Explorer's own Ctrl+Shift+1 still works. |
| W15 (v0.3) (part script) (D17) | Turning Sort by ▸ Keep sorted on sorts the space at once. With it on, a dropped file lands in order and a renamed item moves to its place; a drag inside does not rearrange, while a drag out still moves the file. Turned off, the space rearranges freely again and keeps the order it had. On a lens the entry is checked and greyed, and its order holds. |
| W16 (v0.3) (part script) (D17) | Small icons on a space and on a lens, at 100 % and 150 %: 16 px icons with one label line beside them, rows wrapping and scrolling down; click, Ctrl and Shift click, the marquee, the arrow keys, and type-to-select all hit the inline cells; F2's box sits over the label; drag out works, and a drag to rearrange reorders a normal space (not one kept sorted). Back in Medium icons, the hand arrangement is as it was. |
| W17 (v0.3) (part script) (D17) | List on a space and on a lens, at 100 % and 150 %: 16 px icons with one label line beside them, columns filling top to bottom and running left to right, the wheel scrolling sideways, Home/End/PageUp/PageDown and the arrow keys moving along the columns, and the scroll indicator along the bottom; click, Ctrl and Shift click, the marquee, and type-to-select hit the inline cells; F2's box sits over the label; drag out works, and a drag to rearrange reorders a normal space (not one kept sorted). Check the W16 row again in List. |
| W18 (v0.3) (D17) | A folder in Details beside the same folder in Explorer's Details: the same columns (Name, Date modified, Type, Size), values, alignment, date, and size format; Name takes the width left over; header clicks sort and reverse and the arrow follows; the header stays put while rows scroll; hover and selection fill the row and F2's box sits in Name; 100 % and 150 %; light and dark. |
| W19 (v0.3) (part script) (D17) | A new lens has the status bar and a new space does not; View ▸ Status bar toggles it; the counts follow a click, Ctrl+A, and the marquee, and the size shows for files only; both buttons switch the view and show which is on, and their tips read as Explorer's; folded, the bar is gone; every backdrop at 100 % and 150 %, light and dark. |
| W21 (v0.3) (D17) | Tiles beside Explorer's Tiles on the same folder, at 100 % and 150 %, in light and dark: 48 px icons with name, type, and size beside them, tiles wrapping into rows and scrolling down; click, Ctrl and Shift click, the marquee, the arrow keys, type-to-select, F2 over the name, and drag out. |
| W22 (v0.3) (D17) | Content beside Explorer's Content on the same folder, at 100 % and 150 %, in light and dark: one row per item with the icon and name at the left, "Date modified: …" and "Size: …" at the right, and Explorer's separator between rows; click, Ctrl and Shift click, the marquee, the arrow keys, type-to-select, F2 over the name, and drag out. |
| W23 (v0.3) (script) (D17) | Global's Lenses page sits after New spaces. Change each default (View, Status bar, Sort by, Order, Folders first, Show hidden files, Show as) and "New lens…" uses it; an existing lens keeps its own settings. |
| W25 (v0.3) (part script) (F-07) | Start with a lens on a share whose server is off (or a disconnected network drive): the other spaces draw and respond at once, and that lens shows "Folder not available". Make a junction into the storage folder of a `dev.ps1` scratch config (`mklink /J C:\Temp\into-store "%TEMP%\deskspace-dev\storage"`; drop a file on a space first so the folder exists). Change folder… to it: a message refuses it. Hand-edit a lens's path to it and restart: the lens shows "Folder not available" and lists nothing. |
| W42 (v0.3) (part script) (F-64) | In a Downloads lens, double-click a folder, then one inside it (and once with Enter): each shows in place, the selection clears, the list starts at the top, and the title reads `Downloads › A › B`. Narrow the lens until the title is too long: it is cut from the left (`… › B`). Drop, paste, New shortcut, and Delete there all act on `B`; Open folder opens `B`. A `.lnk` to a folder still opens Explorer. Delete `B` in Explorer while it is shown: "Folder not available" with `B`'s path. A junction to the storage folder inside the lens shows unavailable when browsed into. Change folder…, its undo, and a restart each return to the lens's own folder. F2 on the browsed lens's title edits the lens's own nickname, so its box shows `Downloads`, not the bar's `Downloads › A › B`: confirm that reads clearly. |
| W43 (v0.3) (part script) (F-64) | In a lens browsed two levels down, at 100 % and 150 %: the Back button sits left of the folder icon and goes up one level per click (a double-click goes up two, and never folds the space); Backspace, Alt+Left, Alt+Up, and the mouse's back button over the lens do the same. A click on the button makes that lens current, so Backspace then continues up it; a press on its top-left corner resizes instead. At the lens's own folder the button is gone, Backspace reaches the desktop as before, and Alt+Left does not move the selection. Backspace in a rename edits the text and does not go back. Compare the button's size and place with Explorer's (provisional 24 px, 4 px inset); it has no hover highlight yet. |
| W35 (v0.3) (part script) (F-22) | On a Downloads lens's Lens page, type `*.pdf` in Show only: only PDFs and the folders remain, as you type. Add `*.tmp` to Hide: they go. Type `; ` and extra spaces mid-list: the text stays as typed, the caret does not jump. A filter that matches nothing shows "No items match the filter". Cancel puts the lens back; Apply keeps it over a restart. Typing on a 50,000-file lens never stalls the spaces; the list follows about a quarter of a second behind (B37), and half a second with many patterns: note whether that lag reads as slow. |
| W46 (v0.3) (script) (F-07) | Rename a lens's title to a nickname with F2 (nothing selected), then F2 again, clear it, Enter: the title shows the folder's name and the Lens page shows "Folder name". Ctrl+Z gives the nickname back. With the Lenses page's "Show as" set to Full path, clearing shows the full path; set to Custom, clearing shows the folder's name. Clearing the title of a normal space, or of a lens that already follows its folder, changes nothing. |
| W45 (v0.3) (part script) (F-67) | With a `dev.ps1` scratch config, make a lens on a scratch folder. Rename the folder in Explorer: within 5 s the lens shows the new path and keeps its items, its title follows (under "Folder name"), and a bar along its bottom reads "Folder moved:" and the path for about 3 s, readable on Acrylic, Blur, Solid, and Clear at 100 % and 150 %. Ctrl+Z points it back (it then says "Folder not available"). Move the folder elsewhere on the same drive: it follows. Move it to another drive, or delete it in Explorer: "Folder not available", and it never lists the Recycle Bin. Browse into a subfolder and rename that subfolder: the lens returns to its own folder with no bar. Browse two levels down, then rename the lens's own folder in Explorer: the rename succeeds (the watch no longer holds the subfolder), within 5 s the lens follows the new root with its notice and drops the browse, and Ctrl+Z points it back. |
| W47 (v0.3) (script) (D17) | Sort a space by Date, descending, with View ▸ Status bar on, then press Ctrl+Shift+6 and Ctrl+Shift+2, pick View ▸ List, and click the status bar's Details button: Sort by ▸ still shows Date and Descending, Keep sorted is as it was, and the status bar stays. |
| W27 (v0.3) (F-07) | Hover a lens's title bar: after the usual delay a tip shows the folder the lens displays (a browsed subfolder's path, matching the title's `Root › Sub`), and it leaves with the pointer. An unavailable lens's tip reads "Folder not available" and the path. A normal space's title shows no tip. Hover the title until the tip shows, then F2: the tip goes and the edit is uncovered; moving within the title during the rename shows none. |
| W39 (v0.3) (script) (F-09) | On a space in Large icons, Ctrl+wheel down walks Medium icons, Small icons, List, Details, Tiles, Content and stops; wheel up walks back and stops at Extra large icons; no zoom pop-up shows for a step. Ctrl+Plus and Ctrl+Minus step the same way, Ctrl+0 returns to "Settings icon size", and each step is one Undo entry ("Undo Change view"). A space on "Settings icon size" zooms as before (the F13 row still passes). |
| W28 (v0.3) (part script) (F-07) | On a normal space and a lens: View ▸ "File name extensions" checks what the space shows now and choosing it flips Show and Hide; the View page offers Automatic, Show, and Hide, and Cancel restores. Under Automatic, Match Explorer follows Explorer's own setting (checked and unchecked, with the preset switched while the window is open), Custom hides. F2 on an item selects only the name part while extensions show. |
| W30 (v0.3) (script) (F-63) | On a normal space and a lens in Details, at 100 % and 150 %: right-click the header. Each column you check or uncheck shows or goes at once, its cells line up under its header, and Name is on and grey. A column brought back lands last. The choice survives a restart, and Ctrl+Z puts it back ("Undo Choose columns"). Right-clicking the header leaves the selection alone, including with rows scrolled under it. Hide the sorted column: the items keep their order and no arrow shows. With every column on in a narrow space, note how the right-hand columns are cut off (no sideways scroll yet). |
| W48 (v0.3) (script) (D17) | Fill a space, then in Small icons, Tiles, Details, Content, and List, at 100 % and 150 %, choose Fit to contents from the menu and from the Layout page: the space hugs its items with no scroll bar and no clipped last row (List keeps its height and takes the width; the rest keep their width), with the status bar on and off. With more items than fit, it stops at the work area and scrolls. Ctrl+Z restores the size. Add items and Fit again: it is still offered. |
| W31 (v0.3) (part script) (F-63) | On a space and a lens in Details, at 100 % and 150 %: over a header divider the pointer is the resize cursor. Drag a column's right divider: it follows the pointer, the rows follow, the columns may pass the edge, and on release Name takes the rest and the columns shift flush: judge that shift. The divider right of Name resizes the next column with no shift. Esc during a drag puts the width back and leaves the selection. A divider press does not sort; a click elsewhere on a header still does. Smooth on a 50,000-file lens, and a listing arriving mid-drag (copy files into the folder) does not end it. One Ctrl+Z restores the width, and it survives a restart. Compare the grab distance with Explorer's (provisional 4 px each way). |
| W32 (v0.3) (part script) (F-63) | On a space and a lens in Details, at 100 % and 150 %: drag a header past the other columns: an insertion mark shows between headers, readable on Acrylic, Blur, Solid, and Clear, and the columns do not move until release. Esc removes the mark and changes nothing. Name shows no mark and stays first, and nothing lands before it. A click without a drag sorts, now on release rather than on press: note whether that feels slower. One Ctrl+Z restores the order, and it survives a restart. Cell text follows its column. |
| W33 (v0.3) (part script) (D18) | Measure against Explorer's Compact view (Folder Options > View > Decrease space between items) at 100 % and 150 %, with DeskSpace's row sizes in `COMPACT_CELLS` provisional (`view_layout.rs`) and `NORMAL_CELLS` never measured either: Small icons, List, Details and its header, Tiles, Content. Correct the constants and record the measurement. Text and icons must not clip in a compact row. View ▸ "Compact view" and the View page's toggle agree, Cancel restores, it survives a restart, and the icon views do not change. |
| W37, W38 (v0.3) (part script) (F-65) | Group by ▸ beside Sort by ▸ and the View page agree, Cancel restores, and the Lenses page's choice reaches the next new lens. Compare the group names, order, and header with Explorer on the same folder, at 100 % and 150 %, in every view; a grouped List stacks downward. Week start is Sunday: check another locale. The rule runs along the row's bottom rather than from the text as Explorer's does: judge it. A click on a header folds and opens it. Ctrl+A after folding, then Delete: the question counts only visible items; the marquee and arrows skip folded items. Dragging to rearrange is refused and an icon view's hand arrangement returns on ungrouping. A 50,000-file lens grouped by Type scrolls and hovers with no hitch. |
| Space icons (part script) (D16, S7) | Make the cache by hand first (S3's `trim fluent <upstream icons.json> %TEMP%\deskspace-dev\icons`). With a scratch config, set `icon = "fluent:games"` on one space and `icon = "automatic"` on spaces titled "Games", "Photos", and "Stuff": the first three show their icon left of the title in the title's color, and follow a dark/light switch; "Stuff" and spaces without the key show none and look as before. Renaming a space keeps its icon and the box starts after it; hovering or clicking the icon does what the title bar does. Check at 100 % and 150 %, on Acrylic, Blur, Solid, and Clear, and with a lock. With the cache deleted, nothing shows and nothing fails. |
| Space icons: consent and download (part script) (D16, S8) | With a scratch config and a build without the signing key: set Settings > New spaces > Icon to Automatic, make the default title "Games", and use New space. The prompt "Show icons on spaces?" appears once and names the set, its author, and its license; "Download icons" logs a NotConfigured failure in the log and shows no icon, and nothing else breaks. "No thanks" saves Never and never asks again; set General > Icon downloads back to "Ask first" and the next new space asks again. With the published set and a key in the build: the same steps download it once, every automatic space shows its icon, and the next start reads the cache with no network (`-Verbosity 1`). A `-Fresh` first launch shows no prompt, because the seeded space has no icon. General's note says where the download comes from and that nothing is sent. |
| S5 (part script) (F-62) | Once `DESKSPACE_MINISIGN_KEY` and `UPDATE_PUBKEY` are set, run "Publish icon set" for `fluent`: the release is a pre-release named `icons-fluent-r1`, with the `.json` and `.sig`, and the repository's "Latest" badge still points at the app. A second run fails at the check for an existing release. Then a keyed build's new space asks once, downloads, and shows its title icon. |
| B37 (part script) | With a `dev.ps1` scratch config, a lens on a folder of 50,000 files, ungrouped and grouped by Type: copy files into the folder, change Sort by, toggle Show hidden files, and type in Show only. Other spaces keep responding and the lens never hitches; its list follows a moment later. The selection survives a refresh, and an item renamed with F2 stays selected once the listing arrives. Sort by, then Cancel in Settings, returns the old order. A locked lens places its items instead of stacking them. Change folder…, a browse, and Exit during a big refresh leave no stray list and do not hang. |
| B33 (part script) | A lens on a folder of a few thousand images scrolls without stalls, and its icons fill in. Generic icons show first and each becomes its own, on a lens, a normal space, and the takeover layer, with shortcut arrows and sync badges still drawn. A file rewritten in place keeps its old icon until the new one loads, with no generic flash. A lens on a slow or unplugged network folder leaves every other space responsive, and its title shows the generic folder before the folder's own icon. Note whether the generic flash at startup looks wrong beside a Startup animation. Two lenses on one folder both fill in. |
| B41 (part script) | With a 50,000-file lens in a scratch config, open Settings and select the lens's Shortcuts page: the page appears without a stall, its first page of icons fills in a moment later, and scrolling past that page shows the generic folder and file icons. The X and + tiles still work. |
| F-40 on Windows 10 (D18) | On a Windows 10 desktop, right-click an empty spot → View: confirm it offers only Large, Medium, and Small icons, ignoring Auto arrange icons, Align icons to grid, and Show desktop icons; then choose Small icons and confirm the labels sit below the icons. Windows 11 passed 2026-10-31; this row closes the Windows 10 half of W41. |
| C11 (part script) (D22) | On a space and a lens, at 100 % and 150 %, with "Snap resized spaces to whole columns and rows" on: in the icon views a resize still lands on whole icon cells; in Small icons, List, and Tiles it lands on those views' own cells; in Details and Content only the height fits whole rows, the width follows the 16 px "Snap to grid" step, and the header is not clipped. With the status bar on, the last row is whole. The resize starts after a short dead zone (about half a row) in every view. Turn the setting off: the dragged size stays, snapped only to "Snap to grid". |
| C12 (script) (D23) | Right-click an item in a space: "Fit to contents" sits above Explorer's own entries and is greyed on a locked space and on one that already fits. Choose it on a resized icon-view space: the space fits its items, stays inside the work area (it scrolls if they do not fit), and Ctrl+Z reads "Undo Fit to contents". A desktop item's menu has no Fit entry. |
| P2 (info tip) | Hover an item until its tip shows, then press F2: the tip goes and the rename box is uncovered, on a space and on a loose desktop icon with takeover on. |
| P6 (script) (grouped fit) | Group a space by Type in Details and in List, choose Fit to contents: no scroll and no clipped row. Fold a group and fit again: it shrinks. |
| P7 (part script) (resize minimum) | Shrink a Details, a Content, and a Tiles space from the bottom and the top, at 100 % and 150 %, with the status bar on and off: each stops at one whole row, and the header is not clipped. |
| P8 (label size) | Set the label size to 160 % and look at Small icons, List, Details, Tiles, and Content, normal and compact: no clipped text, at 100 % and 150 %. |
| P23 (script) (new space on start) | With DeskSpace not running, `deskspace --config <scratch> --new-space` starts with one more space than the config holds. With it running, the same command adds one space and starts no second process. |
| P25 (v0.3) (script) (space cap) | Drop items on a space until it reaches the bottom of the monitor: it stops and scrolls. Ctrl+Z puts it back. |
| P15d (part script) (Details indicator) | Over a Details space with every column on, at 100 % and 150 %: a thin bar shows along the bottom, and slides right as you scroll sideways. Widen the space until the columns fit: it goes. With enough items to scroll vertically, both bars show without touching, and with few items only the bottom one does. Other views show no bottom bar unless they scroll that way. |
| P20 (v0.3) (part script) (lens rename undo) | Rename an item in a lens, Ctrl+Z: it stays selected. Ctrl+Y: still selected. |
| P15c (part script) (Details sideways) | Shift+wheel over a Details space with every column on, at 100 % and 150 %: the columns scroll sideways and the header stays with them. |
| P15e (part script) (Details sideways) | On a mouse with a tilt wheel, over a Details space with every column on at 100 % and 150 %: tilting right scrolls the columns right and tilting left scrolls them left, with the header following. In other views the tilt does nothing. |
| P22 (part script) | Open "From installed apps…" from a space's Shortcuts page and from the custom shortcut dialog: it lists apps with icons. Close it, wait for the background refresh (with `--debug`, "cached the installed apps"), and open it again: the apps and icons still show, with no failure or stall. Do it twice in a row. |
| P21 | In a debug build with `--debug` open, preview Bounce repeatedly on a space: the animation stays smooth (no hitch once a second), and the debug panel updates again once it stops. |
| P10 (v0.3) (part script) | U5's and U7's rows still pass: undo and redo of deletes, pastes, moves, and renames behave as before. The rollback itself is test-carried, because the gap between the check and the failing step is too short to hit by hand. |
| C10c (part script) (F-58) | With a scratch config, customize a space's look, then Colors > "Save theme…" and name the file (with or without the suffix). Load it on Global and on another space, once through "Load theme…" and once from the Themes list after closing and reopening Settings: the look, icon spacing, and padding change, and the Dark/Light setting does not. A load on a space that follows Global turns its "Customize this space" switch on. Cancel puts the old look back; Apply keeps it over a restart. With no theme files the Themes list is disabled and says so; a broken or oversized file is reported and changes nothing; saving over an existing theme asks before replacing it. |
| P35 (v0.3) (part script) (unreadable config) | Run `./scripts/dev.ps1` once, make a space, and exit. Hold the scratch config open from PowerShell (`$h = [IO.File]::Open($path, 'Open', 'Read', 'None')`) and run it again: after about a second a "DeskSpace did not start" box opens in front of other windows, names the file, says it was left unchanged, and reads well; OK ends the process with no tray icon or space shown. Close the handle (`$h.Close()`) and run again: every space is there, and no `.bak` was made. |
| P37 (v0.3) (part script) (no replace on a move) | Every move into and out of storage now goes through `MoveFileExW`, so check the plain paths still work. Drop a file from the desktop onto a space: it leaves the desktop and opens from the space. Drop a second file with the same name from another folder: the space lists both, and both open with their own contents. Delete the space while the desktop already has a file of that name: the stored file comes back as "name (2)" and the desktop's own file is untouched. Ctrl+Z, then Ctrl+Y: each file goes back to where it was, then returns. Drop a file from a USB stick or a network share: it stays where it was and the space refers to it. With `-Verbosity 1`, no "could not move" line appears for any of these. |
| P49 (v0.3) (part script) (space paint) | Windows 11 with Transparency effects on, three spaces with different items, `--on-top`: click an item in each space in turn, then fold and unfold one. Each click shows the selection and check box on the item clicked, in that space only; no space ever shows another's title or items, an older picture of itself, or only its blur. Repeat after changing a font in Settings > Fonts and pressing Apply. |
| P50 (v0.3) (part script) (cross-space selection) | Ctrl+click two items in one space, then Ctrl+click one in another: only that one shows selected. Press Delete on a throwaway scratch config: only that file goes to the Recycle Bin, and the first space's two stay. Repeat with Shift+click, and with Explorer's check boxes on in independent mode (tick two in one space, click in another, Delete): the first space's ticks are gone and its files stay. |

## Found while working

One line per issue noticed but not fixed: file, symptom, suggested task.

Triaged 2026-11-05: the folder watch's subtree listing became P47, the page
scroll's focus G13, and the settings header's keyboard path P48; the
`locking_mode` question was answered in `TOOLS.md`, and "Pick a task" was
refreshed.

- `write_shortcut` (`app/files.rs`) picks a name with `unique_destination`, then `std::fs::copy` or the shell link's save writes it, and both replace a file that appears there meanwhile, as `storage.rs` did before P37; in a lens that is the user's own folder. Suggested task: write to a temp name beside it and claim the final name with `move_no_replace`, as `move_to_first_free` does.
