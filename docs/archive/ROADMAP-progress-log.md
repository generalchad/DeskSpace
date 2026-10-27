# ROADMAP progress log (archive)

What landed, oldest first, moved here verbatim from `docs/ROADMAP.md`
Appendix A on 2026-10-13. This file is history: the details describe the state
when each change landed, and `docs/CONTEXT.md` is authoritative for current
behavior. The git log and the GitHub release notes record later changes;
the last section holds history moved out of the main docs.

Feature id provenance: the polled customization backlog (2026-10-03) is
F-24..F-35, the item-interaction session (2026-10-03) is F-36..F-39, and the
native-appearance follow-ups are F-40..F-43.

### 2026-10-02

- **Windows 11 look and control center.**
  - Per-space windows with acrylic/blur and rounded corners.
  - D2D/DirectWrite rendering, with hover and selection highlights.
  - A tray icon and a space context menu.
  - A data-driven settings window with Appearance, General, and About pages
    and live preview.
  - Delete space, and file drops onto spaces.
  - Config v2 with migration.
  - Item positions relative to their space, which fixed items being left
    behind when a space moved.

  This covered most of M1 and parts of M0, M2, and M3.
- **Resize and scroll (M4 phase 0).**
  - Spaces resize from any edge or corner: snapped, at least one cell, with
    resize cursors.
  - A resized space keeps its size, reflows its items into as many columns as
    fit, and scrolls with the mouse wheel, showing a thin indicator.
  - Only visible items go into the scene, so icons load lazily.
  - The icon cache is LRU-bounded. This closed F-03.
- **Spaces, item interaction, clear glass.**
  - Containers renamed to "spaces" throughout (D5).
  - Items open on double-click and show Explorer's own context menu, with
    "Move back to desktop" at the top.
  - Items drag out like Explorer items (`SHDoDragDrop`).
  - Dropped files move into the store (D3).
  - A corner-radius slider (0-24 px).
  - A Clear backdrop that can be fully invisible apart from the items, with
    label shadows to keep labels readable.

### 2026-10-03

- **Guardrails (M0).** CI, `cargo deny`, and binary errors with context for
  each step.
- **Debounced autosave.** `Ui::set_timer`/`clear_timer` deliver `Event::Timer`.
  Layout changes defer their write by about 700 ms.
- **Dark mode** for menus and the settings window.
- **Inline rename (F-02).**
  - Spaces are never activated, so a low-level keyboard hook turns key presses
    into an edit stream.
  - The app owns the edit buffer, and the painter draws the selection and
    caret.
  - IME composition is not supported.
- **Per-space appearance and menu submenus (F-24).**
  - An Appearance submenu with opacity presets, plus the system color picker
    for a tint or text color, and a reset.
  - `MenuEntry` gained submenus.
- **Sort a space (F-31).** Sort by name, type, size, or modified date, in
  either direction. `Item` gained `size`/`modified`, read when the item is
  created.
- **Grid density and label style (F-26, F-27).**
  - Icon spacing and edge padding sliders, which re-flow every space live.
  - Label lines, size, and shadow mode.
- **Window settings wired (M3).** `snap_grid` folded into `GridSpec.snap`, and
  `always_on_desktop` picks desktop-hosted placement at startup.
- **Rearranging items and the Delete key.**
  - Dragging an item within its space reorders it (`ReorderItem`).
  - The keyboard hook gained a watch on the selection keys.
- **Cross-space drag identity fix.**
  - Before: moving an item between spaces re-adopted a `(2)` copy instead of
    moving it.
  - Dropped files are now matched by canonical path, and
    `ItemStore::contains` compares canonical parents.
  - Verified by manual testing.
- **Explorer-parity drag (F-36).**
  - Spaces became OLE `IDropTarget`s, replacing `WM_DROPFILES`. They move by
    default and copy with Shift.
  - An empty drop description suppresses the shell's "Move"/"Copy" hint.
  - "Performed DropEffect" stops a shell source from moving the files a second
    time.
  - Drag-out sets "Preferred DropEffect" to move, so no `- Copy` is left
    behind.
- **Drag image (F-37).**
  - The item is rendered offscreen exactly as its space draws it, and made
    slightly translucent.
  - Inside the space, a layered ghost window shows it. Outside the space,
    `IDragSourceHelper2` shows it with no drop description.
  - Space drop targets forward to `IDropTargetHelper`, so the image stays
    visible over spaces.
