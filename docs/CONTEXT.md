# CONTEXT

DeskSpace's architecture and current behavior; read it before making changes.
`ENGINEERING.md` has the workflow and engineering rules, `ROADMAP.md` the
plan, the `D`-numbered decisions, and feature status.

## 1. What DeskSpace is

A free and open-source desktop space organizer for Windows. It groups desktop
items into movable containers ("spaces") and mirrors folders as live "lenses"
(comparable tools call these Folder Portals), with folding,
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
  `TASKS.md` section K; `crates/deskspace-app/tests/file_size.rs` enforces the
  limit and lists the exceptions. Section 3 lists the `app/` modules.

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
  | `columns` | a Details header: a click sorts on release, a drag reorders, a divider drag resizes |
  | `rename` | `EditState`, inline rename of titles and items |
  | `files` | drops, delete, prune, `resync_desktop_layer` |
  | `clipboard` | cut / copy / paste |
  | `menus` | space, item, and tray menus, `create_space` |
  | `space_icons` | `SpaceIcons`: the title icons' sets, loaded off the main thread, the automatic match memo, the glyph cache, and the download consent (D16) |
  | `lens` | keeping folder watches in line with the model, swapping in a lens's list, browsing into subfolders |
  | `undo` | `UndoStack`, recording an action, and undoing or redoing it (ROADMAP D11) |
  | `animation` | turning the settings and Windows' switch into each window's `Play` (ROADMAP D12) |
  | `settings_session` | the settings window's open, preview, Apply, and Cancel |
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
A space stores a reference to its icon (`SpaceIcon`, `icon.rs`), not the
picture; it is resolved when the space is drawn (S7).

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
folded), `resize_edges_at(point, border) -> ResizeEdges` (none when locked or
folded; corners twice as wide as edges), item lookup/insert/remove, and
`set_position`. Item geometry (hit rects, the scroll extent, and the visible
range) comes from `ViewLayout`, not the space.

