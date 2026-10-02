# CONTEXT

Project context for DeskSpace. Read this before making changes.
`AGENTS.md` documents the build/test workflow and engineering rules.

## 1. What DeskSpace is

DeskSpace is a free and open-source desktop space organizer for Windows: a FOSS
alternative to comparable tools. It groups desktop items into movable containers
("spaces") and, eventually, mirrors folders as live "portals", with roll-up,
auto-hide, and per-space appearance control.

The long-term goal is to match spaces feature-for-feature while remaining
native, lightweight, and offline. The current tree is an early framework, not a
usable product.

### Goals

- Native Win32 rendering and window behavior; no webview, no runtime.
- Windows desktop integration: spaces live on the desktop, not above other apps.
- Fast startup, low idle CPU, no background telemetry.
- Data-driven rendering so UI and platform stay decoupled.
- Simple, forward-compatible configuration.

### Non-goals (for now)

- Cross-platform support. The workspace is Windows-only.
- Cloud sync or accounts.
- Replacing Explorer itself.

### Direction (decided)

See `ROADMAP.md` for the full plan. In short: DeskSpace will take over the
desktop icon layer, render with Direct2D plus DirectComposition, relocate items
dropped into normal spaces by move into a managed store (portals are pure views
and never move data), and ship a tray, context menus, and a settings window in
the first usable release (v0.2).

## 2. Current status

- Initial framework committed as `b90b2ef`.
- `cargo build`, `cargo clippy --workspace --all-targets`, and
  `cargo test --workspace` (23 tests) are green.
- The binary runs: it creates a borderless layered overlay, seeds a `Desktop`
  space from the user's desktop folder, renders spaces with GDI, supports
  dragging spaces by the title bar, roll-up via double-click, a global toggle
  hotkey, and TOML persistence.
- It does not yet integrate with the real desktop icon layer, does not support
  portals, auto-hide, resizing, context menus, a tray, or a settings UI. See
  the parity matrix in section 9.

## 3. Architecture

```
                 +------------------+
                 |  deskspace-app   |  binary: CLI, event handler, scene builder
                 +---------+--------+
                           |  depends on
             +-------------+-------------+
             v                           v
   +------------------+        +----------------------+
   | deskspace-config |        | deskspace-platform   |
   +---------+--------+        |  Win32 + GDI + shell |
             |                 +----------+-----------+
             +------------+---------------+
                          v
                 +------------------+
                 |  deskspace-core  |
                 +------------------+
```

Rules: dependencies point downward only. `deskspace-core` has no OS or IO
dependencies. `deskspace-platform` is the only crate containing `unsafe` or
Win32 types. `deskspace-app` composes everything and owns the Win32 message
loop indirectly through the platform window host.

### Crate responsibilities

- `deskspace-core` — pure domain: geometry, ids, items, spaces, desktop,
  layout engine, command layer, theme. `#![forbid(unsafe_code)]`.
- `deskspace-config` — persisted schema (`Config`, `WindowSettings`),
  versioning/migration, and an atomic `ConfigStore` over TOML.
  `#![forbid(unsafe_code)]`.
- `deskspace-platform` — Win32 window host and message pump, GDI painter,
  desktop-shell discovery (Progman/WorkerW), shell item enumeration and icon
  cache, DPI awareness, global hotkeys, and the event/render data types.
- `deskspace-app` — `clap` CLI, `EventHandler` implementation, model-to-scene
  conversion, and bootstrap seeding.

## 4. Domain model (`deskspace-core`)

### Geometry

- `Point { x, y }`, `Size { width, height }`, `Rect { x, y, width, height }`
  with `contains`, `intersects`, `union`, `offset`, `inset`, `with_position`,
  `with_size`, `center`.
- All are `Copy`, `serde`, `Default`.

### Ids

- `spaceId(u64)` and `ItemId(u64)`, transparent serde newtypes.
- `Desktop::next_space_id` / `next_item_id` derive the next id from the current
  maximum, so ids stay stable across persistence without a serialized counter.

### Item

```
Item { id, path: PathBuf, label: String, kind: ItemKind, position: Point }
ItemKind { File, Folder, Shortcut, Drive, Other }
```

`ItemKind::classify` inspects the filesystem. `label` defaults to the file stem.

### space

```
space {
    id: spaceId,
    title: String,
    rect: Rect,
    items: Vec<Item>,
    locked: bool,
    rolled_up: bool,
    opacity: u8,          // 0..=255, default 210
    title_height: i32,    // default 28
}
```

Helpers: `title_bar()`, `content_rect()` (empty when rolled up), item
lookup/insert/remove, and `required_size` / `resize_to_fit`.

### Desktop

```
Desktop { spaces: Vec<space>, layout: GridSpec }
```

Operations: add/remove space, add/remove/move item, `item_count`,
`space_at(point)` (topmost hit test), `sort_by_id`.

### Layout engine (`layout.rs`)

