# AGENTS.md

Guidance for automated agents and contributors working in this repository.
The project documents live in `docs/`: read `docs/CONTEXT.md` for the
architecture and current state first, then `docs/ROADMAP.md` for the plan and
`docs/TASKS.md` for ready-to-do work items (section V lists what still needs
an on-screen check). `docs/archive/` holds finished tasks and the old progress
log; it is history, so skip it unless a task points there.

## Project

DeskSpace is a free and open-source desktop space organizer for Windows,
written in Rust with native Win32.
The workspace is Windows-only: `deskspace-platform` links the `windows` crate
under `cfg(windows)`, so the workspace does not build on other platforms.

## Prerequisites

- Windows 10 or 11 (x64)
- Rust with the MSVC toolchain. `rust-toolchain.toml` pins the exact stable
  version, which rustup installs automatically (with the components and
  targets it lists). `Cargo.toml` sets `rust-version = "1.88"` as the minimum
  supported version, a separate fact that CI checks with its own job; raising
  it is a deliberate `ci:` commit.
- No other system dependencies

To bump the toolchain, change the version in `rust-toolchain.toml` alone, fix
any new clippy lints in the same commit, and commit as
`ci: bump the Rust toolchain to 1.x.y`.

## Commands

| Task   | Command |
|--------|---------|
| Build  | `cargo build --workspace` |
| Run    | `cargo run -p deskspace-app -- --no-icons` |
| Inspect| `cargo run -p deskspace-app -- --config <scratch.toml> --on-top --settings` |
| Test   | `cargo test --workspace` |
| Lint   | `cargo clippy --workspace --all-targets` |
| Format | `cargo fmt --all` |
| Deps   | `cargo deny check all` (licenses/advisories, per `deny.toml`) |
| Release| `cargo build --release`; the checklist is in `RELEASING.md` |
| Dev loop | `./scripts/dev.ps1` (see below) |

`scripts/dev.ps1` stops any DeskSpace running from `target\` before building,
because the running executable is locked, then runs the app with a scratch
config at `%TEMP%\deskspace-dev\deskspace.toml`. Switches: `-Release`,
`-Fresh` (delete the scratch config), `-RealConfig`, `-OnTop`, `-Settings`,
`-NoIcons`, `-Check` (fmt, clippy `-D warnings`, tests first), `-NoRun`,
`-Verbosity 0..2`; anything after `--` is passed to the app.
`Get-Help ./scripts/dev.ps1` has examples.

Always run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
warnings`, and `cargo test --workspace` before considering a change complete.
CI (`.github/workflows/ci.yml`) treats clippy warnings as failures, checks
the build on the minimum supported Rust, and runs `cargo deny check all`.

## Design priorities

Readability, maintainability, and extensibility come first, ahead of
cleverness and ahead of saving a few lines. DeskSpace is built to be
extended feature by feature (see the ROADMAP feature index) by contributors
and models of varying strength, so every change should leave the code easier
to read and to extend than it found it.

- **Readable.** A reader should understand a function without scrolling or
  jumping between files. Prefer small functions with descriptive names, early
  returns, and named constants over magic numbers. Keep one concept per
  module; when a file passes roughly 800 lines of non-test code, split it
  along its existing seams (see `docs/TASKS.md` section K) rather than adding to it.
- **Maintainable.** One source of truth per fact: tables drive menus
  (`OPACITY_PRESETS`, `SORT_PRESETS`), option lists (`(value, label)` pairs
  in `settings/tables.rs`), and hotkeys (`HOTKEY_BINDINGS`), so adding an
  entry is one row, not edits in several places. Where a table cannot carry
  the behavior, an enum does (`ButtonAction`, `HotkeyAction`), so the
  compiler or a guard test points at every place still to wire up; the
  `docs/TASKS.md` recipes list them. Delete dead code and dead branches instead of keeping
  them "just in case". Keep pure logic (core, `settings.rs`, `scene.rs`,
  helpers such as `next_in_direction`) separate from Win32 calls, so it can be
  unit-tested. When code and docs disagree, fix the docs in the same commit.
- **Extensible.** New behavior plugs into the existing extension points
  instead of special-casing: a `Command` variant for model changes, an
  `Event`/`SpaceEvent` variant for new input, a field on `SpaceScene` /
  `SettingsForm` for new UI, a `#[serde(default)]` field for new settings,
  and a `Surface` for anything that should work on both spaces and the
  desktop layer. `docs/TASKS.md` has a recipe for each. If a feature does not fit
  any extension point, stop and design one rather than bolting it on.
- **Consistent.** Copy the nearest existing pattern (naming, error handling,
  module layout, test style) instead of inventing a new one. Ids that other
  code or docs cite (`F-xx`, `Mx`, `Dx`, task ids, `field::` ids, timer and
  message ids) are never renumbered.

## Tooling

A bundled `rust` MCP server exposes the cargo workflow as tools. Prefer these
over a raw shell, so the same flags (including `--locked`) are applied
everywhere.