- **Space click acts like a desktop click.** It focuses the desktop and clears
  the desktop icon selection through its `IShellView`.
- **Match Explorer's display.**
  - Check boxes, extensions, hidden/protected files, and shell overlays.
  - `SIGDN_NORMALDISPLAY` labels.
  - Multi-selection in the auto-check-select style.
- **Shortcut labels match Explorer.** No `.lnk`/`.url` extension, and both
  label forms are derived from the path.
- **Desktop round-trip fix.**
  - Before: desktop moves were unreliable, and items appeared with a label but
    no icon.
  - Every move is now announced with `SHChangeNotify`, and missing dropped
    paths are ignored.
  - A drag-out only removes the item once its file is gone.
- **Clipboard (F-38).**
  - Ctrl+C/X put `CF_HDROP` on the shell clipboard, and cut items are drawn
    ghosted.
  - Ctrl+V re-points items that are already in a space, and pastes anything
    else through `IFileOperation` (`FOF_RENAMEONCOLLISION | FOF_ALLOWUNDO`).
- **Recycle-Bin delete (F-39).**
  - Delete uses `IFileOperation` (undoable, no prompt).
  - "Remove from space" stays a separate action.
  - A clipboard listener clears the cut ghosting.
- **Item rename and clipboard polish.**
  - F2 renames the focused item through `IFileOperation::RenameItem`.
  - Cut ghosting clears correctly.
  - Clipboard reads retry while another process holds the clipboard.
- **Shell-operation and check-box polish.**
  - Paste ignores paths that do not exist, and missing items are pruned.
  - A `FileOperation` with nothing queued no longer fails.
  - Check boxes keep Explorer's hover and selection behavior.
- **Rubber-band selection, Ctrl+A, multi-item drag (F-42).** A custom
  `IDataObject` serves a multi-file `CF_HDROP`, and `active_items()` gathers
  the items an operation acts on.
- **Independent item check boxes (F-41).** A `CheckBoxMode` with a tick set
  that lasts only for the session, separate from the selection.
- **Compressed/encrypted label colors (F-43, part).**
- **Multi-selection drag fix.** Pressing an already-selected item no longer
  collapses the selection, and a group reorders as a block
  (`Command::ReorderItems`).

### After 2026-10-03

- **Show spaces from the desktop.**
  - While spaces are hidden, a per-user `DesktopBackground\Shell` verb runs
    the executable with `--show`. That posts `SHOW_SPACES` to the running
    instance.
  - On Windows 11 the verb sits under "Show more options" (see M8).
- **Realtime icon size (F-25, part).**
  - A 50-200 % slider with an editable percentage box. The default of 100 % is
    Windows' 48 px desktop icon.
  - `GridSpec::scaled` derives the effective grid, and commands use it through
    `Desktop::apply_with_layout`.
  - The Appearance tab was split into Appearance, Labels, Layout, and Colors.
- **Explorer icon size, item spacing, and info tips (F-40, F-43).**
  - The desktop view supplies its icon size and item spacing, which replace
    DeskSpace's sliders under "Match Explorer".
  - Native tracking tooltips show info tips.
- **Jumbo icons and per-monitor DPI (F-25, M1).**
  - `IShellItemImageFactory` renders icons at the effective size.
  - Overlays come from the system image list and are scaled to the Advanced
    page's overlay size.
  - Each space scales to its own monitor's DPI, and `WM_DPICHANGED` reflows
    it.

### 2026-10-10

- **Per-space looks (F-46).**
  - A first design listed only spaces with one of four overrides (opacity,
    tint, text, icon size) in a rail group beside the global pages. It read
    as more global pages and was replaced the same day.
  - Now the settings window has three panes: the scopes (Global, then every
    space indented, dimmed while it follows Global), the pages of the
    selected scope, and the page under a header with a "Customize this
    space" switch.
  - A customized space keeps its own copy of every look setting
    (`SpaceLook`), kept when the switch is turned off. The context menu's
    Appearance edits customize the space. The old four overrides are folded
    into a look at startup.
  - Selecting a space in the scopes pane (or opening the window on one)
    highlights it on the desktop for about 1.2 s, so it can be found.
