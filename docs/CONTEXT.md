# CONTEXT

DeskSpace's architecture and current behavior; read it before making changes.
`ENGINEERING.md` has the workflow and engineering rules, `ROADMAP.md` the
plan, decisions (D1-D9), and feature status.

## 1. What DeskSpace is

A free and open-source desktop space organizer for Windows. It groups desktop
items into movable containers ("spaces") and, eventually, mirrors folders as
live "lenses" (comparable tools call these Folder Portals), with folding,
auto-hide, and per-space appearance control. The long-term goal is
feature-for-feature parity with comparable tools while staying native,
lightweight, and offline. The tree is an early but usable framework, not a
finished product. The direction is decided in `ROADMAP.md` section 2.

- **Goals:** native Win32 rendering and window behavior (no webview, no
  runtime); spaces live on the desktop, not above other apps; fast startup,
  low idle CPU, no telemetry; data-driven rendering and UI so app and platform
  stay decoupled; simple, forward-compatible configuration.
- **Non-goals (for now):** cross-platform support (the workspace is
  Windows-only), cloud sync or accounts, replacing Explorer.

## 2. Current status

- Build health is what CI reports; releases, progress, and feature status
  are `ROADMAP.md` sections 4 and 7.
- Verification: many behaviors are unit-tested but not confirmed on screen;
  `TASKS.md` section V lists them. Confirm one before building on it. What
  was already confirmed, and when, is in `archive/ROADMAP-progress-log.md`.
- History: commits `07dba93`..`18ffac2` replace the old overlay together and
  only build as a series; bisect across them as one step.
- Code health: a non-test file past the ~800-line guideline gets a K task in
  `TASKS.md` section K; section 3 lists the `app/` modules.

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

The rules that keep this shape (dependency direction, `unsafe` only in the
platform, data-driven UI) are `ENGINEERING.md` section 3.

### Crate responsibilities

`core`, `config`, `update`, and `app` are `#![forbid(unsafe_code)]`.

- `deskspace-core`: the pure domain: geometry, ids, items, spaces, desktop,
  layout engine, command layer, theme and palette. No OS dependencies, no IO.
- `deskspace-config`: the persisted schema (`Config`, `WindowSettings`),
  versioning and migration, and an atomic `ConfigStore` over TOML.
- `deskspace-update`: update checking and installation (ROADMAP F-45) on
  `cargo-packager-updater`; it depends on no workspace crate.
- `deskspace-platform`: the UI runtime and message loop, space windows, DWM
  styling, the Direct2D painter, tray, popup menus, the data-driven settings
  window, desktop-shell discovery, shell items and the icon cache, DPI, and
  global hotkeys.
- `deskspace-app`: two binaries. `deskspace` (`main.rs`) holds the `clap` CLI
  (`cli.rs`), startup and seeding (`bootstrap.rs`), the `EventHandler`
  (`App`, `app.rs`), model-to-scene conversion (`scene.rs`), the settings
  form and its mapping onto `Config` (`settings/`, pure), and the managed
  store for dropped files (`storage.rs`). `deskspace-watchdog`
  (`watchdog_main.rs`) is the takeover crash helper. `app.rs` keeps the `App`
  struct, `on_event`, the timer ids, the `menu` ids, and the preset tables;
  the rest of `App` is split across `app/`, one `impl App` block each:

  | Module | Owns |
  |--------|------|
  | `persist` | `dispatch`, `commit`, `save` / `schedule_save` / `flush_save` |
  | `effect` | `Effect` and `App::run`: a decision changes `App` and returns effects as data, and `run` carries them out against `Ui`, so tests assert what an event means (R17) |
  | `view` | resolving a space's look (`theme_for`, `grid_for`, `palette_for`, `layout_for`), `refresh_space`, `reflow_space`, what Windows reports (`SystemLook`), per-space scroll and icon factor (`ViewCache`), the desktop layer and takeover (`LayerState`) |
  | `hit` | surface lookups (`surface_item_at`, `surface_palette`, `find_item_by_file`) |
  | `selection` | `Selection`, keyboard selection (`on_navigate`, `on_typed`), `active_items` |
  | `pointer` | `PointerState`, mouse events, resize, hover, marquee |
  | `zoom` | icon-size zoom (`ZoomLevel`), Ctrl+wheel, Reset, and the zoom pop-up |
  | `drag` | `Drag`, drag-to-rearrange, the drag image, and drag out |
  | `rename` | `EditState`, inline rename of titles and items |
  | `files` | drops, delete, prune, `resync_desktop_layer` |
  | `clipboard` | cut / copy / paste |
  | `menus` | space, item, and tray menus, `create_space` |
  | `settings_session` | `SettingsUi`; opening, previewing, applying, and cancelling the settings window |
  | `visibility` | show/hide, the desktop "Show spaces" verb, start with Windows |
  | `hotkeys` | `Hotkeys`: registering, re-registering, and releasing the global hotkeys |
  | `info_tip` | the hover info tip |
  | `debug` | the debug window's panel and actions, and the log console's feed (ROADMAP D14) |
  | `update` | `UpdateState`, update checks, downloads, the quiet-moment install, `InstallOnExit` |

## 4. Domain model (`deskspace-core`)

### Geometry and ids

- `Point { x, y }`, `Size { width, height }`, and `Rect { x, y, width, height }`
  (with `contains`, `intersects`, `union`, `offset`, `inset`,
  `with_position`, `with_size`, `center`) are `Copy`, `serde`, `Default`.
- `SpaceId(u64)` and `ItemId(u64)` are transparent serde newtypes.
  `Desktop::next_space_id` / `next_item_id` give the next id above both the
  ids present and every id handed out this session, so an id freed by a
  delete is never reused while undo may restore it (ROADMAP D11). The
  session's highest ids are not saved; after a load the ids present stand in
  for them.

### Item

`Item` and `ItemKind` (`item.rs`; the fields are documented there) are a
path, a label, a kind, a grid position, and facts read once from the file
system.

`core` does no IO, so whoever creates an `Item` passes in the `ItemFacts` it
read once with `deskspace_platform::item_facts` (to `Item::new` or
`Command::AddItem`): `kind`, `size`, `modified`, `compressed`, and
`encrypted`. Nothing refreshes them afterwards; tests pass
`ItemFacts::default()`. `ItemKind::classify(path, is_folder)` is pure: a
folder, else by extension; a path with no extension is `Other`, and only
`with_kind` yields `Drive`. `label` defaults to the file stem. `position` is
the top-left of the item's grid cell **relative to the space's content
area**, so moving a space never touches its items.

### Space

`Space` and `SpaceLook` are in `space.rs`, where every field is documented.

A space follows the global `Theme` and grid spacing until it is customized.
`SpaceLook` is its own copy of every look setting: the whole theme (its
`ui_mode` unused) plus the grid's icon spacing and edge padding. It is seeded
from the global settings (`SpaceLook::from_global`) the first time and kept
when customization is turned off, so turning it back on restores it.
`active_look()`, `theme(global)`, and `grid(global)` resolve what a space is
drawn with.

Files from before `look` stored per-space `opacity`, `tint`, `text`, and
`icon_size`. They are still read (private, never written), and
`merge_legacy_overrides` turns them into a customized look at startup:
opacity becomes the tint opacity it produced, and a tint or text color turns
on custom colors, taking the other color from the palette shown at the time.

Helpers: `title_bar()`, `visible_rect()` (the title bar when folded, else
`rect`; used for window size and hit-testing), `content_rect()` (empty when
folded), `item_at(point, cell, scroll)`, `content_extent` / `max_scroll`,
`resize_edges_at(point, border) -> ResizeEdges` (none when locked or
folded; corners twice as wide as edges), item lookup/insert/remove, and
`set_position`.

### Desktop and desktop layer

`Desktop { spaces: Vec<Space>, layer: DesktopLayer, layout: GridSpec }` has
add/remove space, add/remove/move item, `space_item_count`, `space_at(point)`
(topmost hit on `visible_rect`), and `sort_by_id`. `move_item` validates the
target before removing the item, so a failed move never loses it, and resets
the item's position to the origin for the next re-flow to place. It only
takes items from spaces, so `Command::MoveItem` on a layer item is `Rejected`;
a loose item enters a space through the file-move path. `next_item_id` spans
the spaces and the layer, and `item` finds an item in either.

`DesktopLayer { items: Vec<Item> }` is the desktop's loose items: the
Desktop-folder files no space holds (ROADMAP D7). The shell is the source of
truth, so it is a refreshed cache (`Command::SyncDesktop` reconciles by path,
keeping `ItemId`s) with no chrome. A layer item's `position` is absolute, in
virtual-desktop coordinates. `Desktop::layer()` reads it.

### Layout engine (`layout.rs`)

`GridSpec` (fields documented there; defaults in its `Default`) is the grid
every space lays out on.

- `arrange_space` resizes the space to fit its items (unless locked or
  user-sized) and calls `position_items`, which lays items on the grid in
  content-relative cells: `columns` wide normally, `columns_for(width)` wide
  for user-sized spaces. It stamps `grid.title_height` onto the space, so a
  scale change reaches the title even for user-sized spaces. It does nothing
  for a locked space; `Command::Reflow` handles those (title height and
  positions only, size kept).
