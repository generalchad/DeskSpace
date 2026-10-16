# CONTEXT

Project context for DeskSpace. Read this before making changes.
`AGENTS.md` documents the build/test workflow and engineering rules.

## 1. What DeskSpace is

DeskSpace is a free and open-source desktop space organizer for Windows. It
groups desktop items into movable containers ("spaces") and, eventually,
mirrors folders as live "lenses" (comparable tools call these Folder Portals),
with roll-up, auto-hide, and per-space appearance control.

The long-term goal is to match comparable tools feature-for-feature while
remaining native, lightweight, and offline. The current tree is an early but
usable framework, not a finished product.

### Goals

- Native Win32 rendering and window behavior; no webview, no runtime.
- Windows desktop integration: spaces live on the desktop, not above other apps.
- Fast startup, low idle CPU, no background telemetry.
- Data-driven rendering and UI so app and platform stay decoupled.
- Simple, forward-compatible configuration.

### Non-goals (for now)

- Cross-platform support. The workspace is Windows-only.
- Cloud sync or accounts.
- Replacing Explorer itself.

### Direction (decided)

See `ROADMAP.md` for the full plan. In short: DeskSpace will take over the
desktop icon layer, render each space as its own window (Direct2D content over
a DWM blur material), relocate items dropped into normal spaces by move into a
managed store (lenses are folder views that never move data on their own;
dropping onto a lens moves files into its folder via the shell), and ship a tray,
context menus, and a settings window in the first usable release (v0.2).

## 2. Current status

- `cargo build`, `cargo clippy --workspace --all-targets -D warnings`, and
  `cargo test --workspace` (379 tests) are green. Update this count whenever a
  test is added or removed.
- Released: v0.2.0 (the first public release), v0.2.1 (update checking),
  v0.2.2 (adding shortcuts to spaces), and v0.2.3 (the space menu's Add files
  and Explorer-style right-button drag),
  each as an NSIS installer and a portable zip on GitHub Releases (section 10,
  "Packaging").
- Every space is its own top-level window with a Windows 11 look: acrylic or
  blur material behind a translucent tint, DWM rounded corners and border,
  dark/light mode following Windows, Direct2D/DirectWrite content, hover and
  selection highlights. Verified on Windows 11 build 26300.
- Spaces can be created, deleted, moved, resized, scrolled, rolled up,
  renamed, customized, sorted, and locked. Their items behave like Explorer
  items (selection, rubber-band, keyboard navigation, open, Explorer's context
  menu, drag in and out, cut/copy/paste, delete, rename, info tips) and mirror
  the desktop's display settings. Section 8 lists every control.
- A tray icon, a space right-click menu, and a native three-pane settings
  window (section 6, "Native UI"). Every space follows the global look until
  it is customized (section 4, `SpaceLook`).
- Desktop takeover (Explorer's icons hidden, the loose items drawn on
  DeskSpace's own layer) is built and opt-in (section 6; ROADMAP D7, D8).
- Dragging an app from the Start menu onto a space makes a shortcut
  (`TASKS.md` L2); it awaits an on-screen check (section V). The space
  menu's "New shortcut ▸" picks installed apps, takes a custom target, or
  pastes a shortcut to what is on the clipboard, and a space's Settings page
  "Shortcuts" previews it with an X per item and a + tile (F-53, F-54); F-53
  was checked on screen on 2026-10-13, and "Paste shortcut" awaits its check
  (section V).
- Verification: many behaviors are covered by unit tests but not confirmed on
  screen; `TASKS.md` section V lists them. Confirm one before building on it.
  Checked on screen so far: the blur, corners, space content, and settings
  layout; inline rename, in-space rearranging, the Delete key, per-space
  appearance, sort, grid and label settings, and cross-space moves
  (2026-10-03); a multi-item drag out (2026-10-10); the desktop layer's item
  interaction (`--takeover-layer`) and the takeover (`--takeover`: the icons
  hide, return on exit, re-hide after an Explorer restart, and come back after
  a kill through the watchdog; 2026-10-11); inline-rename Shift/Caps, the
  whole-column and whole-row resize fit, the resize dead zone, "Fit to
  contents" in the settings window, and the accent-colored check boxes
  (2026-10-12). Ctrl+wheel over a space does nothing under any preset
  (`TASKS.md` **Found while working**).
- Not yet: lenses (phase 0 of M4, resize and scroll, is done), auto-hide,
  undo for file moves, Explorer's list/details view modes, code signing, and
  a live update feed. `ROADMAP.md` section 7 is the full feature list.
- History: commits `07dba93`..`18ffac2` replace the old overlay together and
  only build as a series; bisect across them as one step.
- Code health: no non-test file is over the ~800-line guideline (the R1 and
  K1-K6 splits; section 3 lists the `app/` modules).

## 3. Architecture

```
                 +------------------+
                 |  deskspace-app   |  binary: CLI, event handler, scene and
                 +---------+--------+  settings-form builders
                           |  depends on
             +-------------+-------------+---------------------+
             v                           v                     v
   +------------------+        +----------------------+  +------------------+
   | deskspace-config |        | deskspace-platform   |  | deskspace-update |
   +---------+--------+        |  Win32, DWM, D2D,    |  |  update check    |
             |                 |  shell, native UI    |  |  and install     |
             |                 +----------+-----------+  +------------------+
             +------------+---------------+
                          v
                 +------------------+
                 |  deskspace-core  |
                 +------------------+
```

Rules: dependencies point downward only; `deskspace-update` depends on no
workspace crate. `deskspace-core` has no OS
dependencies and does no IO: whoever creates an `Item` passes in the
`ItemFacts` it read (`deskspace_platform::item_facts`). `deskspace-platform` is the only crate containing `unsafe` or
Win32 types. `deskspace-app` describes what to show (scenes, menus, settings
forms) as plain data and reacts to events; the platform turns those into
windows and controls.

### Crate responsibilities

- `deskspace-core`: pure domain: geometry, ids, items, spaces, desktop,
  layout engine, command layer, theme and palette. `#![forbid(unsafe_code)]`.
- `deskspace-config`: persisted schema (`Config`, `WindowSettings`),
  versioning/migration, and an atomic `ConfigStore` over TOML.
  `#![forbid(unsafe_code)]`.
- `deskspace-update`: update checking and installation (ROADMAP F-45), built on
  `cargo-packager-updater`; it depends on no workspace crate.
  `#![forbid(unsafe_code)]`.
- `deskspace-platform`: the UI runtime and message loop, space windows, DWM
  styling, Direct2D painter, tray icon, popup menus, the data-driven settings
  window, desktop-shell discovery, shell item enumeration and icon cache, DPI,
  and global hotkeys.
- `deskspace-app`: two binaries. `deskspace` (`main.rs`) holds the `clap` CLI
  (`cli.rs`), startup and seeding (`bootstrap.rs`), the `EventHandler`
  (`App`, `app.rs`), model-to-scene conversion (`scene.rs`), the settings
  form and its mapping onto `Config` (`settings/`, pure), and the managed
  store for dropped files (`storage.rs`). `deskspace-watchdog`
  (`watchdog_main.rs`) is the takeover crash helper. `app.rs` keeps the
  `App` struct, `on_event`, the timer ids, the `menu` ids, and the preset
  tables; the rest of `App` is split across `app/` submodules, one
  `impl App` block each:

  | Module | Owns |
  |--------|------|
  | `persist` | `dispatch`, `apply_to_space`, `save` / `schedule_save` / `flush_save` |
  | `view` | resolving a space's look (`theme_for`, `grid_for`, `palette_for`, `layout_for`), `refresh_space`, `reflow_space`, the desktop layer and takeover |
  | `hit` | surface lookups (`surface_item_at`, `surface_palette`, `find_item_by_file`) |
  | `selection` | `Selection`, keyboard selection (`on_navigate`, `on_typed`), `active_items` |
  | `pointer` | `PointerState`, mouse events, resize, hover, marquee |
  | `drag` | `Drag`, drag-to-rearrange, the drag image, and drag out |
  | `rename` | `EditState`, inline rename of titles and items |
  | `files` | drops, delete, prune, `resync_desktop_layer` |
  | `clipboard` | cut / copy / paste |
  | `menus` | space, item, and tray menus, `create_space` |
  | `settings_session` | opening, previewing, applying, and cancelling the settings window |
  | `visibility` | show/hide, the desktop "Show spaces" verb, start with Windows |
  | `info_tip` | the hover info tip |

## 4. Domain model (`deskspace-core`)

### Geometry

- `Point { x, y }`, `Size { width, height }`, `Rect { x, y, width, height }`
  with `contains`, `intersects`, `union`, `offset`, `inset`, `with_position`,
  `with_size`, `center`.
- All are `Copy`, `serde`, `Default`.

### Ids

- `SpaceId(u64)` and `ItemId(u64)`, transparent serde newtypes.
- `Desktop::next_space_id` / `next_item_id` derive the next id from the current
  maximum, so ids stay stable across persistence without a serialized counter.

### Item

```
Item {
    id, path: PathBuf, label: String, kind: ItemKind, position: Point,
    size: Option<u64>, modified: Option<u64>,
    compressed: bool, encrypted: bool,   // file attributes, for label colors
}
ItemKind { File, Folder, Shortcut, Drive, Other }
```

`kind` and `size`/`modified`/`compressed`/`encrypted` come from the
`ItemFacts` passed to `Item::new` (and `Command::AddItem`), which the caller
reads once with `deskspace_platform::item_facts`; nothing refreshes them
afterwards. `ItemKind::classify(path, is_folder)` is pure: a folder, else by
extension. Tests pass `ItemFacts::default()`. `classify` never yields `Drive` (only
`with_kind` does), and a path with no extension is `Other`. `label` defaults
to the file stem.
`position` is the top-left of the item's grid cell **relative to the space's
content area**, so moving a space never touches its items.

### Space

```
Space {
    id: SpaceId,
    title: String,
    rect: Rect,           // desktop coordinates
    items: Vec<Item>,
    locked: bool,
    rolled_up: bool,
    custom: bool,              // uses `look` instead of the global settings
    look: Option<SpaceLook>,   // kept while `custom` is off
    title_height: i32,         // default 28
    user_sized: bool,          // set by a resize; keeps size, reflows, scrolls
}

SpaceLook { theme: Theme, gap: i32, padding: i32 }
```

A space follows the global `Theme` and grid spacing until it is customized.
`SpaceLook` is its own copy of every look setting: the whole theme (its
`ui_mode` is unused) plus the grid's icon spacing and edge padding. It is
seeded from the global settings (`SpaceLook::from_global`) the first time,
and kept when customization is turned off, so turning it back on restores it.
`active_look()`, `theme(global)`, and `grid(global)` resolve what a space is
drawn with.

Files from before `look` stored per-space `opacity`, `tint`, `text`, and
`icon_size`. They are still read (private, never written) and
`fold_legacy_overrides` turns them into a customized look at startup:
opacity becomes the tint opacity it produced, and a tint or text color turns
on custom colors, taking the other color from the palette shown at the time.

Helpers: `title_bar()`, `visible_rect()` (the title bar when rolled up, else
`rect`; used for window size and hit-testing), `content_rect()` (empty when
rolled up), `item_at(point, cell, scroll)`, `content_extent` / `max_scroll`,
`resize_edges_at(point, border) -> ResizeEdges` (none when locked or rolled
up; corners are twice as wide as edges), item lookup/insert/remove, and
`set_position`.

### Desktop

```
Desktop { spaces: Vec<Space>, layer: DesktopLayer, layout: GridSpec }
```

Operations: add/remove space, add/remove/move item, `space_item_count`,
`space_at(point)` (topmost hit test on `visible_rect`), `sort_by_id`.
`move_item` validates the target before removing the item, so a failed move
never loses it, and resets the item's position to the origin (the next
re-flow places it). `move_item` only takes items from spaces, so
`Command::MoveItem` on a desktop-layer item is `Rejected`; a loose item enters
a space through the file-move path instead. `next_item_id` spans the spaces
and the layer, and `item` finds an item in either.

