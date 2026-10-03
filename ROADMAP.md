# ROADMAP

Plan for bringing DeskSpace to feature parity with comparable tools.
See `CONTEXT.md` for the current architecture and `AGENTS.md` for engineering
rules. Milestones are ordered by dependency; feature ids (`F-xx`) are stable
references used across milestones and the parity checklist.

## 1. Vision

A native, lightweight, offline Windows desktop organizer that:
- owns the desktop icon layer and draws real desktop items itself,
- groups items into movable, resizable, lockable spaces,
- mirrors folders as live portals,
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
  - *Portals* are pure views over a real folder. They read contents live and
    never move, copy, or duplicate data, regardless of folder size.
  - *Normal spaces* hold individual items. Dropping an item into a normal space
    relocates that one item into a managed store with a move, not a copy (an
    atomic rename on the same volume, so no duplication and no extra disk use).
    The store root defaults to `%LOCALAPPDATA%\DeskSpace\storage\<space>` and is
    configurable. Relocation is protected by undo and a snapshot manifest.
- **D4. v0.2 scope.** Tray icon, right-click context menus, and a settings
  window are required in the first usable release.

## 3. Guiding principles

- Keep the domain pure: all new behavior is expressed as `Command` variants in
  `deskspace-core`; platform code never mutates the model directly.
- Keep rendering data-driven: the app builds a `Scene`; the renderer only draws.
  Swapping GDI for Direct2D must not change the app's contract.
- Make destructive operations recoverable: file moves and layout changes get an
  undo path and a snapshot/restore mechanism.
- Degrade gracefully: if takeover or composition fails, fall back to the
  current overlay rather than crashing.
- Ship small, verified increments: every milestone lands with tests, clippy
  clean, and a runnable binary.

## 4. Milestones

Progress is marked inline as **[done]**, **[partial]**, or unmarked (not
started).

### Progress log

- **Windows 11 look and control center (current).** Per-space windows with
  acrylic/blur, rounded corners, D2D/DirectWrite rendering, hover/selection;
  tray icon; space context menu; data-driven settings window with Appearance,
  General, and About pages and live preview; delete space; file drops onto
  spaces (as references); config v2 with migration; space-relative item
  positions (fixes items being left behind when a space moved). Covers most of
  M1 and parts of M0, M2, and M3.

### M0 - Foundations and guardrails