- `GridSpec` is the logical 96-DPI / 100 % base the config stores;
  `GridSpec::scaled(scale)` is the effective grid for a display. The app
  derives a per-space scale (the space's `Theme.icon_size` or Explorer's icon
  size, times the space's monitor DPI) and shares it through `layout_for(id)`
  / `metrics_for(id)`. The title bar takes the DPI alone
  (`GridSpec::scaled_with_title`), so a zoom never changes its height.
  Commands take the effective grid of the space under the action
  (`Desktop::apply_with_layout(command, layout, bounds)`; `apply` delegates
  with the stored base). Scene geometry, hit tests, and scroll all read the
  same per-space grid, so painter and pointer always agree. A customized
  space's base grid and theme are its own (`Space::grid`, `Space::theme`);
  under the `MatchExplorer` preset, Explorer's item spacing replaces the cell
  and gap outright.
- `resize_rect` moves the grabbed edges to a point, snapped, keeping opposite
  edges fixed, at least `min_space_size()` (one cell plus padding) and inside
  the work area. With `fit_to_grid` on (the default), a left/right resize then
  rounds the width to whole columns (`fit_rect_width` /
  `GridSpec::fit_width`) and a top/bottom resize the height to whole rows
  (`fit_rect_height` / `GridSpec::fit_height`), so a user-sized space keeps
  equal padding on both sides and above and below the remainder; the grabbed
  edge stays put. `scroll_step()` is one row per wheel notch.
- `snap_to_grid` / `snap_rect` snap to the grid, `clamp_to_bounds` keeps
  rects in the work area, and `resolve_collisions` pushes overlapping spaces
  down.
- `placement::new_space_origin` picks where a new space goes (ROADMAP D10).
  The app calls it in `App::create_space` and passes the result to
  `Command::CreateSpace`, which still only clamps, so the seed and tests can
  name an exact spot.

### Command layer (`command/`)

All mutations flow through `Desktop::apply_with_layout(command, layout,
bounds) -> CommandOutcome` (`Applied | NotFound | Rejected`;
`is_applied()` is the usual check), where `layout` is the effective grid and
`bounds` the work area of the monitor nearest the action (`work_area_at`).
`command/mod.rs` holds the types and a dispatch `match` whose every arm calls
a private `Desktop::apply_<variant>`; `space_ops.rs`, `item_ops.rs`, and
`layer_ops.rs` hold those by what they change, `sort.rs` the `SortSpace`
ordering, and `mod.rs` the shared helpers `with_space` (run a change on a
space or report `NotFound`), `rearrange`, and `close_gap`. The variants and
their fields are documented on `Command`; the rules below are the ones a
variant's doc cannot show.

- `CreateSpace { title, at, size, locked }` clamps to the work area (no
  snap). `MoveSpace`, `SetSpaceRect`, `ResizeSpace`, and `FitToContents`
  reject locked spaces and snap to the grid; `SnapAll` skips locked spaces.
- `ResizeSpace` also rejects folded spaces and an empty edge set, marks
  the space user-sized, and (with `fit_to_grid`) fits to whole columns and
  rows. `FitToContents` clears the user size, so the space grows to fit its
  items; `KeepSize` sets it without moving the rect. The app sends `KeepSize`
  before re-flowing a self-sizing space whose icon factor changed
  (`App::keep_size_on_zoom`: a zoom, the icon-size setting, a preset, or
  Explorer's own size, never a DPI move), so an icon-size change keeps every
  space's size and re-flows its columns, as an Explorer window does. Settings
  Cancel lets the spaces its preview pinned fit their items again
  (`Session::pinned`).
- `SortSpace { id, key: SortKey, descending }` sorts by `Name`
  (case-insensitive), `Kind` (folders, files, shortcuts, drives, other),
  `Size`, or `Modified`.
- `CustomizeSpace { id, on, seed }` turns a space's own look on or off,
  seeding it from `seed` (the global settings) only when it has none.
  `SetSpaceLook { id, custom, look }` sets both exactly (clamped): settings
  edits pass `custom: true`, and Cancel uses it to restore a snapshot.
- `RestoreSpaces { spaces }` is undo's and redo's way back (ROADMAP D11): it
  puts recorded spaces back exactly, each at its recorded index, without
  applying the layout, and changes nothing if any item id would clash.

### Theme (`theme.rs`)

`Theme` holds every look setting. Each field is documented in `theme.rs`,
its default is in `Theme::default`, and its range is the `*_RANGE` constant
beside it.

- `Preset` bundles the item-display choices: `MatchExplorer` mirrors
  Explorer's icon size, spacing, check boxes, and extensions; `Custom` uses
  DeskSpace's own item settings (`Theme::match_explorer()` reports which). It
  is the Appearance page's "Item preset" dropdown, and the Layout page's
  icon-size, icon-spacing, and edge-padding fields are greyed under
  `MatchExplorer`. More presets and import/export are planned (ROADMAP F-48).
- Ranges live beside their field (`theme.rs`); `GAP_RANGE` and `SNAP_RANGE`
  are in `layout.rs`, `NEW_SPACE_SIZE_RANGE` in `deskspace-config`'s
  `schema.rs`. Use the constant, never a literal range.
- `Theme::palette(system_dark) -> Palette` resolves concrete colors from the
  Windows 11 dark (`#202020` / white) or light (`#F3F3F3` / `#1A1A1A`) base,
  or the custom colors: `dark`, tint with alpha, title fill, title and item
  text, hover/selected fills, `marquee` (the opaque selection box color),
  border, `check_fill`, and `edit: EditColors` for the rename field (which
  follows the system mode, not the space's). `ThemeMode::resolve(system_dark)`
  decides dark or light; `mode` (through `Theme::is_dark`) colors the spaces,
  and `ui_mode` drives `App::ui_dark` and `Ui::set_ui_dark` for DeskSpace's
  own chrome.
- `Palette::with_accent(accent)` replaces only `selected_fill` and `marquee`
  (made the opaque accent) with the accent at their current alphas.
  `Theme::palette_with_accent`, which `App::surface_palette` (`app/hit.rs`,
  via `palette_for`) calls with `deskspace_platform::system_accent_color()`
  (the DWM `AccentColor` registry value, `0xAABBGGRR`), applies it when
  `use_accent` is on. The border takes the accent only with
  `BorderSource::Accent`; by default it is white in dark mode and black in
  light mode, as Explorer's are, whatever the text color. Whenever an
  accent is known it also sets `check_fill` to it, opaque, whatever
  `use_accent` says, because Explorer's check boxes always use the accent.
- `Backdrop::Solid` forces an opaque tint; `Backdrop::Clear` has no blur and
  lets the tint reach 0 %.
- Enums serialize snake_case. Old spellings still load: `Backdrop::Blur`
  accepts `"mica"`, `corner_radius` reads `corners = "round" | "small" |
  "square"` ("round" and unknown strings load as 8 px, `corner::WINDOWS`), and
  `preset` reads `match_explorer = true|false`.

### Animations (`animation.rs`)

What each kind of moment plays (ROADMAP D12), as data. `EFFECTS` and `KINDS`
are the two tables: an effect's row names the options it uses, and a kind's
row the effects and options it offers, the kind it may follow ("Same as new
space"), and whether it has the startup controls. `Animations` holds one
`AnimationSettings` per kind in `WindowSettings`; `resolve(kind)` follows
"same as" to an effect, and `clamped` replaces an effect a kind does not
offer, or one from a newer version, with the kind's default.

## 5. Configuration (`deskspace-config`)

`Config { version, desktop, theme, window }` and `WindowSettings`, the
non-look settings, are in `schema.rs`, with each field documented and its
default in `Default`.

- A missing `version` key reads as 0. `Config::migrate`
  upgrades version 0/1 files: it resets the theme (v1 colors were opaque and
  tuned for the old color-keyed overlay), clears each space's v1 overrides
  (`clear_legacy_overrides`: opacity, which was a fill alpha, tint, text, and
  icon size), and re-runs `position_items`, because v1 stored absolute item
  positions. Every load also clamps the global theme (`Theme::clamped`), the
  grid (`GridSpec::clamped`: gap, padding, snap), the new-space size
  (`WindowSettings::clamped`), and every space's look (`SpaceLook::clamped`),
  so a hand-edited out-of-range value loads within bounds.
- `WindowSettings`, `GridSpec`, `Theme`, `SpaceLook`, and `DesktopLayer` use
  struct-level `#[serde(default)]`, and `Item.kind`, `position`, `size`,
  `modified`, `compressed`, and `encrypted` default, so a partial table fills
  in defaults instead of failing the file. Unknown keys (for example the
  removed `background_alpha`) are ignored.
- `ConfigStore` resolves its paths through `directories::ProjectDirs`
  (`%APPDATA%\DeskSpace\DeskSpace\config\deskspace.toml`) or takes an explicit
  path. `storage_dir()`, where dropped files are moved, is
  `%LOCALAPPDATA%\DeskSpace\DeskSpace\data\storage`, or a `storage` folder
  beside an explicit config file, so scratch configs never touch real
  storage. `save` writes a sibling `*.toml.tmp` and renames it, so a crash
  never truncates the config.
- `load_or_default` returns defaults when no file exists. An unparsable file
  is copied to `deskspace.toml.unreadable-<millis>.bak` before falling back to
  defaults; a file from a newer version is loaded but first copied to
  `*.newer-<millis>.bak`, because saving would drop unknown fields. `--reset`
  copies the file to `*.reset-<millis>.bak` and refuses to start if that copy
  fails.
- `window.start_with_windows` is applied through
  `deskspace_platform::set_start_with_windows` (`win32/autostart.rs`), which
  writes or deletes the value `DeskSpace` under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (the quoted
  `current_exe()` path). The app writes it on Apply when the value changed and
  once at startup when enabled, so the path follows a moved install; a failure
  is logged at `warn` and never blocks Apply. The uninstaller removes it
  (`packaging/nsis/installer.nsi`).
- `window.take_over_desktop` (M2, opt-in; ROADMAP D7-D9) hides Explorer's
  desktop icons and draws the loose items on DeskSpace's own layer. Turning
  it on (or the hidden `--takeover` flag) also draws the layer; a clean exit
  and every startup restore the icons, and `TaskbarCreated` plus a 5-second
  health timer re-hide them after Explorer restarts. The `deskspace-watchdog`
  helper restores them after a crash (section 6). Apply turns it on or off
  at once.

## 6. Platform layer (`deskspace-platform`)

### Module map

Crate root (`src/`): `lib.rs` (re-exports; every module is private, so the
app imports only from here), `input.rs` (`Event`, `SpaceEvent`,
`SettingsEvent`, `TrayEvent`, `NavKey`, `SelectionKey`, `ClipboardAction`,
`ZoomAction`), `ui.rs` (`MenuEntry`, `SettingsForm` and its parts,
`CursorShape`), `edit.rs` (`TextEdit`), and `error.rs` (`PlatformError`).
`render/`: `mod.rs` (`Icon`, `ItemMetrics`, check-box geometry), `scene.rs`
(`SpaceScene`, `SpaceStyle`), `zoom_bar.rs` (`ZoomBar`, the zoom pop-up's
pure layout and hit test), and `d2d/` (the painters). Everything with
`unsafe` is under `win32/`:

| Area | Modules |
|------|---------|
| Runtime and controller | `runtime`, `controller`, `tray`, `console`, `hotkey`, `outside_click` |
| Windows | `window_class` and `window_state` (class registration and `GWLP_USERDATA` state; every window class uses both), `space_window`, `dwm`, `composition`, `drag_ghost`, `info_tip`, `menu`, `settings_window`, `debug_window`, `color_picker`, `dark_mode`, `dialog`, `app_picker`, `app_catalog`, `shortcut_dialog`, `zoom_bar` |
| Input | `keyboard` (the low-level hook), `drop_target` |
| Shell | `desktop_items`, `icons`, `shell_items`, `shell_view`, `shell_selection`, `display_settings`, `clipboard`, `file_op`, `drag_image`, `folder_watch`, `shortcuts` |
| Desktop takeover | `desktop_shell`, `desktop_icons`, `watchdog`, `desktop_entry` |
| System | `system`, `dpi`, `autostart` |

### Message, timer, and id tables

Ids that other code, docs, or another process cite are never renumbered. A
new one takes the highest value in use plus one; no doc keeps a "next free"
value, because the code shows it. Each list has a uniqueness test or an
exhaustive match:

| Ids | Where | Guard |
|-----|-------|-------|
| `WM_APP + n` messages to the controller | `win32/messages.rs` (each says who posts it; `TaskbarCreated` is a registered message outside the range) | the `ALL` list in its tests |
| App timers | `enum Timer` in `deskspace-app/src/app.rs`, delivered as `Event::Timer` and set through `App::set_timer` / `clear_timer`; each delay is a constant in the module that starts it | `Timer::ALL` and the exhaustive `App::on_timer` |
| Menu ids | `enum MenuId` in `deskspace-app/src/app.rs`, below `0x8000` (`shell_items::SHELL_FIRST_ID`, where the shell's item-menu ids start); a chosen id becomes a `MenuId` through `MenuId::from_id` | `menu_ids_round_trip` and `menu_ids_stay_below_the_shell_range` |
| Settings field ids | `settings/ids.rs`'s `mod field` | `field_ids_are_unique` |

Page indices (`mod page`) are build positions, so a new page goes at the
end; `settings::listed_pages` says which pages a scope lists.

### Events

`Event` (`input.rs`) is everything the app hears, with `SpaceEvent` (a space
or the desktop layer) and `SettingsEvent` (the settings window) nested in
it; each variant is documented there. A new input source adds a variant and
one arm in `App::on_event`, whose match is exhaustive.

### Runtime (`win32/runtime/`)

- `run(RunOptions, Box<dyn EventHandler>)` initializes OLE (single-threaded
  apartment) and common controls, finds the desktop host for the placement,
  creates the controller window, adds the tray icon, installs the console
  handler, delivers `Event::Started`, and pumps messages until quit, routing
  `IsDialogMessageW` to the settings window for Tab/Enter/Escape. Release
  builds are GUI-subsystem (`#![windows_subsystem = "windows"]` on both
  binaries), so the installed app never shows a console; debug builds keep it
  so `-v` logging and Ctrl+C work.
- `EventHandler::on_event(Event, &mut Ui)` is the app's single entry point for
  every window. `Ui` creates and destroys space windows, pushes rects, scenes,
  and styles, shows and hides spaces, shows popup menus and confirmation
  boxes, opens, updates, and closes the settings window, registers hotkeys,
  arms and cancels timers, sets the tray tooltip, and quits.
- Re-entrancy: the handler is moved out of the thread-local runtime while it
  runs. Win32 calls it makes (SetWindowPos, DestroyWindow, TrackPopupMenu,
  the color picker, MessageBox) re-enter window procedures; events raised
  then are queued and delivered by the outermost `dispatch`. Runtime state is
  borrowed only in short sections (`with_runtime`) that never call into Win32.

### Controller (`win32/controller.rs`, `win32/tray.rs`, `win32/console.rs`)

- A hidden top-level window (not message-only, so it receives broadcasts).
  It owns hotkeys (`WM_HOTKEY`), timers (`Ui::set_timer` / `clear_timer`, via
  `WM_TIMER` to `Event::Timer`), the tray callback (`NOTIFYICON_VERSION_4`:
  left click `TrayEvent::Activated`, right click `TrayEvent::MenuRequested`,
  keyboard select (`NIN_KEYSELECT`) opens Settings), `TaskbarCreated` (re-adds
  the icon after Explorer restarts), `WM_SETTINGCHANGE` "ImmersiveColorSet"
  (`Event::SystemThemeChanged`), and session end.
- Outside clicks (`win32/outside_click.rs`): spaces are never activated, so a
  `WH_MOUSE_LL` hook is the only way to see clicks on the desktop or other
  apps. While the app asks for it (`Ui::watch_outside_clicks`: an item is
  selected, a rename is active, or a surface is current), the hook posts each
  button press to the controller, which raises
  `Event::PointerDownOutside { on_desktop }` unless the press hit a space or
  a popup menu. A press on the desktop layer counts as outside, because
  `is_space_window` checks only the spaces.
- Graceful exit (ROADMAP D13): Exit and the update install call
  `App::exit`, which hands `Ui::quit_after` each shown space's Startup Out
  (reverse reading order, the stagger shrunk to fit `EXIT_LIMIT`). The
  spaces play it; the last one to finish (`runtime::space_left`), or a
  thread timer at the limit, posts the same `WM_CLOSE` as `Ui::quit`, so
  the teardown, and the takeover restore after it, run only once the
  spaces have left. A second Exit while leaving closes at once.
- `Ui::quit`, console Ctrl+C or close (`console.rs` posts `WM_CLOSE` and waits
  up to 4.5 s; meaningful only with a console attached, as in debug builds),
  and `WM_ENDSESSION` all lead to exactly one `Event::CloseRequested`; then
  the space and settings windows are destroyed, the tray icon removed, and
  the loop ends.
- Desktop verb (`win32/desktop_entry.rs`): while spaces are hidden, the app
  writes a per-user `DesktopBackground\Shell` verb, "Show DeskSpace spaces",
  that re-launches the executable with `--show`. On start, `--show` posts
  `SHOW_SPACES` to the running instance's controller window (found by class)
  and exits, or starts with the spaces shown when none is running. The verb is
  removed when spaces are shown and on a clean exit.
- The tray and settings-window icon is the `.ico` that `deskspace-app`'s build
  script embeds (`winresource`, from `assets/deskspace.ico`). The tray loads
  resource 1 at the small-icon size (`tray::load_app_icon`), and the settings
  window reuses that handle (`tray::shared_icon`).

### Space windows (`win32/space_window.rs`, `win32/dwm.rs`)

- One window per space, created by `Ui::create_space`, with its state in a
  `Box<SpaceState>` behind `GWLP_USERDATA`, freed in `WM_NCDESTROY`.
  `set_style`, `set_rect`, and `set_scene` return early when the value is
  unchanged (the style is kept in the state, the rect read back from the
  window, the scene compared with the stored one), so restyling every space
  on a settings edit only touches the ones that changed.
- **The desktop layer** (`Ui::create_desktop_layer`,
  `set_desktop_layer_rect` / `_scene`, `destroy_desktop_layer`) reuses this
  window with a `chrome: false` style. It spans the
  virtual desktop (`system::virtual_desktop_rect`) and is a `WS_CHILD` of the
  host when one is in use, else top-level and kept at the bottom, behind the
  spaces. Only the layer pins to `HWND_BOTTOM`; on every z-order change a
  top-level space goes directly above it (`space_window::insert_after`).
  When both pinned to the bottom, an outside z-order change could leave a
  space under the full-screen layer, which then took its clicks and refused
  its drops. The app draws the loose items on it
  (`scene::desktop_layer_scene`, from `DesktopLayer`) while
  `App::layer_active` holds: takeover is on (`take_over_desktop` or
  `--takeover`), or `--takeover-layer` is set; only with `--takeover-layer`
  alone does Explorer keep drawing its own icons underneath (a development
  view). The layer is created visible and, by design (ROADMAP D9), not hidden
  by `Ui::set_spaces_visible`; hiding both is M5's hide-all (F-11).
  - Item interaction is shared, not duplicated (ROADMAP D7): hit-testing,
    selection, hover, rubber-band, drag, drop, info tips, and the scene build
    take a `Surface` (`Space(id)` or `Desktop`), and the `surface_*`
    accessors in `app/hit.rs` resolve a surface's items, rect, layout,
    palette, and cells. New item behavior takes a `Surface` and uses them. The
    layer window's event id is 0 (`DESKTOP_LAYER_ID` in `app.rs`); real
    spaces start at 1.
  - On the layer, a click selects and opens loose items, the wheel is ignored,
    and dragging a loose item repositions it
    (`Command::SetDesktopItemPosition`); dragging it over a space starts the
    normal shell drag, so dropping it there moves the file into that space
    and the layer reconciles from the shell. Space-only gestures (resize,
    move, fold, scroll, inline rename, delete, clipboard paste) stay keyed
    on a space; a loose item's menu is Explorer's alone, an empty spot gets
    the desktop menu (section 8), and Delete does
    nothing on the layer. A drop on the layer opens a new space at the drop
    point and drops into it (`App::drop_onto`; ROADMAP D7, amended
    2026-10-17); a drop that adds nothing closes that space again.
- **Animations** (ROADMAP D12; `win32/space_animation.rs`): a window keeps
  its running `Animation` in its `SpaceState` and a timer of its own
  (`space_animation::TIMER_ID`, on the window, not the controller). Each
  tick moves the window to the frame's rect (hidden while it is empty) and
  repaints through the frame; the app is not called per frame. A request
  that finds an animation heading the other way reverses it, `set_rect`
  while animating moves its target instead of the window, and a window on
  its way out refuses drops (`DragKind::None`). `Ui::set_spaces_visible`
  and `Ui::fold_space` take the `Play` the app built (`app/animation.rs`);
  the accent blur is there for the whole show and gone at the end of a hide,
  while a composition space's follows each frame (D12, "The blur"). With Windows' "Transparency effects" off, Windows
  draws the material flat, which left a dark box under a fading space, so
  while a frame leaves the window see-through (`TransitionFrame::
  covers_window`: Fade, Pixelate, Gradient, Dissolve, Burn) the material
  steps aside, and comes back when the animation ends. The border DWM
  draws steps aside then too, whatever the switch, since it cannot fade
  either (`space_window::Backing`, `show_backing`). A fold is a Roll between the space's rect and its title
  bar: `App::toggle_fold` runs `Effect::FoldSpace` before the repaint, whose
  `set_rect` to the same rect then leaves the roll be, and a folding window
  keeps drawing its unfolded scene (the folded one has no items) until the
  fold ends (`Animation::defer`). Hiding a space mid-fold ends the fold
  first. A new window is created hidden; `App::sync_windows` (the one place
  space windows are made) pushes its scene, then `Ui::show_space` brings it
  in with Startup's play, staggered at startup only (`sync_windows_at_startup`,
  `startup_delays`). A window waiting its turn stays hidden, blur included.
  Every show goes through `space_window::show`, which paints the window
  once more about 100 ms later: the first frame presented right after a
  show is sometimes lost, which left a space blank (blur only) until a
  hover repainted it.
  A deleted space's window leaves the runtime's spaces map at once, so it
  is gone to every lookup, and waits in `Runtime::dying` while it plays
  Delete's Out (a space still arriving turns around instead); its input no
  longer reaches the app, it refuses drops, and it destroys itself at the
  end, or with the rest at teardown.
- **Top-level placement:** `WS_POPUP` + `WS_EX_TOOLWINDOW |
  WS_EX_NOACTIVATE`, kept at `HWND_BOTTOM` through `WM_WINDOWPOSCHANGING`, and
  never activated (`MA_NOACTIVATE`). A space ignores `WM_SYSCOMMAND` /
  `SC_MINIMIZE`, so Show Desktop cannot minimize it: survival never depends on
  the shell's undocumented tool-window exclusion, and the accent blur is never
  torn down. `dwm::apply_style` extends the frame over the client area, sets
  dark mode, corner preference, and border color, and turns on the blur with
  the undocumented accent policy (`SetWindowCompositionAttribute`,
  `ACCENT_ENABLE_ACRYLICBLURBEHIND` / `ACCENT_ENABLE_BLURBEHIND`). Clear uses
  a fully transparent `ACCENT_ENABLE_TRANSPARENTGRADIENT`, because disabling
  the accent after a blur turned the space black until it was recreated. The
  documented `DWMWA_SYSTEMBACKDROP_TYPE` was tried first and does not work:
  system backdrops render only on active windows.
- **Composition** (`win32/composition.rs`, ROADMAP D12, TASKS T12): on
  Windows 11 a top-level space is made with `WS_EX_NOREDIRECTIONBITMAP`
  and shows a visual tree instead of the accent blur: a root clipped to the
  space's corners, a sprite with the host backdrop brush (shown for Acrylic
  and Blur; `DWMWA_USE_HOSTBACKDROPBRUSH`), and a sprite showing the drawing
  surface the painter draws into. The rest of `dwm::apply_style` (corners,
  dark mode) applies as before. `composition::start` decides once,
  at startup, and logs it: Windows 10, "Transparency effects" off (the host
  backdrop ignores the switch, T1), desktop-hosted or click-through spaces,
  and a failure to start keep the accent blur, and the desktop layer always
  does; a window whose tree cannot be built is made again with it. The
  compositor and its devices are per thread, released in the teardown
  before `OleUninitialize`. While a frame leaves the window see-through,
  the painter also draws its opacity and mask into a second surface, the
  blur's mask, and after that paint the backdrop sprite swaps its brush for
  an effect brush that shows the backdrop through the mask's alpha
  (`win32/alpha_mask.rs`, D2D's AlphaMask; T13). At rest it shows the plain
  backdrop brush again, so an idle space costs no effect.
- **Desktop-hosted placement** (`--placement desktop-*`, a visible CLI option
  with no config setting since the "Keep spaces on the desktop" toggle was
  removed): a `WS_CHILD` of Progman / WorkerW, painted opaque and rounded with
  a (non-anti-aliased) window region. With no host found, `run` warns and
  falls back to top-level.
- **Corners and border:** DWM rounds blurred and Solid spaces with square,
  4 px, or 8 px corners (`SpaceStyle::dwm_draws_frame`). The painter rounds
  every Clear space, Solid spaces with other radii, chrome-less windows, and
  hosted spaces (anti-aliased fills); hosted spaces also get a window region
  so the opaque fill does not show past the corners (`Frame` in
  `space_window.rs`). The painter draws every space's border, never DWM
  (`dwm::BorderOwner`), so the border goes through an animation's opacity
  and mask with the rest of the space. Where DWM rounds, the border follows
  DWM's corners, which DWM scales with the window's DPI; Windows 10 has no
  DWM corners, so there the border is square. The zoom pop-up still takes
  DWM's border. The blur fills the whole window rectangle
  and ignores both the window region and the `DwmEnableBlurBehindWindow` blur
  region (tried on build 26300), so Acrylic and Blur spaces use the nearest
  DWM radius at or below the chosen one (`Theme::effective_corner_radius`).
  Larger blurred radii need a composition backdrop (M5).
- **Mouse input** arrives as `SpaceEvent`s in desktop coordinates;
  `TrackMouseEvent` provides `MouseLeave`, and capture is held from button
  down to the last button up. Ctrl and Shift come from the message's `MK_*`
  flags (`space_window::modifiers`), never `GetKeyState`, which reports the
  state as of the last keyboard message the thread read, and a space's thread
  reads none (Ctrl+wheel did nothing until this was fixed on 2026-10-16).
  `WM_MOUSEWHEEL` (screen coordinates, with the Ctrl state) becomes
  `SpaceEvent::Wheel`; it reaches never-focused spaces through Windows'
  "scroll inactive windows" setting (on by default). Ctrl+wheel calls
  `App::adjust_icon_size` (section 8 and ROADMAP D9 give the behavior; under
  "Match Explorer's display" it sets the desktop's own size through
  `set_desktop_icon_size`, `IFolderView2::SetViewModeAndIconSize`).
- **Resizing** is app-driven like moving: the app hit-tests the edges and sets
  the pointer with `Ui::set_space_cursor(CursorShape)`, which the window
  applies at once (`WM_SETCURSOR` is not sent while the mouse is captured)
  and on `WM_SETCURSOR`. The border band is 6 px on edges and 12 px at
  corners; locked or folded spaces cannot be resized. A press on a border
  resizes only once the pointer leaves a half-cell dead zone around the grab
  (`pointer::resize_edges_now`), so a click or a tiny drag never resizes, and
  each axis joins separately: a corner grab that moves on one axis resizes
  only that axis.
- `--on-top` (hidden CLI flag) makes spaces topmost so their look can be
  checked without uncovering the desktop.

### Drops (`win32/drop_target.rs`)

- Every space and the desktop layer is an OLE `IDropTarget`
  (`RegisterDragDrop`, revoked in `WM_DESTROY`), not a `WS_EX_ACCEPTFILES`
  window, so it chooses the drop effect and the shell shows the move/copy
  cursor. `drag_kind` says what a
  drag carries (`DragKind`: real files, shell items, or nothing), and the pure
  `drop_plan` decides what a drop does (`DropPlan`) and which effect to
  report. Every `DragEnter` logs the allowed effects, keys, kind, and offered
  formats at `debug`.
- A file drop becomes `SpaceEvent::FilesDropped` with a `DropAction`: `Move`
  by default (`MOVE`), `KeepInPlace` while Shift is down (`COPY`), or `Copy`
  from a right drop's menu. A right drag carrying files reports `MOVE` while
  hovering over a space and, on `Drop`, shows the menu `right_drop_entries`
  builds, owned by the space window (G5b, G5c). The chosen entry becomes the
  `DropPlan`, the same type a left drop gets from `drop_plan`, so both take
  one path: "Create shortcuts here" is `DropPlan::Shortcuts` (greyed when the
  source refuses `LINK`), and `right_drop_effect` reports what each entry
  means to the source. A drag carrying neither files nor shell items reports
  `DROPEFFECT_NONE`.
- As in Explorer, a `LINK` drop always makes shortcuts: a file list links when
  Alt is held and the source allows a link, or when the source offers neither
  `MOVE` nor `COPY`. A `CF_HDROP` counts as files only when every entry is an
  absolute path: the Start menu puts AppUserModelIDs there (for example
  `com.squirrel.element-desktop.Element`), so its drags are shell items. A
  drag with real files and shell items is treated as files, so a real file
  still moves unless Alt is held.
- A shell-item drag (any form `SHCreateShellItemArrayFromDataObject` reads,
  including a packaged app with no filesystem path) is accepted
  (`shell_drop_effect` prefers `LINK`, then `COPY`, then `MOVE`) and raises
  `SpaceEvent::ShellItemsDropped` with each item's absolute ID list, Explorer
  display name, and filesystem path (`None` for a virtual item). The app
  (`App::add_shortcuts`) copies a dropped `.lnk` or `.url` into the space
  storage, so no shortcut points at a shortcut, and writes a new `.lnk` for
  anything else (`win32/shortcuts.rs` `create_shortcut`,
  `IShellLinkW::SetIDList` + `IPersistFile::Save`) under a clash-free name, as
  Explorer does when such an item is dropped on a folder (F-50). The source is
  never moved or deleted.
- On `DragEnter` the target stores an empty "DropDescription"
  (`DROPIMAGE_NOIMAGE`, no text) on the source's data object, so a shell
  source such as Explorer or the desktop draws no "Move"/"Copy" hint over the
  item image. A move marks the data object with "Performed DropEffect" so a
  shell source does not move the files a second time; a shortcut, copy, or
  keep-in-place drop never sets it.
- Every file DeskSpace moves (into storage, back to the desktop) is announced
  with `notify_moved` (`SHChangeNotify` delete + create, `FLUSHNOWAIT` so it
  never blocks Explorer inside a drop); without it the desktop kept stale
  icons for files already moved into a space, and dragging one dropped a path
  that no longer existed. Dropped paths that do not exist are ignored rather
  than listed (which showed a label with no icon).

### Drag out and the drag image

- Dragging an item out (`shell_items::drag_out`) sets the data object's
  "Preferred DropEffect" to `MOVE`, so a drop on the desktop takes the file out
  of the space instead of leaving a `- Copy`. The drop target alone performs
  the transfer (async mode is off, so it is done when the drag returns), and
  afterwards the app only removes the item if its file is gone. It never moves
  the file itself after a drag: an earlier "dropped on the desktop, so move it
  back" step turned desktop copies into `- Copy` / ` (2)` duplicates.
- The drag image (`win32/drag_image.rs`, `win32/drag_ghost.rs`) is the picture
  that follows the pointer, distinct from the move/copy cursor. When an item
  drag starts, the app builds a drag scene (`scene::drag_scene`) of the whole
  active set, members scrolled out of view included, at their grid positions,
  and `DragImage::of_selection` renders it offscreen (`d2d::render_items`: one
  WIC bitmap render target with a scale transform, running the space's own
  `draw_item`, so icons, labels, and colors match), scaled down to
  `MAX_DRAG_IMAGE` when larger and with a uniform translucency
  (`DRAG_IMAGE_ALPHA`). The hotspot is the grab offset in the scaled group,
  and every view of the drag shows these same pixels.
- Inside the space a layered, click-through, topmost ghost window shows it
  (`UpdateLayeredWindow`, premultiplied DIB), and every dragged slot is drawn
  faded (`ItemState::Dragged`). Once the pointer leaves the space, the ghost
  hides and `drag_out` hands the same image to the shell's drag-image manager
  (`IDragSourceHelper2::InitializeFromBitmap`, straight-alpha DIB, since it
  premultiplies itself; `SetFlags(0)` leaves the drop description off, so a
  target like the desktop cannot add its "Move to ..." label and arrow). Space
  drop targets forward all four calls to one shared `IDropTargetHelper`,
  without which the image vanishes over a space; it is released before
  `OleUninitialize`. Without an image the shell substitutes its own opaque,
  oversized one.

### Selection, display matching, keys, and clipboard

- **Clicking a space acts like clicking the desktop**
  (`win32/shell_selection.rs`, `Ui::on_desktop_click`): spaces never take
  activation, so the shell's desktop window (`GetShellWindow`) is made active
  (the app in front goes inactive) and the desktop icon selection is cleared
  through its `IShellView` (`SelectItem(NULL, SVSI_DESELECTOTHERS)`, found
  through `IShellWindows` and the `SID_STopLevelBrowser` service). A selection
  inside an Explorer window is left alone, as Windows only changes a
  selection within the view that was clicked. The enumerator is cached and
  released before `OleUninitialize`.
- **Explorer display settings** (`win32/display_settings.rs`) are read from
  the desktop's `IShellView` (`win32/shell_view.rs`, shared with the click
  code) and `SHGetSetSettings`: `FWF_CHECKSELECT` / `FWF_AUTOCHECKSELECT`
  (item check boxes), `SSF_SHOWALLOBJECTS` / `SSF_SHOWSUPERHIDDEN` (hidden
  files), `SSF_SHOWEXTENSIONS`, `SSF_SHOWCOMPCOLOR`,
  `IFolderView2::GetViewModeAndIconSize` (the desktop's icon size), and
  `IFolderView::GetSpacing` (item spacing), all carried in `ShellDisplay`.
  The snapshot is re-read at startup, on `Event::SystemThemeChanged`, and on
  every space click; a change repaints every space. When
  `Theme::match_explorer()` is on (the default), the app overlays them:
  - The desktop icon size and spacing replace DeskSpace's icon-size and grid
    settings and drive the jumbo icon fetch and the per-monitor scale, so
    matching is 1:1.
  - `ShellDisplay.check_boxes` is a `CheckBoxMode`: `FWF_CHECKSELECT` gives
    `Independent`, `FWF_AUTOCHECKSELECT` `Selection`, neither `Hidden`.
    `SpaceScene.show_check_boxes` draws an Explorer-style box at each icon's
    corner; a checked box is filled with `SpaceScene.check_fill` (the Windows
    accent, or Windows blue when unknown) under a white tick. In `Selection`
    mode the box shows on hover and while selected and its tick mirrors the
    selection; in `Independent` mode it always shows and its tick is a
    session-only set, separate from the selection, which is then the operand
    for delete, copy, cut, and drag (`active_items`). A plain click ticks one
    item and unticks the rest, Ctrl+click ticks several, and clicking the box
    itself toggles one (`render::item_check_box_hit` is its clickable region).
    `desktop_display_since` keeps the stored mode when the view read fails
    (briefly, for example while Explorer restarts), so a transient failure
    does not hide the boxes; `desktop_display` is the same read starting from
    `Hidden`.
  - `scene::item_label` shows a file's full name when extensions are on,
    except for shortcuts, whose `.lnk` / `.url` Explorer never shows; both
    forms come from the path, so the option can be toggled either way.
    `SSF_SHOWCOMPCOLOR` colors compressed labels blue and encrypted ones green
    (`Item::compressed` / `encrypted`, read at creation).
  - Icons carry the shell's overlay (`SHGFI_ADDOVERLAYS`, plus
    `SHGFI_LINKOVERLAY` for `.lnk` / `.url` only), so shortcut arrows and sync
    badges appear; labels come from
    `IShellItem::GetDisplayName(SIGDN_NORMALDISPLAY)`, and seeding skips
    hidden or protected items Explorer would hide.
- **Inline rename** (`edit.rs`, `win32/keyboard.rs`): the app owns a pure
  `TextEdit` buffer and pushes it in the scene's `editing` (a space title) or
  `editing_item` (an item's label). The painter draws Explorer's rename field:
  an opaque bordered box around the laid-out text, the selection in the
  system highlight, and a blinking caret. An item's box wraps the whole name
  in the label's width and grows downward over the items below, as
  Explorer's does; a title's stays on one line. Spaces are never activated and
  cannot receive `WM_CHAR`, so a `WH_KEYBOARD_LL` hook (installed while
  editing or while either selection watch is on) turns key presses during a
  rename into `Event::EditKey` and consumes them (an Alt or Win combination
  cancels the rename and passes through). The thread never removes keyboard
  messages, so `GetKeyboardState` is frozen: the hook reads each held modifier
  from `GetAsyncKeyState`'s sign bit and Caps Lock's toggle from
  `GetKeyState`'s low bit (`GetAsyncKeyState`'s low bit is an unrelated
  "pressed since the last call" flag), so Shift and Caps stay predictable.
  F2 renames the focused item, or the current space's title with no item
  selected, and a second click on a selected item (a slow double-click)
  starts an item rename. An item rename is committed with
  `IFileOperation::RenameItem` (undoable; the extension is kept when the
  label hid it) and applied through `Command::RenameItem`, which updates the
  item's path and label.
- **Other keys**: the same hook watches keys outside a rename
  (`Ui::watch_selection_keys`); `ENGINEERING.md`'s Keys table is the one list
  of which key needs which flag, and section 8 says what each does. The arrow
  keys raise `Event::Navigate` (the nearest item in that direction, scrolled
  into view; Shift extends from the anchor), typed characters `Event::Typed`
  (type-to-select with a short timeout buffer), and the focused item is drawn
  with a dotted rectangle (`SpaceScene.focused_item`). Alt+Enter calls
  `Ui::show_properties`. The watch is limited to the desktop's foreground
  because the selection is kept when another app is focused, as Explorer
  keeps a desktop selection; otherwise DeskSpace would steal that app's own
  Ctrl+C/V. Each watched key is posted to the controller as its own `WM_APP`
  message (table above) and becomes one `Event`; a new key either reuses
  `SELECTION_KEY` with a new `SelectionKey` variant (the simplest path) or, if
  it carries data, gets its own message.
- **Clipboard** (`win32/clipboard.rs`, `win32/file_op.rs`): Ctrl+C/X put the
  selected items on the shell clipboard as `CF_HDROP` with a "Preferred
  DropEffect" (`MOVE` for a cut), the formats Explorer uses, so they paste
  into Explorer and other apps; cut items render ghosted (`ItemVisual::cut`).
  Ctrl+V pastes into the current space: an item already in a space is
  re-pointed (`MoveItem`), while external files and copies go through
  `IFileOperation` into the store (`FOF_RENAMEONCOLLISION` keeps Explorer's
  Replace dialog off the shared store; `FOF_ALLOWUNDO` records an undo), and
  an `IFileOperationProgressSink` returns each created path so the pasted
  items are listed and selected. `Ui::clipboard_shell_items` reads the
  clipboard's shell items through `OleGetClipboard` and
  `drop_target::shell_items` (sharing `open_clipboard`'s retry) for "New
  shortcut ▸ Paste shortcut" (F-54). The controller registers
  `AddClipboardFormatListener`; `Event::ClipboardChanged` clears cut ghosting
  when another app or the desktop replaces the clipboard and prunes items
  whose files a paste elsewhere moved away. A clipboard read that fails
  (another process holds it) keeps the ghost.
- **Delete** sends the selection to the Recycle Bin (`Ui::delete_files(paths,
  permanent)`, `IFileOperation::DeleteItem` with `FOF_ALLOWUNDO |
  FOF_NOCONFIRMATION`: no prompt, undoable; Shift+Delete passes `permanent`,
  which drops `FOF_ALLOWUNDO`) and drops the items whose files are gone.

### Rendering (`render/scene.rs`, `render/d2d/`)

- `SpaceScene` and `SpaceStyle` are the contract between app and renderer.
  `SpaceScene` carries everything a space draws: chrome, title, resolved
  colors, item metrics, and the visible `ItemVisual`s in window-local
  coordinates. `SpaceStyle` carries the window-level attributes. Both are
  documented field by field in `render/scene.rs`. A new visual is one
  `SpaceScene` field, filled in
  `deskspace-app/src/scene.rs` and drawn in `d2d/`; a `SpaceStyle` field is
  for window-level attributes only (it re-applies DWM state).
- The app scrolls by offsetting item rects and includes only items that
  intersect the content area; the renderer clips items below the title bar.
  Scenes are pushed only on change, and the window stores a `PreparedScene`
  with strings pre-encoded to UTF-16, so painting does not allocate.
  `chrome: false` (the desktop layer) draws no title bar or border and fills
  the window with items; the painter reads the scene's `chrome` for the title
  and scroll thumb and the window's `Frame` reads the style's, so both are set
  together, and `SpaceStyle::dwm_draws_frame` is then false.
- `Icon` is an `Arc` of owned premultiplied-BGRA pixels with a never-reused
  id. The main path decodes the `IShellItemImageFactory` HBITMAP through WIC
  (`d2d::decode_bitmap`); the classic fallback and overlays decode an HICON
  through WIC (`d2d::decode_icon`, which applies a legacy icon's AND mask) and
  destroy it at once, so scenes never point at freed handles.
- `Painter` draws on a `Canvas` (`d2d/canvas.rs`; premultiplied alpha, 96
  DPI so one DIP is one pixel) with a brush and a per-target bitmap cache
  keyed by `Icon::id`, pruned to the current scene's icons on every scene.
  The canvas is the window's own `ID2D1HwndRenderTarget` (`HwndCanvas`,
  which the zoom pop-up uses too), or a composition space's drawing
  surface (`SurfaceCanvas`), whose Direct2D context exists only during a
  draw and sits at an offset in a shared atlas, so the painter draws
  relative to the origin the canvas gives it. The factories and text
  formats are shared per thread (`Graphics`, released in the runtime
  teardown). A lost device (`D2DERR_RECREATE_TARGET`, or the Direct3D
  device under a surface removed or reset) drops device resources and
  invalidates the window, so the next paint recreates them; without that,
  the lost frame left the window blank until a click repainted it. The
  first surface to find its device lost gives the composition graphics
  device a new one, and every space repaints (`runtime::invalidate_spaces`).
- Text uses DirectWrite (Segoe UI Variable Text when installed; the lock glyph
  from Segoe Fluent Icons or MDL2): grayscale antialiasing on translucent
  surfaces and ClearType on opaque ones, ellipsis trimming, one or two wrapped
  label lines at the configured size (formats cached per 10 % size step), and
  a 1 px shadow when the scene asks (label shadow `Auto` below 25 % tint
  alpha, or `On`).
- Animations (ROADMAP D12): `render/transition.rs` is the pure frame math.
  A `Transition` tracks one linear progress and a direction, and every
  frame is a function of the eased progress alone, so an Out is the In
  played backwards and `reverse` continues from the very frame it is on.
  A `TransitionFrame` gives the window rect (Roll, Fly, and CRT move or crop
  it; Fly is clipped to the work area), where the space's content sits in
  it, and the opacity, mask, block size, and glow the painter applies.
  Burn's paper is `render/burn.rs`, pure: a field of smooth noise biased to
  burn the bottom first, and per cell the stage at the current front (gone,
  glowing, charred, scorched, whole), which the painter writes into two
  smooth cell bitmaps: the contents' opacity mask and the marks over it.
  Its flames rise past the frame, where the space's own window would put
  blur behind them, so they are drawn on `win32/flame_overlay.rs`: a
  layered, click-through window owned by the space window (so it stays
  directly above it and dies with it), made when a Burn starts and dropped
  when it ends. `render/flames.rs` (pure) stands tongues on the burning
  edge and shades them on the CPU into its DIB section each frame.
- The zoom pop-up (F-57, `win32/zoom_bar.rs`) is a topmost, never-activated
  `WS_POPUP` with the space material (`dwm::apply_style` from the global
  look; a Clear backdrop becomes Acrylic and the tint is at least 160/255 so
  the text stays readable). `ZoomBarPainter` (`d2d/zoom_bar.rs`) draws the
  `ZoomBar` the app sends in 96-DPI units under a scale transform for the
  monitor under the pointer: zoom out and in (Segoe Fluent glyphs), the
  percentage, a divider, and Reset, with a hover fill and dimmed disabled
  buttons. `Ui::show_zoom_bar` places it at the top center of that monitor's
  work area; a click (press and release on the same button) raises
  `Event::Zoom`. The app hides it after `Timer::ZoomBar` unless
  `Ui::is_zoom_bar_hovered`.

### Native UI (`ui.rs`, `win32/menu.rs`, `win32/settings_window/`)

The menus, the settings form, the color picker, the dialogs, and dark mode
are described in their module docs (`win32/menu.rs`, `win32/settings_window/`,
`win32/color_picker/`, `win32/dialog.rs`, `win32/app_picker.rs`,
`win32/app_catalog.rs`, `win32/shortcut_dialog.rs`, `win32/dark_mode.rs`) and
in `ui.rs`. What spans them:

- **Panes.** The platform knows nothing about spaces: the app fills the
  scopes and categories panes, maps an index back to its meaning, and answers
  each event with a new form. The page shape never changes: every page is
  always built, and the app picks which ones the categories pane lists.
- **Modal windows.** The color picker, the installed-apps picker, and the
  custom shortcut dialog are owned by the settings window when it is open,
  else the controller; each disables its owner and pumps every message while
  open. Picked apps become `ShellItem`s through `shortcuts::shell_item`, the
  function drops use; the custom dialog's `CustomShortcut` is written by
  `shortcuts::create_custom_shortcut`.
- **Highlight.** Selecting a space in the scopes pane, or opening the window
  on one, highlights it on the desktop for about 1.2 s
  (`SpaceScene::highlight`, cleared by `Timer::Highlight`). Selecting another
  space moves it; Cancel, OK, and closing the window clear it.
- **The app's form** (`settings/`): the scopes are Global, then every space
  indented, dimmed while it follows Global. Global lists every page but
  Shortcuts; a space lists the five look pages, which show its values (from
  Global until customized) greyed unless the header's "Customize this space"
  switch is on, and its always-enabled Shortcuts page. Global edits write
  `Config` directly; a space's edits become `SetSpaceLook` commands, the
  switch `CustomizeSpace`, and the Colors page's reset becomes "Match global
  appearance". Cancel restores each space's `SpaceSnapshot`; a space menu edit
  made while the window is open goes into that snapshot, because the menu
  commits at once. These also commit at once, so Cancel does not undo them:
  - the Shortcuts page's X deletes an item after a confirmation
    (`App::delete_space_item`: a stored file goes to the Recycle Bin, an item
    listed in place is only unlisted; ROADMAP D3), and its + offers the New
    shortcut choices (`App::offer_new_shortcut`);
  - a space header's "Delete space" button runs the same delete as the menu
    (with its confirmation), then falls back to Global;
  - the Layout page's "Fit to contents" button (built for every scope so the
    page shape stays constant; grey unless a resized, unlocked space is being
    edited) runs the menu's `FitToContents`.
- **Dark mode.** The app calls `Ui::set_ui_dark` with `Theme.ui_mode`
  resolved against Windows on start, on `SystemThemeChanged`, and while the
  choice is edited. The spaces follow their own `mode`.
- **The debug window** (`--debug`, ROADMAP D14; `win32/debug_window/`,
  `app/debug/`). Modeless and data-driven like settings: the app builds a
  `DebugPanel` from `DEBUG_CONTROLS` and pushes it only when a value
  changed (every second, `Timer::DebugRefresh`, and after each control).
  Every section is built at once as a page inside a scrolled pane, so an
  update never recreates controls. The console is a virtual list view over
  the platform's `ConsoleModel`; the app feeds it from `debug_log.rs`'s
  ring on `Timer::DebugLog`, a timer so a line logged while draining cannot
  feed itself. Its actions call the paths the menus, settings, and drops
  use, save nothing but the animation settings (D14; `app/debug/
  animation_settings.rs`, through `settings::apply_animation_edit`), and
  ask first (`debug_confirm`) when destructive. A control can sit in a
  titled box (`DebugControl::group`), which the page draws as it erases,
  and a choice can be greyed rather than removed, so the panel keeps its
  shape (a shape change rebuilds the window). The
  only state it adds outside `DebugTools` is the session-only
  `AnimationTweak` and the takeover override in `LayerState`.

### Desktop shell, shell items, DPI, hotkeys

- `folder_watch.rs` watches a folder off the UI thread
  (`ReadDirectoryChangesW`, overlapped I/O on one worker thread) and posts
  `FOLDER_CHANGE` when its contents change, debounced by up to 150 ms (the
  pure `Coalescer`) so a multi-file copy wakes the app once. `Ui::watch_folder`
  starts or replaces the single watch (idempotent for the same path) and
  `Ui::unwatch_folder` stops it; the controller raises
  `Event::FolderChanged`. M2 watches the Desktop folder while the layer is
  drawn (a change makes the app re-enumerate and `SyncDesktop`); M4 phase 2
  reuses the watcher for a lens's folder.
- `desktop_shell.rs` finds Progman or the wallpaper `WorkerW` (through the
  `0x052C` spawn trick) for hosted placements.
- `desktop_icons.rs` hides and restores Explorer's icon layer for the
  takeover: `hide` / `restore` hide or show every `SysListView32` under a
  `SHELLDLL_DefView`, re-enumerating each time so a recreated listview is
  found, and `rehide_if_active` re-hides only while takeover is on. The walk
  recurses the desktop shell's (`Progman` / `WorkerW`) whole subtree, because
  on some builds the icon `SHELLDLL_DefView` is nested (for example
  `Progman -> WorkerW -> SHELLDLL_DefView`), and only that subtree, because an
  Explorer window also contains a `SHELLDLL_DefView`. Only the listview is
  hidden, never the `SHELLDLL_DefView`, so `find_host` and the shell-view COM
  reads keep working. `restore` runs in the `run` teardown before
  `OleUninitialize`; `rehide_if_active` runs on `TaskbarCreated` and from the
  app's health timer.
- `watchdog.rs` is the crash-safety half of the takeover (D8). Before hiding
  the icons the app calls `arm_watchdog`, which creates a per-pid clean-exit
  event and launches `deskspace-watchdog.exe` (`<pid>` as its only argument)
  detached, so a bulk kill of `deskspace.exe` by name or a closed console
  leaves it running. The watchdog re-parents itself once
  (`respawn_reparented`) so ending the app's process tree cannot reach it,
  then waits on the main process and re-shows every listview unless the event
  was signaled; the teardown calls `signal_watchdog_clean_exit` right after
  `desktop_icons::restore`, so the watchdog acts only on a crash. A named
  mutex keeps a stray watchdog a no-op, and the pure `watchdog_pid` /
  `is_watchdog` and `should_restore` decide the argument and the restore.
- `win32/desktop_items.rs`: `enumerate_desktop_items` reads the desktop
  view's items and positions (`IFolderView::ItemCount` / `Item` /
  `GetItemPosition`) once OLE is up, and falls back to a name-sorted walk of
  the Desktop folder, without positions, when the view cannot be reached.
- `win32/icons.rs`, `IconCache`: icons are rendered by
  `IShellItemImageFactory` at the requested pixel size; a path the factory
  rejects falls back to the classic 32 px icon, scaled. The overlay (shortcut
  arrow, sync badges) is drawn as Explorer draws it: `SHGetFileInfoW` with
  `SHGFI_OVERLAYINDEX` names the overlay (this works in any process), and
  `IImageList::GetOverlayImage` fetches its full-icon-sized image from the
  system image list nearest the size (`SHIL_SMALL`..`SHIL_JUMBO`),
  area-averaged to `theme.overlay_size` percent and anchored bottom-left
  before compositing. HICONs are decoded through WIC (`d2d::decode_icon`).
  Entries are keyed by path, pixel size, and overlay size, with
  least-recently-used eviction at 1024 entries; `invalidate(path)` forgets
  every size of one path.
- `shell_items.rs`, Explorer behavior for items:
  - `Ui::open_path` runs `ShellExecuteExW` with the default context-menu verb,
    first passing DeskSpace's foreground right on with
    `AllowSetForegroundWindow` (spaces are never active, so opened apps would
    stay behind). Paths are given with backslashes, so it cannot open a URL
    (`TASKS.md` E10 adds `Ui::open_url`).
  - `Ui::item_menu` puts the app's entries above the shell's `IContextMenu`;
    the controller forwards owner-draw and submenu messages to it while open.
  - `Ui::drag_out` (`SHDoDragDrop`): one item uses the shell item's own data
    object; several use `SHCreateDataObject` over the items' ID lists relative
    to their deepest common folder (or full IDs under the desktop when they
    share none), so a multi-item drag carries the ID list the desktop needs to
    drop at the pointer and accepts the drag image, plus "Shell Object
    Offsets" (`CFSTR_SHELLIDLISTOFFSET`, from the items' current cells) so the
    group keeps its arrangement instead of stacking in a column. Async mode is
    forced off so the transfer ends inside the drag loop, and it reports the
    effect and whether the drag ended over the desktop. Drag and drop is why
    the runtime uses `OleInitialize`.
- `dpi.rs` sets per-monitor v2 awareness; `window_dpi` / `scale_for` back the
  runtime's `space_dpi(id)` (the space window's own monitor scale), which the
  app reads per space. `system.rs` provides `work_area_at(point)`,
  `virtual_desktop_rect()`, `cursor_pos()`, `drag_threshold()`,
  `system_uses_dark_mode()`, `system_accent_color()`, and `wide` (UTF-16
  conversion; use it rather than a local copy).
- Info tips (`win32/info_tip.rs`, `Ui::show_info_tip` / `hide_info_tip`):
  after the pointer rests on an item, the app shows Explorer's info tip (name,
  shell type, size, local modification date) in one reusable native tracking
  tooltip (`TTF_TRACK`, `TTM_TRACKPOSITION` / `TTM_TRACKACTIVATE`). The app
  owns the hover delay with a timer; moving off, clicking, or hiding spaces
  dismisses it.