`Space.source` (`source.rs`) says what fills the space. `SpaceSource::Items`
is the default and holds the space's own items, as before. `SpaceSource::Folder`
makes it a lens (ROADMAP D3, M4): the `FolderView` holds the bound path, the
hidden and filter settings, and says how the title follows the folder
(`LensTitle`); `Space.title` is the text every reader shows, computed by the
app when the folder or the title setting changes. A space also holds a `view`
(`SpaceView`, `view.rs`): its layout mode, its sort rule, and whether it keeps
that order as items come and go (D17), apart from `look`, so a normal space and
a lens share them. `Space::keeps_sorted()` is true for a lens always and for a
normal space whose view asks; such a space re-sorts after an item is added,
moved in, or renamed, and when the choice is turned on, and it refuses a
manual rearrangement. `ViewLayout` (`view_layout.rs`) derives every item's
rect, the content extent, the scroll axis, and the visible range from the
mode, the content size, the item count, and the cell metrics. The icon views
hand it the grid; Small icons (W16) and List (W17) use an inline cell (a 16 px
icon and one label line beside it). Small icons wraps the cells into rows and
scrolls down; List fills a column top to bottom and scrolls sideways. Details
(W18) gives every item a full-width row under a fixed header whose columns
sort by the space's `SortRule`; the app formats its column text when it builds
the scene, so painting never asks the shell or the clock. `SpaceView.columns`
(W30) says which columns a space shows and in what order, Name always first;
`DETAILS_COLUMNS` holds what each one is. The list is sanitized on load and
after every edit, and a hand-edited one can never fail the config load. Dragging
a header divider sets a column's width live (`app/columns.rs`): Name is held at
its width for the drag, so the divider follows the pointer, and takes the rest
again on release. Dragging a header moves its column on release, with only an
insertion mark until then (`DetailsView.drop_mark`). None of the
order-placed views writes `Item.position`, so a hand arrangement returns when
the space goes back to an icon view. Tiles (W21) wraps 48 px icons with three
lines beside each (name, type, size), and Content (W22) puts one item on each
full-width row with its labelled date and size on the right and a separator
between rows. `SpaceView.compact` (W33) switches these views' cells to
`COMPACT_CELLS`; the icon views keep the Layout page's spacing.
`SpaceView.group` groups the items in any view (D18). The stored order stays
as it is: `App::sync_grouping` derives a `Grouping` (display order and groups,
with the names from `crate::grouping`), cached behind a digest of what it reads
and checked on every repaint, and `ViewLayout::with_grouping` maps stored
indices to display positions, so its readers need not know. Folded groups are
session state in `ViewCache`; their items have no position, so nothing can hit,
select, or scroll to them, and a rebuild drops them from the selection. `SpaceView.status_bar` adds Explorer's
status bar along the bottom edge (W19): the item and selection counts and the
selected files' size on the left, and the Details and Large icons buttons on
the right. It is chrome, so `Space::content_rect` leaves its height out and
layout, scrolling, and hit tests shrink with it. It is off for new spaces and
on for new lenses. A new lens takes its view, its title mode, and its
hidden-files choice from Global's Lenses page (`WindowSettings.new_lens`);
a new space its view from the New spaces page
(`WindowSettings.new_space_view`, W40). `SpaceView.extensions` (W28)
decides whether a space's labels show filename extensions: `Show` and
`Hide` are its own answer, and `Automatic` follows today's rule,
Explorer's option on a space that mirrors Explorer's display
(`App::shows_extensions`, the one reader). The View ▸ menu's "File name
extensions" check shows what the space shows now and flips to the
opposite; the View page offers all three. Hit tests, the marquee, keyboard
navigation, the drag drop index, and the scroll indicator all read the same
layout. `FolderEntry` is what the platform reads from the folder.
A lens's items are a live cache, built off the UI thread, because filtering and
sorting a big folder takes far longer than a frame. The watch worker lists the
folder and gives the entries to the lens's `LensBuilder` (`lens_build.rs`, pure),
which filters hidden entries and the view's "Show only" and "Hide" patterns
(`wildcard::matches`), sorts by the space's `SortRule`, and reconciles by path so
an `ItemId` survives a refresh. Ids come from a block the watch reserved
(`Desktop::reserve_item_ids`), and an item the model re-points tells the worker
(`App::note_renamed_lens_item`). The worker posts the finished `LensList`, and the
app swaps it in with `Command::SyncLens`, which only places the items; the list
it replaces is freed off the UI thread (`Desktop::take_retired_items`, in
`App::dispatch`). A list names the `LensRules` it was built under, and one that
no longer matches the lens, like one for a folder it no longer shows, is dropped.
A change of rules goes to the worker through `App::sync_watches`, and it builds
again from its last listing without reading the folder. A grouped lens's display
order stays on the UI thread: on 50,000 items it takes about 3.5 ms, 7.4 ms by
Type, and 14 ms the first time each extension is seen (2026-10-31). They are never written to the config,
and `Config::migrate` drops any a hand-edited file lists. `AddItem` and
`MoveItem` into a lens are `Rejected`; file changes there are shell
operations: a drop on a lens moves
(or copies) the files into its folder with `Ui::transfer_files` and
`OnCollision::Ask` (`App::drop_into_lens`). A lens on the storage
folder, or inside it, is kept but never watched, and counts as unavailable:
the folder watch refuses it by its final path, so a junction into the store
is caught too, and the UI thread resolves no lens path to decide it. A
folder the user has just picked or is writing into is also checked on the
spot (`ItemStore::allows_lens`; see its callers), against the store's final
path resolved once at startup.
An unavailable lens (`App::unavailable_lenses`) draws "Folder not available"
and its path in place of items (`SpaceScene::placeholder`).
A lens whose folder was renamed or moved where it may follow (D19) is
re-pointed with `SetLensFolder`, one "Change folder" undo entry, and shows
"Folder moved:" and the new path on its notice bar (`SpaceScene::notice`,
from `App::notices`) until `Timer::Notice` clears it about 3 s later. A
lens browsed into a subfolder returns to its own folder instead; if its own
folder was the one renamed, it then shows "Folder not available", since only
the subfolder's watch saw the move.
Opening a folder in a lens (double-click or Enter) shows it in place
(D18): `App::browsed` keeps, per lens and for the session only, the
subfolder shown and the lens folder it was browsed under, and
`App::lens_folder` joins them. Every file operation and the watch read
`lens_folder`, and so does the title icon, so it and "Folder not
available" describe the same folder; the lens's settings and title source
keep its own folder. A shortcut opens as before, since it can point anywhere, and
`browse_into` never leaves the lens's folder. When the lens's folder
changes or the lens goes, `sync_watches` drops the browse, and Change
folder… drops it even for the same folder, so it, its undo, and a restart
all return to the lens's folder. The
title reads `Title › Sub › Sub2` (`scene::browse_title`) and, when too
long, is cut from its start so the folder shown stays in view
(`SpaceScene::title_keeps_end`). The painter measures the title's
DirectWrite clusters once per title, so a cut never splits a character,
and recuts by arithmetic as the width changes.
A lens's title bar shows its folder's own shell icon, or the generic folder
icon while the folder is unavailable; a normal space's title bar shows none.
Hovering a lens's title bar tips the folder it displays (`scene::title_tip`),
prefixed with "Folder not available" while its folder is gone; a normal
space's title bar tips nothing (W27).
Deleting a lens always asks and only unlists it: its folder and files stay,
it is no longer watched, and `stored_paths` and `release_item` never touch its
files.

### Desktop and desktop layer

