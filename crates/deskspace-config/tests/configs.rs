//! A config written by a released build must keep loading (TASKS.md Z1).
//!
//! "Never break users" (`docs/ENGINEERING.md` section 2, rule 3): a config
//! that loaded yesterday loads today, and a save keeps every space, item, and
//! flag it held. Never edit or delete a file in `tests/configs`; a later
//! release adds its own beside them.

use std::path::{Path, PathBuf};

use deskspace_config::{Config, ConfigStore};
use deskspace_core::{ItemId, Rect, SpaceId};

fn reference(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/configs")
        .join(name)
}

/// A store over a copy of `name` in a scratch folder, so a load that backs
/// the file up writes beside the copy and not into the reference folder.
fn scratch_store(name: &str) -> (tempfile::TempDir, ConfigStore) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::copy(reference(name), &path).unwrap();
    (dir, ConfigStore::with_path(path))
}

fn backups_in(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "bak"))
        .collect()
}

fn load(name: &str) -> (tempfile::TempDir, ConfigStore, Config) {
    let (dir, store) = scratch_store(name);
    let config = store
        .load()
        .unwrap_or_else(|error| panic!("{name} did not load: {error}"));
    (dir, store, config)
}

#[test]
fn every_reference_config_loads_without_a_backup() {
    for name in ["v0.2.4-full.toml", "v0.2.4-default.toml"] {
        let (dir, store, config) = load(name);

        assert_eq!(config.version, 2, "{name} changed its version");
        assert!(!config.is_from_newer_version(), "{name}");

        // The app's own load path, which backs the file up on a failure.
        assert_eq!(store.load_or_default(), config, "{name}");
        assert!(
            backups_in(dir.path()).is_empty(),
            "{name} made a backup, so something treated it as broken"
        );
    }
}

#[test]
fn the_full_reference_keeps_its_spaces_and_items() {
    let (_, _, config) = load("v0.2.4-full.toml");
    let spaces = config.desktop.spaces();
    assert_eq!(spaces.len(), 3);

    let work = &spaces[0];
    assert_eq!(work.id, SpaceId(7));
    assert_eq!(work.title(), "Work");
    assert_eq!(work.rect(), Rect::new(100, 120, 420, 300));
    assert!(work.locked(), "the locked space stayed locked");
    assert!(!work.folded());
    assert!(work.custom(), "the per-space look stayed on");
    assert!(work.user_sized());
    assert_eq!(work.title_height, 32);
    assert!(work.look().is_some());

    let items = work.items();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, ItemId(1));
    assert_eq!(
        items[0].path,
        PathBuf::from("C:/Users/me/Documents/report.docx")
    );
    assert_eq!(items[0].label, "Quarterly report");
    assert_eq!(items[0].size, Some(12345));
    assert_eq!(items[0].modified, Some(1_700_000_000));
    assert!(items[0].compressed);
    assert_eq!(items[1].id, ItemId(2));
    assert!(items[1].encrypted);

    let folded = &spaces[1];
    assert_eq!(folded.id, SpaceId(11));
    assert_eq!(folded.title(), "Folded");
    assert_eq!(folded.rect(), Rect::new(600, 120, 260, 140));
    assert!(folded.folded(), "the folded space stayed folded");
    assert!(!folded.locked() && !folded.custom() && !folded.user_sized());
    assert_eq!(folded.items().len(), 1);
    assert_eq!(folded.items()[0].id, ItemId(3));

    let loose = &spaces[2];
    assert_eq!(loose.id, SpaceId(12));
    assert_eq!(loose.title(), "Loose");
    assert_eq!(loose.rect(), Rect::new(900, 120, 320, 200));
    assert!(loose.items().is_empty());

    let layer = config.desktop.layer().items();
    assert_eq!(layer.len(), 1);
    assert_eq!(layer[0].id, ItemId(20));
    assert_eq!(layer[0].path, PathBuf::from("C:/"));
}

#[test]
fn the_default_reference_still_loads_as_todays_defaults() {
    // A user who changed nothing in v0.2.4 must not see a changed default
    // appear in their config. A deliberate default change amends this test
    // and says why in the commit body.
    let (_, _, config) = load("v0.2.4-default.toml");
    assert_eq!(config, Config::new());
}

#[test]
fn each_reference_config_round_trips_through_a_save() {
    for name in ["v0.2.4-full.toml", "v0.2.4-default.toml"] {
        let (_, store, config) = load(name);
        store
            .save(&config)
            .unwrap_or_else(|error| panic!("{name} did not save: {error}"));
        let reloaded = store
            .load()
            .unwrap_or_else(|error| panic!("{name} did not reload after a save: {error}"));
        assert_eq!(reloaded, config, "{name} changed through a save");
    }
}
