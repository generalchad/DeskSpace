# AGENTS.md

Guidance for automated agents and contributors working in this repository.
For architecture and current state, read `CONTEXT.md` first. For the plan,
see `ROADMAP.md`; for ready-to-do work items, see `TASKS.md`.

## Project

DeskSpace is a free and open-source desktop space organizer for Windows, a
FOSS alternative to comparable tools, written in Rust with native Win32.
The workspace is Windows-only: `deskspace-platform` links the `windows` crate
under `cfg(windows)`, so the workspace does not build on other platforms.

## Prerequisites

- Windows 10 or 11 (x64)
- Rust stable (pinned in `rust-toolchain.toml`) with the MSVC toolchain
- No other system dependencies

## Commands

| Task   | Command |
|--------|---------|
| Build  | `cargo build --workspace` |
| Run    | `cargo run -p deskspace-app -- --no-icons` |
| Inspect| `cargo run -p deskspace-app -- --config <scratch.toml> --on-top --settings` |
| Test   | `cargo test --workspace` |
| Lint   | `cargo clippy --workspace --all-targets` |
| Format | `cargo fmt --all` |
| Release| `cargo build --release` |
| Dev loop | `./scripts/dev.ps1` (scratch config; `-Fresh`, `-OnTop`, `-Settings`, `-Check`, `-Release`; `Get-Help ./scripts/dev.ps1`) |

`scripts/dev.ps1` stops any DeskSpace running from `target\` before building,
because the running executable is locked.

Always run `cargo fmt --all`, `cargo clippy --workspace --all-targets`, and
`cargo test --workspace` before considering a change complete. CI should treat
clippy warnings as failures.

## Workspace layout

```
crates/deskspace-core      Domain model. No OS or IO dependencies.
crates/deskspace-config    Serde/TOML configuration schema and persistence.
crates/deskspace-platform  UI runtime, space windows, DWM, Direct2D painter,
                           tray, menus, settings window, shell, hotkeys.
crates/deskspace-app       Application binary, CLI, event handler, scene and
                           settings-form builders.
```

Dependency direction: `app -> {config, platform} -> core`. `core` depends on
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
  hold. This is the one comment that is never optional.
- A comment must be at least as correct as the code. When you change code,
  update or delete its comment in the same change.

## Gotchas

- The bundled MCP cargo tooling passes `--locked`. Keep `Cargo.lock` current
  when adding or updating dependencies.
- Space and settings window state live behind raw pointers in
  `GWLP_USERDATA` and are freed exactly once in `WM_NCDESTROY`. Do not free
  them elsewhere.
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
  placement is therefore opaque and uses a window region for rounding.
- `WS_EX_TRANSPARENT` only makes a window click-through when combined with
  `WS_EX_LAYERED`, and layering disables the blur. `--click-through` is a
  display-only mode.
- Launching the binary with a hidden show state (for example PowerShell
  `Start-Process -WindowStyle Hidden`) hides the settings window: Windows
  applies the startup show command to the first `ShowWindow` on an overlapped
  window.
- The keyboard hook (`win32/keyboard.rs`) has two modes: inline-edit capture
  (consumes all editing keys) and a selection-key watch (`Ui::watch_selection_keys`):
  Delete, F2, Ctrl+C, and Ctrl+X while an item is selected, and Ctrl+V and
  Ctrl+A while a space is current. The watch only consumes those keys while the
  desktop shell has the foreground, so other apps keep their own shortcuts. The
  hook is installed only while a mode is active; every other key passes
  through. New global key handling goes here, not in a window procedure,
  because spaces are never activated.
- Dragging an item inside its space rearranges it (`ReorderItem`); only once
  the pointer leaves the space does the app start an OLE shell drag, which is
  what moves the real file.
