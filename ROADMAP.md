# ROADMAP

Plan for bringing DeskSpace to feature parity with comparable tools.
See `CONTEXT.md` for the current architecture and `AGENTS.md` for engineering
rules. Milestones are ordered by dependency; feature ids (`F-xx`) are stable
references used across milestones and the parity checklist.

## 1. Vision

A native, lightweight, offline Windows desktop organizer that:
- owns the desktop icon layer and draws real desktop items itself,
- groups items into movable, resizable, lockable spaces,
- mirrors folders as live lenses,
- hides and reveals everything quickly (roll-up, auto-hide, peek),
- looks and feels modern (per-pixel alpha, blur, animation),
- and stays fully configurable from a tray app, context menus, and a settings
  window.

## 2. Decisions locked

- **D1. Desktop takeover.** DeskSpace hides Explorer's `SHELLDLL_DefView` icon
  listview and renders desktop items on a desktop-hosted window. Items are real
  filesystem entries; moving an item into a space relocates the real file.
- **D2. Rendering backend (revised).** Each space is its own window. Content
  is drawn with Direct2D/DirectWrite into a premultiplied-alpha HWND target
  over a DWM-extended frame; the blur material comes from the accent policy
  (`SetWindowCompositionAttribute`) and corners/border/dark mode from DWM
  attributes. GDI is retired. DirectComposition is deferred to the animation
  work in M5. The documented `DWMWA_SYSTEMBACKDROP_TYPE` was rejected because
  system backdrops only render on active windows and spaces are never active.
- **D3. Item storage model.** Two mechanisms, kept strictly separate:
  - *Lenses* (what comparable tools calls "Folder Portals") are views over a
    real folder. They read contents live and never move, copy, or duplicate
    data on their own, regardless of folder size. Dropping files onto a lens
    is a user-initiated move into the bound folder, performed with the
    shell's `IFileOperation` so it shows the standard conflict dialogs and
    can be undone from Explorer.
  - *Normal spaces* hold individual items. Dropping an item into a normal space
    relocates that one item into a managed store with a move, not a copy (an
    atomic rename on the same volume, so no duplication and no extra disk use).
    The store is one folder shared by all spaces,
    `%LOCALAPPDATA%\DeskSpace\storage` (beside the config file for an explicit
    `--config`), so moving an item between spaces never touches the file.
    Holding Shift while dropping keeps the file in place, as does a file on
    another drive (moving it would copy). Name clashes get " (2)" like
    Explorer; nothing is overwritten. Removing an item or deleting its space
    moves stored files back to the desktop. Still planned: undo, a
    configurable root, and a snapshot manifest.
- **D4. v0.2 scope.** Tray icon, right-click context menus, and a settings
  window are required in the first usable release.
- **D5. Naming.** Containers are *spaces* (spaces calls them spaces) in the
  UI, docs, and code (`Space`, `SpaceId`); config files that still use the
  old `spaces` key load unchanged. The folder-mirror feature is called a
  *lens* in the UI and docs. In code it is a space whose source is a folder
  (`SpaceSource::Folder`), so the user-facing name stays out of the model.

## 3. Guiding principles

- Keep the domain pure: all new behavior is expressed as `Command` variants in
  `deskspace-core`; platform code never mutates the model directly.
- Keep rendering and UI data-driven: the app builds a `SpaceScene` /
  `SpaceStyle` per space, `MenuEntry` lists, and `SettingsForm`s; the platform
  only turns them into windows, controls, and pixels.
- Make destructive operations recoverable: file moves and layout changes get an
  undo path and a snapshot/restore mechanism.
- Degrade gracefully: if takeover or blur fails, fall back to plain top-level
  space windows with a translucent tint rather than crashing.
- Ship small, verified increments: every milestone lands with tests, clippy
  clean, and a runnable binary.

## 4. Milestones

Progress is marked inline as **[done]**, **[partial]**, or unmarked (not
started).

### Progress log

- **M0 guardrails.** GitHub Actions runs `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -D warnings`, and
  `cargo test --workspace` on Windows, plus `cargo deny check all` (licenses,
  advisories, bans, sources) from `deny.toml`.
- **Debounced autosave and a timer primitive.** `Ui::set_timer`/`clear_timer`
  deliver `Event::Timer`; layout changes defer their configuration write by
  about 700 ms, while file moves and settings save at once and closing flushes.
