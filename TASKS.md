# TASKS

Concrete, self-contained work items. `ROADMAP.md` says *what* to build and in
what order; this file says *how*, one task per commit. Tasks are written so a
contributor (human or model) can do one without designing anything new.

## How to work a task

1. Read `AGENTS.md` once, then only the `CONTEXT.md` sections a task names.
2. Do one task per commit. Do not combine tasks or fix "nearby" things; note
   them under **Found while working** at the end of this file instead.
3. Line numbers are approximate and drift. Find code by the function or type
   name given; the line is only a hint.
4. If the code does not match the task's description, or the task needs a
   design decision it does not state, **stop and ask**. Do not improvise.
5. Copy the pattern the task points at. Do not copy patterns listed under
   "Known bad examples" below.
6. Done means all of:
   - `cargo fmt --all`
   - `cargo clippy --workspace --all-targets -- -D warnings` (no new
     `#[allow]`)
   - `cargo test --workspace`, with new tests for new logic
   - `CONTEXT.md` updated where the task changes behavior, and the
     `ROADMAP.md` feature index (section 7) updated if a feature's status
     changed
   - the task's own "Done when" list
7. Commit with the repo's style: `feat: ...`, `fix: ...`, `refactor: ...`,
   `docs: ...`, imperative and lowercase. Then tick the task's box here in
   the same commit.
8. Tasks marked **[needs human]** need an on-screen check you cannot do.
   Implement, then list exactly what to check in the commit body and leave the
   box unticked with "(awaiting check)".
9. Tasks in section D are **not** for routine work. Skip them.

### Known bad examples (do not copy until fixed)

- `App::reflow_space` (`deskspace-app/src/app.rs`) mutates a `Space`
  directly instead of through a `Command` (fixed by A5).
- `Ui::show_info_tip` / `hide_info_tip` (`win32/runtime.rs`) hold a runtime
  borrow across a Win32 call that sends messages (fixed by A1).
- `unsafe` blocks without `// SAFETY:` (fixed by A2).
- `pick_space_tint` / `pick_space_text` and the opacity/sort `match` arms in
  `show_space_menu` are duplicated code (fixed by B3/B4).

### Recipe: a new global setting

The pattern used by `Theme.icon_size` / `Theme.overlay_size`:

1. `deskspace-core/src/theme.rs` (or `deskspace-config/src/schema.rs` for a
   `WindowSettings` field): add the field with a `///` doc, give it a value in
   `Default`, and rely on the struct-level `#[serde(default)]`. If it is a
   range, add `pub const NAME_RANGE`/`NAME_DEFAULT` like `OVERLAY_SIZE_RANGE`.
