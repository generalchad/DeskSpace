//! What each kind of text in DeskSpace is drawn with (the type scale): a font
//! family, a size at 96 DPI, and a weight per role. The platform turns a role
//! into an `HFONT` for GDI and an `IDWriteTextFormat` for Direct2D, so the
//! settings window, the debug window, dialogs, and the spaces' own text all
//! read one table.
//!
//! The values are 96-DPI pixels, not points: GDI's `lfHeight` and
//! DirectWrite's text-format size are the same number then, and a role's size
//! means the same thing whichever painter draws it. A family of `""` means
//! Windows' message font (Segoe UI Variable Text on Windows 11, else Segoe
//! UI).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::lenient::Lenient;

/// One kind of text, from the smallest to the largest role.
///
/// `ALL` is the order the settings and debug windows list roles in; a role is
/// never removed or renumbered, so a saved name keeps meaning what it meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Body text: labels, check boxes, buttons, values, and the debug console.
    Body,
    /// A page's largest heading.
    Heading1,
    /// A section or dialog heading.
    Heading2,
    /// A group's heading, and the settings window's semibold rows.
    Heading3,
    /// A note or description under a field.
    Caption,
    /// A space's title bar.
    SpaceTitle,
    /// An item's label.
    ItemLabel,
    /// A Details, Tiles, or Content cell.
    Details,
    /// A space's placeholder text (an unavailable lens, or "No items match").
    Placeholder,
    /// The zoom pop-up's percentage.
    Zoom,
}

impl Role {
    pub const ALL: [Self; 10] = [
        Self::Body,
        Self::Heading1,
        Self::Heading2,
        Self::Heading3,
        Self::Caption,
        Self::SpaceTitle,
        Self::ItemLabel,
        Self::Details,
        Self::Placeholder,
        Self::Zoom,
    ];

    /// The role's name in the settings and debug windows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Body => "Body",
            Self::Heading1 => "Heading 1",
            Self::Heading2 => "Heading 2",
            Self::Heading3 => "Heading 3",
            Self::Caption => "Caption",
            Self::SpaceTitle => "Space title",
            Self::ItemLabel => "Item label",
            Self::Details => "Details",
            Self::Placeholder => "Placeholder",
            Self::Zoom => "Zoom",
        }
    }

    /// The role's name in the configuration file. Never renamed: a saved file
    /// keys its fonts by it.
    pub fn key(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Heading1 => "heading1",
            Self::Heading2 => "heading2",
            Self::Heading3 => "heading3",
            Self::Caption => "caption",
            Self::SpaceTitle => "space_title",
            Self::ItemLabel => "item_label",
            Self::Details => "details",
            Self::Placeholder => "placeholder",
            Self::Zoom => "zoom",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.key() == key)
    }

    /// What this role is drawn for, when the settings and debug windows do not
    /// show it directly.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Body => None,
            Self::SpaceTitle => Some("The title bar of every space."),
            Self::ItemLabel => {
                Some("Item labels; the space's label-size setting scales this size.")
            }
            Self::Details => Some("Details, Tiles, and Content cells."),
            Self::Placeholder => Some("A space's message when it has nothing to show."),
            Self::Zoom => Some("The zoom pop-up's percentage."),
            _ => None,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// The allowed size of a role, at 96 DPI.
pub const FONT_SIZE_RANGE: (f32, f32) = (6.0, 48.0);
/// The weights a font can take, and their Windows names.
pub const FONT_WEIGHTS: &[(u16, &str)] = &[(400, "Regular"), (600, "Semibold"), (700, "Bold")];

/// One role's font.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Font {
    /// A family name, or `""` for Windows' message font.
    pub family: String,
    /// The size at 96 DPI, in pixels.
    pub size: f32,
    /// 400, 600, or 700, as `FONT_WEIGHTS` lists them.
    pub weight: u16,
}

impl Font {
    pub fn new(family: impl Into<String>, size: f32, weight: u16) -> Self {
        Self {
            family: family.into(),
            size,
            weight,
        }
    }

    /// The same font within the allowed ranges: an out-of-range size clamps,
    /// and a weight no preset names becomes the nearest preset.
    pub fn clamped(&self) -> Self {
        Self {
            family: self.family.trim().to_owned(),
            size: if self.size.is_finite() {
                self.size.clamp(FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1)
            } else {
                Self::default_for(Role::Body).size
            },
            weight: nearest_weight(self.weight),
        }
    }

    /// The default font for `role`, which is what a fresh run starts with.
    pub fn default_for(role: Role) -> Self {
        let (size, weight) = match role {
            Role::Body | Role::Caption => (12.0, 400),
            Role::Heading1 => (16.0, 600),
            Role::Heading2 => (14.0, 600),
            Role::Heading3 => (12.0, 600),
            Role::SpaceTitle => (13.0, 600),
            Role::ItemLabel | Role::Details | Role::Placeholder => (12.0, 400),
            Role::Zoom => (14.0, 600),
        };
        Self::new("", size, weight)
    }
}

impl Default for Font {
    fn default() -> Self {
        Self::default_for(Role::Body)
    }
}

fn nearest_weight(weight: u16) -> u16 {
    FONT_WEIGHTS
        .iter()
        .map(|(value, _)| *value)
        .min_by_key(|value| value.abs_diff(weight))
        .unwrap_or(400)
}

/// Every role's font. `Default` is the type scale every run starts with.
///
/// Saved as a table per role that differs from its default, so a role left
/// alone follows a later build's default:
///
/// ```toml
/// [typography.space_title]
/// family = "Bahnschrift"
/// size = 14.0
/// weight = 600
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Typography {
    fonts: [Font; Role::ALL.len()],
}

