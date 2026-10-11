# CONTEXT

Project context for DeskSpace. Read this before making changes.
`AGENTS.md` documents the build/test workflow and engineering rules.

## 1. What DeskSpace is

DeskSpace is a free and open-source desktop space organizer for Windows: a FOSS
alternative to comparable tools. It groups desktop items into movable containers
("spaces") and, eventually, mirrors folders as live "lenses" (the vendor calls
these Folder Portals), with roll-up,
auto-hide, and per-space appearance control.

The long-term goal is to match spaces feature-for-feature while remaining
native, lightweight, and offline. The current tree is an early but usable
framework, not a finished product.

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
  `cargo test --workspace` (211 tests) are green.
- Every space is its own top-level window with a Windows 11 look: acrylic or
  blur material behind a translucent tint, DWM rounded corners and border,
  dark/light mode following Windows, Direct2D/DirectWrite content, hover and
  selection highlights. Verified visually on Windows 11 build 26300.
- A tray icon, a space right-click menu, and a native settings window in three
  panes: the scopes (Global, then every space indented under it), the pages
  (Appearance, Labels, Layout, Colors, Advanced, plus General and About for
  Global), and the page, with live preview and OK/Cancel/Apply.
- Every space follows the global look until it is customized ("Customize
  this space" in the settings window, or any Appearance edit in its menu);
  it then has its own copy of every look setting (section 4, `SpaceLook`).
- Spaces can be created, deleted (with confirmation), moved by the title bar,
  resized from any edge or corner, scrolled with the mouse wheel, rolled up
  (double-click the title bar), renamed inline, given their own look, sorted,
  and locked; their items can be rearranged by dragging.
- Items have Explorer-style multi-selection (plain click selects one, Ctrl+click
  toggles, Shift+click ranges, rubber-band over empty space with Ctrl/Shift to
  add, Ctrl+A for all, Delete removes all selected). Item check boxes follow
  Explorer's mode: auto-check-select mirrors the selection, or independent item
  check boxes keep a separate session tick set that becomes the operand for
  delete/copy/cut/drag. Labels honor Explorer's extension and
  compressed/encrypted color settings, and icons carry shell overlays. See
  section 6 and the display-settings bullet.
- Files dropped onto a space are moved into the space storage (Shift keeps
  them in place); items open on double-click, show Explorer's context menu on
  right-click, and drag out like Explorer items with a drag image, dragging the
  whole selection as one shell drag. A drag out
  defaults to a move and spaces are OLE drop targets, so the shell shows the
  move/copy cursor. Selected items can be cut, copied, and pasted with
  Ctrl+X/C/V through the shell clipboard, and cut items are ghosted until
  pasted. Delete sends the selection to the Recycle Bin (Shift+Delete removes
  it for good), F2 renames the focused item, or the current space's title when no
  item is selected, inline, Alt+Enter opens the focused item's Properties, or
  DeskSpace Settings when no item is selected, Enter opens the focused item, Esc
  clears the selection, F5 refreshes the spaces, and the Menu key or Shift+F10
  opens the context menu.
  Clicking a space also focuses the desktop and clears its
  icon selection, like a desktop click.
- Spaces mirror the desktop's look: item check boxes (both Explorer modes),
  filename extensions, hidden/protected files, and compressed/encrypted label
  colors. With "Match Explorer's display" on, the desktop's own icon size
  (`IFolderView2::GetViewModeAndIconSize`) and item spacing
  (`IFolderView::GetSpacing`) replace the DeskSpace sliders, so spaces are 1:1
  with the desktop. Icons are rendered by `IShellItemImageFactory` at the
  effective pixel size (crisp at large sizes) with shell overlays composited on
  top, and layouts scale to each monitor's DPI. Hovering an item reveals
  Explorer's info tip.
- Verification: the blur, corners, space content, and settings layout were
  checked on screen. Many behaviors are covered by unit tests of the app logic
  but have not been exercised interactively; confirm them before building on
  them. On screen were checked: inline rename, in-space rearranging, the
  Delete key, per-space appearance, sort, grid/label settings, cross-space
  moves (2026-10-03), and a multi-item drag out (the group image and the
  desktop placement; 2026-10-10). Pending on-screen checks: the move/copy
  cursor, the `- Copy` fix, the drag image over Explorer and over spaces, the
  in-space ghost, the space-click focus/deselect, the check-box display and
  both check-box modes, rubber-band/Ctrl+A selection, the clipboard
  cut/copy/paste with its ghosting and external sync, F2 item rename, the
  compressed/encrypted label colors, the info tips, jumbo icons, and
  per-monitor DPI on a mixed-DPI setup. The
  offscreen item render and a synthetic check-box click were probed on the dev
  machine.
- History: commits `07dba93`..`18ffac2` replace the old overlay together and
  only build as a series; bisect across them as one step.
- Not yet: real desktop icon layer takeover (Explorer still shows the desktop
  icons), lenses (phase 0 of M4, resize and scroll, is done), auto-hide, undo
  for file moves, and mapping Explorer's view mode (list/details).
  See the parity matrix in section 9.

## 3. Architecture

```
                 +------------------+
                 |  deskspace-app   |  binary: CLI, event handler, scene and
                 +---------+--------+  settings-form builders
                           |  depends on
             +-------------+-------------+
             v                           v
   +------------------+        +----------------------+
   | deskspace-config |        | deskspace-platform   |
   +---------+--------+        |  Win32, DWM, D2D,    |
             |                 |  shell, native UI    |
             |                 +----------+-----------+
             +------------+---------------+
                          v
                 +------------------+
                 |  deskspace-core  |
                 +------------------+
```

Rules: dependencies point downward only. `deskspace-core` has no OS or IO
dependencies. `deskspace-platform` is the only crate containing `unsafe` or
Win32 types. `deskspace-app` describes what to show (scenes, menus, settings
forms) as plain data and reacts to events; the platform turns those into
windows and controls.

### Crate responsibilities

- `deskspace-core`: pure domain: geometry, ids, items, spaces, desktop,
  layout engine, command layer, theme and palette. `#![forbid(unsafe_code)]`.
- `deskspace-config`: persisted schema (`Config`, `WindowSettings`),
  versioning/migration, and an atomic `ConfigStore` over TOML.
  `#![forbid(unsafe_code)]`.
- `deskspace-platform`: the UI runtime and message loop, space windows, DWM
  styling, Direct2D painter, tray icon, popup menus, the data-driven settings
  window, desktop-shell discovery, shell item enumeration and icon cache, DPI,
  and global hotkeys.
- `deskspace-app`: `clap` CLI, the `EventHandler` (`App`), model-to-scene
  conversion, the settings form and its mapping onto `Config`, and bootstrap
  seeding.

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
}
ItemKind { File, Folder, Shortcut, Drive, Other }
```

`ItemKind::classify` and `size`/`modified` inspect the filesystem once, when
the item is created. `label` defaults to the file stem.
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
Desktop { spaces: Vec<Space>, layout: GridSpec }
```