`Desktop { spaces: Vec<Space>, layer: DesktopLayer, layout: GridSpec }` has
crate-private add/remove space and add/remove/move item, behind `Command`,
and public `space_item_count` and `space_at(point)` (topmost hit on
`visible_rect`). The `layout` field is the global grid, a setting edited in
place like the global theme (its doc says why). `move_item` validates the
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
  for user-sized spaces. A view that places items by order (Small icons and
  List) does
  neither: it keeps the space's size and `Item.position` (D17). It stamps `grid.title_height` onto the space, so a
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
- `resize_rect` moves the grabbed edges to a point, snapped to the fine
  `snap` step, keeping opposite edges fixed, at least `min_space_size()` (one
  icon cell plus padding) and inside the work area. With `fit_to_grid` on
  (the default), `ViewMetrics::fit_rect` then rounds the dragged axes to whole
  cells of the space's current view, keeping the grabbed edge: icon cells for
  the icon views; Small icons, List, and Tiles' own cells; Details and Content
  rows only, with the width left at `snap` (`ViewMode::rows_fill_width`). The
  height fit counts the title bar, the status bar, and Details' header as
  chrome, so no row is cut off by them. `scroll_step()` is one icon row per
  wheel notch.
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
a private `Desktop::apply_<variant>`; `space_ops.rs`, `item_ops.rs`,
`lens_ops.rs`, and `layer_ops.rs` hold those by what they change, and `mod.rs`
the shared helpers `with_space` (run a
change on a space or report `NotFound`), `rearrange`, and `close_gap`. The
variants and their fields are documented on `Command`; the rules below are the
ones a variant's doc cannot show.

- `CreateSpace { title, at, size, locked, view }` clamps to the work area
  (no snap). `MoveSpace`, `SetSpaceRect`, `ResizeSpace`, and `FitToContents`
  reject locked spaces and snap to the grid; `SnapAll` skips locked spaces.
- `ResizeSpace` also rejects folded spaces and an empty edge set, marks
  the space user-sized, and (with `fit_to_grid`) fits to whole cells of the
  `ViewMetrics` it carries. `FitToContents` clears the user size, so the
  space grows to fit its items, never past its work area; an icon view that
  is capped reflows into whole columns and scrolls (D23). A view that places
  items by order keeps its size otherwise, so the command sizes it once from
  the `ViewMetrics` it carries (`fit_size`), and `Space::can_fit` keeps the
  choice on offer. `KeepSize` sets it without moving the rect. The app sends
  `KeepSize` before re-flowing a self-sizing space whose icon factor changed
  (`App::keep_size_on_zoom`: a zoom, the icon-size setting, a preset, or
  Explorer's own size, never a DPI move), so an icon-size change keeps every
  space's size and re-flows its columns, as an Explorer window does. Settings
  Cancel lets the spaces its preview pinned fit their items again
  (`Session::pinned`).
- `SortSpace { id, rule: SortRule }` sorts by the rule's key (name, type,
  size, or modified), direction, and folders-first, and keeps the rule as the
  space's own; a lens uses the same rule in `SyncLens`.
- `CreateLens { title, at, size, path, locked, view }` clamps like
  `CreateSpace` and starts the lens with a default `FolderView` whose title
  mode is `FolderName`. `SetLensFolder { id, path, title }` rebinds it, sets
  its title text, and clears its items until the next sync when the path
  changes; pointing it at the folder it already shows keeps them.
  `SetLensTitle { id, title, text }` sets how the title follows the folder
  (D17); `SetLensView { id, view }` replaces its folder view; `SyncLens { id,
  items }` swaps in a list a `LensBuilder` finished. The builder keeps ids by
  path, drops hidden entries unless the view shows them, and sorts by the
  space's `SortRule` (natural names, so `file2` comes before `file10`; folders
  first when asked). "Show only"
  (`include`) keeps only files whose name matches, and "Hide" (`exclude`)
  drops any match, file or folder (D18). The lists are capped by
  `FolderView::clamped` when the config loads and in `SetLensView`.
- `CustomizeSpace { id, on, seed }` turns a space's own look on or off,
  seeding it from `seed` (the global settings) only when it has none.
  `SetSpaceLook { id, custom, look }` sets both exactly (clamped): settings
  edits pass `custom: true`, and Cancel uses it to restore a snapshot.
- `SetSpaceView { id, view: SpaceView }` sets a space's view (D17): its icon
  mode, sort rule, and keep-sorted choice, apart from the look. The app
  reflows that space and saves, and lists a lens's folder again when its rule
  changes. Every space has a `Space.view`, so a lens needs no separate command.