| Tool | Covers |
|------|--------|
| `cargo-build`, `cargo-check` | build and fast compile check |
| `cargo-test` | tests (`workspace: true` runs every package) |
| `cargo-clippy` | lint (`all_targets: true`; CI adds `-D warnings`) |
| `cargo-fmt` | format (`check: true` verifies without writing) |
| `cargo-deny-check` | dependency policy from `deny.toml` |
| `cargo-machete` | find unused dependencies |
| `cargo-add`, `cargo-remove` | edit `Cargo.toml` and refresh `Cargo.lock` |
| `cargo-metadata`, `cargo-tree`, `workspace-info` | inspect the dependency graph |
| `cargo-doc` | build rustdoc |
| `rustc-explain`, `rustup-show`, `rustup-update` | error codes and toolchains |

The remaining cargo subcommands (`clean`, `update`, `package`, `expand`,
`hack`, `insta`, ...) are exposed as their own tools when a task needs them.
A missing subcommand can be installed first with the matching helper
(`cargo-deny-install`, `cargo-hack-install`, `cargo-machete-install`). These
tools pass `--locked`, so keep `Cargo.lock` current first (see Gotchas).
Release work (`RELEASING.md`; `docs/TASKS.md` R10-R13) also uses `gh`, `actionlint`, and
`cargo-packager` (the NSIS installer, installed with `cargo install`).

## Workspace layout

```
crates/deskspace-core      Domain model. No OS dependencies and no IO.
crates/deskspace-config    Serde/TOML configuration schema and persistence.
crates/deskspace-update    Update checking and installation (ROADMAP F-45);
                           wraps `cargo-packager-updater` and has no workspace
                           dependency.
crates/deskspace-platform  UI runtime, space windows, DWM, Direct2D painter,
                           tray, menus, settings window, shell, hotkeys.
crates/deskspace-app       The `deskspace` binary (CLI, the `App` event
                           handler, scene and settings-form builders, managed
                           storage) and the `deskspace-watchdog` helper binary.
```

`docs/TASKS.md` opens with a file map: where each kind of code lives, so a task
can name a file instead of a search.

Dependency direction: `app -> {config, platform, update} -> core`. `core` depends on
nothing in the workspace. Do not introduce upward dependencies.

## Architectural rules

- `deskspace-core` and `deskspace-config` use `#![forbid(unsafe_code)]`. Keep
  OS and IO concerns out of `core`.
- All `unsafe` and Win32 code lives in `deskspace-platform`.
- Mutate the domain only through `Command` and `Desktop::apply`. Add a new
  `Command` variant instead of mutating `Desktop` directly from UI or platform
  code.
- Rendering and UI are data-driven. `deskspace-app` converts the model into a
  `SpaceScene` / `SpaceStyle` per space, menus into `MenuEntry` lists, and
  settings into a `SettingsForm`; the platform only turns that data into
  windows, controls, and pixels. Keep Win32 types out of `core` and the app.
- Push scenes only when something changed. Space windows repaint from their
  stored `PreparedScene`; there is no per-frame callback into the app.
- Persistent types derive `serde` and mark new fields `#[serde(default)]` for
  forward compatibility. For breaking changes, bump `CURRENT_VERSION` and
  extend `Config::migrate`.
- Library crates return `thiserror` errors; the binary boundary uses `anyhow`.
- Use `tracing` for diagnostics. Do not log at `info` for per-frame or noisy
  paths; use `debug` or `trace`.
- Tests are colocated in each module as `#[cfg(test)] mod tests`.

## Code style

- Rust edition 2024 with rustfmt defaults.
- Prefer clear names and small functions over explanatory comments. See
  Comments below.
- Avoid allocation in the per-frame paint path.
- Keep public APIs documented with `///` only where behavior is non-obvious.

## Commits

Commit after every change; never leave finished work uncommitted. Keep each
commit one logical unit of work, and split unrelated edits into separate
commits - do not batch them. When working from `docs/TASKS.md`, make one commit per
task (see its "How to work a task", rule 7) and tick the task's box in that
same commit. Follow the message style there: `feat: ...`, `fix: ...`,
`refactor: ...`, `docs: ...`, `test: ...`, `perf: ...`, `ci: ...`,
`release: ...`, imperative and lowercase. Run `cargo fmt --all`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace` before committing.

## Comments

Write code that reads on its own, and add a comment only when it supplies
information the code cannot. Code tells the reader how; comments tell the reader
why. Judge comments by quality, not quantity: a bad comment is worse than no
comment, because it adds clutter, costs time to read, and becomes wrong faster
than the code it describes. The following adapts the nine rules from "Best
practices for writing code comments" (Stack Overflow, 2021) to this repository.
See the source when a rule needs more nuance:
https://stackoverflow.blog/2021/12/23/best-practices-for-writing-code-comments/

1. Do not duplicate the code. If a comment restates the line below it, remove
   it. No per-line narration and no closing-brace comments.
2. Do not use comments to excuse unclear code. Rename the variable or split the
   function instead. If a better name removes the need for the comment, do that.
3. If you cannot write a clear comment, the code is probably wrong. Rewrite it
   until the intent is obvious rather than papering over it.
4. Do not cause confusion. If a comment itself needs explaining, delete it.
5. Explain unidiomatic code. This codebase has plenty: manual `LOWORD`/`HIWORD`
   extraction, the `0x052C` WorkerW spawn trick, the undocumented accent
   policy, and raw `GWLP_USERDATA` pointers. Add a short note on why the code is the way it is,
   so nobody simplifies it away.
6. Cite copied code. Any snippet adapted from a forum, gist, or another project
   gets a comment with its URL. Stack Overflow content requires attribution.
7. Link external references where they are most useful. Put Microsoft Learn
   links beside the Win32 calls and magic constants they explain, and RFC or
   spec links beside the behavior they justify.
8. Comment bug fixes. Record the observed behavior and the reason for the fix,
   and reference the issue (`// see issue #1425`) so the workaround can be
   re-evaluated later.
