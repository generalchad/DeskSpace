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

/// The release page, opened by a portable build that only notifies.
pub fn release_page() -> &'static str {
    RELEASES_URL
}

/// Picks the newest version in `candidates` that is newer than `current` and,
/// unless `prerelease`, is not a pre-release. Pure, so the selection can be
/// tested without a feed.
pub fn newest(current: &str, candidates: &[&str], prerelease: bool) -> Option<String> {
    let current = parse(current).ok()?;
    candidates
        .iter()
        .filter_map(|candidate| parse(candidate).ok())
        .filter(|version| *version > current)
        .filter(|version| prerelease || version.pre.is_empty())
        .max()
        .map(|version| version.to_string())
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
    fn newest_picks_the_highest_newer_version() {
        let candidates = ["0.1.0", "0.2.0", "0.2.1", "v0.3.0"];
        assert_eq!(
            newest("0.2.0", &candidates, false).as_deref(),
            Some("0.3.0")
        );
    }

    #[test]
    fn newest_ignores_older_and_equal_versions() {
        let candidates = ["0.1.0", "0.2.0"];
        assert_eq!(newest("0.2.0", &candidates, false), None);
        assert_eq!(newest("0.3.0", &candidates, false), None);
    }

    #[test]
    fn newest_skips_prereleases_unless_enabled() {
        let candidates = ["0.3.0-alpha.1", "0.2.1"];
        assert_eq!(
            newest("0.2.0", &candidates, false).as_deref(),
            Some("0.2.1")
        );
        assert_eq!(
            newest("0.2.0", &candidates, true).as_deref(),
            Some("0.3.0-alpha.1")
        );
    }

    #[test]
    fn newest_tolerates_a_leading_v_and_bad_entries() {
        let candidates = ["v0.4.0", "not-a-version", "0.3.0"];
        assert_eq!(
            newest("0.2.0", &candidates, false).as_deref(),
            Some("0.4.0")
        );
    }
}