```
GridSpec {
    cell_width: 96, cell_height: 104, gap: 10, padding: 12,
    title_height: 28, columns: 4, snap: 16,
}
```

- `arrange_space` lays items into a grid inside the space and resizes the space
  to fit (unless locked).
- `snap_to_grid` / `snap_rect` snap positions to the grid.
- `clamp_to_bounds` keeps rects inside the work area.
- `resolve_collisions` pushes overlapping spaces down.

### Command layer (`command.rs`)

All mutations flow through `Desktop::apply(command, bounds) -> CommandOutcome`
(`Applied | NotFound | Rejected`). Variants:

`Createspace`, `Deletespace`, `Renamespace`, `Movespace`, `SetspaceRect`,
`ToggleLock`, `ToggleRollup`, `AddItem`, `RemoveItem`, `MoveItem`,
`AutoArrange`, `SnapAll`.

Move/resize commands reject locked spaces and snap to the grid. This is the
single choke point for domain changes and the intended place to add new
behavior.

### Theme (`theme.rs`)

`Color { r, g, b, a }` with `rgb`/`rgba`/`with_alpha` and hex parse/format.
`Theme` holds `color_key`, space/title/item colors, and `selection`.

## 5. Configuration (`deskspace-config`)

```
Config { version, desktop: Desktop, theme: Theme, window: WindowSettings }

WindowSettings {
    snap_grid: i32,             // default 16
    background_alpha: u8,       // default 255
    always_on_desktop: bool,    // default false
    toggle_hotkey: String,      // default "Ctrl+Alt+D"
    start_hidden: bool,         // default false
}
```

- `CURRENT_VERSION = 1`. `Config::migrate` currently only backfills version 0.
- `ConfigStore` resolves its path via `directories::ProjectDirs` (Roaming
  AppData on Windows; the exact path is machine-specific) with file name
  `deskspace.toml`, or accepts an explicit path.
- `save` writes to a sibling `*.toml.tmp` then renames, so a crash never
  truncates the config.
- `load_or_default` logs a warning and falls back to defaults on a corrupt file.

## 6. Platform layer (`deskspace-platform`)

### Window host (`win32/window.rs`)

- `WindowConfig { title, x, y, width, height, placement, click_through,
  color_key, visible }`.
- `Placement { TopLevel (default), DesktopBackground, DesktopForeground }`.
- `DesktopWindow::create(config, handler)` registers a window class (once),
  creates the window, stores a `Box<WindowState>` behind `GWLP_USERDATA`, and
  returns a handle. `run(self)` pumps messages until `WM_QUIT`.
- `WindowState` owns the `Box<dyn EventHandler>` and the `Painter`; it is freed
  exactly once in `WM_NCDESTROY`.
- `EventHandler::on_event(WindowEvent, &mut WindowContext)` is the app's entry
  point. `WindowContext` exposes `hwnd`, size, color key, `paint(&Scene)`,
  `request_repaint`, and `show(bool)`.
- The window proc translates Win32 messages into `WindowEvent` values and
  dispatches them. `WM_PAINT` resets a paint flag, dispatches `Paint`, and
  validates the region if the handler did not paint.

### Render data (`render/scene.rs`, `render/gdi.rs`)

- `Scene { width, height, color_key, status, spaces: Vec<spaceVisual> }`,
  `spaceVisual`, `ItemVisual`. This is the only contract between the app and the
  renderer.
- `Painter` owns a memory DC and bitmap, recreated on resize, double-buffers
  each frame, and draws rounded spaces, titles, item tiles, icons, and a status
  line with GDI. Fonts are created once. `IconHandle` is an opaque `isize`
  wrapping a Win32 `HICON`.

### Desktop shell (`win32/desktop_shell.rs`)

- Finds the desktop host window: `Progman`, or the wallpaper `WorkerW` located
  via the `0x052C` spawn trick and `EnumWindows`/`FindWindowExW`.
- `TopLevel` does not use a host. Hosted placements create an opaque
  `WS_CHILD` window (layered children are not supported by Win32).

### Shell integration (`shell.rs`)

- `enumerate_desktop_items` resolves the user Desktop known folder and lists
  entries (skipping `desktop.ini`), sorted by label.
- `IconCache` loads 32px icons via `SHGetFileInfoW`, memoizes by path, and
  destroys them on drop.

### DPI and hotkeys (`win32/dpi.rs`, `win32/hotkey.rs`)

- `enable_per_monitor_awareness` sets per-monitor v2, falling back to
  `SetProcessDPIAware`.
- `hotkey::parse` parses strings like `Ctrl+Alt+D`; `register`/`unregister`
  wrap `RegisterHotKey`. `hotkey_id(0)` is the toggle hotkey id.

### Events (`input.rs`)

