# ENGINEERING

The rules every change follows, whoever writes it. New here? Start with
"First 15 minutes" in `CONTRIBUTING.md`. `CONTEXT.md` describes the
architecture and current behavior, and `TASKS.md` has a file map and a
step-by-step recipe for each kind of change.

## 1. Setup and commands

You need Windows 10 or 11 (x64) and Rust with the MSVC toolchain; there are no
other system dependencies. The workspace is Windows-only: `deskspace-platform`
links the `windows` crate under `cfg(windows)`.

- `rust-toolchain.toml` pins the exact stable version, which rustup installs
  (with the components and targets it lists) on the first build. To bump it,
  change that file alone, fix any new clippy lints in the same commit, and
  commit as `ci: bump the Rust toolchain to 1.x.y`.
- `rust-version = "1.88"` in `Cargo.toml` is a separate fact, the minimum
  supported version, which CI checks with its own job. Raising it is a
  deliberate `ci:` commit.

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
| Dev loop | `./scripts/dev.ps1` (below) |
| On-screen checks | `./scripts/onscreen/smoke.ps1`, `shadows.ps1 [-Pixels]` (they import `scripts/onscreen/OnScreen.psm1`; `TASKS.md` section O) |

`scripts/dev.ps1` stops any DeskSpace running from `target\` (the running
executable is locked), builds, and runs the app with a scratch config at
`%TEMP%\deskspace-dev\deskspace.toml`. Switches: `-Release`, `-Fresh` (delete
the scratch config), `-RealConfig`, `-OnTop`, `-Settings`, `-NoIcons`, `-Check`
(fmt, clippy `-D warnings`, and tests first), `-NoRun`, `-Verbosity 0..2`;
arguments the script does not know (such as `--debug`) go to the app,
with or without a `--` before them. `Get-Help ./scripts/dev.ps1` has
examples.

**The gates.** Before a change is complete, and before every commit, run
`cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace`. CI (`.github/workflows/ci.yml`) fails on clippy
warnings, also builds on the minimum supported Rust, and runs
`cargo deny check all`.

## 2. Design priorities

Readability, maintainability, and extensibility come before cleverness and
before saving a few lines. DeskSpace grows feature by feature (the ROADMAP
feature index) through contributors and models of varying strength, so every
change leaves the code easier to read and extend than it found it.

Four rules come first, because the rest follow from them:

1. **Data structures first.** Get the types right and the code follows.
   Prefer a type that cannot hold a wrong state over code that checks for
   one; prefer returning what changed over making each caller remember what
   to redo.
2. **No special cases.** A branch for one kind of thing means an abstraction
   is missing. `Surface` removed the space-versus-desktop branches; do the
   same when a new `if` appears for "the other kind".
3. **Never break users.** A config that loaded yesterday loads today
   (`#[serde(default)]`, `Config::migrate`, clamping on load); stored files
   are never lost or overwritten; ids, CLI flags, and documented behavior do
   not change under the user. A regression is a bug even when the new
   behavior is "better"; changing on purpose is a recorded decision
   (`ROADMAP.md` section 2).
4. **The compiler enforces; docs explain.** If a rule can be a type, a
   private field, an enum, a lint, or a test, make it one and delete the
   prose. Docs keep only what code cannot say: why, Windows behavior,
   decisions, and the plan (section 7).

- **Readable.** A function should make sense without scrolling or jumping
  between files: small functions with descriptive names, early returns, named
  constants instead of magic numbers, one concept per module. When a file
  passes about 800 lines of non-test code, split it along its existing seams
  (`TASKS.md` section K) rather than adding to it.
