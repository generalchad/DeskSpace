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
      `%LOCALAPPDATA%\DeskSpace\DeskSpace\data\storage`, or a `storage` folder
      beside an explicit `--config` file. Moving an item between spaces
      therefore never touches the file.
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
- **D6. Distribution and updates (2026-10-10; installer moved to
  cargo-packager 2026-10-12).**
  - *Channels:* GitHub Releases is the source of truth. Each release ships
    an NSIS installer (the default), its update feed, and a portable zip.
    winget is added after the first stable release.
  - *Installer:* `cargo-packager` builds a per-user NSIS `setup.exe` with no
    admin prompt, Start-menu and desktop shortcuts, and an uninstaller. It is
    a CI-only tool (`cargo install cargo-packager`), so it adds no dependency
    to the workspace. Chosen over Velopack on 2026-10-12 because Velopack's
    Rust SDK pulled a TLS and advisory stack into the tightened `deny.toml`
    and needed an app-side startup hook; see the progress log.
  - *Updater:* `cargo-packager-updater` (opt-in). It reads a signed JSON feed
    (`latest.json` plus a minisign `.sig`) published as a release asset and
    installs the **full** package; there are no delta updates.
  - *Install root vs user data:* the custom NSIS template
    (`packaging/nsis/installer.nsi`) installs into
    `%LocalAppData%\DeskSpace.App`. NSIS would otherwise default to
    `%LocalAppData%\DeskSpace`, the parent of the item storage
    (`%LocalAppData%\DeskSpace\DeskSpace\data`, via `ProjectDirs`); the two
    must stay apart, or uninstalling would delete the user's stored files.
    The config and the storage live under
    `%AppData%\DeskSpace\DeskSpace\config` and
    `%LocalAppData%\DeskSpace\DeskSpace\data`.
  - *Uninstall:* the template's uninstall section removes the Run value (F-34)
    and the desktop "Show spaces" verb and deletes the install root. It never
    touches stored files: the storage folder stays where it is, and a reinstall
    picks it up again.
  - *Check policy:* opt-in. "Check for updates automatically" on the General
    page is off by default, with no first-run prompt. When it is on, the app
    checks about a minute after start and then every 24 h while running. A
    manual "Check for updates" tray entry is always available. No network
    traffic happens unless the user starts it or opts in.
  - *Apply:* prompt, then restart. The prompt shows the version and notes
    and offers Update / Later / Skip this version. Update downloads,
    installs, and restarts DeskSpace with the same config. Later asks again
    at the next check. A skipped version is stored and not offered again.
  - *Channels within the updater:* stable by default. An opt-in setting
    receives pre-releases (`-alpha.N`, `-beta.N`, `-rc.N`).
  - *Portable zip:* the same opt-in check, but notify only. It offers to open
    the release page and never replaces its own files.
  - *Signing:* SignPath Foundation (free for open source), wired into
    `release.yml` through `cargo-packager`'s `windows.signCommand`. Signing
    ships before or together with auto-update.

- **D7. Desktop takeover model (2026-10-10).**
  - The desktop's loose icons - the Desktop-folder files that are not in any
    space - are their own model type, `DesktopLayer`, not a `Space`. It holds
    items with free positions in virtual-desktop coordinates, one entry per
    loose file, and no chrome (no title bar, border, lock, resize, roll-up, or
    sort). It is drawn on a chrome-less desktop-hosted window and sits behind
    the spaces.
  - The shell is the source of truth for membership and positions.
    `DesktopLayer` is a refreshed cache: `Command::SyncDesktop` replaces it
    from a platform snapshot and reconciles by path so `ItemId`s survive. A
    loose item's position is absolute; a space item's stays relative to its
    content area.
  - Item interaction is shared, not duplicated. The app's hit-test, hover,
    selection, rubber-band, drag, drop, clipboard, delete, and info-tip paths
    take a `Surface` (`Space(id)` or `Desktop`) instead of a `SpaceId`.
  - A loose item dropped into a space uses the existing file-move path (move
    into storage, then `Command::MoveItem`); moving a space item back to the
    desktop is `Command::MoveToDesktop`; dragging a loose item repositions it
    (`Command::SetDesktopItemPosition`).