`WindowEvent`: `Created`, `Paint`, `Resized`, `MouseMove`, `MouseDown`,
`MouseUp`, `MouseLeave` (declared, not currently emitted), `DoubleClick`,
`DpiChanged`, `Hotkey`, `Timer` (declared, no timer currently set),
`VisibilityChanged` (declared, not emitted), `CloseRequested`.

## 7. Runtime flow

1. `main` parses CLI and initializes `tracing`.
2. `app::run` enables DPI awareness, loads or creates `Config`, seeds a default
   `Desktop` space if empty (populated from the desktop folder unless
   `--no-icons`), and builds `WindowConfig` from the CLI.
3. `DesktopWindow::create` creates the overlay and moves the `App` in as the
   event handler.
4. On `Created`, the app records the client size and registers the toggle
   hotkey.
5. On `Paint`, the app builds a `Scene` from `Config.desktop` plus the icon
   cache and hands it to `Painter`.
6. Mouse events hit-test spaces, drag by title bar via `SetspaceRect`, and
   double-clicks toggle roll-up.
7. On `CloseRequested`, the app unregisters the hotkey and saves the config.

## 8. Controls (current)

- Drag a space title bar to move it (snaps to the grid).
- Double-click a space to roll it up or down.
- `Ctrl+Alt+D` (configurable in the config file) hides or shows the overlay.
- CLI flags: `--config`, `--placement`, `--width`, `--height`, `--reset`,
  `--hidden`, `--no-icons`, `--click-through`, `-v`.

## 9. spaces feature parity matrix

| spaces feature | Status |
|---|---|
| spaces (icon groups) | Partial: model + render, but items mirror the desktop folder rather than the real desktop icon layer |
| Folder portals (live folder mirror) | Not implemented |
| Roll-up spaces | Implemented (double-click) |
| Auto-hide / hide all on desktop double-click | Not implemented; only a global overlay hotkey |
| Drag icons between spaces | Not implemented |
| Drag files onto a space to add them | Not implemented |
| Create / rename / delete space | Model + commands exist; no UI |
| Resize space | Not implemented; spaces auto-size to their grid |
| Lock space | Model + move rejection; no UI |
| Snap to grid / auto-arrange | Implemented in core |
| Per-portal sort rules | Not implemented |
| Appearance (color, translucency, blur/Acrylic, icon size, label options) | Minimal: fixed theme, color-key transparency, no blur or per-pixel alpha |
| Multiple pages / virtual desktops | Not implemented |
| Backup / restore / layout snapshots | Config file only |
| Right-click context menu | Not implemented |
| System tray icon | Not implemented |
| Settings UI | Not implemented |
| Multi-monitor | Single fixed-position window |
| Per-monitor DPI | Enabled; not yet propagated to layout/rendering |
| Auto-consolidate new desktop icons | Not implemented |
| Exclude rules / filters | Not implemented |
| Keyboard navigation | Not implemented |
| Quick peek / transparency toggle | Not implemented |

## 10. Known limitations and technical debt

- **No desktop icon integration.** The overlay draws our own tiles; Explorer
  still owns the real desktop icons. spaces parity requires either hiding
  Explorer's icon layer and rendering real items, or driving real desktop items.
- **Color-key transparency, not per-pixel alpha.** Layered child windows are
  unsupported, so hosted placement cannot be translucent or blurred. Acrylic and
  per-space alpha need Direct2D/DirectComposition (or `UpdateLayeredWindow` for
  top-level windows).
- **GDI renderer.** Fine for the skeleton but no anti-aliased rounded corners,
  gradients, or blur. Plan a Direct2D backend behind the same `Scene` contract.
- **Single fixed-size window.** No multi-monitor, no work-area awareness, no
  resize handling beyond tracking client size.
- **No interaction beyond dragging and roll-up.** No item dragging, no context
  menu, no hover states, no resize handles, no keyboard.
- **Portals unimplemented.** No filesystem watching, no live refresh, no sort
  rules, no "show as" modes.
- **`WindowEvent` variants declared but unused** (`MouseLeave`, `Timer`,
  `VisibilityChanged`), and no `SetTimer` is installed.
- **Auto-save only on close** (plus drag end). No periodic or on-change save.
- **Icon cache has no eviction** and is never invalidated.
- **Config is not yet migrated beyond version 0**, and `window.background_alpha`
  / `always_on_desktop` / `start_hidden` are not wired to behavior.
- **No CI, no `deny.toml`, no installer, no code signing.**
- **License is GPL-3.0-or-later** and is a reversible decision if a more
  permissive license is preferred.

## 11. Glossary

- **space** — a movable, resizable container that groups items on the desktop.
- **Portal** — a space whose contents mirror a folder live.
- **Roll-up** — collapsing a space to its title bar.
- **Auto-hide** — hiding all spaces and desktop icons until the desktop is
  interacted with.
- **Placement** — where the overlay lives: a top-level window or hosted on the
  desktop's Progman/WorkerW window.
- **Scene** — the renderer-agnostic draw list produced by the app each frame.