`spaces` also reads the pre-rename key `spaces`, so older config files load.

Operations: add/remove space, add/remove/move item, `item_count`,
`space_at(point)` (topmost hit test on `visible_rect`), `sort_by_id`. `move_item` validates the target before removing the item, so a
failed move never loses it.

### Layout engine (`layout.rs`)

```
GridSpec {
    cell_width: 96, cell_height: 104, gap: 10, padding: 12,
    title_height: 28, columns: 4, snap: 16,
}
```

- `arrange_space` resizes the space to fit its items (unless locked or
  user-sized) and calls `position_items`, which lays items on the grid in
  content-relative cells: `columns` wide normally, `columns_for(width)` wide
  for user-sized spaces. It also stamps `grid.title_height` onto the space, so
  a scale change reaches the title even for user-sized spaces.
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
  theme has `match_explorer` on, Explorer's item spacing replaces the cell and
  gap outright.
- `resize_rect` moves the grabbed edges to a point, snapped, keeping opposite
  edges fixed, at least `min_space_size()` (one cell plus padding) and inside
  the work area. `scroll_step()` is one row per wheel notch.
- `snap_to_grid` / `snap_rect` snap positions to the grid.
- `clamp_to_bounds` keeps rects inside the work area.
- `resolve_collisions` pushes overlapping spaces down.

### Command layer (`command.rs`)

All mutations flow through `Desktop::apply_with_layout(command, layout, bounds)
-> CommandOutcome` (`Applied | NotFound | Rejected`), where `layout` is the
effective (scaled) grid; `apply(command, bounds)` is a convenience that uses
the stored base grid. Variants:

`CreateSpace`, `DeleteSpace`, `RenameSpace`, `MoveSpace`, `SetSpaceRect`,
`ResizeSpace`, `FitToContents`, `Reflow`, `CustomizeSpace`, `SetSpaceLook`,
`SortSpace`, `ToggleLock`, `ToggleRollup`, `AddItem`,
`RemoveItem`, `MoveItem`, `ReorderItem`, `ReorderItems`, `RenameItem`,
`AutoArrange`, `SnapAll`.

Move/resize commands reject locked spaces and snap to the grid; `ResizeSpace`
also rejects rolled-up spaces and marks the space user-sized, and
`FitToContents` clears that again. `bounds` is the
work area of the monitor nearest the action (`work_area_at`).
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
    label_size: u8,           // percent, default 100
    icon_size: u8,            // percent, default 100; scales icon and grid cell
    label_shadow: LabelShadow,// Auto (default) | On | Off
    custom_colors: bool,      // default false
    tint: Color, text: Color, // used only when custom_colors
    match_explorer: bool,     // default true; mirror Explorer's display
    use_accent: bool,         // default false; accent for selection and border
}
```

`Theme::palette(system_dark) -> Palette` resolves concrete colors (tint with
alpha, title fill, text, hover/selected fills, border) from the Windows 11
dark (`#202020` / white) or light (`#F3F3F3` / `#1A1A1A`) base, or the custom
colors. `ThemeMode::resolve(system_dark)` decides dark/light for a mode.
`ui_mode` drives `App::ui_dark` and `Ui::set_ui_dark` for DeskSpace's own
chrome; `mode` (through `Theme::is_dark`) colors the spaces.
`Palette::with_accent(accent)` replaces only `selected_fill` and
`border` with the accent at their current alphas; the app applies it in
`App::palette` when `use_accent` is on and
`deskspace_platform::system_accent_color()` (the DWM `AccentColor` registry
value, `0xAABBGGRR`) is known.
`Backdrop::Solid` forces an opaque tint; `Backdrop::Clear` has no
blur and lets the tint reach 0 %. Enums serialize snake_case;
`Backdrop::Blur` also accepts `"mica"` from early v2 files, and
`corner_radius` also reads the old `corners = "round" | "small" | "square"`.

