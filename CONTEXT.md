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
  `cargo test --workspace` (81 tests) are green.
- Every space is its own top-level window with a Windows 11 look: acrylic or
  blur material behind a translucent tint, DWM rounded corners and border,
  dark/light mode following Windows, Direct2D/DirectWrite content, hover and
  selection highlights. Verified visually on Windows 11 build 26300.
- A tray icon, a space right-click menu, and a native settings window
  (Appearance, General, About) with live preview and OK/Cancel/Apply.
- Spaces can be created, deleted (with confirmation), moved by the title bar,
  resized from any edge or corner, scrolled with the mouse wheel, rolled up
  (double-click the title bar), and locked. Files dragged from Explorer or the
  desktop onto a space are added to it. Resize and wheel scrolling were
  checked on screen with synthesized input.
- Verification: the blur, corners, space content, and settings layout were
  checked on screen. Delete space, file drops, settings live preview, and the
  tray menu are covered by unit tests of the app logic but have not been
  exercised interactively yet; confirm them before building on them.
- History: commits `07dba93`..`18ffac2` replace the old overlay together and
  only build as a series; bisect across them as one step.
- Not yet: real desktop icon layer takeover (items are references; files are
  not moved), lenses (phase 0 of M4, resize and scroll, is done), auto-hide,
  rename, item drag between spaces.
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
Item { id, path: PathBuf, label: String, kind: ItemKind, position: Point }
ItemKind { File, Folder, Shortcut, Drive, Other }
```

`ItemKind::classify` inspects the filesystem. `label` defaults to the file stem.
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
`ResizeSpace`, `FitToContents`, `ToggleLock`, `ToggleRollup`, `AddItem`,
`RemoveItem`, `MoveItem`, `AutoArrange`, `SnapAll`.

Move/resize commands reject locked spaces and snap to the grid; `ResizeSpace`
also rejects rolled-up spaces and marks the space user-sized, and
`FitToContents` clears that again. `bounds` is the
work area of the monitor nearest the action (`work_area_at`).

### Theme (`theme.rs`)

```
Theme {
    mode: ThemeMode,          // System (default) | Dark | Light
    backdrop: Backdrop,       // Acrylic (default) | Blur | Solid
    corners: CornerStyle,     // Round (8 px) | Small (4 px) | Square
    tint_opacity: u8,         // percent, default 40
    title_shading: u8,        // percent, default 30
    show_border: bool,        // default true
    custom_colors: bool,      // default false
    tint: Color, text: Color, // used only when custom_colors
}
```

`Theme::palette(system_dark) -> Palette` resolves concrete colors (tint with
alpha, title fill, text, hover/selected fills, border) from the Windows 11
dark (`#202020` / white) or light (`#F3F3F3` / `#1A1A1A`) base, or the custom
colors. `Backdrop::Solid` forces an opaque tint. Enums serialize snake_case;
`Backdrop::Blur` also accepts `"mica"` from early v2 files.

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
  path.
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

- `run(RunOptions, Box<dyn EventHandler>)` initialises COM (apartment) and
  common controls, finds the desktop host for the placement, creates the
  controller window, adds the tray icon, installs the console handler,
  delivers `Event::Started`, and pumps messages until quit. It routes
  `IsDialogMessageW` to the settings window for Tab/Enter/Escape handling.
- `EventHandler::on_event(Event, &mut Ui)` is the app's single entry point for
  every window. `Ui` creates/destroys space windows, pushes rects, scenes and
  styles, shows/hides spaces, shows popup menus and confirmation boxes, opens/
  updates/closes the settings window, registers hotkeys, sets the tray
  tooltip, and quits.
- Re-entrancy: the handler is moved out of the thread-local runtime while it
  runs. Win32 calls it makes (SetWindowPos, DestroyWindow, TrackPopupMenu,
  ChooseColor, MessageBox) re-enter window procedures; events raised then are
  queued and delivered by the outermost `dispatch`. Runtime state is only
  borrowed in short sections (`with_runtime`) that never call into Win32.

