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
  `cargo test --workspace` (99 tests) are green.
- Every space is its own top-level window with a Windows 11 look: acrylic or
  blur material behind a translucent tint, DWM rounded corners and border,
  dark/light mode following Windows, Direct2D/DirectWrite content, hover and
  selection highlights. Verified visually on Windows 11 build 26300.
- A tray icon, a space right-click menu, and a native settings window
  (Appearance, General, About) with live preview and OK/Cancel/Apply.
- Spaces can be created, deleted (with confirmation), moved by the title bar,
  resized from any edge or corner, scrolled with the mouse wheel, rolled up
  (double-click the title bar), and locked. Files dropped onto a space are
  moved into the space storage (Shift keeps them in place). Items open on
  double-click, show Explorer's context menu on right-click, and drag out like
  Explorer items. Resize, wheel scrolling, the clear backdrop, the corner
  slider, and the item context menu were checked on screen; dropping in and
  dragging out have not been exercised interactively yet.
- Verification: the blur, corners, space content, and settings layout were
  checked on screen. Delete space, file drops, settings live preview, and the
  tray menu are covered by unit tests of the app logic but have not been
  exercised interactively yet; confirm them before building on them.
- History: commits `07dba93`..`18ffac2` replace the old overlay together and
  only build as a series; bisect across them as one step.
- Not yet: real desktop icon layer takeover (Explorer still shows the desktop
  icons), lenses (phase 0 of M4, resize and scroll, is done), auto-hide,
  rename, undo for file moves.
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
    opacity: u8,          // multiplier on the theme's tint alpha, default 255
    tint: Option<Color>,  // body tint override; None uses the theme
    text: Option<Color>,  // title/label text override; None uses the theme
    title_height: i32,    // default 28
    user_sized: bool,     // set by a resize; keeps size, reflows, scrolls
}
```

Helpers: `title_bar()`, `visible_rect()` (the title bar when rolled up, else
`rect`; used for window size and hit-testing), `content_rect()` (empty when
rolled up), `item_at(point, cell, scroll)`, `content_extent` / `max_scroll`,
`resize_edges_at(point, border) -> ResizeEdges` (none when locked or rolled
up; corners are twice as wide as edges), item lookup/insert/remove, and
`required_size` / `resize_to_fit`.

### Desktop

```
Desktop { spaces: Vec<Space>, layout: GridSpec }
```

`spaces` also reads the pre-rename key `spaces`, so older config files load.

Operations: add/remove space, add/remove/move item, `item_count`,
`item_with_path`, `space_at(point)` (topmost hit test on `visible_rect`),
`sort_by_id`. `move_item` validates the target before removing the item, so a
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
  for user-sized spaces.
- `resize_rect` moves the grabbed edges to a point, snapped, keeping opposite
  edges fixed, at least `min_space_size()` (one cell plus padding) and inside
  the work area. `scroll_step()` is one row per wheel notch.
- `snap_to_grid` / `snap_rect` snap positions to the grid.
- `clamp_to_bounds` keeps rects inside the work area.
- `resolve_collisions` pushes overlapping spaces down.

### Command layer (`command.rs`)

All mutations flow through `Desktop::apply(command, bounds) -> CommandOutcome`
(`Applied | NotFound | Rejected`). Variants:

`CreateSpace`, `DeleteSpace`, `RenameSpace`, `MoveSpace`, `SetSpaceRect`,
`ResizeSpace`, `FitToContents`, `SetSpaceOpacity`, `SetSpaceTint`,
`SetSpaceText`, `SortSpace`, `ToggleLock`, `ToggleRollup`, `AddItem`,
`RemoveItem`, `MoveItem`, `AutoArrange`, `SnapAll`.

Move/resize commands reject locked spaces and snap to the grid; `ResizeSpace`
also rejects rolled-up spaces and marks the space user-sized, and
`FitToContents` clears that again. `bounds` is the
work area of the monitor nearest the action (`work_area_at`).

### Theme (`theme.rs`)

```
Theme {
    mode: ThemeMode,          // System (default) | Dark | Light
    backdrop: Backdrop,       // Acrylic (default) | Blur | Clear | Solid
    corner_radius: u8,        // px, 0..=24, default 8 (theme::corner presets)
    tint_opacity: u8,         // percent, default 40
    title_shading: u8,        // percent, default 30
    show_border: bool,        // default true
    label_lines: u8,          // 1 or 2, default 2
    label_size: u8,           // percent, default 100
    label_shadow: LabelShadow,// Auto (default) | On | Off
    custom_colors: bool,      // default false
    tint: Color, text: Color, // used only when custom_colors
}
```

`Theme::palette(system_dark) -> Palette` resolves concrete colors (tint with
alpha, title fill, text, hover/selected fills, border) from the Windows 11
dark (`#202020` / white) or light (`#F3F3F3` / `#1A1A1A`) base, or the custom
colors. `Backdrop::Solid` forces an opaque tint; `Backdrop::Clear` has no
blur and lets the tint reach 0 %. Enums serialize snake_case;
`Backdrop::Blur` also accepts `"mica"` from early v2 files, and
`corner_radius` also reads the old `corners = "round" | "small" | "square"`.

