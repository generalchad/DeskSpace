//! The icon sets DeskSpace offers, their cache on disk, and the trimmer that
//! makes a published set from its upstream package (D16).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use deskspace_core::icon_set::{IconAlias, IconData, IconSet};

use crate::error::ConfigError;

/// One icon set: where it comes from, what of it is kept, and its credits.
/// Everything else (the cache, the download, the picker, the saved names) is
/// keyed by `prefix`, so a new set is a new row.
#[derive(Debug)]
pub struct IconSetSpec {
    /// The Iconify prefix, saved in every `<prefix>:<name>` a config holds.
    pub prefix: &'static str,
    pub label: &'static str,
    pub author: &'static str,
    pub license: &'static str,
    /// The npm package the set is trimmed from.
    pub upstream: &'static str,
    pub upstream_version: &'static str,
    /// Only icons whose name ends with this are kept, without it.
    pub keep_suffix: &'static str,
    /// Bumped whenever the published file changes, so a build fetches the
    /// file it was made for.
    pub revision: u32,
    /// English words to icon names, for space titles the set does not name
    /// itself.
    pub words: &'static [(&'static str, &'static str)],
}

impl IconSetSpec {
    /// The published file, which is also the cached file's name.
    pub fn file_name(&self) -> String {
        format!("{}-r{}.json", self.prefix, self.revision)
    }

    /// The GitHub release the file is published under.
    pub fn release_tag(&self) -> String {
        format!("icons-{}-r{}", self.prefix, self.revision)
    }

    /// Whether `file_name` is this set's file at some revision.
    fn is_revision_file(&self, file_name: &str) -> bool {
        file_name
            .strip_prefix(self.prefix)
            .and_then(|rest| rest.strip_prefix("-r"))
            .and_then(|rest| rest.strip_suffix(".json"))
            .is_some_and(|revision| {
                !revision.is_empty() && revision.bytes().all(|b| b.is_ascii_digit())
            })
    }
}

/// Every icon set DeskSpace offers. The first is the default, which
/// automatic icons use.
pub const ICON_SETS: &[IconSetSpec] = &[IconSetSpec {
    prefix: "fluent",
    label: "Fluent UI System Icons",
    author: "Microsoft Corporation",
    license: "MIT",
    upstream: "@iconify-json/fluent",
    upstream_version: "1.2.60",
    // One-color outlines, as Windows 11's own title-bar glyphs are.
    keep_suffix: "-20-regular",
    revision: 1,
    words: FLUENT_WORDS,
}];

/// Keyed by the singular where a word has one: matching tries a title
/// word's singular too, so "photo" serves "Photo" and "Photos". Words the set
/// names itself ("apps", "games", "mail") are left out; matching finds them.
const FLUENT_WORDS: &[(&str, &str)] = &[
    ("art", "paint-brush"),
    ("backup", "cloud-backup"),
    ("bill", "receipt"),
    ("client", "people"),
    ("design", "design-ideas"),
    ("download", "arrow-download"),
    ("family", "people-community"),
    ("favorite", "star"),
    ("finance", "data-trending"),
    ("fitness", "dumbbell"),
    ("health", "heart-pulse"),
    ("homework", "notebook"),
    ("idea", "lightbulb"),
    ("kid", "emoji"),
    ("meeting", "people-audience"),
    ("movie", "movies-and-tv"),
    ("music", "music-note-2"),
    ("network", "wifi-1"),
    ("office", "building"),
    ("photo", "image-multiple"),
    ("picture", "image"),
    ("podcast", "mic"),
    ("presentation", "slide-text"),
    ("project", "folder-open"),
    ("recipe", "food"),
    ("research", "book-search"),
    ("school", "hat-graduation"),
    ("science", "beaker"),
    ("security", "shield"),
    ("shopping", "cart"),
    ("social", "people-chat"),
    ("spreadsheet", "table"),
    ("study", "book-open"),
    ("tax", "calculator"),
    ("temp", "broom"),
    ("terminal", "window-console"),
    ("todo", "task-list-ltr"),
    ("tool", "toolbox"),
    ("travel", "airplane"),
    ("utility", "wrench"),
    ("weather", "weather-partly-cloudy-day"),
    ("web", "globe"),
    ("work", "briefcase"),
    ("writing", "pen"),
];