- **D8. Takeover placement and safety (2026-10-10).** Resolves open questions
  1 and 2.
  - *Placement:* the desktop layer is a `WS_CHILD` of the desktop host
    (`Progman` / wallpaper `WorkerW`), so it survives Show Desktop (Win+D) and
    is opaque, as other hosted windows are. Spaces stay top-level with the
    DWM/accent blur.
    - Show Desktop was expected to hide the top-level spaces, and this decision
      called for a Win+D watcher to re-show them. Verified 2026-10-11 on build
      26300: it does not hide them, because Show Desktop (`MinimizeAll`) skips
      `WS_EX_TOOLWINDOW` windows and a space is
      `WS_POPUP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`. The watcher is dropped;
      see the 2026-10-11 progress entry. A space also ignores
      `WM_SYSCOMMAND`/`SC_MINIMIZE`, so survival does not depend on that
      exclusion and Show Desktop cannot tear down the accent blur.
  - *Hiding Explorer's icons:* hide the `SysListView32` of every
    `SHELLDLL_DefView` (there is one per monitor) and restore it on a clean
    exit. Re-hide after Explorer restarts (`TaskbarCreated`) and on a
    low-frequency health timer.
  - *Crash safety:* a small watchdog helper process, launched before the icons
    are hidden, re-shows every listview if the main process exits without a
    clean shutdown. The main process signals a clean exit, so the watchdog
    only acts on a crash.
  - *Opt-in during construction:* hiding is behind an off-by-default setting
    (and a hidden `--takeover` flag) until the layer and watchdog land, so a
    partial build never hides the desktop. Both have landed. **v0.2 ships it
    opt-in** (decided 2026-10-11, D9): it is an advertised General-page
    setting, off by default. The default may flip in a later release once the
    on-screen checks pass and v0.2 users have reported on it; that is a new
    decision, recorded here when made.

- **D9. Interaction and build decisions (2026-10-11).**
  - *Takeover default:* opt-in for v0.2 (see D8).
  - *Ctrl+wheel under "Match Explorer's display":* it drives the desktop's
    own icon size (`IFolderView2::SetViewModeAndIconSize`), exactly as
    Ctrl+wheel on the desktop does, and spaces that match Explorer follow
    it, so they stay 1:1 with the desktop. Under the `Custom` preset it keeps
    changing DeskSpace's own global icon size. A customized space with its
    own `Custom` look keeps its size either way. Work item: `TASKS.md` F13.
  - *Rust toolchain:* pinned to an exact version in `rust-toolchain.toml`,
    which CI and the release workflow both use, and bumped deliberately in
    its own commit. `rust-version` in `Cargo.toml` stays the minimum
    supported version. Work item: `TASKS.md` E13.
  - *Right-button drag:* Explorer-style. A right drag on an item always
    starts a shell drag (it never reorders inside the space). Dropping on
    Explorer or the desktop shows the shell's own Move / Copy / Create
    shortcut menu; dropping on a space shows DeskSpace's equivalent menu.
    Work items: `TASKS.md` G5a-G5c.
  - *Hiding spaces leaves the desktop layer visible:* "Hide spaces" (tray,
    space menu, hotkey) hides spaces only. The takeover layer stands in for
    Explorer's icons, which hiding spaces never hid. Hiding everything
    (spaces and icons) is M5's hide-all (F-11), a separate command.

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
- **Readable, maintainable, extensible first.** Every feature arrives through
  an existing extension point (`Command`, `Event`, `SpaceScene` /
  `SettingsForm` field, `#[serde(default)]` setting, `Surface`) or a newly
  designed one, never as a special case. When a milestone grows a file past
  about 800 lines of non-test code, the split is part of that milestone, not
  a later cleanup (`TASKS.md` section K). AGENTS.md "Design priorities" has
  the details.

## 4. Where we are

**Done:** M0 (foundations) and M1 (renderer and Windows 11 look).

**In progress:**
- *M2:* spaces and their items are complete and behave like Explorer.
  Desktop takeover is built and opt-in (the layer, hide, restore,
  Explorer-restart re-hide, the crash watchdog, and the Desktop-folder watch
  are in); undo and non-file shell drags (F-50) remain.
- *M3:* the tray, menus, and settings window work, with all nine pages.
  What remains depends on later milestones (lens and snapshot entries) or on
  design (more item presets).

**Next, in order:**

1. **Readability fixes and file splits** (`TASKS.md` sections A, B, and K).
   Small, safe, and they make every later task easier for any contributor.
2. **On-screen verification pass.** Many behaviors have only been
   unit-tested or smoke-tested (`CONTEXT.md` section 2 lists them, and every
   "(awaiting check)" task in `TASKS.md`). Confirm them before more work
   builds on them.
3. **Release readiness for v0.2:** ship the watchdog in the zip (E12), pin
   the toolchain (E13), then the reproducible build and release checklist
   (E3, E4).