### Desktop layer

`DesktopLayer { items: Vec<Item> }` is the desktop's loose items: the
Desktop-folder files no space holds (ROADMAP D7). The shell is the source of
truth, so it is a refreshed cache (`Command::SyncDesktop` reconciles by path,
keeping `ItemId`s) with no chrome of its own. Unlike a space item's
content-relative `position`, a layer item's `position` is absolute, in
virtual-desktop coordinates. `layer()` / `layer_mut()` reach it; the layer has
`items()`, `item`/`item_mut`, `item_at(point, cell)`, `push_item`,
`remove_item`, `replace_items`, and `item_count`.

### Layout engine (`layout.rs`)

```
GridSpec {
    cell_width: 96, cell_height: 104, gap: 10, padding: 12,
    title_height: 28, columns: 4, snap: 16, fit_to_grid: true,
}
```

- `arrange_space` resizes the space to fit its items (unless locked or
  user-sized) and calls `position_items`, which lays items on the grid in
  content-relative cells: `columns` wide normally, `columns_for(width)` wide
  for user-sized spaces. It also stamps `grid.title_height` onto the space, so
  a scale change reaches the title even for user-sized spaces. It does
  nothing for a locked space; `Command::Reflow` handles those (title height
  and positions only, size kept).
- `GridSpec` is the logical 96-DPI / 100 % base and is what the config stores.
  `GridSpec::scaled(scale)` produces the effective grid for a display. The app
  derives a per-space scale (the space's `Theme.icon_size` setting or
  Explorer's icon size, times the space's monitor DPI) and shares it through
  `layout_for(id)` / `metrics_for(id)`; commands take the effective grid of the
  space under the action: `Desktop::apply_with_layout(command, layout, bounds)`,
  with `apply` delegating using the stored base. Scene geometry, hit tests, and
  scroll all read the same per-space grid, so the grid the painter uses always
  matches the grid the pointer uses. The base grid and theme are the space's
  own while it is customized (`Space::grid`, `Space::theme`). When the space's
  theme uses the `MatchExplorer` preset, Explorer's item spacing replaces the
  cell and gap outright.
- `resize_rect` moves the grabbed edges to a point, snapped, keeping opposite
  edges fixed, at least `min_space_size()` (one cell plus padding) and inside
  the work area. When `fit_to_grid` is on (the default), a left/right resize
  then rounds the width to whole columns (`fit_rect_width` /
  `GridSpec::fit_width`) and a top/bottom resize rounds the height to whole
  rows (`fit_rect_height` / `GridSpec::fit_height`), so a user-sized space
  keeps equal padding on both sides and above and below the remainder; the
  grabbed edge stays put. `scroll_step()` is one row per wheel notch.
- `snap_to_grid` / `snap_rect` snap positions to the grid.
- `clamp_to_bounds` keeps rects inside the work area.
- `resolve_collisions` pushes overlapping spaces down.

### Command layer (`command/`)

All mutations flow through `Desktop::apply_with_layout(command, layout, bounds)
-> CommandOutcome` (`Applied | NotFound | Rejected`), where `layout` is the
effective (scaled) grid; `apply(command, bounds)` is a convenience that uses
the stored base grid. `command/mod.rs` holds the types and a dispatch `match`
whose every arm calls a private `Desktop::apply_<variant>` method;
`space_ops.rs`, `item_ops.rs`, and `layer_ops.rs` hold those methods by what
they change, and `sort.rs` the `SortSpace` ordering. Shared helpers in
`mod.rs`: `with_space` (run a change on a space or report `NotFound`),
`rearrange`, and `close_gap`. Variants:

`CreateSpace`, `DeleteSpace`, `RenameSpace`, `MoveSpace`, `SetSpaceRect`,
`ResizeSpace`, `FitToContents`, `Reflow`, `CustomizeSpace`, `SetSpaceLook`,
`SortSpace`, `ToggleLock`, `ToggleRollup`, `AddItem`,
`RemoveItem`, `MoveItem`, `ReorderItem`, `ReorderItems`, `RenameItem`,
`AutoArrange`, `SnapAll`, `SyncDesktop`, `MoveToDesktop`,
`SetDesktopItemPosition`.

`CreateSpace { title, at, size, locked }` clamps to the work area (it does
not snap). `MoveSpace`, `SetSpaceRect`, `ResizeSpace`, and `FitToContents`
reject locked spaces and snap to the grid; `SnapAll` skips locked spaces.
`ResizeSpace` also rejects rolled-up spaces and an empty edge set, marks the
space user-sized, and (with `fit_to_grid`) fits the dragged width to whole
columns and the height to whole rows, and
`FitToContents` clears the user size again (so the space grows to fit its
items). `SortSpace { id, key: SortKey, descending }` sorts by `Name`
(case-insensitive), `Kind` (folders, files, shortcuts, drives, other),
`Size`, or `Modified`. `bounds` is the work area of the monitor nearest the
action (`work_area_at`). `CommandOutcome::is_applied()` is the usual check.
`CustomizeSpace { id, on, seed }` turns a space's own look on or off, seeding
it from `seed` (the global settings) only when it has none. `SetSpaceLook {
id, custom, look }` sets both exactly (clamped): settings edits pass `custom:
true`, and Cancel uses it to restore a snapshot.

### Theme (`theme.rs`)

```
Theme {
    mode: ThemeMode,          // System (default) | Dark | Light; colors spaces
    ui_mode: ThemeMode,       // dark/light for the settings window and menus
    backdrop: Backdrop,       // Acrylic (default) | Blur | Clear | Solid
    corner_radius: u8,        // px, 0..=24, default 8 (theme::corner presets)
    tint_opacity: u8,         // percent, default 40
    title_shading: u8,        // percent, default 30
    show_border: bool,        // default true
    label_lines: u8,          // 1 or 2, default 2
    label_size: u8,           // percent, default 100 (LABEL_SIZE_RANGE 60..=160)
    icon_size: u8,            // percent, default 100 (ICON_SIZE_RANGE 50..=200);
                              // scales icon and grid cell
    overlay_size: u8,         // percent of the icon, default 80 (OVERLAY_SIZE_RANGE 40..=100)
    label_shadow: LabelShadow,// Auto (default) | On | Off
    custom_colors: bool,      // default false
    tint: Color, text: Color, // used only when custom_colors
    marquee_color: Color,     // default Windows blue; the selection box color
    show_marquee_border: bool,// default true; the selection box border
    preset: Preset,           // Custom | MatchExplorer (default); item display
    use_accent: bool,         // default false; accent for selection and border
}
```

`Preset` bundles the item-display choices: `MatchExplorer` mirrors Explorer's
icon size, spacing, check boxes, and extensions, and `Custom` uses DeskSpace's
own item settings. `Theme::match_explorer()` reports `preset ==
MatchExplorer`. The settings window exposes it as the Appearance page's "Item
preset" dropdown; the Layout page's icon-size, icon-spacing, and edge-padding
fields are greyed under `MatchExplorer`. The field also reads the older
`match_explorer = true|false` setting. More presets and preset import/export
are planned (ROADMAP F-48).

The ranges live beside the field (`theme.rs`); `GAP_RANGE` and `SNAP_RANGE`
live in `layout.rs`, and `NEW_SPACE_SIZE_RANGE` in `deskspace-config`'s
`schema.rs`. Use the constant, never a literal range.

`Theme::palette(system_dark) -> Palette` resolves concrete colors (`dark`,
tint with alpha, title fill, title and item text, hover/selected fills,
`marquee` (the opaque selection box color), border, `check_fill`, and
`edit: EditColors` for the rename field, which
follows the system mode rather than the space's) from the Windows 11
dark (`#202020` / white) or light (`#F3F3F3` / `#1A1A1A`) base, or the custom
colors. `ThemeMode::resolve(system_dark)` decides dark/light for a mode.
`ui_mode` drives `App::ui_dark` and `Ui::set_ui_dark` for DeskSpace's own
chrome; `mode` (through `Theme::is_dark`) colors the spaces.
`Palette::with_accent(accent)` replaces only `selected_fill`, `marquee`, and
`border` with the accent at their current alphas (the selection box becomes the
opaque accent); the app applies it in
`App::surface_palette` (`app/hit.rs`, reached through `palette_for`) when
`use_accent` is on and `deskspace_platform::system_accent_color()` (the DWM
`AccentColor` registry value, `0xAABBGGRR`) is known. Whenever an accent is
known, `surface_palette` also sets `check_fill` to it (opaque), whatever
`use_accent` says, because Explorer's check boxes always use the accent.
`Backdrop::Solid` forces an opaque tint; `Backdrop::Clear` has no
blur and lets the tint reach 0 %. Enums serialize snake_case;
`Backdrop::Blur` also accepts `"mica"` from early v2 files, and
`corner_radius` also reads the old `corners = "round" | "small" | "square"`
("round" and unknown strings load as 8 px, `corner::WINDOWS`).

## 5. Configuration (`deskspace-config`)

```
Config { version, desktop: Desktop, theme: Theme, window: WindowSettings }

WindowSettings {
    toggle_hotkey: String,      // default "Ctrl+Alt+D"; show/hide spaces
    new_space_hotkey: String,   // default "" (unbound); create a space at the pointer
    settings_hotkey: String,    // default "" (unbound); open settings
    start_hidden: bool,         // default false
    start_with_windows: bool,   // default false; per-user Run key at sign-in
    take_over_desktop: bool,    // default false; hide Explorer's icons (M2)
    new_space_title: String,    // default "New space"
    new_space_width: i32,       // default 240; clamped to NEW_SPACE_SIZE_RANGE
    new_space_height: i32,      // default 120
    new_space_locked: bool,     // default false
    check_for_updates: bool,        // default false; opt-in, about once a day
    last_update_check: Option<u64>, // Unix seconds of the last completed check
    skipped_version: Option<String>, // a version the user chose to skip
}
```

- `CURRENT_VERSION = 2`. `Config::migrate` upgrades version 0/1 files: it
  resets the theme (v1 colors were opaque and tuned for the old color-keyed
  overlay), clears each space's v1 overrides (`clear_legacy_overrides`:
  opacity, which was a fill alpha, tint, text, and icon size), and re-runs
  `position_items` because v1 stored absolute item positions. A missing
  `version` key deserializes as 0. On every load it also clamps the global
  theme (`Theme::clamped`), the grid (`GridSpec::clamped`: gap, padding, and
  snap), the new-space size (`WindowSettings::clamped`), and every space's look
  (`SpaceLook::clamped`), so a hand-edited out-of-range value loads within
  bounds.
- `WindowSettings`, `GridSpec`, `Theme`, `SpaceLook`, and `DesktopLayer` use
  struct-level `#[serde(default)]`, and `Item.kind`, `position`, `size`,
  `modified`, `compressed`, and `encrypted` default, so partial
  tables fill in defaults instead of failing the whole file. Unknown keys (for
  example the removed `background_alpha`) are ignored.
- `ConfigStore` resolves its paths via `directories::ProjectDirs` (on Windows
  `%APPDATA%\DeskSpace\DeskSpace\config\deskspace.toml`), or accepts an
  explicit path. `storage_dir()` is where dropped files are moved
  (`%LOCALAPPDATA%\DeskSpace\DeskSpace\data\storage`), or a `storage` folder
  beside an explicit config file, so scratch configs never touch real storage.
- `save` writes to a sibling `*.toml.tmp` then renames, so a crash never
  truncates the config.