- **Inline rename (F-02).** The space menu renames a title in place. Spaces are
  never activated, so a low-level keyboard hook translates keys into an edit
  stream; the app owns the edit buffer and the painter draws the selection and
  caret. Enter commits, Escape cancels, clicking away commits. IME composition
  and clipboard are not supported yet.
- **Per-space appearance and menu submenus (F-24).** The space menu's
  Appearance submenu sets an opacity preset and opens the system color picker
  for a tint or text override, with a reset; `Space::tint` and `Space::text`
  are optional overrides applied by the scene builder. `MenuEntry` gained
  submenus (shared by the space and item menus).
- **Sort a space (F-31).** The space menu's Sort by submenu reorders items by
  name (case-insensitive), type, size, or modified date, ascending or
  descending, then re-lays the grid. `Item` gains `size`/`modified` read at
  creation; a kept rule and lens sorting (F-08) remain open.
- **Window settings wired (M3).** `snap_grid` was folded into `GridSpec.snap`
  with a General slider (0 disables snapping); `always_on_desktop` picks
  desktop-hosted placement at startup when no `--placement` is given; a
  General note explains it applies next launch.
- **Grid density and label style (F-26, F-27).** Appearance gains icon
  spacing and edge padding sliders (they re-flow all spaces live) and label
  controls: one or two lines, size 60-160 %, and shadow automatic/always/
  never. Label formats are cached per size step.
- **Rearranging items and the Delete key.** Dragging an item within its space
  reorders it (`ReorderItem`) so the arrangement sticks; dragging past the
  space edge still starts a shell drag. The keyboard hook gained a Delete-only
  watch while an item is selected, delivering `Event::DeleteSelection` to
  remove it (stored files return to the desktop).
- **Cross-space drag identity fix.** Dropped files are matched by canonical
  path (case- and 8.3-tolerant) and `ItemStore::contains` compares canonical
  parents, so moving an item between spaces moves it instead of re-adopting a
  `(2)` copy; an item whose file vanished is pruned before a drag. Verified by
  manual testing 2026-10-03 (cross-space moves stay unique and persist).
  Explorer parity (move/copy effects, ghost image, clipboard, Recycle-Bin
  delete) is queued as F-36..F-39. **Item interaction through F-39 is complete;
  start the next item session on the native-appearance follow-ups below.**