4. **M2 undo** for file moves and space deletion. First settle open
   question 3.
5. **Cut v0.2**, with takeover as an opt-in setting (D8, D9).

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

Remaining:
1. **Watch stored files.** Only the Desktop folder is watched; a change to a
   stored file made outside DeskSpace is noticed only by an item action or at
   startup. Lenses will reuse the same watcher (M4 phase 2).
2. **Undo** for file moves, item removal, and space deletion (open question
   3). F-02's "delete with undo" depends on it.
3. **Right-button drag** with Explorer's drop menu (D9; `TASKS.md`
   G5a-G5c). Its "Create shortcuts here" shares the shortcut-creation code
   with F-50.
4. **Accept non-file shell drags (F-50).** A Start menu or taskbar app drag
   carries a Shell ID List, not `CF_HDROP`, so a space shows the red no-drop
   cursor. It should create a shortcut in the space, as Explorer does, including
   packaged apps that have no filesystem path. Needs design; see F-50.

Exit criteria:
- With DeskSpace running, Explorer's icon layer is gone and the desktop items
  are drawn and interactive.
- Dragging moves real files, and the move can be undone.
- Exiting, or a crash, restores the Explorer desktop.
- Show Desktop (Win+D) leaves the spaces visible.

### M3 - Control center: tray, context menus, settings  **[partial]**

Goal: every runtime setting can be edited without touching the TOML file.

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

Remaining:
1. Tray and menu entries:
   - pause takeover (unblocked now that the takeover exists);
   - new lens (M4);
   - snapshots (M6);
   - an Arrange submenu.
2. More item presets and preset import/export (F-48). The "Item preset"
   dropdown now seeds the system with `Match Explorer's display` and
   `Custom`; named presets and Save/Open come later (`TASKS.md` C6).
3. A modern color picker (F-51). The Colors page and the space menu's
   Appearance submenu use the dated system `ChooseColorW` dialog; replace it
   with a native-feeling, themed picker (`TASKS.md` F14). The maintainer wants
   this soon.

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
2. Hide all: hide spaces and icons (including the takeover layer, which
   "Hide spaces" leaves visible, D9), and reveal them on a desktop click, a
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
   - Clicking a miniature selects that space in the F-46 scopes pane.
   - Later, it hosts monitor assignment, the page switcher, and snapshot
     previews.
   - It needs monitor enumeration (today the platform only has
     `work_area_at`) and its own draw-list contract, so the platform stays
     data-driven.
   - It comes after items 1 and 3, so it is built on the final monitor and
     page model.
6. Tabs in spaces (F-49). Off by default. When on, a space is paginated: its
   items split across pages, one page shown at a time, so a space holds more
   icons without scrolling. A tab strip drawn on a chosen edge (top, bottom,
   left, right) switches pages, and its size is set per space. A per-space
   "Tabs" settings section carries the basic options now (enable, orientation,
   size), with room for advanced visual and placement settings later.
   - Distinct from F-12: pages are whole-desktop layouts per monitor; tabs
     paginate a single space. They share the idea of a page switcher but not
     the model.

Exit criteria: layouts survive monitor changes, pages switch instantly, and a
snapshot restores both the layout and the store's contents.

### M7 - Rules, keyboard, accessibility, performance

Goal: new icons sort themselves, and the whole app works without a mouse.

Remaining:
1. Rules for new desktop items: consolidate them into target spaces, create
   new spaces, or leave them loose (F-21).
2. Include/exclude filters and an ignore list for shortcuts (F-22).
3. Keyboard navigation between spaces and onto the desktop layer (F-23).
   Within a space it is done: arrows, type-to-select, Home/End/Page, Enter,
   F2, Delete, and the Menu key.
4. Accessibility: keyboard focus, a high-contrast theme, and screen-reader
   labels.
5. Performance pass: idle CPU near zero, and large spaces handled well. Scenes
   already carry only the visible items, and the icon cache is already
   LRU-bounded.

Exit criteria: new icons file themselves according to the rules, and a full
workflow is possible without a mouse.

### M8 - Release engineering

Goal: a signed installer that works on a fresh machine.

Decisions: D6. Work items: `TASKS.md` section E (ready to do) and R10-R13
(maintainer work).

Remaining:
1. A cargo-packager installer (D6):
   - a per-user NSIS install in `%LocalAppData%\DeskSpace.App`, separate from
     the user data;
   - an option to start with Windows (F-34);
   - an uninstaller that removes the Run value and the desktop verb, restores
     the desktop if takeover was on, and leaves stored files in place.
