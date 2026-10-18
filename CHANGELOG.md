# Changelog

Notable changes to DeskSpace, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) as described in
[`RELEASING.md`](RELEASING.md).

## [Unreleased]

### Added

- Hiding and showing the spaces, folding one, a new space appearing (at
  startup too, one after another), deleting one, and exiting DeskSpace are
  animated, a roll by default. Settings > Animations has a page each for
  Startup, Delete, and Fold. Settings >
  Animations picks the effect (roll, fade, pixelate, gradient, dissolve, fly,
  CRT, or burn) and its speed, and DeskSpace follows Windows' "Animation
  effects" switch.
- "Install updates automatically" in Settings > General: the installed version
  downloads a new release in the background and installs it after you have
  been away for 10 minutes, or when you choose "Restart to update" in the tray
  menu. Off by default.
- "Create shortcuts here" in the menu a right drag shows on a space, as in
  Explorer: the files stay where they are and the space gets a shortcut to
  each.
- A zoom pop-up at the top of the screen shows the icon size as a percentage,
  with zoom out, zoom in, and Reset. Ctrl+Plus, Ctrl+Minus, and Ctrl+0 zoom
  as well as Ctrl+wheel.
- With the desktop takeover on, dropping something on the desktop (a file, a
  Start or taskbar app, or an item from a space) opens a new space holding it.
- With the desktop takeover on, right-clicking an empty spot of the desktop
  shows New space, DeskSpace settings, and Hide/Show spaces above Explorer's
  own desktop menu. The Menu key and Shift+F10 open it too.
- DeskSpace tells you once when it cannot save its settings, for example when
  the config file is read-only, instead of losing changes silently.

### Changed

- "Roll up" is now called "Fold" (and "Roll down" "Unfold") in the space
  menu, so it no longer sounds like hiding. Nothing else changes.
- A new space opens beside the row of spaces at the top of the monitor,
  with the same gap as the others, instead of on top of whatever is under the
  pointer.
- Under "Match Explorer's display", Ctrl+wheel changes the desktop's own icon
  size, as it does on the desktop, and spaces follow it.
- Changing the icon size keeps every space's size and its title bar; use
  "Fit to contents" to fit a space to its items.

### Fixed

- Turning on "Check for updates automatically" now takes effect at once
  instead of after a restart.
- Installing an update shuts DeskSpace down cleanly first, so the desktop
  icons are restored and the settings saved before the installer runs.
- With the desktop takeover on, a space could slip under the desktop layer
  after Show desktop or a task switch and stop taking clicks and drops.
- The takeover desktop's icons follow an icon-size change.
- Sorting or arranging a space that another space overlaps lays its icons on
  its own grid.
- Ctrl+wheel, Ctrl+click, and Shift+click in a space see the Ctrl and Shift
  keys, so they no longer act as a plain scroll or click.
- The update controls are greyed out in builds that cannot check for updates.
- The release now publishes its update feed as `latest.json`, the name the
  updater reads.
- The settings window fails to build loudly instead of silently skipping a
  page that cannot be created.

## [0.2.3] - 2026-10-14

### Added

- A right-button drag of an item starts a shell drag, and a right drag onto a
  space opens a menu to move, copy, or just show the files.
- "Add files…" in the space menu moves files you pick into the space.
- "Paste shortcut" in the New shortcut submenu adds a shortcut to what you
  copied in Explorer.

### Fixed

- The desktop takeover hides every desktop icon list view, not only a direct
  child of the desktop window.

## [0.2.2] - 2026-10-14

### Added

- A per-space Shortcuts page in Settings, with a preview of the space.
- A New shortcut submenu with an installed-apps picker (Store apps included)
  and custom shortcuts to any file, folder, program, or web address.
- Apps dragged from the Start menu or the taskbar, and other drags that carry
  no file, become shortcuts.

### Changed

- The item menu no longer has "Remove from space": Delete sends the item to
  the Recycle Bin, and dragging it onto the desktop takes it out of the space.

### Fixed

- Every drag a space sees is logged, to diagnose drag sources.

## [0.2.1] - 2026-10-13

### Added

- In-app update checking and installation, on demand or about once a day when
  turned on in Settings.

### Fixed

- The release binaries run without a console window.

## [0.2.0] - 2026-10-13

The first public release (preceded by `0.2.0-alpha.1` the same day).

### Added

- Spaces: movable, resizable, lockable boxes for desktop icons, with title
  bars, roll-up, translucent blurred backgrounds, and rounded corners.
- Real files: dropping into a space moves the file into a private store, and
  items move between spaces without being duplicated.
- Explorer-like item controls: selection, drag and drop, cut, copy, paste,
  delete to the Recycle Bin, rename in place, and hover details.
- The optional desktop takeover, with `deskspace-watchdog` restoring the
  desktop icons if DeskSpace crashes.
- Shared themes with per-space overrides.
- A tray icon, context menus, keyboard shortcuts, global hotkeys, and a
  settings window with live preview.
- A portable zip and an NSIS installer.

[Unreleased]: https://github.com/generalchad/DeskSpace/compare/v0.2.3...HEAD
[0.2.3]: https://github.com/generalchad/DeskSpace/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/generalchad/DeskSpace/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/generalchad/DeskSpace/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/generalchad/DeskSpace/releases/tag/v0.2.0