9. Mark incomplete work. Use `// TODO(name):` or `// FIXME(name):` with a short
   description and, when one exists, an issue reference. Reuse existing markers
   instead of inventing new ones.

Rust specifics:

- `///` doc comments are positive documentation and are expected on public
  library items whose behavior is not obvious. Keep them concise.
- Every `unsafe` block carries a `// SAFETY:` comment stating why the invariants
  hold. This is the one comment that is never optional; the workspace lint
  `clippy::undocumented_unsafe_blocks` (root `Cargo.toml`) enforces it.
- A comment must be at least as correct as the code. When you change code,
  update or delete its comment in the same change.

## Gotchas

- The bundled MCP cargo tooling passes `--locked`. Keep `Cargo.lock` current
  when adding or updating dependencies.
- Window state lives behind raw pointers in `GWLP_USERDATA`, always through
  `win32/window_state.rs`: owned state is freed exactly once in
  `WM_NCDESTROY` (`window_state::free`), and a modal window's borrowed state
  is only detached there. Do not free it elsewhere.
- `deskspace_platform::run` takes the `EventHandler` and blocks on the message
  pump. The handler is checked out of the runtime while it runs; never hold a
  `with_runtime` / `with_state` borrow across a Win32 call that can send
  messages, and make `runtime::dispatch` the last statement in a window-proc
  branch, because the handler may destroy that window.
- Spaces are never activated. Anything that depends on activation does not
  work for them. In particular, `DWMWA_SYSTEMBACKDROP_TYPE` (Mica/Acrylic
  system backdrops) renders a flat color on inactive windows; the blur comes
  from the accent policy in `win32/dwm.rs`.
- Spaces sit at the bottom of the z-order, so they are hidden behind your
  editor. Use `--on-top` with a scratch `--config` to look at them.
- To check the look, run with a scratch `--config` and `--on-top` over a
  high-contrast window (bright stripes or text) and take a screenshot. A dark
  editor behind the space makes blur and flat translucency indistinguishable.
  Find window rects by enumerating the `DeskSpaceSpace` and
  `DeskSpaceSettings` classes. Remove the scratch config afterwards.
- Child windows get no DWM material or composited alpha. Desktop-hosted
  placement (`--placement desktop-*`, and the desktop layer when a host is
  found) is therefore opaque and uses a window region for rounding.
- `WS_EX_TRANSPARENT` only makes a window click-through when combined with
  `WS_EX_LAYERED`, and layering disables the blur. `--click-through` is a
  display-only mode.
- Launching the binary with a hidden show state (for example PowerShell
  `Start-Process -WindowStyle Hidden`) hides the settings window: Windows
  applies the startup show command to the first `ShowWindow` on an overlapped
  window.
- The keyboard hook (`win32/keyboard.rs`, `fn handle`) is the only way
  DeskSpace sees keys, because spaces are never activated. New key handling
  goes there, not in a window procedure. It has three flags: `EDITING`
  (inline rename; consumes every editing key, and an Alt or Win combination
  cancels the rename and passes through), `WATCH_SELECTION` (an item is
  selected), and `WATCH_PASTE` (a space is current; the name is historical).
  `Ui::watch_selection_keys` sets the last two. Which key needs which flag:
  - either watch flag: F2, Alt+Enter, Ctrl+A, Esc, F5, the Menu key, and
    Shift+F10;
  - `WATCH_SELECTION`: Enter, Delete, Shift+Delete, Ctrl+C, Ctrl+X;
  - `WATCH_PASTE`: Ctrl+V, the arrow keys (Shift extends, Ctrl moves the focus
    only), Home, End, PageUp, PageDown, and typed characters (type-to-select).

  The watch consumes keys only while the desktop shell has the foreground, so
  other apps keep their own shortcuts, and passes everything through while a
  popup menu is open (`keyboard::set_menu_open`). The hook is installed only
  while a flag is set.
- Dragging an item inside its space rearranges it (`ReorderItem`); only once
  the pointer leaves the space does the app start an OLE shell drag, which is
  what moves the real file.
