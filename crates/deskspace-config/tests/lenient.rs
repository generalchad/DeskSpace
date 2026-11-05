//! An older build must load a config or theme file a newer one wrote, even
//! when a word it does not know appears in a field that holds an enum.

use std::path::PathBuf;

use deskspace_config::{Config, ThemeFile};
use deskspace_core::{
    Command, Extensions, ItemFacts, Point, Rect, Size, SortKey, SortRule, SpaceIcon, SpaceLook,
    SpaceView, ViewMode,
};

/// A word no build knows, standing in for one a later version writes.
const UNKNOWN: &str = "a_word_from_a_later_build";

fn bounds() -> Rect {
    Rect::new(0, 0, 1920, 1080)
}

/// A config with a space and an item, a lens, and a customized look, so every
/// enum that a saved file holds appears in it.
fn sample_config() -> Config {
    let mut config = Config::new();
    let view = SpaceView {
        mode: ViewMode::Details,
        sort: SortRule {
            key: SortKey::Modified,
            descending: true,
            folders_first: true,
        },
        status_bar: true,
        extensions: Extensions::Show,
        ..SpaceView::default()
    };
    assert!(
        config
            .desktop
            .apply(
                Command::CreateSpace {
                    title: "Work".into(),
                    at: Point::new(10, 20),
                    size: Size::new(300, 200),
                    locked: false,
                    icon: SpaceIcon::None,
                    view,
                },
                bounds(),
            )
            .is_applied()
    );
    let id = config.desktop.spaces()[0].id;
    assert!(
        config
            .desktop
            .apply(
                Command::AddItem {
                    space: id,
                    path: "C:/Docs/a.txt".into(),
                    facts: ItemFacts {
                        size: Some(3),
                        modified: Some(4),
                        created: Some(2),
                        ..ItemFacts::default()
                    },
                },
                bounds(),
            )
            .is_applied()
    );
    let seed = SpaceLook::from_global(&config.theme, &config.desktop.layout);
    assert!(
        config
            .desktop
            .apply(Command::CustomizeSpace { id, on: true, seed }, bounds())
            .is_applied()
    );
    assert!(
        config
            .desktop
            .apply(
                Command::CreateLens {
                    title: "Docs".into(),
                    at: Point::new(400, 20),
                    size: Size::new(300, 200),
                    path: "C:/Docs/lens".into(),
                    locked: false,
                    view: SpaceView::default(),
                },
                bounds(),
            )
            .is_applied()
    );
    config
}

/// Every string leaf in `value`.
fn string_count(value: &toml::Value) -> usize {
    match value {
        toml::Value::String(_) => 1,
        toml::Value::Table(table) => table.values().map(string_count).sum(),
        toml::Value::Array(items) => items.iter().map(string_count).sum(),
        _ => 0,
    }
}

/// Replaces string leaf number `index`, counting from zero, with [`UNKNOWN`].
fn replace_nth(value: &mut toml::Value, index: &mut usize) -> bool {
    match value {
        toml::Value::String(text) => {
            if *index > 0 {
                *index -= 1;
                return false;
            }
            *text = UNKNOWN.into();
            true
        }
        toml::Value::Table(table) => table.iter_mut().any(|(_, entry)| replace_nth(entry, index)),
        toml::Value::Array(items) => items.iter_mut().any(|entry| replace_nth(entry, index)),
        _ => false,
    }
}

#[test]
fn every_word_in_a_saved_config_loads_as_its_fields_default() {
    let config = sample_config();
    let text = toml::to_string_pretty(&config).unwrap();
    let base: toml::Value = toml::from_str(&text).unwrap();
    let words = string_count(&base);
    assert!(words > 20, "the sample should hold every enum's word");
    let spaces = config.desktop.spaces().len();

    for index in 0..words {
        let mut probe = base.clone();
        let mut target = index;
        assert!(
            replace_nth(&mut probe, &mut target),
            "word {index} is not a string"
        );
        let text = toml::to_string(&probe).unwrap();
        let mut loaded: Config = toml::from_str(&text).unwrap_or_else(|error| {
            panic!("word {index} made the config fail to load: {error}\n{text}")
        });
        loaded.migrate();
        assert_eq!(
            loaded.desktop.spaces().len(),
            spaces,
            "word {index} lost a space\n{text}"
        );
    }
}

#[test]
fn every_word_in_the_full_reference_theme_loads_as_its_fields_default() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/packs/themes/full.deskspace-theme.toml");
    let base: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let words = string_count(&base);
    assert!(words > 5, "the reference file should hold enum words");
    let dir = tempfile::tempdir().unwrap();
    let probe_path = dir.path().join("probe.deskspace-theme.toml");

    for index in 0..words {
        let mut probe = base.clone();
        let mut target = index;
        assert!(
            replace_nth(&mut probe, &mut target),
            "word {index} is not a string"
        );
        std::fs::write(&probe_path, toml::to_string(&probe).unwrap()).unwrap();
        ThemeFile::load(&probe_path).unwrap_or_else(|error| {
            panic!("word {index} made the theme file fail to load: {error}")
        });
    }
}
