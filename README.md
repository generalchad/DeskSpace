# DeskSpace

DeskSpace is a free and open-source desktop organizer for Windows. It groups your desktop icons into movable boxes called
*spaces* that you can arrange, resize, and style on a per-space basis. Spaces fit right in on
Windows 11, with translucent, softly blurred backgrounds and rounded corners.
DeskSpace is a native app that works offline: no browser engine, no background
service, and most importantly: no tracking or telemetry. The only network
access is the update check, which runs when you choose "Check for updates…" or,
if you turn it on in Settings, about once a day. You can also let the installed
version install updates on its own while you are away. It asks GitHub for the latest
release and sends nothing about you or your files.

Spaces hold desktop items and shortcuts, and *lenses* show the live contents
of a folder. Other planned features include
auto-hiding, multiple pages, and saved layouts; see
[`ROADMAP.md`](docs/ROADMAP.md) for the full plan.

Status: early but usable. Spaces, desktop items, and the tray, menus, and
settings window are in place; some behaviors are still being checked. This is
not a finished product yet.

## Features

- **Spaces.** Movable, resizable, lockable boxes for your desktop icons, each
  with its own title bar. Fold a space up to just its title when you need room.
- **Real files.** Everything in a space is a real file. Dropping something into
  a space moves the file into a private store (nothing is copied), so items can
  move between spaces without being duplicated.
- **Familiar item controls.** Click, Ctrl+click, Shift+click, and drag a box to
  select; drag items in, out, and between spaces with a picture of what you are
  moving; cut, copy, and paste; delete to the Recycle Bin; rename in place; and
  hover for details. Icons keep their shortcut arrows and badges.