- **Explorer-standard controls (`TASKS.md` G1-G4).**
  - A typed letter selects the next item whose label starts with it (a short
    timeout buffer, like typing on the desktop).
  - Home/End/PageUp/PageDown scroll the current space.
  - Ctrl+wheel changes the global icon size, as on the desktop, so spaces
    that follow Global resize together.
  - The arrow keys move the selection across the grid (Shift extends it, Ctrl
    moves a dotted focus rectangle on its own), scrolling the item into view.
  - Clicking an already-selected item's label a second time (a slow
    double-click) starts an inline rename, as Explorer does.
- **Settings performance.** A slider drag used to restyle and re-flow every
  space, re-apply DWM attributes, flush the menu themes, and repaint the
  rail on every tick. Unchanged styles, rects, scenes, menu modes, and rail
  selections are now skipped, and space previews wait for an idle message
  queue (`PREVIEW_TIMER`), so a drag collapses into one pass per pause.

### 2026-10-11

- **M2 desktop takeover (`TASKS.md` I1-I5).**
  - The desktop's loose items and their positions are read through the shell
    (`IFolderView::Item`/`GetItemPosition`), with the folder walk as a
    fallback, and cached in a `DesktopLayer`: absolute virtual-desktop
    positions, reconciled by path so ids survive with `Command::SyncDesktop`
    (I1-I2).
  - A chrome-less desktop-hosted layer window draws the loose items through
    the same item code as the spaces (I3).
  - Item hit-testing, hover, selection, rubber-band, drag, drop, info tips,
    and the scene build go through a `Surface` (`Space` or `Desktop`) instead
    of a space id, so one path serves spaces and the desktop layer (I4).
  - `desktop_icons::hide`/`restore` hide and show Explorer's `SysListView32`
    icon listviews, re-enumerating on each call; the icons re-hide after an
    Explorer restart (`TaskbarCreated`) and on a health timer, and are
    restored before `OleUninitialize` (I5).
  - The takeover is opt-in (`take_over_desktop`, and the hidden `--takeover`
    flag).
- **I7 closed as not needed (Win+D watcher).**
  - Show Desktop (`MinimizeAll`) skips `WS_EX_TOOLWINDOW` windows, so the
    top-level, never-activated spaces are already left visible across Win+D.
    Verified with the running app (top-level, `--on-top`, `--takeover`,
    `--takeover-layer`, and both hosted placements; a normal window was
    minimized in the same run) and with an isolated style matrix, on Windows 11
    build 26300. The planned watcher would have been a no-op, so it was
    dropped along with its config toggle.
- **M2 crash-safe restore (`TASKS.md` I6).**
  - Before the icons are hidden the app launches a dedicated watchdog
    (`deskspace-watchdog.exe`, a second binary of `deskspace-app`) detached,
    and the watchdog re-parents itself so killing the app by name or by process
    tree leaves it running. It waits on the main process and re-shows every
    listview unless a clean-exit event was signaled.
  - The runtime teardown signals that event right after it restores the icons,
    so the watchdog acts only on a crash; a named mutex keeps a stray watchdog
    a no-op.
- **A space's Delete button in the settings window (F-02).** A space's scope
  header now carries a "Delete space" button beside the "Customize this space"
  switch. It runs the same delete (with its confirmation for a non-empty
  space) as the context menu, then the window falls back to Global; deletion is
  immediate, so Cancel does not bring the space back. It is tinted red to mark
  it as destructive.
- **Desktop folder watch (`TASKS.md` I8).** A reusable `FolderWatch`
  (`ReadDirectoryChangesW` on a worker thread, debounced to 150 ms) re-syncs
  the desktop layer when files change outside DeskSpace.