## 5. Configuration (`deskspace-config`)

```
Config { version, desktop: Desktop, theme: Theme, window: WindowSettings }

WindowSettings {
    always_on_desktop: bool,    // default false; hosted placement at startup
    toggle_hotkey: String,      // default "Ctrl+Alt+D"
    start_hidden: bool,         // default false
    start_with_windows: bool,   // default false; per-user Run key at sign-in
}
```

- `CURRENT_VERSION = 2`. `Config::migrate` upgrades version 0/1 files: it
  resets the theme (v1 colors were opaque and tuned for the old color-keyed
  overlay), drops each space's v1 `opacity` (it was a fill alpha), and re-runs `position_items`
  because v1 stored absolute item positions. A missing `version` key
  deserializes as 0.
- `WindowSettings`, `GridSpec`, and `Theme` use struct-level
  `#[serde(default)]`, and `Item.kind` / `Item.position` default, so partial
  tables fill in defaults instead of failing the whole file. Unknown keys (for
  example the removed `background_alpha`) are ignored.
- `ConfigStore` resolves its path via `directories::ProjectDirs` (Roaming
  AppData on Windows) with file name `deskspace.toml`, or accepts an explicit
  path. `storage_dir()` is where dropped files are moved: local AppData
  `DeskSpace\storage`, or a `storage` folder beside an explicit config file,
  so scratch configs never touch real storage.
- `save` writes to a sibling `*.toml.tmp` then renames, so a crash never
  truncates the config.
- `window.start_with_windows` is applied through
  `deskspace_platform::set_start_with_windows` (a new `win32/autostart.rs`),
  which writes or deletes the value `DeskSpace` under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (the quoted
  `current_exe()` path). The app writes it on Apply when the value changed and
  once at startup when enabled, so the stored path follows a moved install; a
  failure is logged at `warn` and never blocks Apply. The uninstaller (E7)
  removes it.
- `load_or_default` returns defaults when no file exists. If the file cannot be
  parsed, it copies it to `deskspace.toml.unreadable-<millis>.bak` before
  falling back to defaults; a file from a newer version is loaded but copied to
  `*.newer-<millis>.bak` first, because saving would drop unknown fields.
  `--reset` copies the existing file to `*.reset-<millis>.bak` and refuses to
  start if that copy fails.

## 6. Platform layer (`deskspace-platform`)

### Runtime (`win32/runtime.rs`)

- `run(RunOptions, Box<dyn EventHandler>)` initialises OLE (single-threaded apartment) and
  common controls, finds the desktop host for the placement, creates the
  controller window, adds the tray icon, installs the console handler,
  delivers `Event::Started`, and pumps messages until quit. It routes
  `IsDialogMessageW` to the settings window for Tab/Enter/Escape handling.
- `EventHandler::on_event(Event, &mut Ui)` is the app's single entry point for
  every window. `Ui` creates/destroys space windows, pushes rects, scenes and
  styles, shows/hides spaces, shows popup menus and confirmation boxes, opens/
  updates/closes the settings window, registers hotkeys, arms and cancels
  timers, sets the tray tooltip, and quits.
- Re-entrancy: the handler is moved out of the thread-local runtime while it
  runs. Win32 calls it makes (SetWindowPos, DestroyWindow, TrackPopupMenu,
  ChooseColor, MessageBox) re-enter window procedures; events raised then are
  queued and delivered by the outermost `dispatch`. Runtime state is only
  borrowed in short sections (`with_runtime`) that never call into Win32.

### Controller (`win32/controller.rs`, `win32/tray.rs`, `win32/icon.rs`)

- A hidden top-level window (not message-only, so it receives broadcasts). It
  owns hotkeys (`WM_HOTKEY`), timers (`Ui::set_timer`/`clear_timer` via
  `WM_TIMER` → `Event::Timer`), the tray callback (`NOTIFYICON_VERSION_4`: left
  click → `TrayEvent::Activated`, right click → `TrayEvent::MenuRequested`),
  `TaskbarCreated` (re-adds the icon after Explorer restarts),
  `WM_SETTINGCHANGE`/"ImmersiveColorSet" (`Event::SystemThemeChanged`), and
  session end.
- Outside clicks (`win32/outside_click.rs`): while the app asks for it
  (`Ui::watch_outside_clicks`, on only while an item is selected), a
  `WH_MOUSE_LL` hook posts each button press to the controller, which
  raises `Event::PointerDownOutside` unless the press hit a space or a popup
  menu. Spaces are never activated, so this is the only way to see clicks on
  the desktop or other apps.
