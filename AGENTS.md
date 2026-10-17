# AGENTS.md

A brief for automated agents working in this repository. The rules themselves
are in `docs/ENGINEERING.md` and bind agents and humans alike; human
contributors start at `CONTRIBUTING.md`.

DeskSpace is a free and open-source desktop space organizer for Windows,
written in Rust with native Win32. It builds only on Windows.

## Read first

1. `docs/ENGINEERING.md`: commands, gates, design priorities, architectural
   rules, comments, commits, and the Win32 gotchas. Read all of it.
2. `docs/CONTEXT.md`: architecture and current behavior. Read section 3, then
   only the sections your change touches.
3. `docs/TASKS.md`: "How to work a task", the file map, and the recipes;
   section V lists what still awaits an on-screen check. `docs/ROADMAP.md`
   is the plan. `docs/archive/` is history; skip it unless a task points
   there.

## Non-negotiables

- Run the gates before calling a change done: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace`.
- Commit after every change, one logical unit per commit; for a `TASKS.md`
  task, tick and archive it in the same commit.
- If a task needs a design decision it does not state, or no extension point
  fits, stop and ask. Do not improvise.
- You cannot see the screen. Implement, then hand the on-screen check to a
  human as a **[needs human]** item (`TASKS.md` rule 8).

## Tooling

A bundled `rust` MCP server exposes the cargo workflow as tools. Prefer them
over a raw shell, so the same flags (including `--locked`) apply everywhere.

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

Other cargo subcommands (`clean`, `update`, `package`, `expand`, `hack`,
`insta`, ...) appear as their own tools when a task needs them; install a
missing one with its helper (`cargo-deny-install`, `cargo-hack-install`,
`cargo-machete-install`). Because these tools pass `--locked`, keep
`Cargo.lock` current when adding or updating dependencies. Release work
(`RELEASING.md`; `TASKS.md` R10-R13) also uses `gh`, `actionlint`, and
`cargo-packager` (the NSIS installer, installed with `cargo install`).
