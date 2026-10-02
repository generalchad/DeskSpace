# DeskSpace

A free and open-source desktop space organizer for Windows. DeskSpace groups
desktop items into movable, resizable spaces.

Status: early framework, not yet feature complete.

## Build

```
cargo build --release
```

Requires the stable Rust toolchain with the MSVC target.

## Run

```
cargo run -p deskspace-app -- --no-icons
```

Options:

```
--config <FILE>          Path to an explicit configuration file
--placement <PLACEMENT>  top-level | desktop-background | desktop-foreground
--width <WIDTH>          Overlay width in physical pixels
--height <HEIGHT>        Overlay height in physical pixels
--reset                  Ignore the saved configuration and start fresh
--hidden                 Start with the overlay hidden
--no-icons               Do not populate spaces from the desktop folder
--click-through          Make the whole overlay click-through (display only)
-v, --verbose            Increase log verbosity
```

## Controls

- Drag a space title bar to move it.
- Double-click a space to roll it up or down.
- Press `Ctrl+Alt+D` (configurable) to hide or show the overlay.

## Layout

```
crates/deskspace-core      Domain model: spaces, items, geometry, layout, commands
crates/deskspace-config    Configuration schema and persistence
crates/deskspace-platform  Win32 window host, GDI renderer, shell integration
crates/deskspace-app       Application binary and CLI
```

Configuration is saved to a TOML file under the per-user config directory.

## License

GPL-3.0-or-later.