pub fn icon_set(prefix: &str) -> Option<&'static IconSetSpec> {
    ICON_SETS.iter().find(|spec| spec.prefix == prefix)
}

/// The set automatic icons are matched in.
pub fn default_set() -> &'static IconSetSpec {
    &ICON_SETS[0]
}

/// The downloaded icon sets, one file per set. Trusted once written: only
/// the download is verified (D16).
#[derive(Debug, Clone)]
pub struct IconStore {
    dir: PathBuf,
}

impl IconStore {
    /// A store in `dir`, normally `ConfigStore::icons_dir()`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, spec: &IconSetSpec) -> PathBuf {
        self.dir.join(spec.file_name())
    }

    /// The cached set, or `None` when it has not been downloaded. A file that
    /// does not parse, or holds another set, is deleted and returned as an
    /// error, so the next need fetches it again.
    pub fn load(&self, spec: &IconSetSpec) -> Result<Option<IconSet>, ConfigError> {
        let path = self.path(spec);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ConfigError::io(path, error)),
        };
        match parse(spec, &path, &bytes) {
            Ok(set) => Ok(Some(set)),
            Err(error) => {
                if let Err(remove) = std::fs::remove_file(&path) {
                    tracing::warn!(%remove, path = %path.display(), "could not delete a bad icon set");
                }
                Err(error)
            }
        }
    }

    /// Checks that `bytes` are this set, writes them atomically, and deletes
    /// the set's other revisions. Returns the parsed set.
    pub fn save(&self, spec: &IconSetSpec, bytes: &[u8]) -> Result<IconSet, ConfigError> {
        let path = self.path(spec);
        let set = parse(spec, &path, bytes)?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|source| ConfigError::io(self.dir.clone(), source))?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, bytes).map_err(|source| ConfigError::io(temp.clone(), source))?;
        std::fs::rename(&temp, &path).map_err(|source| ConfigError::io(path.clone(), source))?;
        self.prune_other_revisions(spec);
        Ok(set)
    }

    fn prune_other_revisions(&self, spec: &IconSetSpec) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let current = spec.file_name();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name != current && spec.is_revision_file(name) {
                let path = entry.path();
                if let Err(error) = std::fs::remove_file(&path) {
                    tracing::warn!(%error, path = %path.display(), "could not delete an old icon set");
                }
            }
        }
    }
}

fn parse(spec: &IconSetSpec, path: &Path, bytes: &[u8]) -> Result<IconSet, ConfigError> {
    let bad = |reason: String| ConfigError::IconSet {
        path: path.to_owned(),
        reason,
    };
    let set: IconSet = serde_json::from_slice(bytes).map_err(|error| bad(error.to_string()))?;
    if set.prefix != spec.prefix {
        return Err(bad(format!(
            "holds the set {:?}, not {:?}",
            set.prefix, spec.prefix
        )));
    }
    Ok(set)
}

/// The published set made from its upstream package: the icons whose name
/// ends with `keep_suffix`, without it, and the plain aliases that end with it
/// and lead to a kept icon. A size every kept icon shares moves to the set.
pub fn trim(upstream: IconSet, spec: &IconSetSpec) -> IconSet {
    let short = |name: &str| {
        name.strip_suffix(spec.keep_suffix)
            .filter(|short| !short.is_empty())
            .map(str::to_owned)
    };
    let sized: BTreeMap<String, (IconData, u32, u32)> = upstream
        .icons
        .iter()
        .filter_map(|(name, icon)| {
            let width = icon.width.unwrap_or(upstream.width);
            let height = icon.height.unwrap_or(upstream.height);
            Some((short(name)?, (icon.clone(), width, height)))
        })
        .collect();
    let width = shared(sized.values().map(|(_, w, _)| *w)).unwrap_or(upstream.width);
    let height = shared(sized.values().map(|(_, _, h)| *h)).unwrap_or(upstream.height);
    let icons: BTreeMap<String, IconData> = sized
        .into_iter()
        .map(|(name, (icon, w, h))| {
            let icon = IconData {
                width: (w != width).then_some(w),
                height: (h != height).then_some(h),
                ..icon
            };
            (name, icon)
        })
        .collect();
    let aliases = upstream
        .aliases
        .keys()
        .filter_map(|name| {
            let parent = short(upstream.canonical(name)?)?;
            icons.contains_key(&parent).then_some(())?;
            let alias = IconAlias {
                parent,
                ..IconAlias::default()
            };
            Some((short(name)?, alias))
        })
        .collect();
    IconSet {
        prefix: spec.prefix.to_owned(),
        info: upstream.info,
        icons,
        aliases,
        width,
        height,
    }
}