- **Maintainable.** One source of truth per fact. Tables drive menus
  (`OPACITY_PRESETS`, `SORT_MENU_IDS`), option lists (`(value, label)` pairs in
  `settings/tables.rs`), and hotkeys (`HOTKEY_BINDINGS`), so a new entry is one
  row. Where a table cannot carry the behavior, an enum does (`ButtonAction`,
  `HotkeyAction`), so the compiler or a guard test points at every place still
  to wire up; the `TASKS.md` recipes list them. Delete dead code and branches
  instead of keeping them "just in case". Keep pure logic (core,
  `settings.rs`, `scene.rs`, helpers such as `next_in_direction`) apart from
  Win32 calls so it can be unit-tested. When code and docs disagree, fix the
  docs in the same commit.
- **Extensible.** New behavior plugs into an extension point instead of a
  special case: a `Command` variant for a model change, an `Event` /
  `SpaceEvent` variant for new input, a `SpaceScene` / `SettingsForm` field
  for new UI, a `#[serde(default)]` field for a new setting, and a `Surface`
  for anything that works on both spaces and the desktop layer. `TASKS.md` has
  a recipe for each. If a feature fits none, stop and design one rather than
  bolting it on.
- **Consistent.** Copy the nearest existing pattern (naming, error handling,
  module layout, test style) instead of inventing one. Ids that other code or
  docs cite (`F-xx`, `Mx`, `Dx`, task ids, `field::` ids, timer and message
  ids) are never renumbered.

## 3. Architectural rules

The crates and what each owns are in `CONTEXT.md` section 3.

- Dependencies point one way: `app -> {config, platform, update} -> core`.
  `core` depends on nothing in the workspace. Never add an upward dependency.
- `deskspace-core`, `deskspace-config`, `deskspace-update`, and
  `deskspace-app` use `#![forbid(unsafe_code)]`. Keep OS and IO concerns out of `core`.
- All `unsafe` and Win32 code lives in `deskspace-platform`.
- Mutate the domain only through `Command` and `Desktop::apply`. Add a
  `Command` variant instead of mutating `Desktop` from UI or platform code.
  The compiler enforces it: `Space`'s fields and `Desktop`'s spaces and
  mutators are private to `deskspace-core`, so other crates read through
  accessors. App tests build their model with `Command`s too
  (`deskspace-app/src/fixtures.rs`).
- Rendering and UI are data-driven. The app turns the model into a
  `SpaceScene` / `SpaceStyle` per space, menus into `MenuEntry` lists, and
  settings into a `SettingsForm`; the platform only turns that data into
  windows, controls, and pixels. Keep Win32 types out of `core` and the app.
- Push scenes only when something changed. Space windows repaint from their
  stored `PreparedScene`; there is no per-frame callback into the app.
- Persistent types derive `serde` and mark new fields `#[serde(default)]`.
  For a breaking change, bump `CURRENT_VERSION` and extend `Config::migrate`.
- Library crates return `thiserror` errors; the binary boundary uses `anyhow`.
- Use `tracing` for diagnostics, and `debug` or `trace` (never `info`) on
  per-frame or noisy paths.
- Tests sit in each module as `#[cfg(test)] mod tests`.
- Each crate tests on its own: `cargo test -p <crate>` passes with no
  other crate's tests, and a change re-tests only that crate and the
  crates that depend on it. So put a pure type or pure logic in the
  lowest crate that can hold it. In particular, keep it out of a crate
  that needs the `windows` crate, or every test that uses it pays for
  Win32. `deskspace-platform` still mixes the two; R20 plans the split.
- While working, test only what a change can affect with
  `./scripts/dev.ps1 -Check -Changed` (B31). The gates (section 1)
  still run on the whole workspace before every commit, and CI always
  does. Cargo caches builds, not test results, so skipping is the only
  saving.

## 4. Code style and comments

Rust edition 2024 with rustfmt defaults. Prefer clear names and small
functions over explanatory comments, avoid allocation in the per-frame paint
path, and document public APIs with `///` only where behavior is not obvious.