- `hotkey.rs`: `parse("Ctrl+Alt+D")`, `hotkey_id(index)`, and
  register/unregister on the controller window through `Ui`. The table
  `settings::HOTKEY_BINDINGS` drives the three bindings (section 8): each row
  names its `HotkeyAction`, two field ids, a label, and the `WindowSettings`
  field that stores it, and its position is its registration index;
  `App::on_hotkey` matches on the action. An empty binding is unregistered.

## 7. Runtime flow

1. `main` parses the CLI and initializes `tracing` (`-v` debug, `-vv` trace;
   `RUST_LOG` overrides both), then calls `bootstrap::run`. `--debug` adds a
   console layer (`debug_log.rs`) with its own level, and every build logs
   panics at error.
2. `bootstrap::run`: with `--show`, hands off to a running instance and exits
   before loading anything. Otherwise it enables DPI awareness, loads or
   creates `Config` (migrating older files; `--reset` backs up the old file
   and refuses to start if the backup fails), merges legacy per-space overrides
   into looks (saving if anything changed), seeds a "Desktop" space if there
   are none (filled from the desktop folder unless `--no-icons`), and calls
   `deskspace_platform::run`. Placement is `--placement` when given, else
   top-level. `--show` overrides `--hidden` and start-hidden.
3. On `Started`, the app sets the menu and settings dark mode, registers every
   binding in `HOTKEY_BINDINGS`, syncs the desktop "Show spaces" verb and the
   start-with-Windows Run value, reads Explorer's display options and the
   desktop items, applies takeover and start-hidden, creates one window per
   space with its style and scene, re-flows every space, prunes items whose
   files are gone, and optionally opens settings (`--settings`).
