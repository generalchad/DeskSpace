# DeskSpace

DeskSpace is a free and open-source desktop organizer for Windows: a FOSS
alternative to comparable tools. It groups desktop items into movable, resizable
containers called *spaces*, and draws them itself with a native Windows 11 look:
per-pixel-alpha Direct2D content over a DWM blur material, with rounded corners.
It is deliberately lightweight and offline: native Win32, no webview, no
runtime, no telemetry.

The long-term goal is to match spaces feature-for-feature while staying native.
Planned next are *lenses* (live folder views, spaces' "Folder Portals"), roll-up
and auto-hide policies, multiple pages, and layout snapshots. See
[`ROADMAP.md`](ROADMAP.md) for the plan and the full feature index.

Status: early but usable. The renderer, the opt-in desktop takeover, and the
control center (tray, context menus, settings) are in place; some behavior is
still being verified on screen. This is not yet a finished product.

## Features

- **Spaces.** Movable, resizable, lockable, rollable containers for desktop
  items, each its own window with a Windows 11 look.
- **Real desktop items.** Items are real filesystem entries. Dropping an item
  into a space moves the file into a managed store, and moving it between spaces
  never duplicates it.
- **Explorer-like item behavior.** Multi-selection (click, Ctrl+click,
  Shift+click, rubber-band, Ctrl+A), OLE drag and drop with a drag image,
  cut/copy/paste, Recycle-Bin delete, inline rename, info tips, shell icons with
  overlays, and both of Explorer's check-box modes.
- **Desktop takeover (opt-in).** DeskSpace can hide Explorer's icon layer and
  draw the loose desktop items itself, with a crash watchdog that restores the
  icons if the process dies.
- **Per-space looks.** A space follows the global theme until it is customized,
  then keeps its own copy of every look setting.
- **A control center.** A tray icon, space and item context menus, global
  hotkeys, and a data-driven settings window with live preview, OK/Cancel/Apply,
  and Windows accent-color support.

## Requirements

- Windows 10 or 11 (x64).
- The stable Rust toolchain with the MSVC target (pinned in
  `rust-toolchain.toml`). There are no other system dependencies.

The workspace is Windows-only: the platform crate links the `windows` crate
under `cfg(windows)`, so it does not build on other platforms. Blur needs
Windows 10 1803+ or Windows 11; rounded corners need Windows 11.

## Build and run

```
cargo build --release
cargo run -p deskspace-app -- --no-icons
```

### Command-line options

```
--config <FILE>          Path to an explicit configuration file
--placement <PLACEMENT>  top-level | desktop-background | desktop-foreground
                         (desktop-hosted spaces are opaque, no blur; defaults
                         to top-level, or desktop-hosted when the
                         always_on_desktop setting is on)
--reset                  Ignore the saved configuration and start fresh
--hidden                 Start with spaces hidden
--show                   Signal the running instance to show its spaces, or
                         start with the spaces shown when none is running
--settings               Open the settings window on start
--no-icons               Do not populate spaces from the desktop folder
--click-through          Make spaces click-through (display only)
-v, --verbose            Increase log verbosity
```

Hidden development flags `--on-top`, `--takeover`, and `--takeover-layer` help
inspect the look and the desktop layer.

### Development commands

| Task | Command |
|------|---------|
| Build | `cargo build --workspace` |
| Run | `cargo run -p deskspace-app -- --no-icons` |
| Test | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets` |
| Format | `cargo fmt --all` |
| Dependencies | `cargo deny check all` |
| Release | `cargo build --release` |
| Dev loop | `./scripts/dev.ps1` |

`scripts/dev.ps1` builds and runs a scratch profile; see
`Get-Help ./scripts/dev.ps1` for its `-Fresh`, `-OnTop`, `-Settings`, `-Check`,
and `-Release` switches.

## Controls

- Drag a space title bar to move it; drag an edge or corner to resize it. The
  wheel scrolls a resized space, and Ctrl+wheel changes the global icon size.
- Double-click a space title bar to roll it up or down. Right-click a space for
  roll up, rename, lock, fit to contents, appearance, sort, delete, new space,
  settings, and exit.
- Click an item to select it (Ctrl+click toggles, Shift+click ranges, drag empty
  space for a rubber-band selection, Ctrl+A selects all). Drag a selection to
  rearrange it in a space, or out to another space, a folder, an app, or the
  desktop.
- Double-click an item to open it; right-click for Explorer's context menu.
- `Ctrl+X` / `Ctrl+C` / `Ctrl+V` cut, copy, and paste items. `Delete` sends the
  selection to the Recycle Bin (Shift+Delete removes it for good).
- `F2` renames the focused item, or the current space's title when no item is
  selected. `Alt+Enter` opens the focused item's Properties, or DeskSpace
  Settings when no item is selected. `Enter` opens the focused item, `Escape`
  clears the selection, and `F5` refreshes the spaces.
- Drag files from Explorer or the desktop onto a space to move them into it;
  hold Shift to leave them where they are.
- The tray icon opens Settings on left-click and offers Settings, hide/show
  spaces, new space, and exit on right-click.
- Press `Ctrl+Alt+D` (editable on the Hotkeys settings page) to hide or show the
  spaces.

## Configuration

Settings are saved to a TOML file under the per-user configuration directory
(`%APPDATA%\DeskSpace\DeskSpace\config\deskspace.toml`). Files dropped into
spaces are moved into a shared store
(`%LOCALAPPDATA%\DeskSpace\DeskSpace\data\storage`). Passing an explicit
`--config <FILE>` puts both the configuration and a `storage` folder beside that
file, so a scratch profile never touches real data.

Every runtime setting can also be edited from the settings window; the TOML file
is optional. Older configuration files are migrated on load, and an unreadable
file is backed up before defaults are used.

## Architecture

```
crates/deskspace-core      Domain model: geometry, items, spaces, layout, theme, commands
crates/deskspace-config    Configuration schema, migration, and atomic persistence
crates/deskspace-platform  UI runtime: space windows, Direct2D painter, DWM blur,
                           tray, menus, settings window, shell integration
crates/deskspace-app       Application binary, CLI, event handler, and the
                           deskspace-watchdog crash helper
```

Dependencies point downward only: `app -> {config, platform} -> core`. The
`core` crate has no OS or IO dependencies and forbids `unsafe`; all Win32 code
lives in `platform`. Behavior is data-driven: the app describes what to show as
scenes, menus, and settings forms, and the platform turns that data into windows
and pixels.

## Documentation

| File | Contents |
|------|----------|
| [`CONTEXT.md`](CONTEXT.md) | Architecture and current behavior (authoritative) |
| [`ROADMAP.md`](ROADMAP.md) | The plan, milestones, and the feature/parity index |
| [`TASKS.md`](TASKS.md) | Concrete, one-commit work items |
| [`AGENTS.md`](AGENTS.md) | Engineering rules and the build/test workflow |

## Contributing

Contributions are welcome. Read [`AGENTS.md`](AGENTS.md) first, then only the
`CONTEXT.md` sections a change touches. Before considering a change complete,
run:

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check all
```

Keep each commit one logical unit of work, and add tests for new behavior.

## License

Copyright 2026 Timothy W. Brown. Licensed under the Apache License, Version 2.0;
see [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).