Code says how; a comment says why, and only when the code cannot. A bad
comment is worse than none: it clutters, costs reading time, and goes stale
faster than the code. The rules, adapted from "Best practices for writing code
comments" (Stack Overflow, 2021; see it for nuance:
https://stackoverflow.blog/2021/12/23/best-practices-for-writing-code-comments/):

1. Do not duplicate the code: no restating the next line, no per-line
   narration, no closing-brace comments.
2. Do not excuse unclear code: rename or split instead.
3. If you cannot write a clear comment, the code is probably wrong; rewrite it
   until the intent is obvious.
4. Do not cause confusion: a comment that needs explaining gets deleted.
5. Explain unidiomatic code, so nobody simplifies it away. This codebase has
   plenty: manual `LOWORD`/`HIWORD` extraction, the `0x052C` WorkerW spawn
   trick, the undocumented accent policy, raw `GWLP_USERDATA` pointers.
6. Cite copied code: a snippet adapted from a forum, gist, or another project
   gets its URL. Stack Overflow content requires attribution.
7. Link external references where they help most: Microsoft Learn beside the
   Win32 calls and magic constants they explain, RFCs and specs beside the
   behavior they justify.
8. Comment bug fixes: the observed behavior, the reason for the fix, and the
   issue (`// see issue #1425`), so the workaround can be re-evaluated.
9. Mark incomplete work as `// TODO(name):` or `// FIXME(name):` with a short
   description and an issue when one exists. Reuse existing markers.

Rust specifics: `///` doc comments are expected on non-obvious public library
items, kept concise. Every `unsafe` block carries a `// SAFETY:` comment saying
why the invariants hold; it is never optional, and the workspace lint
`clippy::undocumented_unsafe_blocks` (root `Cargo.toml`) enforces it. A
comment must be as correct as the code: change or delete it with the code.

## 5. Commits

Commit after every change; never leave finished work uncommitted. One logical
unit per commit; split unrelated edits. Working from `TASKS.md`, make one
commit per task and tick its box in that commit ("How to work a task", rule
7). Messages are imperative and lowercase with a type prefix: `feat:`,
`fix:`, `refactor:`, `docs:`, `test:`, `perf:`, `ci:`, `release:`. The body
says why; the diff already shows what. Run the gates (section 1) first.

## 6. Gotchas

- **Never wait on something only you can complete.** A canceled overlapped
  read finishes only when its packet leaves the I/O completion port, and
  only the worker's own wait takes it off. Waiting for it in
  `GetOverlappedResult` on that same thread hung the folder watch, and with
  it Ctrl+C and exit. Hand the buffer to the thread that receives the packet
  and bound every wait. A hung exit looks like an ignored signal; read the
  stacks first (`TOOLS.md` section 4).
- **Spaces are never activated**, so anything that depends on activation does
  not work for them. `DWMWA_SYSTEMBACKDROP_TYPE` (Mica/Acrylic system
  backdrops) renders a flat color on inactive windows; the blur comes from
  the accent policy in `win32/dwm.rs`.
- **Seeing a space.** Spaces sit at the bottom of the z-order, behind your
  editor; use `--on-top` with a scratch `--config`. To check the look, run over
  a high-contrast window (bright stripes or text) and take a screenshot: over a
  dark editor, blur and flat translucency look the same. `scripts/zorder.ps1`
  lists DeskSpace's windows with their rects and z-order, and which window
  a click at each space's or taskbar's center reaches; run it while a window
  will not take clicks or drops. Remove the scratch config afterwards.
- **Hidden launch hides settings.** Starting the binary with a hidden show
  state (for example PowerShell `Start-Process -WindowStyle Hidden`) hides the
  settings window: Windows applies the startup show command to the first
  `ShowWindow` on an overlapped window.
- **Window state** lives behind raw pointers in `GWLP_USERDATA`, always through
  `win32/window_state.rs`. Owned state is freed exactly once in `WM_NCDESTROY`
  (`window_state::free`); a modal window's borrowed state is only detached
  there. Do not free it anywhere else.