4. Pointer events update selection and hover (re-pushing that space's scene),
   drag spaces through `SetSpaceRect` (moving the window), resize them through
   `ResizeSpace` from the border band, scroll them on the wheel (per-space
   offsets live in the app, not the config), toggle folding on a title-bar
   double-click, and open the space menu on right-click. Items open on
   double-click, show the item menu on right-click, and start a shell drag
   past the system drag threshold. Drops add items (moving new files into
   storage through `storage::ItemStore`) or move them between spaces. Saves
   are debounced: drags, resizes, folding, lock, and space creation schedule a
   save about 700 ms after the last change, while file-moving operations
   (drops, item removal, delete) and Apply/OK save at once, and closing
   flushes any pending write.
5. Settings edits apply at once as a preview: global edits to the live
   config, a customized space's through `SetSpaceLook`. Repainting is deferred
   to `Timer::Preview`, which fires only once the message queue is idle, so a
   slider drag collapses into one restyle or re-flow per pause (all spaces for
   a global edit, one for a space edit). Apply/OK validate and re-register
   every hotkey, sync start-with-Windows, apply or remove takeover, and save;
   Cancel or closing the window restores the saved theme and window settings
   and every space's snapshot (`settings::needs_relayout` / `needs_restyle`
   decide what to redo).