// Every font in a `Typography` went through `Font::clamped`, so no size is
// NaN and equality is reflexive.
impl Eq for Typography {}

impl Serialize for Typography {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let changed: BTreeMap<&str, &Font> = self
            .iter()
            .filter(|(role, font)| **font != Font::default_for(*role))
            .map(|(role, font)| (role.key(), font))
            .collect();
        changed.serialize(serializer)
    }
}

/// A role's saved font, where a missing part keeps the role's default.
#[derive(Deserialize)]
struct SavedFont {
    family: Option<String>,
    size: Option<f32>,
    weight: Option<u16>,
}

impl<'de> Deserialize<'de> for Typography {
    /// A role this build does not know, or a font it cannot read, is skipped
    /// rather than failing the whole file; the role keeps its default.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let saved = BTreeMap::<String, Lenient<SavedFont>>::deserialize(deserializer)?;
        let mut typography = Self::new();
        for (key, font) in saved {
            let (Some(role), Lenient::Valid(font)) = (Role::from_key(&key), font) else {
                continue;
            };
            let default = Font::default_for(role);
            typography.set(
                role,
                Font {
                    family: font.family.unwrap_or(default.family),
                    size: font.size.unwrap_or(default.size),
                    weight: font.weight.unwrap_or(default.weight),
                },
            );
        }
        Ok(typography)
    }
}

impl Typography {
    pub fn new() -> Self {
        Self {
            fonts: Role::ALL.map(Font::default_for),
        }
    }

    pub fn font(&self, role: Role) -> &Font {
        &self.fonts[role as usize]
    }

    /// Sets `role`'s font, clamped to the allowed ranges.
    pub fn set(&mut self, role: Role, font: Font) {
        self.fonts[role as usize] = font.clamped();
    }

    /// Every role's font in `Role::ALL` order.
    pub fn iter(&self) -> impl Iterator<Item = (Role, &Font)> {
        Role::ALL.into_iter().map(|role| (role, self.font(role)))
    }

    /// Every role within the allowed ranges.
    pub fn clamped(&self) -> Self {
        Role::ALL
            .into_iter()
            .fold(Self::new(), |mut typography, role| {
                typography.set(role, self.font(role).clone());
                typography
            })
    }
}

impl Default for Typography {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_has_a_default_within_the_range() {
        let typography = Typography::new();
        for (role, font) in typography.iter() {
            assert!((FONT_SIZE_RANGE.0..=FONT_SIZE_RANGE.1).contains(&font.size));
            assert!(
                FONT_WEIGHTS
                    .iter()
                    .any(|(weight, _)| *weight == font.weight),
                "{role}"
            );
        }
    }

    #[test]
    fn a_role_keeps_its_font_apart_from_the_others() {
        let mut typography = Typography::new();
        let heading = typography.font(Role::Heading1).clone();
        typography.set(Role::Body, Font::new("Consolas", 10.0, 400));
        assert_eq!(typography.font(Role::Body).family, "Consolas");
        assert_eq!(typography.font(Role::Heading1), &heading);
    }

    #[test]
    fn an_out_of_range_font_clamps() {
        let mut typography = Typography::new();
        typography.set(Role::Body, Font::new("  Georgia  ", 100.0, 500));
        let font = typography.font(Role::Body);
        assert_eq!(font.family, "Georgia", "trimmed");
        assert_eq!(font.size, FONT_SIZE_RANGE.1);
        assert_eq!(font.weight, 400, "500 is nearest Regular");
        typography.set(Role::Body, Font::new("", f32::NAN, 600));
        assert_eq!(typography.font(Role::Body).size, 12.0);
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct File {
        #[serde(default)]
        typography: Typography,
    }

    fn read(text: &str) -> Typography {
        toml::from_str::<File>(text).unwrap().typography
    }

    #[test]
    fn the_default_scale_saves_nothing() {
        let text = toml::to_string(&File {
            typography: Typography::new(),
        })
        .unwrap();
        assert!(!text.contains("typography."), "{text}");
        assert_eq!(read(&text), Typography::new());
    }

    #[test]
    fn a_changed_role_saves_and_loads_alone() {
        let mut typography = Typography::new();
        typography.set(Role::SpaceTitle, Font::new("Bahnschrift", 14.5, 700));
        let text = toml::to_string(&File {
            typography: typography.clone(),
        })
        .unwrap();
        assert!(text.contains("[typography.space_title]"), "{text}");
        assert!(!text.contains("body"), "{text}");
        assert_eq!(read(&text), typography);
    }

    #[test]
    fn a_missing_part_keeps_the_roles_default() {
        let typography = read("[typography.heading1]\nfamily = \"Georgia\"\n");
        let font = typography.font(Role::Heading1);
        assert_eq!(font.family, "Georgia");
        assert_eq!(font.size, Font::default_for(Role::Heading1).size);
        assert_eq!(font.weight, Font::default_for(Role::Heading1).weight);
    }

    #[test]
    fn an_unknown_role_or_unreadable_font_is_skipped() {
        let typography = read(
            "[typography.sidebar]\nsize = 20.0\n\
             [typography.body]\nsize = \"huge\"\n\
             [typography.caption]\nsize = 99.0\n",
        );
        assert_eq!(typography.font(Role::Body), &Font::default_for(Role::Body));
        assert_eq!(
            typography.font(Role::Caption).size,
            FONT_SIZE_RANGE.1,
            "clamped on load"
        );
    }

    #[test]
    fn the_role_keys_are_unique() {
        for role in Role::ALL {
            assert_eq!(Role::from_key(role.key()), Some(role));
        }
    }

    #[test]
    fn the_role_names_are_unique() {
        let mut labels: Vec<&str> = Role::ALL.iter().map(|role| role.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), Role::ALL.len());
    }
}
