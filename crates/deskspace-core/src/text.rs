//! UI text and the language seam (ROADMAP F-60, decided in D20). Every string
//! the UI shows is meant to come from a `Key` through `lookup`, so a language
//! is one catalog away; today the English table is in this file, and the
//! Debug window can switch to a pseudo-language to show what a translated,
//! longer string does to a layout.
//!
//! D20 chooses Fluent for the real catalogs. This module is the seam that
//! change fills in: call sites name a `Key`, never an English sentence, and
//! `lookup` stays the one reader.

use std::fmt;

/// A language the interface can speak. The pseudo-language is English with
/// accents and a bracket frame, as Windows' pseudo-locale is: it is longer
/// than the real text and marks every string, so a layout that clips or a
/// string that never came through the catalog shows at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    Pseudo,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::Pseudo];

    /// The language's name in the Debug window and the settings.
    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Pseudo => "Pseudo (accented English)",
        }
    }

    /// The BCP 47 tag a catalog and Windows' display language share.
    pub fn tag(self) -> &'static str {
        match self {
            Self::English => "en-US",
            // The tag Windows uses for its own pseudo-locale.
            Self::Pseudo => "qps-ploc",
        }
    }

    /// The language `tag` names, else English. A newer build may write a tag
    /// this one does not know.
    pub fn from_tag(tag: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|language| language.tag().eq_ignore_ascii_case(tag))
            .unwrap_or_default()
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A user-visible message. A new message takes the next value; one is never
/// renamed or renumbered, because a translator's catalog names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    /// The application's name, as a window title and in About.
    AppName,
    /// The Debug window's font sample, proving the catalog resolves.
    FontSample,
    /// The Debug window's language sample.
    LanguageSample,
}

impl Key {
    pub const ALL: [Self; 3] = [Self::AppName, Self::FontSample, Self::LanguageSample];
}

/// The English text for `key`. The fallback catalog: every key needs a line
/// here, and `every_key_has_english` checks it.
const fn english(key: Key) -> &'static str {
    match key {
        Key::AppName => "DeskSpace",
        Key::FontSample => "The quick brown fox jumps over the lazy dog.",
        Key::LanguageSample => "This line follows the interface language.",
    }
}

/// `key`'s text in `language`. A pseudo string is the English one with
/// accented vowels and a bracket frame around it, so it is clearly longer.
pub fn lookup(language: Language, key: Key) -> String {
    let text = english(key);
    match language {
        Language::English => text.to_owned(),
        Language::Pseudo => pseudo(text),
    }
}

/// `text` pseudo-localized: `[!! … !!]`, each ASCII vowel accented in turn.
/// The brackets stand in for the extra length and the accent marks for the
/// wider glyphs a translation brings.
fn pseudo(text: &str) -> String {
    const ACCENTS: &[(char, char)] = &[
        ('a', 'á'),
        ('e', 'é'),
        ('i', 'í'),
        ('o', 'ó'),
        ('u', 'ú'),
        ('A', 'Á'),
        ('E', 'É'),
        ('I', 'Í'),
        ('O', 'Ó'),
        ('U', 'Ú'),
    ];
    let accented: String = text
        .chars()
        .map(|ch| {
            ACCENTS
                .iter()
                .find(|(plain, _)| *plain == ch)
                .map_or(ch, |(_, accented)| *accented)
        })
        .collect();
    format!("[!! {accented} !!]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_english() {
        for key in Key::ALL {
            assert!(!english(key).trim().is_empty(), "{key:?}");
        }
    }

    #[test]
    fn english_is_the_text_itself() {
        assert_eq!(lookup(Language::English, Key::AppName), "DeskSpace");
    }

    #[test]
    fn the_pseudo_language_marks_and_lengthens_every_string() {
        for key in Key::ALL {
            let pseudo = lookup(Language::Pseudo, key);
            assert!(pseudo.starts_with("[!! ") && pseudo.ends_with(" !!]"));
            assert!(pseudo.chars().count() > english(key).chars().count());
        }
        assert!(lookup(Language::Pseudo, Key::FontSample).contains('é'));
    }

    #[test]
    fn a_tag_names_its_language_and_an_unknown_one_is_english() {
        assert_eq!(Language::from_tag("en-US"), Language::English);
        assert_eq!(Language::from_tag("QPS-PLOC"), Language::Pseudo);
        assert_eq!(Language::from_tag("fr-FR"), Language::English);
    }

    #[test]
    fn the_language_names_are_unique() {
        let mut labels: Vec<&str> = Language::ALL
            .iter()
            .map(|language| language.label())
            .collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), Language::ALL.len());
    }
}