6. On `CloseRequested`, the app unregisters every hotkey, cancels a rename,
   discards unapplied settings edits, flushes the save (`flush_save`), and
   removes the desktop verb. The platform teardown then restores Explorer's
   icons, signals the watchdog's clean-exit event, and releases COM.

## 8. Controls (current)

**Spaces**

- Drag a space's title bar to move it (snaps to the grid); double-click it to
  fold or unfold the space.
- Drag an edge or corner to resize; the wheel scrolls overflowing content.
- Ctrl+wheel changes the icon size. Under the default "Match Explorer's
  display" preset it changes the desktop's own icon size (8 px a notch,
  16-256 px), as Ctrl+wheel on the desktop does, and the matching spaces
  follow (ROADMAP D9); under Custom it changes the global icon size (10 % a
  notch). A space with its own Custom look keeps its icon size. Every space
  keeps its size and re-flows its icons into more or fewer columns (scrolling
  when they no longer fit), as an Explorer window does; "Fit to contents"
  sizes it to its items again.
- Every zoom shows the zoom pop-up at the top center of the monitor under the
  pointer (F-57): the size as a percentage (the desktop's 48 px "Medium
  icons" is 100 %), zoom out and in (one notch each), and Reset (back to
  100 %). Buttons grey out at the ends of the range, and Reset at 100 %. It
  hides 2 s after the last zoom, or once the pointer leaves it.
