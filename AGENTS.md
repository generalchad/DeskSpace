# AGENTS.md

A brief for automated agents working in this repository. The rules themselves
are in `docs/ENGINEERING.md` and bind agents and humans alike; human
contributors start at `CONTRIBUTING.md`.

DeskSpace is a free and open-source desktop space organizer for Windows,
written in Rust with native Win32. It builds only on Windows.

## Directive 1: ask before anything destructive

Every destructive action waits for the user's explicit go-ahead in the
conversation, every time. Destructive means anything that deletes, overwrites,
moves, or empties files or folders you did not create in this session, or
that cannot be undone: `Remove-Item`, `rm`, `git reset --hard`,
`git checkout -- <file>` over someone's edits, `git push --force`, emptying
the Recycle Bin, and running a script or a copy of one with a flag that
deletes (`dev.ps1 -Fresh` wipes `%TEMP%\deskspace-dev`, the maintainer's dev
config and stored items). Say what will be lost, then wait. Approval covers
that one action, not the next. Scratch files you made yourself are yours to
remove. When unsure, it is destructive.

## Read first

1. `docs/TASKS.md`: "How to work a task", the file map, and the recipe for
   your kind of change, which names the files and rules that apply.
   Section V lists what still awaits an on-screen check.
2. `docs/ENGINEERING.md`: the rules. Read the sections your recipe or task
   names, and all of it before an architectural change (a new module,
   crate, dependency, or extension point) or Win32 work. `docs/TOOLS.md`
   lists the tools and scripts.
3. `docs/CONTEXT.md`: architecture and current behavior. Read section 3, then
   only the sections your change touches. `docs/ROADMAP.md` is the plan.
   `docs/archive/` is history; skip it unless a task points there.

## Non-negotiables

- Run the gates before calling a change done: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`, and
  `cargo test --workspace --locked`.
- Commit after every change, one logical unit per commit; for a `TASKS.md`
  task, retire it in the same commit (its rule 7).
- If a task needs a design decision it does not state, or no extension point
  fits, stop and ask. Do not improvise.
- Follow the four rules at the top of `ENGINEERING.md` section 2: data
  structures first, no special cases, never break users, and the compiler
  enforces while docs explain. Never copy into a doc a list the code holds
  (`ENGINEERING.md` section 7).
- You cannot see the screen. Implement, then hand the on-screen check to a
  human as a **[needs human]** item (`TASKS.md` rule 8).

## Tooling

`docs/TOOLS.md` lists the agent tools (the `rust` MCP server, which the
repository's MCP configs start once `rust-mcp-server` is installed), the
scripts, the app's own diagnostics, and the recipes for finding a hang or a
missed shutdown. Prefer the MCP tools over a raw shell; without them, pass
`--locked` to cargo yourself. Keep `Cargo.lock` current when adding or
updating dependencies.