/// The one value every item has, or `None` for none or several.
fn shared(mut values: impl Iterator<Item = u32>) -> Option<u32> {
    let first = values.next()?;
    values.all(|value| value == first).then_some(first)
}

/// The names in `shipped` (one per line) that `set` no longer has.
pub fn missing_names(set: &IconSet, shipped: &str) -> Vec<String> {
    shipped
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && set.canonical(name).is_none())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: IconSetSpec = IconSetSpec {
        prefix: "test",
        label: "Test",
        author: "Nobody",
        license: "MIT",
        upstream: "@iconify-json/test",
        upstream_version: "1.0.0",
        keep_suffix: "-20-regular",
        revision: 2,
        words: &[],
    };

    const UPSTREAM: &str = r#"{
        "prefix": "test",
        "info": { "name": "Test", "author": { "name": "Nobody" }, "license": { "title": "MIT", "spdx": "MIT" }, "total": 9 },
        "lastModified": 1,
        "icons": {
            "home-20-regular": { "body": "<path d=\"h\"/>" },
            "home-24-regular": { "body": "<path d=\"H\"/>", "width": 24, "height": 24 },
            "home-20-filled": { "body": "<path d=\"f\"/>" },
            "tall-20-regular": { "body": "<path d=\"t\"/>", "height": 21, "hidden": true },
            "-20-regular": { "body": "<path d=\"e\"/>" }
        },
        "aliases": {
            "house-20-regular": { "parent": "home-20-regular" },
            "abode-20-regular": { "parent": "house-20-regular" },
            "back-20-regular": { "parent": "home-20-regular", "hFlip": true },
            "house-20-filled": { "parent": "home-20-filled" },
            "odd-20-regular": { "parent": "home-24-regular" }
        },
        "width": 20,
        "height": 20
    }"#;

    fn upstream() -> IconSet {
        serde_json::from_str(UPSTREAM).unwrap()
    }

    #[test]
    fn an_iconify_file_parses_with_unknown_keys() {
        let set = upstream();
        assert_eq!(set.prefix, "test");
        assert_eq!(set.info.author.name, "Nobody");
        assert_eq!(set.info.license.spdx, "MIT");
        assert_eq!(set.icons.len(), 5);
        assert!(set.aliases["back-20-regular"].h_flip);
        assert_eq!(set.icons["home-24-regular"].width, Some(24));
    }

    #[test]
    fn trim_keeps_renames_and_drops_as_stated() {
        let set = trim(upstream(), &SPEC);
        assert_eq!(set.prefix, "test");
        assert_eq!(set.info.name, "Test");
        assert_eq!(set.names(), ["abode", "home", "house", "tall"]);
        // A chain is flattened to the icon it leads to.
        assert_eq!(set.aliases["abode"].parent, "home");
        assert_eq!(set.aliases["house"].parent, "home");
        assert_eq!(set.glyph("house").unwrap().body, "<path d=\"h\"/>");
    }

    #[test]
    fn trim_moves_a_shared_size_to_the_set() {
        let set = trim(upstream(), &SPEC);
        assert_eq!((set.width, set.height), (20, 20));
        assert_eq!(set.icons["home"].width, None);
        assert_eq!(set.icons["home"].height, None);
        assert_eq!(set.icons["tall"].height, Some(21));
        assert_eq!(set.glyph("tall").unwrap().height, 21);

        let mut upstream = upstream();
        upstream.icons.remove("home-20-regular");
        upstream.icons.remove("-20-regular");
        let only_tall = trim(upstream, &SPEC);
        assert_eq!(only_tall.height, 21);
        assert_eq!(only_tall.icons["tall"].height, None);
    }

    #[test]
    fn missing_names_lists_shipped_names_the_set_lacks() {
        let set = trim(upstream(), &SPEC);
        let shipped = "abode\nhome\ngone\n\nhouse\nback\n";
        assert_eq!(missing_names(&set, shipped), ["gone", "back"]);
    }

    #[test]
    fn the_store_round_trips_a_set() {
        let dir = tempfile::tempdir().unwrap();
        let store = IconStore::new(dir.path().join("icons"));
        assert!(store.load(&SPEC).unwrap().is_none());
        let bytes = serde_json::to_vec(&trim(upstream(), &SPEC)).unwrap();
        let saved = store.save(&SPEC, &bytes).unwrap();
        assert_eq!(store.load(&SPEC).unwrap(), Some(saved));
        assert!(dir.path().join("icons").join("test-r2.json").is_file());
    }

    #[test]
    fn a_corrupt_or_foreign_cached_set_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let store = IconStore::new(dir.path());
        let path = dir.path().join(SPEC.file_name());
        for bytes in [&b"{ not json"[..], br#"{ "prefix": "other" }"#] {
            std::fs::write(&path, bytes).unwrap();
            assert!(matches!(
                store.load(&SPEC),
                Err(ConfigError::IconSet { .. })
            ));
            assert!(!path.exists());
            assert!(store.load(&SPEC).unwrap().is_none());
        }
    }

    #[test]
    fn save_refuses_another_set_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = IconStore::new(dir.path());
        assert!(store.save(&SPEC, br#"{ "prefix": "other" }"#).is_err());
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn save_prunes_the_sets_other_revisions_only() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "test-r1.json",
            "test-r10.json",
            "test-extra-r1.json",
            "test-r.json",
            "other-r1.json",
        ] {
            std::fs::write(dir.path().join(name), b"{}").unwrap();
        }
        let store = IconStore::new(dir.path());
        store.save(&SPEC, br#"{ "prefix": "test" }"#).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "other-r1.json",
                "test-extra-r1.json",
                "test-r.json",
                "test-r2.json"
            ]
        );
    }

    #[test]
    fn file_name_and_release_tag_carry_prefix_and_revision() {
        assert_eq!(SPEC.file_name(), "test-r2.json");
        assert_eq!(SPEC.release_tag(), "icons-test-r2");
        assert_eq!(default_set().file_name(), "fluent-r1.json");
        assert_eq!(default_set().release_tag(), "icons-fluent-r1");
        assert_eq!(icon_set("fluent").map(|spec| spec.prefix), Some("fluent"));
        assert!(icon_set("nope").is_none());
    }

    fn shipped_names(spec: &IconSetSpec) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/icons")
            .join(format!("{}-names.txt", spec.prefix));
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    #[test]
    fn every_names_file_is_sorted_without_duplicates() {
        for spec in ICON_SETS {
            let text = shipped_names(spec);
            let names: Vec<&str> = text.lines().collect();
            assert!(!names.is_empty(), "{}", spec.prefix);
            assert!(
                names.windows(2).all(|pair| pair[0] < pair[1]),
                "{} names are not sorted and unique",
                spec.prefix
            );
        }
    }

    #[test]
    fn every_curated_target_is_a_shipped_name() {
        for spec in ICON_SETS {
            let text = shipped_names(spec);
            for (word, icon) in spec.words {
                assert!(
                    text.lines().any(|name| name == *icon),
                    "{}: {word} -> {icon} is not in the names file",
                    spec.prefix
                );
            }
        }
    }

    #[test]
    fn curated_words_are_single_lowercase_words_without_duplicates() {
        for spec in ICON_SETS {
            let mut words: Vec<&str> = spec.words.iter().map(|(word, _)| *word).collect();
            for word in &words {
                assert!(
                    !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase()),
                    "{}: {word:?}",
                    spec.prefix
                );
            }
            words.sort_unstable();
            let count = words.len();
            words.dedup();
            assert_eq!(words.len(), count, "{} has a duplicate word", spec.prefix);
        }
    }

    #[test]
    fn prefixes_are_unique() {
        let mut prefixes: Vec<&str> = ICON_SETS.iter().map(|spec| spec.prefix).collect();
        prefixes.sort_unstable();
        prefixes.dedup();
        assert_eq!(prefixes.len(), ICON_SETS.len());
    }
}