- Rename a space from its right-click menu: the title is edited in place
  (Enter commits, Escape cancels, clicking away commits).

**Menus**

- Right-click a space (`App::show_space_menu`, `app/menus.rs`): Fold/Unfold,
  Rename, Lock position, Fit to contents (resized, unlocked spaces only); New
  shortcut ▸, Add files…, Appearance ▸, Sort by ▸; Delete space (asks first
  unless the space is empty); New space; Settings…, Hide/Show spaces, Exit
  DeskSpace.
- New space, from any menu or the hotkey, opens beside the top row of spaces
  on the monitor of the click or the pointer, one typical gutter to the
  right (ROADMAP D10).
- **Appearance ▸** sets a tint-opacity preset (100/80/60/40/20 %), an icon size
  (Small 75, Medium 100, Large 150, Extra large 200 %), or a custom tint or
  text color. Each customizes the space, starting from what it shows now; "Use
  global appearance" (greyed unless customized) makes it follow Global again
  (keeping its look for later), and "Space settings…" opens the settings
  window on that space. **Sort by ▸** reorders items by name (A–Z or Z–A),
  type, size (smallest or largest), or date (oldest or newest). The three
  submenus are built from tables in `app.rs` (`OPACITY_PRESETS`,
  `ICON_SIZE_PRESETS`, `SORT_PRESETS`).
