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
    traffic happens unless the user starts it or opts in.
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
    ships before or together with auto-update.

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
    space's own window carries the blur over its whole rect; a Smoke
    toggle gives it a neutral gray plume behind the flames, off by
    default and offered only in the debug window until it is tuned,
    amended 2026-10-20, maintainer), and Smoke (amended 2026-10-20, maintainer: the space
    sublimates into plumes of its own tint that rise from a few seeded
    sources along its lower edge, fading the window where they pass, while
    its text and icons ride the same current as separate sprites and land
    in their places; a show condenses the smoke and flies the contents in,
    and the smoke rises past the frame, so it uses Burn's
    overlay; each play is seeded when it starts, so it never looks the
    same twice while a reversal still retraces the same wisps; until it
    is tuned, only the debug window offers it, and the settings window
    lists it only while it is already chosen). Black
    hole (the space swallowed by a hole that opens on the desktop) is
    planned after composition (T14). Also
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
    - **Magic lamp:** the macOS genie. The space narrows along curved sides
      and pours into a point, and comes back out of it. It warps the
      shape, which the rect-wide blur cannot follow, so it is drawn on a
      click-through overlay (as Burn's flames are) while the space's own
      window is hidden. Where the point is waits on a decision (T21).

    A kind can limit the list: Fold offers only None, Roll, and Scroll.
  - *Controls:* each kind has the common ones: its effect and a duration
    (100 to 1000 ms). An effect adds only the options it uses: Detail (the
    block size, grain, or edge width) for Pixelate, Dissolve, Burn, and
    Smoke; Smoke (off by default) for Burn; and Direction (which edge;
    Automatic is the effect's own: the top for Roll and Gradient, the
    nearest for Fly) for Roll, Gradient, and Fly. A control the
    chosen effect does not use is greyed, so each page keeps its shape. The
    controls live in a Global-only Animations group of pages, one per kind,
    so no page grows too tall for a 1080-pixel display. Defaults: Roll at
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
    (show, hide, create, destroy, fold). A window timer in the platform
    runs the frames from pure frame math in `render/`, so there is still
    no per-frame callback into the app.
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
    window keeps its shape.
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

- **D15. Extensibility: themes, textures, animations, verbs, and scripts
  (2026-10-19, maintainer).** Decides F-21, F-58, F-59, and F-62.
  - *Crate boundary:* a new `#![forbid(unsafe_code)]` crate, `deskspace-ext`
    (`app -> {config, platform, ext, update} -> core` and `platform -> ext`),
    owns the sandboxed `rhai` scripting engine, extension manifests, and
    compiled script evaluation. `deskspace-core` keeps zero IO and zero
    scripting dependencies.
  - *Folders:* three folders sit beside `deskspace.toml` (`ConfigStore`):
    `themes/`, `animations/`, and `scripts/`, so a scratch `--config` has its
    own. DeskSpace re-scans them when the settings window opens, when the user
    clicks "Reload extensions" in Settings or Debug, or on `--verb reload`,
    and watches them live with `ReadDirectoryChangesW` while `--debug` is on.
  - *Themes (F-58) and space textures (F-62):*
    - A theme is either a single `<name>.deskspace-theme.toml` file in
      `themes/` (for the standard Save and Open file dialogs) or a pack folder
      `themes/<slug>/manifest.toml`.
    - Each holds `format = 1`, `name`, an optional `author`, a `[look]` table
      (`SpaceLook`, `preset` included), and an optional `[animations]` table
      (`Animations`, applied in the Global scope and ignored on a single
      space). `ui_mode` is ignored on apply, as it is for a space.
    - A pack folder `themes/<slug>/` can also bundle custom animations under
      `themes/<slug>/animations/<anim-slug>/` and space textures (`[textures]`
      in `manifest.toml`): **body**, **title_bar**, and **frame** image layers
      with `tile`, `cover`, or `nine_slice` placement and a per-layer opacity.
      Images are decoded once through WIC (`IWICImagingFactory` in
      `render/d2d/bitmap.rs`), adding no image crate.
    - Controls live on a **Themes** look page (listed for Global and for a
      space, keeping `Colors` short enough for a 1080-pixel display): a
      Themes dropdown, "Save theme…", "Load theme…", and "Open themes folder".
  - *Custom animations (F-33, F-62):*
    - One subfolder per animation: `animations/<slug>/manifest.toml` (name,
      author, offered `options` and `kinds`) plus `effect.rhai`, stored in
      `EffectChoice` as `"custom:<slug>"` (clamped back to the kind's default
      when missing).
    - One unified model evaluated on the space window timer from a
      pre-compiled `rhai::AST`: `fn frame(u, ctx)` runs per tick to return the
      rect, offset, opacity, block size, glow, and mask parameters; an
      optional `fn field(x, y, ctx)` runs once when the animation starts to
      build a 2D heightmap cell grid that Rust thresholds per frame; an
      optional `fn slices(u, ctx)` returns strip or slice meshes.
    - Discovered animations appear in the Effect dropdowns on the existing
      Animations pages (`Startup`, `Delete`, `Fold`).
  - *Scripts and auto-organization rules (F-21, F-62):*
    - One subfolder per script: `scripts/<slug>/manifest.toml` plus
      `script.rhai`. A script can declare a desktop rule
      (`fn on_desktop_item(item, ctx)`), a manual or hotkey action
      (`fn on_action(ctx)`), or both.
    - Scripts run in a sandboxed `rhai::Engine` (strict operation, call-stack,
      string, and array limits; no OS, filesystem, or network access). They
      read desktop and item facts and queue DeskSpace `Command`s and app
      actions: create, move, resize, fold, sort, and lock spaces; route and
      move items; switch themes; and delete spaces or recycle items (always
      asking with a confirmation dialog before anything destructive).
    - Every script run is recorded as one undo entry (D11).
    - A Global-only **Scripts** settings page toggles discovered scripts,
      runs action scripts, opens the folders, and shows any load or runtime
      error.
  - *Scripting verbs (F-59):* `deskspace.exe --verb <command>` (`show`,
    `hide`, `new-space`, `settings`, `apply-theme <name>`,
    `run-script <slug>`, `reload`) sends the verb over `WM_COPYDATA` to
    `DeskSpaceController` (UIPI-protected on the UI thread, no background
    thread). `--show` and `--settings` stay as shorthand aliases.
  - *Work items:* `TASKS.md` C10 and section P.

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
update checking, F-45), v0.2.2 (adding shortcuts to spaces, F-50/F-53), and
v0.2.3 (the space menu's Add files, F-55, and the right-button drag, F-36).
Takeover ships opt-in (D8, D9).

**Done:** M0 (foundations) and M1 (renderer and Windows 11 look).

**In progress:** M2 (undo and the stored-file watch remain), M3 (desktop
verbs, F-52, and entries that wait on later milestones or design), and M4
(phase 1, the lens core model, is done; the platform watcher, app behavior,
and views remain). Section 5 lists what each has done.

**Next, in order:**

1. **L1's on-screen findings** (`TASKS.md` section L): record what each
   drag source offers. The rest of adding items to spaces (F-50,
   F-53..F-55) has landed.
2. **On-screen verification pass** (`TASKS.md` section V, R7). Confirm the
   built-but-unchecked behaviors before more work builds on them.
3. **Release dry run on clean machines** (R12) and **code signing** (R10),
   before turning on the update feed.
4. **M2 undo** for layout changes, file moves, and space deletion (D11;
   `TASKS.md` section U).
5. **Animations** and graceful startup and exit (M5 item 5, D12, D13;
   `TASKS.md` section T),
   ahead of the rest of M5. The composition spike (T1) runs alongside.

Not next: mapping Explorer's list/details view mode (the rest of F-40). It
needs DeskSpace to have list and details views first, and those arrive with
M4 phase 4.

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
   startup. Lenses will reuse the same watcher (M4 phase 2).
2. **Undo** for layout changes, file moves, item removal, and space
   deletion (D11). F-02's "delete with undo" depends on it.

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
   `Custom`; named presets and Save/Open come later (`TASKS.md` C6).
4. Theme files (F-58): save a whole look to a file and load it on Global or
   a space, so themes can be shared without forking (decided in D15;
   `TASKS.md` C10).

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
- **Phase 1 - core model. [done]**
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
  - Deleting a lens removes only the view; its folder and the files in it
    are never deleted or moved (D3). The delete confirmation says so.
  - Double-click to open already works through `Ui::open_path`.
- **Phase 4 - views and kept sort rules.**
  - List and details views and icon sizes (F-09). Map Explorer's
    `FOLDERVIEWMODE` onto these views under "Match Explorer" (the rest of
    F-40).
  - A kept sort rule for lenses and normal spaces (F-08, F-31).
  - A Lenses settings page with defaults (F-17).
- **Later.** Browse into subfolders inside the lens, with a back control (as
  comparable tools do). Until then, subfolders open in Explorer.

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
   new spaces, or leave them loose (F-21, D15; `TASKS.md` section P).
2. Include/exclude filters and an ignore list for shortcuts (F-22).
3. Keyboard navigation between spaces and onto the desktop layer (F-23).
   Within a space it is done: arrows, type-to-select, Home/End/Page, Enter,
   F2, Delete, and the Menu key.
4. Accessibility: keyboard focus, a high-contrast theme, and screen-reader
   labels.
5. Performance pass: idle CPU near zero, and large spaces handled well. Scenes
   already carry only the visible items, and the icon cache is already
   LRU-bounded.
6. Scripting verbs (F-59, D15): a small, stable set of `--verb` commands
   handed to the running instance over `WM_COPYDATA`, the out-of-process
   extension point that the desktop menu (F-52), "Send to" (F-56), and user
   automation share (`TASKS.md` section P). In-process native DLL plugins stay
   a non-goal; user scripts run in the sandboxed `rhai` engine (`deskspace-ext`).

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
   `UPDATE_PUBKEY`, then sign before shipping (item 3), because unsigned
   updates trigger SmartScreen on every version.
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
1. **Design.** Settle open question 4 (`TASKS.md` R18).
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

- **v0.2 "Take the desktop"** (v0.2.0-v0.2.3, section 4): M0, M1, and most
  of M2 and M3, with takeover opt-in (D8, D9). The rest of M2 and M3 (undo,
  the desktop verbs) ships in a v0.2.x release or with v0.3, as the
  maintainer decides.
- **v0.3 "Lenses"**: M4.
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
| F-02 | Create / rename / delete space | M2 | Done; delete has no undo yet |
| F-03 | Resize space | M2, M4 | Done (edges/corners, scroll, fit to contents); whole-column and whole-row fit checked 2026-10-12, gated by the General page's "Snap resized spaces" (`GridSpec.fit_to_grid`) |
| F-04 | Fold spaces (formerly roll-up) | M2 (manual), M5 (policy) | Manual done; policy open |
| F-05 | Lock space | M2 | Done |
| F-06 | Drag icons in / out / between spaces | M2 | Done for spaces and for loose items on the takeover layer (opt-in); a drop on the takeover desktop opens a new space (D7, amended 2026-10-17); checks pending for the layer |
| F-07 | Lenses (folder portals) | M4 | Partial: core model only (M4 phase 1); platform watcher, menus, and settings open |
| F-08 | Lens sort rules | M4 | Open |
| F-09 | Lens show-as / icon size | M4 | Open |
| F-10 | Hide all on desktop double-click | M5 | Open |
| F-11 | Auto-hide | M5 | Open |
| F-12 | Pages / virtual desktops | M6 | Open |
| F-13 | Quick peek / transparency toggle | M5 | Open |
| F-14 | Right-click context menu | M2 (basic), M3 (full) | Space menu, Explorer item menu, and the takeover desktop menu done (desktop menu check pending); lens and arrange entries open |
| F-15 | Snap to grid / auto-arrange | Core | Done |
| F-16 | System tray | M3 | Done; a pause-takeover entry is open, snapshot entries wait for M6 |
| F-17 | Settings window | M3, M4 | Partial: 9 pages in a scopes/pages/page layout (New spaces: check pending); Lenses page open |
| F-18 | Hotkeys | M3 | Done; check pending (Hotkeys page binds show/hide, new space, and settings) |
| F-19 | Multi-monitor | M1, M6 | Partial: per-monitor work areas and DPI; monitor assignment open |
| F-20 | Backup / restore / snapshots | M6 | Open (config file only) |
| F-21 | Auto-consolidate new icons | M7 | Open; decided in D15 (`TASKS.md` section P) |
| F-22 | Exclude rules / filters | M7 | Open |
| F-23 | Keyboard navigation | M7 | Partial: within one space (`TASKS.md` G1, G3; check pending); between spaces open |
| F-24 | Appearance: color, translucency, blur | M1, M3 | Done, globally and per space (a customized space has its own copy of every look setting); border thickness and color built 2026-10-19, check pending (`TASKS.md` section V, "Border settings") |
| F-25 | Icon size | M3 | Done (50-200 %, jumbo icons, per-monitor DPI, per space; Ctrl+wheel per D9); check pending (`TASKS.md` V: F13, Display) |
| F-26 | Grid density | M3 | Done (icon spacing and edge padding) |
| F-27 | Label style | M3 | Done (lines, size, shadow mode) |
| F-28 | Title bar style | M5 | Open |
| F-29 | Windows accent color | M3 | Done; check pending |
| F-30 | Settings and menu theme override | M3 | Done; check pending |
| F-31 | Sort items in normal spaces | M3, M4 | One-off sort done; kept rule open (with F-08) |
| F-32 | Hover opacity | M5 | Open |
| F-33 | Animations: startup and exit, hide/show, new space, delete, fold | M5 | Open; decided in D12 and D15 (`TASKS.md` sections T and P); Smoke done (T22), Bounce, Scroll, Magic lamp (T19-T21), and user Rhai animations (P4-P7) planned |
| F-34 | Start with Windows | M3, M8 | Done; check pending |
| F-35 | Snap to other spaces | M6 | Open |
| F-36 | Explorer-parity drag (move default, OLE drop target, right-button drag per D9) | M2 | Done (G5a-G5c); check pending |
| F-37 | Drag image | M2 | Done; check pending |
| F-38 | Clipboard cut / copy / paste | M2 | Done; check pending |
| F-39 | Delete to the Recycle Bin | M2 | Done; check pending |
| F-40 | Explorer icon size and view mode | M2, M4 | Icon size done; view mode waits for M4 phase 4 views |
| F-41 | Independent item check boxes | M2 | Done; check pending |
| F-42 | Rubber-band, Ctrl+A, multi-item drag | M2 | Done within one space; check pending |
| F-43 | Info tips, compressed/encrypted colors, Explorer spacing | M2 | Done; check pending |
| F-44 | Title bar style (duplicate) | - | Merged into F-28 |
| F-45 | Auto-update from GitHub Releases | M8 | Done (D6; `TASKS.md` E8-E11); check pending; the minisign key is a maintainer setup step |
| F-46 | Per-space looks in the settings window (scopes pane, Customize switch, highlight) | M3 | Done; check pending (`TASKS.md` H5, H4) |
| F-47 | Desktop overview map (monitors, wallpaper, miniature spaces) | M6 | Open; after F-19 and F-12 |
| F-48 | Item display presets (Match Explorer / Custom; more presets and import/export later) | M3, M6 | Partial: `Match Explorer's display` / `Custom` dropdown (`TASKS.md` C5); import/export open (C6) |
| F-49 | Tabs in spaces (per-space pagination, off by default) | M6 | Open (`TASKS.md` section J) |
| F-50 | Accept non-file shell drags (Start menu / taskbar apps) as shortcuts | M2 | Done; check pending (`TASKS.md` V: L2; L1 findings) |
| F-51 | Modern color picker (replace the dated system `ChooseColor` dialog) | M3 | Done; check pending (`TASKS.md` V: F14) |
| F-52 | Spaces entries on the desktop background context menu (New space, DeskSpace Settings, ...) | M3 | Open (`TASKS.md` C9); the "Show spaces" verb exists while spaces are hidden (M8 item 4 moves it into the Windows 11 top-level menu) |
| F-53 | "New shortcut ▸": pick installed apps (Store apps included), or describe a custom target, to add as shortcuts | M3 | Done: the space menu's submenu and a space's Settings page "Shortcuts" (a preview with an X per item and a + tile); checked on screen 2026-10-13 |
| F-54 | "Paste shortcut" into a space, as in Explorer | M3 | Done; check pending (`TASKS.md` L5) |
| F-55 | "Add files…": pick files to add to a space | M3 | Done; check pending (`TASKS.md` V: L6) |
| F-56 | Explorer "Send to" / "Add to a DeskSpace space" entry | M3, M8 | Deferred; needs an instance hand-off and, for the Windows 11 menu, M8 item 4 |
| F-57 | Zoom pop-up: the icon size as a percentage at the top center of the screen, with zoom out, zoom in, and Reset | M3 | Done; check pending (`TASKS.md` V) |
| F-58 | Theme files: save, load, and share a look | M3 | Open; decided in D15 (`TASKS.md` C10a-C10c) |
| F-59 | Scripting verbs for the running instance (out-of-process extension point) | M7 | Open; decided in D15 (`TASKS.md` P8-P10) |
| F-60 | Translation: the UI follows the Windows display language, with a Language override | M9 | Open; design awaits open question 4 (`TASKS.md` R18) |
| F-61 | Debug window (`--debug`): live log console, test-space tools, diagnostics, and simulations | Tooling | Built; awaiting on-screen check (`TASKS.md` section V, X4-X20 and extras) |
| F-62 | User extensions: custom space textures in theme packs, Rhai animation scripts, and Rhai action/rule scripts | M3, M5, M7 | Open; decided in D15 (`TASKS.md` section P) |


## 8. Open questions

Resolve each one during the milestone that raises it.

1. **Takeover safety (M2).** *Resolved:* D8.
2. **Desktop layer vs blur (M2).** *Resolved:* D8.
3. **Undo model (M2).** *Resolved:* D11.
4. **Translation format (M9).** Which message format, and where do the
   catalogs live?
   - *Fluent* (`fluent-bundle`): plurals, named placeholders, and grammar
     live in the translator's file, and the tooling is mature. It adds
     dependencies for `cargo deny` to clear, and its keys are strings, so a
     test or a generated enum must check them.
   - *A hand-rolled table:* a message enum with English in the code and one
     TOML file per language, checked by tests. The compiler checks every key
     and nothing is added to the dependency tree, but each language's plural
     rules are written by hand.
   - *gettext `.po`:* the format translators know best, but the pure-Rust
     readers are less maintained.
   - Catalogs embedded in the executable keep the portable zip one file;
     catalogs beside it let a translator try a change without building.
   - Proposed, awaiting the maintainer: Fluent, embedded, with a test that
     every key the code uses has English text.

Every other question raised so far is answered by a decision in section 2
(D2, D3, D6-D9, D11-D13).

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
