# Changelog

Notable changes to DeskSpace, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) as described in
[`RELEASING.md`](RELEASING.md).

## [Unreleased]

### Added

- "Install updates automatically" in Settings > General: the installed version
  downloads a new release in the background and installs it after you have
  been away for 10 minutes, or when you choose "Restart to update" in the tray
  menu. Off by default.

### Fixed

- Turning on "Check for updates automatically" now takes effect at once
  instead of after a restart.
- Installing an update shuts DeskSpace down cleanly first, so the desktop
  icons are restored and the settings saved before the installer runs.

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