- **Re-entrancy.** `deskspace_platform::run` takes the `EventHandler` and
  blocks on the message pump. The handler is checked out of the runtime while
  it runs: never hold a `with_runtime` / `with_state` borrow across a Win32
  call that can send messages, and make `runtime::dispatch` the last statement
  in a window-procedure branch, because the handler may destroy that window.
- **Child windows** get no DWM material or composited alpha, so desktop-hosted
  placement (`--placement desktop-*`, and the desktop layer when a host is
  found) is opaque and rounded with a window region.
- **Click-through.** `WS_EX_TRANSPARENT` makes a window click-through only with
  `WS_EX_LAYERED`, and layering disables the blur, so `--click-through` is a
  display-only mode.
- **Dragging** an item inside its space rearranges it (`ReorderItem`); only
  once the pointer leaves the space does the app start an OLE shell drag,
  which is what moves the real file.
- **Keys.** The keyboard hook (`win32/keyboard.rs`) is the only
  way DeskSpace sees keys, because spaces are never activated, so new key
  handling goes there, not in a window procedure. This table is the one list
  of which key needs which flag (`fn watched_key` is the source of truth, and
  its `keys_for_*` tests check every row). The
  hook's flags: `EDITING` (inline rename; consumes every editing key, and an
  Alt or Win combination cancels the rename and passes through),
  `WATCH_SELECTION` (an item is selected), and `WATCH_PASTE` (a space is
  current; the name is historical), and `WATCH_BACK` (the current space is a
  lens shown below its own folder, D18). `Ui::watch_selection_keys` sets the
  last three.

  | Flag | Keys |
  |------|------|
  | either watch flag | F2, Alt+Enter, Ctrl+A, Esc, F5, the Menu key, Shift+F10, Ctrl+Plus, Ctrl+Minus, Ctrl+0 (zoom; main and keypad) |
  | `WATCH_SELECTION` | Enter, Delete, Shift+Delete, Ctrl+C, Ctrl+X |
  | `WATCH_PASTE` | Ctrl+V, Ctrl+Z, Ctrl+Y, Ctrl+Shift+1..8 (Explorer's views), the arrow keys (Shift extends, Ctrl moves the focus only), Home, End, PageUp, PageDown, typed characters (type-to-select) |
| `WATCH_BACK` | Backspace, Alt+Left, Alt+Up (the parent folder; otherwise they pass through, and the Alt arrows never move the selection) |

  Every other key passes through. The watch consumes keys only while the
  desktop shell has the foreground (`desktop_is_foreground`), so other apps
  keep their own shortcuts, and
  passes everything through while a popup menu
  is open (`keyboard::set_menu_open`). The hook is installed only while a flag
  is set.

## 7. Documentation

Docs drift; code does not. Every fact has one home, and the doc that does
not own it links to it instead of copying it.

| Fact | Home |
|------|------|
| Types, fields, defaults, ranges, variants, ids, timers, messages | The code, with `///` docs; the `ALL` lists and uniqueness tests guard ids |
| Architecture, where code lives, non-obvious behavior, Windows quirks | `CONTEXT.md` |
| Rules, gate commands, gotchas | This file |
| Tools, scripts, diagnostics | `TOOLS.md` |
| Decisions (`Dx`), milestones, feature status (`F-xx`) | `ROADMAP.md` |
| Open work, recipes, on-screen checks | `TASKS.md` |
| What a user sees and does | `README.md` |

- Never copy a list the code holds (struct fields, enum variants, `Ui`
  methods, ids with their "next free" value). Name the item and its file;
  `rustdoc` and the compiler keep it true.
- No hand-kept counts (tests, methods, lines). A dated measurement in a task
  is fine; a number every commit must update is not.
- A rule a test can check gets the test, and the doc points at it.
- Change or delete a sentence in the same commit as the code it describes.
  When in doubt, delete it.
- Short sentences, one parenthetical at most, plain words.