### Controller (`win32/controller.rs`, `win32/tray.rs`, `win32/icon.rs`)

- A hidden top-level window (not message-only, so it receives broadcasts). It
  owns hotkeys (`WM_HOTKEY`), the tray callback (`NOTIFYICON_VERSION_4`: left
  click → `TrayEvent::Activated`, right click → `TrayEvent::MenuRequested`),
  `TaskbarCreated` (re-adds the icon after Explorer restarts),
  `WM_SETTINGCHANGE`/"ImmersiveColorSet" (`Event::SystemThemeChanged`), and
  session end.
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
  `ACCENT_ENABLE_BLURBEHIND`). The documented `DWMWA_SYSTEMBACKDROP_TYPE` was
  tried first and does not work here: system backdrops only render on active
  windows, and spaces are never active.
- Desktop-hosted placement (`--placement desktop-*`): `WS_CHILD` of Progman /
  WorkerW, painted opaque, rounded with a (non-anti-aliased) window region.
- Mouse input is reported in desktop coordinates as `SpaceEvent`s;
  `TrackMouseEvent` provides `MouseLeave`; capture is held from button-down to
  the last button-up. `WM_MOUSEWHEEL` (screen coordinates) becomes
  `SpaceEvent::Wheel`; it reaches never-focused spaces through Windows'
  "scroll inactive windows" setting (on by default). `WM_DROPFILES` becomes
  `SpaceEvent::FilesDropped`.
- Resizing is app-driven like moving: the app hit-tests edges and sets the
  pointer with `Ui::set_space_cursor(CursorShape)`, which the window applies at
  once (WM_SETCURSOR is not sent while the mouse is captured) and on
  `WM_SETCURSOR`.
- `--on-top` (hidden CLI flag) makes spaces topmost so their look can be
  checked without uncovering the desktop.

### Rendering (`render/scene.rs`, `render/d2d.rs`)

- `SpaceScene` (title, colors, corner radius, flags, `ItemVisual`s in
  window-local coordinates, optional `scroll_thumb` rect) and `SpaceStyle`
  (backdrop, corners, dark, border) are the contract between app and
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
  `Icon::id`, pruned to the current scene's icons whenever a scene is set. Text uses DirectWrite (Segoe UI Variable Text when
  installed), grayscale antialiasing on translucent surfaces, ellipsis
  trimming, two-line wrapped labels. `D2DERR_RECREATE_TARGET` drops device
  resources for recreation on the next paint.

### Native UI (`ui.rs`, `win32/menu.rs`, `win32/settings_window.rs`)

- `MenuEntry` lists become `TrackPopupMenuEx` menus (`Ui::popup_menu` returns
  the chosen id).
- `SettingsForm` → pages → `Field`s (`Heading`, `Note`, `Choice`, `Toggle`,
  `Slider`, `Color`, `Text`, `Button`). The settings window lays them out with
  common controls v6 under a tab control (pages are container windows that
  forward notifications), uses Mica in the caption, reports edits as
  `SettingsEvent::Changed { field, value }` / `Button` / `Ok` / `Cancel` /
  `Apply`, and opens `ChooseColorW` for color fields. `Ui::update_settings`
  diffs a new form of the same shape into the existing controls.
- The app embeds the common-controls v6 manifest dependency via
  `crates/deskspace-app/build.rs`.

### Desktop shell, shell items, DPI, hotkeys

- `desktop_shell.rs` finds Progman or the wallpaper `WorkerW` (via the
  `0x052C` spawn trick) for hosted placements.
- `shell.rs`: `enumerate_desktop_items` and `IconCache` (32 px icons via
  `SHGetFileInfoW`, decoded to `Icon`s, least-recently-used eviction at 1024
  entries, `invalidate(path)`).
