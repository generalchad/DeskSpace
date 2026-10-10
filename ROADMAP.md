# ROADMAP

The plan for bringing DeskSpace to feature parity with comparable tools.
`CONTEXT.md` describes the current architecture and `AGENTS.md` the engineering
rules. `TASKS.md` breaks the next steps into concrete, one-commit work items.

How to read this file:

- Sections 1-3 are stable. They cover the goal, the decisions already made,
  and the principles.
- Section 4 says where the project is and what comes next.
- Section 5 lists the milestones in dependency order. Each one gives its goal,
  what is done, what remains (in the order to build it), and how to tell it is
  finished.
- Section 7 is the single index of features. Feature ids (`F-xx`), milestone
  ids (`Mx`), and decision ids (`Dx`) are stable and are cited from code and
  other docs, so never renumber them.
- Appendix A is the history of what landed, in order.

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
  listview and draws the desktop items itself on a desktop-hosted window.
  Items are real filesystem entries, so moving an item into a space relocates
  the real file.
- **D2. Rendering backend.** Each space is its own window.
  - Content is drawn with Direct2D/DirectWrite into a premultiplied-alpha HWND
    target over a DWM-extended frame.
  - The blur comes from the accent policy (`SetWindowCompositionAttribute`).
    Corners, border, and dark mode come from DWM attributes.
  - GDI is retired. DirectComposition waits for the animation work in M5.
  - The documented `DWMWA_SYSTEMBACKDROP_TYPE` was rejected: system backdrops
    only render on active windows, and spaces are never active.
  - Fallback: without the accent API, spaces keep a translucent tint with no
    blur. Rounded corners need Windows 11. Desktop-hosted spaces are opaque,
    because child windows get no DWM material.
- **D3. Item storage model.** There are two mechanisms, kept strictly
  separate:
  - *Lenses* (comparable tools calls them "Folder Portals") are views over a
    real folder. They read its contents live and never move, copy, or
    duplicate data on their own, whatever the folder's size. Dropping files
    onto a lens is a move into the bound folder that the user starts. It uses
    the shell's `IFileOperation`, so the standard conflict dialogs appear and
    Explorer can undo it.
  - *Normal spaces* hold individual items. Dropping an item into a normal
    space moves that one item into a managed store. It is an atomic rename on
    the same volume, so nothing is duplicated and no extra disk space is used.
    - The store is one folder shared by all spaces:
      `%LOCALAPPDATA%\DeskSpace\storage`, or a `storage` folder beside an
      explicit `--config` file. Moving an item between spaces therefore never
      touches the file.
    - Two kinds of file stay in place: one dropped with Shift held, and one on
      another drive, where a move would be a copy.
    - Name clashes get " (2)" as in Explorer, and nothing is overwritten.
    - Removing an item, or deleting its space, moves stored files back to the
      desktop.
- **D4. v0.2 scope.** The first usable release must have a tray icon,
  right-click context menus, and a settings window.
- **D5. Naming.** Containers are called *spaces* (spaces calls them spaces)
  in the UI, the docs, and the code (`Space`, `SpaceId`). Config files that
  still use the old `spaces` key load unchanged. The folder-mirror feature is
  called a *lens* in the UI and docs. In code it is a space whose source is a
  folder (`SpaceSource::Folder`), which keeps the user-facing name out of the
  model.
