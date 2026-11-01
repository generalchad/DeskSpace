# RELEASING

How to cut a DeskSpace release. `docs/ENGINEERING.md` has the build and test workflow,
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

## Update feed and signing (F-45)

The in-app updater reads a `latest.json` asset served from the latest release.
`release.yml` signs the NSIS installer with `cargo packager signer sign` and
builds the feed from that signature, but only when the `DESKSPACE_MINISIGN_KEY`
Actions secret is set. Without it a release ships no feed, and the app reports
that no update channel is configured.

One-time setup (maintainer):

1. `cargo install cargo-packager --locked --version 0.11.8` (matches CI), then
   `cargo packager signer generate --ci`, which skips the password prompts; the
   workflows pass no password. It writes a private key file and a `.pub` file
   beside it.
2. Store the **private** key file's contents as the repository secret
   `DESKSPACE_MINISIGN_KEY` (Settings -> Secrets and variables -> Actions), and
   keep a copy somewhere safe, such as a password manager.
3. Paste the **public** key file's contents (one base64 line) into
   `crates/deskspace-update/src/lib.rs` as `UPDATE_PUBKEY`.

Never commit the private key. The updater verifies the feed with the compiled
public key, so both halves must be rotated together. Losing the private key
means every installed copy rejects new updates and icon sets until it is
reinstalled by hand.

## Publishing an icon set

Icon sets are signed pre-releases of their own (ROADMAP D16). To add or update
one:

1. Bump or add its row in `ICON_SETS` (`crates/deskspace-config/src/icons.rs`).
2. Run the example's `trim` on the upstream `icons.json`:
   `cargo run -p deskspace-config --example icon_set -- trim <prefix> <icons.json> <out dir>`.
3. Commit the row and `assets/icons/<prefix>-names.txt`.
4. Run the "Publish icon set" workflow (`icons.yml`) with the prefix.

Never delete an `icons-*` release: shipped builds download it by its revision.

## Checklist

1. `main` is green in CI.
2. Bump `workspace.package.version` in the root `Cargo.toml`, then refresh
   `Cargo.lock` with `cargo check --workspace`.
3. Move any finished milestone or feature items in `docs/ROADMAP.md` and
   `docs/CONTEXT.md`.
4. In `CHANGELOG.md`, rename "Unreleased" to `[X.Y.Z] - YYYY-MM-DD`, start a
   new empty "Unreleased" above it, and update the compare links at the
   bottom.
5. Commit as `release: vX.Y.Z`.
6. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
7. Watch the workflow: `gh run watch`.
8. Download the zip and the installer (`deskspace_<version>_x64-setup.exe`)
   from the release page, check the zip's `.sha256`, and smoke-test the zip
   with a scratch `--config` and the installer by installing and uninstalling
   it.
9. Replace the generated release notes on the GitHub release page with the
   version's `CHANGELOG.md` section.

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
  nsis`, which writes `deskspace_<version>_x64-setup.exe` (unsigned until R10);
- **Sign installer and build the update feed**: only when the
  `DESKSPACE_MINISIGN_KEY` secret is set. `cargo packager signer sign` the
  installer, then write `latest.json` from its `.sig`. This is the feed the
  in-app updater reads;
- **Publish**: `gh release create <tag> <zip> <zip>.sha256 <setup.exe>
  [latest.json] --title "DeskSpace <tag>" --generate-notes`, adding
  `--prerelease` when the tag contains `-`. Any existing release for the tag is
  deleted first, so re-running after moving a tag is idempotent.

## Recovering from a bad release

Delete the release and its tag together, fix the problem, and publish the next
patch version:

```
gh release delete vX.Y.Z --cleanup-tag
```

Never re-use a published version number.
