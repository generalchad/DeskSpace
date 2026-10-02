# AGENTS.md

Guidance for automated agents and contributors working in this repository.
For architecture and current state, read `CONTEXT.md` first.

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
| Test   | `cargo test --workspace` |
| Lint   | `cargo clippy --workspace --all-targets` |
| Format | `cargo fmt --all` |
| Release| `cargo build --release` |

Always run `cargo fmt --all`, `cargo clippy --workspace --all-targets`, and
`cargo test --workspace` before considering a change complete. CI should treat
clippy warnings as failures.

## Workspace layout

```
crates/deskspace-core      Domain model. No OS or IO dependencies.
crates/deskspace-config    Serde/TOML configuration schema and persistence.
crates/deskspace-platform  Win32 window host, GDI renderer, shell, hotkeys.
crates/deskspace-app       Application binary, CLI, event handler, scene builder.
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
- Rendering is data-driven. `deskspace-app` converts the model into a
  `deskspace_platform::Scene`; the platform `Painter` only draws the scene.
  Keep Win32 types out of `core` and out of the app where possible.
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
   extraction, the `0x052C` WorkerW spawn trick, color-key math, and raw
   `GWLP_USERDATA` pointers. Add a short note on why the code is the way it is,
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
- Window state lives behind a raw `*mut WindowState` stored in `GWLP_USERDATA`
  and is freed exactly once in `WM_NCDESTROY`. Do not free it elsewhere.
- `DesktopWindow::create` moves the `EventHandler` into the window;
  `DesktopWindow::run` blocks on the Win32 message pump.
- Win32 does not support layered child windows. Desktop-hosted placement
  (`Placement::DesktopBackground` / `DesktopForeground`) is therefore opaque
  and non-layered; only `TopLevel` uses color-key transparency.
- `WS_EX_TRANSPARENT` makes the entire window click-through. Do not use it to
  achieve click-through for empty areas; color-keyed layered windows already
  pass mouse input through color-keyed pixels.