- `window.start_with_windows` is applied through
  `deskspace_platform::set_start_with_windows` (a new `win32/autostart.rs`),
  which writes or deletes the value `DeskSpace` under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (the quoted
  `current_exe()` path). The app writes it on Apply when the value changed and
  once at startup when enabled, so the stored path follows a moved install; a
  failure is logged at `warn` and never blocks Apply. The uninstaller removes
  it through the custom NSIS template at `packaging/nsis/installer.nsi`.
- `window.take_over_desktop` (M2 takeover, opt-in) hides Explorer's desktop
  icons and draws the loose items on DeskSpace's own layer. Turning it on (or
  the hidden `--takeover` flag) also draws the layer; a clean exit and every
  startup restore the icons, and `TaskbarCreated` plus a 5-second health timer
  re-hide them after Explorer restarts. Apply turns it on or off at once.
- `load_or_default` returns defaults when no file exists. If the file cannot be
  parsed, it copies it to `deskspace.toml.unreadable-<millis>.bak` before
  falling back to defaults; a file from a newer version is loaded but copied to
  `*.newer-<millis>.bak` first, because saving would drop unknown fields.
  `--reset` copies the existing file to `*.reset-<millis>.bak` and refuses to
  start if that copy fails.

## 6. Platform layer (`deskspace-platform`)

### Module map

Crate root (`src/`): `lib.rs` (re-exports; every module is private, so the
app can import only from here),
`input.rs` (`Event`, `SpaceEvent`, `SettingsEvent`, `TrayEvent`, `NavKey`,
`SelectionKey`, `ClipboardAction`), `ui.rs` (`MenuEntry`, `SettingsForm` and
its parts, `CursorShape`), `edit.rs` (`TextEdit`), and `error.rs`
(`PlatformError`). `render/`: `mod.rs` (`Icon`,
`ItemMetrics`, check-box geometry), `scene.rs` (`SpaceScene`, `SpaceStyle`),
`d2d/` (the painter). Everything with `unsafe` is under `win32/`:

| Area | Modules |
|------|---------|
| Runtime and controller | `runtime`, `controller`, `tray`, `console`, `hotkey`, `outside_click` |
| Windows | `window_class` and `window_state` (class registration and `GWLP_USERDATA` state; every window class uses both), `space_window`, `dwm`, `drag_ghost`, `info_tip`, `menu`, `settings_window`, `color_picker`, `dark_mode`, `dialog`, `app_picker`, `app_catalog`, `shortcut_dialog` |
| Input | `keyboard` (the low-level hook), `drop_target` |
| Shell | `desktop_items`, `icons`, `shell_items`, `shell_view`, `shell_selection`, `display_settings`, `clipboard`, `file_op`, `drag_image`, `folder_watch`, `shortcuts` |
| Desktop takeover | `desktop_shell`, `desktop_icons`, `watchdog`, `desktop_entry` |
| System | `system`, `dpi`, `autostart` |

### Message, timer, and id tables

Never renumber these; add new ones at the next free value.

`WM_APP + n` messages posted to the controller window, all defined in
`win32/messages.rs` (`TaskbarCreated` is a registered message, not in this
range):

| n | Constant | Posted by |
|---|----------|-----------|
| 1 | `TRAY_CALLBACK` | the shell, for the tray icon (`tray.rs`) |
| 2 | `OUTSIDE_PROBE` | `outside_click.rs` |
| 3 | `EDIT_KEY` | `keyboard.rs` |
| 4 | `DELETE_KEY` | `keyboard.rs` |
| 5 | `CLIPBOARD_KEY` | `keyboard.rs` |
| 6 | `RENAME_KEY` | `keyboard.rs` |
| 7 | `SELECT_ALL_KEY` | `keyboard.rs` |
| 8 | `SHOW_SPACES` | a second launch (`desktop_entry.rs`) |
| 9 | `PROPERTIES_KEY` | `keyboard.rs` |
| 10 | `WAKE` | `Waker` (`runtime/mod.rs`) |
| 11 | `SELECTION_KEY` | `keyboard.rs` |
| 12 | `TYPE_KEY` | `keyboard.rs` |
| 13 | `NAV_KEY` | `keyboard.rs` |
| 14 | `FOLDER_CHANGE` | `folder_watch.rs` |

Next free: 15. A new message also goes in the `ALL` list of the
`messages.rs` tests, which check the values are unique.

App timer ids (`deskspace-app/src/app.rs`, delivered as `Event::Timer`):

| Id | Constant | Period | Purpose |
|----|----------|--------|---------|
| 1 | `AUTOSAVE_TIMER` | 700 ms | debounced config save |
| 2 | `CARET_TIMER` | 530 ms | rename caret blink |
| 3 | `INFO_TIP_TIMER` | 600 ms | hover delay before an info tip |
| 4 | `UPDATE_TIMER` | 60 s, then hourly | periodic update check (F-45) |
| 5 | `HIGHLIGHT_TIMER` | 1200 ms | settings-window highlight on a space |
| 6 | `PREVIEW_TIMER` | 15 ms | deferred settings preview (fires when the queue is idle) |
| 7 | `TYPE_TIMER` | 900 ms | type-to-select prefix buffer |
| 8 | `HEALTH_TIMER` | 5000 ms | re-hide Explorer's icons while takeover is on |

Next free: 9. Menu ids are in `app.rs`'s `mod menu` (1-12, 20-28, 30-36,
41-48; next free 13, or 49; 9, the removed "Remove from space", stays
unused); item-menu app ids must stay below
`0x8000` (`shell_items::SHELL_FIRST_ID`), where the shell's ids start.
Settings field ids are in `settings/ids.rs`'s `mod field` (next free 72; 25 and
26 were freed by the removed placement setting and stay unused). Page indices
(`mod page`) are build positions; a new page goes at the end (Shortcuts is 9,
after About) and `settings::listed_pages` says which pages a scope lists.

### Events

`Event` (`input.rs`) is everything the app hears: `Started`, `Space { .. }`,
`Hotkey { .. }`, `Tray(TrayEvent)`, `Settings(SettingsEvent)`,
`EditKey(EditKey)`, `DeleteSelection`, `Clipboard(ClipboardAction)`,
`ClipboardChanged`, `RenameSelection`, `SelectionKey(SelectionKey)`,
`ShowProperties`, `SelectAll`, `Typed(char)`, `Navigate(NavKey)`,
`Timer { .. }`, `PointerDownOutside { on_desktop }`, `ShowSpaces`, `FolderChanged`, `SystemThemeChanged`, and
`CloseRequested`. `SpaceEvent` is `MouseMove`, `MouseDown { ctrl, shift }`,
`MouseUp`, `DoubleClick`, `MouseLeave`, `Wheel`, `FilesDropped`,
`ShellItemsDropped`,
`CaptureLost`, and `DpiChanged`. `SettingsEvent` is `Changed`, `Button`,
`NavChanged`, `ScopeChanged`, `HeaderSwitch { on }`, `HeaderButton`,
`PreviewRemove { field, index }`, `PreviewAdd { field, x, y }`, `Ok`,
`Cancel`, and `Apply`. A new input source adds a variant here and one arm in
`App::on_event`.

### Runtime (`win32/runtime/`)

- `run(RunOptions, Box<dyn EventHandler>)` initialises OLE (single-threaded apartment) and
  common controls, finds the desktop host for the placement, creates the
  controller window, adds the tray icon, installs the console handler,
  delivers `Event::Started`, and pumps messages until quit. It routes
  `IsDialogMessageW` to the settings window for Tab/Enter/Escape handling.
  Release builds are GUI-subsystem (`#![windows_subsystem = "windows"]` on both
  binaries), so launching the installed app never shows a console window; debug
  builds keep the console so `-v` logging and Ctrl+C work while developing.
- `EventHandler::on_event(Event, &mut Ui)` is the app's single entry point for
  every window. `Ui` creates/destroys space windows, pushes rects, scenes and
  styles, shows/hides spaces, shows popup menus and confirmation boxes, opens/
  updates/closes the settings window, registers hotkeys, arms and cancels
  timers, sets the tray tooltip, and quits.
- Re-entrancy: the handler is moved out of the thread-local runtime while it
  runs. Win32 calls it makes (SetWindowPos, DestroyWindow, TrackPopupMenu,
  the color picker, MessageBox) re-enter window procedures; events raised then are
  queued and delivered by the outermost `dispatch`. Runtime state is only
  borrowed in short sections (`with_runtime`) that never call into Win32.

### Controller (`win32/controller.rs`, `win32/tray.rs`, `win32/console.rs`)

- A hidden top-level window (not message-only, so it receives broadcasts). It
  owns hotkeys (`WM_HOTKEY`), timers (`Ui::set_timer`/`clear_timer` via
  `WM_TIMER` → `Event::Timer`), the tray callback (`NOTIFYICON_VERSION_4`: left
  click → `TrayEvent::Activated`, right click → `TrayEvent::MenuRequested`),
  `TaskbarCreated` (re-adds the icon after Explorer restarts),
  `WM_SETTINGCHANGE`/"ImmersiveColorSet" (`Event::SystemThemeChanged`), and
  session end.
- Outside clicks (`win32/outside_click.rs`): while the app asks for it
  (`Ui::watch_outside_clicks`, on while an item is selected, a rename is
  active, or a surface is current), a `WH_MOUSE_LL` hook posts each button
  press to the controller, which raises `Event::PointerDownOutside {
  on_desktop }` unless the press hit a space or a popup menu. A press on the
  desktop layer counts as outside, because `is_space_window` checks only the
  spaces. Spaces are never activated, so this is the only way to see clicks
  on the desktop or other apps.
- `Ui::quit`, console Ctrl+C/close (`console.rs` posts `WM_CLOSE` and waits up
  to 4.5 s; only meaningful when a console is attached, as in debug builds),
  and `WM_ENDSESSION` all lead to exactly one
  `Event::CloseRequested`, then space and settings windows are destroyed, the
  tray icon removed, and the loop ends.
- Desktop integration (`win32/desktop_entry.rs`): while spaces are hidden, the
  app writes a per-user `DesktopBackground\Shell` verb ("Show DeskSpace
  spaces") whose command re-launches the executable with `--show`; on start
  `--show` posts `SHOW_SPACES` (`WM_APP + 8`) to the running instance's controller window
  (found by class) and exits, or starts with the spaces shown when none is
  running. The verb is removed when spaces are shown and on a clean exit.
- The tray and settings-window icon is the `.ico` embedded in the executable
  by `deskspace-app`'s build script (`winresource`, from
  `assets/deskspace.ico`). The tray loads resource 1 at the small-icon size
  (`tray::load_app_icon`); the settings window reuses that handle
  (`tray::shared_icon`). The tray also opens Settings from the keyboard
  (`NIN_KEYSELECT`).

### Space windows (`win32/space_window.rs`, `win32/dwm.rs`)

- One window per space, created by `Ui::create_space`. State is a
  `Box<SpaceState>` behind `GWLP_USERDATA`, freed in `WM_NCDESTROY`.
  `set_style`, `set_rect`, and `set_scene` return early when the value is
  unchanged (the style is kept in the state, the rect read back from the
  window, the scene compared with the stored one), so restyling every space
  on a settings edit only touches the ones that changed.