2. **[done]** An `.ico` resource for the executable (`assets/deskspace.ico`,
   embedded with version info by `winresource`), replacing the icon that was
   drawn in code (`TASKS.md` E1-E2; on-screen check pending).
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
   `RELEASING.md` documents the checklist and the versioning policy.
   `.github/workflows/release.yml` already publishes a zip (`deskspace.exe`,
   `deskspace-watchdog.exe`, `LICENSE`, `NOTICE`, `README.md`) and its SHA-256
   to GitHub Releases on a `v*` tag that matches the workspace version.
6. Documentation: a user guide, and troubleshooting for restoring after
   takeover and for conflicts with Explorer.
7. Auto-update from GitHub Releases (F-45), following the policy in D6.
   - The `cargo-packager-updater` SDK and the network IO live in a new
     `deskspace-update` crate, driven by the app; never in `core` or `config`.
   - It installs the **full** package only (no deltas), from a signed
     `latest.json` plus a minisign `.sig` published as a release asset.
   - Settings live in `WindowSettings` as `#[serde(default)]` fields: the
     automatic check, the pre-release channel, the last check time, and the
     skipped version. The General page gets check boxes, and the tray gets a
     "Check for updates" entry.
   - A portable build only notifies.
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
  D2D rendering and the Windows 11 look, desktop takeover as an opt-in
  setting (D8, D9), and the tray, context menu, and settings.
- **v0.3 "Lenses"**: M4.
- **v0.4 "Flow"**: M5.
- **v0.5 "Rooms"**: M6, M7.
- **v1.0**: M8.

## 7. Feature index

This is the single list of features, which also serves as the spaces parity
checklist. "Check pending" means the feature is implemented and unit-tested
or smoke-tested, but has not yet been confirmed on screen. Global settings
belong in `Theme` / `WindowSettings` (a look setting in `Theme` is per space too,
through `SpaceLook`), and other per-space state in `Space`, as
`#[serde(default)]` fields changed through `Command`s.

| Id | Feature | Milestone | Status |
|----|---------|-----------|--------|
| F-01 | Spaces / icon groups | M2 | Done; the desktop takeover is opt-in and its on-screen checks are pending |
| F-02 | Create / rename / delete space | M2 | Done; delete has no undo yet |
| F-03 | Resize space | M2, M4 | Done (edges/corners, scroll, fit to contents); whole-column and whole-row fit checked 2026-10-12, gated by the General page's "Snap resized spaces" (`GridSpec.fit_to_grid`) |
| F-04 | Roll-up spaces | M2 (manual), M5 (policy) | Manual done; policy open |
| F-05 | Lock space | M2 | Done |
| F-06 | Drag icons in / out / between spaces | M2 | Done for spaces and for loose items on the takeover layer (opt-in); check pending for the layer |
| F-07 | Lenses (spaces: Folder Portals) | M4 | Open |
| F-08 | Lens sort rules | M4 | Open |
| F-09 | Lens show-as / icon size | M4 | Open |
| F-10 | Hide all on desktop double-click | M5 | Open |
| F-11 | Auto-hide | M5 | Open |
| F-12 | Pages / virtual desktops | M6 | Open |
| F-13 | Quick peek / transparency toggle | M5 | Open |
| F-14 | Right-click context menu | M2 (basic), M3 (full) | Space menu and Explorer item menu done; lens and arrange entries open |
| F-15 | Snap to grid / auto-arrange | Core | Done |
| F-16 | System tray | M3 | Done; a pause-takeover entry is open, snapshot entries wait for M6 |
| F-17 | Settings window | M3, M4 | Partial: 9 pages in a scopes/pages/page layout (New spaces: check pending); Lenses page open |
| F-18 | Hotkeys | M3 | Done; check pending (Hotkeys page binds show/hide, new space, and settings) |
| F-19 | Multi-monitor | M1, M6 | Partial: per-monitor work areas and DPI; monitor assignment open |
| F-20 | Backup / restore / snapshots | M6 | Open (config file only) |
| F-21 | Auto-consolidate new icons | M7 | Open |
| F-22 | Exclude rules / filters | M7 | Open |
| F-23 | Keyboard navigation | M7 | Partial: within one space (`TASKS.md` G1, G3; check pending); between spaces open |
| F-24 | Appearance: color, translucency, blur | M1, M3 | Done, globally and per space (a customized space has its own copy of every look setting) |
| F-25 | Icon size | M3 | Done; check pending (global 50-200 % slider, jumbo icons, per-monitor DPI, per space through its look). Ctrl+wheel does nothing over a space under any preset (2026-10-12); Match Explorer behavior is `TASKS.md` F13 |
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
| F-36 | Explorer-parity drag (move default, OLE drop target) | M2 | Done; check pending. Right-button drag decided (D9), open (`TASKS.md` G5a-G5c) |
| F-37 | Drag image | M2 | Done; check pending |
| F-38 | Clipboard cut / copy / paste | M2 | Done; check pending |
| F-39 | Delete to the Recycle Bin | M2 | Done; check pending |
| F-40 | Explorer icon size and view mode | M2, M4 | Icon size done; view mode waits for M4 phase 4 views |
| F-41 | Independent item check boxes | M2 | Done; check pending |
| F-42 | Rubber-band, Ctrl+A, multi-item drag | M2 | Done within one space; check pending |
| F-43 | Info tips, compressed/encrypted colors, Explorer spacing | M2 | Done; check pending |
| F-44 | Title bar style (duplicate) | - | Merged into F-28 |
| F-45 | Auto-update from GitHub Releases | M8 | Open; cargo-packager installer workflow decided (D6); in-app updater still open |
| F-46 | Per-space looks in the settings window (scopes pane, Customize switch, highlight) | M3 | Done; check pending (`TASKS.md` H5, H4) |
| F-47 | Desktop overview map (monitors, wallpaper, miniature spaces) | M6 | Open; after F-19 and F-12 |
| F-48 | Item display presets (Match Explorer / Custom; more presets and import/export later) | M3, M6 | Partial: `Match Explorer's display` / `Custom` dropdown (`TASKS.md` C5); import/export open (C6) |
| F-49 | Tabs in spaces (per-space pagination, off by default) | M6 | Open (`TASKS.md` section J) |
| F-50 | Accept non-file shell drags (Start menu / taskbar apps) as shortcuts | M2 | Open; a space shows the no-drop cursor for them (drop target accepts `CF_HDROP` only) |
| F-51 | Modern color picker (replace the dated system `ChooseColor` dialog) | M3 | Open; the Colors page and the Appearance menu use `ChooseColorW`, which looks out of place on Windows 11 |

