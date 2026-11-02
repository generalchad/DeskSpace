# Changelog

Notable changes to DeskSpace, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) as described in
[`RELEASING.md`](RELEASING.md).

## [Unreleased]

### Added

- A redesigned color picker: a color square with hue and opacity sliders,
  hex, RGB, or HSL text, copy and paste, an eyedropper that samples any pixel
  on screen, and recent colors that survive a restart and a settings Cancel.
  On Windows 11 22H2 and later it is acrylic, as Explorer's own picker is.
- Space icons: a space can show a Fluent UI glyph left of its title, matched
  from its title or named in the config (`icon = "fluent:games"`). New spaces
  can start with one (Settings > New spaces), and the icon set is downloaded
  once from DeskSpace's releases after asking (Settings > General); nothing
  about you or your files is sent. The icon takes the title's color, so it
  follows a dark and light switch, and a space without one keeps its old
  title bar. Settings > About credits the icon sets.
- DeskSpace entries on the desktop's right-click menu: New space, DeskSpace
  Settings, and Show spaces while the spaces are hidden. Each opens the
  running app's window or adds the space there, without starting a second
  copy.
- Lenses: a space that shows a folder's live contents. "New lens…" in a
  space's menu, the tray, and the desktop menu asks for a folder and shows it;
  files dropped, pasted, or added as shortcuts go into that folder, and
  deleting a lens never touches it. A lens keeps its sort rule as files come
  and go, has Open folder and Change folder… entries, and shows its folder's
  icon and name in its title bar.
- Explorer's eight views for every space: Extra large, Large, Medium, and
  Small icons, List, Details, Tiles, and Content, from the View menu,
  Ctrl+Shift+1..8, or the status bar's buttons. Details has Explorer's
  columns and sortable headers, Tiles its type and size, and Content its
  separators.
- A status bar for a space, on for new lenses: Explorer's item counts, the
  selected files' size, and its two view buttons. Settings > View toggles it.
- Settings > Lens for a lens (folder, title, and hidden files), Settings >
  View for every space (view, status bar, and sort rule), and Settings >
  Lenses for the defaults a new lens starts with.
- Sort by now has Explorer's shape: Name, Date modified, Type, and Size;
  Ascending or Descending; Folders first; and Keep sorted, which holds a
  space's order as items change.
- Browsing inside a lens: double-click a folder to show it in place, and go
  back with the title bar's Back button, Backspace, Alt+Left, Alt+Up, or the
  mouse's back button. A lens never goes above its own folder.
- Group by for every space, with Explorer's keys and group names, and groups
  that fold.
- Lens filters: "Show only" and "Hide" lists of `*` and `?` wildcards on
  Settings > Lens.
- A lens follows its folder when it is renamed or moved on the same volume,
  says where it went, and Ctrl+Z points it back.
- Details columns: choose them from the header's menu, drag a divider to
  resize, and drag a header to reorder. Date created joins as a column and a
  sort key.
- A compact view per space, file name extensions per space (Automatic, Show,
  or Hide), and a default view for new spaces on Settings > New spaces.
- Ctrl+wheel and Ctrl+Plus/Minus step through the views on a space set to one
  of them, and Ctrl+0 goes back to "Settings icon size".
- Hovering a lens's title shows its folder's path.
- "Fit to contents" on an item's menu in a space.
- A Bounce animation: a space drops in from beyond an edge of its monitor's
  work area and settles in shrinking bounces, and hides the same way
  backwards. It plays at twice the chosen duration, so each bounce can be
  seen. Settings > Animations offers it for Startup and Delete, with its own
  edge.

- Undo and redo for layout changes, file moves, item removal, space creation
  and deletion, copies, pastes, and new shortcuts. Ctrl+Z and Ctrl+Y act while
  a space is current, and the space menu and the tray name the action ("Undo
  Move 3 items"). Undo and redo last for the session. Shift+Delete and edits
  in the settings window are not undone.
- Drawn shadows on Windows 11: DeskSpace draws each space's shadow, so it
  follows the space's corners and fades with an animation where Windows casts
  none. Settings > Appearance adds "Show shadow" (on by default), "Shadow
  size", and "Shadow opacity".
- A border thickness, a border color (the Windows accent or a custom color),
  and its own opacity in Settings > Appearance and Settings > Colors.
- With the desktop takeover on, a loose desktop icon renames in place, as an
  item in a space does.

### Changed

- The window border is white in dark mode and black in light mode, as
  Explorer's borders are. "Use Windows accent color" now colors the selection
  only, unless "Border color" is set to the Windows accent.
- "Snap resized spaces to whole columns and rows" fits the current view's
  cells: Details and Content snap to rows only.
- Fit to contents never makes a space larger than its monitor's work area; a
  space that is capped scrolls.

### Fixed

- A lens on a very large folder no longer stalls every space when it refreshes
  or when its sort or filters change: its list is built in the background.
- A locked lens places its items instead of stacking them.
- A slow or network folder no longer stalls every space while its icons load.
  An item shows a generic icon at once, and its own fills in, as in Explorer.
- With the desktop takeover on: a taskbar that ended up under the desktop
  layer takes clicks again; Delete removes loose desktop icons; New › Folder
  opens a visible rename box; and an icon updates when its file changes in
  place, without a restart.
- A right drag shows DeskSpace's drag image rather than the shell's, and each
  entry in the right-drop menu is greyed when the source cannot do it.
- The settings window's selection follows a change to the Windows accent
  color.

## [0.2.4] - 2026-10-18

### Added

- Hiding and showing the spaces, folding one, a new space appearing (at
  startup too, one after another), deleting one, and exiting DeskSpace are
  animated, a roll by default. Settings > Animations has a page each for
  Startup, Delete, and Fold. Among the effects, Burn burns the space up like
  a sheet of paper, with flames licking up around its frame. Settings >
  Animations picks the effect (roll, fade, pixelate, gradient, dissolve, fly,
  CRT, or burn) and its speed, and DeskSpace follows Windows' "Animation
  effects" switch.
- "Install updates automatically" in Settings > General: the installed version
  downloads a new release in the background and installs it after you have
  been away for 10 minutes, or when you choose "Restart to update" in the tray
  menu. Off by default. If an update cannot be installed, DeskSpace starts
  again on the current version and tells you why.
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

[Unreleased]: https://github.com/generalchad/DeskSpace/compare/v0.2.4...HEAD
[0.2.4]: https://github.com/generalchad/DeskSpace/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/generalchad/DeskSpace/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/generalchad/DeskSpace/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/generalchad/DeskSpace/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/generalchad/DeskSpace/releases/tag/v0.2.0