- The chrome-less desktop layer (`Ui::create_desktop_layer`,
  `set_desktop_layer_rect`/`_scene`, `destroy_desktop_layer`) reuses this
  window with a `chrome: false` style and no OLE drop target. It spans the
  virtual desktop (`system::virtual_desktop_rect`), is a `WS_CHILD` of the
  host when one is in use and top-level (kept at the bottom) otherwise, so it
  sits behind the spaces. A top-level layer and a top-level space both pin to
  `HWND_BOTTOM`, so `Ui::create_space` lowers the layer again after each space
  (`space_window::lower`); otherwise the full-screen layer would end up above
  the spaces and swallow their clicks. The app draws the loose desktop items
  on it (`scene::desktop_layer_scene`, from the model's `DesktopLayer`) while
  `App::layer_active` holds: takeover is on (`take_over_desktop` or the hidden
  `--takeover`), or the hidden `--takeover-layer` flag is set. Only with
  `--takeover-layer` alone does Explorer keep drawing its own icons under it
  (a development view). The layer is created visible and is not hidden by
  `Ui::set_spaces_visible`, by design (ROADMAP D9): it stands in for
  Explorer's icons, which hiding spaces never hid. Hiding both is M5's
  hide-all (F-11).
  - Item interaction is shared, not duplicated (ROADMAP D7): the app routes
    hit-testing, selection, hover, rubber-band, drag, drop, info tips, and the
    scene build through a `Surface` (`Space(id)` or `Desktop`) instead of a
    bare `SpaceId`, so one item path serves both. The `surface_*` accessors in
    `app/hit.rs` resolve a surface's items, rect, layout, palette, and cells;
    new item behavior should take a `Surface` and use them. The layer window's
    event id is 0 (`DESKTOP_LAYER_ID` in `app.rs`); real spaces start at 1. A click on the
    layer selects and opens its loose items, the wheel is ignored (there is
    nothing to scroll), and dragging a loose item repositions it
    (`Command::SetDesktopItemPosition`); dragging it over a space leaves the
    layer and starts the normal shell drag, so dropping it on a space moves
    the file into that space (the layer then reconciles from the shell).
    Space-only gestures (resize, move, roll up, scroll, inline rename, delete,
    clipboard paste) stay keyed on a space; a loose item's menu is Explorer's
    alone, and Delete does nothing on the layer.
- Top-level placement: `WS_POPUP` + `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`,
  kept at `HWND_BOTTOM` via `WM_WINDOWPOSCHANGING`, never
  activated (`MA_NOACTIVATE`). A space ignores `WM_SYSCOMMAND`/`SC_MINIMIZE`,
  so Show Desktop cannot minimize it: survival never depends on the shell's
  undocumented tool-window exclusion, and the accent blur is never torn down.
  `dwm::apply_style` extends the frame over the
  client area, sets dark mode, corner preference, and border color, and turns
  on the blur material with the undocumented accent policy
  (`SetWindowCompositionAttribute`, `ACCENT_ENABLE_ACRYLICBLURBEHIND` /
  `ACCENT_ENABLE_BLURBEHIND`; Clear uses a fully transparent
  `ACCENT_ENABLE_TRANSPARENTGRADIENT`, because disabling the accent after a
  blur turned the space black until it was recreated). The documented `DWMWA_SYSTEMBACKDROP_TYPE` was
  tried first and does not work here: system backdrops only render on active
  windows, and spaces are never active.
- Desktop-hosted placement (`--placement desktop-*`, a visible CLI option with
  no config setting since the "Keep spaces on the desktop" toggle was
  removed): `WS_CHILD` of Progman / WorkerW, painted opaque, rounded with a
  (non-anti-aliased) window region. When no host is found, `run` warns and
  falls back to top-level.
- Corners and border: DWM draws them for blurred and Solid spaces with
  square, 4 px, or 8 px corners (`SpaceStyle::dwm_draws_frame`). Every Clear
  space, Solid spaces with other radii, chrome-less windows, and hosted
  spaces are rounded by the painter (anti-aliased fills and border); hosted
  spaces also get a window region so the opaque fill does not
  show past the corners (`Frame` in `space_window.rs`). The blur material
  fills the whole window rectangle and ignores both the window region and
  the `DwmEnableBlurBehindWindow` blur region (tried on build 26300), so
  Acrylic and Blur spaces use the nearest DWM radius at or below the chosen
  one (`Theme::effective_corner_radius`). Larger blurred radii need a
  composition backdrop (M5).
- Mouse input is reported in desktop coordinates as `SpaceEvent`s;
  `TrackMouseEvent` provides `MouseLeave`; capture is held from button-down to
  the last button-up. `WM_MOUSEWHEEL` (screen coordinates, with the Ctrl state)
  becomes `SpaceEvent::Wheel`; it reaches never-focused spaces through Windows'
  "scroll inactive windows" setting (on by default). Ctrl+wheel changes the
  global icon size (`App::adjust_icon_size`), as it does on the desktop; a
  customized space keeps its own size.
- File drops (`win32/drop_target.rs`): every space is an OLE `IDropTarget`
  (`RegisterDragDrop`, revoked in `WM_DESTROY`), not a `WS_EX_ACCEPTFILES`
  window, so it chooses the drop effect and the shell shows the move/copy
  cursor. `drag_kind` says what a drag carries (`DragKind`: real files,
  shell items, or nothing) and the pure `drop_plan` decides what a drop does
  (`DropPlan`) and which effect to report. A file drop becomes
  `SpaceEvent::FilesDropped` with a `DropAction`: `Move` by default (`MOVE`),
  `KeepInPlace` while Shift is down (`COPY`), or `Copy` from a right drop's
  menu. A right drag over a space reports `MOVE` while hovering (the menu
  decides the real action) and, on `Drop`, shows a Move here / Copy here /
  Show here without moving / Cancel menu owned by the space window; the chosen
  action becomes the `DropAction`, reporting `MOVE` for Move and `COPY`
  otherwise (G5b; the "Create shortcuts here" entry is G5c). As in
  Explorer, a `LINK` drop always makes shortcuts: a file list links when Alt
  is held and the source allows a link, or when the source offers neither
  `MOVE` nor `COPY`. A `CF_HDROP` counts as files only when every entry is
  an absolute path: the Start menu puts AppUserModelIDs there (for example
  `com.squirrel.element-desktop.Element`), so its drags are shell items.
  Every `DragEnter` logs the allowed effects, keys, kind, and offered
  formats at `debug`. On `DragEnter` it also stores an empty "DropDescription"
  (`DROPIMAGE_NOIMAGE`, no text) on the source's data object, so a shell source
  such as Explorer or the desktop does not draw its default "Move"/"Copy" action
  hint over the item image. A move marks the source's data object with the
  "Performed DropEffect" format so a shell source does not move the files a
  second time after DeskSpace has; a shortcut, copy, or keep-in-place drop
  never sets it, since the original was not moved. A drag that carries
  neither files nor shell items reports
  `DROPEFFECT_NONE`. A shell-item drag (any form
  `SHCreateShellItemArrayFromDataObject` reads, including a packaged app with
  no filesystem path) is accepted (`shell_drop_effect` prefers `LINK`, then
  `COPY`, then `MOVE`). A shortcut drop raises
  `SpaceEvent::ShellItemsDropped` with each item's absolute ID list, Explorer
  display name, and filesystem path (`None` for a virtual item). The app
  (`App::add_shortcuts`) copies a dropped `.lnk` or `.url` into the space
  storage, so no shortcut points at a shortcut, and writes a new `.lnk` for
  anything else (`win32/shortcuts.rs` `create_shortcut`,
  `IShellLinkW::SetIDList` + `IPersistFile::Save`), under a clash-free name,
  as Explorer does when such an item is dropped on a folder (F-50). The
  source is never moved or deleted. A drag with real files and shell items
  is treated as files, so a real file still moves unless Alt is held.
- Dragging an item out (`shell_items::drag_out`) sets the source data object's
  "Preferred DropEffect" to `MOVE`, so dropping it on the desktop takes the file
  out of the space instead of leaving a `- Copy` behind. The drop target alone
  performs the transfer (the shell data object has async mode off, so it is
  done when the drag returns); afterwards the app only removes the item if its
  file is gone. It never moves the file itself after a drag: an earlier
  "dropped on the desktop, so move it back" step turned desktop copies into
  `- Copy` / ` (2)` duplicates.
- Clicking a space is made to behave like clicking the desktop
  (`win32/shell_selection.rs`, `Ui::on_desktop_click`): the shell's desktop
  window (`GetShellWindow`) is made active, so the app in front goes inactive,
  and the desktop icon selection is cleared through its `IShellView`
  (`SelectItem(NULL, SVSI_DESELECTOTHERS)`, found via `IShellWindows` and the
  `SID_STopLevelBrowser` service). Spaces never take activation, so nothing
  else would do this. A selection inside an Explorer window is left alone, as
  native Windows only changes a selection within the view that was clicked.
  The enumerator is cached and released before `OleUninitialize`.
- Explorer display settings (`win32/display_settings.rs`) are read from the
  desktop's `IShellView` (`win32/shell_view.rs`, shared with the click code)
  and `SHGetSetSettings`: `FWF_CHECKSELECT`/`FWF_AUTOCHECKSELECT` (item check
  boxes), `SSF_SHOWALLOBJECTS`/`SSF_SHOWSUPERHIDDEN` (hidden files),
  `SSF_SHOWEXTENSIONS` (filename extensions), plus `IFolderView2::GetViewModeAndIconSize`
  (the desktop's icon size) and `IFolderView::GetSpacing` (item spacing).
  `ShellDisplay` carries all of them; the app overlays them when
  `Theme::match_explorer()` is on. In particular the desktop icon size and item
  spacing replace the DeskSpace icon-size and grid settings (and drive the
  jumbo icon fetch and the per-monitor layout scale), so matching is 1:1. `ShellDisplay.check_boxes` is a
  `CheckBoxMode`: `FWF_CHECKSELECT` gives `Independent`, `FWF_AUTOCHECKSELECT`
  gives `Selection`, neither `Hidden`. When `Theme::match_explorer()` is
  on (the default), the app feeds them into each scene: `SpaceScene.show_check_boxes`
  draws an Explorer-style check box at each icon's corner, and a checked box is
  filled with `SpaceScene.check_fill` (the Windows accent, or Windows blue when
  it is unknown) under a white tick. In `Selection` mode
  the box is revealed on hover and while selected and its tick mirrors the
  selection; in `Independent` mode it is always shown and its tick is a
  session-only set separate from the selection. `desktop_display_since` keeps
  the stored mode when the view read fails (the view is briefly unavailable,
  for example while Explorer restarts), so a transient failure no longer hides
  the boxes; `desktop_display` is the same read starting from `Hidden`.
  `scene::item_label` shows a file's full name when
  extensions are on, except for shortcuts, whose `.lnk`/`.url` extension
  Explorer never shows; both forms are derived from the path, so the option can
  be toggled either way. `SSF_SHOWCOMPCOLOR` colors compressed labels blue and
  encrypted labels green (`Item::compressed`/`encrypted`, read at creation).
  A plain click ticks one item and unticks the rest,
  Ctrl+click ticks several, and clicking the box itself toggles one item
  (`render::item_check_box_hit` is the clickable region); in `Independent` mode
  the tick set is the operand for delete/copy/cut/drag (`active_items`), while
  in `Selection` mode the selection is. The snapshot is
  re-read at startup, on
  `Event::SystemThemeChanged`, and on every space click, and a change repaints
  every space. Icons carry the shell's overlay (`SHGFI_ADDOVERLAYS`, plus
  `SHGFI_LINKOVERLAY` for `.lnk`/`.url` only) so shortcut arrows and sync
  badges appear; labels come from
  `IShellItem::GetDisplayName(SIGDN_NORMALDISPLAY)`, and seeding skips hidden
  or protected items Explorer would hide.
- Every file DeskSpace moves (into storage, back to the desktop) is reported
  with `notify_moved` (`SHChangeNotify` delete + create, `FLUSHNOWAIT` so it
  never blocks Explorer waiting inside a drop). Without it the desktop kept
  stale icons for files already moved into a space, and dragging one dropped
  a path that no longer existed. Dropped paths that do not exist are ignored
  rather than listed (which showed a label with no icon).
- Drag image (`win32/drag_image.rs`, `win32/drag_ghost.rs`): the picture that
  follows the pointer, distinct from the move/copy cursor. When an item drag
  starts, the app builds a drag scene (`scene::drag_scene`) holding the whole
  active set, including members scrolled out of view at their grid positions,
  and `DragImage::of_selection` renders them together offscreen
  (`d2d::render_items`: one WIC bitmap render target with a scale transform,
  running the same `draw_item` as the space, so icons, labels, and colors
  match), scaling the group to `MAX_DRAG_IMAGE` when it is larger and baking
  in a uniform translucency (`DRAG_IMAGE_ALPHA`). The hotspot is the grab
  offset in the scaled group. Every view of the drag shows these same pixels.
  Inside the space a layered, click-through, topmost ghost window shows it
  (`UpdateLayeredWindow`, premultiplied DIB) and every dragged slot is drawn
  faded (`ItemState::Dragged`). Once the pointer leaves the space the ghost
  hides and `drag_out` hands the same image to the shell's drag-image manager
  (`IDragSourceHelper2::InitializeFromBitmap`, straight-alpha DIB, since it
  premultiplies itself; `SetFlags(0)` leaves the drop description off, so a
  target like the desktop cannot add its "Move to ..." label and movement
  arrow). Space drop targets forward all four calls to one shared
  `IDropTargetHelper`, without which the image vanishes over a space;
  it is released before `OleUninitialize`. Without an image the shell
  substitutes its own opaque, oversized one.
- Inline rename (`edit.rs`, `win32/keyboard.rs`): the app owns a pure
  `TextEdit` buffer and pushes it in the scene's `editing` (a space title) or
  `editing_item` (an item's label) field; the painter draws Explorer's rename
  field, an opaque box with a border around the laid-out text, the selection in
  the system highlight, and a blinking caret. An item's box wraps the whole
  name in the label's width and grows downward, drawn over the items below it,
  as Explorer's does; a title's box stays on one line. A `WH_KEYBOARD_LL` hook
  (installed while editing or while either selection watch is on) translates
  key presses during a rename into `Event::EditKey` and consumes them (an Alt
  or Win combination cancels the rename and passes through), because
  spaces are never activated and cannot receive `WM_CHAR`. The hook cannot use
  `GetKeyboardState` for modifiers or Caps Lock: the thread never removes
  keyboard messages, so that state is frozen. It reads each held modifier from
  `GetAsyncKeyState`'s sign bit and sets or clears it, and reads Caps Lock's
  toggle from `GetKeyState`'s low bit (`GetAsyncKeyState`'s low bit is an
  unrelated "pressed since the last call" flag), so Shift and Caps stay
  predictable. F2 renames the
  focused item, or the current space's title when no item is selected, and a
  second click on an already-selected item (a slow double-click) starts an
  inline item rename; an item
  rename is committed with `IFileOperation::RenameItem`
  (undoable; the extension is preserved when the label hid it) and applied
  through `Command::RenameItem`, which updates the item's path and label. The
  same hook watches keys outside a rename (`Ui::watch_selection_keys`): F2,
  Alt+Enter, Ctrl+A, Esc, F5, the Menu key, and Shift+F10 whenever an item is
  selected or a space is current; Enter, Delete, Shift+Delete, and Ctrl+C/X
  only with a selection; and Ctrl+V, Home, End, PageUp, PageDown, the arrow
  keys, and typed characters only with a current space. Every other key
  passes through. AGENTS.md's Gotchas has the flag-by-key table. Alt+Enter
  shows the focused item's Properties (`Ui::show_properties`) or DeskSpace
  Settings; Enter opens the focused item, Esc clears the selection, F5 refreshes
  the spaces, the Menu key or Shift+F10 opens the item's or space's context
  menu, Home/End/PageUp/PageDown scroll the space, and a typed character selects
  the next item whose label starts with it (type-to-select, with a short
  timeout buffer; `Event::Typed`). An arrow key moves the selection to the
  nearest item in that direction (`Event::Navigate`), Shift+arrow extends the
  selection from the anchor, and Ctrl+arrow moves the item focus without
  changing the selection; the moved item is scrolled into view. The focused
  item is drawn with a dotted rectangle (`SpaceScene.focused_item`). While a popup menu is open it owns every key, so the hook passes them
  through (`keyboard::set_menu_open`). The watch
  only consumes those keys while the desktop shell has the foreground
  (`desktop_is_foreground`), because the selection is kept when another app is
  focused, as Explorer keeps a desktop selection; otherwise DeskSpace would
  steal that app's own Ctrl+C/V. IME composition and clipboard are not
  supported in a rename. Each watched key is posted to the controller as its
  own `WM_APP` message (see the table above) and becomes one `Event`; a new
  key either reuses `SELECTION_KEY` with a new `SelectionKey` variant (the
  simplest path) or, if it carries data, gets its own message.
- Clipboard (`win32/clipboard.rs`, `win32/file_op.rs`): Ctrl+C/X put the
  selected items on the shell clipboard as `CF_HDROP` with a "Preferred
  DropEffect" (`MOVE` for a cut), the formats Explorer uses, so they paste into
  Explorer and other apps; cut items render ghosted (`ItemVisual::cut`).
  Ctrl+V pastes into the current space: an item already in a space is
  re-pointed (`MoveItem`), while external files and copies go through
  `IFileOperation` into the store (`FOF_RENAMEONCOLLISION` guards the shared
  store against Explorer's Replace dialog; `FOF_ALLOWUNDO` records an undo),
  and an `IFileOperationProgressSink` returns each created path so the pasted
  items are listed and selected. `Ui::clipboard_shell_items` reads the
  clipboard's shell items through `OleGetClipboard` and `drop_target::shell_items`
  (sharing `open_clipboard`'s retry) for "New shortcut ▸ Paste shortcut"
  (F-54). The keys arrive through the same low-level
  keyboard hook as Delete, now gated on a selection (Ctrl+C/X) or a current
  space (Ctrl+V). Delete sends the selection to the Recycle Bin
  (`Ui::delete_files(paths, permanent)` → `IFileOperation::DeleteItem` with
  `FOF_ALLOWUNDO | FOF_NOCONFIRMATION`, no prompt, undoable; Shift+Delete
  passes `permanent`, which drops `FOF_ALLOWUNDO`) and drops the items whose
  files are gone. The controller registers
  `AddClipboardFormatListener`; `Event::ClipboardChanged` clears cut ghosting
  when another app or the desktop replaces the clipboard and prunes items whose
  files a paste elsewhere moved away. A clipboard read that fails (another
  process holds it) keeps the ghost rather than clearing it.
- Resizing is app-driven like moving: the app hit-tests edges and sets the
  pointer with `Ui::set_space_cursor(CursorShape)`, which the window applies at
  once (WM_SETCURSOR is not sent while the mouse is captured) and on
  `WM_SETCURSOR`. The border band is 6 px on edges and 12 px at corners, and
  locked or rolled-up spaces cannot be resized. A press on a border only resizes once the pointer leaves a
  half-cell dead zone around the grab (`pointer::resize_edges_now`), so a click
  or a tiny drag on a border never resizes the space, and each axis joins
  separately: a corner grab that moves on one axis resizes only that axis.
- `--on-top` (hidden CLI flag) makes spaces topmost so their look can be
  checked without uncovering the desktop.

### Rendering (`render/scene.rs`, `render/d2d/`)

- `SpaceScene` and `SpaceStyle` are the contract between app and renderer.
  `SpaceScene` carries: `chrome`, `title`, `title_height`, the colors (`tint`,
  `title_fill`, `title_text`, `item_text`, `hover_fill`, `selected_fill`,
  `marquee_color` with `show_marquee_border`, `border`, `check_fill`, `edit`), optional `text_shadow`, label lines/size,
  `metrics: ItemMetrics`, `locked`, `rolled_up`, the `ItemVisual`s in
  window-local coordinates (id, rect, label, icon, state, hovered, checked,
  cut, `label_color`), optional `scroll_thumb` and rubber-band `marquee`,
  `editing` / `editing_item` with `caret_visible`, `show_check_boxes` /
  `check_boxes_persistent` (Independent vs Selection display),
  `focused_item`, and `highlight`. `SpaceStyle` is backdrop, corner radius,
  dark, border, and `chrome`. A new visual is one field here, filled in
  `deskspace-app/src/scene.rs` and drawn in `d2d/`; a `SpaceStyle` field
  is for window-level attributes only (it re-applies DWM state). The app scrolls by offsetting item rects and includes only
  items that intersect the content area; the renderer clips items below the
  title bar. Scenes are pushed only when something changes; the window stores
  a `PreparedScene` with strings pre-encoded to UTF-16, so painting does not
  allocate. `chrome: false` (the desktop layer) draws no title bar or border
  and fills the whole window with items; the painter reads the scene's
  `chrome` for the title and scroll thumb, and the window's `Frame` reads the
  style's, so both are set together. `SpaceStyle::dwm_draws_frame` is then
  false, so DWM draws no frame either.
- `Icon` is an `Arc` of owned premultiplied-BGRA pixels with a never-reused
  id. The main path decodes the `IShellItemImageFactory` HBITMAP through WIC
  (`d2d::decode_bitmap`); the classic fallback and overlays decode an HICON
  (`icons::decode_icon_masked`), destroyed right away. Scenes therefore never
  point at freed handles.
- `Painter` owns an `ID2D1HwndRenderTarget` (premultiplied alpha, 96 DPI so
  one DIP is one pixel), a brush, and a per-target bitmap cache keyed by
  `Icon::id`, pruned to the current scene's icons whenever a scene is set.
  The factories and text formats are shared per thread (`Graphics`, released
  in the runtime teardown). Text uses DirectWrite (Segoe UI Variable Text when
  installed; the lock glyph from Segoe Fluent Icons or MDL2), grayscale
  antialiasing on translucent surfaces and ClearType on opaque ones, ellipsis
  trimming, wrapped labels (one or two lines at the configured size; formats
  are cached per 10 % size step), and a 1 px shadow when the scene asks for
  one (label shadow mode `Auto` below 25 % tint alpha, or `On`). `D2DERR_RECREATE_TARGET` drops
  device resources for recreation on the next paint.

### Native UI (`ui.rs`, `win32/menu.rs`, `win32/settings_window/`)

- `MenuEntry` lists become `TrackPopupMenuEx` menus (`Ui::popup_menu` returns
  the chosen id).
- `SettingsForm` → pages → `Field`s (`Heading`, `Note`, `Choice`, `Toggle`,
  `Slider` (with a unit `suffix`, optionally labeled `SliderMark`s and, when
  `editable`, a number box that commits on focus loss instead of a static
  value), `Color`, `Text` (optionally `read_only`), `Button`, and
  `SpacePreview`). A `SpacePreview` (`settings_window/preview.rs`) is a
  child window of its page that paints a styled picture of a space with GDI
  into a back buffer: the title bar and body as opaque fills
  (`settings::space_preview_colors` approximates the tint over the blur),
  each item's icon and label in grid order with an X in its upper-right
  corner, and a trailing + tile; it scrolls with the wheel and reports
  `PreviewRemove` and `PreviewAdd`. Its data stays with the field in the
  settings state (found by control id), so an update only repaints it; its
  pure `PreviewLayout` serves both painting and hit-testing. It has no
  keyboard support yet. A new kind of
  control is a `FieldKind` variant plus its builder, `plan_field` diff, and
  command handling in `settings_window/commands.rs`; prefer composing existing kinds. Appearance is split across Appearance, Labels, Layout,
  Colors, and Advanced so no page clips on a 1080-pixel display. The settings
  window lays the pages out with common controls v6 (pages are container
  windows that forward notifications), uses Mica in the caption, reports edits
  as `SettingsEvent::Changed { field, value }` / `Button` / `Ok` / `Cancel` /
  `Apply` (plus the pane events below), and opens DeskSpace's own color picker
  for color fields (`Ui::pick_color`). The picker (`win32/color_picker.rs`) is
  a rounded, themed popup that opens on a palette grid after the Windows color
  dialog (a "Recent colors" row and a "Windows colors" grid), with the full
  saturation/value square, hue and alpha strips one toggle away ("Custom
  colors…"), a preview, a hex readout, and OK/Cancel. It replaces the dated
  system `ChooseColor` dialog (ROADMAP F-51). It is modal: it disables its
  owner, and pumps every message so Windows does not mark the owner "not
  responding", though only the picker can be driven. It paints first into a DIB
  back buffer (so it does not flicker) and deliberately does not extend the DWM
  frame into the client, which would make DWM read GDI's zero alpha and render
  the popup as glass. Its marker circles are composited into the buffer's
  pixels with per-pixel coverage anti-aliasing, because GDI draws ellipses and
  lines with none; its text, its check mark, and the settings window's color
  button glyph come from the system message font and the symbol font
  (`system::message_logfont`, `system::glyph_font`, `system::draw_glyph`). A
  color field is one owner-draw button: a slightly rounded box
  (`color_picker::draw_color_box`) with an eyedropper glyph, so the swatch
  itself opens the picker, with its hex value beside it.
  `Ui::update_settings`
  diffs a new form of the same shape into the existing controls, and touches
  the panes and header only when they change, since a slider drag sends an
  update per tick.
- `SettingsForm.nav` (a `SettingsNav`) fills the window with panes, separated
  by hairlines: a scopes pane (`scopes`, each with an
  `indent` and a `dimmed` flag; `ScopeChanged`), a categories pane (`groups`
  of `SettingsNavEntry`s, each pointing at a page and optionally `marked`
  with a dot; untitled groups have no title row; `NavChanged`), and a
  `SettingsHeader` above the page with a title, an optional
  `SettingsSwitch` (reported as `HeaderSwitch`), and an optional
  `SettingsButton` (reported as `HeaderButton`; shown only with a switch;
  the app uses it for a space's "Delete space", owner-drawn with a red tint
  to mark it as destructive). Both
  panes are owner-drawn list boxes painted like Windows
  11 navigation (an accent bar on the selection). The platform knows nothing
  about spaces: the app fills the panes, maps an index back to what it means,
  and answers each event with a new form. The page shape never changes: every
  page is always built, and the app picks which ones the categories pane
  lists.
- Selecting a space in the scopes pane, or opening the window on one,
  highlights it on the desktop for about 1.2 s so it can be found: the
  space's scene sets `SpaceScene::highlight`, and the painter draws a 2 px
  outline just inside the frame in the space's `selected_fill` at full alpha.
  `HIGHLIGHT_TIMER` clears it; selecting another space moves it, and Cancel,
  OK, and closing the window clear it too.
- The app's form (`settings/`): the scopes are Global, then every space
  indented, dimmed while it follows Global. Global lists every page but
  Shortcuts; a space lists the five look pages, which show its values (from
  Global until customized) greyed unless the header's "Customize this space"
  switch is on, and its Shortcuts page, which is always enabled. The
  Shortcuts page's X deletes an item after a confirmation
  (`App::delete_space_item`: a stored file goes to the Recycle Bin, an item
  listed in place is only unlisted; ROADMAP D3) and its + opens the New
  shortcut choices (`App::offer_new_shortcut`); both commit at once, so
  Cancel does not undo them. Global edits write `Config` directly; a space's edits become
  `SetSpaceLook` commands, the switch becomes `CustomizeSpace`, and the Colors
  page's reset becomes "Match global appearance". A space's header also carries
  a "Delete space" button, which runs the same delete (with its confirmation)
  and then falls back to Global; deletion is immediate, so Cancel does not
  bring the space back. The Layout page carries a "Fit to contents" button
  (built for every scope so the page shape stays constant, grey unless a
  resized, unlocked space is being edited), which runs the same
  `FitToContents` as the space menu; like a menu edit it commits at once, so
  Cancel does not resize the space back. Cancel restores each
  space's `SpaceSnapshot`; a space menu edit made while the window is open is
  taken into that snapshot, because the menu commits at once.
- Dark mode (`win32/dark_mode.rs`): `Ui::set_ui_dark`, which the app calls
  with `Theme.ui_mode` resolved against Windows on start, on the
  `SystemThemeChanged` event, and while the ui_mode choice is edited, switches
  popup menus (uxtheme ordinals 135/136, `SetPreferredAppMode` and
  `FlushMenuThemes`) and the settings window: dark caption, body and command
  bar colors (`UiColors`), `DarkMode_Explorer` / `DarkMode_CFD` control
  styles, and owner-drawn panes and header in both modes. The settings
  window's accent bar is a fixed
  `UiColors.accent`, not the Windows accent. Static labels are never disabled (Windows
  embosses disabled static text); disabled fields grey them through
  `WM_CTLCOLORSTATIC`. The spaces follow their own `mode` setting.
- DeskSpace's own modal dialogs (`win32/dialog.rs`: the frame, fonts,
  colors, control helper, and modal pump) are the installed-apps picker
  (`win32/app_picker.rs`, `Ui::pick_apps`) and the custom shortcut dialog
  (`win32/shortcut_dialog.rs`, `Ui::custom_shortcut`). Like the color picker
  they are owned by the settings window when it is open, else the
  controller, disable their owner, and pump every message while open
  (`IsDialogMessageW` gives them Tab, Enter, and Escape). The picker
  enumerates `FOLDERID_AppsFolder` (`BHID_EnumItems`) into a report-view list
  view with check boxes. The list and its icons come from
  `win32/app_catalog.rs`, a cache the app fills on a background thread (its
  own COM apartment) on `Started` (`Ui::prefetch_apps`) and that the picker
  refreshes in the background after each use, so the picker opens at once and
  newly installed apps appear next time; before the first listing finishes
  the picker lists synchronously without icons and renders them lazily. Each
  row's icon enters the image list on `LVN_GETDISPINFOW`, resampled from the
  cache (`icons::resample`) or rendered (`icons::shell_item_icon`). Picked apps become `ShellItem`s through
  `shortcuts::shell_item`, the function drops use; the dialog's result is a
  `CustomShortcut` (`ui.rs`; `ShortcutTarget::parse` reads typed text),
  written by `shortcuts::create_custom_shortcut`.
- The app embeds the common-controls v6 manifest dependency via
  `crates/deskspace-app/build.rs`.

### Desktop shell, shell items, DPI, hotkeys

- `folder_watch.rs` watches a folder off the UI thread (`ReadDirectoryChangesW`
  with overlapped I/O on one worker thread) and posts `controller::FOLDER_CHANGE`
  (`WM_APP + 14`) when its contents change, debounced by up to 150 ms into one
  message so a multi-file copy does not wake the app once per file; the
  debounce is the pure `Coalescer`. `Ui::watch_folder` starts or replaces the
  single watch (idempotent for the same path) and `Ui::unwatch_folder` stops
  it; the controller turns the message into `Event::FolderChanged`. M2 watches
  the Desktop folder while the layer is drawn (a change makes the app
  re-enumerate and `SyncDesktop`); M4 phase 2 reuses the same watcher for a
  lens's bound folder.
- `desktop_shell.rs` finds Progman or the wallpaper `WorkerW` (via the
  `0x052C` spawn trick) for hosted placements.
- `desktop_icons.rs` hides and restores Explorer's icon layer for the takeover
  (M2): `hide`/`restore` hide or show every `SysListView32` under a
  `SHELLDLL_DefView`, re-enumerating each time so a listview
  Explorer recreates is found, and `rehide_if_active` re-hides only while
  takeover is on. The walk recurses the whole subtree of the desktop shell
  (`Progman` / `WorkerW`), not just its direct children, because on some builds
  the icon `SHELLDLL_DefView` is nested under the shell (for example
  `Progman -> WorkerW -> SHELLDLL_DefView`); it is limited to the shell's
  subtree because an Explorer window also contains a `SHELLDLL_DefView`. Only
  the listview is hidden, never the `SHELLDLL_DefView`, so
  `find_host` and the shell-view COM reads keep working. `restore` runs in the
  `run` teardown before `OleUninitialize`; `rehide_if_active` runs on
  `TaskbarCreated` (Explorer restart) and from the app's health timer.
- `watchdog.rs` is the crash-safety half of the takeover (D8). Before the icons
  are hidden the app calls `arm_watchdog`, which creates a per-pid clean-exit
  event and launches the dedicated `deskspace-watchdog.exe` (a second binary of
  `deskspace-app`, `<pid>` as its only argument) detached, so a bulk kill of
  `deskspace.exe` by name or a closed console leaves it running. The watchdog
  re-parents itself once (`respawn_reparented`) so ending the app's process
  tree cannot reach it either, then waits on the main process and re-shows every
  listview unless the event was signaled; the runtime teardown calls
  `signal_watchdog_clean_exit` right after `desktop_icons::restore`, so the
  watchdog acts only on a crash. A named mutex keeps a stray watchdog a no-op,
  and the pure `watchdog_pid` / `is_watchdog` and `should_restore` decide the
  argument and the restore.
- `win32/desktop_items.rs`: `enumerate_desktop_items` and the folder-walk
  fallback; `win32/icons.rs`: `IconCache`.
  `enumerate_desktop_items` reads the desktop view's items and their positions
  (`IFolderView::ItemCount`/`Item`/`GetItemPosition`) once OLE is up, and
  falls back to a name-sorted walk of the user's Desktop folder, without
  positions, when the view cannot be reached. Icons are rendered by
  `IShellItemImageFactory` at the requested pixel size, and a path the factory
  rejects falls back to the classic 32 px icon (scaled). The overlay (shortcut
  arrow, sync badges) is drawn the way Explorer draws it: `SHGetFileInfoW` with
  `SHGFI_OVERLAYINDEX` names the overlay (this works in any process), and
  `IImageList::GetOverlayImage` fetches its full-icon-sized image from the
  system image list nearest the size (`SHIL_SMALL`..`SHIL_JUMBO`), area-averaged
  to `theme.overlay_size` percent and anchored in the bottom-left corner before
  compositing. HICONs are decoded from their color and
  mask bitmaps (`decode_icon_masked`). Entries are keyed by path, pixel size,
  and overlay size, with least-recently-used eviction at 1024 entries.
  `invalidate(path)` forgets every size of one path.
- `shell_items.rs`: Explorer behavior for items. `Ui::open_path`
  (`ShellExecuteExW` with the default context-menu verb; it first passes
  DeskSpace's foreground right on with `AllowSetForegroundWindow`, because
  spaces are never active and opened apps would otherwise stay behind), and
  paths are given to the shell with backslashes (so it cannot open a URL;
  `TASKS.md` E10 adds `Ui::open_url`). `Ui::item_menu` (the app's entries above the shell's
  `IContextMenu`; the controller forwards owner-draw and submenu messages to
  it while it is open), and `Ui::drag_out` (`SHDoDragDrop`; one item uses the
  shell item's own data object, several use `SHCreateDataObject` over the
  items' ID lists relative to their deepest common folder, or full IDs under
  the desktop when they share none, so a multi-item drag carries the shell ID
  list the desktop needs to drop at the pointer and accepts the drag image,
  plus "Shell Object Offsets" (`CFSTR_SHELLIDLISTOFFSET`, from the items'
  current cells) so the group keeps its arrangement instead of stacking in a
  column; async mode is forced off so the transfer ends inside the drag loop;
  reports the effect and whether it ended over the desktop). The
  runtime uses `OleInitialize`, which drag and drop needs.
- `dpi.rs` sets per-monitor v2 awareness; `window_dpi`/`scale_for` back the
  runtime's `space_dpi(id)` (the space window's own monitor scale), which the app
  reads per space. `system.rs` provides `work_area_at(point)`,
  `virtual_desktop_rect()`, `cursor_pos()`, `drag_threshold()`,
  `system_uses_dark_mode()`, `system_accent_color()`, and `wide` (UTF-16
  conversion; use it rather than a local copy).
- Info tips (`win32/info_tip.rs`, `Ui::show_info_tip`/`hide_info_tip`): after the
  pointer rests on an item, the app shows Explorer's info tip (name, shell type,
  size, local modification date) in one reusable native tracking tooltip
  (`TTF_TRACK`, `TTM_TRACKPOSITION`/`TTM_TRACKACTIVATE`). The app owns the hover
  delay with a timer; moving off, clicking, or hiding spaces dismisses it.
- `hotkey.rs`: `parse("Ctrl+Alt+D")`, `hotkey_id(index)`, register/unregister
  (on the controller window via `Ui`). The app registers three bindings, each at
  its own index and editable on the Hotkeys page: show/hide spaces (`Ctrl+Alt+D`
  by default), create a new space at the pointer, and open settings (both
  unbound by default). An empty binding is unregistered. The table
  `settings::HOTKEY_BINDINGS` drives all of it: each row names its
  `HotkeyAction`, its two field ids, its label, and the `WindowSettings`
  field that stores it, and its position is its registration index;
  `App::on_hotkey` matches on the action.

## 7. Runtime flow

1. `main` parses the CLI and initializes `tracing` (`-v` debug, `-vv` trace;
   `RUST_LOG` overrides both), then calls `bootstrap::run`.
2. `bootstrap::run`: with `--show`, hands off to a running instance and exits
   before loading anything. Otherwise it enables DPI awareness, loads or
   creates `Config` (migrating older files; `--reset` backs up the old file
   and refuses to start if the backup fails), folds legacy per-space overrides
   into looks (saving if anything changed), seeds a "Desktop" space if there
   are none (filled from the desktop folder unless `--no-icons`), and calls
   `deskspace_platform::run`. Placement is `--placement` when given, else
   top-level. `--show` overrides `--hidden` and start-hidden.
3. On `Started`, the app sets the menu/settings dark mode, registers every
   binding in `HOTKEY_BINDINGS`, syncs the desktop "Show spaces" verb and the
   start-with-Windows Run value, reads Explorer's display options and the
   desktop items, applies takeover, applies start-hidden, creates one window
   per space with its style and scene, re-flows every space, prunes items
   whose files are gone, and optionally opens settings (`--settings`).
4. Pointer events update selection/hover (re-pushing that space's scene), drag
   spaces via `SetSpaceRect` (moving the window), resize them via
   `ResizeSpace` when grabbed on the border band, scroll them on the wheel
   (per-space offsets live in the app, not the config), toggle roll-up on
   title-bar double-click, and open the space menu on right-click. Items open
   on double-click, show the item menu on right-click, and start a shell drag
   past the system drag threshold. Drops add items (moving new files into
   storage via `storage::ItemStore`) or move them between spaces. Config is
   written. Configuration writes are debounced: drags, resizes, roll-up, lock,
   and space creation schedule a save that fires about 700 ms after the last
   change, while file-moving operations (drops, item removal, delete) and
   Apply/OK save at once, and closing flushes any pending write.
5. Settings edits apply at once as a preview: global edits to the live config,
   a customized space's edits through `SetSpaceLook`. Repainting the spaces is
   deferred to `PREVIEW_TIMER`, which only fires once the message queue is
   idle, so a slider drag collapses into one restyle or re-flow per pause
   (all spaces for a global edit, one for a space edit). Apply/OK validate and
   re-register every hotkey, sync start-with-Windows, apply or remove
   takeover, and save; Cancel or closing the window restores the saved theme
   and window settings and every space's snapshot (`settings::needs_relayout`
   / `needs_restyle` decide what to redo).
6. On `CloseRequested`, the app unregisters every hotkey, cancels a rename,
   discards unapplied settings edits, flushes the save (`flush_save`), and
   removes the desktop verb. The platform teardown then restores Explorer's
   icons, signals the watchdog's clean-exit event, and releases COM.

## 8. Controls (current)

- Drag a space title bar to move it (snaps to the grid).
- Drag a space edge or corner to resize it; the wheel scrolls overflowing
  content, and Ctrl+wheel changes the global icon size (10 % a notch). Under
  the default "Match Explorer's display" preset Explorer's icon size wins, so
  Ctrl+wheel has no visible effect there yet. ROADMAP D9 decides it should
  change the desktop's own icon size instead, as Ctrl+wheel on the desktop
  does (`TASKS.md` F13).
- A right-button press on an item selects it; holding it and dragging past
  the drag threshold starts a shell drag (ROADMAP D9, `TASKS.md` G5a), which
  never rearranges inside the space, so a right-drag onto Explorer or the
  desktop shows the shell's own Move / Copy / Create shortcut menu. A right
  drag onto a space shows DeskSpace's own Move here / Copy here / Show here
  without moving / Cancel menu (G5b): "Copy here" copies the files into the
  store and leaves the originals, "Show here without moving" lists them where
  they are, and Cancel drops nothing. A right click without moving still
  opens the item or space menu. The "Create shortcuts here" entry (G5c) is
  still open.
- Double-click a space title bar to roll it up or down.
- Rename a space from its right-click menu; the title is edited in place
  (Enter commits, Escape cancels, clicking away commits).
- Right-click a space (`App::show_space_menu`, `app/menus.rs`): Roll up/down,
  Rename, Lock position, Fit to contents (resized, unlocked spaces only);
  New shortcut ▸, Appearance ▸, Sort by ▸; Delete space (asks first unless the space is
  empty); New space; Settings…, Hide/Show spaces, Exit DeskSpace.
- The space menu's Appearance submenu sets a tint-opacity preset (100/80/60/
  40/20 %), an icon size (Small 75, Medium 100, Large 150, Extra large 200 %),
  or a custom tint or text color. Each of these customizes the space,
  starting from what it shows now; "Use global appearance" (greyed unless the
  space is customized) makes it follow Global again (keeping its look for
  later), and "Space settings…" opens the settings window on that space. Sort
  by reorders items by name (A–Z or Z–A), type, size (smallest or largest),
  or date (oldest or newest). All three submenus are built from tables in
  `app.rs` (`OPACITY_PRESETS`, `ICON_SIZE_PRESETS`, `SORT_PRESETS`).