- `Ui::quit`, console Ctrl+C/close, and `WM_ENDSESSION` all lead to exactly one
  `Event::CloseRequested`, then space and settings windows are destroyed, the
  tray icon removed, and the loop ends.
- Desktop integration (`win32/desktop_entry.rs`): while spaces are hidden, the
  app writes a per-user `DesktopBackground\Shell` verb ("Show DeskSpace
  spaces") whose command re-launches the executable with `--show`; on start
  `--show` posts `SHOW_SPACES` to the running instance's controller window
  (found by class) and exits, or starts with the spaces shown when none is
  running. The verb is removed when spaces are shown and on a clean exit.
- The tray and settings-window icon is drawn in code (`icon.rs`) until release
  packaging adds an `.ico` resource.

### Space windows (`win32/space_window.rs`, `win32/dwm.rs`)

- One window per space, created by `Ui::create_space`. State is a
  `Box<SpaceState>` behind `GWLP_USERDATA`, freed in `WM_NCDESTROY`.
  `set_style`, `set_rect`, and `set_scene` return early when the value is
  unchanged (the style is kept in the state, the rect read back from the
  window, the scene compared with the stored one), so restyling every space
  on a settings edit only touches the ones that changed.
- Top-level placement: `WS_POPUP` + `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`,
  kept at `HWND_BOTTOM` via `WM_WINDOWPOSCHANGING`, never
  activated (`MA_NOACTIVATE`). `dwm::apply_style` extends the frame over the
  client area, sets dark mode, corner preference, and border color, and turns
  on the blur material with the undocumented accent policy
  (`SetWindowCompositionAttribute`, `ACCENT_ENABLE_ACRYLICBLURBEHIND` /
  `ACCENT_ENABLE_BLURBEHIND`; Clear uses a fully transparent
  `ACCENT_ENABLE_TRANSPARENTGRADIENT`, because disabling the accent after a
  blur turned the space black until it was recreated). The documented `DWMWA_SYSTEMBACKDROP_TYPE` was
  tried first and does not work here: system backdrops only render on active
  windows, and spaces are never active.
- Desktop-hosted placement (`--placement desktop-*`): `WS_CHILD` of Progman /
  WorkerW, painted opaque, rounded with a (non-anti-aliased) window region.
- Corners and border: DWM draws them for square, 4 px, and 8 px corners
  (`SpaceStyle::dwm_draws_frame`). Clear and Solid spaces with other radii,
  and hosted spaces, are rounded by the painter (anti-aliased fills and
  border); hosted spaces also get a window region so the opaque fill does not
  show past the corners (`Frame` in `space_window.rs`). The blur material
  fills the whole window rectangle and ignores both the window region and
  the `DwmEnableBlurBehindWindow` blur region (tried on build 26300), so
  Acrylic and Blur spaces use the nearest DWM radius at or below the chosen
  one (`Theme::effective_corner_radius`). Larger blurred radii need a
  composition backdrop (M5).
- Mouse input is reported in desktop coordinates as `SpaceEvent`s;
  `TrackMouseEvent` provides `MouseLeave`; capture is held from button-down to
  the last button-up. `WM_MOUSEWHEEL` (screen coordinates) becomes
  `SpaceEvent::Wheel`; it reaches never-focused spaces through Windows'
  "scroll inactive windows" setting (on by default).
- File drops (`win32/drop_target.rs`): every space is an OLE `IDropTarget`
  (`RegisterDragDrop`, revoked in `WM_DESTROY`), not a `WS_EX_ACCEPTFILES`
  window, so it chooses the drop effect and the shell shows the move/copy
  cursor. A `CF_HDROP` drop becomes `SpaceEvent::FilesDropped`; the effect is
  `MOVE` by default and `COPY` while Shift is down, so `keep_in_place` is set
  only for the latter. On `DragEnter` it also stores an empty "DropDescription"
  (`DROPIMAGE_NOIMAGE`, no text) on the source's data object, so a shell source
  such as Explorer or the desktop does not draw its default "Move"/"Copy" action
  hint over the item image. `Drop` marks the source's data object with the
  "Performed DropEffect" format so a shell source does not move the files a
  second time after DeskSpace has. A non-`CF_HDROP` drag reports `DROPEFFECT_NONE`.
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
  `Theme.match_explorer` is on. In particular the desktop icon size and item
  spacing replace the DeskSpace icon-size and grid settings (and drive the
  jumbo icon fetch and the per-monitor layout scale), so matching is 1:1. `ShellDisplay.check_boxes` is a
  `CheckBoxMode`: `FWF_CHECKSELECT` gives `Independent`, `FWF_AUTOCHECKSELECT`
  gives `Selection`, neither `Hidden`. When `Theme.match_explorer` is
  on (default), the app feeds them into each scene: `SpaceScene.show_check_boxes`
  draws an Explorer-style check box at each icon's corner. In `Selection` mode
  the box is revealed on hover and while selected and its tick mirrors the
  selection; in `Independent` mode it is always shown and its tick is a
  session-only set separate from the selection (a failed view read leaves the
  last known mode, so a stale read never hides the boxes).
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
  as Explorer's does; a title's box stays on one line. A `WH_KEYBOARD_LL` hook, installed only while editing,
  translates key presses into `Event::EditKey` and consumes them, because
  spaces are never activated and cannot receive `WM_CHAR`. F2 renames the
  focused item, or the current space's title when no item is selected; an item
  rename is committed with `IFileOperation::RenameItem`
  (undoable; the extension is preserved when the label hid it) and applied
  through `Command::RenameItem`, which updates the item's path and label. The
  same hook watches Delete, F2, Alt+Enter, Enter, Shift+Delete, Esc, F5, the
  Menu key, and Ctrl+C/X while an item is selected and F2, Alt+Enter, F5, the
  Menu key, Ctrl+A, Ctrl+V, Home, End, PageUp, PageDown, and typed characters
  while a space is current (`Ui::watch_selection_keys`) and passes every other
  key through. Alt+Enter
  shows the focused item's Properties (`Ui::show_properties`) or DeskSpace
  Settings; Enter opens the focused item, Esc clears the selection, F5 refreshes
  the spaces, the Menu key or Shift+F10 opens the item's or space's context
  menu, Home/End/PageUp/PageDown scroll the space, and a typed character selects
  the next item whose label starts with it (type-to-select, with a short
  timeout buffer; `Event::Typed`). While a popup menu is open it owns every key, so the hook passes them
  through (`keyboard::set_menu_open`). The watch
  only consumes those keys while the desktop shell has the foreground
  (`desktop_is_foreground`), because the selection is kept when another app is
  focused, as Explorer keeps a desktop selection; otherwise DeskSpace would
  steal that app's own Ctrl+C/V. IME composition and clipboard are not
  supported.
- Clipboard (`win32/clipboard.rs`, `win32/file_op.rs`): Ctrl+C/X put the
  selected items on the shell clipboard as `CF_HDROP` with a "Preferred
  DropEffect" (`MOVE` for a cut), the formats Explorer uses, so they paste into
  Explorer and other apps; cut items render ghosted (`ItemVisual::cut`).
  Ctrl+V pastes into the current space: an item already in a space is
  re-pointed (`MoveItem`), while external files and copies go through
  `IFileOperation` into the store (`FOF_RENAMEONCOLLISION` guards the shared
  store against Explorer's Replace dialog; `FOF_ALLOWUNDO` records an undo),
  and an `IFileOperationProgressSink` returns each created path so the pasted
  items are listed and selected. The keys arrive through the same low-level
  keyboard hook as Delete, now gated on a selection (Ctrl+C/X) or a current
  space (Ctrl+V). Delete sends the selection to the Recycle Bin
  (`IFileOperation::DeleteItem` with `FOF_ALLOWUNDO | FOF_NOCONFIRMATION`, no
  prompt, undoable) and drops the items whose files are gone; the item menu's
  "Remove from space" stays a distinct action. The controller registers
  `AddClipboardFormatListener`; `Event::ClipboardChanged` clears cut ghosting
  when another app or the desktop replaces the clipboard and prunes items whose
  files a paste elsewhere moved away. A clipboard read that fails (another
  process holds it) keeps the ghost rather than clearing it.
- Resizing is app-driven like moving: the app hit-tests edges and sets the
  pointer with `Ui::set_space_cursor(CursorShape)`, which the window applies at
  once (WM_SETCURSOR is not sent while the mouse is captured) and on
  `WM_SETCURSOR`.
- `--on-top` (hidden CLI flag) makes spaces topmost so their look can be
  checked without uncovering the desktop.

### Rendering (`render/scene.rs`, `render/d2d.rs`)

- `SpaceScene` (title, colors, optional `text_shadow`, label lines/size,
  flags, `ItemVisual`s in window-local coordinates, optional `scroll_thumb`
  rect, optional rubber-band `marquee`) and `SpaceStyle`
  (backdrop, corner radius, dark, border) are the contract between app and
  renderer. The app scrolls by offsetting item rects and includes only items
  that intersect the content area; the renderer clips items below the title
  bar. Scenes are pushed only when something changes; the window stores a
  `PreparedScene` with strings pre-encoded to UTF-16, so painting does not
  allocate.
- `Icon` is an `Arc` of owned premultiplied-BGRA pixels with a never-reused
  id, decoded from the shell's HICON (which is destroyed right away). Scenes
  therefore never point at freed handles.
- `Painter` owns an `ID2D1HwndRenderTarget` (premultiplied alpha, 96 DPI so
  one DIP is one pixel), a brush, and a per-target bitmap cache keyed by
  `Icon::id`, pruned to the current scene's icons whenever a scene is set.
  Text uses DirectWrite (Segoe UI Variable Text when
  installed), grayscale antialiasing on translucent surfaces, ellipsis
  trimming, wrapped labels (one or two lines at the configured size; formats
  are cached per 10 % size step), and a 1 px shadow when the scene asks for
  one (label shadow mode `Auto` below 25 % tint alpha, or `On`). `D2DERR_RECREATE_TARGET` drops
  device resources for recreation on the next paint.

### Native UI (`ui.rs`, `win32/menu.rs`, `win32/settings_window.rs`)

- `MenuEntry` lists become `TrackPopupMenuEx` menus (`Ui::popup_menu` returns
  the chosen id).
- `SettingsForm` → pages → `Field`s (`Heading`, `Note`, `Choice`, `Toggle`,
  `Slider` (optionally with labeled `SliderMark`s and, when `editable`, a
  number box that commits on focus loss instead of a static value), `Color`,
  `Text`, `Button`). Appearance is split across Appearance, Labels, Layout,
  Colors, and Advanced so no page clips on a 1080-pixel display. The settings
  window lays the pages out with common controls v6 (pages are container
  windows that forward notifications), uses Mica in the caption, reports edits
  as `SettingsEvent::Changed { field, value }` / `Button` / `Ok` / `Cancel` /
  `Apply`, and opens `ChooseColorW` for color fields. `Ui::update_settings`
  diffs a new form of the same shape into the existing controls, and touches
  the panes and header only when they change, since a slider drag sends an
  update per tick.
- `SettingsForm.nav` (a `SettingsNav`) replaces the tab control with panes,
  separated by hairlines: a scopes pane (`scopes`, each with an `indent` and a
  `dimmed` flag; `ScopeChanged`), a categories pane (`groups` of entries, each
  pointing at a page; untitled groups have no title row; `NavChanged`), and a
  `header` above the page with a title and an optional switch
  (`HeaderSwitch`). Both panes are owner-drawn list boxes painted like Windows
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
- The app's form (`settings.rs`): the scopes are Global, then every space
  indented, dimmed while it follows Global. Global lists all seven pages; a
  space lists only the five look pages and shows its values (from Global
  until customized) greyed unless the header's "Customize this space" switch
  is on. Global edits write `Config` directly; a space's edits become
  `SetSpaceLook` commands, the switch becomes `CustomizeSpace`, and the Colors
  page's reset becomes "Match global appearance". Cancel restores each
  space's `SpaceSnapshot`; a space menu edit made while the window is open is
  taken into that snapshot, because the menu commits at once.
- Dark mode (`win32/dark_mode.rs`): `Ui::set_ui_dark`, which the app calls
  with `Theme.ui_mode` resolved against Windows on start, on the
  `SystemThemeChanged` event, and while the ui_mode choice is edited, switches
  popup menus (uxtheme ordinals 135/136, `SetPreferredAppMode` and
  `FlushMenuThemes`) and the settings window: dark caption, body and command
  bar colors (`UiColors`), `DarkMode_Explorer` / `DarkMode_CFD` control
  styles, and a tab strip painted by a subclass in both modes, because the
  tab control has no dark style. Static labels are never disabled (Windows
  embosses disabled static text); disabled fields grey them through
  `WM_CTLCOLORSTATIC`. The spaces follow their own `mode` setting.
- The app embeds the common-controls v6 manifest dependency via
  `crates/deskspace-app/build.rs`.

### Desktop shell, shell items, DPI, hotkeys

- `desktop_shell.rs` finds Progman or the wallpaper `WorkerW` (via the
  `0x052C` spawn trick) for hosted placements.
- `shell.rs`: `enumerate_desktop_items` and `IconCache`. Icons are rendered by
  `IShellItemImageFactory` at the requested pixel size, and a path the factory
  rejects falls back to the classic 32 px icon (scaled). The overlay (shortcut
  arrow, sync badges) is drawn the way Explorer draws it: `SHGetFileInfoW` with
  `SHGFI_OVERLAYINDEX` names the overlay (this works in any process), and
  `IImageList::GetOverlayImage` fetches its full-icon-sized image from the
  system image list nearest the size (`SHIL_SMALL`..`SHIL_JUMBO`), area-averaged
  to `theme.overlay_size` percent and anchored in the bottom-left corner before
  compositing. HICONs are decoded from their color and
  mask bitmaps (`decode_icon_masked`). Entries are
  keyed by path and size with least-recently-used eviction at 1024 entries.
  `invalidate(path)` forgets every size of one path.
- `shell_items.rs`: Explorer behavior for items. `Ui::open_path`
  (`ShellExecuteExW` with the default context-menu verb; it first passes
  DeskSpace's foreground right on with `AllowSetForegroundWindow`, because
  spaces are never active and opened apps would otherwise stay behind), and
  paths are given to the shell with backslashes. `Ui::item_menu` (the app's entries above the shell's
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
  `drag_threshold()`, and `system_uses_dark_mode()`.
- Info tips (`win32/info_tip.rs`, `Ui::show_info_tip`/`hide_info_tip`): after the
  pointer rests on an item, the app shows Explorer's info tip (name, shell type,
  size, local modification date) in one reusable native tracking tooltip
  (`TTF_TRACK`, `TTM_TRACKPOSITION`/`TTM_TRACKACTIVATE`). The app owns the hover
  delay with a timer; moving off, clicking, or hiding spaces dismisses it.
- `hotkey.rs`: `parse("Ctrl+Alt+D")`, `hotkey_id(index)`, register/unregister
  (on the controller window via `Ui`).

## 7. Runtime flow

1. `main` parses CLI and initializes `tracing`.
2. `app::run` enables DPI awareness, loads or creates `Config` (migrating
   older files), seeds a `Desktop` space from the desktop folder if there are
   none (unless `--no-icons`), and calls `deskspace_platform::run`. Placement
   is `--placement` when given, else desktop-hosted when `always_on_desktop`
   is set, else top-level.
3. On `Started`, the app registers the toggle hotkey, applies start-hidden,
   creates one window per space with its style and scene, and optionally opens
   settings (`--settings`).
4. Pointer events update selection/hover (re-pushing that space's scene), drag
   spaces via `SetSpaceRect` (moving the window), resize them via
   `ResizeSpace` when grabbed within 6 px of an edge, scroll them on the wheel
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
   re-register the hotkey and save; Cancel or closing the window restores the
   saved theme and window settings and every space's snapshot.
6. On `CloseRequested`, the app unregisters the hotkey, discards unapplied
   settings edits, and saves.

## 8. Controls (current)

- Drag a space title bar to move it (snaps to the grid).
- Drag a space edge or corner to resize it; the wheel scrolls overflowing
  content.
- Double-click a space title bar to roll it up or down.
- Rename a space from its right-click menu; the title is edited in place
  (Enter commits, Escape cancels, clicking away commits).
- Right-click a space: Roll up/down, Rename, Lock position, Fit to contents
  (resized spaces only), Appearance, Sort by, Delete space (asks first unless
  the space is empty), New space, Settings…, Hide/Show spaces, Exit DeskSpace.
- The space menu's Appearance submenu sets a tint-opacity preset, an icon size
  (Small/Medium/Large/Extra large), or a custom tint or text color. Each of
  these customizes the space, starting from what it shows now; "Use global
  appearance" makes it follow Global again (keeping its look for later), and
  "Space settings…" opens the settings window on that space. Sort by reorders
  items by name, type, size, or modified date.
- In Settings, pick Global or a space in the left pane. Global edits apply to
  every space that is not customized. A space's pages are greyed, showing the
  global values, until "Customize this space" is ticked; its edits then apply
  to that space alone. Selecting a space flashes it on the desktop, so it can
  be found.
- Settings previews live across its Appearance, Labels, Layout, Colors, and
  Advanced pages: material, corners, tint, custom colors, the Windows accent for
  the selection and border, label lines/size/shadow,
  icon size/spacing/edge padding (which re-flow every space; icon size scales
  the icon and the grid cell around it), and the shell overlay size (the shortcut
  arrow badge, as a percent of the icon; default 80 %, 100 % matches Explorer).
  Icon size defaults to 100 %, which is
  Windows' 48 px desktop icon, and can also be typed as a percentage.
- Tray icon: left-click opens Settings; right-click for Settings…, Hide/Show
  spaces, New space, Exit DeskSpace.
- Drop files from Explorer or the desktop onto a space to move them into it;
  hold Shift to leave them where they are and only show them in the space.
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
- Double-click an item to open it. Right-click it for Explorer's context
  menu, with "Move back to desktop" (or "Remove from space" for items left in
  place) on top; right-clicking an item already in a multi-selection keeps it.
  Rest the pointer on an item for a moment to see Explorer's info tip.
- Item check boxes follow Explorer's mode (when the desktop shows them and
  `match_explorer` is on). With auto-check-select, hovering reveals the box and
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
  The item menu's "Remove from space" / "Move back to desktop" is separate and
  returns a stored file to the desktop.
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
  PageDown scroll the space to the top, the bottom, or one page.
- `Ctrl+Alt+D` (editable in Settings → General) hides or shows spaces. While
  spaces are hidden, a "Show DeskSpace spaces" entry is added to the desktop's
  right-click menu (a per-user `DesktopBackground\Shell` verb) and removed once
  they are shown. It runs the executable with `--show`, which signals the
  running instance through its controller window instead of starting a second
  copy.
- CLI flags: `--config`, `--placement`, `--reset`, `--hidden`, `--show`,
  `--settings`, `--no-icons`, `--click-through`, `-v`, and hidden `--on-top`.

## 9. spaces feature parity matrix

| spaces feature | Status |
|---|---|
| Spaces (icon groups) | Partial: per-space windows; dropped files move into storage; the desktop icon layer is not taken over |
| Folder portals (DeskSpace: lenses) | Not implemented; resize and scroll groundwork done |
| Roll-up spaces | Implemented (title-bar double-click, context menu) |
| Auto-hide / hide all on desktop double-click | Not implemented; hotkey and tray hide/show |
| Drag icons in / out / between spaces | Implemented (OLE drag-out of the whole selection and multi-item rearrange; spaces are OLE `IDropTarget`s) |
| Drag files onto a space to add them | Implemented; files move into storage, Shift keeps them in place |
| Create / rename / delete space | Implemented (create/delete via menus, inline rename) |
| Resize space | Implemented (edges/corners, scrolling, fit to contents) |
| Lock space | Implemented (context menu, lock glyph) |
| Snap to grid / auto-arrange | Implemented in core |
| Per-lens sort rules | Not implemented |
| Appearance (color, translucency, blur, corners) | Implemented globally and per space (a customized space has its own copy of every look setting) |
| Multiple pages / virtual desktops | Not implemented |
| Backup / restore / layout snapshots | Config file only |
| Right-click context menu | Space menu; item menu is Explorer's |
| System tray icon | Implemented |
| Settings UI | Appearance (+ Labels, Layout, Colors, Advanced), General, About |
| Multi-monitor | Spaces can sit on any monitor; work area of the nearest monitor bounds moves |
| Per-monitor DPI | Implemented (per-space layout, icons, and hit tests; `WM_DPICHANGED` reflows) |
| Explorer icon size / spacing / info tips | Implemented under match; jumbo icons at any size |
| Auto-consolidate new desktop icons | Not implemented |
| Exclude rules / filters | Not implemented |
| Keyboard navigation | Not implemented |
| Quick peek / transparency toggle | Not implemented |

## 10. Known limitations and technical debt

- **No desktop icon integration.** Explorer still owns the desktop icon
  layer. Dropping a file moves it into storage, so it leaves the desktop.
- **File moves have no undo yet.** "Move back to desktop" and deleting a space
  return stored files; files from another drive stay in place (a move there
  would copy). There is no filesystem watcher: a stored file deleted outside
  DeskSpace stays listed until an item action notices it is gone, and an
  item's `size`/`modified` (used by sorting) are read once at creation, so
  they can go stale.
- **Spaces can be hidden by Show Desktop (Win+D).** Top-level spaces sit at
  the bottom of the z-order but are not part of the desktop window. Desktop
  hosting fixes this but loses blur (child windows get no DWM material).
- **Blur relies on an undocumented API** (`SetWindowCompositionAttribute`).
  If a future Windows build removes it, spaces fall back to a translucent tint
  without blur. Rounded corners need Windows 11.
- **Dark mode relies on undocumented uxtheme exports** for menus and on
  Explorer's dark control styles. The confirmation message box and the color
  picker stay light; neither has a dark mode.
- **Per-monitor DPI is scaled, with caveats.** Space layout, icons, and hit
  tests scale to each monitor's DPI and `WM_DPICHANGED` reflows a moved space.
  The settings window is still laid out for the DPI at creation, and an item
  moved between monitors of different DPI is positioned from the destination
  space's grid (correct, but not yet verified on screen).
- **Drops use an OLE `IDropTarget`** (`win32/drop_target.rs`), which shows the
  move/copy cursor and replaced `WM_DROPFILES`. `DragOver` recomputes the
  effect from the modifier keys, and the app performs the file move; an
  external shell source is told the move was already performed. Dragging out
  uses OLE too and now defaults to a move.
- **`always_on_desktop` applies at next launch.** Switching placement recreates
  every space window (and drops blur), so it is honored when DeskSpace starts
  rather than live.
- **Icon cache is invalidated only after shell menu commands and renames.** It
  is keyed by path and pixel size, so an icon-size or DPI change reloads
  automatically; lens watchers will invalidate changed files.
- **Scrolling is wheel-only.** The scroll indicator cannot be dragged, and
  wheel events need Windows' "scroll inactive windows" setting.
- **Inline rename has no IME or clipboard support.** A low-level keyboard hook
  handles the active layout's printable characters and navigation keys only;
  composition (Japanese, Chinese, ...) is deferred. Space titles and item file
  names both rename inline; an item rename goes through the shell, so a name
  clash shows Explorer's conflict prompt.
- **Explorer display matching is broad but not total.** Item check boxes handle
  both Explorer modes: auto-check-select (the tick mirrors the selection) and
  independent item check boxes (a session-only tick set that is the operand for
  operations). Compressed/encrypted label colors are imported
  (`SSF_SHOWCOMPCOLOR`). Under "Match Explorer's display" the desktop's icon
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
  undo record, but without a filesystem watcher an undo (or a move made outside
  DeskSpace) leaves the item stale until an icon action prunes it.
- **The desktop "Show spaces" verb is a legacy menu item.** It is a per-user
  `DesktopBackground\Shell` entry, so on Windows 11 it appears under "Show more
  options" (Shift+right-click), not the new top-level menu. Reaching the new
  menu needs an `IExplorerCommand` handler inside an MSIX package with the
  `windows.fileExplorerContextMenus` extension, an associated stub executable,
  and signing (or Developer Mode) - deferred to M8 packaging. Its icon is a
  real `.ico` written beside the storage folder (`icon::write_icon`), since the
  executable has no icon resource yet.
- **No installer, no code signing, no auto-update.** CI (GitHub Actions) runs
  fmt, clippy, and tests, with `deny.toml` for licenses/advisories. Pushing a
  `v*` tag that matches the workspace version runs `release.yml`, which
  publishes an unsigned zip and its SHA-256 to GitHub Releases. Auto-update is
  planned in M8 (F-45).
- **License is Apache-2.0** (copyright Timothy W. Brown; `LICENSE`,
  `NOTICE`). Release packaging must ship both files.

## 11. Glossary

- **Space**: a movable container window that groups items on the desktop
  (comparable tools: "space").
- **Storage**: the folder files dropped into spaces are moved to.
- **Lens**: a space whose contents mirror a folder live (comparable tools:
  "Folder Portal"). In code, a space with a folder source.
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