- **Explorer-parity drag, part 1 (F-36).** Spaces are OLE `IDropTarget`s
  (`win32/drop_target.rs`, `RegisterDragDrop`, revoked in `WM_DESTROY`) rather
  than `WS_EX_ACCEPTFILES` windows, so the shell shows the move/copy cursor and
  `DragOver` picks the effect: move by default, copy while Shift is held (the
  app's "Shift preserves the original" convention). A drag that carries no
  `CF_HDROP` is rejected, and `Drop` marks the source's data object with the
  "Performed DropEffect" format so a shell source does not move the files a
  second time. Dragging an item out sets the "Preferred DropEffect" to move, so
  dropping it on the desktop no longer leaves a `- Copy`. `WM_DROPFILES` is
  gone. Smoke-tested only; confirm both behaviors on screen.
- **Drag image (F-37).** The picture under the pointer during an item drag
  was the shell's opaque, oversized default. Now the item is rendered
  offscreen exactly as its space draws it (32 px icon, label, selection
  highlight) and made slightly translucent, like Explorer's picked-up item,
  into a `DragImage` whose hotspot keeps the grabbed spot; the same pixels
  are used everywhere. Inside the space a layered click-through
  ghost window shows it and the slot fades (`ItemState::Dragged`); outside,
  `IDragSourceHelper` gives the same image to the shell, and space drop
  targets forward to `IDropTargetHelper` so it stays visible over spaces.
  The offscreen render was probed; on-screen checks pending.
  The target-cell highlight in the original F-37 note was dropped: the item
  already moves into its target cell live.
- **Clipboard (F-38).** Ctrl+C and Ctrl+X put the selected items on the shell
  clipboard as `CF_HDROP` (plus "Preferred DropEffect" `MOVE` for a cut), so
  they paste into Explorer and other apps, and cut items are drawn ghosted.
  Ctrl+V pastes into the current space: a cut re-points items already in a
  space, while external files and copies go through `IFileOperation` with
  `FOF_RENAMEONCOLLISION` (protecting the shared store against Explorer's
  Replace dialog) and `FOF_ALLOWUNDO`; a progress sink reports each created
  path so the pasted items are listed and selected. The keys arrive through the
  same low-level hook as Delete, gated on a selection (Ctrl+C/X) or a current
  space (Ctrl+V). Unit-tested only; on-screen checks pending.
- **Recycle-Bin delete (F-39).** The Delete key now sends the selected files to
  the Recycle Bin with `IFileOperation` (`FOF_ALLOWUNDO | FOF_NOCONFIRMATION`,
  no prompt, undoable) and drops any item whose file is gone; the item menu's
  "Remove from space"/"Move back to desktop" stays a distinct action that
  returns a stored file to the desktop. A clipboard-change listener
  (`AddClipboardFormatListener`) also clears cut ghosting when another app or
  the desktop replaces the clipboard. Smoke-tested only; confirm on screen.
- **Item rename and clipboard polish.** F2 renames the focused item inline
  (`IFileOperation::RenameItem`, undoable, extension preserved when the label
  hides it) and updates its path and label through `Command::RenameItem`; the
  painter draws the edit buffer, selection, and caret on the item's label. Cut
  ghosting now clears correctly when another item is cut (every affected space
  repaints) and when another app replaces the clipboard, and a cut pasted into
  Explorer or the desktop drops the now-missing item (`Event::ClipboardChanged`
  prunes). Clipboard reads retry a held clipboard, so a transient failure no
  longer clears the ghost.
- **Shell-operation and check-box polish.** Paste no longer adds a path the
  shell reported but that does not exist, and missing items are pruned at
  startup and by Delete, so a label with no icon cannot linger; a `FileOperation`
  with nothing queued no longer fails (which had aborted the delete). Item
  check boxes keep Explorer's behavior: revealed on hover, left ticked while
  the item stays selected, and cleared only by clicking empty space or the
  desktop — clicking another app keeps the selection, and a failed view-flag
  read leaves the last known value.
- **Rubber-band selection, Ctrl+A, and multi-item drag (F-42).** Dragging over
  empty content draws a marquee and selects the items it touches (Ctrl/Shift
  adds to the selection); Ctrl+A selects every item in the current space.
  Dragging a selected item out drags the whole selection: a custom
  `IDataObject` (`win32/data_object.rs`) serves a multi-file `CF_HDROP` and
  passes the shell's drag image through, since the shell's own object covers
  only one item and `SHCreateDataObject` needs a shared parent. `active_items()`
  centralizes the operand (delete/copy/cut/drag) so F-41 can swap in the
  independent check-box set. Offscreen-tested only; confirm on screen.
- **Independent item check boxes (F-41).** `ShellDisplay.check_boxes` is now a
  `CheckBoxMode` (`Hidden` / `Selection` / `Independent`). In Independent mode
  the boxes are always shown and the tick is a session-only set separate from
  the selection; clicking a box toggles the tick and moves the highlight, the
  ticks survive empty-space and desktop clicks, and `active_items()` makes the
  ticked items the operand for delete/copy/cut/drag. The set clears when the
  mode changes and is pruned as items leave.
- **Space click acts like a desktop click.** Clicking a space now focuses the
  desktop (deactivating the app in front) and clears the desktop icon
  selection through its `IShellView`, matching native Windows. A selection
  inside an Explorer window is left alone: native only changes a selection in
  the view that was clicked. Needs on-screen confirmation.
- **Match Explorer's display (native-display core).** Spaces now read the
  desktop's display options and mirror them when `Theme.match_explorer` is on
  (default, with an Appearance toggle): item check boxes
  (`FWF_CHECKSELECT`/`FWF_AUTOCHECKSELECT`), filename extensions
  (`SSF_SHOWEXTENSIONS`), and hidden/protected files
  (`SSF_SHOWALLOBJECTS`/`SSF_SHOWSUPERHIDDEN`). Icons get shell overlays
  (`SHGFI_ADDOVERLAYS | SHGFI_LINKOVERLAY`, so shortcut arrows and sync badges
  appear) and labels use `SIGDN_NORMALDISPLAY`. The desktop uses Explorer's
  auto-check-select mode, so check boxes mirror the selection: a plain click
  ticks one item and unticks the rest, Ctrl+click ticks several, Shift+click
  ranges, and clicking the box toggles one item. Unlike the independent
  "item check boxes" mode, check state is not stored separately.
  Items have Explorer-style multi-selection (Delete removes all selected).
  Interaction is done; icon size/view mode, spacing, info tips, and
  compressed/encrypted colors remain (F-25, F-28). Verified the desktop's
  flags read correctly and a synthetic click toggled a real item on the dev
  machine; on-screen checks pending.
- **Shortcut labels match Explorer.** With Explorer's "File name extensions"
  option on, spaces no longer append `.lnk`/`.url` to shortcuts; only real
  files gain an extension, and both label forms are derived from the path so
  toggling the option (or a label captured while it was on) never leaves a
  stale extension.
- **Desktop round-trip fix.** Manual testing found desktop moves unreliable
  and a desktop item dropped on a space that already held it appearing as a
  label with no icon. Causes: DeskSpace moved files with a plain rename and
  never told the shell, so the desktop kept stale icons whose files were
  already in storage (dragging one dropped a missing path, which was then
  listed in place); and after a drag ended over the desktop the app moved the
  stored file out itself even when the desktop had copied it. Now every move
  is announced with `SHChangeNotify`, missing dropped paths are ignored, and
  a drag-out only removes the item when its file is gone. Needs on-screen
  confirmation.
- **Windows 11 look and control center (current).** Per-space windows with
  acrylic/blur, rounded corners, D2D/DirectWrite rendering, hover/selection;
  tray icon; space context menu; data-driven settings window with Appearance,
  General, and About pages and live preview; delete space; file drops onto
  spaces (as references); config v2 with migration; space-relative item
  positions (fixes items being left behind when a space moved). Covers most of
  M1 and parts of M0, M2, and M3.
- **Resize and scroll (M4 phase 0).** Spaces resize from any edge or corner
  (snapped, minimum one cell, resize cursors); a resized space keeps its size,
  reflows items into as many columns as fit, and scrolls with the mouse wheel
  with a thin scroll indicator. Only visible items are put in the scene, so
  icons load lazily. Icons are decoded into owned pixel buffers and the icon
  cache is LRU-bounded. Closes F-03.
- **Spaces, item interaction, clear glass.** "spaces" renamed to "spaces"
  throughout (D5). Items open on double-click, show Explorer's own context
  menu (with "Move back to desktop" on top), and drag out like Explorer items
  (OLE `SHDoDragDrop`): into folders and apps, onto other spaces, or onto the
  desktop to leave the space. Dropped files move into the store (D3), Shift
  keeps them in place. Appearance gains a corner-radius slider (0-24 px with
  labeled presets; non-DWM radii drawn by the painter) and a Clear backdrop
  that can be made fully invisible apart from the items, with label shadows
  for legibility.

### M0 - Foundations and guardrails

- **[done]** CI: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`,
  `cargo test --workspace`, on Windows.
- **[done]** `deny.toml` for licenses/advisories; `cargo-deny` in CI.
- **[done]** Wire the declared-but-unused events: the old `WindowEvent` enum
  was replaced by `Event`/`SpaceEvent`, which only declares what is emitted;
  mouse tracking (`WM_MOUSELEAVE`) drives hover. Timers are added when a
  feature needs one (debounced autosave, M5 policies).
- **[done]** Work-area awareness: commands are bounded by the work area of
  the nearest monitor (`work_area_at`).
- **[done]** Structured error boundaries: library crates return `thiserror`
  errors; the binary boundary uses `anyhow` with per-step context (per
  `AGENTS.md`).
- **[done]** Autosave on change (debounced), not just on close.

Exit criteria: green CI, no unused event variants, work area drives bounds.

### M1 - Direct2D renderer and Windows 11 look

- **[done]** `render/d2d.rs` implementing the `SpaceScene` contract.
- **[done]** Per-pixel alpha; DWM rounded corners; text via DirectWrite.
- **[done]** Acrylic/blur behind space bodies on Windows 10 1803+ and 11
  (accent policy; see D2), verified on Windows 11 build 26300.
- **[done]** Device-lost handling (`D2DERR_RECREATE_TARGET`).
- Per-monitor DPI scaling of space layout and scenes.
- **[dropped]** GDI fallback painter: the D2D path has no known failure mode
  that GDI would cover, and GDI cannot draw over the blur material correctly.

Exit criteria: spaces render through D2D with a Windows 11 look and blur works
on Windows 10/11. Remaining: per-monitor DPI.

### M2 - Desktop takeover core

- Discover and hide the desktop icon listview (`SHELLDLL_DefView` /
  `SysListView32`) without breaking Explorer; restore on exit.
- Enumerate real desktop items (icons, labels, positions) via `IShellFolder`
  and `IShellItemImageFactory` (jumbo icons), replacing `SHGetFileInfoW`.
- Render desktop items in a free-grid; hit-test, hover, select, rubber-band.
- Drag items: within the desktop, into a space, and between spaces (`F-06`).
  **[partial]** Drops onto spaces (an OLE `IDropTarget` with move/copy
  feedback), shell drag-out (`SHDoDragDrop`, defaulting to move), and
  rearranging items within a space (`ReorderItem`, with a drag image) work;
  clipboard and Recycle-Bin delete (F-38, F-39) and the desktop free-grid wait
  for later work.
- **[partial]** Drop files onto a normal space relocates them into the managed
  store by move (`D3`); lenses are unaffected because they never touch the
  store. Undo is open.
- Update the item model and refresh after file operations.
- Space chrome: resize handles, lock toggle, rename inline, delete with undo
  (`F-02`, `F-03`, `F-05`). **[partial]** Lock (menu + glyph), delete (menu
  + confirmation, no undo), resize (edges/corners, "Fit to contents"), and
  inline rename (space menu; Enter commits, Escape cancels) are done.
- **[done]** Basic right-click context menu for space actions (`F-14`
  partial). Items get Explorer's context menu plus "Move back to desktop".
- Follow desktop item changes with a filesystem watcher and re-sync.

Exit criteria: with DeskSpace running, the Explorer icon layer is gone, items
are drawn and interactive, and dragging moves real files with undo.

### M3 - Control center: tray, context menu, settings

- System tray icon with menu: show/hide, pause takeover, open settings, snapshots
  (`F-16`). **[partial]** Settings, show/hide, new space, exit; takeover and
  snapshots do not exist yet.
- Full context menu system: create space/lens, arrange, sort, lock, roll up,
  appearance, remove (`F-14`). **[partial]** Create, delete, lock, roll up,
  fit to contents, per-space appearance, and sort (one-off) are done; lenses
  and submenus for create/arrange remain.
- Settings window (own HWND) with live-applied pages: General, Appearance,
  Spaces, Lenses, Hotkeys, Advanced, About (`F-17`). **[partial]** Data-driven
  form engine plus Appearance (live preview: material, corners, colors,
  labels, grid spacing), General, and About pages with OK/Cancel/Apply.
- **[done]** Persist all `WindowSettings` fields and wire them to behavior.
  `start_hidden` and `toggle_hotkey` are wired; `snap_grid` moved to
  `GridSpec.snap`, edited by a General slider; `always_on_desktop` selects
  desktop-hosted placement at startup. (`background_alpha` was superseded by
  `theme.tint_opacity` and removed.)
- Hotkey editor and additional bindings (`F-18` partial). **[partial]** The
  toggle hotkey is editable as text with validation and conflict reporting.
- **[done]** Per-space appearance overrides (opacity, tint, text) from the
  space menu's Appearance submenu.
- **[done]** Dark-mode settings window and menus, following Windows' app
  mode (undocumented uxtheme app mode for menus, Explorer's dark control
  styles, a custom-painted tab strip). The message box and color picker stay
  light.

Exit criteria: every runtime setting is editable without hand-editing TOML, and
the tray menu can show/hide and open settings.

### M4 - Lenses (folder views)

Phased; each phase lands green.

- **[done] Phase 0 - resize and scroll.** `ResizeSpace` / `FitToContents`
  commands and `Space::user_sized`; columns follow width; edge/corner resize
  with cursors; `SpaceEvent::Wheel` scrolling with a scroll indicator; scenes
  carry only visible items, clipped below the title bar; owned `Icon` pixels
  and an LRU-bounded `IconCache` (`F-03`).
- **Phase 1 - core model.** `Space.source: SpaceSource { Items, Folder(FolderView) }`
  with `#[serde(default)]`; `FolderView { path, sort, descending,
  folders_first, show_hidden, include, exclude }`; `FolderEntry { path,
  label, kind, size, modified, hidden }` supplied by the platform so `core`
  does no IO. Commands `CreateLens`, `SetLensFolder`,
  `SetLensView`, and `SyncLens { id, entries }`, which filters, sorts
  (natural order), and reconciles by path so `ItemId`s survive refreshes.
  `AddItem` / `MoveItem` into a lens are `Rejected`. Lens items are not
  persisted (`F-07`, `F-08`).
- **Phase 2 - platform.** `FindFirstFileExW` enumeration; one watcher thread
  per lens using `ReadDirectoryChangesW`, debounced (~100 ms), full rescan off
  the UI thread, posted to the controller with a `WM_APP` wake;
  `Event::Folder { Snapshot | Unavailable }` with periodic retry;
  `Ui::watch_folder`, `pick_folder` (`IFileOpenDialog`), `open_path`
  (`ShellExecuteExW`), and `move_into_folder` (`IFileOperation` with undo; it
  pumps messages, so no runtime borrow may be held across it). Submenus for
  `MenuEntry`.
- **Phase 3 - app.** "New lens..." from space and tray menus; double-click
  opens items (also in normal spaces); drops onto a lens move into its folder
  (skipping items already there and rejecting a folder into itself); lens
  menu: Open folder, Change folder..., Sort by, Folders first, Show hidden
  files; an "unavailable" placeholder.
- **Phase 4 - views.** List and details modes, icon sizes (`F-09`), and a
  Lenses settings page with defaults.
- **Later.** Browsing into subfolders inside the lens with a back control
  (spaces does this); until then subfolders open in Explorer.

Exit criteria: creating, renaming, or deleting a file in a lens's folder
updates the lens within ~150 ms, and sort/view settings persist.

### M5 - Visibility and flow

- Roll-up policy: manual, auto after idle, and on hover-out (`F-04` full).
- Auto-hide / hide-all: hide spaces and icons; reveal on desktop click, hotkey,
  or peek (`F-11`, `F-13`).
- Double-click empty desktop toggles icon visibility, matching spaces (`F-10`).
- Per-space roll-up button and title-bar affordances.
- Smooth show/hide/roll animations via DirectComposition.
- Blurred spaces with corner radii beyond DWM's 8 px: a composition backdrop
  clipped to an anti-aliased rounded shape. The accent blur ignores window
  and blur regions, so blurred spaces are limited to DWM's radii until then.
  Check first that a host backdrop renders on never-activated windows.

Exit criteria: hide-all and peek are instant and reversible, and roll-up policy
is configurable per space.

### M6 - Multi-monitor, pages, snapshots

- Per-monitor work areas and DPI; spaces are assigned to and movable across
  monitors, and survive monitor changes (`F-19`).
- Pages: multiple named space layouts per monitor with a page switcher (`F-12`).
- Snapshots: export/import full layout and managed-store manifest; restore
  (`F-20`).
- Space-to-space snapping and alignment guides.

Exit criteria: layouts survive monitor changes, pages switch instantly, and a
snapshot restores both layout and store contents.

### M7 - New-icon rules, keyboard, polish

- Rules for new desktop items: auto-consolidate into target spaces, create new
  spaces, or leave loose (`F-21`).
- Include/exclude filters and "ignore list" for shortcuts (`F-22`).
- Keyboard navigation: move between items/spaces, open, rename, delete (`F-23`).
- Accessibility: keyboard focus, high-contrast theme, screen-reader labels.
- Performance pass: virtualize large spaces, icon cache eviction, idle CPU near
  zero.

Exit criteria: new icons file themselves per rules, and a full workflow is
possible without a mouse.

### M8 - Release engineering

- Installer (MSI or WiX), per-user install, uninstall that restores the desktop.
- Code signing and update notes.
- Release checklist, versioning policy, and a reproducible build.
- Documentation: user guide, troubleshooting (takeover restore, Explorer
  conflicts).

Exit criteria: a signed installer installs, runs, and uninstalls cleanly on a
fresh Windows 10 and Windows 11 machine.

### Customization backlog (polled 2026-10-03)

The user asked for all of these; several have since landed (marked done).
Each is listed under the
milestone that already builds what it depends on. Global settings go in
`Theme` / `WindowSettings`, and per-space overrides go in `Space`, as
`#[serde(default)]` fields. All of them are changed through `Command`s.