- **Item preset (F-48, C5).** "Match Explorer's display" became the
  Appearance page's "Item preset" dropdown (`Match Explorer's display` /
  `Custom`); the Layout page greys the fields Explorer supplies.
- **Hotkeys and New spaces pages (F-17, F-18; C7, C8).** Three editable
  global hotkeys (show/hide, new space, open settings), table-driven by
  `HOTKEY_BINDINGS`, and the defaults a new space is created with.
- **App split (`TASKS.md` R1, R2).** `app.rs` (about 4,500 lines) became
  `app/` submodules with one `impl App` each, and `Selection`, `EditState`,
  and `PointerState` were pulled out of `App`. Pure moves.
- **Space polish (F8-F12).** Shift and Caps Lock decode predictably in
  rename; a horizontal resize fits whole columns; "Fit to contents" joined a
  space's Layout page; a border press waits out a dead zone before resizing;
  checked item boxes use the Windows accent with a white tick.
- **Embedded icon (`TASKS.md` E1, E2).** `assets/deskspace.ico` and version
  info are compiled into the executable; the tray, the settings window, and
  the desktop verb use it, and the icon drawn in code is gone.
- **Show Desktop hardening.** Spaces ignore `WM_SYSCOMMAND`/`SC_MINIMIZE`, so
  survival across Win+D no longer depends on the tool-window exclusion.
- **Placement setting removed.** The General page's "Keep spaces on the
  desktop" (`always_on_desktop`) is gone; desktop-hosted placement is only
  the `--placement` CLI option. Old config files still load (the key is
  ignored).
- **Docs review.** `AGENTS.md` gained "Design priorities"
  (readability, maintainability, extensibility); `CONTEXT.md` gained module,
  message, timer, and id tables; `TASKS.md` gained a file map, extension
  recipes, and section K (file splits).
- **Decisions D9.** Takeover stays opt-in for v0.2; Ctrl+wheel under Match
  Explorer drives the desktop's icon size; the toolchain is pinned; right
  drag follows Explorer's drop menu; hiding spaces leaves the desktop layer.
  Work items F13, E13, and G5a-G5c.

### 2026-10-12

- **Installer framework switch (D6 amended).** The Windows installer moves
  from Velopack to `cargo-packager` 0.11.8, a CI-only tool rather than a
  workspace dependency.
  - Velopack's Rust SDK pulled a TLS and advisory stack (`ring`, `rustls`,
    `derivative`) past the tightened `deny.toml` and needed an app-side startup
    hook; `cargo-packager` is CI-only, so the dependency graph and `deny.toml`
    are unchanged. Trade-off: no delta updates.
  - A per-user NSIS installer and uninstaller are built from a vendored custom
    template at `packaging/nsis/installer.nsi`, configured under
    `[package.metadata.packager]` in `crates/deskspace-app/Cargo.toml`. The
    artifact is `deskspace_<version>_x64-setup.exe`, published to the same
    GitHub Release as the portable zip.
  - The template installs into `%LocalAppData%\DeskSpace.App`, keeping the
    install root apart from the user's stored files under
    `%AppData%\DeskSpace\DeskSpace\config` and
    `%LocalAppData%\DeskSpace\DeskSpace\data`.
- **A modern color picker (F-51; `TASKS.md` F14).** The Colors page and the
  space menu's Appearance submenu open DeskSpace's own themed popup
  (`win32/color_picker.rs`) instead of the dated system `ChooseColor` dialog.
  It opens on a palette (a "Recent colors" row and a "Windows colors" grid)
  after the Windows color dialog, with the saturation/value square, hue and
  alpha strips behind a "Custom colors…" toggle, plus a preview, a hex readout,
  and OK/Cancel. `deskspace-core` gained `Hsv`, `Color::to_hsv`, and
  `Hsv::to_color`. The popup paints opaque GDI and does not extend the DWM
  frame into the client, which had made it transparent; it uses the system
  message font, and its swatches and the settings window's color previews are
  slightly rounded boxes. A later polish pass made the color field's swatch
  itself the button (with an eyedropper glyph), moved the picker to a
  flicker-free DIB back buffer with per-pixel anti-aliased markers and an
  anti-aliased glyph check mark, and fixed a modal-loop stall. On-screen check
  pending.

### 2026-10-13

- **In-app updates (F-45; `TASKS.md` E8-E11).** A new `deskspace-update` crate
  wraps `cargo-packager-updater`: it checks a minisign-signed `latest.json`
  published as a release asset and installs the full NSIS package (no deltas).
  The app exposes an opt-in "Check for updates automatically" toggle and a
  "Check for updates now" button on the General page, and a "Check for
  updates…" tray entry; a portable build only opens the release page. The
  platform gained `Ui::open_url` and a cross-thread `Waker` (`Event::Wake`).
  `release.yml` signs the installer and builds the feed when the
  `DESKSPACE_MINISIGN_KEY` secret is set. The updater's `time` dependency moved
  the minimum Rust version to 1.88.
- **No console window in release.** Both binaries are built with the Windows
  subsystem in release (`#![windows_subsystem = "windows"]`), so an installed
  or launched copy no longer flashes a console; debug builds keep it for logs.