- CI: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`,
  `cargo test --workspace`, on Windows.
- `deny.toml` for licenses/advisories; `cargo-deny` in CI.
- **[done]** Wire the declared-but-unused events: the old `WindowEvent` enum
  was replaced by `Event`/`spaceEvent`, which only declares what is emitted;
  mouse tracking (`WM_MOUSELEAVE`) drives hover. Timers are added when a
  feature needs one (debounced autosave, M5 policies).
- **[done]** Work-area awareness: commands are bounded by the work area of
  the nearest monitor (`work_area_at`).
- Structured error boundaries: one `AppError` at the binary boundary; library
  crates keep `thiserror`.
- Autosave on change (debounced), not just on close.

Exit criteria: green CI, no unused event variants, work area drives bounds.

### M1 - Direct2D renderer and Windows 11 look

- **[done]** `render/d2d.rs` implementing the `spaceScene` contract.
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
  **[partial]** Shell drops onto a space are accepted (`WM_DROPFILES`) and add
  references; an OLE `IDropTarget` is needed for drop-effect feedback and
  drag-out.
- Drop files onto a normal space relocates them into the managed store by move
  (`D3`), with undo; portals are unaffected because they never touch the store.
- Update the item model and refresh after file operations.
- space chrome: resize handles, lock toggle, rename inline, delete with undo
  (`F-02`, `F-03`, `F-05`). **[partial]** Lock (menu + glyph) and delete (menu
  + confirmation, no undo) are done; resize and rename are not.
- **[done]** Basic right-click context menu for space actions (`F-14`
  partial). Item actions wait for real item support.
- Follow desktop item changes with a filesystem watcher and re-sync.

Exit criteria: with DeskSpace running, the Explorer icon layer is gone, items
are drawn and interactive, and dragging moves real files with undo.

### M3 - Control center: tray, context menu, settings

- System tray icon with menu: show/hide, pause takeover, open settings, snapshots
  (`F-16`). **[partial]** Settings, show/hide, new space, exit; takeover and
  snapshots do not exist yet.
- Full context menu system: create space/portal, arrange, sort, lock, roll up,
  appearance, remove (`F-14`). **[partial]** Create, delete, lock, roll up.
- Settings window (own HWND) with live-applied pages: General, Appearance,
  spaces, Portals, Hotkeys, Advanced, About (`F-17`). **[partial]** Data-driven
  form engine plus Appearance (live preview), General, and About pages with
  OK/Cancel/Apply.
- Persist all `WindowSettings` fields and wire them to behavior. **[partial]**
  `start_hidden` and `toggle_hotkey` are wired; `background_alpha` was
  superseded by `theme.tint_opacity` and removed; `always_on_desktop` and
  `snap_grid` remain.
- Hotkey editor and additional bindings (`F-18` partial). **[partial]** The
  toggle hotkey is editable as text with validation and conflict reporting.
- Per-space appearance overrides (tint, opacity) from the space menu.
- Dark-mode settings window and menus (needs owner-drawn controls or the
  undocumented uxtheme dark-mode APIs).

Exit criteria: every runtime setting is editable without hand-editing TOML, and
the tray menu can show/hide and open settings.

### M4 - Folder portals

- `spaceKind { Normal, Portal }` in the domain (`F-07`).
- Portal binds to a folder path; contents mirror the folder live.
- `ReadDirectoryChangesW` watcher drives incremental refresh.
- Sort rules: name, type, size, date, custom, folders-first (`F-08`).
- Show-as modes: grid, list, tiles, details; icon-size options (`F-09`).
- Portal-only actions: open folder, include/exclude patterns, "match folder
  view".

Exit criteria: editing a portal's backing folder updates the portal within
milliseconds, and sort/show modes persist.

### M5 - Visibility and flow

- Roll-up policy: manual, auto after idle, and on hover-out (`F-04` full).
- Auto-hide / hide-all: hide spaces and icons; reveal on desktop click, hotkey,
  or peek (`F-11`, `F-13`).
- Double-click empty desktop toggles icon visibility, matching spaces (`F-10`).
- Per-space roll-up button and title-bar affordances.
- Smooth show/hide/roll animations via DirectComposition.

Exit criteria: hide-all and peek are instant and reversible, and roll-up policy
is configurable per space.

### M6 - Multi-monitor, pages, snapshots

- Per-monitor overlays with independent work areas; spaces are assigned to and
  movable across monitors (`F-19`).
- Pages: multiple named space layouts per monitor with a page switcher (`F-12`).
- Snapshots: export/import full layout and managed-store manifest; restore
  (`F-20`).
- space-to-space snapping and alignment guides.

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

## 5. Release mapping

- **v0.2 "Take the desktop"**: M0, M1, M2, M3. The first usable release:
  D2D rendering with the Windows 11 look, real desktop takeover, tray +
  context menu + settings.
- **v0.3 "Portals"**: M4.
- **v0.4 "Flow"**: M5.
- **v0.5 "Rooms"**: M6, M7.
- **v1.0**: M8.

## 6. spaces parity checklist

| Id | Feature | Milestone | Status |
|----|---------|-----------|--------|
| F-01 | spaces / icon groups | M2 | Partial (references, no takeover) |
| F-02 | Create / rename / delete space | M2 | Create, delete done; rename open |
| F-03 | Resize space | M2 | Open |
| F-04 | Roll-up spaces | M2 (manual), M5 (policy) | Manual done |
| F-05 | Lock space | M2 | Done |
| F-06 | Drag icons in / out / between spaces | M2 | Drop-in done (references) |
| F-07 | Folder portals | M4 | Open |
| F-08 | Portal sort rules | M4 | Open |
| F-09 | Portal show-as / icon size | M4 | Open |
| F-10 | Hide all on desktop double-click | M5 | Open |
| F-11 | Auto-hide | M5 | Open |
| F-12 | Pages / virtual desktops | M6 | Open |
| F-13 | Quick peek / transparency toggle | M5 | Open |
| F-14 | Right-click context menu | M2 (basic), M3 (full) | Basic done |
| F-15 | Snap to grid / auto-arrange | Core | Done |
| F-16 | System tray | M3 | Done (takeover items pending) |
| F-17 | Settings window | M3 | Partial (3 of 7 pages) |
| F-18 | Hotkeys | M2 (toggle), M3 (editor) | Toggle + text editor done |
| F-19 | Multi-monitor | M6 | Partial (per-monitor work areas) |
| F-20 | Backup / restore / snapshots | M6 | Open |
| F-21 | Auto-consolidate new icons | M7 | Open |
| F-22 | Exclude rules / filters | M7 | Open |
| F-23 | Keyboard navigation | M7 | Open |
| F-24 | Appearance: color, translucency, blur | M1 (renderer), M3 (settings) | Done globally; per-space open |

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
4. Store layout (M2): one subfolder per space, and what happens to relocations
   when a space is renamed or deleted (restore items to the desktop vs keep in
   store)?

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