- The space menu's New shortcut submenu (F-53, F-54; `NEW_SHORTCUT_ENTRIES` in
  `app.rs`, also enabled for a locked space) offers "From installed
  apps…", a picker listing every app in the Applications folder (Store apps
  included) with a check box per row and a search box, "Custom
  shortcut…", a dialog for a target (file, folder, program, or web address,
  with Browse…), optional arguments, and a name that follows the target until
  edited, and (after a separator) "Paste shortcut", which is enabled when the
  clipboard holds shell items (`Ui::clipboard_shell_items`) and leaves the
  clipboard, its files, and any cut ghosting untouched. A typed path is
  resolved as the Run dialog does (`%VARIABLES%`, a
  bare program name on the search path), and a target that does not exist is
  reported in the dialog. A web address becomes a `.url`; everything else a
  `.lnk` (a copied `.lnk` or `.url` is duplicated into the store). All three
  end in `App::add_shortcuts`, which writes into the space
  storage and lists the shortcuts at the end of the space. The same choices
  open from the + tile on a space's Settings page "Shortcuts", whose preview
  also deletes an item from its X (after asking).
- The space menu's "Add files…" (F-55) picks files in the standard Open
  dialog (`deskspace_platform::Ui::pick_files`, an `IFileOpenDialog` with
  multi-select, started on the Desktop folder) and moves them into the space
  through the same `App::add_dropped` path a drop uses (ROADMAP D3), so a
  picked file is relocated into the space storage exactly like a dragged one.
  Cancelling adds nothing.
