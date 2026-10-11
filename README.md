# DeskSpace

A free and open-source desktop space organizer for Windows. DeskSpace groups
desktop items into movable spaces with a Windows 11 look: blurred, translucent
backgrounds and rounded corners.

Status: early framework, not yet feature complete. See `ROADMAP.md`.

## Build

```
cargo build --release
```

Requires Windows 10 or 11 and the stable Rust toolchain with the MSVC target.
Blur works on Windows 10 1803+ and Windows 11; rounded corners need Windows 11.

## Run

```
cargo run -p deskspace-app -- --no-icons
```

Options:

```
--config <FILE>          Path to an explicit configuration file
--placement <PLACEMENT>  top-level | desktop-background | desktop-foreground
                         (desktop-hosted spaces are opaque, no blur; defaults
                         to top-level, or desktop-hosted when the
                         always_on_desktop setting is on)
--reset                  Ignore the saved configuration and start fresh
--hidden                 Start with spaces hidden
--settings               Open the settings window on start
--no-icons               Do not populate spaces from the desktop folder
--click-through          Make spaces click-through (display only)
-v, --verbose            Increase log verbosity
```

## Controls

- Drag a space title bar to move it; drag an edge or corner to resize (the wheel
  scrolls a resized space).
- Double-click a space title bar to roll it up or down; right-click a space for
  roll up, rename, lock, fit to contents, appearance, sort, delete, new space,
  settings, and exit.
- Click an item to select it (Ctrl+click toggles, Shift+click ranges, drag empty
  space for a rubber-band selection, Ctrl+A selects all). Drag a selection to
  rearrange it in a space or out to another space, folder, app, or the desktop.
- Double-click an item to open it; right-click for Explorer's context menu.
- `Ctrl+X` / `Ctrl+C` / `Ctrl+V` cut, copy, and paste items; `Delete` sends the
  selection to the Recycle Bin; `F2` renames the focused item, or the current
  space's title when no item is selected; `Alt+Enter` opens the focused item's
  Properties, or DeskSpace Settings when no item is selected.
- Drag files from Explorer or the desktop onto a space to move them into it
  (hold Shift to leave them where they are).
- Tray icon: left-click opens Settings; right-click for more.
- Press `Ctrl+Alt+D` (editable in Settings) to hide or show spaces.

## Layout

```
crates/deskspace-core      Domain model: spaces, items, geometry, layout, theme, commands
crates/deskspace-config    Configuration schema, migration, and persistence
crates/deskspace-platform  Space windows, Direct2D rendering, DWM blur, tray, menus,
                           settings window, shell integration
crates/deskspace-app       Application binary, CLI, and event handling (plus the
                           deskspace-watchdog crash helper)
```

Configuration is saved to a TOML file under the per-user config directory.

## License

Copyright 2026 Timothy W. Brown. Licensed under the Apache License, Version
2.0; see `LICENSE` and `NOTICE`.