- **First public release (v0.2.0).** The repository was made public and
  `v0.2.0` published as a normal (non-pre-release) release, so it appears as
  the repository's Latest release. The release workflow's Publish step is now
  idempotent (it replaces an existing release for the tag).

## Moved from the main docs

### 2026-10-16: ROADMAP.md milestone history

Moved verbatim from `docs/ROADMAP.md` section 5, which keeps a summary.
M0 and M1 in full:

#### M0 - Foundations and guardrails  **[done]**

Goal: a codebase that stays healthy as it grows.

- **[done]** CI on Windows runs `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -D warnings`, and
  `cargo test --workspace`. It also runs `cargo deny check all` (licenses,
  advisories, bans, sources) from `deny.toml`.
- **[done]** Events are honest. `Event`/`SpaceEvent` declare only what is
  emitted, and `WM_MOUSELEAVE` drives hover. Timers (`Ui::set_timer`) are
  added only when a feature needs one.
- **[done]** Work-area awareness. Commands are bounded by the work area of the
  monitor nearest the action (`work_area_at`).
- **[done]** Error boundaries. Library crates return `thiserror` errors, and
  the binary uses `anyhow` with context for each step.
- **[done]** Debounced autosave. Layout changes save about 700 ms after the
  last change. File moves and Apply/OK save at once, and closing flushes any
  pending write.

Exit criteria (met): CI is green, no event variant goes unused, and the work
area drives bounds.

#### M1 - Direct2D renderer and Windows 11 look  **[done]**

Goal: spaces that look native on Windows 10 and 11.

- **[done]** `render/d2d.rs` implements the `SpaceScene` contract. It draws
  with per-pixel alpha and DirectWrite text, and handles device loss
  (`D2DERR_RECREATE_TARGET`).
- **[done]** Acrylic/blur behind space bodies on Windows 10 1803+ and 11
  (see D2), with DWM rounded corners. Verified on Windows 11 build 26300.
- **[done]** Per-monitor DPI.
  - Each space reads its own monitor's DPI (`GetDpiForWindow`) and scales its
    layout, icons, and hit tests.
  - `WM_DPICHANGED` reflows a space that moves to a display with a different
    DPI.
  - Not yet checked on a mixed-DPI setup.
- **[dropped]** GDI fallback painter. The D2D path has no known failure that
  GDI would cover, and GDI cannot draw over the blur correctly.

Exit criteria (met): spaces render through D2D with a Windows 11 look, and
blur works on Windows 10 and 11.

#### M2's "Done" list

Done:
- **[done]** Space chrome:
  - create, and delete with a confirmation (F-02);
  - resize from edges and corners, scroll, and "Fit to contents" (F-03);
  - lock, shown by a glyph (F-05);
  - rename inline from the space menu (F-02);
  - the space context menu (F-14, basic).
- **[done]** The managed store (D3). Drops move files into the store, and
  Shift keeps them in place. Every move is announced to the shell with
  `SHChangeNotify`. Dropped files are matched by canonical path, so a move
  between spaces never creates a duplicate.
- **[done]** Items inside spaces behave like Explorer items (F-06,
  F-36..F-43):
  - Items open on double-click and show Explorer's context menu.
  - Selection:
    - plain click, Ctrl+click, and Shift+click;
    - rubber-band selection and Ctrl+A;
    - both of Explorer's check-box modes.
  - Drag and drop:
    - dragging an item within its space rearranges it, and a multi-selection
      moves as a block;
    - OLE drag-out carries the whole selection and defaults to a move, with a
      drag image;
    - spaces are OLE drop targets with move/copy feedback.
  - Editing:
    - Ctrl+X/C/V through the shell clipboard;
    - Delete sends items to the Recycle Bin;
    - F2 renames an item inline.
  - Display matches Explorer's settings:
    - extensions, hidden files, and compressed/encrypted label colors;
    - shell overlays, icon size, and item spacing;
    - info tips.
- **[done]** Shell-sourced icons. `IShellItemImageFactory` renders icons
  at the effective pixel size, with overlays from the system image list.
  `SHGetFileInfoW` remains a fallback.