- `RestoreSpaces { spaces }` is undo's and redo's way back (ROADMAP D11): it
  puts recorded spaces back exactly, each at its recorded index, without
  applying the layout, and changes nothing if any item id would clash. Undo
  stores no lens items (`Space::undo_snapshot`), so a lens brought back keeps
  the items it holds while it shows the same folder, and its worker lists or
  rebuilds as `App::sync_watches` tells it to.

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
are the two tables: an effect's row names the options it uses and where
"Automatic" sends it, and a kind's row the effects (`Offers`) and options
it offers, the kind it may follow ("Same as startup"), and whether it has
the startup controls. The platform turns an edge into a `Side`, with
Automatic decided, through `Play::side`. `Animations` holds one
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
- A space saves its `source`; a lens's items are omitted because they are a
  live view of its folder (`Space`'s `Serialize`). On load `Config::migrate`
  drops any items a hand-edited file listed under a lens, and the next
  `SyncLens` fills them again.
- `ConfigStore` resolves its paths through `directories::ProjectDirs`
  (`%APPDATA%\DeskSpace\DeskSpace\config\deskspace.toml`) or takes an explicit
  path. `storage_dir()`, where dropped files are moved, is
  `%LOCALAPPDATA%\DeskSpace\DeskSpace\data\storage`, or a `storage` folder
  beside an explicit config file, so scratch configs never touch real
  storage. `save` writes a sibling `*.toml.tmp`, flushes it to disk, and
  renames it, so a crash or a power cut never truncates the config.
- `icons_dir()`, the cache of downloaded icon sets (`icons.rs`,
  `IconStore`, D16), is `%LOCALAPPDATA%\DeskSpace\DeskSpace\data\icons`, or
  an `icons` folder beside an explicit config. One file per set and
  revision. The cache is trusted once written: only the download is
  verified, and a cached file that does not parse is deleted so the next
  need fetches it again.
- `window.icon_downloads` (D16) is `Ask` by default: the app reads a set
  from the cache first, and only `Allowed` downloads a missing one (the
  signed file). `Ask` shows one question a session and saves the answer;
  `Declined` never downloads. `window.new_space_icon` is `Automatic` (match
  the title) or `None`, and is what the New space path passes to
  `CreateSpace`; the seeded first space is always `None`, so a first launch
  never asks.
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
pure layout and hit test), `glyph.rs` (an icon-set glyph as an SVG
document), and `d2d/` (the painters, including the glyph rasterizer).
Everything with `unsafe` is under `win32/`:

| Area | Modules |
|------|---------|
| Runtime and controller | `runtime`, `controller`, `tray`, `console`, `hotkey`, `outside_click` |
| Windows | `window_class` and `window_state` (class registration and `GWLP_USERDATA` state; every window class uses both), `space_window`, `dwm`, `composition`, `drag_ghost`, `info_tip`, `menu`, `settings_window`, `debug_window`, `color_picker`, `dark_mode`, `dialog`, `app_picker`, `app_catalog`, `shortcut_dialog`, `zoom_bar` |
| Input | `keyboard` (the low-level hook), `drop_target` |
| Shell | `desktop_items`, `icons`, `glyphs`, `shell_items`, `shell_view`, `shell_selection`, `display_settings`, `clipboard`, `file_op`, `drag_image`, `folder_watch`, `shortcuts` |
| Desktop takeover | `desktop_shell`, `desktop_icons`, `watchdog`, `desktop_entry` |
| System | `system`, `dpi`, `autostart` |

### Message, timer, and id tables

Ids that other code, docs, or another process cite are never renumbered. A
new one takes the highest value in use plus one; no doc keeps a "next free"
value, because the code shows it. Each list has a uniqueness test or an
exhaustive match:

| Ids | Where | Guard |
|-----|-------|-------|
| `WM_APP + n` messages to the controller (and `FRAME` to space windows) | `win32/messages.rs` (each says who posts it; `TaskbarCreated` is a registered message outside the range) | the `ALL` list in its tests |
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
- Process-wide state: every `static` or `thread_local!` that holds state is
  marked `// Process state:` with what it holds, why it cannot be passed, and
  who clears it; `rg -n "Process state:"` lists them.

### Controller (`win32/controller.rs`, `win32/tray.rs`, `win32/console.rs`)

- The controller is a hidden top-level window because a message-only window
  would miss broadcasts such as `TaskbarCreated` (its module doc). Every
  process-wide input source lands here; `outside_click.rs` explains why
  outside presses need a hook at all, and `tray.rs` owns the icon.
- Exit and the update install leave through one path (`App::exit` →
  `Ui::quit_after`), so the teardown and the takeover restore wait for the
  spaces' Startup Out. A second Exit, a console close, and `WM_ENDSESSION`
  converge on one `CloseRequested`. `console.rs` bounds its wait because
  Windows ends the process soon after a console close handler returns.
- The desktop verb and the `--show` hand-off are one re-launch path
  (`desktop_entry.rs`): a running instance is found by class and told to
  show, else the new process starts shown.
- The tray, settings, and debug windows take their icons from the embedded
  resource (`app_icon`); each window owns its big and small sizes, and the
  debug window's copy carries its badge.

### Space windows (`win32/space_window.rs`, `win32/dwm.rs`)

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
- Everything else about a space window lives where it happens: `dwm.rs` owns
  the material, corners, and border; `composition.rs` the visual tree and
  the startup choice; `space_animation.rs` and `frame_clock.rs` the frames;
  `space_window.rs` the backing, resizing, and mouse modifiers; and
  `shadow_window.rs` / `render/shadow.rs` the shadow. ROADMAP D12 and D15
  give the decisions.

