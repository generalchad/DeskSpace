# ROADMAP

The plan for bringing DeskSpace to feature parity with comparable tools.
`CONTEXT.md` describes the current architecture and `ENGINEERING.md` the engineering
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
- The history of what landed up to v0.2.0 is in
  `archive/ROADMAP-progress-log.md`; read it only for background. A task id
  cited here (`E7`, `L2`, ...) is in `TASKS.md` while open and in
  `archive/TASKS-done.md` once finished.

## 1. Vision

A native, lightweight, offline Windows desktop organizer that:

- owns the desktop icon layer and draws real desktop items itself,
- groups items into movable, resizable, lockable spaces,
- mirrors folders as live lenses,
- hides and reveals everything quickly (folding, auto-hide, peek),
- looks and feels modern (per-pixel alpha, blur, animation),
- stays fully configurable from a tray app, context menus, and a settings
  window,
- and speaks the user's language.

## 2. Decisions locked

- **D1. Desktop takeover.** DeskSpace hides Explorer's `SHELLDLL_DefView` icon
  listview and draws the desktop items itself on a desktop-hosted window.
  Items are real filesystem entries, so moving an item into a space relocates
  the real file.
- **D2. Rendering backend.** Each space is its own window.
  - Content is drawn with Direct2D/DirectWrite into a premultiplied-alpha HWND
    target over a DWM-extended frame.
  - The blur comes from the accent policy (`SetWindowCompositionAttribute`).
    Corners and dark mode come from DWM attributes. The border is painted
    by the space itself (amended 2026-10-19, maintainer): DWM draws its
    border outside the window's pixels, so it could not fade, dissolve, or
    burn with the space in an animation (D12), and it had one thickness
    and color. Its color is white in dark mode and black in light mode,
    as Explorer's window borders are; the Windows accent is a choice of its
    own, so "Use Windows accent color" now colors only the selection
    (amended 2026-10-19, maintainer: the accent border looked off and is
    not how Explorer draws borders).
  - GDI is retired. DirectComposition waits for the animation work in M5.
  - The documented `DWMWA_SYSTEMBACKDROP_TYPE` was rejected: system backdrops
    only render on active windows, and spaces are never active.
  - Fallback: without the accent API, spaces keep a translucent tint with no
    blur. Rounded corners need Windows 11. Desktop-hosted spaces are opaque,
    because child windows get no DWM material.
- **D3. Item storage model.** There are two mechanisms, kept strictly
  separate:
  - *Lenses* (comparable tools call them "Folder Portals") are views over a
    real folder. They read its contents live and never move, copy, or
    duplicate data on their own, whatever the folder's size. Dropping files
    onto a lens is a move into the bound folder that the user starts. It uses
    the shell's `IFileOperation`, so the standard conflict dialogs appear and
    Explorer can undo it. Amended 2026-10-21: a "Copy here" drop copies,
    keep-in-place (Shift) is refused, and paste and new shortcuts land in
    the folder the same way. A lens item dropped on a normal space is an
    ordinary file there and moves into the store. A lens never binds the
    store or a folder inside it.
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
    - Deleting a space moves its stored files back to the desktop.
    - Removing a single item is a delete (amended 2026-10-13): a stored file
      goes to the Recycle Bin, as the Delete key and Explorer's Delete do, and
      an item listed in place (Shift-dropped, or on another drive) is only
      unlisted, so the user's original is never deleted. The one way to take
      an item out of a space onto the desktop is to drag it there (not while
      takeover is on; D7, amended 2026-10-17). The item
      menu's old "Remove from space" / "Move back to desktop" entry is gone.
  - *Deleting a lens never deletes its folder.* A lens is only a view, so
    deleting the lens removes the view and leaves the bound folder and every
    file in it untouched. Removing a single item from a lens is a delete of
    that file in the folder (through the shell, to the Recycle Bin), the same
    as deleting it in Explorer.
- **D4. v0.2 scope.** The first usable release must have a tray icon,
  right-click context menus, and a settings window.
- **D5. Naming.** Containers are called *spaces* in the UI, the docs, and the
  code (`Space`, `SpaceId`). The folder-mirror feature is called a *lens* in
  the UI and docs. In code it is a space whose source is a folder
  (`SpaceSource::Folder`), which keeps the user-facing name out of the model.
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
    and needed an app-side startup hook; see the archived progress log
    (2026-10-12).
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
    traffic happens unless the user starts it or opts in; the icon
    download (D16) asks first.
  - *Apply:* prompt, then restart. The prompt shows the version and notes
    and offers Update / Later / Skip this version. Update downloads,
    installs, and restarts DeskSpace with the same config. Later asks again
    at the next check. A skipped version is stored and not offered again.
    An install that cannot start never leaves DeskSpace closed: it starts
    again on the current version and says why (2026-10-18).
  - *Automatic install (2026-10-16):* an opt-in "Install updates
    automatically" toggle under the check toggle, off by default and only for
    the installed build. With it on, an automatic check downloads a new,
    unskipped version without asking and installs it silently after 10
    minutes without input (not during a full-screen app or presentation, and
    not while the settings window or a rename is open). The tray offers to
    restart sooner. Every install quits through the normal teardown first,
    so the desktop is restored before the installer runs.
  - *Channels within the updater:* stable only. An opt-in pre-release
    channel (`-alpha.N`, `-beta.N`, `-rc.N`) is deferred, because GitHub's
    "latest" feed never serves a pre-release.
  - *Portable zip:* the same opt-in check, but notify only. It offers to open
    the release page and never replaces its own files.
  - *Signing:* SignPath Foundation (free for open source), wired into
    `release.yml` through `cargo-packager`'s `windows.signCommand`. Signing
    ships before or together with auto-update. Amended 2026-10-31
    (maintainer): the update feed turns on in v0.3.0, ahead of signing. The
    minisign signature already proves an update is ours, and waiting would
    tie the release to SignPath's approval. Installers show SmartScreen's
    warning until R10 lands.

- **D7. Desktop takeover model (2026-10-10).**
  - The desktop's loose icons - the Desktop-folder files that are not in any
    space - are their own model type, `DesktopLayer`, not a `Space`. It holds
    items with free positions in virtual-desktop coordinates, one entry per
    loose file, and no chrome (no title bar, border, lock, resize, fold, or
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
  - *Amended 2026-10-17 (maintainer):* a drop on the taken-over desktop
    (the layer) opens a new space holding what was dropped: files from
    Explorer, Start-menu and taskbar apps (as shortcuts), and items dragged
    out of another space alike, with the drop's usual modifiers and right-drop
    menu. The space opens at the drop point, nudged so it keeps one gutter
    from every space and from the work area's edges, the taskbar included
    (`placement::drop_space_origin`); it has the configured new-space title,
    size, and lock. So while takeover is on, an item leaves its space for the
    desktop only when the space is deleted. Before this the layer refused
    every drop. Later, an option may allow loose items on a taken-over
    desktop (DeskSpace drawing its own desktop).

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
  - *Hiding Explorer's icons:* hide every `SysListView32` under a
    `SHELLDLL_DefView` in the desktop shell's window subtree (the view may be
    nested, not just a direct child) and restore it on a clean
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
    own `Custom` look keeps its size either way. Done (`TASKS.md` F13).
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

- **D10. New space placement (2026-10-17).** A new space used to open at
  the click or the pointer, so it often landed on top of another space.
  - Every way to create one (space menu, tray, desktop menu, hotkey) now
    uses the click or the pointer only to pick the monitor. The space goes
    one gutter to the right of that monitor's top row of spaces, its top in
    line with the row's. A full row gives way to the next one down, then to
    a new row under all of them; an empty monitor starts one gutter in from
    its top-left corner.
  - The gutter is the median gap between neighbouring spaces there (side by
    side first, then stacked), so it matches an even layout and takes the
    typical value of an uneven one. A gap the new space would fit in counts
    as open desktop, not a gutter. With nothing to measure, it is
    `DEFAULT_SPACE_GAP`, scaled for the monitor's DPI.
  - A slot must keep half a gutter clear of every space. When none does,
    the space opens at the click or the pointer, as before.
  - Code: `deskspace-core/src/placement.rs`.

