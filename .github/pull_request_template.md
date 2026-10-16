## What and why

<!-- What does this change, and why? Link the issue or TASKS.md id it addresses. -->

## How it was checked

<!-- Tests added or run. For anything visible on screen, a screenshot or a note
on what you checked by hand. -->

## Checklist

- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
      and `cargo test --workspace` pass
- [ ] `cargo deny check all` passes (if dependencies changed)
- [ ] New logic has tests
- [ ] `docs/CONTEXT.md` is updated where behavior changed
- [ ] `CHANGELOG.md` has an "Unreleased" entry for user-visible changes
- [ ] For a `docs/TASKS.md` task: its box is ticked and it is moved to the archive