### Drops (`win32/drop_target.rs`)

- Every space and the desktop layer is an OLE `IDropTarget` (`register` /
  `revoke` around the window's life), so a drop chooses its effect and the
  shell draws the move/copy cursor. The pure `drag_kind`, `drop_plan`, and
  `right_drop_*` functions decide what a drag carries, what a drop does, and
  what the source is told; the window only raises `FilesDropped` or
  `ShellItemsDropped`.
- A drop's destination comes from the app through the scene
  (`SpaceScene::drop_into`), so a lens receives a folder and a space its own
  list; the app also owns what a dropped shell item becomes
  (`App::add_shortcuts`). Every file DeskSpace moves is announced with
  `notify_moved` (`win32/desktop_items.rs`), so open shell views follow.

### Drag out and the drag image

- A drag that leaves a space is the shell's (`shell_items::drag_out`), and
  the drop target performs the whole transfer; the app only prunes an item
  whose file is gone. The picture is built once by the app
  (`scene::drag_scene`, `DragImage::of_selection`) and shown two ways: the
  in-space ghost window while rearranging, and the shell's drag-image manager
  once the pointer leaves. A right drag goes straight to the shell, so the
  app builds the same image for it first (`App::drag_image_of`).

### Selection, display matching, keys, and clipboard

- **Clicking a space acts like clicking the desktop**
  (`win32/shell_selection.rs`, `Ui::on_desktop_click`): the shell's desktop
  window is focused and its icon selection cleared; a selection in an Explorer
  window is left alone. The shell view is cached and released before
  `OleUninitialize` (`win32/shell_view.rs`).
- **Explorer display settings** (`win32/display_settings.rs`) are read into
  `ShellDisplay` from the desktop's `IShellView` and `SHGetSetSettings`, then
  re-read at startup, on `Event::SystemThemeChanged`, and on every space
  click; a change repaints every space. Under `Theme::match_explorer()`, the
  app overlays them: icon size and spacing drive the grid, the jumbo icon
  fetch, and the per-monitor scale, while `CheckBoxMode` drives the check
  boxes and the shell's options drive labels, colors, overlays, and display
  names.
- **Inline rename** (`edit.rs`, `win32/keyboard.rs`): the app owns a pure
  `TextEdit` and the painter draws the field. A `WH_KEYBOARD_LL` hook turns
  keys into `Event::EditKey` because spaces never receive `WM_CHAR`, and reads
  modifiers live because `GetKeyboardState` is frozen. F2 renames the focused
  item, or the title with no item selected; a slow second click starts an item
  rename. An item rename goes through `IFileOperation::RenameItem` and
  `Command::RenameItem`; the desktop layer re-syncs and keeps it selected.
- **Other keys**: `Ui::watch_selection_keys` watches outside a rename, and
  `ENGINEERING.md`'s Keys table owns which key needs which flag. The watch
  runs only while the desktop has the foreground, so another app keeps
  Ctrl+C/V. New keys join the `SELECTION_KEY` message or get their own when
  they carry data.
- **Clipboard** (`win32/clipboard.rs`, `win32/file_op.rs`): Ctrl+C/X write a
  `CF_HDROP` and "Preferred DropEffect" (`MOVE` for cut); cut items are drawn
  ghosted. Ctrl+V moves a listed item or copies external files through
  `IFileOperation`, whose progress sink returns the created paths so they are
  listed and selected. Clipboard changes clear stale ghosting and prune files
  a paste moved away; a failed read keeps the ghost.
- **Delete** sends the selection to the Recycle Bin through
  `IFileOperation::DeleteItem` without a prompt; Shift+Delete passes
  `permanent`. Items whose files are gone are dropped, and `restore_recycled`
  finds the newest matching Recycle Bin entry and restores it with the shell's
  `"undelete"` verb.

### Rendering (`render/scene.rs`, `render/d2d/`)

- `SpaceScene` and `SpaceStyle` are the contract between app and renderer.
  `render/scene.rs` documents both field by field: a scene carries everything
  a space draws, and a style only the window-level attributes. Scenes are
  pushed only on change, and the window stores its `PreparedScene`, so a
  repaint allocates nothing.
- `Icon` is an owned bitmap shared by a never-reused id. `d2d/` holds the
  painters, their per-target caches, and the shared Direct2D and DirectWrite
  factories; canvas, text, glyph, and title-bar details belong to the module
  that owns them. A lost device is replaced once and every space repaints
  (`runtime::invalidate_spaces`).
- Animations are pure frame math in `render/`; the window applies each frame,
  and an effect that draws past the window gets an overlay
  (`win32/overlay.rs`). An effect's row may stretch the chosen duration, which
  the app applies when it makes the `Play`. `TASKS.md` section T has the
  recipe.
- The zoom pop-up is its own topmost, never-activated window using the space
  material; `win32/zoom_bar.rs` and `render/zoom_bar.rs` own it.

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
- **Recent colors.** Every pick, from the settings window or a menu, goes
  through `runtime::pick_color`, which updates the runtime's recent colors
  and queues `Event::RecentColors` (queued, so the settings window's color
  branch still dispatches last). The app keeps them in
  `Config::recent_colors`, outside `window`, because a settings Cancel
  restores `window` wholesale.
- **Highlight.** Selecting a space in the scopes pane, or opening the window
  on one, highlights it on the desktop for about 1.2 s
  (`SpaceScene::highlight`, cleared by `Timer::Highlight`). Selecting another
  space moves it; Cancel, OK, and closing the window clear it.
- **The app's form** (`settings/`): the scopes are Global, then every space
  indented, dimmed while it follows Global. Each row shows its kind's glyph in
  the row's text color, so a dimmed row dims its glyph too. Global lists every
  page but View, Shortcuts and Lens, and adds a Lenses page after New spaces
  for a new lens's defaults; a space lists the five look pages, which
  show
  its values (from Global until customized) greyed unless the header's
  "Customize this space" switch is on, its always-enabled View page (the view
  mode, the status bar, the extension choice, and the sort rule, which
  preview through `SetSpaceView`), and its always-enabled Shortcuts page.
  A lens also lists a Lens page: its folder (or "Folder not available"), Open
  folder and Change folder…, its Title (Show as and a custom title, which
  preview through `SetLensTitle`), Show hidden files, which previews
  through `SetLensView`, and its Filter: "Show only" and "Hide", each a
  `;`-separated list (`settings::pages::parse_patterns`). A filter edit
  is a `SetLensView` whose new rules the worker applies to its last listing,
  without reading the folder (`FieldEffect::Lens`); the field shows the text as typed
  (`Session::filter_texts`), so the form never rewrites it mid-edit. A
  lens whose filters leave no entry says "No items match the filter".
  Global edits write `Config` directly; a space's edits
  become `SetSpaceLook` commands, the switch `CustomizeSpace`, and the Colors
  page's reset becomes "Match global appearance". Cancel restores each space's
  `SpaceSnapshot` (its look, its view, a lens's folder view, and its title); a space menu edit
  made while the window is open goes into that snapshot, because the menu
  commits at once. These also commit at once, so Cancel does not undo them:
  - the Lens page's "Change folder…", which writes the new path into the
    snapshot so Cancel keeps it;
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
  choice is edited. The spaces follow their own `mode`. The selection bar,
  marked dots, and picker ring take Windows' accent in the shade Windows'
  own settings use (`UiColors::for_system`, WinRT `UISettings`), and the
  default accent's shades when it cannot be read; an open settings window
  follows an accent change.
- **The debug window** (`--debug`, ROADMAP D14; `win32/debug_window/`,
  `app/debug/`). Modeless and data-driven like settings: the app builds a
  `DebugPanel` from `DEBUG_CONTROLS` and pushes it only when a value
  changed (every second, `Timer::DebugRefresh`, and after each control).
  Every section is built at once as a page inside a scrolled pane, so an
  update never recreates controls. The console is a virtual list view over
  the platform's `ConsoleModel`; the app feeds it from `debug_log.rs`'s
  ring on `Timer::DebugLog`, a timer so a line logged while draining cannot
  feed itself. Its actions call the paths the menus, settings, and drops
  use, save nothing but the animation and shadow settings (D14, D15;
  `app/debug/animation_settings.rs` and `shadow_settings.rs`, through
  `settings::apply_animation_edit` and `apply_shadow_edit`), and
  ask first (`debug_confirm`) when destructive. A control can sit in a
  titled box (`DebugControl::group`), which the page draws as it erases,
  and a choice can be greyed rather than removed, so the panel keeps its
  shape (a shape change rebuilds the window). The
  only state it adds outside `DebugTools` is the session-only
  `AnimationTweak` and the takeover override in `LayerState`.

### Desktop shell, shell items, DPI, hotkeys

- `folder_watch.rs` is the one watcher: one worker on an I/O completion
  port serves every watch, lists the folder, and builds a lens's list, so
  the UI thread does no IO. Its module doc and the `FolderListing` variants
  own the debounce, the retries, the store refusal, and the D19 follow rule.
  The app watches the Desktop only while the layer is drawn and each lens's
  folder while the lens exists (`App::sync_watches`).
- A listing names the folder it read: a posted change can wait in the event
  queue while a key's watch moves, so a lens drops a stale one. The desktop
  layer re-reads the shell instead, and treats another folder as the Desktop
  having moved (`app/lens.rs`).
- `desktop_shell.rs` finds the host, `desktop_icons.rs` hides and restores
  Explorer's listviews, and `watchdog.rs` is the crash-safe half; each
  module doc owns its rules, and the takeover as a whole is D8's.
- `desktop_items.rs`, `icons.rs`, and `info_tip.rs` read the shell without
  blocking the UI thread; their module docs own the enumeration, the icon
  cache and its worker (`icon_queue.rs`), and the tracking tooltip.
- `shell_items.rs` is Explorer behavior for an item: open, the item menu,
  and drag out. Its module doc and `drag_out` own the foreground hand-off,
  the data object, and the image. Drag and drop is why the runtime uses
  `OleInitialize`.
- `dpi.rs` sets per-monitor v2 awareness and `system.rs` wraps the Windows
  queries; use them rather than reading the registry or metrics again.
- `hotkey.rs` parses and registers, and `settings::HOTKEY_BINDINGS` is the
  one table tying each binding to its action, field ids, label, and setting;
  its position is the registration index.

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

`README.md` "Controls" is the user's list, and `ENGINEERING.md`'s Keys table
owns when each key is watched. This section keeps only what those leave out:
exact steps, ranges, edge cases, and the code behind them.

**Spaces**

- Moving snaps to the grid; resizing follows the view's cells (ROADMAP D22).
- Ctrl+wheel: under the default "Match Explorer's display" preset it sets
  the desktop's own icon size, 8 px a notch between 16 and 256, and matching
  spaces follow (ROADMAP D9); under Custom it changes the global size 10 % a
  notch. A space with its own Custom look or a fixed icon view keeps its
  size. Every space re-flows its icons into more or fewer columns;
  "Fit to contents" sizes it to its items again.
- On a space set to one of Explorer's views, Ctrl+wheel, Ctrl+Plus, and
  Ctrl+Minus step through the View menu's order, stop at the ends, and each
  step is one "Change view" undo entry; no zoom pop-up shows for it (W39).
  Ctrl+0 returns to "Settings icon size".
- The zoom pop-up (F-57) shows the size as a percentage (the desktop's
  48 px "Medium icons" is 100 %), greys its buttons at the range ends and
  Reset at 100 %, and hides 2 s after the last zoom or when the pointer
  leaves it.
- Renaming a space (menu or F2) commits on Enter or a click away and cancels
  on Escape. A typed title on a lens is a nickname that stops the title
  following the folder. An empty one follows again, as the Lenses page's
  "Show as" says (Folder name when that is Custom), as one undo entry; on a
  normal space, or on a lens that already follows its folder, it is ignored.

**Menus**

- The space and tray menus are listed in README and built in `app/menus.rs`.
  Rules README leaves out: Undo and Redo are named for the top entry and
  greyed when empty; Fit to contents is offered only on an unlocked space
  that is resized or in an order-placed view; Delete space asks first unless
  the space is empty; a lens's Change folder… refuses a folder
  `ItemStore::allows_lens` rejects with a message, and its Sort by… and Show
  hidden files rebuild the list on the worker. With `--debug`, the tray adds
  Debug… (D14).
- New space opens beside the monitor's top row, one typical gutter right
  (ROADMAP D10); a space from the tray opens on the pointer's monitor, and
  creating one while hidden shows the spaces. New lens… (`App::create_lens`)
  asks for a folder (`Ui::pick_folder`), refuses the store, opens where New
  space would, and starts the watch (`sync_watches`).
- Appearance presets: tint opacity 100/80/60/40/20 % and icon size Small 75,
  Medium 100, Large 150, Extra large 200 %. "Use global appearance" is greyed
  unless customized and keeps the look for later. View presets set 256, 96,
  or 48 px at 96 DPI (out of Ctrl+wheel and Match Explorer); Small icons and
  List draw 16 px inline cells. In Sort by, a key keeps the direction, a
  direction keeps the key, Folders first toggles, and Keep sorted is checked
  and greyed on a lens. The tables are `OPACITY_PRESETS`,
  `ICON_SIZE_PRESETS`, `VIEW_PRESETS`, `SORT_MENU_IDS`, and
  `NEW_SHORTCUT_ENTRIES` in `app.rs`, with `VIEW_MODES` and `SORT_KEYS` in
  `settings/tables.rs`.
- New shortcut ▸ offers installed apps, a custom target, and Paste shortcut,
  enabled only when the clipboard holds shell items, which it leaves
  untouched. A typed target resolves as the Run dialog does (`%VARIABLES%`,
  a bare program on the search path); a missing one is reported in the
  dialog. A web address becomes `.url`, anything else `.lnk`. All paths end
  in `App::add_shortcuts`, which writes into the store or the lens's folder.
- Add files… uses `Ui::pick_files` (multi-select, starting on the Desktop)
  and moves through the drop path (`App::add_dropped`); Cancel adds nothing.

**Settings**

- Global edits apply to every space that is not customized. A space's pages
  show the global values, greyed, until "Customize this space" is ticked; its
  edits then apply to that space alone. A space's header also has a "Delete
  space" button. Selecting a space highlights it on the desktop for about
  1.2 s.
- Edits preview live. Icon size, spacing, and edge padding re-flow every
  space; icon size scales the icon and its grid cell, defaults to 100 %
  (Windows' 48 px desktop icon), and can be typed as a percentage. The shell
  overlay size is the shortcut arrow badge as a percent of the icon (default
  80 %; 100 % matches Explorer). The Layout page's "Fit to contents" button
  (grey unless a resized, unlocked space is selected) commits at once like
  the menu entry.
- The pages, scopes, and fields are `settings/pages.rs`; every editable field
  has a handler (`settings/apply.rs`), and the uniqueness and coverage tests
  point at a missing one. Animations are one page per kind from
  `ANIMATION_PAGES` (D12); everything greys while Windows' "Animation
  effects" is off, which `Event::SystemAnimationsChanged` follows live.

**Items**

- A right drop greys an entry the source refuses; a dropped shortcut file is
  copied rather than linked to.
- Clicking anywhere on a space focuses the desktop and clears its icon
  selection, leaving an Explorer window's selection alone. Empty-space and
  desktop clicks clear the space selection; hiding spaces and folding too.
- A right press selects; past the drag threshold it starts a shell drag, and
  a right click without moving opens the menu.
- Delete has no prompt and is undoable; Shift+Delete is permanent and records
  no undo. A stored file leaves only by dragging it to the desktop (not with
  takeover on) or deleting its space.
- The desktop layer's menu has DeskSpace's entries above Explorer's; a shell
  command re-syncs the layer, and `take_explorer_rename` hands New › Folder's
  rename to DeskSpace. If the view's menu adds nothing, the Desktop folder's
  own menu stands in, so the menu never ends after DeskSpace's entries.

**Keys** (`ENGINEERING.md`'s Keys table says when each is watched)

- F2 keeps a hidden extension when it renames an item and never renames a
  lens's folder. With no space current, Ctrl+Z and Ctrl+Y pass through to
  Explorer. Type-to-select clears its buffer after about a second.
- In Details, right-clicking the header opens Explorer's column menu (Name on
  and grey; one "Choose columns" undo entry); dragging a divider resizes a
  column (Esc or losing the mouse restores; one "Resize column" entry on
  release); dragging a header reorders with a drop mark (Esc cancels; one
  "Reorder columns" entry on release, Name stays first). A click that never
  becomes a drag sorts on release.
- A group header click folds or opens its group (session only). README covers
  the Back rules; `WATCH_BACK` is the hook state that enables them.

**Hotkeys and the command line**

- The Hotkeys page binds show/hide (`Ctrl+Alt+D`), a new space on the
  pointer's monitor, and Settings; the last two start unbound. While spaces
  are hidden, the desktop verb "Show DeskSpace spaces" runs `--show`, which
  signals the running instance instead of starting a second.
- CLI flags are in README; the hidden `--debug` (D14) is not, and `cli.rs` is
  the source of truth for all of them.

## 9. Feature parity

Moved to `ROADMAP.md` section 7, the one feature index; this heading keeps the
section numbers stable.

## 10. Known limitations and technical debt

- **Desktop takeover is opt-in and new** (ROADMAP D8, D9). Explorer owns the
  icon layer unless "Take over the desktop" (or `--takeover`) is on; the
  mechanics and crash safety are in sections 5 and 6. The release zip ships
  `deskspace-watchdog.exe` beside `deskspace.exe` (I6).
- **A taskbar can fall under the desktop layer.** With takeover on, the
  primary taskbar was once seen with its topmost style cleared and below
  the layer, which spans the whole desktop, so its clicks reached the
  layer (2026-10-18). The likely cause is Explorer's full-screen detection
  taking the layer for a full-screen app; the trigger is not known.
  `App::check_taskbars`, on the 5-second health tick, sinks the layer back
  below such a taskbar and logs each occurrence with the taskbars and the
  foreground window (Debug → Windows → Taskbars shows them live). If the
  repair does not hold, the robust fix is one layer per monitor sized to
  its work area.
- **Stored files are not watched.** Deleting a space returns its stored
  files to the desktop; files on another drive stay in place, since a move
  there would copy. Only the Desktop folder is watched (while the layer is drawn), so a stored
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
- **The icon cache is invalidated only after shell menu commands, renames,
  a desktop-layer sync, and a lens listing.** Its key includes the pixel and
  overlay size, so an icon-size or DPI change reloads, and
  `sync_desktop_layer` forgets a loose item whose modification time changed.
  A lens listing invalidates every entry whose `modified` changed. Nothing
  watches the files in spaces, so one whose icon changed in place keeps the
  old one.
- **The scroll indicator is display-only.** Spaces scroll with the wheel,
  Home/End/PageUp/PageDown, and by keeping a keyboard-selected item in view,
  but the indicator cannot be dragged, and wheel events need Windows' "scroll
  inactive windows" setting.
- **Inline rename has no IME or clipboard support.** The hook handles the
  active layout's printable characters and navigation keys only; composition
  (Japanese, Chinese, ...) is deferred. An item rename goes through the shell,
  so a name clash shows Explorer's conflict prompt.
- **Explorer display matching is broad but not total.** Only the desktop's own
  options are read, and its view has only icon sizes to follow (F-40), so a
  check-box setting enabled in one
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