- **D11. Undo (2026-10-17).** Resolves open question 3.
  - *Depth:* multi-level, with redo. One user action is one entry (a drag of
    five files is one), labelled for the menus ("Move 3 items"). The stack
    keeps about 50 entries, and a new action clears the redo side.
  - *Session only:* the history lives in the app, like scroll state, and is
    cleared on exit. Nothing is added to the config.
  - *Scope:* every change DeskSpace makes through a user action: arranging,
    moving, resizing, sorting, locking, folding, and renaming spaces;
    creating and deleting spaces; moving items between spaces; and the
    files behind them. Files dropped into the store go back where they came
    from. Files a space's deletion returned to the desktop go back into the
    store. Copies, pastes, and new shortcuts go to the Recycle Bin. Shell
    renames are renamed back. Items deleted to the Recycle Bin are restored
    from it. Not covered: Shift+Delete (nothing to restore), settings-window
    edits (Cancel undoes those), positions on the taken-over desktop (the
    shell owns them), and drags out to Explorer (Explorer's own undo has
    them).
  - *Never clobber:* an entry stores, for each space it changed, the space
    before and after, and the file changes it made. It is undone only while
    those spaces still equal its after state, and every file is still where
    the action left it with its old place free; redo checks the before
    state the same way. Anything else changed them since (a prune, a look
    edit, a re-flow that moved items, a file moved outside DeskSpace), so
    the entry is stale: it is dropped with a message naming it, and nothing
    is changed. All checks run before the first file is touched.
  - *Model:* a restore is a `Command` that puts back exactly the stored
    spaces, so the model still changes only through `Command`s. Ids are
    never reused within a session, so a restored space or item cannot
    collide with a newer one.
  - *Keys and menus:* Ctrl+Z and Ctrl+Y act while a space is current, the
    rule Ctrl+V follows; otherwise they still reach Explorer. The space menu
    and the tray offer Undo and Redo, named for the action ("Undo Move 3
    items").
  - Work items: `TASKS.md` section U.

- **D12. Animations (2026-10-17).** Decides F-33 and M5 item 5.
  - *Kinds:* three moments animate, each its own kind with its own
    settings (amended 2026-10-18, maintainer: Hide and show and New space
    merged into Startup). The desktop layer never animates.
    - **Startup:** spaces coming onto the screen and leaving it. Its In
      plays when DeskSpace starts, when the spaces are shown (tray, space
      menu, hotkey, desktop verb), and when a space comes into being (a
      menu, the hotkey, a drop, or undo); its Out when DeskSpace exits
      (D13) and when the spaces are hidden. Only startup and exit stagger:
      all at once, or one space after another (startup in reading order:
      top to bottom, left to right where tops are level; exit in reverse)
      with a set delay, 50 ms by default. Showing, hiding, and a new space
      move every space together. "Start with spaces hidden" plays nothing.
      M5's hide-all, auto-hide, and peek (F-11, F-13) reuse it when they
      land.
    - **Delete:** a space going away, by delete or undo. The model changes
      at once; only the window stays for the animation. Its default is
      "Same as startup", played as its Out.
    - **Fold:** a space folding to its title bar and unfolding (until now
      called roll-up; see below). Always a roll.
  - *Effects:* one shared list, so a new effect is one row and every kind
    gets it. Each effect has an "in" and an "out"; the out is the in played
    backwards. None, Roll (into the top edge and back down), Fade,
    Pixelate, Gradient (a soft edge sweeping down from the title bar),
    Dissolve, Fly (to and from the nearest edge of the space's monitor's
    work area, clipped to that work area so it never shows on another
    monitor or over the taskbar), CRT (a bright line that opens to the
    full space, and closes to a line, then a dot), and Burn (amended
    2026-10-18: the space burns up like a sheet of paper, from the bottom
    up along a ragged front, charring ahead of a glowing edge, with flames
    licking up around its frame; the flames rise past the space, so they
    are drawn on a click-through overlay window above it, since the
    space's own window carries the blur over its whole rect; amended
    2026-10-22, maintainer: the fire spreads unevenly, from the bottom
    and a few spots, climbing faster than it spreads, with embers
    smouldering in the char, warm light ahead of the edge, turbulent
    flames, and ash lifting off; it looks the same at every DPI and
    differs from play to play; `TASKS.md` T24-T33). Black hole (the space swallowed by a hole that
    opens at its own center) is planned after composition (T14). It
    needs the composition path; on Windows 10 and the accent path it
    plays Fade instead (amended 2026-10-29, maintainer). Also
    planned (amended 2026-10-18, maintainer; `TASKS.md` T19-T21):
    - **Bounce:** the space drops in from beyond an edge of its monitor's
      work area (Automatic is the top), lands on its place, and settles in
      a few shrinking bounces; out, it plays that backwards. It moves the
      window's rect, as Fly does, so the blur follows it.
    - **Scroll:** the space rolls up like a paper scroll. The body curls
      into a shaded cylinder, with the contents wrapped around it, that
      travels toward the edge (Automatic is the top) and grows as it
      winds, so it reads as 3D; in, it unrolls. The rect shrinks as Roll's
      does, so the blur follows it.
    - **Magic lamp:** a genie returning to its lamp. A spout forms on the
      space's own title, and the body narrows along curved sides and
      pours up into it (amended 2026-10-29, maintainer). In Fold, the
      spout then settles back into the title, which stays. On Delete and
      Startup's Out (exit and hide), the title, now holding the space,
      flies into DeskSpace's tray icon. If the icon is hidden in the
      overflow or on another monitor, it flies to the notification area
      on the primary monitor, else to the middle of the bottom edge of the
      space's work area. Startup's In plays it backwards, out of the tray.
      It warps the shape, which the rect-wide blur cannot follow, so it is
      drawn on a click-through overlay (as Burn's flames are) while the
      space's own window is hidden (T21).

    A kind can limit the list: Fold offers only None, Roll, Scroll, and
    Magic lamp.
  - *Controls:* each kind has the common ones: its effect and a duration
    (100 to 1000 ms). An effect adds only the options it uses: Detail (the
    block size, grain, or edge width) for Pixelate, Dissolve, and Burn, and
    Direction (which edge; Automatic is the effect's own: the top for Roll
    and Gradient, the nearest for Fly) for Roll, Gradient, and Fly. A control the
    chosen effect does not use is greyed, so each page keeps its shape. The
    controls live in a Global-only Animations group of pages, one per kind,
    so no page grows too tall for a 1080-pixel display. Each effect sets
    its own pace (amended 2026-10-22, maintainer): its row stretches the
    chosen duration and picks its progress curve, so an effect that needs
    longer, or a different rhythm, is one value in its row, not a special
    case. Burn plays at twice the chosen duration on a curve that catches
    slowly and then races, so 250 ms burns in half a second. This changes
    how Burn plays for anyone already using it, on purpose (section 3,
    "never break users"). Defaults: Roll at
    250 ms for Startup and Fold; startup one after another at 50 ms;
    Delete the same as Startup. An upgrade starts
    animating; this change is on purpose (section 3, "never break users").
  - *Windows' setting:* when Windows' "Animation effects" is off
    (`SPI_GETCLIENTAREAANIMATION`), nothing animates, the page's controls
    are greyed, and a note says why, with a button that opens the Windows
    page holding the switch (`ms-settings:easeofaccess-visualeffects` on
    Windows 11, `ms-settings:easeofaccess-display` on Windows 10). The
    page follows a change made in Windows while it is open.
  - *Reversing:* any animation can turn around at any frame and play back
    from exactly where it is. An action that interrupts an animation
    reverses it rather than starting another effect over it: hiding a
    space that is still flying in flies it back out. A space that is
    hiding ignores drops; other input works as usual.
  - *The app decides, the platform plays:* the app turns the settings and
    Windows' switch into a `Play` (effect, in or out, duration, delay,
    detail, direction) for each window and passes it with the request
    (show, hide, create, destroy, fold). The platform runs one frame per
    display refresh (amended 2026-10-19: a window timer drifted against
    the refresh and stuttered) from pure frame math in `render/`, so
    there is still no per-frame callback into the app.
  - *Frames and overlays (amended 2026-10-19, maintainer):* a frame says
    how the space's contents are drawn as one `Content` (whole,
    pixelated, lit by CRT's line), never one frame field per effect;
    Scroll's strips (T20) are a new variant. An effect that draws beyond
    the space's own window does it on an `Overlay`, chosen per effect
    with a match that holds every effect to an answer: Burn's flames
    today, the magic lamp (T21) and the black hole's disk (T14) later.
    Each is an extension point the compiler points at, so a new effect
    never adds a branch on one effect's name.
  - *The blur:* the Acrylic and Blur material is drawn by DWM behind the
    whole window (the accent policy, D2) and can only be on or off. Roll,
    Fly, and CRT's opening change the window's rect, so the blur follows
    them exactly. The other effects change pixels: the painter applies them
    to the tint and the contents, the same on every path. For the blur to
    follow those too, spaces move to the Windows composition engine
    (`Windows.UI.Composition`), where the blur is a host backdrop brush
    that takes the same opacity and masks (T12, T13). The painter draws
    each see-through frame's opacity and mask a second time, into the
    blur's mask, which the backdrop shows through D2D's AlphaMask effect
    (a `CompositionMaskBrush` refuses a host backdrop); Pixelate's blocks
    are not masked, so the blur only fades with them. Wherever composition
    is not available (Windows 10, or when it fails to start), spaces keep
    the accent blur: it appears at the start of an "in" and goes at the end
    of an "out", in one step. The host backdrop
    ignores Windows' "Transparency effects" switch (T1), so with the switch
    off when DeskSpace starts, spaces keep the accent blur too, and a
    change while it runs applies at the next start (amended 2026-10-18,
    maintainer). Clear and Solid
    spaces have no blur, so they animate fully either way.
  - *Fold, not roll-up:* "roll up" read like a kind of hiding, so a space
    collapsed to its title bar is now **folded**, and the menu says "Fold"
    and "Unfold". The config key stays `rolled_up`, so every config still
    loads, and an older DeskSpace still reads a newer file.
  - Work items: `TASKS.md` section T.

- **D13. Graceful startup and exit (2026-10-18).** DeskSpace comes onto
  the screen and leaves it the way a space does, with the Startup
  animation (D12).
  - *Startup:* already so (T10): every window is created hidden, gets its
    scene, and then plays Startup's In, staggered; with takeover on,
    Explorer's icons are hidden and the desktop layer drawn first.
  - *Exit:* Exit from the tray or a space menu, and the quiet-moment
    update install, play Startup's Out on every shown space, staggered in
    reverse reading order, and only then tear down. Explorer's icons come
    back after the spaces have left, as the teardown already restores them
    after destroying every window; the watchdog still covers a crash.
  - *Limits:* the whole exit takes at most `EXIT_LIMIT` (2 s): the stagger
    shrinks to fit, and a timer tears down at the limit whatever is still
    playing. A second Exit while leaving quits at once. Signing out or
    shutting down Windows skips the animation (Windows allows an app only
    seconds there), and so do Ctrl+C in a console and a forced end, which
    nothing can animate.
  - *The app decides, the platform plays:* the app builds each space's
    `Play` and calls `Ui::quit_after(plays, limit)`; the platform plays
    them and posts the usual close when the last one ends or the limit
    passes. `Event::CloseRequested`, the save, and the teardown are
    unchanged.
  - Work items: `TASKS.md` T16.

- **D14. The debug window (2026-10-18, maintainer).** Decides F-61.
  `--debug` opens a DeskSpace Debug window: a live log console plus tools
  for setting up, inspecting, and breaking the app on purpose while
  developing and checking the section V rows.
  - *Who and where:* a hidden flag, like `--on-top`, accepted by every
    build. Release builds have no console, so this is their only way to
    see the log, which also helps a user attach one to a bug report. It is
    left out of `--help` and the README.
  - *The window:* its own modeless, resizable top-level window, not a
    settings page, because its actions commit at once and its console
    must stay open while the spaces and the settings window are used. A
    section list on the left, the chosen section's controls to its right,
    and the console under both. It follows the dark or light setting as
    the dialogs do. Closing it does not quit DeskSpace; the tray's
    "Debug…" entry (only with `--debug`) opens it again.
  - *Data-driven like settings:* the app describes the window as a
    `DebugPanel` (sections of buttons, toggles, choices, readouts, and
    tables), the platform builds it, and input comes back as
    `Event::Debug`. The panel's shape is fixed for a session; only values
    change, and the app pushes the panel only when they did. A control is
    one id, one table row, and one match arm.
  - *The console:* a `tracing` layer beside the stderr one, installed only
    with `--debug`, records into a bounded ring (10,000 lines, oldest
    dropped) from the moment `main` starts, so startup is in it. It has its
    own level, changed live from the console, and leaves stderr and
    `RUST_LOG` alone. The window shows it with a level and text filter,
    pause, auto-scroll, clear, copy, and save. A panic is logged at error
    in every build, with or without `--debug`.
  - *One way to change things:* every action calls what the menus,
    settings, and drops already call. The debug window never gets its own
    path to the model or the files. Its toggles are session-only and never
    saved; its actions record no undo entry (D11), because they are tools,
    not user edits. The animation settings are the one exception (amended
    2026-10-18, maintainer): an "Animation settings" box in Animations
    shows the kind chosen in its Kind box (Startup, Delete, or Fold), and
    an edit there is the settings window's own edit, saved at once, so an
    effect tried there stays. An open settings window shows it, and its
    Cancel keeps it. The box keeps every row for every kind and greys what
    the kind or its effect does not use, as the settings pages do, so the
    window keeps its shape. The shadow settings are a second exception
    (D15).
  - *Destructive actions* (Delete all spaces, Delete stress spaces, Crash
    now, read-only config) always ask first. The question names the config file and, when no
    `--config` was given, warns that this is the real configuration.
  - *Delete all spaces* sends every stored file to the Recycle Bin, unlike
    Delete space, which returns them to the desktop (D3): test files would
    otherwise pile up on the real desktop on every cycle, even with a
    scratch `--config`. Items listed in place are only unlisted, as for a
    single delete. A space whose files cannot be recycled stays, as Delete
    space keeps one whose files cannot be returned. *Delete stress spaces*
    does the same for the spaces "Create a stress space" made, found by the
    title it gave them ("Test: Stress 50"); a renamed one is left alone
    (amended 2026-10-18, maintainer: deleting a stress space by hand had
    put its shortcuts on the real desktop).
  - *Create test spaces* makes four ordinary spaces (Apps, Tools, Places,
    Web) through the New space path, each holding shortcuts made through
    the one shortcut writer, covering a program, a program with
    arguments, a folder, and a web address. Nothing marks them as test
    spaces.
  - *The other sections:* Spaces (fold, lock, refresh, prune, make and
    delete stress spaces), Config (dump, open folders), Diagnostics (versions, paths,
    resources, copy a report), Windows (on-top, highlight, window and
    monitor tables), System (what Windows reports), Animations (slow
    motion, ignore Windows' switch, play an effect on a space, and the
    chosen kind's settings), Takeover
    (restore or re-hide Explorer's icons, re-sync, watchdog), and
    Simulate (theme change, Explorer restart, a read-only config, a
    crash).
  - Work items: `TASKS.md` section X.

- **D15. Drawn shadows (2026-10-19, maintainer).** Moving spaces to
  composition (D12) took away the Windows 11 window shadow: it comes with
  DWM's rounded corners, and it could not fade with an animation, so
  composition spaces turned DWM's corners off. DeskSpace draws its own.
  - *Where:* each space gets a shadow window of its own. It is layered,
    click-through, and never activated, kept directly below its space and
    above the desktop layer, and sized to the space plus the shadow's
    reach, so clicks pass through it. Making the space window bigger was
    rejected: the margin would take clicks from the desktop and other
    apps, and every rect and hit test would shift.
  - *Which spaces:* the ones DWM casts no shadow for. That means
    composition spaces, and accent spaces whose corners DWM does not round.
    An accent space DWM rounds keeps the system shadow whatever the
    setting. Clear spaces have no edge to cast from. Desktop-hosted spaces
    and the desktop layer get no shadow either.
  - *Settings:* three look settings on the Appearance page, so a space can
    have its own. "Show shadow" is on by default, so an upgrade gets back
    the shadow composition took away. That is a change on purpose
    (section 3). "Shadow size" is how far the blur reaches, in pixels at
    96 DPI, and "Shadow opacity" how strongly the black is laid down.
    (Amended 2026-10-27, maintainer: the setting is the black's own alpha,
    not the darkest pixel. The blur leaves its peak at about three quarters
    of it below the space and half beside it; this is what shipped and what
    O3 checks.)
  - *Animations:* the shadow follows the window's rect (Roll, Fly, CRT).
    It fades with any frame that leaves the window see-through.
  - *Debug window (amends D14):* a "Shadow settings" box edits the three
    settings as the settings window does, and saves them at once, as the
    animation settings box does. Two session-only toggles outline every
    shadow window and hide every shadow, and the Windows table lists the
    shadow windows.
- **D16. Space icons (2026-10-18, maintainer).** Decides F-62. A space can
  show an icon to the left of its title, picked from its name or chosen by
  the user.
  - *Library:* Fluent UI System Icons (Microsoft, MIT), the one-color set
    Windows 11's own icons come from, drawn in the title's color so it
    follows the theme. The Segoe Fluent Icons font was ruled out: it is
    only on Windows 11, has no keywords, and its terms do not cover
    shipping it. Sets are read in the Iconify JSON format, which every
    candidate set is published in, so more sets can become a user's choice
    later (the colorful Fluent System Color Icons and Fluent Emoji Flat
    first, then Lucide, Phosphor, Tabler, or Material Symbols) with no new
    loader. Each set's license and author show on Settings > About, and
    `NOTICE` names the sets DeskSpace offers.
  - *Packs (amended 2026-10-18, maintainer: scaffold for more packs soon):*
    every set is one row of a table (prefix, upstream package and version,
    the size and style kept, a revision, its curated words, its credits),
    and everything else (the cache, the download, the picker, the saved
    names) is keyed by the set's prefix. Adding a pack is a row plus its
    published file; no code names Fluent outside its row. The picker shows
    a choice of set once there are two.
  - *Delivery:* downloaded on first use, not bundled. A script trims the
    pinned upstream set to one size and style and publishes it as an asset
    of DeskSpace's own GitHub release, signed with the updater's minisign
    key (D6). DeskSpace fetches it the first time an icon is needed (a new
    space with an automatic icon, or the picker opening), verifies it, and
    keeps it under `%LocalAppData%`; a release that needs a newer set
    fetches it once. Offline or on a failed check, spaces show no icon and
    the picker says why; nothing else waits on it.
  - *Consent (amended 2026-10-18, maintainer):* D6's promise holds: no
    network traffic unless the user starts it or opts in. The first time
    an icon is needed, DeskSpace asks once whether to download the icons
    from GitHub, and saves the answer (allowed or declined); the General
    page can change it. The first-run seed space starts with no icon, so
    the very first launch never asks. Declined, spaces show no icon and
    the picker says so. A build with no signing key cannot verify a set, so
    it does not ask, saves no answer, and greys the General page's choice
    (amended 2026-10-31, maintainer).
  - *Matching:* pure and unit-tested, outside the platform. The space's
    title is split into words and scored against a curated table of
    English words to icon names ("games", "work", "photos", "school"), then
    against the set's own names and aliases. The best score wins; ties
    break by a fixed order; below a threshold, no icon. Other languages
    follow M9.
  - *Model:* each space stores its icon as automatic, none, or a chosen
    set and name, a `#[serde(default)]` field changed through a `Command`.
    Icon names are saved, so a set's trimmed copy never renames or drops
    one it has shipped. The match runs again when an automatic space is
    renamed; a chosen icon never changes by itself.
  - *Existing spaces:* default to no icon, so nothing changes under current
    users. New spaces start automatic; the New spaces page gets an "Icon"
    choice (Automatic or None).
  - *Changing it:* while the title is being renamed, the icon becomes a
    button that opens an icon picker: a search box, a grid, and Automatic
    and None. The picker is its own activated window, as the color picker
    is, since spaces never take keys. A space's Settings page has the same
    picker beside its title.
  - *Interaction (amended 2026-10-18, maintainer):* changing the icon is
    part of renaming. Outside a rename the icon is display only: no hover,
    no cursor change, no click, and no menu entry. Once a rename starts,
    the icon (or a placeholder, for a space with none) becomes a button,
    and one click opens the picker; the rename stays open behind it. The
    picker's choice applies at once.
  - *Settings (amended 2026-10-18, maintainer):* a space's header pairs its
    icon with its name, as the space does on the desktop, and the scopes
    pane shows each space's icon beside its name. Double-clicking the
    header's name renames the space inline; during that rename the icon
    becomes the same button. Both apply at once, as a desktop rename does,
    so the window's Cancel does not undo them.
- **D17. Views, sort rules, and lens settings (2026-10-21, maintainer).**
  Decides M4 phase 4 (F-08, F-09, F-17's Lenses page, F-31's kept rule).
  - *Telling a lens from a space:* each row of the settings window's
    scopes pane gets a glyph for its kind (Global, space, lens), from
    Segoe Fluent Icons (Segoe MDL2 Assets on Windows 10), so it follows
    DPI and dark mode. On the desktop, a lens's title bar shows its
    folder's own shell icon, so a lens on Downloads shows the Downloads
    icon. A normal space's title bar is unchanged.
  - *Views belong to every space,* not only lenses. Normal-space items
    carry the same facts (size, type, date), and a lens-only path in the
    renderer would be the special case rule 2 forbids. The views are
    Explorer's eight (Extra large, Large, Medium, and Small icons, List,
    Details, Tiles, Content), with Explorer's Ctrl+Shift+1..8. A view is
    per space, as Explorer keeps one per folder, so it lives in `Space`,
    not `SpaceLook`, as tabs do (J1). The default view, "Settings icon
    size", is today's: the look's icon size, Ctrl+wheel, and Match
    Explorer (D9) as before. The four icon views set a fixed size for that
    space (256, 96, 48, and 16 px at 96 DPI), which Ctrl+wheel leaves
    alone, as it leaves a customized Custom look alone. List, Details,
    Tiles, and Content place items by their order and never write
    `Item.position`, so switching back to an icon view restores a hand
    arrangement.
  - *One sort rule for both kinds:* `SortRule { key, descending,
    folders_first }`. A space keeps its last rule, and "Keep sorted"
    decides whether it holds as items come and go. A lens is always kept
    sorted, since its items have no saved positions. Dragging to rearrange
    a kept-sorted space is refused, as Explorer refuses it in a sorted
    folder. Sort by ▸ takes Explorer's folder shape: the keys as radio
    items, then Ascending and Descending, then Folders first and Keep
    sorted (checked and grey on a lens).
  - *The status bar* is per-space chrome too, not a look setting. A look
    setting cannot be on for lenses and off for spaces while both follow
    Global. It shows Explorer's segments ("12 items", "3 items
    selected", and the selected files' size) on the left. On the right
    it has Explorer's two view buttons: Details (Ctrl+Shift+6), "Displays
    information about each item in the window", and Large icons
    (Ctrl+Shift+2), "Displays items by using large thumbnails". It is off
    for new spaces and on for new lenses, from the Lenses defaults page.
    Spaces that exist before it lands keep it off.
  - *A lens's title* follows its label setting: the folder's name (the
    default for a new lens), its path from the user's profile folder
    (`C:\Users\me\Documents\Work` shows as `Documents\Work`, and a folder
    outside the profile shows its full path), the full path, or custom
    text. Renaming the title by hand (F2, or the Lens page) makes it
    custom. A lens that predates the setting loads as custom, so its
    title does not change under the user.
  - *Settings pages:* every space lists a View page (the view, the status
    bar, and the sort rule), always enabled like Shortcuts, because
    views are not part of the look. A lens also lists a Lens page (its
    folder, its title, and hidden files). Global gains a Lenses page that
    holds the defaults for new lenses. View and Lens edits preview, and
    Cancel undoes them. Change folder… commits at once, as the menu
    does.
  - Lenses are unreleased (2026-10-21), so `FolderView`'s sort fields may
    move into `SortRule` without a migration.
  - Work items: `TASKS.md` section W, phase 4 (W8-W23).

- **D18. Lens and view extras, M4 phase 5 (2026-10-21, maintainer).**
  Builds on D17. It was planned to ship after v0.3 so it would never hold
  up the lens release; it landed first, so v0.3.0 includes it (amended
  2026-10-31, maintainer).
  - *Browsing into subfolders:* double-clicking a real folder in a lens
    shows that folder in place, as Explorer does. A shortcut to a folder
    still opens as today, because a shortcut can point anywhere. A Back
    button in the title bar, Alt+Left, Backspace, Alt+Up, and the mouse's
    back button go to the parent. A lens never goes above its own folder.
    The folder shown is session state: a restart, or Change folder…,
    returns to the lens's folder. The title bar shows `Root › Sub`; the
    scopes pane and the settings window keep the lens's own title.
    Every file operation (drop, paste, new shortcut, delete) acts on the
    folder shown, and the storage-folder rule (D3) applies to every folder
    shown, through its final path, so a junction cannot lead into the
    store.
  - *Group by,* for every space, with Explorer's keys and group names.
    A grouped space places items by group and order, and dragging to
    rearrange is refused, as for Keep sorted. Folded groups are session
    state. An item in a folded group cannot be selected: Ctrl+A and the
    marquee skip it, and folding a group drops its items from the
    selection, so a delete never acts on files the user cannot see.
  - *Ctrl+wheel through the views:* on a space set to one of Explorer's
    views, Ctrl+wheel and Ctrl+Plus/Minus step through them in the View
    menu's order and stop at the ends; Ctrl+0 returns to "Settings icon
    size". The zoom pop-up does not show for a view step. A space on the
    default view zooms exactly as D9 says.
  - *Lens filters:* "Show only" and "Hide" lists of `*` and `?` wildcards,
    separated by `;`, matched without case against names only. "Show
    only" applies to files, so folders stay browsable. "Hide" applies to
    both. No regular expressions: a hand-written matcher is linear in time
    and adds no dependency. Pattern count and length are capped, and
    clamped when the config loads. A lens whose filters hide everything
    says so in its placeholder.
  - *File name extensions* per space: Automatic (today's rule: Explorer's
    setting under Match Explorer, else hidden), Show, or Hide.
  - *Details columns:* Name is always first and always shown. Explorer's
    header menu chooses the others; dragging a divider resizes and
    dragging a header reorders. Widths and order are kept per space.
    Date created joins as a column and a sort key. It comes from the same
    listing record as the other dates, at no extra IO.
  - *Compact view* per space, as Explorer's. It tightens the views that
    place items by order. The icon views keep the Layout page's spacing,
    so there is one spacing control per view.
  - *The lens's path as a tip* on hovering its title, read from the
    model and never from the disk.
  - *The default view for new spaces* goes on the New spaces page; its
    default is today's view.
  - *The rest of F-40:* Explorer's desktop offers only icon views, which
    the default view already follows. A check confirms this, then F-40
    closes.
  - Work items: `TASKS.md` section W, phase 5 (W24-W49).

- **D19. A lens follows its folder (2026-10-29, maintainer).** Resolves
  open question 5 and decides F-67.
  - When a lens's folder is renamed or moved, the lens follows it, but
    only on the same volume, where a move is a rename the watch's handle
    follows. It never follows into the Recycle Bin (deleting in Explorer
    moves a folder there) or into DeskSpace's storage folder (D3). In
    every other case it says "Folder not available", as today.
  - Following is one undo entry, as Change folder… is. Ctrl+Z points the
    lens back at the old path.
  - The lens says where it went. A bar over its content reads
    "Folder moved:" and the new path for about 3 s, and a log line records the
    move. The bar is a general space notice, drawn and timed like the zoom
    pop-up, so later messages reuse it.
  - A lens browsed into a subfolder (D18) whose subfolder moves returns
    to its own folder.
  - F2 on a lens's title stays a nickname, never a folder rename (W46).
    Renaming the folder from DeskSpace is a later, explicit
    "Rename folder…" in the lens menu.
  - Work items: `TASKS.md` W45.

- **D20. Translation format (2026-10-29, maintainer).** Resolves open
  question 4 for F-60. Messages use Fluent (`fluent-bundle`), so plurals,
  named placeholders, and grammar live in the translator's file. Catalogs
  are embedded in the executable, so the portable zip stays one file. A
  test checks that every key the code uses has English text. The rest of
  the design is R18 in `TASKS.md`.

- **D21. Measuring guides (2026-10-29, maintainer).** Decides F-66 (M6
  item 4). The guides come with snapping (F-35) and share its engine:
  they show while a space is moved or resized with snapping on. One
  setting, "Show measuring guides", lives on the General page beside
  "Snap resized spaces".

- **D22. Resizing follows the view's grid (2026-10-31, maintainer).**
  Resolves the rest of F-03's whole-cell fit for the views added in D17.
  - "Snap resized spaces to whole columns and rows" keeps its name, field
    (`GridSpec::fit_to_grid`), and default, but fits to the current view's
    cells: icon cells for the icon views; Small icons, List, and Tiles' own
    cells; Details and Content rows only, with the width left free at the
    `snap` step. Details' header and a space's status bar count as chrome.
  - The resize dead zone is half an item's shorter side (a row view uses its
    row height), never less than half `snap`, so a wide List cell does not
    make the grab sluggish.
  - Work items: `TASKS.md` C11.

- **D23. Fit to contents on an item's menu (2026-10-31, maintainer).**
  - An item's right-click menu in a space offers "Fit to contents" beside
    DeskSpace's Rename, before Explorer's own entries. It fits the space
    that holds the item, as the space menu's entry does, and is greyed when
    the space is locked or already fits. A desktop item's menu does not
    offer it.
  - Fit to contents never makes a space larger than the work area of its
    monitor; an icon view that is capped scrolls. This closes the gap where
    only the order-placed views were capped.
  - Work items: `TASKS.md` C12.

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
  a later cleanup (`TASKS.md` section K). ENGINEERING.md "Design priorities" has
  the details.

## 4. Where we are

**Released:** v0.2.0 (2026-10-13, the first public release), v0.2.1 (in-app
update checking, F-45), v0.2.2 (adding shortcuts to spaces, F-50/F-53), v0.2.3
(the space menu's Add files, F-55, and the right-button drag, F-36), and
v0.2.4 (animations, F-33, and the debug window, F-61). Takeover ships opt-in
(D8, D9).

**Done:** M0 (foundations), M1 (renderer and Windows 11 look), and M4
(lenses, phases 0-5).

**In progress:** M2 (only the stored-file watch remains), M3 (desktop verbs,
F-52, and entries that wait on later milestones or design), and M5's space
icons (the title icon and the download consent are built; the picker follows
v0.3.0).
Undo (D11), drawn shadows (D15), and lenses (M4 phases 1-5) have landed since
v0.2.4 and ship in the next release. Section 5 lists what each has done.

**Next, in order** (decided 2026-10-31, maintainer: performance first, then
the release path; amended the same day: the icon picker moves after v0.3.0,
and the update feed no longer waits for signing):

1. **[done] Lens performance** (`TASKS.md` B37, archived): a big lens's list
   is built off the UI thread, and swapping in 50,000 items takes about 0.3 ms.
2. **The signing key and the icon set.** The maintainer's minisign key
   (`RELEASING.md`), then the "Publish icon set" workflow (S5). The key turns
   on the update feed (D6), and the set makes the automatic title icons of
   new spaces work, which a keyed build otherwise offers and fails to fetch.
3. **On-screen verification pass** (`TASKS.md` section V, R7), lens rows
   first, starting with the checks a script can make (`TASKS.md` section O),
   once `smoke.ps1`'s capture failure is understood.
   Confirm the built-but-unchecked behaviors before more work builds on them.
   L1's findings (`TASKS.md` section L) can be recorded in the same pass.
4. **Release dry run on clean machines** (R12), then **v0.3.0**.
5. **The icon picker** (M5 item 7, F-62, `TASKS.md` S9-S12), which needs a
   screen, and **code signing** (R10), when SignPath approves it.
6. **Animations beyond the eight that landed** (M5 item 5, D12, D13;
   `TASKS.md` section T): Bounce, Scroll, and Magic lamp (T19-T21) are
   ready. The composition spike (T1) runs alongside.

Not next: mapping Explorer's desktop view mode. The desktop offers only the
three icon views, and its Small icons keep labels below, so F-40 closes with
no mapping.

## 5. Milestones

Milestones are ordered by dependency. Each item is marked **[done]**,
**[partial]**, or not marked at all (not started). Within "Remaining", items
are listed in the order to build them.

### M0 - Foundations and guardrails  **[done]**

Goal: a codebase that stays healthy as it grows: CI gates, honest events,
work-area bounds, error boundaries, and debounced autosave. The full section
is archived (`archive/ROADMAP-progress-log.md`, "Moved from the main
docs").

### M1 - Direct2D renderer and Windows 11 look  **[done]**

Goal: spaces that look native on Windows 10 and 11: the D2D renderer, the
accent blur with DWM corners, and per-monitor DPI (a mixed-DPI setup is still
to check, `TASKS.md` section V). The full section is archived with M0's.

### M2 - Desktop items and takeover  **[partial]**

Goal: DeskSpace owns the desktop icon layer, and every item behaves like an
Explorer item, both inside spaces and loose on the desktop.

Done (each feature's status is in section 7; the full list is archived with
M0's): space chrome (F-02, F-03, F-05, F-14), the managed store (D3),
Explorer-like items in spaces (F-06, F-36..F-43), shell-sourced icons, the
opt-in desktop takeover (D7, D8), non-file shell drags (F-50), and the right
drag's G5a-G5c (check pending).

Remaining:
1. **Watch stored files.** Only the Desktop folder is watched; a change to a
   stored file made outside DeskSpace is noticed only by an item action or at
   startup. Lenses will reuse the same watcher (M4 phase 2). No task covers
   it yet; it is scoped with M4 phase 2.

Exit criteria:
- With DeskSpace running, Explorer's icon layer is gone and the desktop items
  are drawn and interactive.
- Dragging moves real files, and the move can be undone.
- Exiting, or a crash, restores the Explorer desktop.
- Show Desktop (Win+D) leaves the spaces visible.

### M3 - Control center: tray, context menus, settings  **[partial]**

Goal: every runtime setting can be edited without touching the TOML file.

Done (status in section 7; the full list is archived with M0's): the tray
(F-16), the space menu (F-14, F-24, F-31), the settings window (F-17) with
per-space looks (F-46), every `WindowSettings` field wired (F-18), dark mode
for DeskSpace's own chrome (F-30), the zoom pop-up (F-57), the color picker
(F-51), and the ways to add items without a drag (F-53..F-55).

Remaining:
1. Tray and menu entries:
   - pause takeover (unblocked now that the takeover exists);
   - new lens (M4);
   - snapshots (M6);
   - an Arrange submenu;
   - spaces entries on the desktop's own background menu - "New space",
     "DeskSpace Settings", and the like - beside the existing "Show spaces"
     verb, so the desktop is a control surface like the tray (F-52).
2. An Explorer "Send to" or right-click "Add to a DeskSpace space" entry
   (F-56). Deferred: it needs a hand-off to the running instance (the
   `--show` verb is the model) and, for the Windows 11 top-level menu, the
   M8 item 4 packaging.
3. More item presets and preset import/export (F-48). The "Item preset"
   dropdown now seeds the system with `Match Explorer's display` and
   `Custom`; named presets (Compact, Spacious) come later (`TASKS.md` C6).
   Sharing a preset is a theme file's job (item 4).
4. Theme files (F-58): save a whole look to a file and load it on Global or
   a space, so themes can be shared without forking (`TASKS.md` C10). A
   theme carries the item preset, since it sets icon size and spacing
   (decided 2026-10-29, maintainer).

Exit criteria: every runtime setting can be edited in the UI, and the tray
menu can show and hide spaces and open settings.

### M4 - Lenses (folder views)  **[done]**

Goal: spaces that mirror a folder live. Each phase lands green.

- **[done] Phase 0 - resize and scroll.**
  - `ResizeSpace` / `FitToContents`, and `Space::user_sized`; the columns
    follow the width.
  - Wheel scrolling with an indicator.
  - Scenes carry only the visible items.
  - An LRU-bounded `IconCache` (F-03).
- **Phase 1 - core model. [done]**
  - A source on each space:
    `Space.source: SpaceSource { Items, Folder(FolderView) }`, with
    `#[serde(default)]`.
  - The view settings: `FolderView { path, show_hidden, include, exclude }`,
    and `SpaceView { mode, sort }` since W14.
  - Entries come from the platform, so `core` does no IO: `FolderEntry { path,
    label, kind, size, modified, hidden }`.
  - New commands: `CreateLens`, `SetLensFolder`, `SetLensView`, and
    `SyncLens { id, items }`. A `LensBuilder` on the watch worker filters,
    sorts by the space's `SortRule`, and reconciles by path, so `ItemId`s
    survive refreshes (B37).
  - `AddItem` / `MoveItem` into a lens are `Rejected`.
  - Lens items are not persisted (F-07, F-08).
- **Phase 2 - platform. [done]**
  - Folder listing with `FindFirstFileExW`.
  - A watcher (shared with M2) using `ReadDirectoryChangesW`:
    - keyed, so the Desktop folder and every lens are watched at once, all
      on one worker thread (an I/O completion port; decided 2026-10-20);
    - debounced (150 ms);
    - a full rescan on the worker, never the UI thread, then a `WM_APP`
      wake posted to the controller.
  - `Event::FolderChanged { key, listing: Listed | Unavailable }`, with a
    periodic retry on the worker.
  - New `Ui` calls:
    - `Ui::watch_folder(key, path)` and `unwatch_folder(key)`;
    - `pick_folder` (`IFileOpenDialog`);
    - `transfer_files` with `OnCollision::Ask` (`IFileOperation` with undo
      and Explorer's Replace or Skip dialog). It pumps messages, so no
      runtime borrow may be held across it.
- **Phase 3 - app. [done]**
  - "New lens..." in the space, tray, and desktop layer menus.
  - Drops onto a lens move files into its folder. Items already there are
    skipped, and a folder cannot be dropped into itself.
  - A lens menu: Open folder, Change folder..., and Sort by, which holds
    Folders first and Show hidden files.
  - A placeholder when the folder is unavailable.
  - Deleting a lens removes only the view; its folder and the files in it
    are never deleted or moved (D3). The delete confirmation says so.
  - Double-click to open already works through `Ui::open_path`.
- **Phase 4 - views and kept sort rules. [done]** Decided in D17;
  `TASKS.md` W8-W23.
  - Lens markers: kind glyphs in the scopes pane, the folder's icon in a
    lens's title bar.
  - Explorer's eight views and icon sizes, for every space (F-09), and
    their Ctrl+Shift+1..8 keys. Under "Match Explorer", the desktop's own
    view needs no mapping: its View menu offers only the three icon views,
    which the default view already follows (F-40, W41).
  - A kept sort rule for lenses and normal spaces, with Explorer's Sort by
    shape (F-08, F-31).
  - Explorer's status bar: item and selection counts, the Details and
    Large icons buttons.
  - A lens's title from its folder name, profile path, full path, or
    custom text.
  - A View page for every space, a Lens page for each lens, and a Global
    Lenses page with defaults (F-17).
- **Phase 5 - extras. [done]** Decided in D18;
  `TASKS.md` W24-W49.
  - Two fixes first: a listing carries the folder it read, and the
    storage check moves off the UI thread.
  - Browse into subfolders inside the lens, with a back control (F-64).
  - Group by (F-65); Details columns: choose, resize, reorder, and Date
    created (F-63).
  - Lens filters (part of F-22), per-space file name extensions, compact
    view, the path tip, Ctrl+wheel through the views, and the default
    view for new spaces.
  - F-40 closed: the desktop offers only the three icon views, so no
    view-mode mapping is needed (W41).

Exit criteria: creating, renaming, or deleting a file in a lens's folder
updates the lens within about 150 ms, and the sort and view settings persist.

### M5 - Visibility and flow

Goal: hide, reveal, and peek instantly; then animate it.

Remaining:
1. Fold policy: manual (done), automatic after idle, and fold on
   mouse-out / unfold on hover (F-04). Add a per-space fold button.
2. Hide all: hide spaces and icons (including the takeover layer, which
   "Hide spaces" leaves visible, D9), and reveal them on a desktop click, a
   hotkey, or a peek (F-11, F-13). Double-clicking empty desktop toggles them,
   as in comparable tools (F-10).
3. Hover opacity: idle spaces fade to a chosen opacity and become solid on
   hover (F-32).
4. Title bar style: hidden, hover-only, or always shown, with alignment and
   size (F-28). A hidden title bar needs another way to move the space, such
   as Alt+drag or a grip.
5. Animations for hide and show, new spaces and startup, delete, and fold,
   from one shared list of effects, and spaces that animate in at startup
   and out at exit (F-33, D12, D13; `TASKS.md` section T). The
   composition groundwork they need for the blur (T1, T12, T13) is also the
   start of item 6.
6. Blurred spaces with corner radii beyond DWM's 8 px, using a composition
   backdrop clipped to an anti-aliased rounded shape.
   - Today the accent blur ignores window and blur regions, so blurred spaces
     are limited to DWM's radii.
   - First check that a host backdrop renders on windows that are never
     activated.
7. Space icons: an icon left of each space's title, matched from its name
   or chosen in a picker from the rename box or the space's settings, from
   a downloaded Fluent icon set (F-62, D16). Build it with or after F-28,
   which reworks the same title bar. Partial: the model, the library, the
   cache, the signed download, the title icon, and the consent are built (S1-S8,
   S13, S14); the picker and the settings follow (S9-S12).

Exit criteria: hide-all and peek are instant and reversible, and the fold
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
   - Optional measuring guides while a space is moved or resized (F-66),
     like the smart guides in Photoshop. Lines show when an edge or center
     lines up with another space or the work area. Labels give the gap to
     each neighbour in pixels, and mark equal gaps and equal sizes, so a
     layout can be made even by eye. A setting turns them on or off.
   - They come with snapping, and the setting is on the General page
     (D21).
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
6. Scripting verbs (F-59): a small, stable set of `--verb` commands handed to
   the running instance, the out-of-process extension point that the desktop
   menu (F-52) and "Send to" (F-56) can share (`TASKS.md` R16). In-process
   plugins are a non-goal for now.

Exit criteria: new icons file themselves according to the rules, and a full
workflow is possible without a mouse.

### M8 - Release engineering

Goal: a signed installer that works on a fresh machine.

Decisions: D6. Work items: R10-R13 in `TASKS.md` section D (maintainer
work); the finished E tasks are archived.

Remaining:
1. **[done]** A cargo-packager installer (D6), built by `release.yml` since
   v0.2.0; the clean-machine check is R12:
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
5. **[done]** A release checklist and versioning policy (`RELEASING.md`) and
   a reproducible build. `.github/workflows/release.yml` publishes the
   installer, a zip (`deskspace.exe`, `deskspace-watchdog.exe`, `LICENSE`,
   `NOTICE`, `README.md`), and its SHA-256 on a `v*` tag that matches the
   workspace version.
6. Documentation: a user guide, and troubleshooting for restoring after
   takeover and for conflicts with Explorer.
7. **[done]** Auto-update from GitHub Releases (F-45), following D6. The
   `cargo-packager-updater` SDK and the network IO live in `deskspace-update`,
   driven by the app, never in `core` or `config`; CONTEXT sections 5, 8, and
   10 describe the settings, controls, and code. `cargo deny` passes (the
   updater's `time` dependency moved the minimum Rust version to 1.88). Still
   to do (maintainer): set the `DESKSPACE_MINISIGN_KEY` secret and the public
   `UPDATE_PUBKEY`, which turns the feed on. It does not wait for code signing
   (item 3; D6, amended 2026-10-31), so until then unsigned updates trigger
   SmartScreen on every version.
8. A winget manifest in `microsoft/winget-pkgs` for the installer, updated
   per stable release by a workflow step, after the first stable release.

Exit criteria: a signed installer installs, runs, updates to a newer release,
and uninstalls cleanly on a fresh Windows 10 machine and a fresh Windows 11
machine.

### M9 - Translation

Goal: DeskSpace speaks the user's Windows display language, and adding a
language takes a translator, not a programmer (F-60).

Where we start: the UI is data-driven (section 3), so nearly every string a
user sees is built in `deskspace-app` (menus, the settings form, message
boxes, the tray tooltip) and handed to the platform as data. A few are
written in the platform: the color picker, the installed-apps picker, the
custom shortcut dialog, the right-drop menu, and the info tip. Explorer's own
menus and dialogs (the item menu, conflict prompts, Properties) are already
in the user's language.

Remaining:
1. **Design.** The format is decided (D20); the rest is `TASKS.md` R18.
2. **Extraction.** Every user-facing string goes through one lookup, with
   English as the source text and no visible change. The platform takes its
   few strings from the app as arguments, so it holds no catalogs. A test
   fails on a key with no English text. Text the user owns (a space's title,
   the configured new-space title) is data and is never translated after it
   is written.
3. **Choosing the language.** Follow the Windows display language
   (`GetUserPreferredUILanguages`), falling back from region to language to
   English. A "Language" choice on the General page (Windows default, then
   each bundled language) applies on Apply; menus are built on demand
   already, and the open settings window is rebuilt.
4. **Layout that fits every language.** Settings labels have a fixed width
   today (`Builder::field`), and German or Finnish run about 30 % longer than
   English. Measure or wrap the text instead. A pseudo-locale (accented,
   padded English, as Windows' `qps-ploc`) shows untranslated strings and
   truncation in one on-screen pass.
5. **Plurals and placeholders.** Messages with counts (the tray tooltip's
   spaces and items) follow each language's plural rules, and placeholders
   are named so a translator can reorder them.
6. **The first languages,** from contributors, with a translator guide in
   `CONTRIBUTING.md`. The installer's pages follow (NSIS language files,
   through cargo-packager).
7. **Later.**
   - Right-to-left languages (Arabic, Hebrew) need mirrored layout
     (`WS_EX_LAYOUTRTL`, DirectWrite's reading direction) in spaces, the
     settings window, and the pop-ups; a project of its own.
   - Locale-aware natural sort of item names (`CompareStringEx`).
   - Full East Asian use also needs IME composition in inline rename
     (`CONTEXT.md` section 10).

Not planned for now: translating the docs, and a hosted translation platform
(Weblate, for example) before there are several translators.

Exit criteria: with Windows set to a bundled language, every DeskSpace string
is in that language and none is cut off at 100 % and 150 %; the pseudo-locale
shows no English; and adding a language needs no code change beyond its
catalog.

## 6. Release mapping

- **v0.2 "Take the desktop"** (v0.2.0-v0.2.4, section 4): M0, M1, and most
  of M2 and M3, with takeover opt-in (D8, D9). The rest of M2 and M3 (the
  stored-file watch, the desktop verbs) ships in a v0.2.x release or with
  v0.3, as the maintainer decides. Undo (D11) landed after v0.2.4.
- **v0.3 "Lenses"**: M4, phases 0-5 (D18, amended 2026-10-31).
- **v0.4 "Flow"**: M5.
- **v0.5 "Rooms"**: M6, M7.
- **v1.0**: M8.
- **Translation (M9)** is not yet assigned to a release. Extraction (item 2)
  changes nothing visible and can land in any release; the maintainer picks
  the release that ships the first language.

## 7. Feature index

This is the single list of features, which also serves as the feature parity
checklist. "Check pending" means the feature is implemented and unit-tested
or smoke-tested, but has not yet been confirmed on screen. Global settings
belong in `Theme` / `WindowSettings` (a look setting in `Theme` is per space too,
through `SpaceLook`), and other per-space state in `Space`, as
`#[serde(default)]` fields changed through `Command`s.

| Id | Feature | Milestone | Status |
|----|---------|-----------|--------|
| F-01 | Spaces / icon groups | M2 | Done; the desktop takeover is opt-in and its on-screen checks are pending |
| F-02 | Create / rename / delete space | M2 | Done; undo of a delete awaits its on-screen check (U5) |
| F-03 | Resize space | M2, M4 | Done (edges/corners, scroll, fit to contents); whole-column and whole-row fit checked 2026-10-12, gated by the General page's "Snap resized spaces" (`GridSpec.fit_to_grid`), and since D22 it fits the current view's cells (Details and Content rows only); Fit to contents is also on an item's menu and capped to the work area (D23) |
| F-04 | Fold spaces (formerly roll-up) | M2 (manual), M5 (policy) | Manual done; policy open |
| F-05 | Lock space | M2 | Done |
| F-06 | Drag icons in / out / between spaces | M2 | Done for spaces and for loose items on the takeover layer (opt-in); a drop on the takeover desktop opens a new space (D7, amended 2026-10-17); checks pending for the layer |
| F-07 | Lenses (folder portals) | M4 | Done: lenses work end to end and phase 4 added the views, sort rules, status bar, and lens settings (M4 phases 1-4, W1-W23, 2026-10-24); on-screen checks in `TASKS.md` section V |
| F-08 | Lens sort rules | M4 | Done: one `SortRule`, Explorer's Sort by (W14), and Keep sorted (W15); check pending (`TASKS.md` V: W15) |
| F-09 | Lens show-as / icon size | M4 | Done: show-as (W11) and all eight Explorer views (W12, W16-W22), with Ctrl+wheel stepping through them (W39); checks pending (`TASKS.md` section V) |
| F-10 | Hide all on desktop double-click | M5 | Open |
| F-11 | Auto-hide | M5 | Open |
| F-12 | Pages / virtual desktops | M6 | Open |
| F-13 | Quick peek / transparency toggle | M5 | Open |
| F-14 | Right-click context menu | M2 (basic), M3 (full) | Space menu, Explorer item menu, and the takeover desktop menu done (desktop menu check pending); lens and arrange entries open |
| F-15 | Snap to grid / auto-arrange | Core | Done |
| F-16 | System tray | M3 | Done; a pause-takeover entry is open, snapshot entries wait for M6 |
| F-17 | Settings window | M3, M4 | Done: a scopes/pages/page layout and the Lens, View, and Lenses pages (W10, W20, W23); checks pending (`TASKS.md` section V) |
| F-18 | Hotkeys | M3 | Done; check pending (Hotkeys page binds show/hide, new space, and settings) |
| F-19 | Multi-monitor | M1, M6 | Partial: per-monitor work areas and DPI; monitor assignment open |
| F-20 | Backup / restore / snapshots | M6 | Open (config file only) |
| F-21 | Auto-consolidate new icons | M7 | Open |
| F-22 | Exclude rules / filters | M7 | Partial: lens filters are done, check pending (`TASKS.md` V: W35); rules for normal spaces are open; decided in D18 |
| F-23 | Keyboard navigation | M7 | Partial: within one space (`TASKS.md` G1, G3; check pending); between spaces open |
| F-24 | Appearance: color, translucency, blur | M1, M3 | Done, globally and per space (a customized space has its own copy of every look setting); border thickness and color and drawn shadows (D15) built 2026-10-19, check pending (`TASKS.md` section V, "Border settings", "Drawn shadows") |
| F-25 | Icon size | M3 | Done (50-200 %, jumbo icons, per-monitor DPI, per space; Ctrl+wheel per D9); check pending (`TASKS.md` V: F13, Display) |
| F-26 | Grid density | M3 | Done (icon spacing and edge padding) |
| F-27 | Label style | M3 | Done (lines, size, shadow mode) |
| F-28 | Title bar style | M5 | Open |
| F-29 | Windows accent color | M3 | Done; check pending |
| F-30 | Settings and menu theme override | M3 | Done; check pending |
| F-31 | Sort items in normal spaces | M3, M4 | Done: Explorer's Sort by (W14) and Keep sorted (W15); check pending (`TASKS.md` V: W15) |
| F-32 | Hover opacity | M5 | Open |
| F-33 | Animations: startup and exit, hide/show, new space, delete, fold | M5 | Partial: built for Startup, Delete, and Fold with eight effects (D12; `TASKS.md` section T), awaiting the on-screen checks in section V; Bounce, Scroll, and Magic lamp ready (T19-T21) |
| F-34 | Start with Windows | M3, M8 | Done; check pending |
| F-35 | Snap to other spaces | M6 | Open |
| F-36 | Explorer-parity drag (move default, OLE drop target, right-button drag per D9) | M2 | Done (G5a-G5c); check pending |
| F-37 | Drag image | M2 | Done; check pending |
| F-38 | Clipboard cut / copy / paste | M2 | Done; check pending |
| F-39 | Delete to the Recycle Bin | M2 | Done; check pending |
| F-40 | Explorer icon size and view mode | M2, M4 | Done: icon size follows the desktop, whose View menu offers only Large, Medium, and Small icons, with labels below in Small icons (Windows 11, 2026-10-31; Windows 10 unverified, `TASKS.md` section V), so no view-mode mapping is needed (D18, W41) |
| F-41 | Independent item check boxes | M2 | Done; check pending |
| F-42 | Rubber-band, Ctrl+A, multi-item drag | M2 | Done within one space; check pending |
| F-43 | Info tips, compressed/encrypted colors, Explorer spacing | M2 | Done; check pending |
| F-44 | Title bar style (duplicate) | - | Merged into F-28 |
| F-45 | Auto-update from GitHub Releases | M8 | Done (D6; `TASKS.md` E8-E11); check pending; the minisign key is a maintainer setup step |
| F-46 | Per-space looks in the settings window (scopes pane, Customize switch, highlight) | M3 | Done; check pending (`TASKS.md` H5, H4) |
| F-47 | Desktop overview map (monitors, wallpaper, miniature spaces) | M6 | Open; after F-19 and F-12 |
| F-48 | Item display presets (Match Explorer / Custom; more presets and import/export later) | M3, M6 | Partial: `Match Explorer's display` / `Custom` dropdown (`TASKS.md` C5); named presets open (C6), sharing through theme files (C10) |
| F-49 | Tabs in spaces (per-space pagination, off by default) | M6 | Open (`TASKS.md` section J) |
| F-50 | Accept non-file shell drags (Start menu / taskbar apps) as shortcuts | M2 | Done; check pending (`TASKS.md` V: L2; L1 findings) |
| F-51 | Modern color picker (replace the dated system `ChooseColor` dialog) | M3 | Done, and redesigned after the mockup: acrylic, hue and opacity sliders, hex/RGB/HSL entry, copy and paste, an eyedropper, and recent colors; check pending (`TASKS.md` V: F14) |
| F-52 | Spaces entries on the desktop background context menu (New space, DeskSpace Settings, ...) | M3 | Open (`TASKS.md` C9); the "Show spaces" verb exists while spaces are hidden (M8 item 4 moves it into the Windows 11 top-level menu) |
| F-53 | "New shortcut ▸": pick installed apps (Store apps included), or describe a custom target, to add as shortcuts | M3 | Done: the space menu's submenu and a space's Settings page "Shortcuts" (a preview with an X per item and a + tile); checked on screen 2026-10-13 |
| F-54 | "Paste shortcut" into a space, as in Explorer | M3 | Done; check pending (`TASKS.md` L5) |
| F-55 | "Add files…": pick files to add to a space | M3 | Done; check pending (`TASKS.md` V: L6) |
| F-56 | Explorer "Send to" / "Add to a DeskSpace space" entry | M3, M8 | Deferred; needs an instance hand-off and, for the Windows 11 menu, M8 item 4 |
| F-57 | Zoom pop-up: the icon size as a percentage at the top center of the screen, with zoom out, zoom in, and Reset | M3 | Done; check pending (`TASKS.md` V) |
| F-58 | Theme files: save, load, and share a look | M3 | Open (`TASKS.md` C10) |
| F-59 | Scripting verbs for the running instance (out-of-process extension point) | M7 | Open; needs design (`TASKS.md` R16) |
| F-60 | Translation: the UI follows the Windows display language, with a Language override | M9 | Open; format decided in D20, the rest of the design is `TASKS.md` R18 |
| F-61 | Debug window (`--debug`): live log console, test-space tools, diagnostics, and simulations | Tooling | Built; awaiting on-screen check (`TASKS.md` section V, X4-X20 and extras) |
| F-62 | Space icons: an icon beside the title, matched from the space's name, changeable in the rename box and the space's settings | M5 | Partial: the model, the icon-set library, the cache, the signed download, the title icon, and the download consent are built (S1-S8, S13, S14); the picker follows v0.3.0 (S9-S12), decided in D16 |
| F-63 | Details columns: choose, resize, reorder, Date created | M4 | Done; check pending (`TASKS.md` V: W30, W31, W32); decided in D18 |
| F-64 | Browse into subfolders inside a lens | M4 | Done; check pending (`TASKS.md` V: W42, W43); decided in D18 |
| F-65 | Group by, as Explorer (every space) | M4 | Done; check pending (`TASKS.md` V: W37, W38); decided in D18 |
| F-66 | Measuring guides while moving or resizing a space: alignment lines, gaps in pixels, and equal-gap and equal-size marks (optional) | M6 | Open; decided in D21 (M6 item 4) |
| F-67 | A lens follows its folder when it is renamed or moved | M4 | Done; check pending (`TASKS.md` V: W45); decided in D19 |


## 8. Open questions

Resolve each one during the milestone that raises it.

1. **Takeover safety (M2).** *Resolved:* D8.
2. **Desktop layer vs blur (M2).** *Resolved:* D8.
3. **Undo model (M2).** *Resolved:* D11.
4. **Translation format (M9).** *Resolved:* D20.
5. **A lens whose folder is renamed or moved (M4).** *Resolved:* D19.

Every other question raised so far is answered by a decision in section 2.

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
- **Undocumented drag sources.** What the Start menu and the taskbar put in a
  drag (formats and allowed effects) is not documented and may change between
  Windows builds. Mitigation: decide a drop by the allowed effects rather than
  by format (`TASKS.md` L2), and offer a non-drag path ("New shortcut ▸ From
  installed apps…", F-53).
- **Scope.** The parity checklist covers a lot of ground. The milestone gates
  exist to keep each release shippable.

## Appendix A. Progress log

Moved to `archive/ROADMAP-progress-log.md` (history up to v0.2.0). The git log
and the GitHub release notes record what landed since.