- In Settings, pick Global or a space in the left pane. Global edits apply to
  every space that is not customized. A space's pages are greyed, showing the
  global values, until "Customize this space" is ticked; its edits then apply
  to that space alone. A space's header also has a "Delete space" button.
  Selecting a space highlights it on the desktop for about 1.2 s, so it can
  be found.
- Settings previews live across its Appearance, Labels, Layout, Colors, and
  Advanced pages: material, corners, tint, custom colors, the Windows accent for
  the selection and border, the selection box color and border, label lines/size/shadow,
  icon size/spacing/edge padding (which re-flow every space; icon size scales
  the icon and the grid cell around it), and the shell overlay size (the shortcut
  arrow badge, as a percent of the icon; default 80 %, 100 % matches Explorer).
  Icon size defaults to 100 %, which is
  Windows' 48 px desktop icon, and can also be typed as a percentage. The
  Layout page also has a "Fit to contents" button (grey unless a resized,
  unlocked space is selected), which commits at once like the menu entry.
- The pages and their fields (`settings/pages.rs`, one builder function per page):

  | Page | Scope | Fields |
  |------|-------|--------|
  | Appearance | look | Color mode, Background material, Corner radius, Tint opacity, Title bar shading, Show window border, Item preset |
  | Labels | look | Label lines, Label size, Label shadow |
  | Layout | look | Icon size, Icon spacing, Edge padding, Fit to contents |
  | Colors | look | Use custom colors, Background tint, Text, Use Windows accent, Selection box color, Show selection box border, Restore default / Match global appearance |
  | Advanced | look | Shortcut overlay size |
  | General | Global | Start with spaces hidden, Snap to grid, Snap resized spaces to whole columns and rows, Start DeskSpace when you sign in, Take over the desktop, Settings and menus (theme) |
  | New spaces | Global | Default title, width, height, Start new spaces locked (affect only spaces created afterwards) |
  | Hotkeys | Global | Show or hide spaces, Create a new space, Open settings |
  | About | Global | Version, summary, license, configuration file path |
  | Shortcuts | space | A note, and a preview of the space: an X on each item (delete, after a confirmation) and a + tile (New shortcut) |

  "Look" pages appear for Global and for every space; Shortcuts only for a
  space; the rest only for Global.