- **Optional desktop takeover** (off by default; turn on "Take over the
  desktop" in Settings > General). DeskSpace can hide Explorer's own desktop icons
  and draw them itself for a consistent look. Closing DeskSpace puts them back,
  and a small helper (`deskspace-watchdog.exe`, kept beside `deskspace.exe`)
  puts them back if it crashes.
- **Themes.** All spaces share one look by default, and you can customize a
  single space without changing the rest.
- **Control center.** A tray icon, right-click menus, keyboard shortcuts, and a
  settings window where every option can be changed and previewed live.

## Install

DeskSpace runs on Windows 10 or 11 (x64). Blur needs Windows 10 1803 or newer;
rounded corners need Windows 11.

Download the latest release from the
[Releases page](https://github.com/generalchad/DeskSpace/releases/latest):

- **Installer**: `deskspace_<version>_x64-setup.exe`. It adds a Start menu
  entry and an uninstaller, and the in-app updater can install new versions.
- **Portable**: `deskspace-v<version>-x86_64-pc-windows-msvc.zip`. Unzip it
  anywhere and run `deskspace.exe`; keep `deskspace-watchdog.exe` beside it.
  Compare the zip with its `.sha256` file to check the download.

DeskSpace is not yet code-signed, so Windows SmartScreen may warn about an
unrecognized app; choose "More info", then "Run anyway".

## Build from source

You need [rustup](https://rustup.rs) with the MSVC toolchain; it installs the
pinned Rust version from `rust-toolchain.toml` on the first build. There are no
other system dependencies.

```
cargo build --release
cargo run -p deskspace-app -- --no-icons
```

[`CONTRIBUTING.md`](CONTRIBUTING.md) covers the development loop, tests, and
checks.

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
-v, --verbose            More logging: -v debug, -vv trace (RUST_LOG overrides)
```

The hidden development flags `--on-top`, `--takeover`, and `--takeover-layer`
help inspect the look and the desktop layer.

## Controls

- Drag a space title bar to move it; drag an edge or corner to resize it. The
  wheel scrolls a space whose items do not fit, and Ctrl+wheel changes the
  icon size: under "Match Explorer's display" (the default) it resizes the
  desktop's own icons and the spaces together, as Ctrl+wheel on the desktop
  does; under "Custom" it changes DeskSpace's icon size. A pop-up at the top
  of the screen shows the size as a percentage, with buttons to zoom out, zoom
  in, and reset to 100 %.
- A new space (from any menu or the hotkey) opens on the screen you clicked
  or pointed at, to the right of the top row of spaces there, lined up with
  them and spaced like them. When that row is full it goes to the next row,
  or a new row underneath.
- Double-click a space title bar to fold or unfold it (animated; Settings >
  Animations > Fold sets the speed). Right-click a space for
  fold, rename, lock, fit to contents, new shortcut, appearance, sort,
  delete, new space, new lens, settings, hide/show spaces, and exit. "New shortcut"
  lists your installed apps (Store apps included) to tick and add,
  opens "Custom shortcut…" to type or browse to any file, folder, program, or
  web address, pastes a shortcut to what you copied in Explorer ("Paste
  shortcut"), or moves files you pick ("Add files…"). In Settings, a space's
  "Shortcuts" page shows a preview of the space: click an item's ✕ to delete
  it, or the + tile to add a shortcut.
- "New lens…" (in a space's menu, the tray menu, and the desktop menu) asks
  for a folder and opens a lens on it: a space that shows what is in the
  folder and follows changes made in Explorer. Dropping, pasting, or adding
  files or shortcuts on a lens puts them in its folder (Explorer asks before
  replacing a file, and "Copy here" copies); deleting an item from a lens
  sends its file to the Recycle Bin. Deleting a lens leaves the folder and
  its files where they are. Right-click a lens to open its folder, change
  it, or choose its order and show hidden files.
- "Sort by" (in a space's menu) is Explorer's menu: Name, Date modified,
  Type, and Size, then Ascending or Descending, then Folders first and Keep
  sorted. A normal space sorts once, or holds the order as items come and go
  once Keep sorted is on; a lens always holds it. "Folders first" stands in
  both directions.
- Click an item to select it (Ctrl+click toggles, Shift+click selects a range,
  drag empty space for a box selection, Ctrl+A selects all). Drag a selection to
  rearrange it in a space (not one that keeps sorted), or out to another space,
  a folder, an app, or the desktop.
- Double-click an item to open it; right-click for Explorer's context menu,
  with DeskSpace's Rename on top; its Delete sends the item to the Recycle Bin,
  and dragging an item onto the desktop takes it out of the space (with the
  desktop takeover on, it opens a new space instead). Click a selected
  item again (slowly) to rename it.
- With the desktop takeover on, right-click an empty spot of the desktop for
  the classic desktop menu (View, Sort by, New, Personalize, ...; on
  Windows 11 the one "Show more options" opens) with New space, New lens,
  DeskSpace settings, and hide/show spaces on top.
- `Ctrl+X` / `Ctrl+C` / `Ctrl+V` cut, copy, and paste items. `Delete` sends the
  selection to the Recycle Bin (Shift+Delete removes it for good).
- `F2` renames the selected item, or the current space's title when no item is
  selected. `Alt+Enter` opens the selected item's Properties, or DeskSpace
  Settings when no item is selected. `Enter` opens the selected item, `Escape`
  clears the selection, `F5` refreshes the spaces, and the Menu key or
  `Shift+F10` opens the context menu. `Ctrl+Plus` and `Ctrl+Minus` make the
  icons larger and smaller, `Ctrl+0` resets them to 100 %, and `Ctrl+Shift+1`,
  `Ctrl+Shift+2`, and `Ctrl+Shift+3` pick Extra large, Large, or Medium icons.
- The arrow keys move the selection (Shift extends it, Ctrl moves only the
  focus), typing a name jumps to it, and `Home` / `End` / `PageUp` /
  `PageDown` scroll the space.
- Drag files from Explorer or the desktop onto a space to move them into it;
  hold Shift to leave them where they are, or Alt to add a shortcut instead.
  A right drag onto a space opens a menu to move or copy the files, create
  shortcuts to them, or just show them.
  Apps dragged from the Start menu or the taskbar become shortcuts.
- With the desktop takeover on, dropping anything on the desktop itself opens
  a new space there with it, an item dragged out of another space included.
- Hiding the spaces (tray, space menu, or hotkey) hides only the spaces; with
  the desktop takeover on, your loose desktop icons stay visible. Hiding and
  showing are animated (a roll by default); pick the effect and its speed
  under Settings > Animations > Startup, which also animates new spaces and
  the spaces appearing at startup (one after another by default) and
  leaving at exit, before Explorer's icons come back (within 2 s). Delete
  and Fold have their own pages; deleting plays Startup's effect by
  default. Press the hotkey again mid-way and the animation turns around.
  DeskSpace follows Windows' "Animation effects" switch: with it off,
  nothing animates.
- The tray icon opens Settings on left-click and offers Settings, hide/show
  spaces, new space, and exit on right-click.
- Press `Ctrl+Alt+D` to hide or show the spaces. The Hotkeys settings page
  changes it and can also bind keys for a new space and for Settings. While
  spaces are hidden, the desktop's right-click menu (under "Show more options"
  on Windows 11) has "Show DeskSpace spaces".

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
crates/deskspace-update    Checking for, downloading, and installing updates
crates/deskspace-app       The program itself: the command line, event
                           handling, and the crash helper
```

The platform is the only crate that talks to Windows, and the parts that do not
(the model, layout, and settings) stay free of it. The app describes what to
show and the platform turns that into windows and pixels.

## Documentation

| File | Contents |
|------|----------|
| [`ENGINEERING.md`](docs/ENGINEERING.md) | Engineering rules, commands, and gotchas |
| [`CONTEXT.md`](docs/CONTEXT.md) | Architecture and current behavior (authoritative) |
| [`ROADMAP.md`](docs/ROADMAP.md) | The plan, milestones, and the feature index |
| [`TASKS.md`](docs/TASKS.md) | Open, one-commit work items and the on-screen check list |
| [`docs/archive/`](docs/archive/) | Finished tasks and the progress log up to v0.2.0 (history) |
| [`CHANGELOG.md`](CHANGELOG.md) | Notable changes in each release |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to report bugs and contribute changes |
| [`AGENTS.md`](AGENTS.md) | A brief for automated agents (points to the above) |
| [`RELEASING.md`](RELEASING.md) | How to cut a release, and the versioning policy |
| [`SECURITY.md`](SECURITY.md) | Supported versions and how to report a vulnerability privately |
| [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) | The Contributor Covenant this project follows |

## Contributing

Contributions are welcome. [`CONTRIBUTING.md`](CONTRIBUTING.md) explains how to
set up, find a task, and submit a change; [`ENGINEERING.md`](docs/ENGINEERING.md)
has the engineering rules. Please report security problems privately as described in
[`SECURITY.md`](SECURITY.md), and follow the
[Code of Conduct](CODE_OF_CONDUCT.md).

## License

Copyright 2026 Timothy W. Brown. Licensed under the Apache License, Version 2.0;
see [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).
