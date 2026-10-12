# RELEASING

How to cut a DeskSpace release. `AGENTS.md` has the build and test workflow,
and `docs/ROADMAP.md` section 6 maps versions to milestones.

## Versioning

- Releases follow SemVer on `workspace.package.version` in the root
  `Cargo.toml`. That one value is the app version; every crate inherits it.
- While the version is below 1.0, the minor version follows `docs/ROADMAP.md`
  section 6 (v0.2, v0.3, ...) and patch versions are fixes.
- Pre-releases use `-alpha.N`, `-beta.N`, or `-rc.N`. `release.yml` marks a
  tag as a GitHub pre-release when it contains a `-`, so use
  `0.2.0-alpha.1`, not `0.2.0+alpha.1`.
- The config schema version (`CURRENT_VERSION` in `deskspace-config`) is
  independent of the app version and is bumped only by a breaking config
  change, with a `Config::migrate` step.

## Checklist

1. `main` is green in CI.
2. Bump `workspace.package.version` in the root `Cargo.toml`, then refresh
   `Cargo.lock` with `cargo check --workspace`.
3. Move any finished milestone or feature items in `docs/ROADMAP.md` and
   `docs/CONTEXT.md`.
4. Commit as `release: vX.Y.Z`.
5. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
6. Watch the workflow: `gh run watch`.
7. Download the zip and the installer (`deskspace_<version>_x64-setup.exe`)
   from the release page, check the zip's `.sha256`, and smoke-test the zip
   with a scratch `--config` and the installer by installing and uninstalling
   it.
8. Edit the generated release notes on the GitHub release page.

## What the workflow does

`.github/workflows/release.yml` runs on a `v*` tag. Its steps, in order:

- **checkout** the repository;
- **Install Rust toolchain** (`rustup toolchain install`) and **Show Rust
  version** (`rustc -Vv`), which use the pinned version in
  `rust-toolchain.toml`;
- **Check tag matches workspace version**: `cargo metadata` reads the
  `deskspace-app` version and the step fails unless `v<version>` equals the
  tag;
- **Test**: `cargo test --workspace --locked`;
- **Build**: `cargo build --release --locked -p deskspace-app` with the
  reproducible `RUSTFLAGS`, which builds both `deskspace.exe` and
  `deskspace-watchdog.exe`;
- **Package**: stages `deskspace.exe`, `deskspace-watchdog.exe`, `LICENSE`,
  `NOTICE`, and `README.md` into
  `deskspace-<tag>-x86_64-pc-windows-msvc/`, zips it, and writes a lowercase
  `.sha256` beside it;
- **Install cargo-packager**: `cargo install cargo-packager --locked
  --version 0.11.8`, the CI-only installer tool (not a workspace dependency);
- **Build installer**: `cargo packager -p deskspace-app --release --formats
  nsis`, which writes `deskspace_<version>_x64-setup.exe`; it is unsigned
  until R10;
- **Publish**: `gh release create <tag> <zip> <zip>.sha256
  deskspace_<version>_x64-setup.exe --title "DeskSpace <tag>"
  --generate-notes`, adding `--prerelease` when the tag contains `-`.

## Recovering from a bad release

Delete the release and its tag together, fix the problem, and publish the next
patch version:

```
gh release delete vX.Y.Z --cleanup-tag
```

Never re-use a published version number.