- Tray icon: left-click (or keyboard select) opens Settings; right-click for
  Settings…, Hide/Show spaces, New space, Exit DeskSpace. A space created
  from the tray goes to the center of the work area, and creating a space
  while spaces are hidden shows them.
- Drop files from Explorer or the desktop onto a space to move them into it;
  hold Shift to leave them where they are and only show them in the space,
  or Alt to add a shortcut to them instead. A right drag onto a space shows a
  menu to move, copy, or just show the files (G5b). An app dragged from the
  Start menu or the taskbar (or any drag that offers only a link) creates a
  shortcut in the space, as Explorer does when one is dropped on a folder; a
  dropped shortcut file is copied, and its original stays where it was.
- Click an item to select it; Ctrl+click adds or removes it from the
  selection, and Shift+click selects the range from the last click. Drag from
  empty space to rubber-band select (Ctrl or Shift adds to the selection), and
  Ctrl+A selects every item in the current space. Clicking
  empty space in a space or the desktop clears the selection (hiding spaces and
  rolling up do too); clicking another application keeps it, as Explorer keeps
  a desktop selection. Clicking anywhere on a space acts like a
  click on the desktop: it focuses the desktop (deactivating the app in front)
  and clears the desktop icon selection, but leaves any selection inside a
  Explorer window alone.
- Double-click an item to open it. Clicking an already-selected item a second
  time (a slow double-click) starts an inline rename of it, as Explorer does.
  Right-click it for Explorer's context
  menu, with DeskSpace's "Rename" on top (Explorer's own Delete in it sends the
  file to the Recycle Bin; ROADMAP D3); right-clicking an item already in a
  multi-selection keeps it. A loose item on the desktop layer gets Explorer's
  menu alone.
  Rest the pointer on an item for a moment to see Explorer's info tip.
