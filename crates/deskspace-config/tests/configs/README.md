# Reference configs

A config a released build wrote must load in every later build. That is the
"never break users" rule (`docs/ENGINEERING.md` section 2, rule 3), and these
files prove it.

- `v0.2.4-full.toml` and `v0.2.4-default.toml` were written by the released
  v0.2.4 build's own `ConfigStore`, from a worktree at the `v0.2.4` tag on
  2026-11-05. The full one has every field v0.2.4 had off its default; the
  default one is what `Config::new()` wrote.
- Never edit or delete a reference file. A format that changed gets a new file
  from the release that changed it, beside the old one. The tests in
  `../configs.rs` load each file and pin what it holds, so an edit would hide
  the break it exists to catch.