- **New shortcut ▸** (F-53, F-54; `NEW_SHORTCUT_ENTRIES` in `app.rs`, also
  enabled on a locked space): "From installed apps…", a picker listing every
  app in the Applications folder (Store apps included) with a check box per
  row and a search box; "Custom shortcut…", a dialog for a target (file,
  folder, program, or web address, with Browse…), optional arguments, and a
  name that follows the target until edited; and, after a separator, "Paste
  shortcut", enabled when the clipboard holds shell items
  (`Ui::clipboard_shell_items`), which leaves the clipboard, its files, and any
  cut ghosting untouched. A typed path is resolved as the Run dialog does
  (`%VARIABLES%`, a bare program name on the search path), and a missing
  target is reported in the dialog. A web address becomes a `.url`,
  everything else a `.lnk` (a copied `.lnk` or `.url` is duplicated into the
  store). All three end in `App::add_shortcuts`, which writes into the space
  storage and lists the shortcuts at the end of the space. The same choices
  open from the + tile on a space's Settings page "Shortcuts", whose preview
  also deletes an item from its X (after asking).
- **Add files…** (F-55) picks files in the standard Open dialog
  (`Ui::pick_files`, an `IFileOpenDialog` with multi-select, started on the
  Desktop folder) and moves them in through the drop path (`App::add_dropped`,
  ROADMAP D3), so a picked file is relocated exactly like a dragged one.
  Cancelling adds nothing.
- Tray icon: left-click (or keyboard select) opens Settings; right-click
  offers Settings…, Hide/Show spaces, New space, Check for updates… (or
  "Restart to update to X" once an automatic update is downloaded), and Exit
  DeskSpace, and, with `--debug`, Debug…. A space created from the tray goes to the center of the work
  area, and creating a space while spaces are hidden shows them.

**Settings**

- Pick Global or a space in the left pane. Global edits apply to every space
  that is not customized. A space's pages show the global values, greyed,
  until "Customize this space" is ticked; its edits then apply to that space
  alone. A space's header also has a "Delete space" button. Selecting a space
  highlights it on the desktop for about 1.2 s.
- Edits preview live. Icon size, spacing, and edge padding re-flow every
  space; icon size scales the icon and its grid cell, defaults to 100 %
  (Windows' 48 px desktop icon), and can be typed as a percentage. The shell
  overlay size is the shortcut arrow badge as a percent of the icon (default
  80 %; 100 % matches Explorer). The Layout page's "Fit to contents" button
  (grey unless a resized, unlocked space is selected) commits at once like
  the menu entry.
- The pages (`settings/pages.rs`, one builder function each). "Look" pages
  appear for Global and every space, Shortcuts only for a space, the rest
  only for Global:

  | Page | Scope | Fields |
  |------|-------|--------|
  | Appearance | look | Color mode, Background material, Corner radius, Tint opacity, Title bar shading, Show window border, Border thickness, Item preset |
  | Labels | look | Label lines, Label size, Label shadow |
  | Layout | look | Icon size, Icon spacing, Edge padding, Fit to contents |
  | Colors | look | Use custom colors, Background tint, Text, Use Windows accent (the selection), Border color (Automatic, Windows accent, Custom), Custom border color, Border opacity, Selection box color, Show selection box border, Restore default / Match global appearance |
  | Advanced | look | Shortcut overlay size |
  | General | Global | Start with spaces hidden, Snap to grid, Snap resized spaces to whole columns and rows, Start DeskSpace when you sign in, Take over the desktop, Settings and menus (theme), Check for updates automatically, Install updates automatically, Check for updates now |
  | New spaces | Global | Default title, width, height, Start new spaces locked (affect only spaces created afterwards) |
  | Hotkeys | Global | Show or hide spaces (`Ctrl+Alt+D` by default), Create a new space (on the pointer's monitor), Open settings (both unbound by default) |
  | About | Global | Version, summary, license, configuration file path |
  | Shortcuts | space | A note, and a preview of the space: an X on each item (delete, after a confirmation) and a + tile (New shortcut) |
  | Animations › one page per kind | Global | Built from `ANIMATION_PAGES` and the core tables (ROADMAP D12): the Windows note and "Open Windows animation settings…", Effect, Duration, and the options the kind offers, greyed when the effect does not use them; New space adds the startup order and delay. Everything is greyed while Windows' "Animation effects" is off, which `Event::SystemAnimationsChanged` follows live |

**Items**

- Drop files from Explorer or the desktop onto a space to move them into it;
  hold Shift to leave them in place and only show them in the space, or Alt to
  add a shortcut instead. A right drag onto a space shows a menu to move,
  copy, link, or just show the files (G5b, G5c). An app dragged from the Start menu or
  the taskbar (or any drag that offers only a link) becomes a shortcut in the
  space, as Explorer does on a folder; a dropped shortcut file is copied, and
  its original stays.
- Click an item to select it; Ctrl+click adds or removes it, Shift+click
  selects the range from the last click. Drag from empty space to rubber-band
  select (Ctrl or Shift adds), and Ctrl+A selects every item in the current
  space. Clicking empty space in a space or the desktop clears the selection
  (so do hiding spaces and folding one); clicking another application keeps
  it, as Explorer keeps a desktop selection. Clicking anywhere on a space acts
  like a desktop click: it focuses the desktop (deactivating the app in
  front) and clears the desktop icon selection, leaving any selection inside
  an Explorer window alone.
- Double-click an item to open it; click a selected item again (a slow
  double-click) to rename it inline. Right-click it for Explorer's context
  menu with DeskSpace's "Rename" on top (Explorer's own Delete there sends the
  file to the Recycle Bin; ROADMAP D3); right-clicking an item in a
  multi-selection keeps the selection. A loose item on the desktop layer gets
  Explorer's menu alone. Rest the pointer on an item for Explorer's info tip.
- Right-click an empty spot of the desktop layer (or press the Menu key with
  nothing selected there) for "New space", "DeskSpace settings…", and
  Hide/Show spaces above Explorer's own desktop background menu
  (`App::show_desktop_menu`). Takeover hides Explorer's view, so without it
  that menu would be unreachable. A shell command re-syncs the layer. When
  the view's menu adds nothing, the Desktop folder's own background menu
  (New, Display settings, Personalize; no View or Sort by) stands in, so the
  menu never ends after DeskSpace's entries.
- Item check boxes follow Explorer's mode (when the desktop shows them and the
  `MatchExplorer` preset is on). A checked box is filled with the Windows
  accent under a white tick (`Palette::check_fill`; Windows blue when the
  accent is unknown), independent of the space's selection colors; an
  unchecked box is a bordered light box. With auto-check-select, hovering
  reveals the box and it mirrors the selection; clicking an item ticks it and
  unticks the others, Ctrl+click ticks several, and clicking the box toggles
  one. With independent check boxes, the boxes always show, clicking a box
  toggles a persistent tick and moves the highlight, the ticks survive
  empty-space and desktop clicks, and delete, copy, cut, and drag act on the
  ticked items.
- Drag an item within its space to rearrange it (`ReorderItem`); dragging one
  member of a multi-selection moves the whole block (`ReorderItems`). The
  selection's drag image follows the pointer and every member's slot fades;
  the new order sticks until a Sort by reorders it.