- Item check boxes follow Explorer's mode (when the desktop shows them and the
  `MatchExplorer` preset is on). A checked box is filled with the Windows accent
  and shows a white tick, as Explorer draws it (`Palette::check_fill`, the
  system accent or Windows blue when it is unknown), independent of the space's
  own selection colors; an unchecked box is a bordered light box. With
  auto-check-select, hovering reveals the box and
  it mirrors the selection; clicking an item ticks it and unticks the others,
  Ctrl+click ticks several, and clicking the box toggles one. With independent
  item check boxes, the boxes are always shown, clicking a box toggles a
  persistent tick and moves the highlight, the ticks survive empty-space and
  desktop clicks, and delete/copy/cut/drag act on the ticked items.
- Drag an item within its space to rearrange it (`ReorderItem`); dragging one
  member of a multi-selection moves the whole block (`ReorderItems`). The whole
  selection's drag image follows the pointer and every member's slot fades; the
  new order sticks until a Sort by reorders it.
- Drag an item out: into a folder or app like an Explorer item, onto another
  space to move it there, or onto the desktop to take it out of the space. When
  the grabbed item is part of the selection, the whole selection drags.
- Press Delete with items selected to send their files to the Recycle Bin
  (undoable, no prompt), as Explorer does; Shift+Delete removes them for good.
  The only ways a stored file goes back to the desktop are dragging it there
  and deleting its space.
- Cut, copy, and paste items with Ctrl+X, Ctrl+C, and Ctrl+V. Cut items are
  ghosted until pasted; paste goes into the current space, moving (cut) or
  duplicating (copy) the files through the shell's file operation.
- Press F2 with an item selected to rename its file inline (Enter commits,
  Escape cancels, clicking away commits); the extension is kept when the label
  hides it. With no item selected, F2 renames the current space's title.
  Alt+Enter opens the selected item's Properties, or DeskSpace Settings when no
  item is selected. With an item selected, Enter opens it, and the Menu key or
  Shift+F10 opens its context menu (the space's menu with no item selected).
  Escape clears the selection and F5 refreshes the spaces. With a space current,
  typing a letter selects the next item whose label starts with the typed
  letters (the buffer clears after about a second), and Home, End, PageUp, and
  PageDown scroll the space to the top, the bottom, or one page. The arrow keys
  move the selection across the grid (Shift extends, Ctrl moves the dotted
  focus rectangle only).
- The Settings → Hotkeys page binds the global hotkeys: show/hide spaces
  (`Ctrl+Alt+D` by default), create a new space at the pointer, and open
  settings (both unbound). While spaces are hidden, a "Show DeskSpace spaces"
  entry is added to the desktop's right-click menu (a per-user
  `DesktopBackground\Shell` verb) and removed once they are shown. It runs the
  executable with `--show`, which signals the running instance through its
  controller window instead of starting a second copy.
- CLI flags (`cli.rs`): `--config`, `--placement` (`top-level` |
  `desktop-background` | `desktop-foreground`), `--reset`, `--hidden`,
  `--show`, `--settings`, `--no-icons`, `--click-through`, `-v`/`-vv`, and
  the hidden development flags `--on-top`, `--takeover-layer`, and
  `--takeover`.

## 9. Feature parity

The feature list and its status live in one place: `ROADMAP.md` section 7
(the feature index). This section is kept so the section numbers cited
elsewhere stay stable.

## 10. Known limitations and technical debt

- **Desktop takeover is opt-in and new.** Explorer owns the desktop icon layer
  by default, and v0.2 keeps it that way (ROADMAP D8, D9). With "Take over the desktop" on (or the hidden `--takeover`
  flag), DeskSpace hides every desktop `SysListView32` and draws the loose
  items on its own layer; a clean exit and every startup restore the icons, and
  `TaskbarCreated` and a 5-second health timer re-hide them after Explorer
  restarts. A watchdog helper (`deskspace-watchdog.exe`, shipped beside
  `deskspace.exe` in the release zip) launched before the icons are hidden
  restores them if the process crashes (I6).
- **File moves have no undo yet.** Deleting a space returns its stored
  files to the desktop (deleting an item recycles it, which the Recycle Bin
  can undo); files from another drive stay in place (a move there
  would copy). The Desktop folder is watched while the layer is drawn, so the
  loose items follow outside changes; a stored file is not, so one deleted
  outside DeskSpace stays listed until an item action or the next startup
  notices it is gone, and an item's `size`/`modified` (used by sorting) are
  read once at creation, so they can go stale.
- **Blur relies on an undocumented API** (`SetWindowCompositionAttribute`).
  If a future Windows build removes it, spaces fall back to a translucent tint
  without blur. Rounded corners need Windows 11.
- **Dark mode relies on undocumented uxtheme exports** for menus and on
  Explorer's dark control styles. The confirmation message box stays light; it
  has no dark mode. The color picker is DeskSpace's own themed popup
  (`win32/color_picker.rs`), so it follows the dark/light mode (F-51).
- **Per-monitor DPI is scaled, with caveats.** Space layout, icons, and hit
  tests scale to each monitor's DPI and `WM_DPICHANGED` reflows a moved space.
  The settings window is still laid out for the DPI at creation, and an item
  moved between monitors of different DPI is positioned from the destination
  space's grid (correct, but not yet verified on screen).
- **Start-menu and taskbar drags are undocumented.** What they carry was
  found by observation (`TASKS.md` L1): Start offered `MOVE` and a
  `CF_HDROP` holding an AppUserModelID rather than a path, and the effects
  it offers vary by item. `drop_plan` and `is_file_list` handle what has
  been seen; a future Windows build may differ, and the L1 debug log is the
  way to find out. If DeskSpace runs elevated, Windows blocks every drop
  from Explorer and Start (UIPI).
- **Placement is fixed for a run.** Only `--placement` chooses it (the
  config setting was removed); there is no live switch, because recreating
  every space window would drop the blur. Top-level is the default and the
  only placement with blur.
- **Icon cache is invalidated only after shell menu commands and renames.** It
  is keyed by path, pixel size, and overlay size, so an icon-size or DPI
  change reloads automatically. A Desktop-folder change (`FolderChanged`)
  re-syncs the layer but does not invalidate icons, so a file whose icon
  changed in place keeps the old one; lens watchers will need to invalidate.
- **Scroll indicator is display-only.** Spaces scroll with the wheel,
  Home/End/PageUp/PageDown, and by keeping a keyboard-selected item in view,
  but the indicator cannot be dragged, and wheel events need Windows' "scroll
  inactive windows" setting.
- **Inline rename has no IME or clipboard support.** A low-level keyboard hook
  handles the active layout's printable characters and navigation keys only;
  composition (Japanese, Chinese, ...) is deferred. Space titles and item file
  names both rename inline; an item rename goes through the shell, so a name
  clash shows Explorer's conflict prompt.
- **Explorer display matching is broad but not total.** Item check boxes handle
  both Explorer modes: auto-check-select (the tick mirrors the selection) and
  independent item check boxes (a session-only tick set that is the operand for
  operations). Compressed/encrypted label colors are imported
  (`SSF_SHOWCOMPCOLOR`). Under the `MatchExplorer` preset the desktop's icon
  size (`IFolderView2::GetViewModeAndIconSize`) and item spacing
  (`IFolderView::GetSpacing`) are imported too, replacing the DeskSpace sliders,
  and icons are fetched at the effective pixel size with shell overlays, so
  large sizes are crisp; info tips match Explorer's. Explorer's view mode
  (list/details) is not mapped, only the desktop's own options are read (a
  check-box setting enabled in one Explorer window is not reflected). Matching is
  part of the look, so a customized space can turn it off for itself.
- **Selection is within one space.** Rubber-band (marquee) selection, Ctrl+A,
  and dragging the whole selection are implemented; a selection cannot span
  spaces, and Ctrl/Shift-dragging between spaces is not distinguished from a
  plain move.
- **Item drag is close to Explorer but not identical.** Dragging out defaults
  to a move (a preferred drop effect of `MOVE`), so it no longer leaves a
  `- Copy` on the desktop; cross-space moves still match by file identity, and
  drops onto a space go through the OLE `IDropTarget`. The drag image is the
  whole active set drawn as Explorer draws it (icons and labels at their grid
  positions, scaled to `MAX_DRAG_IMAGE` when large); drags that start in
  Explorer keep Explorer's own image. Explorer's Shift=move is deliberately
  inverted on DeskSpace targets, where Shift preserves the original. Dropping
  a group onto the desktop can still shuffle its layout: Windows 11 repacks
  desktop icons when "Align icons to grid" is on, and an Explorer drag does
  the same, so this is native desktop behavior rather than DeskSpace's.
- **Clipboard and delete match Explorer for files only.** Copy and cut
  advertise `CF_HDROP` and "Preferred DropEffect"; the richer shell formats for
  virtual items are not provided. Paste and delete use `IFileOperation` with an
  undo record, but stored files are not watched (only the Desktop folder is,
  for the layer), so an undo (or a move made outside DeskSpace) leaves a space
  item stale until an item action or the next startup prunes it.
- **The desktop "Show spaces" verb is a legacy menu item.** It is a per-user
  `DesktopBackground\Shell` entry, so on Windows 11 it appears under "Show more
  options" (Shift+right-click), not the new top-level menu. Reaching the new
  menu needs an `IExplorerCommand` handler inside an MSIX package with the
  `windows.fileExplorerContextMenus` extension, an associated stub executable,
  and signing (or Developer Mode) - deferred to M8 packaging. Its icon is the
  executable's embedded icon resource (`"{exe}",0`), not a file written beside
  the storage folder.
- **Packaging, signing, and auto-update.** The policy is ROADMAP D6 and the
  release steps are `RELEASING.md`; the facts that matter in the code:
  - CI (`ci.yml`) runs fmt, clippy with `-D warnings`, tests (all with
    `--locked`), a `cargo check` on the `rust-version` toolchain, and
    `cargo deny check all`. A `v*` tag matching the workspace version runs
    `release.yml`, which publishes `deskspace_<version>_x64-setup.exe` (NSIS,
    built by the CI-only `cargo-packager` 0.11.8 from
    `[package.metadata.packager]` in `crates/deskspace-app/Cargo.toml` and the
    vendored `packaging/nsis/installer.nsi`), a portable zip with the
    watchdog, and its `.sha256`.
  - The installer is per user, installs to `%LocalAppData%\DeskSpace.App`
    (apart from the stored files under `%LocalAppData%\DeskSpace`), and is
    unsigned until R10.
  - Auto-update (F-45, `deskspace-update`) is opt-in and installs the full
    package from a minisign-signed `latest.json`. Until the maintainer sets
    the `DESKSPACE_MINISIGN_KEY` secret and `UPDATE_PUBKEY`, releases ship
    without a feed, and the app greys out the tray's "Check for updates…" and
    the Settings update controls (`deskspace_update::is_configured`).
  - The toolchain is pinned in `rust-toolchain.toml`, and the release build
    is reproducible (`--remap-path-prefix` in `release.yml`, `/Brepro` in
    `build.rs`).
- **License is Apache-2.0** (copyright Timothy W. Brown; `LICENSE`,
  `NOTICE`). Release packaging must ship both files.

## 11. Glossary

- **Space**: a movable container window that groups items on the desktop.
- **Storage**: the folder files dropped into spaces are moved to.
- **Lens**: a space whose contents mirror a folder live (comparable tools
  call it a Folder Portal). In code, a space with a folder source. Deleting
  a lens removes only the view, never the folder (ROADMAP D3).
- **User-sized**: a space the user resized; it keeps its size, reflows items
  to its width, and scrolls instead of growing.
- **Roll-up**: collapsing a space to its title bar.
- **Auto-hide**: hiding all spaces and desktop icons until the desktop is
  interacted with.
- **Placement**: where space windows live: top-level windows or children of
  the desktop's Progman/WorkerW window.
- **Scene**: the renderer-agnostic draw list (`SpaceScene`) the app builds for
  one space.
- **Material / backdrop**: the blurred layer DWM draws behind a space's tint.