The polled customization backlog (2026-10-03) is F-24..F-35. The
item-interaction session (2026-10-03) is F-36..F-39, and the
native-appearance follow-ups are F-40..F-43. Each has been placed in the
milestone that builds what it depends on.

## 8. Open questions

Resolve each one during the milestone that raises it.

1. **Takeover safety (M2).** *Resolved (D8):* a watchdog helper process
   re-shows the icon layer after a crash, and DeskSpace re-hides it after an
   Explorer restart and on a health timer.
2. **Desktop layer vs blur (M2).** *Resolved (D8):* the desktop layer is
   desktop-hosted and spaces stay top-level and blurred. (A planned Win+D
   watcher to re-show the spaces was dropped; Show Desktop already leaves
   `WS_EX_TOOLWINDOW` windows alone. See D8 and the 2026-10-11 progress
   entry.)
3. **Undo model (M2).** Should the undo stack be single-level or
   multi-level? Should it cover file moves, which are destructive? Explorer
   can already undo the shell operations (paste, delete, rename) through
   `FOF_ALLOWUNDO`. Moves into the store are plain renames with no undo
   record.

Resolved:
- **Blur fallback (M1).** See D2.
- **Store layout (M2).** One shared folder, and deleting a space moves its
  stored files back to the desktop. See D3.
- **Takeover model (M2).** A `DesktopLayer` and a shared `Surface`. See D7.
- **Takeover safety and placement (M2).** Hosted layer, top-level spaces, and
  a watchdog helper. See D8.
- **Update policy (M8).** cargo-packager, opt-in, prompt then restart, and
  skip a version. See D6.
- **Takeover default, Ctrl+wheel, toolchain, right drag, hide vs layer.**
  See D9.

## 9. Risks

- **Takeover fragility.** Hiding and driving Explorer's icon layer is
  undocumented and changes between Windows versions. It needs a watchdog and a
  hard "restore desktop" path.
- **Data loss on relocation.** Moving real files is destructive. Mitigations:
  move into the store with undo, never overwrite, and snapshot the manifest
  before bulk operations.
- **Uninstall deleting user data.** The installer deletes its install root on
  uninstall. Mitigation (D6): the custom NSIS template installs into
  `%LocalAppData%\DeskSpace.App`, distinct from the data parent
  `%LocalAppData%\DeskSpace`, and a test asserts the config and storage paths
  are never inside the install root (`TASKS.md` E7).
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
