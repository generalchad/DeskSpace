//! A space's icon: a reference to a glyph in an icon set, not the picture.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A named icon in a set, written `<set>:<name>` (for example `fluent:games`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconRef {
    pub set: String,
    pub name: String,
}

/// The Iconify name rule: lowercase ASCII letters, digits, and hyphens.
/// <https://iconify.design/docs/types/iconify-json.html>
fn is_icon_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl IconRef {
    /// Parses `<set>:<name>`; `None` unless both halves follow the name rule.
    pub fn parse(text: &str) -> Option<Self> {
        let (set, name) = text.split_once(':')?;
        (is_icon_word(set) && is_icon_word(name)).then(|| Self {
            set: set.to_owned(),
            name: name.to_owned(),
        })
    }
}

impl fmt::Display for IconRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.set, self.name)
    }
}

const AUTOMATIC: &str = "automatic";

/// What a space shows left of its title. Saved as a string: `"automatic"` or
/// `"<set>:<name>"`. Any other string loads as `None`, so a value a newer
/// version wrote never fails the file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum SpaceIcon {
    #[default]
    None,
    /// Picked from the title by matching it against the default icon set.
    Automatic,
    Chosen(IconRef),
}

impl SpaceIcon {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl From<String> for SpaceIcon {
    fn from(text: String) -> Self {
        if text == AUTOMATIC {
            Self::Automatic
        } else {
            IconRef::parse(&text).map_or(Self::None, Self::Chosen)
        }
    }
}

impl From<SpaceIcon> for String {
    fn from(icon: SpaceIcon) -> Self {
        match icon {
            // `None` is never written (`skip_serializing_if`); an empty
            // string loads back as `None` anyway.
            SpaceIcon::None => String::new(),
            SpaceIcon::Automatic => AUTOMATIC.to_owned(),
            SpaceIcon::Chosen(icon) => icon.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_round_trip() {
        let icon = IconRef::parse("fluent:games-20").unwrap();
        assert_eq!(icon.set, "fluent");
        assert_eq!(icon.name, "games-20");
        assert_eq!(icon.to_string(), "fluent:games-20");
    }

    #[test]
    fn bad_references_are_rejected() {
        for bad in [
            "",
            "games",
            ":games",
            "fluent:",
            "a:b:c",
            "Fluent:games",
            "fluent:Games",
            "fluent:ga mes",
            "fluent:gämes",
            "fluent:ga_mes",
        ] {
            assert_eq!(IconRef::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_space_icon_converts_to_and_from_its_string() {
        let chosen = SpaceIcon::Chosen(IconRef::parse("fluent:games").unwrap());
        for icon in [SpaceIcon::Automatic, chosen] {
            assert_eq!(SpaceIcon::from(String::from(icon.clone())), icon);
        }
        assert_eq!(SpaceIcon::from("?".to_owned()), SpaceIcon::None);
        assert_eq!(SpaceIcon::from(String::new()), SpaceIcon::None);
        assert_eq!(SpaceIcon::from("Automatic".to_owned()), SpaceIcon::None);
    }
}