- `dpi.rs` sets per-monitor v2 awareness; `system.rs` provides
  `work_area_at(point)` and `system_uses_dark_mode()`.
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
   title-bar double-click, and open the space menu on right-click. Drops add
   or move items. Config is saved after drags and resizes, roll-up, lock,
   create/delete, drops, and Apply/OK.
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
- Right-click a space: Roll up/down, Lock position, Fit to contents (resized
  spaces only), Delete space…, New space, Settings…, Hide/Show spaces, Exit
  DeskSpace.
- Tray icon: left-click opens Settings; right-click for Settings…, Hide/Show
  spaces, New space, Exit DeskSpace.
- Drag files from Explorer or the desktop onto a space to add them.
- `Ctrl+Alt+D` (editable in Settings → General) hides or shows spaces.
- CLI flags: `--config`, `--placement`, `--reset`, `--hidden`, `--settings`,
  `--no-icons`, `--click-through`, `-v`, and hidden `--on-top`.

## 9. spaces feature parity matrix

| spaces feature | Status |
|---|---|
| Spaces (icon groups) | Partial: per-space windows; items are references, the real desktop icons stay |
| Folder portals (DeskSpace: lenses) | Not implemented; resize and scroll groundwork done |
| Roll-up spaces | Implemented (title-bar double-click, context menu) |
| Auto-hide / hide all on desktop double-click | Not implemented; hotkey and tray hide/show |
| Drag icons between spaces | Not implemented (dropping a path already in another space moves it) |
| Drag files onto a space to add them | Implemented as references; files are not moved yet |
| Create / rename / delete space | Create and delete via menus; rename has no UI |
| Resize space | Implemented (edges/corners, scrolling, fit to contents) |
| Lock space | Implemented (context menu, lock glyph) |
| Snap to grid / auto-arrange | Implemented in core |
| Per-lens sort rules | Not implemented |
| Appearance (color, translucency, blur, corners) | Implemented globally in Settings; no per-space UI |
| Multiple pages / virtual desktops | Not implemented |
| Backup / restore / layout snapshots | Config file only |
| Right-click context menu | Basic space menu |
| System tray icon | Implemented |
| Settings UI | Appearance, General, About |
| Multi-monitor | Spaces can sit on any monitor; work area of the nearest monitor bounds moves |
| Per-monitor DPI | Process is PMv2; layout is not yet scaled per monitor |
| Auto-consolidate new desktop icons | Not implemented |
| Exclude rules / filters | Not implemented |
| Keyboard navigation | Not implemented |
| Quick peek / transparency toggle | Not implemented |

## 10. Known limitations and technical debt

- **No desktop icon integration.** Explorer still owns the real desktop icons;
  spaces show references. Dropping a file adds a reference without moving it.
- **Spaces can be hidden by Show Desktop (Win+D).** Top-level spaces sit at
  the bottom of the z-order but are not part of the desktop window. Desktop
  hosting fixes this but loses blur (child windows get no DWM material).
- **Blur relies on an undocumented API** (`SetWindowCompositionAttribute`).
  If a future Windows build removes it, spaces fall back to a translucent tint
  without blur. Rounded corners need Windows 11.
- **Settings window is light-only**; classic controls have no supported dark
  mode. Popup menus are light as well.
- **No per-monitor DPI scaling of spaces.** Sizes are in physical pixels at
  96-DPI values; the settings window is laid out for the DPI at creation.
- **Drops use `WM_DROPFILES`**, which carries file paths only and shows no
  drop-effect feedback. An OLE `IDropTarget` is needed for richer feedback and
  for drag-out.
- **WindowSettings `snap_grid` and `always_on_desktop` are not wired.**
- **Icon cache is never invalidated automatically.** It is LRU-bounded and has
  `invalidate(path)`, but nothing calls it until lens watchers exist.
- **Scrolling is wheel-only.** The scroll indicator cannot be dragged, and
  wheel events need Windows' "scroll inactive windows" setting.
- **No CI, no `deny.toml`, no installer, no code signing.**
- **License is GPL-3.0-or-later** and is a reversible decision if a more
  permissive license is preferred.

## 11. Glossary

- **Space**: a movable container window that groups items on the desktop.
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