Appearance:
- **[done] F-24** Per-space colors and opacity: tint, text, and opacity
  overrides from the space menu's Appearance submenu (M3).
- **F-25** Icon size: small, medium, large, and extra large, globally and per
  space. Needs the jumbo-icon enumeration from M2 and DPI-aware layout. Lens
  icon sizes (F-09) share the setting (M2/M4).
- **[done] F-26** Grid density: icon spacing and edge padding sliders in
  Appearance, written to `GridSpec.gap`/`padding`; changing them re-flows
  every space (M3).
- **[done] F-27** Label style: one or two label lines, size (60-160 %), and
  shadow (automatic, always, never) in Appearance (M3).
- **F-28** Title bar style: hidden, shown on hover only, or always; title
  alignment and size. A hidden title bar needs another way to move the space,
  such as Alt+drag or a grip (M5, with the hover work).
- **F-29** Windows accent color for the selection highlight and border (M3).
- **F-30** Settings-window and menu theme: follow Windows (now), always dark,
  or always light. Feeds `Ui::set_ui_dark` (M3).

Behavior:
- **F-04** Roll up on mouse-out and expand on hover (M5, roll-up policy).
- **[done, one-off] F-31** Sort a normal space by name, type, size, or date,
  ascending or descending, from the space menu's Sort by submenu. A kept sort
  rule (for normal spaces and lenses, F-08) remains open. `Item` now carries
  `size` and `modified`, read once at creation.
