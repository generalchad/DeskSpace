//! The subset of the Iconify JSON format that DeskSpace reads.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Iconify's default for a set or icon that gives no size.
const DEFAULT_SIZE: u32 = 16;

/// How many aliases `glyph` follows before it gives up, so a cycle ends.
const MAX_ALIAS_STEPS: usize = 8;

/// An icon set in the Iconify JSON format
/// (<https://iconify.design/docs/types/iconify-json.html>). Unknown keys are
/// ignored, and every key is optional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IconSet {
    pub prefix: String,
    pub info: IconSetInfo,
    pub icons: BTreeMap<String, IconData>,
    pub aliases: BTreeMap<String, IconAlias>,
    /// The size of an icon that gives none.
    pub width: u32,
    pub height: u32,
}

impl Default for IconSet {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            info: IconSetInfo::default(),
            icons: BTreeMap::new(),
            aliases: BTreeMap::new(),
            width: DEFAULT_SIZE,
            height: DEFAULT_SIZE,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IconData {
    /// The SVG content, without the `<svg>` element.
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

/// Another name for an icon, optionally turned or flipped. DeskSpace draws
/// only the plain ones.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IconAlias {
    pub parent: String,
    pub rotate: u8,
    pub h_flip: bool,
    pub v_flip: bool,
}

impl IconAlias {
    fn is_plain(&self) -> bool {
        self.rotate.is_multiple_of(4) && !self.h_flip && !self.v_flip
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IconSetInfo {
    pub name: String,
    pub author: IconSetAuthor,
    pub license: IconSetLicense,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IconSetAuthor {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IconSetLicense {
    pub title: String,
    pub spdx: String,
    pub url: String,
}

/// One icon ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    /// `"<prefix>:<name>"`, the name under which it is cached and saved.
    pub key: String,
    pub body: String,
    pub width: u32,
    pub height: u32,
}

impl IconSet {
    /// The icon `name` stands for: itself, or the icon a plain alias leads
    /// to. `None` for an unknown name, a cycle, or an alias that turns or
    /// flips.
    pub fn canonical(&self, name: &str) -> Option<&str> {
        let mut name = name;
        for _ in 0..=MAX_ALIAS_STEPS {
            if let Some((key, _)) = self.icons.get_key_value(name) {
                return Some(key);
            }
            let alias = self.aliases.get(name).filter(|alias| alias.is_plain())?;
            name = &alias.parent;
        }
        None
    }

    pub fn glyph(&self, name: &str) -> Option<Glyph> {
        let key = self.canonical(name)?;
        let icon = &self.icons[key];
        Some(Glyph {
            key: format!("{}:{key}", self.prefix),
            body: icon.body.clone(),
            width: icon.width.unwrap_or(self.width),
            height: icon.height.unwrap_or(self.height),
        })
    }

    /// Every icon and every plain alias that leads to one, sorted.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .icons
            .keys()
            .chain(self.aliases.keys())
            .map(String::as_str)
            .filter(|name| self.canonical(name).is_some())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A set whose icons are named `icons`, each a square, plus `aliases`
    /// as `(name, parent)` pairs.
    pub(crate) fn set(icons: &[&str], aliases: &[(&str, &str)]) -> IconSet {
        let mut set = IconSet {
            prefix: "test".to_owned(),
            ..IconSet::default()
        };
        for name in icons {
            let body = format!("<path d=\"{name}\"/>");
            set.icons.insert(
                (*name).to_owned(),
                IconData {
                    body,
                    ..IconData::default()
                },
            );
        }
        for (name, parent) in aliases {
            set.aliases.insert(
                (*name).to_owned(),
                IconAlias {
                    parent: (*parent).to_owned(),
                    ..IconAlias::default()
                },
            );
        }
        set
    }

    #[test]
    fn an_alias_leads_to_its_icon() {
        let set = set(&["image"], &[("picture", "image"), ("pic", "picture")]);
        assert_eq!(set.canonical("pic"), Some("image"));
        let glyph = set.glyph("pic").unwrap();
        assert_eq!(glyph.key, "test:image");
        assert_eq!(glyph.body, "<path d=\"image\"/>");
        assert_eq!(set.glyph("missing"), None);
    }

    #[test]
    fn a_cycle_and_a_dangling_alias_give_none() {
        let set = set(&[], &[("a", "b"), ("b", "a"), ("c", "nowhere")]);
        assert_eq!(set.glyph("a"), None);
        assert_eq!(set.glyph("c"), None);
        assert!(set.names().is_empty());
    }

    #[test]
    fn a_turned_or_flipped_alias_is_refused() {
        let mut set = set(&["arrow"], &[("back", "arrow"), ("up", "arrow")]);
        set.aliases.get_mut("back").unwrap().h_flip = true;
        set.aliases.get_mut("up").unwrap().rotate = 1;
        assert_eq!(set.glyph("back"), None);
        assert_eq!(set.glyph("up"), None);
        assert_eq!(set.names(), ["arrow"]);
    }

    #[test]
    fn sizes_fall_back_to_the_sets() {
        let mut set = set(&["a", "b"], &[]);
        set.width = 20;
        set.height = 24;
        set.icons.get_mut("b").unwrap().width = Some(10);
        let a = set.glyph("a").unwrap();
        assert_eq!((a.width, a.height), (20, 24));
        let b = set.glyph("b").unwrap();
        assert_eq!((b.width, b.height), (10, 24));
    }

    #[test]
    fn a_set_without_sizes_is_16_square() {
        let set = IconSet::default();
        assert_eq!((set.width, set.height), (16, 16));
    }

    #[test]
    fn names_lists_icons_and_plain_aliases_sorted() {
        let set = set(&["zebra", "apple"], &[("fruit", "apple")]);
        assert_eq!(set.names(), ["apple", "fruit", "zebra"]);
    }
}
