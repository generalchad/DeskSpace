# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

Only open work lives here. Finished tasks are moved, verbatim, to
`archive/TASKS-done.md`; read it only for history (its file and line pointers
describe the code when each task landed, often before the R1 and K splits).
Work that is built but not yet confirmed on screen is listed in section V.

**Pick a task:** section L (adding items to spaces) is the current priority;
start with L1. G5a-G5c are ready, in order. F13 first needs the Ctrl+wheel bug
under **Found while working** explained. Tasks marked **[needs decision]** or
in section D are not ready; skip them.

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
   `docs: ...`, `test: ...`, `perf: ...`, `ci: ...`, `release: ...`,
   imperative and lowercase, with the task id in parentheses (`refactor: split d2d.rs (K2)`).
   In the same commit, tick the task's box and move the task, verbatim, to
   the end of `archive/TASKS-done.md`, so this file holds only open work.
9. Tasks marked **[needs human]** need an on-screen check you cannot do.
   Implement, list exactly what to check in the commit body, and add a
   one-row summary of the check to section V; then archive the task as in
   rule 8. Anything only a human can record (for example L1's findings) stays
   here until it is recorded.
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
| Keys (spaces are never focused) | `deskspace-platform/src/win32/keyboard.rs` (`fn handle`) |
| Space window procedure, DWM | `deskspace-platform/src/win32/{space_window,dwm}.rs` |
| Drawing | `deskspace-platform/src/render/scene.rs`, `render/d2d/` |
| Settings window controls | `deskspace-platform/src/win32/settings_window/` (`mod.rs`, `build.rs`, `update.rs`, `nav.rs`, `paint.rs`, `commands.rs`) |
| Shell: open, item menu, drag out | `deskspace-platform/src/win32/shell_items.rs` |
| Shortcuts from dropped shell items | `deskspace-platform/src/win32/shortcuts.rs` |
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
2. `deskspace-app/src/settings/`: add a `field::` id in `ids.rs` (the next
   free number; see the id tables), and a `Field` on the right page in
   `pages.rs`: a look setting goes in `look_pages` (fed the theme being
   edited, which is a space's own while it is customized), a Global-only one
   in `general_page` (or the page it belongs on); an option table goes in
   `tables.rs`. Then add the match arm that clamps and returns the right
   `Effect` (`None`, `Appearance` to restyle, `Layout` to re-flow, `Pending`
   for apply-on-OK) in `apply.rs`: in `apply_look_field` for a look setting,
   which makes it per space with no further work, or in `apply_field`'s
   Global-only arms otherwise.
3. If Cancel must undo a re-flow or restyle, extend `settings::needs_relayout`
   or `needs_restyle` (`App::cancel_settings` in `app/settings_session.rs`
   calls them).
4. If Apply must do something outside the config (registry, hotkeys,
   takeover), add it to `App::apply_settings` (`app/settings_session.rs`)
   next to `sync_start_with_windows`.
5. Tests: a serde round-trip / missing-field default test in core or config,
   and an `apply_field` test in `settings/apply.rs` (copy
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
3. `render/d2d/`: draw it in `Painter`'s paint path. Do not allocate there;
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

## L. Adding items to spaces (ROADMAP F-50, F-53..F-55)

Today an item reaches a space by a file drag, a paste, or (F-50) a drag that
carries shell items. The maintainer reports that dragging an app from the
Start menu onto a space still does not work, so users drag it to the desktop
first and then into the space. These tasks fix that drop and add menu-driven
ways to add items. Do them in order; L3-L6 do not depend on L1-L2.

Shared code, already in place (F-50, commit `01043e8`):

- `win32/drop_target.rs`: `DragKind`, `drag_kind`, `shell_items`,
  `shell_item`, `display_name`, `shell_drop_effect`, and `log_formats`.
- `input.rs`: `ShellItem { name, id_list }` and
  `SpaceEvent::ShellItemsDropped`.
- `win32/shortcuts.rs` `create_shortcut(item, path)`; `app/files.rs`
  `App::add_shortcuts(ui, space, at, items)` and `shortcut_file_name`;
  `storage.rs` `ItemStore::unique_path`.

Every way of adding a shortcut must end in `App::add_shortcuts`; never add a
second shortcut writer.

- [ ] **L1. Log what a drag offers, then diagnose the Start-menu drop.**
  **[needs human]**
  - Likely cause (unconfirmed): a desktop app in Start is a real `.lnk` under
    `Start Menu\Programs`, so its drag may carry `CF_HDROP` as well as a Shell
    ID List. `drag_kind` lets files win, and `desired_effect` asks for `MOVE`
    or `COPY`. If Start offers only `DROPEFFECT_LINK`, the result is
    `DROPEFFECT_NONE` (the no-drop cursor). If it offers `COPY`, the space
    lists Start's own `.lnk` in place, so a later Delete would remove the
    user's Start entry; if it offers `MOVE`, the shortcut leaves Start.
  - Code, `win32/drop_target.rs` `DragEnter`: log every drag at `debug`, not
    only refused ones: one line with `allowed.0`, `keys.0`, and the chosen
    `DragKind` (derive `Debug` on it), then call `log_formats(data)` whatever
    the kind. `DragEnter` runs once per drag, so this is not a hot path.
    Update `log_formats`' doc comment, which says it only runs on a refusal.
  - Check (maintainer): with DeskSpace **not elevated** (UIPI silently blocks
    drops from Start or Explorer into an elevated window), run
    `./scripts/dev.ps1 -Verbosity 1` and drag onto a space: a desktop app
    from Start's "Pinned" grid, one from "All apps", a Store app (for example
    Calculator), a taskbar-pinned app, and a plain file from Explorer. Record
    per source the allowed effects, the formats, the cursor shown, and the
    result, here under this task (replace this bullet).
  - Done when: the log line lands (`fix: log every drag a space sees (L1)`)
    and the findings are recorded. If they contradict L2's plan, **stop and
    ask** before L2.

- [ ] **L2. Decide a drop by what the source allows; a link makes a
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

- [ ] **L3. A picker for apps (platform).** Independent of L1-L2.
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

- [ ] **L4. "Add ▸ Apps…" in the space menu (F-53).** After L3. Follow
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

- [ ] **L5. "Add ▸ Paste shortcut" (F-54).** After L4. Explorer's "Paste
  shortcut": copy anything in Explorer, then make a shortcut to it in a space.
  - Platform: `Ui::clipboard_shell_items(&mut self) -> Vec<ShellItem>` in
    `runtime/ui_shell.rs`, implemented in `win32/clipboard.rs`:
    `OleGetClipboard`, then the same path as drops (make
    `drop_target::shell_items` `pub(crate)`, or move it beside `shell_item`
    in `shortcuts.rs`; do not copy it). Reuse the retry the clipboard file
    reader already has for a clipboard another process holds.
  - App: `menu::PASTE_SHORTCUT = 46` in the "Add" submenu after a separator,
    greyed when `ui.clipboard_shell_items()` is empty (read it once while
    building the menu). The arm calls `add_shortcuts`; L2's copy-a-`.lnk`
    rule applies because it lives there. Paste shortcut never moves or
    deletes the clipboard's files and leaves the clipboard and any cut
    ghosting alone.
  - No key: Explorer has no default key for it, and Ctrl+V keeps meaning
    paste.
  - Docs as in L4, and the F-54 row.
  - Done when: tests pass. **[needs human]** check: copy a file and a folder
    in Explorer; "Paste shortcut" in a space makes two shortcuts and the
    originals stay; with nothing on the clipboard the entry is grey.

- [ ] **L6. "Add ▸ Files…" (F-55).** After L4. **[needs decision]**: does a
  file picked in a dialog move into the space (as a drop does, D3), stay in
  place, or become a shortcut? Recommendation: move, like a drop, because one
  rule for "add a file" is easiest to learn; but a dialog that relocates files
  may surprise users, so the maintainer decides first.
  - Platform: `pickers::pick_files(owner) -> Vec<PathBuf>` and
    `Ui::pick_files`, an `IFileOpenDialog` with
    `FOS_ALLOWMULTISELECT | FOS_FORCEFILESYSTEM`, opened on the Desktop
    folder. One dialog cannot pick files and folders together, so folders
    stay drag-only for now.
  - App: `menu::ADD_FILES = 47` in the "Add" submenu; the arm calls
    `add_dropped(ui, id, at, paths, keep_in_place)` per the decision, so it
    shares the drop path exactly.
  - Docs as in L4, and the F-55 row.
  - Done when: tests pass. **[needs human]** check: pick two files; they land
    in the space as decided, and Cancel adds nothing.

## C. Features (ROADMAP M3)

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

- [ ] **C9. Spaces entries on the desktop background menu (F-52).**
  The desktop's right-click background menu already carries the "Show
  DeskSpace spaces" verb while spaces are hidden (`win32/desktop_entry.rs`,
  `App::sync_desktop_show_entry`); make the desktop a small control surface
  that is there while DeskSpace runs.
  - Platform, `win32/desktop_entry.rs`: replace the single
    `set_desktop_show_entry(enabled, label, command, icon)` with a
    data-driven `set_desktop_verbs(verbs: &[DesktopVerb])`, where
    `DesktopVerb { key: &'static str, label: &'static str, command: &'static
    str, icon: &'static str }` and each `key` is a child of
    `DesktopBackground\Shell` under a common `DeskSpace` prefix. Delete each
    known key first (`RegDeleteTreeW` per key), then write them; keep the
    `create` / `set_string` helpers.
  - Each verb's command re-launches the executable with a flag the running
    instance handles, as `--show` does today: `--settings` exists; add
    `--new-space` (a `cli.rs` field; `bootstrap.rs` posts a new
    `controller::NEW_SPACE` and exits, and the controller raises an event the
    app handles by creating a space at the pointer). Take the next free
    `WM_APP` id and update the table in `CONTEXT.md` section 6.
  - App, `app/visibility.rs`: `App::sync_desktop_verbs` writes "New space",
    "DeskSpace Settings", and - only while hidden - "Show DeskSpace spaces";
    `remove_desktop_show_entry` becomes `remove_desktop_verbs`; call it on
    `Started` and on every visibility change.
  - Docs: `CONTEXT.md` sections 6 and 8 (the CLI flag list), and the
    `ROADMAP.md` F-52 row.
  - Done when: CI is green (registry writes stay behind `cfg(test)`) and the
    flag/event round trip is unit-tested. **[needs human]** check: with
    DeskSpace running, right-click the desktop and use "New space" and
    "DeskSpace Settings"; the entries are gone after a clean exit.
    **[needs decision]** on which entries are always shown versus hidden-only.

## D. Reserved (strong model or maintainer; do not start)

Listed so they are not attempted piecemeal. Each needs design work first.
Ids are `R1`-`R13` so they do not collide with the ROADMAP decision ids
(`D1`-`D9`).

- **R3. Platform module layout.** The mechanical splits have landed in
  section K (`win32/settings_window.rs`, `render/d2d.rs`, `settings.rs`,
  `win32/runtime.rs`, `shell.rs`). What remains needs design:
  - one icon decoder instead of two: `icons::decode_icon_masked` (GDI
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
  `app/files.rs` (`add_dropped`, `add_shortcuts`), `app/clipboard.rs`
  (paste), and `bootstrap.rs` (seeding). Needs a
  decision on where the reads live (the app's `storage.rs` is a candidate).
- **R6. Undo design.** Needs an answer to ROADMAP open question 3.
- **R7. On-screen verification pass** (maintainer). The list is section V.
- **R10. Code signing (maintainer, ROADMAP M8 item 3, D6).** Apply to
  SignPath Foundation (the project must meet their OSS terms: an OSI
  license, which Apache-2.0 is, and a public repository and build). Then add
  their GitHub Actions integration to `release.yml`: sign `deskspace.exe`
  before both the zip and the installer, and sign the installer
  (`deskspace_<version>_x64-setup.exe`) through cargo-packager's
  `windows.signCommand` (a shell command containing `%1` for the file path).
  Store the credentials as Actions secrets. The updater shipped in v0.2.1
  with no feed (the feed waits for the `DESKSPACE_MINISIGN_KEY` secret); sign
  before turning the feed on, because unsigned updates trigger SmartScreen on
  every version.
- **R11. Windows 11 top-level context menu (ROADMAP M8 item 4).** Needs R10,
  and M2 may make it unnecessary.
- **R12. Release dry run on clean machines (maintainer).** v0.2.0 and v0.2.1
  were published without it; do it before the next release. Follow
  `RELEASING.md` with a pre-release version (for example `0.3.0-alpha.1`).
  Check:
  - the zip contains `deskspace-watchdog.exe` beside `deskspace.exe` (E12);
  - the workflow publishes a pre-release with the zip, the `.sha256`, and
    the cargo-packager installer `deskspace_<version>_x64-setup.exe`;
  - on a clean Windows 10 VM and a clean Windows 11 VM: the extracted exe
    runs, shows its icon, and lists the version under Properties > Details;
    `deskspace_<version>_x64-setup.exe` installs into
    `%LocalAppData%\DeskSpace.App` with no admin prompt and creates a
    Start-menu shortcut;
  - after E7: uninstalling removes the install root, the Run value, and the
    desktop verb, and leaves the storage folder and the config untouched;
  - after E10/E11: install `alpha.1`, publish `alpha.2`, and confirm that the
    manual tray check offers it and the update restarts into `alpha.2` with
    the same spaces;
  - what SmartScreen shows.

  Record the findings in `CONTEXT.md` section 10.
- **R13. winget (maintainer, ROADMAP M8 item 8).** After the first stable
  (non-pre-release) version that has the installer.
  - Submit the first manifest to `microsoft/winget-pkgs` by hand (`wingetcreate
    new` with the `deskspace_<version>_x64-setup.exe` URL), with package
    identifier `generalchad.DeskSpace`.
  - Then add a `release.yml` job that runs only for non-pre-release tags and
    updates the manifest (`wingetcreate update --submit`, or the
    `winget-releaser` action), using a fine-grained PAT stored as an Actions
    secret.

## F. Ad-hoc features

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
    **Result (2026-10-12):** Ctrl+wheel over a space does nothing under any
    preset, so the branch never fires (see **Found while working**).
    Deprioritized.

## G. More Explorer-standard controls

Remaining gaps from the controls review. Do them in order.

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

- [ ] **G5c. "Create shortcuts here".** After G5b and L2. The entry goes
  between "Copy here" and "Show here without moving". Choosing it takes L2's
  `DropPlan::Shortcuts` path: read the items with `shell_items(data)`, raise
  `ShellItemsDropped`, and return `LINK`, so `App::add_shortcuts` writes the
  shortcuts (or copies a dropped `.lnk`). No new `DropAction` variant and no
  second shortcut writer.
- [ ] **G6. Ctrl+Z undo.** Design first; see ROADMAP.md section 8, open
  question 3.

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

AGENTS.md asks for files under about 800 lines of non-test code. No file is
over it today; K1-K6 (archived) split the last ones. When a file grows past
it, add a K task here naming what stays in `mod.rs` and what moves where
(the archived K tasks are the model). Each K task is a **pure move**: no
renames, no logic changes, no signature changes beyond visibility. That keeps
them safe for any contributor and easy to review (the diff should be almost
all moved lines).

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

## V. Awaiting on-screen check

Built and unit-tested, but not yet confirmed on screen. This is the list for
the maintainer's verification pass (R7). Check with a scratch `--config`
(`./scripts/dev.ps1`), with `--on-top` over a high-contrast window for
anything visual, in light and dark mode where it matters. When a row passes,
delete it (and say so in the commit); when it fails, replace it with a task
or a **Found while working** line. The task text for each id is in
`archive/TASKS-done.md`.

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
| H5, H4 (F-46) | Panes, hairlines, dimmed rows, and keyboard movement in both modes; a space is greyed until customized, then edits apply to it alone and Cancel restores; the switch off and on restores custom values; a menu edit with the window open survives Cancel; an old config with per-space `icon_size`/`tint` opens customized; the highlight outline is visible on Clear and Solid (Acrylic passed). |
| I8 | With takeover on, creating, renaming, and deleting a desktop file updates the layer within about 150 ms. |
| Drag and drop (F-36, F-37) | The move/copy cursor; no `- Copy` left on the desktop after a drag out; the drag image over Explorer and over spaces; the in-space ghost. |
| Selection (F-41, F-42) | A space click focuses the desktop and clears its selection; check boxes in both Explorer modes; rubber-band and Ctrl+A. |
| Clipboard and rename (F-38, F-02) | Cut, copy, and paste, with cut ghosting and its clearing when another app takes the clipboard; F2 item rename. |
| Display (F-25, F-43) | Compressed/encrypted label colors; info tips; jumbo icons; per-monitor DPI on a mixed-DPI setup. |

## Found while working

Add one line per issue noticed but not fixed: file, symptom, suggested task.

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
- `deskspace-platform/src/win32/runtime/mod.rs`: `is_space_window` checks only
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
- `too_many_arguments` allows that are still needed (B9 removed only the stale
  ones): `scene.rs` `desktop_layer_scene` (8), `desktop_layer_drag_scene` (9),
  `drag_scene` (8); `render/d2d/` `draw_item` (8), `draw_check_box` (8),
  `draw_edit_buffer` (9); `win32/settings_window/build.rs` `Builder::control` (8).
  Group the scene builders into a `ViewState`-like struct, and the `d2d`
  draw helpers, so the allows can go. Suggested task: a `ViewState`-style
  scene-args struct plus a `d2d` arguments struct.
- `deskspace-app/src/app/pointer.rs` `adjust_icon_size` (`SpaceEvent::Wheel`):
  the maintainer reports (2026-10-12) that Ctrl+wheel over a space does nothing
  under any preset, so even the `Custom` branch that should change
  `Theme.icon_size` never runs. The G2 check was therefore premature. Find out
  why the Ctrl+wheel event does not reach `adjust_icon_size` (before doing F13).