- **F-10** Hide all on desktop double-click (M5).
- **F-32** Hover opacity: idle spaces fade to a chosen opacity and become
  solid on hover (M5; the transparency half of F-13).
- **F-33** Animations on or off, and their speed (M5, together with the
  DirectComposition animations).
- **F-34** Start with Windows: a per-user `Run` key toggled from General
  settings; the installer also offers it (M3, M8).
- **F-35** Snap to other spaces, with alignment guides (M6).

### Item interaction (next session, from manual testing 2026-10-03)

Manual testing found cross-space drags duplicated items and desktop drags left
`- Copy` files. The identity fix (canonical-path matching, `ItemStore::contains`)
and the F-36..F-39 interaction changes have landed.

- **[done] F-36** Explorer-parity drag: dragging out defaults to **move**
  (`CFSTR_PREFERREDDROPEFFECT` on the drag data object), and spaces are OLE
  `IDropTarget`s that show the move/copy cursor and accept drops directly,
  replacing `WM_DROPFILES`. On a DeskSpace target Shift means "preserve the
  original" (a copy), the reverse of Explorer's Shift=move, matching the rest
  of the app. Commit `28ad7af` split the identity fix out from this; confirmed
  by smoke test only, so verify on screen.
- **[done] F-37** Drag image: the item as drawn (icon and label), slightly
  translucent, follows the pointer inside a space (layered ghost window, faded
  slot) and outside it (shell drag-image manager, also over other spaces).
  Single-item only. Needs on-screen verification.