## 5. Configuration (`deskspace-config`)

```
Config { version, desktop: Desktop, theme: Theme, window: WindowSettings }

WindowSettings {
    snap_grid: i32,             // default 16 (not wired; layout.snap is used)
    always_on_desktop: bool,    // default false (not wired)
    toggle_hotkey: String,      // default "Ctrl+Alt+D"
    start_hidden: bool,         // default false
}
```

- `CURRENT_VERSION = 2`. `Config::migrate` upgrades version 0/1 files: it
  resets the theme (v1 colors were opaque and tuned for the old color-keyed
  overlay), sets every space's `opacity` to 255, and re-runs `position_items`
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
- The tray and settings-window icon is drawn in code (`icon.rs`) until release
  packaging adds an `.ico` resource.

### Space windows (`win32/space_window.rs`, `win32/dwm.rs`)

- One window per space, created by `Ui::create_space`. State is a
  `Box<SpaceState>` behind `GWLP_USERDATA`, freed in `WM_NCDESTROY`.
- Top-level placement: `WS_POPUP` + `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE |
  WS_EX_ACCEPTFILES`, kept at `HWND_BOTTOM` via `WM_WINDOWPOSCHANGING`, never
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
  "scroll inactive windows" setting (on by default). `WM_DROPFILES` becomes
  `SpaceEvent::FilesDropped`, with `keep_in_place` set when Shift is down.
- Inline rename (`edit.rs`, `win32/keyboard.rs`): the app owns a pure
  `TextEdit` buffer and pushes it in the scene's `editing` field; the painter
  draws the selection highlight and caret. A `WH_KEYBOARD_LL` hook, installed
  only while editing, translates key presses into `Event::EditKey` and consumes
  them, because spaces are never activated and cannot receive `WM_CHAR`. IME
  composition and clipboard are not supported.
- Resizing is app-driven like moving: the app hit-tests edges and sets the
  pointer with `Ui::set_space_cursor(CursorShape)`, which the window applies at
  once (WM_SETCURSOR is not sent while the mouse is captured) and on
  `WM_SETCURSOR`.
- `--on-top` (hidden CLI flag) makes spaces topmost so their look can be
  checked without uncovering the desktop.

### Rendering (`render/scene.rs`, `render/d2d.rs`)

- `SpaceScene` (title, colors, optional `text_shadow`, label lines/size,
  flags, `ItemVisual`s in window-local coordinates, optional `scroll_thumb`
  rect) and `SpaceStyle`
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
  `Slider` (optionally with labeled `SliderMark`s), `Color`, `Text`,
  `Button`). The settings window lays them out with
  common controls v6 under a tab control (pages are container windows that
  forward notifications), uses Mica in the caption, reports edits as
  `SettingsEvent::Changed { field, value }` / `Button` / `Ok` / `Cancel` /
  `Apply`, and opens `ChooseColorW` for color fields. `Ui::update_settings`
  diffs a new form of the same shape into the existing controls.
- Dark mode (`win32/dark_mode.rs`): `Ui::set_ui_dark`, which the app calls
  with Windows' app mode on start and on `SystemThemeChanged`, switches
  popup menus (uxtheme ordinals 135/136, `SetPreferredAppMode` and
  `FlushMenuThemes`) and the settings window: dark caption, body and command
  bar colors (`UiColors`), `DarkMode_Explorer` / `DarkMode_CFD` control
  styles, and a tab strip painted by a subclass in both modes, because the
  tab control has no dark style. Static labels are never disabled (Windows
  embosses disabled static text); disabled fields grey them through
  `WM_CTLCOLORSTATIC`. The spaces follow their own Color mode setting.
- The app embeds the common-controls v6 manifest dependency via
  `crates/deskspace-app/build.rs`.

### Desktop shell, shell items, DPI, hotkeys

- `desktop_shell.rs` finds Progman or the wallpaper `WorkerW` (via the
  `0x052C` spawn trick) for hosted placements.
- `shell.rs`: `enumerate_desktop_items` and `IconCache` (32 px icons via
  `SHGetFileInfoW`, decoded to `Icon`s, least-recently-used eviction at 1024
  entries, `invalidate(path)`).
- `shell_items.rs`: Explorer behavior for items. `Ui::open_path`
  (`ShellExecuteExW` with the default context-menu verb; it first passes
  DeskSpace's foreground right on with `AllowSetForegroundWindow`, because
  spaces are never active and opened apps would otherwise stay behind), and
  paths are given to the shell with backslashes. `Ui::item_menu` (the app's entries above the shell's
  `IContextMenu`; the controller forwards owner-draw and submenu messages to
  it while it is open), and `Ui::drag_out` (`SHDoDragDrop` with the item's
  shell data object; reports the effect and whether it ended over the
  desktop). The runtime uses `OleInitialize`, which drag and drop needs.