- Drag an item out: into a folder or app like an Explorer item, onto another
  space to move it there, or onto the desktop to take it out of the space.
  With takeover on, a drop on the desktop instead opens a new space at the
  drop point, clear of the other spaces and the taskbar, holding what was
  dropped, whatever its source (ROADMAP D7, amended 2026-10-17).
  When the grabbed item is part of the selection, the whole selection drags.
- A right-button press on an item selects it; dragging past the drag
  threshold starts a shell drag (ROADMAP D9, `TASKS.md` G5a) that never
  rearranges inside the space, so a right drag onto Explorer or the desktop
  shows the shell's Move / Copy / Create shortcut menu. A right drag onto a
  space shows DeskSpace's Move here / Copy here / Create shortcuts here /
  Show here without moving / Cancel menu (G5b, G5c): "Copy here" copies the
  files into the store and leaves the originals, "Create shortcuts here" adds
  a shortcut to each (a dropped shortcut file is copied) and is grey when the
  source refuses a link, "Show here without moving" lists them where they
  are, and Cancel drops nothing. A right click without moving still opens the
  item or space menu.
- Delete sends the selected items' files to the Recycle Bin (undoable, no
  prompt), as Explorer does; Shift+Delete removes them for good. A stored file
  goes back to the desktop only by dragging it there (not with takeover on)
  or deleting its space.
- Ctrl+X, Ctrl+C, and Ctrl+V cut, copy, and paste. Cut items are ghosted until
  pasted; paste goes into the current space, moving (cut) or duplicating
  (copy) the files through the shell's file operation.

**Keys** (`ENGINEERING.md`'s Keys table says when each is watched)

- F2 renames the selected item's file inline (Enter commits, Escape cancels,
  clicking away commits; the extension is kept when the label hides it), or
  the current space's title with no item selected.
- Alt+Enter opens the selected item's Properties, or DeskSpace Settings with
  nothing selected. With an item selected, Enter opens it, and the Menu key or
  Shift+F10 opens its context menu (the space's menu with no item selected).
  Escape clears the selection, and F5 refreshes the spaces.
- Ctrl+Plus and Ctrl+Minus zoom in and out one notch and Ctrl+0 resets to
  100 %, as the zoom pop-up's buttons do (F-57).
- With a space current, typing selects the next item whose label starts with
  the typed letters (the buffer clears after about a second); Home, End,
  PageUp, and PageDown scroll the space to the top, the bottom, or one page;
  and the arrow keys move the selection across the grid (Shift extends, Ctrl
  moves the dotted focus rectangle only).

**Hotkeys and the command line**

- Settings → Hotkeys binds the global hotkeys: show/hide spaces
  (`Ctrl+Alt+D` by default), a new space on the pointer's monitor, and open settings
  (both unbound). While spaces are hidden, the desktop's right-click menu has
  "Show DeskSpace spaces" (a per-user `DesktopBackground\Shell` verb, removed
  once they are shown). It runs the executable with `--show`, which signals
  the running instance instead of starting a second copy.
- CLI flags (`cli.rs`): `--config`, `--placement` (`top-level` |
  `desktop-background` | `desktop-foreground`), `--reset`, `--hidden`,
  `--show`, `--settings`, `--no-icons`, `--click-through`, `-v` / `-vv`, and
  the hidden development flags `--on-top`, `--takeover-layer`,
  `--takeover`, and `--debug`.

## 9. Feature parity

Moved to `ROADMAP.md` section 7, the one feature index; this heading keeps the
section numbers stable.

## 10. Known limitations and technical debt

- **Desktop takeover is opt-in and new** (ROADMAP D8, D9). Explorer owns the
  icon layer unless "Take over the desktop" (or `--takeover`) is on; the
  mechanics and crash safety are in sections 5 and 6. The release zip ships
  `deskspace-watchdog.exe` beside `deskspace.exe` (I6).
- **File moves have no undo yet.** Deleting a space returns its stored files
  to the desktop (deleting an item recycles it, which the Recycle Bin can
  undo); files on another drive stay in place, since a move there would copy.
  Only the Desktop folder is watched (while the layer is drawn), so a stored
  file deleted, moved, or restored by an undo outside DeskSpace stays listed
  until an item action or the next startup prunes it. An item's `size` and
  `modified` (used by sorting) are read once at creation and can go stale.
- **Blur relies on an undocumented API** (`SetWindowCompositionAttribute`).
  If a Windows build removes it, spaces fall back to a translucent tint
  without blur. Rounded corners need Windows 11.
- **Dark mode relies on undocumented uxtheme exports** for menus and on
  Explorer's dark control styles. The confirmation message box stays light;
  it has no dark mode.
- **Per-monitor DPI is scaled, with caveats.** Layout, icons, and hit tests
  scale to each monitor's DPI, and `WM_DPICHANGED` re-flows a moved space. The
  settings window is laid out for the DPI at creation, and an item moved
  between monitors of different DPI is positioned from the destination
  space's grid (correct, but not verified on screen).
- **Start-menu and taskbar drags are undocumented.** What they carry was found
  by observation (`TASKS.md` L1): Start offered `MOVE` and a `CF_HDROP`
  holding an AppUserModelID rather than a path, and the effects it offers vary
  by item. `drop_plan` and `is_file_list` handle what has been seen; a future
  build may differ, and the L1 debug log is the way to find out. If DeskSpace
  runs elevated, Windows blocks every drop from Explorer and Start (UIPI).
- **Placement is fixed for a run.** Only `--placement` chooses it; there is no
  live switch, because recreating every space window would drop the blur.
  Top-level, the default, is the only placement with blur.
- **The icon cache is invalidated only after shell menu commands and
  renames.** Its key includes the pixel and overlay size, so an icon-size or
  DPI change reloads, but a Desktop-folder change (`FolderChanged`) re-syncs
  the layer without invalidating icons, so a file whose icon changed in place
  keeps the old one. Lens watchers will need to invalidate.
- **The scroll indicator is display-only.** Spaces scroll with the wheel,
  Home/End/PageUp/PageDown, and by keeping a keyboard-selected item in view,
  but the indicator cannot be dragged, and wheel events need Windows' "scroll
  inactive windows" setting.
- **Inline rename has no IME or clipboard support.** The hook handles the
  active layout's printable characters and navigation keys only; composition
  (Japanese, Chinese, ...) is deferred. An item rename goes through the shell,
  so a name clash shows Explorer's conflict prompt.
- **Explorer display matching is broad but not total.** Explorer's view mode
  (list/details) is not mapped (it waits for M4 phase 4), and only the
  desktop's own options are read, so a check-box setting enabled in one
  Explorer window is not reflected. Matching is part of the look, so a
  customized space can turn it off for itself.
- **Selection is within one space.** A selection cannot span spaces, and
  Ctrl/Shift-dragging between spaces is not distinguished from a plain move.
- **Item drag is close to Explorer, not identical.** Cross-space moves match
  by file identity. Drags that start in Explorer keep Explorer's own image.
  Explorer's Shift=move is deliberately inverted on DeskSpace targets, where
  Shift keeps the original in place. Dropping a group onto the desktop can
  still shuffle its layout: Windows 11 repacks desktop icons when "Align icons
  to grid" is on, as an Explorer drag does, so this is native behavior.
- **Clipboard and delete match Explorer for files only.** Copy and cut
  advertise `CF_HDROP` and "Preferred DropEffect"; the richer shell formats for
  virtual items are not provided.
- **The desktop "Show spaces" verb is a legacy menu item.** As a per-user
  `DesktopBackground\Shell` entry it appears under "Show more options"
  (Shift+right-click) on Windows 11, not the new top-level menu, which needs
  an `IExplorerCommand` handler in a signed MSIX package with the
  `windows.fileExplorerContextMenus` extension and a stub executable
  (deferred to M8 packaging). Its icon is the executable's embedded icon
  resource (`"{exe}",0`), not a file beside the storage folder.
- **Packaging, signing, and auto-update.** Policy: ROADMAP D6; release steps
  and what `release.yml` publishes: `RELEASING.md`. In the code:
  - The installer is per user, installs to `%LocalAppData%\DeskSpace.App`
    (apart from the stored files under `%LocalAppData%\DeskSpace`), and is
    unsigned until R10. It is built by the CI-only `cargo-packager` from
    `[package.metadata.packager]` in `crates/deskspace-app/Cargo.toml` and the
    vendored `packaging/nsis/installer.nsi`.
  - Auto-update (F-45, `deskspace-update`, driven by `app/update.rs`) is
    opt-in and installs the full package from a minisign-signed
    `latest.json`. "Install updates automatically" is enabled only with the
    check on and in the installed build (`deskspace_update::is_installed`).
    Without it, a found update is offered in a prompt (Update and restart /
    Later / Skip this version). With it, the app downloads and verifies in the
    background and installs silently once the user has been idle 10 minutes
    with no full-screen app or presentation (`idle_time`,
    `user_is_presenting`), never while the settings window or a rename is
    open; until then the tray offers "Restart to update to X". A skipped
    version is never installed automatically.
  - Every install first quits through the normal teardown (save, restore
    Explorer's icons, signal the watchdog); `bootstrap::run` runs the
    installer (`InstallOnExit`) after `deskspace_platform::run` returns, and
    the NSIS installer (`/P` or `/S`, plus `/R`) relaunches DeskSpace. If the
    installer cannot start, DeskSpace starts itself again with the same
    arguments (less `--reset` and `--settings`) and then explains in a box
    that has no owner window (`deskspace_platform::alert`).
  - Until the maintainer sets the `DESKSPACE_MINISIGN_KEY` secret and
    `UPDATE_PUBKEY`, releases ship without a feed, and the app greys out the
    tray's "Check for updates…" and the Settings update controls
    (`deskspace_update::is_configured`).
  - The release build is reproducible (`--remap-path-prefix` in
    `release.yml`, `/Brepro` in `build.rs`) on the toolchain pinned in
    `rust-toolchain.toml`.
- **License is Apache-2.0** (copyright Timothy W. Brown; `LICENSE`, `NOTICE`).
  Release packaging must ship both files.

## 11. Glossary

- **Space**: a movable container window that groups items on the desktop.
- **Storage**: the folder files dropped into spaces are moved to.
- **Lens**: a space whose contents mirror a folder live (comparable tools
  call it a Folder Portal); in code, a space with a folder source. Deleting a
  lens removes only the view, never the folder (ROADMAP D3).
- **User-sized**: a space the user resized; it keeps its size, re-flows items
  to its width, and scrolls instead of growing.
- **Fold**: collapsing a space to its title bar (called roll-up before D12;
  the config key is still `rolled_up`).
- **Auto-hide**: hiding all spaces and desktop icons until the desktop is
  interacted with.
- **Placement**: where space windows live: top-level windows or children of
  the desktop's Progman/WorkerW window.
- **Scene**: the renderer-agnostic draw list (`SpaceScene`) the app builds for
  one space.
- **Material / backdrop**: the blurred layer DWM draws behind a space's tint.
