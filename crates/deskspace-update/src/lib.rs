//! Update checking and installation for DeskSpace (ROADMAP F-45), built on
//! `cargo-packager-updater`.
//!
//! The app never contacts the network on its own: `check` runs only when the
//! user asks (the tray's "Check for updates…" or the Settings button) or when
//! the opt-in "Check for updates automatically" setting is on. It reads a
//! signed feed (`latest.json`) published as an asset of the latest GitHub
//! release and verifies it with a minisign public key compiled into the binary.
//!
//! The package is always the full NSIS installer; there are no deltas. An
//! update is installed in two steps, so the app can shut down cleanly in
//! between: `download` fetches and verifies the package on a worker thread,
//! and `install` runs it once the app's message loop has ended.

#![forbid(unsafe_code)]

use std::path::Path;

use cargo_packager_updater::{
    Config, Update, WindowsConfig, WindowsUpdateInstallMode, check_update, semver::Version,
};
use thiserror::Error;

/// The release feed, always served from the latest published release.
const UPDATE_ENDPOINT: &str =
    "https://github.com/generalchad/DeskSpace/releases/latest/download/latest.json";
/// Where to send a user who wants to download manually.
const RELEASES_URL: &str = "https://github.com/generalchad/DeskSpace/releases";
/// The minisign public key the feed's installer signature is verified against.
///
/// TODO(maintainer): replace with the project's minisign public key. Generate a
/// keypair once with `minisign -G`, keep the private key in the
/// `DESKSPACE_MINISIGN_KEY` GitHub Actions secret (used by `release.yml`), and
/// paste the public key (the contents of `minisign.pub`) here. Until this is
/// set, `check` returns `UpdateError::NotConfigured`.
const UPDATE_PUBKEY: &str = "";

#[derive(Debug, Error)]
pub enum UpdateError {
    /// No update feed is configured for this build (see `UPDATE_PUBKEY`).
    #[error("this build has no update channel configured")]
    NotConfigured,
    #[error("could not check for updates: {0}")]
    Check(#[from] cargo_packager_updater::Error),
    #[error("could not download the update: {0}")]
    Download(cargo_packager_updater::Error),
    #[error("could not start the update installer: {0}")]
    Install(cargo_packager_updater::Error),
    #[error("unexpected version {0:?}")]
    Version(String),
}

/// A newer release the user can install. It also holds the updater's own
/// `Update` so the download and install do not have to check again.
pub struct Available {
    pub version: String,
    pub notes: String,
    pub page_url: String,
    update: Update,
}

/// A downloaded, signature-verified update package, ready to install.
pub struct Package {
    pub version: String,
    update: Update,
    bytes: Vec<u8>,
}

/// How the installer shows itself while it replaces the app. Both modes need
/// no input and relaunch DeskSpace when they finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    /// A small progress window, for an update the user just asked for.
    Passive,
    /// No window at all, for an automatic update while the user is away.
    Silent,
}

impl InstallMode {
    fn sdk_mode(self) -> WindowsUpdateInstallMode {
        match self {
            Self::Passive => WindowsUpdateInstallMode::Passive,
            Self::Silent => WindowsUpdateInstallMode::Quiet,
        }
    }
}

/// Whether this build can check for updates at all. The app disables its
/// update controls when it cannot, rather than offering a check that always
/// fails.
pub fn is_configured() -> bool {
    !UPDATE_PUBKEY.trim().is_empty()
}

/// Checks the feed for a release newer than the running build. Blocking; call
/// it on a worker thread. `Ok(None)` means the app is up to date.
pub fn check() -> Result<Option<Available>, UpdateError> {
    if !is_configured() {
        return Err(UpdateError::NotConfigured);
    }
    let current = parse(env!("CARGO_PKG_VERSION"))?;
    let endpoint = UPDATE_ENDPOINT
        .parse()
        .map_err(|_| UpdateError::NotConfigured)?;
    let config = Config {
        endpoints: vec![endpoint],
        pubkey: UPDATE_PUBKEY.to_string(),
        ..Config::default()
    };
    let Some(update) = check_update(current, config)? else {
        return Ok(None);
    };
    let version = update.version.trim_start_matches('v').to_string();
    Ok(Some(Available {
        page_url: format!("{RELEASES_URL}/tag/v{version}"),
        notes: update.body.clone().unwrap_or_default(),
        version,
        update,
    }))
}

/// Whether this executable is the installed build (under
/// `%LOCALAPPDATA%\DeskSpace.App`, where `packaging/nsis/installer.nsi` puts
/// it) rather than a portable copy. Only the installed build can replace
/// itself; a portable build only points the user at the release page.
pub fn is_installed() -> bool {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return false;
    };
    let root = Path::new(&local).join("DeskSpace.App");
    std::env::current_exe().is_ok_and(|exe| exe.starts_with(&root))
}

/// Downloads the update package and verifies its signature. Blocking; call it
/// on a worker thread.
pub fn download(available: &Available) -> Result<Package, UpdateError> {
    let bytes = available.update.download().map_err(UpdateError::Download)?;
    Ok(Package {
        version: available.version.clone(),
        update: available.update.clone(),
        bytes,
    })
}

/// Runs the installer for a downloaded package. Call it only after the app
/// has shut down (its windows closed, Explorer's icons restored, and the
/// watchdog told the exit was clean): the updater ends this process as soon
/// as the installer starts, so on success this never returns. The installer
/// replaces the files and relaunches DeskSpace.
pub fn install(package: Package, mode: InstallMode) -> Result<(), UpdateError> {
    let mut update = package.update;
    update.config.windows = Some(WindowsConfig {
        install_mode: Some(mode.sdk_mode()),
        ..WindowsConfig::default()
    });
    update.install(package.bytes).map_err(UpdateError::Install)
}

fn parse(version: &str) -> Result<Version, UpdateError> {
    Version::parse(version.trim_start_matches('v'))
        .map_err(|_| UpdateError::Version(version.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_workflow_publishes_the_feed_the_updater_reads() {
        let feed = UPDATE_ENDPOINT.rsplit('/').next().unwrap();
        let workflow = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.github/workflows/release.yml");
        let workflow = std::fs::read_to_string(workflow).unwrap();
        assert!(
            workflow.contains(&format!("FEED={feed}")),
            "release.yml must upload `{feed}`"
        );
    }

    #[test]
    fn install_modes_relaunch_the_app_after_installing() {
        for mode in [InstallMode::Passive, InstallMode::Silent] {
            assert!(mode.sdk_mode().nsis_args().contains(&"/R"), "{mode:?}");
        }
        assert!(InstallMode::Silent.sdk_mode().nsis_args().contains(&"/S"));
    }

    #[test]
    fn parse_accepts_a_leading_v_and_rejects_bad_versions() {
        assert_eq!(parse("v0.4.0").unwrap(), Version::new(0, 4, 0));
        assert_eq!(parse("0.4.0").unwrap(), Version::new(0, 4, 0));
        assert!(matches!(
            parse("not-a-version"),
            Err(UpdateError::Version(_))
        ));
    }
}
