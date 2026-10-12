# DeskSpace

DeskSpace is a free and open-source desktop organizer for Windows. It groups your desktop icons and desired folder contents into movable boxes called
*spaces* that you can arrange, resize, and style on a per-space basis. Spaces fit right in on
Windows 11, with translucent, softly blurred backgrounds and rounded corners.
DeskSpace is a native app that works offline: no browser engine, no background
service, and most importantly: No tracking or telemetry. DeskSpace will never phone home except to look for updates - and even that is an opt-in setting.

Currently, only desktop items and shortcuts are supported. Very soon, we intend to release *lenses*:
 Lenses are spaces that show the live contents of a folder. Other planned features include auto-hiding, multiple pages,
and saved layouts. See [`ROADMAP.md`](ROADMAP.md) but be forewarned - it's a bit wordy right now!

Status: early but usable. Spaces, desktop items, and the tray, menus, and
settings window are in place; some behaviors are still being checked. This is
not a finished product yet.

## Features

- **Spaces.** Movable, resizable, lockable boxes for your desktop icons, each
  with its own title bar. Roll a space up to just its title when you need room.
- **Real files.** Everything in a space is a real file. Dropping something into
  a space moves the file into a private store (nothing is copied), so items can
  move between spaces without being duplicated.
- **Familiar item controls.** Click, Ctrl+click, Shift+click, and drag a box to
  select; drag items in, out, and between spaces with a picture of what you are
  moving; cut, copy, and paste; delete to the Recycle Bin; rename in place; and
  hover for details. Icons keep their shortcut arrows and badges.
- **Optional desktop takeover.** DeskSpace can hide Explorer's own desktop icons
  and draw them itself for a consistent look. A small helper restores them if
  DeskSpace is closed or crashes.
- **Themes.** All spaces share one look by default, and you can customize a
  single space without changing the rest.
- **Control center.** A tray icon, right-click menus, keyboard shortcuts, and a
  settings window where every option can be changed and previewed live.

## Requirements

- Windows 10 or 11.
- The stable Rust toolchain with the MSVC target (pinned in
  `rust-toolchain.toml`). There are no other system dependencies.

DeskSpace is Windows-only. Blur needs Windows 10 1803 or newer; rounded corners
need Windows 11.

## Build and run

```
cargo build --release
cargo run -p deskspace-app -- --no-icons
```

### Command-line options

```
--config <FILE>          Path to an explicit configuration file
--placement <PLACEMENT>  top-level | desktop-background | desktop-foreground
                         (desktop-hosted spaces are opaque, with no blur;
                         defaults to top-level)
--reset                  Ignore the saved configuration and start fresh
--hidden                 Start with spaces hidden
--show                   Ask the running instance to show its spaces, or start
                         with the spaces shown when none is running
--settings               Open the settings window on start
--no-icons               Do not populate spaces from the desktop folder
--click-through          Make spaces click-through (display only)
-v, --verbose            Increase log verbosity
```

The hidden development flags `--on-top`, `--takeover`, and `--takeover-layer`
help inspect the look and the desktop layer.

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
  wheel scrolls a resized space, and Ctrl+wheel changes the icon size.
- Double-click a space title bar to roll it up or down. Right-click a space for
  roll up, rename, lock, fit to contents, appearance, sort, delete, new space,
  settings, and exit.
- Click an item to select it (Ctrl+click toggles, Shift+click selects a range,
  drag empty space for a box selection, Ctrl+A selects all). Drag a selection to
  rearrange it in a space, or out to another space, a folder, an app, or the
  desktop.
- Double-click an item to open it; right-click for Explorer's context menu.
- `Ctrl+X` / `Ctrl+C` / `Ctrl+V` cut, copy, and paste items. `Delete` sends the
  selection to the Recycle Bin (Shift+Delete removes it for good).
- `F2` renames the selected item, or the current space's title when no item is
  selected. `Alt+Enter` opens the selected item's Properties, or DeskSpace
  Settings when no item is selected. `Enter` opens the selected item, `Escape`
  clears the selection, and `F5` refreshes the spaces.
- Drag files from Explorer or the desktop onto a space to move them into it;
  hold Shift to leave them where they are.
- The tray icon opens Settings on left-click and offers Settings, hide/show
  spaces, new space, and exit on right-click.
- Press `Ctrl+Alt+D` (editable on the Hotkeys settings page) to hide or show the
  spaces.

## Configuration

Settings are saved to a TOML file in your user profile
(`%APPDATA%\DeskSpace\DeskSpace\config\deskspace.toml`). Files dropped into
spaces are moved into a shared store
(`%LOCALAPPDATA%\DeskSpace\DeskSpace\data\storage`). Passing an explicit
`--config <FILE>` puts both the settings and a `storage` folder next to that
file, so a scratch profile never touches your real data.

Every setting can also be changed in the settings window, so you never have to
edit the file by hand. Older settings files are upgraded on load, and an
unreadable file is backed up before defaults are used.

## Project layout

```
crates/deskspace-core      The data model and layout logic (no Windows code)
crates/deskspace-config    Reading, writing, and upgrading the settings file
crates/deskspace-platform  The Windows layer: windows, drawing, tray, menus,
                           and the settings window
crates/deskspace-app       The program itself: the command line, event
                           handling, and the crash helper
```

The platform is the only crate that talks to Windows, and the parts that do not
(the model, layout, and settings) stay free of it. The app describes what to
show and the platform turns that into windows and pixels.

## Documentation

| File | Contents |
|------|----------|
| [`CONTEXT.md`](CONTEXT.md) | Architecture and current behavior (authoritative) |
| [`ROADMAP.md`](ROADMAP.md) | The plan, milestones, and the feature index |
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
