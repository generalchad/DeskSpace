# Contributing to DeskSpace

Thanks for helping. Bug reports, ideas, and code are all welcome. By taking
part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Reporting bugs and asking for features

Use the issue forms on GitHub ("New issue"). Search the existing issues first;
a thumbs-up on an existing issue helps more than a duplicate.

Security problems go through a private report instead of an issue; see
[`SECURITY.md`](SECURITY.md).

## Setting up

You need Windows 10 or 11 (x64) and [rustup](https://rustup.rs) with the MSVC
toolchain. `rust-toolchain.toml` pins the exact Rust version, which rustup
installs on the first build. There are no other system dependencies.

```
git clone https://github.com/generalchad/DeskSpace.git
cd DeskSpace
./scripts/dev.ps1 -Fresh
```

`scripts/dev.ps1` builds and runs DeskSpace with a scratch profile in
`%TEMP%\deskspace-dev`, so your real settings and files are never touched.
`Get-Help ./scripts/dev.ps1` lists its switches. Spaces sit behind other
windows; add `-OnTop` to see them over your editor.

## Finding something to work on

- [`docs/TASKS.md`](docs/TASKS.md) lists ready-to-do work items, each sized for
  one commit, with the files to touch and a "Done when" list. Its "How to work
  a task" section explains the rules.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) has the bigger plan. For a feature that
  is not in `TASKS.md`, open an issue to agree on the design before writing
  much code.

## Making a change

1. Read [`AGENTS.md`](AGENTS.md). Despite the name it is the engineering guide
   for every contributor: the design priorities, the architectural rules, the
   extension points, and the comment style. Then read only the
   [`docs/CONTEXT.md`](docs/CONTEXT.md) sections your change touches.
2. Keep each commit one logical unit of work, with tests for new logic.
   Update `docs/CONTEXT.md` in the same commit when behavior changes.
3. Before committing, run:

   ```
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo deny check all
   ```

   (`./scripts/dev.ps1 -Check -NoRun` runs the first three.) CI runs the same
   checks and also builds on the minimum supported Rust version.
4. Write commit messages in the imperative, lowercase, with a type prefix:
   `feat: ...`, `fix: ...`, `refactor: ...`, `docs: ...`, `test: ...`,
   `perf: ...`, `ci: ...`. Add the task id when there is one:
   `refactor: split d2d.rs (K2)`.
5. Add a line under "Unreleased" in [`CHANGELOG.md`](CHANGELOG.md) for any
   change a user would notice.
6. Open a pull request against `main` and fill in the template. Changes to how
   something looks or behaves on screen need a screenshot or a note on what
   you checked by hand.

## Licensing

DeskSpace is licensed under the [Apache License 2.0](LICENSE). Unless you say
otherwise, any contribution you submit is licensed under the same terms, as
section 5 of the license states. New dependencies must pass `cargo deny`
(`deny.toml` lists the allowed licenses).
