//! Update checking and installation for DeskSpace (ROADMAP F-45), built on
//! `cargo-packager-updater`.
//!
//! The app never contacts the network on its own: `check` runs only when the
//! user asks (the tray's "Check for updates…" or the Settings button) or when
//! the opt-in "Check for updates automatically" setting is on. It reads a
//! signed feed (`latest.json`) published as an asset of the latest GitHub
//! release and verifies it with a minisign public key compiled into the binary.
//!
//! The package is always the full NSIS installer; there are no deltas.

#![forbid(unsafe_code)]

use cargo_packager_updater::{Config, Update, check_update, semver::Version};
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

/// Checks the feed for a release newer than the running build. Blocking; call
/// it on a worker thread. `Ok(None)` means the app is up to date.
pub fn check() -> Result<Option<Available>, UpdateError> {
    if UPDATE_PUBKEY.trim().is_empty() {
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

/// Downloads and verifies the update package, then runs it, replacing the
/// installed app. Blocking; call it on a worker thread. The installer
/// terminates and relaunches DeskSpace, so it normally does not return.
pub fn download_and_install(available: &Available) -> Result<(), UpdateError> {
    available.update.download_and_install()?;
    Ok(())
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
    fn parse_accepts_a_leading_v_and_rejects_bad_versions() {
        assert_eq!(parse("v0.4.0").unwrap(), Version::new(0, 4, 0));
        assert_eq!(parse("0.4.0").unwrap(), Version::new(0, 4, 0));
        assert!(matches!(
            parse("not-a-version"),
            Err(UpdateError::Version(_))
        ));
    }
}