- **[done]** Desktop takeover (D7/D8, `TASKS.md` I1-I8), opt-in:
  - the desktop's items and positions are read through the shell's
    `IFolderView` into a `DesktopLayer` (I1-I2);
  - a chrome-less layer window draws the loose items with the space item code,
    and item interaction goes through a `Surface` (`Space` or `Desktop`), so
    dragging works within the desktop and into a space (I3-I4);
  - Explorer's icon listviews are hidden, restored on a clean exit, re-hidden
    after an Explorer restart and on a health timer (I5), and restored by a
    watchdog helper on a crash (I6);
  - spaces survive Show Desktop without a watcher (I7 closed; spaces also
    ignore `SC_MINIMIZE`);
  - the Desktop folder is watched while the layer is drawn, so loose items
    follow outside changes (I8).
- **[done]** Non-file shell drags (F-50; `TASKS.md` L2). A space accepts a
  drag that carries only shell items, including a packaged app with no
  filesystem path, and writes a shortcut into the space storage, as Explorer
  does on a folder. A drop decides by the effects the source allows (`LINK`
  makes shortcuts, as does Alt), so a Start, taskbar, or Alt drag never moves
  or lists the source's own `.lnk`, and a file list of AppUserModelIDs, which
  the Start menu sends, counts as shell items. Awaits the on-screen check and
  the rest of L1's per-source findings.
- **[partial]** Right-button drag with Explorer's drop menu (D9): G5a and G5b
  are done (check pending).

#### M3's "Done" list

Done:
- **[done]** Tray icon (F-16): Settings, Hide/Show spaces, New space, and Exit.
  While spaces are hidden, a desktop right-click verb, "Show DeskSpace spaces",
  brings them back.
- **[done]** Space context menu (F-14):
  - create, delete, lock, roll up, and fit to contents;
  - an Appearance submenu: tint opacity, icon size, and custom tint and text
    colors, each of which customizes the space (F-24), plus "Use global
    appearance" and "Space settings…";
  - a one-off Sort by (F-31).
- **[done]** Settings window (F-17). A data-driven form engine with live
  preview, OK/Cancel/Apply, and the pages Appearance, Labels, Layout, Colors,
  Advanced, General, New spaces, Hotkeys, and About. It covers the material,
  corners, colors, label style (F-27), grid density (F-26), icon size (F-25),
  overlay size, the new-space defaults, and the global hotkeys.
- **[done]** Per-space looks (F-46). The settings window has three panes:
  the scopes (Global, then every space indented, dimmed while it follows
  Global), the pages of the selected scope, and the page. A space shows the
  five look pages, greyed with the global values until "Customize this
  space" is on; it then keeps its own copy of every look setting
  (`SpaceLook`), kept while customization is off. Edits preview live through
  `SetSpaceLook` / `CustomizeSpace`, and Cancel restores every space.
  Selecting a space highlights it on the desktop.
- **[done]** Every `WindowSettings` field is wired to behavior:
  - `start_hidden`;
  - the global hotkeys, editable as text on the Hotkeys page with validation
    and conflict reporting: show/hide, new space, and open settings (F-18);
  - snapping, now `GridSpec.snap`.
- **[done]** Dark mode for the settings window and menus, following Windows.
- **[done]** A zoom pop-up (F-57): every icon-size change shows the size as
  a percentage at the top center of the screen, with zoom out, zoom in, and
  Reset buttons, in the spaces' look.
- **[done]** A modern color picker (F-51): the settings Colors page and the
  space menu's Appearance submenu open DeskSpace's own themed popup (a
  "Recent colors" row and a "Windows colors" grid, with the saturation/value
  square, hue and alpha strips behind a "Custom colors…" toggle, plus a
  preview, hex readout, and OK/Cancel) instead of the system `ChooseColor`
  dialog (`TASKS.md` F14; check pending).

- **[done]** Ways to add items without a drag (`TASKS.md` section L), each
  ending in the F-50 shortcut path (`App::add_shortcuts`): "New shortcut ▸"
  ("From installed apps…" and "Custom shortcut…", F-53), a space's Settings
  page "Shortcuts" (an X per item and a + tile, F-53), "Paste shortcut"
  (F-54), and "Add files…" (F-55). CONTEXT section 8 describes them.

