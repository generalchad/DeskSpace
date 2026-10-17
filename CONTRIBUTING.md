# Contributing to DeskSpace

Thanks for helping. Bug reports, ideas, and code are all welcome. By taking
part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## First 15 minutes

Enough to make a small fix safely. The longer docs are for when you need them.

**1. Run it.** You need Windows 10 or 11 (x64) and
[rustup](https://rustup.rs) with the MSVC toolchain; rustup installs the
pinned Rust version on the first build. There is nothing else to install.

```powershell
git clone https://github.com/generalchad/DeskSpace.git
cd DeskSpace
./scripts/dev.ps1 -Fresh -OnTop
```

`dev.ps1` builds and runs DeskSpace with a scratch profile in
`%TEMP%\deskspace-dev`, so your real settings and files are never touched.
Spaces normally sit behind every other window; `-OnTop` brings them in front
of your editor. `Get-Help ./scripts/dev.ps1` lists the other switches.

**2. Know the shape.** Five crates, with dependencies pointing down only:

```text
                         deskspace-app
         CLI, event handling; builds scenes, menus, and settings forms
           /                    |                     \
  deskspace-config      deskspace-platform        deskspace-update
  the settings file     all Win32 and unsafe      the updater; no
           \                    /                 workspace dependency
                         deskspace-core
            the pure model: spaces, items, layout, commands
```

**3. Keep three rules.** Most review comments come from these:

- **The model changes only through a `Command`.** To change a space or item,
  add or reuse a `Command` variant and apply it with `Desktop::apply`. The
  compiler enforces it: the model's fields and mutators are private to
  `deskspace-core`.
- **`unsafe` and Win32 live only in `deskspace-platform`.** Every `unsafe`
  block needs a `// SAFETY:` comment (a lint enforces it). `core`, `config`,
  `update`, and `app` forbid `unsafe` outright.
- **The app describes and the platform draws.** The app builds plain data
  (`SpaceScene`, `MenuEntry`, `SettingsForm`), and the platform turns it into
  windows and pixels. Keep Win32 types out of the app.

**4. Pass the gates** before every commit; CI runs the same checks:

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

(`./scripts/dev.ps1 -Check -NoRun` runs all three.) That's enough for a small
fix. For anything bigger, keep reading.

**Where to go next:**

| You want to... | Read |
|----------------|------|
| Pick something to work on | [`docs/TASKS.md`](docs/TASKS.md): one-commit tasks, a file map, and a recipe per kind of change |
| Know the rules in full | [`docs/ENGINEERING.md`](docs/ENGINEERING.md): design priorities, architecture rules, comments, and Win32 gotchas |
| Understand how a part works | [`docs/CONTEXT.md`](docs/CONTEXT.md): read section 3, then only the sections you touch |
| See the bigger plan | [`docs/ROADMAP.md`](docs/ROADMAP.md) |

## Reporting bugs and asking for features

Use the issue forms on GitHub ("New issue"). Search the existing issues first;
a thumbs-up on an existing issue helps more than a duplicate.

Security problems go through a private report instead of an issue; see
[`SECURITY.md`](SECURITY.md).

## Finding something to work on

- [`docs/TASKS.md`](docs/TASKS.md) lists ready-to-do work items, each sized for
  one commit, with the files to touch and a "Done when" list. Its "How to work
  a task" section explains the rules.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) has the bigger plan. For a feature that
  is not in `TASKS.md`, open an issue to agree on the design before writing
  much code.

## Making a change

1. Read [`docs/ENGINEERING.md`](docs/ENGINEERING.md), the engineering guide:
   the design priorities, the architectural rules, the extension points, and
   the comment style. Then read only the
   [`docs/CONTEXT.md`](docs/CONTEXT.md) sections your change touches.
2. Keep each commit one logical unit of work, with tests for new logic.
   Update `docs/CONTEXT.md` in the same commit when behavior changes.
3. Before committing, run the gates above and, when dependencies change,
   `cargo deny check all`. CI also builds on the minimum supported Rust
   version.
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