- `dpi.rs` sets per-monitor v2 awareness; `system.rs` provides
  `work_area_at(point)`, `drag_threshold()`, and `system_uses_dark_mode()`.
- `hotkey.rs`: `parse("Ctrl+Alt+D")`, `hotkey_id(index)`, register/unregister
  (on the controller window via `Ui`).

## 7. Runtime flow

1. `main` parses CLI and initializes `tracing`.
2. `app::run` enables DPI awareness, loads or creates `Config` (migrating
   older files), seeds a `Desktop` space from the desktop folder if there are
   none (unless `--no-icons`), and calls `deskspace_platform::run`.
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
5. Settings edits apply to the live config immediately (preview); Apply/OK
   validate and re-register the hotkey and save; Cancel or closing the window
   restores the saved theme and window settings.
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
  (resized spaces only), Appearance (opacity presets, custom tint or text,
  reset), Delete space (asks first unless the space is empty), New space,
  Settings…, Hide/Show spaces, Exit DeskSpace.
- The space menu's Appearance submenu sets an opacity preset or a custom tint
  or text color (with a reset); Sort by reorders items by name, type, size, or
  modified date.
- Settings → Appearance previews live: material, corners, tint, custom colors,
  label lines/size/shadow, and icon spacing/edge padding (which re-flows every
  space).
- Tray icon: left-click opens Settings; right-click for Settings…, Hide/Show
  spaces, New space, Exit DeskSpace.
- Drop files from Explorer or the desktop onto a space to move them into it;
  hold Shift to leave them where they are and only show them in the space.
- Click an item to select it; click empty space, the desktop, or another
  app to clear the selection (hiding spaces and rolling up do too).
- Double-click an item to open it. Right-click it for Explorer's context
  menu, with "Move back to desktop" (or "Remove from space" for items left in
  place) on top.
- Drag an item out: into a folder or app like an Explorer item, onto another
  space to move it there, or onto the desktop to take it out of the space.
- `Ctrl+Alt+D` (editable in Settings → General) hides or shows spaces.
- CLI flags: `--config`, `--placement`, `--reset`, `--hidden`, `--settings`,
  `--no-icons`, `--click-through`, `-v`, and hidden `--on-top`.

## 9. spaces feature parity matrix

| spaces feature | Status |
|---|---|
| Spaces (icon groups) | Partial: per-space windows; dropped files move into storage; the desktop icon layer is not taken over |
| Folder portals (DeskSpace: lenses) | Not implemented; resize and scroll groundwork done |
| Roll-up spaces | Implemented (title-bar double-click, context menu) |
| Auto-hide / hide all on desktop double-click | Not implemented; hotkey and tray hide/show |
| Drag icons in / out / between spaces | Implemented (shell drag out, `WM_DROPFILES` in) |
| Drag files onto a space to add them | Implemented; files move into storage, Shift keeps them in place |
| Create / rename / delete space | Create and delete via menus; rename has no UI |
| Resize space | Implemented (edges/corners, scrolling, fit to contents) |
| Lock space | Implemented (context menu, lock glyph) |
| Snap to grid / auto-arrange | Implemented in core |
| Per-lens sort rules | Not implemented |
| Appearance (color, translucency, blur, corners) | Implemented globally in Settings (incl. clear glass, corner slider); no per-space UI |
| Multiple pages / virtual desktops | Not implemented |
| Backup / restore / layout snapshots | Config file only |
| Right-click context menu | Space menu; item menu is Explorer's |
| System tray icon | Implemented |
| Settings UI | Appearance, General, About |
| Multi-monitor | Spaces can sit on any monitor; work area of the nearest monitor bounds moves |
| Per-monitor DPI | Process is PMv2; layout is not yet scaled per monitor |
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
- **No per-monitor DPI scaling of spaces.** Sizes are in physical pixels at
  96-DPI values; the settings window is laid out for the DPI at creation.
- **Drops use `WM_DROPFILES`**, which carries file paths only and shows no
  drop-effect feedback (the cursor does not say "Move"). An OLE `IDropTarget`
  would fix that. Dragging out uses OLE already.
- **WindowSettings `snap_grid` and `always_on_desktop` are not wired.**
- **Icon cache is invalidated only after shell menu commands.** Lens watchers
  will invalidate changed files.
- **Scrolling is wheel-only.** The scroll indicator cannot be dragged, and
  wheel events need Windows' "scroll inactive windows" setting.
- **Inline rename has no IME or clipboard support.** A low-level keyboard hook
  handles the active layout's printable characters and navigation keys only;
  composition (Japanese, Chinese, ...) is deferred.
- **No installer and no code signing.** CI (GitHub Actions) runs fmt, clippy,
  and tests, with `deny.toml` for licenses/advisories.
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