### 2026-10-16: on-screen confirmations recorded in CONTEXT.md

Moved verbatim from `docs/CONTEXT.md` section 2 ("Verification").
Later confirmations are recorded where their `TASKS.md` section V row is
deleted (the commit says so).

Checked on screen (Windows 11 build 26300): the blur, corners, space
content, and settings layout; inline rename, in-space rearranging, the
Delete key, per-space appearance, sort, grid and label settings, and
cross-space moves (2026-10-03); a multi-item drag out (2026-10-10); the
desktop layer's item interaction (`--takeover-layer`) and the takeover
(`--takeover`: icons hide, return on exit, re-hide after an Explorer
restart, and come back after a kill through the watchdog; 2026-10-11);
inline-rename Shift/Caps, the whole-column and whole-row resize fit, the
resize dead zone, "Fit to contents" in the settings window, and the
accent-colored check boxes (2026-10-12); F-53's New shortcut (2026-10-13).

### 2026-10-16: partial passes recorded in TASKS.md section V

Moved from the section V rows, which now list only what is left to check.

- F9: the on state (resized spaces fit to whole columns and rows) passed 2026-10-12.
- H5, H4 (F-46): the highlight outline is visible on Acrylic.
- Mouse modifiers: Ctrl+wheel reaching a space with Ctrl set was confirmed from a log on 2026-10-16.
- F13 (F-25): the default preset with takeover on (the layer's icons resize and re-space with the spaces, both ways) passed 2026-10-16.
- F-57 zoom pop-up: passed 2026-10-16 at 100 % in dark mode on Acrylic: Ctrl+wheel, the buttons, Ctrl+Plus/Minus/0, the hide timer and hover hold, and no focus steal.
- F15: passed 2026-10-16: zooming keeps a never-resized space's size, re-flows its icons, and leaves its title bar and title text alone.

### 2026-10-21: the desktop layer is never topmost

- `fix: never make the desktop layer topmost` passed on screen: with
  takeover on and `./scripts/dev.ps1 -OnTop`, the layer spanning both
  monitors stayed behind other windows, the spaces sat on top, and other
  apps took clicks (maintainer; the window styles of the running instance,
  read with `GetWindowLongW`, showed only the spaces as `WS_EX_TOPMOST`).

### 2026-10-27: drawn shadows, the structure (O2)

`scripts/onscreen/shadows.ps1` passed here on the composition path (Windows
11, "Transparency effects" on): Acrylic, Blur, Solid, and Clear at radii 0,
4, 8, and 16, with exactly one visible `DeskSpaceShadow` per space that
should cast one; each directly below its space; each with `ShadowLook::rect`
at the space's DPI, including a space with its own `shadow_size` of 40
beside the global 16; each layered, click-through, never activated, and a
tool window; and the bands beside and below each space reach the backdrop.
The desktop layer cast none, and `show_shadow = false` cast none. Not
covered: the desktop-hosted placement (this session had no desktop host),
the pixels (O3), animations, the Settings preview, the debug toggles,
Windows 10, and 150 %.

### 2026-10-27: drawn shadows, the pixels (O3)

`scripts/onscreen/shadows.ps1 -Pixels` passed here at 125 % on the
composition path. Over a white backdrop, against two 300 px Solid spaces
per run: the alpha beside a space is half the `shadow_opacity` setting and
below it three quarters (within 0.09 and 0.10), nothing outside the space
is darker than the setting, both falloff lines only fall and reach 0 at the
shadow's edge, a 16 px corner's diagonal reads 0.008 where a square one
reads 0.035, opacity 0 hides the shadow window and leaves the backdrop
white, and a Solid space's centre is its tint. The harness's backdrop is
now topmost and is created before the spaces (Start-DeskSpace
-BackdropColor), so another application cannot show through where a shadow
is measured; its window procedure moved into C# so a second run in the same
PowerShell session does not re-register the class with a collected
delegate. Not covered: 100 % and 150 %, Acrylic and Blur as a backdrop,
Windows 10, transparency off, the animations, the Settings preview, the
debug toggles, and the desktop-hosted placement.

The run also showed that `shadow_opacity` is the black's own alpha, not the
darkest pixel: the blur leaves the peak at about three quarters of it below
the space and half beside it. The maintainer chose on 2026-10-27 to keep
that look; the theme field's comment and D15 now say so.