2. `deskspace-app/src/settings.rs`: add a `field::` id (next free number) and
   a `Field` on the right page: a look setting goes in `look_pages` (fed the
   theme being edited, which is a space's own while it is customized), a
   Global-only one in `general_page`. Then add the match arm that clamps and
   returns the right `Effect` (`Appearance` to restyle, `Layout` to re-flow,
   `Pending` for apply-on-OK): in `apply_look_field` for a look setting, which
   makes it per space with no further work, or in `apply_field`'s Global-only
   arms otherwise.
3. If Cancel must undo a re-flow, extend `App::cancel_settings`.
4. Tests: a serde round-trip / missing-field default test in core or config,
   and an `apply_field` test in `settings.rs` (copy the existing ones).

### Per-space settings

There is no per-space override recipe any more. A space's look is a whole
copy of the look settings (`SpaceLook`: the `Theme` plus grid spacing), so
every look setting added through the recipe above is per space too. A look
setting outside `Theme` and `GridSpec` would go into `SpaceLook` (with
`from_global`, `clamped`, and `grid` kept in step). The app reads a space's
values through `theme_for(id)`, `grid_for(id)`, and `palette_for(id)`, never
`config.theme` directly.

---

## A. Fix the bad examples (do these first)

- [x] **A1. Do not hold the runtime borrow across info-tip calls.**
  - File: `crates/deskspace-platform/src/win32/runtime.rs`,
    `Ui::show_info_tip` and `Ui::hide_info_tip` (~line 482-514).
  - Problem: `tip.show(...)` / `tip.hide()` send window messages while
    `with_runtime` holds the borrow, which the module header forbids.
  - Fix: copy the check-out pattern in `show_drag_ghost` and `with_ghost`
    right above: `take()` the `InfoTip` out of the runtime, call it, put it
    back. Add a small `with_info_tip(&mut self, f)` helper like `with_ghost`.
  - Done when: no `with_runtime` closure in `runtime.rs` calls a method that
    sends messages. Behavior unchanged.

- [x] **A2. Add the missing `// SAFETY:` comments.**
  - Locations (from a heuristic scan; re-scan, there may be a few more or
    fewer): `win32/clipboard.rs` 132, 133, 135, 143, 146, 198;
    `win32/desktop_shell.rs` 46, 68, 80; `win32/dpi.rs` 8, 16;
    `win32/drop_target.rs` 299, 301; `win32/hotkey.rs` 27, 34;
    `win32/keyboard.rs` 197, 209, 229, 239, 246, 278.
  - Each comment states why the call's invariants hold, in one or two lines,
    in the style of the existing ones (for example "the hook must pass the
    event on" or "`hwnd` is a live window owned by this thread").
  - Also: `win32/drop_target.rs` ~384 uses `/// SAFETY:` on an `unsafe fn`;
    change it to a `/// # Safety` section. `win32/drag_image.rs` ~147 refers to
    `pixels.len()`, which does not exist; reword it to the real variable.
  - Comments only. No code changes.

- [x] **A3. Fix stale and misleading comments.** Comments only.
  - `deskspace-app/src/scene.rs` ~238: the doc for the scroll thumb ("A thin
    bar along the right edge...") sits on `auto_text_shadow`. Move it to the
    scroll-thumb function it describes, or delete it if that one already has
    a doc.
  - `deskspace-platform/src/edit.rs` line 1: says it is for renaming a space
    title; it also renames item labels.
  - `win32/info_tip.rs` ~123: `send` says "Posts"; it sends.
  - `win32/tray.rs` ~101: `shared_icon` says "A copy"; it returns the same
    handle.
  - `render/d2d.rs` ~621: mentions `render_item`; the function is
    `render_items`.
  - `win32/keyboard.rs` ~137: `stop` says "clears both modes"; it clears all
    three flags. Line ~8 lists the watched keys; add F2 and Ctrl+A. Line ~173
    `is_key`'s doc restates the code; delete it.
  - `deskspace-app/src/app.rs` ~179: the `edit` field says "The space being
    renamed inline"; it is a space title or an item label.

- [x] **A4. Explain the magic constants AGENTS.md calls out.**
  - `win32/desktop_shell.rs` ~9 `SPAWN_WORKER_MESSAGE = 0x052C`: add a short
    comment that this undocumented Progman message makes Explorer create the
    wallpaper WorkerW, and why the timeout value is what it is.
  - `win32/keyboard.rs` ~76 `VK_A_CODE = 0x41`: replace with
    `windows::Win32::UI::Input::KeyboardAndMouse::VK_A` and `is_key`.
  - `win32/hotkey.rs`: replace raw key codes (`0x70`, `0x20`, `0x09`,
    `0x0D`, `0x1B`) with `VK_F1`, `VK_SPACE`, `VK_TAB`, `VK_RETURN`,
    `VK_ESCAPE`; document `HOTKEY_BASE_ID`.
  - `win32/data_object.rs` ~192 `0x8007000E`: use `E_OUTOFMEMORY`.
  - Done when: existing hotkey parsing tests still pass unchanged.

- [x] **A5. Route space re-flow through a `Command`.**
  - Add `Command::Reflow { id }` in `deskspace-core/src/command.rs`. Its arm
    does exactly what `App::reflow_space` does today: for a locked space set
    `title_height` from the layout and call `position_items`; otherwise call
    `arrange_space`. Return `NotFound` for a missing id, else `Applied`.
  - In `App::reflow_space`, replace the `space_mut` block with
    `self.config.desktop.apply_with_layout(Command::Reflow { id }, layout,
    bounds)`. Get `bounds` the way `App::dispatch` does (look at it; use the
    space's center as the point).
  - Tests in `command.rs`: a locked space keeps its rect but gets the new
    title height; an unlocked auto-sized space resizes to fit.
  - Done when: `reflow_space` contains no `space_mut`. Update the Commands
    list in `CONTEXT.md` section 4.

- [x] **A6. Cancel after toggling "Match Explorer" must re-flow.**
  - Bug: `field::MATCH_EXPLORER` returns `Effect::Layout`, but
    `App::cancel_settings` only re-flows when `layout` or `icon_size`
    changed, so cancelling a match toggle leaves spaces on the wrong grid.
  - Fix: add `|| session.saved_theme.match_explorer !=
    self.config.theme.match_explorer` to the `relayout` condition.
  - Prefer a small pure helper `fn needs_relayout(saved: &Session, config:
    &Config) -> bool` in `settings.rs` with a test, used by `cancel_settings`.

## B. Small cleanups

Each is independent. None changes behavior.

- [x] **B1. Remove clearly dead code.** For each item, `grep` the whole
  workspace first; delete only if nothing outside its own tests uses it.
  - `deskspace-core/src/id.rs` `IdGenerator` (and its tests).
  - `deskspace-core/src/space.rs` `Space::required_size` and
    `Space::resize_to_fit` (duplicates of `arrange_space`).
  - `deskspace-core/src/desktop.rs` `Desktop::item_with_path` (the app uses
    `App::find_item_by_file`).
  - `deskspace-platform/src/error.rs` `PlatformError::NoDesktopWindow`.
  - Keep `Command::{MoveSpace, AutoArrange, SnapAll}`: the M3 Arrange
    submenu will use them.

- [x] **B2. Name the repeated clamp ranges.**
  - The ranges `50..=200` (icon size), `60..=160` (label size), `0..=24`
    (grid gap/padding), and `0..=64` (snap) are repeated in
    `deskspace-app/src/settings.rs` (`build_form`, `apply_field`),
    `deskspace-app/src/app.rs` (`icon_scale`), `deskspace-app/src/scene.rs`,
    and `render/d2d.rs`.
  - Add `ICON_SIZE_RANGE` and `LABEL_SIZE_RANGE` next to
    `OVERLAY_SIZE_RANGE` in `theme.rs`, and `GAP_RANGE` / `SNAP_RANGE` in
    `layout.rs`. Use them everywhere.

- [x] **B3. Drive the opacity and sort menus from one table each.**
  - `deskspace-app/src/app.rs`: the opacity values appear both in
    `space_appearance_menu`'s `OPACITIES` and in `show_space_menu`'s match;
    the sort ids are mapped both in `space_sort_menu` and in the match.
  - Move both tables to module-level `const`s. Build the menus from them, and
    in `show_space_menu` look the chosen id up in the table instead of
    matching each id.
  - Test: every table id is unique and does not collide with other `menu::`
    ids.

- [x] **B4. Merge `pick_space_tint` and `pick_space_text`.**
  - One `fn pick_space_color(&mut self, ui, id, at, which: SpaceColor)` with
    a small private `enum SpaceColor { Tint, Text }` choosing the current
    value and the `Command`.

- [x] **B5. One helper for "dispatch, repaint, schedule a save".**
  - `fn apply_to_space(&mut self, ui: &mut Ui, id: SpaceId, command: Command,
    at: Point) -> bool` in `app.rs`. Use it in `set_space_opacity`,
    `sort_space`, the `FIT` and `LOCK` arms, `toggle_rollup`, and wherever
    else the three-line pattern appears. Keep any extra step (like
    `self.scroll.remove(&id)` for `FIT`) at the call site.

- [x] **B6. Use the shared `wide` helper.**
  - `win32/info_tip.rs` ~69 re-implements UTF-16 conversion; use
    `crate::win32::system::wide`. Replace fully qualified
    `crate::win32::system::wide(...)` calls with a `use` at the top of each
    file.

- [x] **B7. Simplify the keyboard hook's pass-through.**
  - `win32/keyboard.rs` `hook_proc` calls `CallNextHookEx` in seven places.
    Move the body into `fn handle(code, wparam, event) -> bool` (true =
    consumed) and have `hook_proc` call `CallNextHookEx` once when it returns
    false. No behavior change; keep the comments that explain each branch.

## C. Features (ROADMAP M3, remaining items 1-4)

Do them in order. Each is one commit plus a `docs:` commit if the docs change
is large.

- [x] **C1. Per-space icon size (rest of F-25).** Superseded: a space's icon
  size is now part of its look (section H), and this field is folded into it
  on load. Kept for the record.
  - `Space.icon_size: Option<u8>`, a percent; `None` follows the global
    setting.
  - `Command::SetSpaceIconSize { id, icon_size: Option<u8> }`, clamped to
    `ICON_SIZE_RANGE` (from B2; if B2 is not done, use `(50, 200)` and leave a
    note).
  - Menu: an "Icon size" submenu in `space_appearance_menu` with "Default"
    (None), "Small" 75, "Medium" 100, "Large" 150, "Extra large" 200; the
    current one checked.
  - `App::space_scale(id)`: when the space has an override, return
    `percent / 100 * space_dpi(id.0)`, ignoring Explorer's icon size and the
    global setting. Leave the other paths unchanged.
  - After the command, call `reflow_space` (not just `refresh_space`) and
    `schedule_save`. `reset_space_appearance` also clears it.
  - Tests: command clamps and sets/clears; serde default is `None`.
  - **[needs human]** check: a Large space next to a Default space, both
    crisp, hit-testing correct.

- [ ] **C2. Windows accent color (F-29).** Follow "Recipe: a new global
  setting". (awaiting check)
  - Platform: `pub fn system_accent_color() -> Option<Color>` in
    `win32/system.rs`, reading `HKCU\Software\Microsoft\Windows\DWM` value
    `AccentColor` (a DWORD laid out `0xAABBGGRR`). Return `None` on any
    failure. Copy the registry-call style from `win32/desktop_entry.rs`. Add
    a pure, tested `fn color_from_abgr(u32) -> Color`. Re-export from
    `lib.rs`.
  - Core: `Theme.use_accent: bool` (default false), and
    `Palette::with_accent(self, accent: Color) -> Palette` that replaces
    `selected_fill` and `border` with the accent at their current alphas.
    Test it.
  - App: store `system_accent: Option<Color>` on `App`, read at start and on
    `Event::SystemThemeChanged` (Windows sends it when the accent changes).
    Wherever a scene palette is built (`refresh_space`, and the drag scene),
    apply `with_accent` when `use_accent` is on and an accent is known. Add
    one helper `fn palette(&self) -> Palette` and use it at every
    `self.config.theme.palette(self.system_dark)` call site.
  - Settings: a "Use Windows accent color" toggle on the Colors page,
    `Effect::Appearance`.
  - **[needs human]** check: selection and border pick up the accent, and
    change live when the accent changes in Windows Settings.

- [ ] **C3. Theme override for settings and menus (F-30).** Follow "Recipe:
  a new global setting". (awaiting check)
  - Core: `Theme.ui_mode: ThemeMode` (default `System`). It is separate from
    `Theme.mode`, which only colors the spaces.
  - App: `fn ui_dark(&self) -> bool` returning `self.config.theme.ui_mode
    .is_dark(...)`-style logic (reuse `Theme::is_dark`'s match; add a
    `ThemeMode::resolve(system_dark) -> bool` in core if that is cleaner, and
    make `Theme::is_dark` use it). Replace both `ui.set_ui_dark(self
    .system_dark)` calls in `on_event` with `ui.set_ui_dark(self.ui_dark())`.
  - Settings: a "Settings and menus" choice on the General page reusing the
    `MODES` table. In `on_settings_event`, after an `Effect::Appearance`, also
    call `ui.set_ui_dark(self.ui_dark())` so it previews live; Cancel must
    restore it (extend `cancel_settings`).
  - **[needs human]** check: switching to Dark/Light repaints the open
    settings window and the next popup menu.

- [ ] **C4. Start with Windows (F-34).** Follow "Recipe: a new global
  setting". (awaiting check)
  - Config: `WindowSettings.start_with_windows: bool` (default false).
  - Platform: `pub fn set_start_with_windows(enabled: bool) -> Result<()>`
    in a new `win32/autostart.rs`, writing or deleting value `DeskSpace`
    under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. The value is
    the quoted path of `std::env::current_exe()`. Copy registry handling and
    error mapping (`PlatformError::Registry`) from `win32/desktop_entry.rs`.
    Put the command-line string in a pure, tested function. Re-export from
    `lib.rs`.
  - App: apply it in `apply_settings` when the value changed, and once at
    startup when enabled (so the stored path follows a moved executable).
    Log failures with `tracing::warn!`; do not block Apply.
  - Settings: a toggle on the General page, `Effect::Pending`.
  - Tests must not touch the registry.
  - **[needs human]** check: the value appears/disappears in the Run key and
    DeskSpace starts at sign-in.

## D. Reserved (strong model or maintainer; do not start)

Listed so they are not attempted piecemeal. Each needs design work first.

- **D1. Split `deskspace-app/src/app.rs`** (~2,900 lines) into `app/`
  submodules, each with its own `impl App` block: `persist`, `view`, `hit`,
  `selection`, `visibility`, `rename`, `pointer` (drag), `info_tip`,
  `files`, `clipboard`, `menus`, `settings_session`. Move `app::run` into
  `bootstrap.rs`. Pure moves, one commit, no behavior change.
- **D2. Extract `Selection`, `PointerState`, and `EditState` from `App`.**
  After D1. Collapses the five copies of "select only this item" and the
  three copies of "forget this item everywhere".
- **D3. Platform module layout.** Move `shell.rs` under `win32/` and split it
  (desktop items, icon loading, pixel helpers); one icon decoder instead of
  two; a `win32/messages.rs` owning every `WM_APP + n`; generic helpers for
  `GWLP_USERDATA` window state and window-class registration; split
  `settings_window.rs` and `render/d2d.rs`; make internal modules private.
- **D4. Keep IO out of core.** `deskspace-core/src/item.rs` reads file
  metadata; move it to the app/platform and pass the values into
  `Command::AddItem`.
- **D5. M2 desktop takeover design.** Needs answers to ROADMAP open questions
  1 and 2.
- **D6. Undo design.** Needs an answer to ROADMAP open question 3.
- **D7. On-screen verification pass** (maintainer). The list is in
  `CONTEXT.md` section 2, "Pending on-screen checks".
- **D8. Auto-update design.** Resolved: ROADMAP D6. The work is E5 and
  E8-E11.
- **D9. Installer design.** Resolved: ROADMAP D6. The work is E5-E7.
- **D10. Code signing (maintainer, ROADMAP M8 item 3, D6).** Apply to
  SignPath Foundation (the project must meet their OSS terms: an OSI
  license, which Apache-2.0 is, and a public repository and build). Then add
  their GitHub Actions integration to `release.yml`: sign `deskspace.exe`
  before both the zip and `vpk pack`, and sign the Velopack `Setup.exe`
  (see `vpk pack -h` for the signing options). Store the credentials as
  Actions secrets. This must land before or with the first release that has
  E9 (auto-update), because unsigned updates trigger SmartScreen on every
  version.
- **D11. Windows 11 top-level context menu (ROADMAP M8 item 4).** Needs D10,
  and M2 may make it unnecessary.
- **D12. First release dry run (maintainer).** After E1-E6. Follow
  `RELEASING.md` with a pre-release version (for example `0.2.0-alpha.1`).
  Check:
  - the workflow publishes a pre-release with the zip, the `.sha256`, and
    the Velopack assets (`DeskSpace.App-win-Setup.exe`, the full `.nupkg`,
    `releases.win.json`);
  - on a clean Windows 10 VM and a clean Windows 11 VM: the extracted exe
    runs, shows its icon, and lists the version under Properties > Details;
    `Setup.exe` installs into `%LocalAppData%\DeskSpace.App` with no admin
    prompt and creates a Start menu shortcut;
  - after E7: uninstalling removes the install root, the Run value, and the
    desktop verb, and leaves the storage folder and the config untouched;
  - after E9: install `alpha.1`, publish `alpha.2`, and confirm that the
    manual tray check offers it and the update restarts into `alpha.2` with
    the same spaces;
  - what SmartScreen shows.

  Record the findings in `CONTEXT.md` section 10.
- **D13. winget (maintainer, ROADMAP M8 item 8).** After the first stable
  (non-pre-release) version that has the installer.
  - Submit the first manifest to `microsoft/winget-pkgs` by hand (`wingetcreate
    new` with the `Setup.exe` URL), with package identifier
    `generalchad.DeskSpace`.
  - Then add a `release.yml` job that runs only for non-pre-release tags and
    updates the manifest (`wingetcreate update --submit`, or the
    `winget-releaser` action), using a fine-grained PAT stored as an Actions
    secret.

## E. Release engineering (ROADMAP M8)

Do E1-E4 in order. E5-E11 have their own dependencies, stated in each task.
Code signing (D10), the first dry run (D12), and winget (D13) are maintainer
work. `.github/workflows/release.yml` already publishes a zip on a `v*` tag;
read it before starting.

- [ ] **E1. Commit an `.ico` asset and embed it, with version info, in the
  executable (ROADMAP M8 item 2).**
  - Asset: `assets/deskspace.ico` at the repository root. Generate it once
    from the existing drawing in `win32/icon.rs`, using `write_icon`'s ICO
    layout at sizes 16, 20, 24, 32, 40, 48, 64, and 256. Use a throwaway
    local change (for example a temporarily widened `FILE_SIZES` and a
    scratch test). Commit only the `.ico`, not the scratch code. From now on
    the `.ico` is the source of truth.
  - Build: add `winresource` as a build-dependency of `deskspace-app`
    (`rust_cargo-add` with `dependency_type = "build"`, so `Cargo.lock` stays
    current). In `crates/deskspace-app/build.rs`, inside the existing
    `windows` + `msvc` branch, compile a resource with
    `set_icon_with_id("../../assets/deskspace.ico", "1")`. `winresource`
    fills `FileVersion`, `ProductVersion`, and `ProductName` from Cargo
    metadata. Set `FileDescription` to "DeskSpace" and `LegalCopyright` to the
    line in the About page (`settings.rs`, `field::ABOUT_LICENSE`). Keep the
    manifest link args as they are. Add
    `cargo:rerun-if-changed=../../assets/deskspace.ico`.
  - If `rc.exe` is not found locally, or `cargo deny check` rejects
    `winresource` or its dependencies, **stop and ask**.
  - Do not change any runtime code; E2 does that.
  - Done when: `cargo build --release` produces a `deskspace.exe` that shows
    the icon in Explorer, and Properties > Details shows the workspace
    version. `cargo deny check` passes. **[needs human]** check: the icon at
    Explorer's small, medium, and extra-large views.

- [ ] **E2. Use the embedded icon at runtime and delete the drawn one.**
  After E1.
  - `win32/tray.rs`: build `tray_icon` / `shared_icon` from the resource
    with `LoadImageW(GetModuleHandleW(None), MAKEINTRESOURCE(1), IMAGE_ICON,
    cx, cy, LR_DEFAULTCOLOR)`. Use `GetSystemMetrics(SM_CXSMICON)` for the
    tray size. Keep the existing destroy-once rule (see `tray.rs` ~94).
    `settings_window.rs` ~247 already uses `shared_icon`; leave it unchanged.
  - `deskspace-app/src/app.rs` (the desktop "Show spaces" entry, ~620): drop
    the `write_icon` call and pass `"{exe}",0` as the icon. Delete a
    `deskspace.ico` left over beside the storage folder by older versions,
    ignoring errors.
  - Delete `win32/icon.rs`, the `write_icon` re-exports in `win32/mod.rs` and
    `lib.rs`, and the `TODO(deskspace)` there.
  - Update `CONTEXT.md` section 6 (icon) and section 10 (the "Show spaces"
    verb note that mentions `icon::write_icon`).
  - Done when: `grep -r write_icon crates` finds nothing. **[needs human]**
    check: the tray, the settings title bar, and the Shift+right-click desktop
    menu entry show the icon at 100 % and 150 % scaling.

- [ ] **E3. Reproducible release build (ROADMAP M8 item 5).**
  - `.github/workflows/release.yml`: on the Build step, set
    `RUSTFLAGS: --remap-path-prefix=${{ github.workspace }}=. --remap-path-prefix=C:\Users\runneradmin\.cargo=cargo`.
    Then confirm with `strings`/`Select-String` that no runner path is left
    in the exe.
  - Make the linker deterministic: in `crates/deskspace-app/build.rs`, inside
    the msvc branch, add `cargo:rustc-link-arg-bins=/Brepro`. It zeroes the
    PE timestamp and replaces it with a content hash.
  - Check locally: build twice, from two clones in different directories,
    with the same `--remap-path-prefix` (relative to each clone), and compare
    `Get-FileHash` of both `deskspace.exe`. If they differ, compare the two
    `dumpbin /headers` outputs, record what differs under **Found while
    working**, and **stop and ask**.
  - Done when: the two hashes match. Add one line on how to reproduce a
    release build to the "No installer, no code signing, no auto-update" note
    in `CONTEXT.md` section 10.

- [ ] **E4. Release checklist and versioning policy (ROADMAP M8 item 5).**
  - New `RELEASING.md` at the repository root. Link it from `AGENTS.md`
    (extend the existing "Release" row of the Commands table) and from
    `ROADMAP.md` M8 item 5. Add `release:` to the commit types in rule 7
    above. Contents:
    - Versioning: SemVer on `workspace.package.version`. While below 1.0, the
      minor version follows ROADMAP section 6 (v0.2, v0.3, ...) and patch
      versions are fixes. Pre-releases use `-alpha.N` / `-beta.N` / `-rc.N`,
      which `release.yml` marks as pre-releases. The config schema version
      (`CURRENT_VERSION`) is independent of the app version.
    - Checklist: `main` is green in CI; bump the version in `Cargo.toml`
      and refresh `Cargo.lock` (`cargo check --workspace`); move the
      finished items in `ROADMAP.md` / `CONTEXT.md`; commit as
      `release: vX.Y.Z`; `git tag vX.Y.Z && git push origin vX.Y.Z`; watch
      the run with `gh run watch`; download the zip, check the `.sha256`, and
      smoke-test it with a scratch `--config`; edit the generated notes on
      the release page.
    - Recovering from a bad release: delete the release and the tag
      (`gh release delete vX.Y.Z --cleanup-tag`), fix, and publish the next
      patch version. Never re-use a published version number.
  - Done when: the checklist matches `release.yml` step for step (the tag
    check, `--locked`, the asset names).

### Installer and auto-update (ROADMAP D6, F-45)

Read ROADMAP D6 before any of E5-E11; it is the spec, and these tasks only
say where things go. The Velopack Rust docs are thin. Confirm every API
named below on docs.rs for the pinned crate version and at
https://docs.velopack.io; if one differs, **stop and ask**.

- [ ] **E5. Add the `deskspace-update` crate and the Velopack startup hook.**
  - New library crate `crates/deskspace-update` with
    `#![forbid(unsafe_code)]`, `thiserror` errors, and `tracing`. It depends
    on `velopack` (`rust_cargo-add`) and on nothing in the workspace. Add it
    to `[workspace.dependencies]`, and make `deskspace-app` depend on it.
  - `pub const PACK_ID: &str = "DeskSpace.App";`, documented as "must match
    `--packId` in `release.yml`; never `DeskSpace`, see ROADMAP D6".
  - `pub fn startup()` wraps `velopack::VelopackApp::build().run()`. Call it
    as the **first** statement of `main` in
    `crates/deskspace-app/src/main.rs`, before `Cli::parse()`. Velopack
    starts the exe with its own arguments during install, update, and
    uninstall, and may exit the process; clap would reject those arguments.
    Add a comment saying so.
  - `pub enum InstallKind { Installed, Portable }` and `pub fn
    install_kind() -> InstallKind`: `Installed` when
    `velopack::UpdateManager::new(...)` succeeds for this process, else
    `Portable`. `cargo run` and the zip build are `Portable`. Log the kind at
    `debug` on startup.
  - Update `AGENTS.md` (workspace layout, dependency direction
    `app -> {config, platform, update} -> core`; `update` depends on no
    workspace crate) and `CONTEXT.md` section 3.
  - If `cargo deny check` rejects `velopack` or anything it pulls in,
    **stop and ask**; do not edit `deny.toml` on your own.
  - Done when: `cargo run -p deskspace-app -- --no-icons` still starts
    normally, and a test asserts `install_kind()` is `Portable` under
    `cargo test`.

- [ ] **E6. Package with Velopack in `release.yml`.** After E1 and E5.
  - After the Build step: `actions/setup-dotnet` (8.x), then `dotnet tool
    install -g vpk --version <the velopack crate version in Cargo.lock>`.
    The tool and the crate versions must match; read the crate version from
    `cargo metadata` in the step instead of hard-coding it.
  - `vpk download github --repoUrl <repo> --token ${{ github.token }}
    --outputDir releases`, so `vpk pack` can build deltas. It must not fail
    the job when there is no previous Velopack release (the first one).
  - Stage `deskspace.exe`, `LICENSE`, and `NOTICE` into a directory and run
    `vpk pack --packId DeskSpace.App --packVersion <version> --packDir
    <stage> --mainExe deskspace.exe --packTitle DeskSpace --packAuthors
    "Timothy W. Brown" --icon assets/deskspace.ico --noPortable --outputDir
    releases`. `--noPortable` is required: D6 says the portable build is our
    own zip, which only notifies and never self-updates.
  - Keep the existing `gh release create` (zip and `.sha256`), then add the
    Velopack assets to that same release with `vpk upload github --repoUrl
    <repo> --token ${{ github.token }} --tag <tag> --merge`, plus `--pre` for
    pre-release tags.
  - Check every flag against `vpk pack -h` / `vpk upload github -h` for the
    pinned version, and **stop and ask** if one is missing or renamed.
  - Update `RELEASING.md` (the asset list and the pack id warning) and the
    `CONTEXT.md` section 10 note.
  - Done when: the workflow passes `actionlint` (or a careful read if it is
    not installed). The real check is D12. **[needs human]**

- [ ] **E7. Uninstall cleanup and an install-root safety test.** After E5;
  the Run-key part needs C4.
  - Find the argument Velopack passes the main exe on uninstall (the
    "hooks" / fast-callback docs and the crate's `app.rs`; the C# SDK calls
    it `--veloapp-uninstall`). In `main.rs`, before `startup()`, if that
    argument is present: call `deskspace_platform::set_desktop_show_entry(
    false, "", "", "")` and (if C4 is done) `set_start_with_windows(false)`,
    log any failures at `warn`, and return. Do not touch the config or the
    storage folder (D6). If the SDK exposes a proper uninstall hook instead,
    use that. If neither exists, **stop and ask**.
  - Put the argument check in a pure, tested function (`fn
    is_uninstall_hook(args: &[String]) -> bool`).
  - Safety test in `deskspace-app` (it sees both `config` and `update`):
    using `ConfigStore::discover()`, assert that neither the config path nor
    the storage path is inside `%LOCALAPPDATA%\<PACK_ID>`. Add a pure helper
    `fn is_inside(path, root) -> bool` with its own tests (case-insensitive,
    and `DeskSpace.App` is not inside `DeskSpace`).
  - Done when: both tests pass. **[needs human]** check during D12: uninstall
    removes the Run value and the verb and leaves `%LOCALAPPDATA%\DeskSpace`
    intact.

- [ ] **E8. Cross-thread wake-up for the app (platform).** Independent of
  E5-E7.
  - The update check (E10) runs on a worker thread and has to hand its
    result to the UI thread. Add a generic mechanism that knows nothing
    about updates:
    - `#[derive(Clone)] pub struct Waker` (`Send + Sync`), obtained from
      `Ui::waker()`. `Waker::wake(&self, id: usize)` posts
      `WM_APP + 10` (next free; the ids are in `controller.rs`,
      `keyboard.rs`, and `outside_click.rs`) to the controller window with
      `id` in `wparam`.
    - The controller's window procedure dispatches it as a new
      `Event::Wake { id: usize }`, documented like `Event::Timer`.
  - Copy the cross-thread `PostMessageW` pattern and its `// SAFETY:`
    wording from `win32/console.rs`. A wake after the window is gone must
    just fail silently.
  - The app pairs it with a `std::sync::mpsc` channel it owns; the platform
    carries only the id.
  - Done when: `Event::Wake` is in `CONTEXT.md` section 6 next to the timer
    events. No behavior change.

- [ ] **E9. A three-button question dialog (platform).** Independent.
  - `Ui::choose(&mut self, title: &str, text: &str, buttons: &[&str],
    default: usize) -> Option<usize>`: a modal `TaskDialogIndirect` owned by
    the controller window, with custom buttons (`TDF_USE_COMMAND_LINKS` off).
    It returns the index of the button pressed, or `None` if the dialog was
    closed. The Common Controls v6 manifest in `build.rs` makes
    `TaskDialogIndirect` available.
  - Follow `Ui::confirm` (`runtime.rs` ~574) for ownership, foreground
    handling, and not holding a runtime borrow across the modal call.
  - Link the `TaskDialogIndirect` page on Microsoft Learn beside the call.
  - Done when: it compiles and clippy is clean. **[needs human]** check: a
    temporary call shows three buttons and returns the right index (remove
    the temporary call before committing).

- [ ] **E10. Update settings, schedule, check, and prompt (F-45).** After
  E5, E8, and E9.
  - Config, following "Recipe: a new global setting", all in
    `WindowSettings` with `#[serde(default)]`:
    `check_for_updates: bool` (false), `update_prereleases: bool` (false),
    `last_update_check: Option<u64>` (Unix seconds),
    `skipped_version: Option<String>`. Serde default tests for each.
  - Settings: on the General page, a "Check for updates automatically" toggle
    and an "Include pre-releases" toggle, both `Effect::Pending`. Under them,
    a note saying the check contacts GitHub, and only when enabled or when
    started from the tray.
  - `deskspace-update`:
    - `pub struct Available { version: String, notes: String, page_url:
      String }`.
    - `pub fn check(prerelease: bool) -> Result<Option<Available>,
      UpdateError>`. Installed: `UpdateManager` with
      `sources::GithubSource::new(env!("CARGO_PKG_REPOSITORY"), None,
      prerelease)`, then `check_for_updates()`. Portable: query
      `https://api.github.com/repos/<owner>/<repo>/releases` with the HTTP
      client Velopack already pulls in (`ureq`, same major version, so there
      is no second copy in `Cargo.lock`), and pick the newest release
      newer than `CARGO_PKG_VERSION`.
    - Keep the choice in pure, tested functions: `fn newest(releases:
      &[Release], current: &Version, prerelease: bool) -> Option<&Release>`,
      parsed from fixture JSON in the test.
    - Blocking is fine here; the app calls it on a worker thread.
  - App (`app.rs`):
    - `UPDATE_TIMER = 4`. On `Started`, when `check_for_updates` is on, arm
      it for 60 s, then hourly. When it fires and `fn check_due(last:
      Option<u64>, now: u64) -> bool` (24 h, tested) is true, start a check.
    - A check runs on `std::thread::spawn`, sends the result over the
      channel, and calls `waker.wake(UPDATE_WAKE)`. On
      `Event::Wake { id: UPDATE_WAKE }`, take the result, set
      `last_update_check`, and `schedule_save`. Keep one check in flight at
      a time (an `update_in_flight` flag).
    - Tray: a "Check for updates…" entry (new `menu::CHECK_UPDATES`, next
      free id) above the separator in `show_tray_menu`, always shown. A
      manual check ignores `check_for_updates` and `skipped_version`, and
      reports "DeskSpace is up to date" or the error through `ui.choose`
      with one "OK" button.
    - On an available version (automatic checks skip `skipped_version`),
      show `ui.choose` with the version and the first ~600 characters of the
      notes:
      - Installed: "Update and restart" / "Later" / "Skip this version".
      - Portable: "Open release page" / "Later" / "Skip this version".
      - "Skip" stores `skipped_version`. "Later" does nothing. "Open release
        page" opens `page_url` (check whether `ui.open_path` handles a URL;
        if not, add `Ui::open_url` with `ShellExecuteW`).
      - "Update and restart" calls the E11 entry point. Until E11 lands,
        open the release page there and leave a `// TODO(deskspace): E11`.
  - Done when: the tests above pass and nothing contacts the network with
    the default config (check by reading the code path, then note it in the
    commit body). **[needs human]** check: a manual tray check on a portable
    build against a repo with a newer pre-release, with "Include
    pre-releases" on.

- [ ] **E11. Download, apply, and restart (installed builds).** After E10.
  - `deskspace-update`: `pub fn download(&Pending) -> Result<(), _>` (call
    it on a worker thread) and `pub fn apply_and_restart(&Pending) -> !`
    (or a `Result`, if the SDK returns one), wrapping `download_updates` and
    `apply_updates_and_restart`. `Pending` holds the Velopack `UpdateInfo`
    from the E10 check, so the app does not check twice.
  - App: "Update and restart" starts the download on a worker thread (same
    channel and `Waker` as E10, with a new wake id), and sets the tray
    tooltip to "Downloading update…" with `ui.set_tray_tooltip`. When the
    download finishes:
    - flush the configuration synchronously, the same way the app does on
      `CloseRequested` (a pending autosave must not be lost);
    - unregister the hotkey;
    - call `apply_and_restart`.
    On failure, restore the tooltip and report the error with `ui.choose`.
  - Keep Velopack's default `set_auto_apply_on_startup(true)`: an update that
    finished downloading but did not restart is applied on the next launch,
    which matches D6 ("apply on restart").
  - Remove the E10 `TODO`. Mark F-45 "Done; check pending" in the ROADMAP
    feature index, and update `CONTEXT.md` (sections 6, 7, and 10).
  - **[needs human]** check during D12: an `alpha.1` install updates to
    `alpha.2` and comes back with the same spaces and settings.

## F. Ad-hoc features

Small requests, one commit each.

- [x] **F1. F2 renames the space title when no item is selected.** The
  selection-key watch (`win32/keyboard.rs`) now watches F2 whenever a space is
  current, and `App::begin_f2_rename` renames the focused item, or the current
  space's title (`begin_rename`) when the selection is empty. **[needs human]**
  check: click a space with no item selected and press F2 (title rename); with
  an item selected, F2 still renames the file.
- [x] **F2. Alt+Enter opens item Properties or DeskSpace Settings.** Added
  `Event::ShowProperties`, `Ui::show_properties` (the shell `properties` verb
  through `ShellExecuteExW`), and the `Alt+Enter` watch. Alt+Enter opens the
  focused item's Properties, or DeskSpace Settings when no item is selected.
  **[needs human]** check: with an item selected, Alt+Enter shows its
  Properties; with a space current and nothing selected, it opens Settings.
- [x] **F3. Enter opens the focused item.** Added the `SELECTION_KEY` message
  and `SelectionKey` event (with a `keyboard::set_menu_open` guard so an open
  popup keeps its own keys). **[needs human]** check: select an item and press
  Enter; it opens.
- [x] **F4. Escape clears the selection.** **[needs human]** check: with items
  selected, Escape clears them; Escape still closes an open menu.
- [x] **F5. Shift+Delete deletes permanently.** `Ui::delete_files` and
  `file_op::delete` take a `permanent` flag that drops `FOF_ALLOWUNDO`.
  **[needs human]** check: Shift+Delete does not go to the Recycle Bin.
- [x] **F6. F5 refreshes the spaces.** Re-reads Explorer's display options and
  re-flows every space.
- [x] **F7. The Menu key or Shift+F10 opens the context menu.** Anchored at the
  focused item's cell center, or the space center with no item selected.

## G. More Explorer-standard controls

Remaining gaps from the controls review. Do them in order.

- [x] **G1. Type-to-select, and Home/End/Page scroll.** Typing a letter moves
  the selection to the next item whose name starts with it (a short timeout
  buffer, as Explorer uses); Home/End/PageUp/PageDown scroll the space. Extend
  the selection-key watch and add a pure prefix-match helper with tests.
  Landed as `feat: type-to-select and Home/End/Page scrolling (G1)`.
  **[needs human]** check: on the desktop shell, type a letter to jump to an
  item and watch the buffer expire and start fresh; press Home/End/PageUp/
  PageDown over a scrolled space.
- [x] **G2. Ctrl+wheel changes the icon size.** Add `ctrl` to
  `SpaceEvent::Wheel` and adjust the icon size the way Explorer's desktop does,
  clamped to `ICON_SIZE_RANGE`. Landed as
  `feat: change the icon size with Ctrl+wheel (G2)`. The standard behavior was
  chosen: it changes the global `Theme.icon_size` (10 % a notch), so spaces
  that follow Global resize together while a customized space keeps its own
  size; it re-lays out the spaces and saves. **[needs human]** check: Ctrl+wheel
  over a space resizes the icons of every non-customized space, clamped at
  50 %/200 %.
- [x] **G3. Arrow-key selection navigation.** Grid-aware Left/Right/Up/Down with
  Shift to extend and Ctrl to move focus, scroll the focused item into view, and
  draw a focus rectangle. Needs its own movement model; the largest item here.
  Landed as `feat: arrow-key selection navigation (G3)`. The hook posts
  `NAV_KEY` (yielding `Event::Navigate`); `next_in_direction` (a pure helper
  with tests) picks the nearest item by primary-axis distance, tie-broken by the
  cross axis; the app moves the selection, extends it (Shift, in item order like
  Shift+click), or moves a `focused` item (Ctrl); `SpaceScene.focused_item`
  draws a dotted focus rectangle. **[needs human]** check: with the desktop
  shell foreground and a space holding a few items, arrow around the grid,
  Shift+arrow to extend, Ctrl+arrow to move only the dotted focus rectangle and
  scroll it into view.
- [ ] **G4. Slow double-click renames.** Clicking an already-selected item's
  label again starts an inline rename, as Explorer does.
- [ ] **G5. Right-button drag.** A right drag shows the copy/move/create-shortcut
  menu on drop.
- [ ] **G6. Ctrl+Z undo.** Design first; see ROADMAP D6.

## H. Per-space looks in the settings window (ROADMAP M3, F-46)

The settings window has three panes: the **scopes** (Global, then every
space indented under it, dimmed while it follows Global), the **pages** of the
selected scope, and the **page**, under a header with the scope's name and,
for a space, a "Customize this space" switch. A space shows only the look
pages (Appearance, Labels, Layout, Colors, Advanced), greyed with the global
values until the switch is on. CONTEXT.md sections 4 and 6 describe it.

Design rules (decided 2026-10-10, replacing the first rail design the same
day; do not re-open without asking):

- **A customized space has its own copy of every look setting** (`SpaceLook`:
  the theme plus icon spacing and edge padding), seeded from Global the first
  time. Turning the switch off keeps the copy, and turning it on again
  restores it.
- **The page shape never changes.** All seven pages are always built; the app
  chooses which ones the pages pane lists and fills them with the scope's
  values, so the in-place update machinery keeps working.
- **The platform knows nothing about spaces.** It draws the scopes, pages,
  and header from `SettingsNav` and reports `ScopeChanged`, `NavChanged`, and
  `HeaderSwitch`; the app maps indices back to spaces.
- **Space edits go through `Command`s** (`SetSpaceLook`, `CustomizeSpace`);
  `settings.rs` stays pure and returns them. Global edits write `Config`
  directly, as before.
- **Context-menu edits commit immediately.** Each one customizes the space,
  starting from what it shows now. If one lands while the window is open, it
  also updates that space's Cancel snapshot, so Cancel never undoes it.
- ROADMAP M3's planned page of defaults for new spaces must be titled "New
  spaces", so it is not confused with the scopes pane.

- [x] **H1-H3. The first design (a rail with a Spaces group of customized
  spaces and one shared Space page with four overrides).** Built, then
  replaced by the three-pane design above at the user's request, because the
  Spaces entries read as more global pages. Commits `08c6ec9`..`a35ee61`,
  replaced by `283b7fb` and `38eac47`.
- [ ] **H5. Three panes, the Customize switch, and per-space looks.**
  (awaiting check) Landed as `283b7fb` (platform) and `38eac47` (model and
  app). **[needs human]** check, with two spaces and the window open on a
  high-contrast background (`--on-top`):
  - light and dark mode: the panes, the hairlines, the dimmed rows, and Tab
    and the arrow keys moving through both panes;
  - a space greyed until "Customize this space" is ticked; an edit then
    changes that space alone, live; Cancel puts it back;
  - turning the switch off and on again restores the space's custom values;
  - a menu edit (for example Opacity 60%) with the window open customizes the
    space, un-dims its row, and survives Cancel;
  - an older config with per-space `icon_size`/`tint` opens with that space
    customized and looking the same.

- [ ] **H4. Flash the selected space.** After H5. (awaiting check)
  - `render/scene.rs`: add `SpaceScene.highlight: bool` (default false).
    When it is set, `d2d.rs` draws a 2 px outline just inside the frame,
    following the corner radius.
  - Color: the space's `palette_for(id).selected_fill` at full alpha, so it
    follows the Windows accent when the space uses it.
  - App: `highlighted: Option<SpaceId>`, and `HIGHLIGHT_TIMER = 5` (E10
    reserves 4 for `UPDATE_TIMER`; `PREVIEW_TIMER` is 6) at about 1200 ms.
    `SettingsEvent::ScopeChanged` to a space, and `open_settings_with` on a
    space, start it: set the flag and refresh that space. The timer clears it
    and refreshes again. Selecting another space moves the highlight. Cancel,
    OK, and closing the window clear it.
  - Done when: a scene test shows the flag reaching the scene (it does,
    `the_settings_highlight_reaches_the_scene`). **[needs human]** check: a
    space behind other windows is hard to see, so check with `--on-top` that
    the outline is visible on Acrylic (checked, visible), Clear, and Solid
    backdrops.

## Found while working

Add one line per issue noticed but not fixed: file, symptom, suggested task.

- `deskspace-app/src/app.rs` `cancel_settings` decides `restyle` with
  `saved_theme != self.config.theme`, which now includes `ui_mode`. Cancelling
  a change that only touched `ui_mode` repaints every space for nothing.
  Harmless; compare the colors that matter, or exclude `ui_mode`.
- `deskspace-platform/src/win32/controller.rs` handles `WM_SETTINGCHANGE`
  with "ImmersiveColorSet" as `Event::SystemThemeChanged`, which is what
  refreshes the accent. Windows also broadcasts
  `WM_DWMCOLORIZATIONCOLORCHANGED` when the accent changes; if
  "ImmersiveColorSet" is not sent for every accent change, the palette is
  stale until the next theme change. Watch for it during the F-29 check.
- `deskspace-core/src/space.rs` `SpaceLook`: values are clamped when a
  command sets them (`SpaceLook::clamped`), but an out-of-range value in a
  hand-edited `look` is only clamped where it is used (icon size in
  `App::icon_scale`), so the menu shows no preset checked. The global theme
  has the same gap. Consider clamping both on load (a `fix:` task).
- `deskspace-platform/src/win32/runtime.rs` `Ui::update_settings`:
  `settings_window::update` returns `false` when the new form has a different
  shape, but `update_settings` drops that result, and `App::refresh_settings`
  cannot see it. A shape-changing form is silently not shown. No form changes
  shape today, and H is designed to keep it that way. Still, either rebuild
  the window on `false` or `debug_assert!` against it (a `fix:` task).
- `ROADMAP.md` D3 says the store is `%LOCALAPPDATA%\DeskSpace\storage`, but
  `ConfigStore::discover` uses `ProjectDirs::from("", "DeskSpace",
  "DeskSpace")`. On Windows that should give
  `%LOCALAPPDATA%\DeskSpace\DeskSpace\data\storage`, and the config under
  `%APPDATA%\DeskSpace\DeskSpace\config\`. Confirm by running it, then fix
  the docs (a `docs:` task). This matters for E7's safety test and the
  uninstall docs.