- **D6. Distribution and updates (2026-10-10).**
  - *Channels:* GitHub Releases is the source of truth. Each release ships
    a Velopack installer (the default), with its update feed, and a portable
    zip. winget is added after the first stable release.
  - *Installer and updater:* Velopack. It gives a per-user install with no
    admin, shortcuts, an uninstaller, and delta updates fed from the release
    assets.
  - *Install root vs user data:* the Velopack pack id is `DeskSpace.App`, so
    the install root is `%LocalAppData%\DeskSpace.App`. Velopack deletes the
    whole install root on uninstall. The config and the item storage (under
    `%LocalAppData%\DeskSpace\` and `%AppData%\DeskSpace\`, via
    `ProjectDirs`) must never be inside it, or uninstalling would delete the
    user's stored files.
  - *Uninstall:* removes the Run value (F-34) and the desktop "Show spaces"
    verb by handling Velopack's uninstall callback. It never touches stored
    files: the storage folder stays where it is, and a reinstall picks it up
    again.
  - *Check policy:* opt-in. "Check for updates automatically" on the General
    page is off by default, with no first-run prompt. When it is on, the app
    checks about a minute after start and then every 24 h while running. A
    manual "Check for updates" tray entry is always available. No network
    traffic happens unless the user starts it or opts in.
  - *Apply:* prompt, then restart. The prompt shows the version and notes
    and offers Update / Later / Skip this version. Update downloads,
    applies, and restarts DeskSpace with the same config. Later asks again
    at the next check. A skipped version is stored and not offered again.
  - *Channels within the updater:* stable by default. An opt-in setting
    receives pre-releases (`-alpha.N`, `-beta.N`, `-rc.N`).
  - *Portable zip:* the same opt-in check, but notify only. It offers to open
    the release page and never replaces its own files.
  - *Signing:* SignPath Foundation (free for open source), wired into
    `release.yml`. Signing ships before or together with auto-update.

## 3. Guiding principles

- **Keep the domain pure.** All new behavior is a `Command` variant in
  `deskspace-core`. Platform code never mutates the model directly.
- **Keep rendering and UI data-driven.** The app builds a `SpaceScene` /
  `SpaceStyle` per space, `MenuEntry` lists, and `SettingsForm`s. The platform
  only turns them into windows, controls, and pixels.
- **Make destructive operations recoverable.** File moves and layout changes
  get an undo path and a snapshot/restore mechanism.
- **Degrade gracefully.** If takeover or blur fails, fall back to plain
  top-level space windows with a translucent tint instead of crashing.
- **Verify before building on top.** Each milestone lands with tests, a clean
  clippy run, and a runnable binary. Behavior that has only been unit-tested is
  checked on screen before other work depends on it.

## 4. Where we are

**Done:** M0 (foundations) and M1 (renderer and Windows 11 look).

**In progress:**
- *M2:* spaces and their items are complete and behave like Explorer.
  Desktop takeover and undo have not started.
- *M3:* the tray, menus, and settings window work. A few pages and settings
  are still missing.

**Next, in order:**

1. **On-screen verification pass.** Many behaviors from the item-interaction
   work have only been unit-tested or smoke-tested (`CONTEXT.md` section 2
   lists them). Confirm them before more work builds on them.
2. **M2 takeover.** Enumerate the desktop through the shell. Draw the loose
   desktop items with the same item code the spaces use. Then hide Explorer's
   icon layer, with a safe restore path. This is the remaining v0.2 blocker.
3. **M2 undo** for file moves and space deletion. First settle open
   question 3.
4. **Finish M3:** the per-space settings rail (F-46, `TASKS.md` section H),
   and the New spaces and Hotkeys settings pages. Then cut v0.2.

Not next: mapping Explorer's list/details view mode (the rest of F-40). It
needs DeskSpace to have list and details views first, and those arrive with
M4 phase 4.

## 5. Milestones

Milestones are ordered by dependency. Each item is marked **[done]**,
**[partial]**, or not marked at all (not started). Within "Remaining", items
are listed in the order to build them.

### M0 - Foundations and guardrails  **[done]**

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

### M1 - Direct2D renderer and Windows 11 look  **[done]**

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

### M2 - Desktop items and takeover  **[partial]**

Goal: DeskSpace owns the desktop icon layer, and every item behaves like an
Explorer item, both inside spaces and loose on the desktop.

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
- **[partial]** Shell-sourced icons. `IShellItemImageFactory` renders icons
  at the effective pixel size, with overlays from the system image list.
  `SHGetFileInfoW` remains a fallback.

Remaining:
1. **Enumerate the desktop through the shell.** List items with
   `IShellFolder` and read their positions from the desktop view, instead of
   walking the folder and keeping positions in the model.
2. **Draw loose desktop items** in a free grid on a desktop-hosted window. Use
   the same item drawing, hit-testing, selection, rubber-band, and drag code
   that spaces use. Then dragging works within the desktop, into a space, and
   out of one (F-06, desktop half).
3. **Hide Explorer's icon layer** (`SHELLDLL_DefView` / `SysListView32`):
   - restore it on exit and after a crash;
   - re-hide it after Explorer restarts, using a watchdog (open question 1);
   - decide how spaces survive Show Desktop (Win+D) while keeping their blur
     (open question 2).
4. **Watch the filesystem.** Keep the item model current after file
   operations and outside changes. Build the watcher so lenses can reuse it
   (M4 phase 2).
5. **Undo** for file moves, item removal, and space deletion (open question
   3). F-02's "delete with undo" depends on it.

Exit criteria:
- With DeskSpace running, Explorer's icon layer is gone and the desktop items
  are drawn and interactive.
- Dragging moves real files, and the move can be undone.
- Exiting, or a crash, restores the Explorer desktop.

### M3 - Control center: tray, context menus, settings  **[partial]**

Goal: every runtime setting can be edited without touching the TOML file.

Done:
- **[done]** Tray icon (F-16): Settings, Hide/Show spaces, New space, and Exit.
  While spaces are hidden, a desktop right-click verb, "Show DeskSpace spaces",
  brings them back.
- **[done]** Space context menu (F-14):
  - create, delete, lock, roll up, and fit to contents;
  - an Appearance submenu: per-space opacity, tint, and text color (F-24);
  - a one-off Sort by (F-31).
- **[done]** Settings window (F-17). A data-driven form engine with live
  preview, OK/Cancel/Apply, and the tabs Appearance, Labels, Layout, Colors,
  Advanced, General, and About. It covers the material, corners, colors, label
  style (F-27), grid density (F-26), icon size (F-25, global), and overlay
  size.
- **[done]** Every `WindowSettings` field is wired to behavior:
  - `start_hidden`;
  - `toggle_hotkey`, editable as text, with validation and conflict reporting
    (F-18, partial);
  - `always_on_desktop`, applied at the next launch;
  - snapping, now `GridSpec.snap`.
- **[done]** Dark mode for the settings window and menus, following Windows.

Remaining:
1. A per-space settings surface (F-46, `TASKS.md` section H). The settings
   window gets a navigation rail on the left in place of its top tabs:
   - a Settings group with the global pages;
   - a Spaces group listing every space that has an appearance override.

   Selecting a space shows its overrides in one shared page, previews edits
   live, and flashes that space on the desktop. Cancel reverts. Edits go
   through the `SetSpace*` commands.
2. A "New spaces" settings page (defaults for new spaces), and a Hotkeys page
   with more bindings (F-17, F-18). The page is not called "Spaces", so it
   does not clash with the rail group.
3. Tray and menu entries that depend on later milestones:
   - pause takeover (after M2);
   - new lens (M4);
   - snapshots (M6);
   - an Arrange submenu.

Exit criteria: every runtime setting can be edited in the UI, and the tray
menu can show and hide spaces and open settings.

### M4 - Lenses (folder views)

Goal: spaces that mirror a folder live. Each phase lands green.

- **[done] Phase 0 - resize and scroll.**
  - `ResizeSpace` / `FitToContents`, and `Space::user_sized`; the columns
    follow the width.
  - Wheel scrolling with an indicator.
  - Scenes carry only the visible items.
  - An LRU-bounded `IconCache` (F-03).
- **Phase 1 - core model.**
  - A source on each space:
    `Space.source: SpaceSource { Items, Folder(FolderView) }`, with
    `#[serde(default)]`.
  - The view settings: `FolderView { path, sort, descending, folders_first,
    show_hidden, include, exclude }`.
  - Entries come from the platform, so `core` does no IO: `FolderEntry { path,
    label, kind, size, modified, hidden }`.
  - New commands: `CreateLens`, `SetLensFolder`, `SetLensView`, and
    `SyncLens { id, entries }`. `SyncLens` filters, sorts in natural order, and
    reconciles by path, so `ItemId`s survive refreshes.
  - `AddItem` / `MoveItem` into a lens are `Rejected`.
  - Lens items are not persisted (F-07, F-08).
- **Phase 2 - platform.**
  - Folder listing with `FindFirstFileExW`.
  - A watcher (shared with M2) using `ReadDirectoryChangesW`:
    - debounced (about 100 ms);
    - a full rescan off the UI thread, then a `WM_APP` wake posted to the
      controller.
  - `Event::Folder { Snapshot | Unavailable }`, with a periodic retry.
  - New `Ui` calls:
    - `Ui::watch_folder`;
    - `pick_folder` (`IFileOpenDialog`);
    - `move_into_folder` (`IFileOperation` with undo). It pumps messages, so
      no runtime borrow may be held across it.
- **Phase 3 - app.**
  - "New lens..." in the space and tray menus.
  - Drops onto a lens move files into its folder. Items already there are
    skipped, and a folder cannot be dropped into itself.
  - A lens menu: Open folder, Change folder..., Sort by, Folders first, and
    Show hidden files.
  - A placeholder when the folder is unavailable.
  - Double-click to open already works through `Ui::open_path`.
- **Phase 4 - views and kept sort rules.**
  - List and details views and icon sizes (F-09). Map Explorer's
    `FOLDERVIEWMODE` onto these views under "Match Explorer" (the rest of
    F-40).
  - A kept sort rule for lenses and normal spaces (F-08, F-31).
  - A Lenses settings page with defaults (F-17).
- **Later.** Browse into subfolders inside the lens, with a back control (as
  spaces does). Until then, subfolders open in Explorer.

Exit criteria: creating, renaming, or deleting a file in a lens's folder
updates the lens within about 150 ms, and the sort and view settings persist.

### M5 - Visibility and flow

Goal: hide, reveal, and peek instantly; then animate it.

Remaining:
1. Roll-up policy: manual (done), automatic after idle, and roll up on
   mouse-out / expand on hover (F-04). Add a per-space roll-up button.
2. Hide all: hide spaces and icons, and reveal them on a desktop click, a
   hotkey, or a peek (F-11, F-13). Double-clicking empty desktop toggles them,
   as in spaces (F-10).
3. Hover opacity: idle spaces fade to a chosen opacity and become solid on
   hover (F-32).
4. Title bar style: hidden, hover-only, or always shown, with alignment and
   size (F-28). A hidden title bar needs another way to move the space, such
   as Alt+drag or a grip.
5. DirectComposition groundwork, then smooth show/hide/roll animations, with
   an on/off switch and a speed setting (F-33).
6. Blurred spaces with corner radii beyond DWM's 8 px, using a composition
   backdrop clipped to an anti-aliased rounded shape.
   - Today the accent blur ignores window and blur regions, so blurred spaces
     are limited to DWM's radii.
   - First check that a host backdrop renders on windows that are never
     activated.

Exit criteria: hide-all and peek are instant and reversible, and the roll-up
policy can be set per space.

### M6 - Multi-monitor, snapshots, pages

Goal: layouts that survive hardware changes and can be saved and switched.

Done:
- **[done]** Spaces work on any monitor. Bounds come from each monitor's work
  area, and scaling from its DPI (F-19, partial).

Remaining:
1. Assign spaces to monitors, and keep the layout when monitors are added,
   removed, or rearranged (F-19).
2. Snapshots: export and import the full layout plus a manifest of the managed
   store, and restore from them (F-20). Take a snapshot before bulk
   operations.
3. Pages: several named layouts per monitor, with a page switcher (F-12).
   A page is a stored layout, so it builds on the snapshot format.
4. Snapping to other spaces, with alignment guides (F-35).
5. A desktop overview map (F-47): a scaled picture of every monitor with
   its wallpaper and miniature spaces.
   - Clicking a miniature selects that space in the F-46 rail.
   - Later, it hosts monitor assignment, the page switcher, and snapshot
     previews.
   - It needs monitor enumeration (today the platform only has
     `work_area_at`) and its own draw-list contract, so the platform stays
     data-driven.
   - It comes after items 1 and 3, so it is built on the final monitor and
     page model.

Exit criteria: layouts survive monitor changes, pages switch instantly, and a
snapshot restores both the layout and the store's contents.

### M7 - Rules, keyboard, accessibility, performance

Goal: new icons sort themselves, and the whole app works without a mouse.

Remaining:
1. Rules for new desktop items: consolidate them into target spaces, create
   new spaces, or leave them loose (F-21).
2. Include/exclude filters and an ignore list for shortcuts (F-22).
3. Keyboard navigation: move between items and spaces, and open, rename, or
   delete (F-23).
4. Accessibility: keyboard focus, a high-contrast theme, and screen-reader
   labels.
5. Performance pass: idle CPU near zero, and large spaces handled well. Scenes
   already carry only the visible items, and the icon cache is already
   LRU-bounded.

Exit criteria: new icons file themselves according to the rules, and a full
workflow is possible without a mouse.

### M8 - Release engineering

Goal: a signed installer that works on a fresh machine.

Decisions: D6. Work items: `TASKS.md` section E (ready to do) and D10-D13
(maintainer work).

Remaining:
1. A Velopack installer (D6):
   - a per-user install in `%LocalAppData%\DeskSpace.App`, separate from the
     user data;
   - an option to start with Windows (F-34);
   - an uninstaller that removes the Run value and the desktop verb, restores
     the desktop (after M2), and leaves stored files in place.
2. An `.ico` resource for the executable, replacing the icon drawn in code
   (`win32/icon.rs`).
3. Code signing through SignPath Foundation (D6), and release notes.
4. A Windows 11 top-level context menu entry. Move "Show spaces" out of
   "Show more options" with an `IExplorerCommand` handler.
   - The handler ships in a signed MSIX sparse package with the
     `windows.fileExplorerContextMenus` extension and an associated stub
     executable.
   - Registry-only verbs cannot reach the new menu, so this needs code
     signing.
   - M2 takeover may make it unnecessary by giving DeskSpace its own desktop
     menu.
5. A release checklist, a versioning policy, and a reproducible build.
   `.github/workflows/release.yml` already publishes a zip (exe, `LICENSE`,
   `NOTICE`, `README.md`) and its SHA-256 to GitHub Releases on a `v*` tag
   that matches the workspace version.
6. Documentation: a user guide, and troubleshooting for restoring after
   takeover and for conflicts with Explorer.
7. Auto-update from GitHub Releases (F-45), following the policy in D6.
   - The Velopack SDK and the network IO live in a new `deskspace-update`
     crate, driven by the app; never in `core` or `config`.
   - Settings live in `WindowSettings` as `#[serde(default)]` fields: the
     automatic check, the pre-release channel, the last check time, and the
     skipped version. The General page gets check boxes, and the tray gets a
     "Check for updates" entry.
   - A portable build (not installed by Velopack) only notifies.
   - New dependencies must pass `deny.toml`.
   - Ship it together with item 3: unsigned updates trigger SmartScreen on
     every version.
8. A winget manifest in `microsoft/winget-pkgs` for the installer, updated
   per stable release by a workflow step, after the first stable release.

Exit criteria: a signed installer installs, runs, updates to a newer release,
and uninstalls cleanly on a fresh Windows 10 machine and a fresh Windows 11
machine.

## 6. Release mapping

- **v0.2 "Take the desktop"**: M0, M1, M2, M3. The first usable release, with
  D2D rendering and the Windows 11 look, real desktop takeover, and the tray,
  context menu, and settings.
- **v0.3 "Lenses"**: M4.
- **v0.4 "Flow"**: M5.
- **v0.5 "Rooms"**: M6, M7.
- **v1.0**: M8.

## 7. Feature index

This is the single list of features, which also serves as the spaces parity
checklist. "Check pending" means the feature is implemented and unit-tested
or smoke-tested, but has not yet been confirmed on screen. Global settings
belong in `Theme` / `WindowSettings`, and per-space overrides in `Space`, as
`#[serde(default)]` fields changed through `Command`s.

| Id | Feature | Milestone | Status |
|----|---------|-----------|--------|
| F-01 | Spaces / icon groups | M2 | Partial: spaces work; desktop takeover open |
| F-02 | Create / rename / delete space | M2 | Done; delete has no undo yet |
| F-03 | Resize space | M2, M4 | Done (edges/corners, scroll, fit to contents) |
| F-04 | Roll-up spaces | M2 (manual), M5 (policy) | Manual done; policy open |
| F-05 | Lock space | M2 | Done |
| F-06 | Drag icons in / out / between spaces | M2 | Done for spaces; loose desktop items wait for takeover |
| F-07 | Lenses (spaces: Folder Portals) | M4 | Open |
| F-08 | Lens sort rules | M4 | Open |
| F-09 | Lens show-as / icon size | M4 | Open |
| F-10 | Hide all on desktop double-click | M5 | Open |
| F-11 | Auto-hide | M5 | Open |
| F-12 | Pages / virtual desktops | M6 | Open |
| F-13 | Quick peek / transparency toggle | M5 | Open |
| F-14 | Right-click context menu | M2 (basic), M3 (full) | Space menu and Explorer item menu done; lens and arrange entries open |
| F-15 | Snap to grid / auto-arrange | Core | Done |
| F-16 | System tray | M3 | Done; takeover and snapshot entries wait for M2 and M6 |
| F-17 | Settings window | M3, M4 | Partial: 7 tabs; New spaces, Hotkeys, and Lenses pages open; navigation rail with F-46 |
| F-18 | Hotkeys | M3 | Toggle hotkey and its text editor done; more bindings open |
| F-19 | Multi-monitor | M1, M6 | Partial: per-monitor work areas and DPI; monitor assignment open |
| F-20 | Backup / restore / snapshots | M6 | Open (config file only) |
| F-21 | Auto-consolidate new icons | M7 | Open |
| F-22 | Exclude rules / filters | M7 | Open |
| F-23 | Keyboard navigation | M7 | Open |
| F-24 | Appearance: color, translucency, blur | M1, M3 | Done, globally and per space |
| F-25 | Icon size | M3 | Done; check pending (global 50-200 % slider, jumbo icons, per-monitor DPI, per-space override) |
| F-26 | Grid density | M3 | Done (icon spacing and edge padding) |
| F-27 | Label style | M3 | Done (lines, size, shadow mode) |
| F-28 | Title bar style | M5 | Open |
| F-29 | Windows accent color | M3 | Done; check pending |
| F-30 | Settings and menu theme override | M3 | Done; check pending |
| F-31 | Sort items in normal spaces | M3, M4 | One-off sort done; kept rule open (with F-08) |
| F-32 | Hover opacity | M5 | Open |
| F-33 | Animations toggle | M5 | Open |
| F-34 | Start with Windows | M3, M8 | Done; check pending |
| F-35 | Snap to other spaces | M6 | Open |
| F-36 | Explorer-parity drag (move default, OLE drop target) | M2 | Done; check pending |
| F-37 | Drag image | M2 | Done; check pending |
| F-38 | Clipboard cut / copy / paste | M2 | Done; check pending |
| F-39 | Delete to the Recycle Bin | M2 | Done; check pending |
| F-40 | Explorer icon size and view mode | M2, M4 | Icon size done; view mode waits for M4 phase 4 views |
| F-41 | Independent item check boxes | M2 | Done; check pending |
| F-42 | Rubber-band, Ctrl+A, multi-item drag | M2 | Done within one space; check pending |
| F-43 | Info tips, compressed/encrypted colors, Explorer spacing | M2 | Done; check pending |
| F-44 | Title bar style (duplicate) | - | Merged into F-28 |
| F-45 | Auto-update from GitHub Releases | M8 | Open; policy decided (D6), release zip workflow done |
| F-46 | Per-space settings rail (customized spaces in the settings window) | M3 | Open; design decided (`TASKS.md` section H) |
| F-47 | Desktop overview map (monitors, wallpaper, miniature spaces) | M6 | Open; after F-19 and F-12 |

The polled customization backlog (2026-10-03) is F-24..F-35. The
item-interaction session (2026-10-03) is F-36..F-39, and the
native-appearance follow-ups are F-40..F-43. Each has been placed in the
milestone that builds what it depends on.

## 8. Open questions

Resolve each one during the milestone that raises it.

1. **Takeover safety (M2).** When Explorer restarts or recovers from a crash,
   how aggressively should DeskSpace re-hide `SHELLDLL_DefView`? It probably
   needs a watchdog timer.
2. **Desktop layer vs blur (M2).** Top-level spaces have blur but Show Desktop
   (Win+D) hides them. Desktop-hosted spaces survive Win+D but cannot blur.
   Takeover (D1) needs a placement that gets both, or a Win+D watcher that
   shows the top-level spaces again.
3. **Undo model (M2).** Should the undo stack be single-level or
   multi-level? Should it cover file moves, which are destructive? Explorer
   can already undo the shell operations (paste, delete, rename) through
   `FOF_ALLOWUNDO`. Moves into the store are plain renames with no undo
   record.

Resolved:
- **Blur fallback (M1).** See D2.
- **Store layout (M2).** One shared folder, and deleting a space moves its
  stored files back to the desktop. See D3.
- **Update policy (M8).** Velopack, opt-in, prompt then restart, and skip a
  version. See D6.

## 9. Risks

- **Takeover fragility.** Hiding and driving Explorer's icon layer is
  undocumented and changes between Windows versions. It needs a watchdog and a
  hard "restore desktop" path.
- **Data loss on relocation.** Moving real files is destructive. Mitigations:
  move into the store with undo, never overwrite, and snapshot the manifest
  before bulk operations.
- **Uninstall deleting user data.** Velopack deletes its install root on
  uninstall. Mitigation (D6): a distinct pack id (`DeskSpace.App`), and a test
  that the storage and config paths are never inside the install root.
- **Composition complexity.** DirectComposition plus per-monitor DPI plus
  animation is the largest single technical investment.
- **Undocumented blur API.** `SetWindowCompositionAttribute` could change in a
  future Windows release. If it does, spaces simply lose their blur, and the
  code is isolated in `win32/dwm.rs`.
- **Scope.** The parity checklist covers a lot of ground. The milestone gates
  exist to keep each release shippable.

## Appendix A. Progress log

What landed, oldest first. The details here describe the state when each
change landed. `CONTEXT.md` is authoritative for current behavior.

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
  - "spaces" renamed to "spaces" throughout (D5).
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