- **[done] F-38** Clipboard: Ctrl+X / Ctrl+C / Ctrl+V on the selection via the
  shell clipboard (`CF_HDROP`) and `IFileOperation`. Multi-item, pastes into the
  current space, cut items are ghosted. Needs on-screen verification.
- **[done] F-39** Delete sends the selection to the Recycle Bin with the shell
  delete (undoable, no prompt) and drops the items whose files are gone; the
  item menu's "Remove from space" / "Move back to desktop" stays a distinct
  action. Needs on-screen verification.

### Native-appearance follow-ups (from the display work)

These continue "match Explorer as closely as possible" after the display core
landed (check boxes mirroring selection, filename extensions, hidden files,
icon overlays). All need the desktop's view settings read through
`win32/display_settings.rs` / `win32/shell_view.rs`.

- **F-40** Icon size and view mode, driven by the desktop's
  `IFolderView2::GetViewModeAndIconSize` (and Explorer's own icon rendering);
  also covers **F-25**. Needs DPI-aware layout and jumbo icons (M2).
- **[done] F-41** Independent "item check boxes" mode (`FWF_CHECKSELECT`): boxes
  are always shown and the tick is a session-only set separate from the
  selection, and the ticked items are the operand for delete/copy/cut/drag.
- **[done] F-42** Rubber-band (marquee) selection over empty space, Ctrl+A to
  select all, and dragging the whole multi-selection (a custom `IDataObject`
  serves the multi-file `CF_HDROP`). `active_items()` now centralizes the
  operand so F-41 can swap in the independent check-box set.
- **F-43** Info tips, compressed/encrypted label colors (`SSF_SHOWCOMPCOLOR`),
  and icon spacing from `IFolderView::GetSpacing`.
- **F-44** Title bar style (hidden / hover-only) — see **F-28** (M5).

## 5. Release mapping

- **v0.2 "Take the desktop"**: M0, M1, M2, M3. The first usable release:
  D2D rendering with the Windows 11 look, real desktop takeover, tray +
  context menu + settings.
- **v0.3 "Lenses"**: M4.
- **v0.4 "Flow"**: M5.
- **v0.5 "Rooms"**: M6, M7.
- **v1.0**: M8.

## 6. spaces parity checklist

| Id | Feature | Milestone | Status |
|----|---------|-----------|--------|
| F-01 | Spaces / icon groups | M2 | Partial (references, no takeover) |
| F-02 | Create / rename / delete space | M2 | Create, delete, inline rename done |
| F-03 | Resize space | M2 | Done (edges/corners, scroll, fit to contents) |
| F-04 | Roll-up spaces | M2 (manual), M5 (policy) | Manual done |
| F-05 | Lock space | M2 | Done |
| F-06 | Drag icons in / out / between spaces | M2 | Done (rearranges within a space; OLE drop feedback; drag image) |
| F-07 | Lenses (spaces: Folder Portals) | M4 | Open (phase 0 done) |
| F-08 | Lens sort rules | M4 | Open |
| F-09 | Lens show-as / icon size | M4 | Open |
| F-10 | Hide all on desktop double-click | M5 | Open |
| F-11 | Auto-hide | M5 | Open |
| F-12 | Pages / virtual desktops | M6 | Open |
| F-13 | Quick peek / transparency toggle | M5 | Open |
| F-14 | Right-click context menu | M2 (basic), M3 (full) | Space menu + shell item menu |
| F-15 | Snap to grid / auto-arrange | Core | Done |
| F-16 | System tray | M3 | Done (takeover items pending) |
| F-17 | Settings window | M3 | Partial (3 of 7 pages) |
| F-18 | Hotkeys | M2 (toggle), M3 (editor) | Toggle + text editor done |
| F-19 | Multi-monitor | M6 | Partial (per-monitor work areas) |
| F-20 | Backup / restore / snapshots | M6 | Open |
| F-21 | Auto-consolidate new icons | M7 | Open |
| F-22 | Exclude rules / filters | M7 | Open |
| F-23 | Keyboard navigation | M7 | Open |
| F-24 | Appearance: color, translucency, blur | M1 (renderer), M3 (settings) | Done globally and per space (menu: opacity, tint, text) |
| F-25 | Icon size | M2/M4 | Open (polled) || F-26 | Grid density | M3 | Done (icon spacing + edge padding sliders) |
| F-27 | Label style | M3 | Done (lines, size, shadow mode) |
| F-28 | Title bar style | M5 | Open (polled) |
| F-29 | Windows accent color | M3 | Open (polled) |
| F-30 | Settings and menu theme override | M3 | Open (polled); follows Windows today |
| F-31 | Sort items in normal spaces | M3/M4 | Done (one-off by name/type/size/date; kept rule open) |
| F-32 | Hover opacity | M5 | Open (polled) |
| F-33 | Animations toggle | M5 | Open (polled) |
| F-34 | Start with Windows | M3, M8 | Open (polled) |
| F-35 | Snap to other spaces | M6 | Open (polled) |

## 7. Open questions

Progress note: the v0.2 cut and the managed-store model are decided (see
sections 2 and 5). Items below are still open and should be resolved during the
milestone that raises them.

1. Takeover safety (M2): when Explorer restarts or recovers from a crash, how
   aggressively should DeskSpace re-hide `SHELLDLL_DefView`? A watchdog timer is
   likely needed.
2. **Resolved.** Blur/Acrylic fallback (M1): the accent policy blurs on
   Windows 10 1803+ and Windows 11 regardless of activation; rounded corners
   need Windows 11. Without the API, spaces keep a translucent tint with no
   blur. Desktop-hosted spaces are opaque because child windows get no DWM
   material.
5. Desktop layer vs blur (M2): top-level spaces have blur but are hidden by
   Show Desktop (Win+D); desktop-hosted spaces survive Win+D but cannot blur.
   Takeover (D1) needs a placement that gets both, or a Win+D watcher that
   re-shows top-level spaces.
3. Undo model (M2): single-level or multi-level undo stack, and does it cover
   file moves (which are destructive)?
4. **Resolved.** Store layout (M2): one shared folder; deleting a space moves
   its stored files back to the desktop (see D3).

## 8. Risks

- **Takeover fragility.** Hiding and driving the Explorer icon layer is
  undocumented and version-sensitive; a watchdog and a hard "restore desktop"
  path are required.
- **Data loss on relocation.** Moving real files is destructive. Mitigations:
  move-into-store with undo, never overwrite, snapshot the manifest before
  bulk operations.
- **Composition complexity.** DirectComposition plus per-monitor DPI plus
  animation is the largest single technical investment.
- **Undocumented blur API.** `SetWindowCompositionAttribute` could change in a
  future Windows release. The failure mode is graceful (no blur), and the
  code path is isolated in `win32/dwm.rs`.
- **Scope.** The parity checklist spans a lot of surface area; the milestone
  gates exist to keep each release shippable.
