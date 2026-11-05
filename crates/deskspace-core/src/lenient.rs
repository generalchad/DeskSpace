//! Reading values a newer build may have written, so an older build still
//! loads the file.

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

/// A value of `T`, or whatever else the file holds there.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Lenient<T> {
    Valid(T),
    Invalid(IgnoredAny),
}

/// Reads a word field so that a word this build does not know becomes
/// `T::default()` rather than failing the whole file. A value of another type
/// is still an error.
pub fn known_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Word<T> {
        Known(T),
        Unknown(String),
    }

    Ok(match Word::deserialize(deserializer)? {
        Word::Known(value) => value,
        Word::Unknown(word) => {
            // The word is deliberately unused: it only proves the file held a
            // string this build does not know.
            let _ = word;
            T::default()
        }
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Word {
        #[default]
        First,
        Second,
    }

    #[derive(Debug, Deserialize)]
    struct Holder {
        #[serde(deserialize_with = "known_or_default")]
        word: Word,
    }

    #[test]
    fn a_known_word_is_kept_and_an_unknown_one_takes_the_default() {
        let known: Holder = toml::from_str("word = 'second'").unwrap();
        assert_eq!(known.word, Word::Second);
        let unknown: Holder = toml::from_str("word = 'from_a_newer_build'").unwrap();
        assert_eq!(unknown.word, Word::First);
    }

    #[test]
    fn a_value_of_another_type_still_fails() {
        assert!(toml::from_str::<Holder>("word = 3").is_err());
    }
}
