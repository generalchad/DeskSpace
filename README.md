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
                         (desktop-hosted spaces are opaque, no blur)
--reset                  Ignore the saved configuration and start fresh
--hidden                 Start with spaces hidden
--settings               Open the settings window on start
--no-icons               Do not populate spaces from the desktop folder
--click-through          Make spaces click-through (display only)
-v, --verbose            Increase log verbosity
```

## Controls

- Drag a space title bar to move it.
- Double-click a space title bar to roll it up or down.
- Right-click a space for roll up, lock, delete, new space, settings, and exit.
- Drag files from Explorer or the desktop onto a space to add them (the files
  are not moved).
- Tray icon: left-click opens Settings; right-click for more.
- Press `Ctrl+Alt+D` (editable in Settings) to hide or show spaces.

## Layout

```
crates/deskspace-core      Domain model: spaces, items, geometry, layout, theme, commands
crates/deskspace-config    Configuration schema, migration, and persistence
crates/deskspace-platform  Space windows, Direct2D rendering, DWM blur, tray, menus,
                           settings window, shell integration
crates/deskspace-app       Application binary, CLI, and event handling
```

Configuration is saved to a TOML file under the per-user config directory.

## License

GPL-3.0-or-later.
