//! Picking an icon for a space's title, and searching a set (D16).

use std::cmp::Reverse;
use std::collections::BTreeSet;

use crate::icon_set::IconSet;

/// Words that never pick an icon.
pub const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "new", "space", "stuff", "misc", "other",
];

/// Shortest word that picks an icon.
const MIN_WORD_LEN: usize = 3;

const SCORE_CURATED: u32 = 100;
const SCORE_CURATED_SINGULAR: u32 = 95;
const SCORE_NAME: u32 = 80;
const SCORE_NAME_SINGULAR: u32 = 75;
const SCORE_FIRST_PART: u32 = 60;

/// Lowercase alphanumeric runs of `text`.
fn split_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The singular of an English plural, if the word looks like one:
/// "-ies" to "-y", "-es" after s, x, ch, or sh, else a trailing "s" on a word
/// longer than 3 letters.
fn singular(word: &str) -> Option<String> {
    if word.chars().count() <= 3 {
        return None;
    }
    if let Some(stem) = word.strip_suffix("ies") {
        return Some(format!("{stem}y"));
    }
    if let Some(stem) = word.strip_suffix("es")
        && ["s", "x", "ch", "sh"].iter().any(|end| stem.ends_with(end))
    {
        return Some(stem.to_owned());
    }
    word.strip_suffix('s').map(str::to_owned)
}

/// The icon a curated row names for `word`, if the set has it.
fn curated<'a>(word: &str, set: &'a IconSet, words: &[(&str, &str)]) -> Option<&'a str> {
    let (_, icon) = words.iter().find(|(w, _)| *w == word)?;
    set.canonical(icon)
}

/// What decides between two candidates: the score, then the earlier word,
/// then the shorter name, then alphabetical order.
type Rank<'a> = (u32, Reverse<usize>, Reverse<usize>, Reverse<&'a str>);

/// The best icon for a space's title, or `None` when nothing scores.
///
/// Each word of the title (without stop words and short words) scores as
/// written and in its singular form: 100 and 95 for a curated `words` row, 80
/// and 75 for an icon or plain alias named exactly so, 60 for an icon whose
/// first hyphen-separated part is the word. The highest score wins; ties go
/// to the earlier word, then the shorter name, then alphabetical order.
pub fn best_icon<'a>(title: &str, set: &'a IconSet, words: &[(&str, &str)]) -> Option<&'a str> {
    let names = set.names();
    let mut best: Option<(Rank<'a>, &'a str)> = None;
    let title_words = split_words(title)
        .into_iter()
        .filter(|w| w.chars().count() >= MIN_WORD_LEN && !STOP_WORDS.contains(&w.as_str()));
    for (position, word) in title_words.enumerate() {
        let single = singular(&word);
        let forms = [
            (Some(word.as_str()), SCORE_CURATED, SCORE_NAME),
            (
                single.as_deref(),
                SCORE_CURATED_SINGULAR,
                SCORE_NAME_SINGULAR,
            ),
        ];
        for (form, curated_score, name_score) in forms {
            let Some(form) = form else { continue };
            let mut consider = |score: u32, icon: &'a str| {
                let rank = (score, Reverse(position), Reverse(icon.len()), Reverse(icon));
                if best.as_ref().is_none_or(|(top, _)| rank > *top) {
                    best = Some((rank, icon));
                }
            };
            if let Some(icon) = curated(form, set, words) {
                consider(curated_score, icon);
            }
            for name in &names {
                let Some(icon) = set.canonical(name) else {
                    continue;
                };
                if *name == form {
                    consider(name_score, icon);
                } else if name.split('-').next() == Some(form) {
                    consider(SCORE_FIRST_PART, icon);
                }
            }
        }
    }
    best.map(|(_, icon)| icon)
}

/// The icons for the picker's search box, best first.
///
/// An icon matches when every query word is a prefix of one of its
/// hyphen-separated name parts (or an alias's), or a curated row maps the word
/// to it. Curated hits come first, then icons with a part equal to a query
/// word, then the rest, each group alphabetical. An empty query lists every
/// icon.
pub fn search<'a>(query: &str, set: &'a IconSet, words: &[(&str, &str)]) -> Vec<&'a str> {
    let query = split_words(query);
    // Each icon with the name parts of itself and of its aliases.
    let mut parts: Vec<(&str, Vec<&str>)> = Vec::new();
    for name in set.names() {
        let Some(icon) = set.canonical(name) else {
            continue;
        };
        let name_parts = name.split('-');
        match parts.iter_mut().find(|(i, _)| *i == icon) {
            Some((_, all)) => all.extend(name_parts),
            None => parts.push((icon, name_parts.collect())),
        }
    }
    let mut groups: [BTreeSet<&str>; 3] = Default::default();
    for (icon, icon_parts) in parts {
        let mut by_curated = false;
        let mut by_equal = false;
        let all_match = query.iter().all(|word| {
            if curated(word, set, words) == Some(icon) {
                by_curated = true;
                return true;
            }
            by_equal |= icon_parts.contains(&word.as_str());
            icon_parts
                .iter()
                .any(|part| part.starts_with(word.as_str()))
        });
        if all_match {
            let group = if by_curated {
                0
            } else if by_equal {
                1
            } else {
                2
            };
            groups[group].insert(icon);
        }
    }
    groups.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon_set::tests::set;

    #[test]
    fn plurals_become_singular() {
        assert_eq!(singular("libraries").as_deref(), Some("library"));
        assert_eq!(singular("boxes").as_deref(), Some("box"));
        assert_eq!(singular("buses").as_deref(), Some("bus"));
        assert_eq!(singular("watches").as_deref(), Some("watch"));
        assert_eq!(singular("dishes").as_deref(), Some("dish"));
        assert_eq!(singular("games").as_deref(), Some("game"));
        assert_eq!(singular("cats").as_deref(), Some("cat"));
        assert_eq!(singular("bus"), None);
        assert_eq!(singular("game"), None);
    }

    #[test]
    fn common_plural_titles_reach_their_icon() {
        let set = set(&["library", "box", "game"], &[]);
        assert_eq!(best_icon("Libraries", &set, &[]), Some("library"));
        assert_eq!(best_icon("Boxes", &set, &[]), Some("box"));
        assert_eq!(best_icon("Games", &set, &[]), Some("game"));
    }

    #[test]
    fn each_score_level_beats_the_one_below() {
        let words = [("photos", "film"), ("photo", "camera")];
        let all = set(&["camera", "film", "photo", "photos", "photo-album"], &[]);
        // 100: a curated row for the word as written.
        assert_eq!(best_icon("photos", &all, &words), Some("film"));
        // 95: a curated row for its singular beats an exact name.
        assert_eq!(best_icon("photos", &all, &words[1..]), Some("camera"));
        // 80: an exact name beats the singular's name.
        assert_eq!(best_icon("photos", &all, &[]), Some("photos"));
        // 75: the singular's name beats a first part.
        let singular_name = set(&["photo", "photo-album"], &[]);
        assert_eq!(best_icon("photos", &singular_name, &[]), Some("photo"));
        // 60: a first part alone still matches.
        let first_part = set(&["photo-album"], &[]);
        assert_eq!(best_icon("photos", &first_part, &[]), Some("photo-album"));
    }

    #[test]
    fn a_curated_row_for_an_icon_the_set_lacks_is_skipped() {
        let set = set(&["photo"], &[]);
        assert_eq!(
            best_icon("photo", &set, &[("photo", "gone")]),
            Some("photo")
        );
    }

    #[test]
    fn an_alias_resolves_to_its_icon() {
        let set = set(&["image"], &[("picture", "image")]);
        assert_eq!(best_icon("Pictures", &set, &[]), Some("image"));
    }

    #[test]
    fn ties_go_to_the_earlier_word() {
        let set = set(&["music", "video"], &[]);
        assert_eq!(best_icon("video music", &set, &[]), Some("video"));
        assert_eq!(best_icon("music video", &set, &[]), Some("music"));
    }

    #[test]
    fn ties_in_one_word_go_to_the_shorter_name_then_alphabetical() {
        let long = set(&["game-pad", "game-controller"], &[]);
        assert_eq!(best_icon("games", &long, &[]), Some("game-pad"));
        let same = set(&["game-bb", "game-aa"], &[]);
        assert_eq!(best_icon("games", &same, &[]), Some("game-aa"));
    }

    #[test]
    fn stop_words_and_short_words_never_match() {
        let set = set(&["the", "go", "new", "space", "stuff", "misc"], &[]);
        assert_eq!(best_icon("The go", &set, &[]), None);
        assert_eq!(best_icon("New space", &set, &[]), None);
        assert_eq!(best_icon("Stuff misc", &set, &[]), None);
        assert_eq!(best_icon("", &set, &[]), None);
    }

    #[test]
    fn punctuation_splits_words() {
        let set = set(&["game"], &[]);
        assert_eq!(best_icon("Work/Games!", &set, &[]), Some("game"));
    }

    #[test]
    fn search_with_an_empty_query_lists_every_icon() {
        let set = set(&["b", "a"], &[("c", "a")]);
        assert_eq!(search("", &set, &[]), ["a", "b"]);
        assert_eq!(search("  ", &set, &[]), ["a", "b"]);
    }

    #[test]
    fn search_matches_name_part_prefixes() {
        let set = set(&["photo-album", "camera-photo", "phone", "film"], &[]);
        assert_eq!(
            search("pho", &set, &[]),
            ["camera-photo", "phone", "photo-album"]
        );
        assert_eq!(search("photo", &set, &[]), ["camera-photo", "photo-album"]);
    }

    #[test]
    fn search_needs_every_word_to_match() {
        let set = set(&["photo-album", "photo-camera", "music-album"], &[]);
        assert_eq!(search("photo album", &set, &[]), ["photo-album"]);
        assert!(search("photo zebra", &set, &[]).is_empty());
    }

    #[test]
    fn search_orders_curated_then_equal_part_then_prefix() {
        let set = set(&["camera", "photo-album", "photograph", "film"], &[]);
        let words = [("photo", "film")];
        assert_eq!(
            search("photo", &set, &words),
            ["film", "photo-album", "photograph"]
        );
    }

    #[test]
    fn search_finds_an_icon_by_its_alias_and_lists_it_once() {
        let set = set(
            &["image", "picture-frame"],
            &[("pic", "image"), ("picture", "image")],
        );
        assert_eq!(search("pic", &set, &[]), ["image", "picture-frame"]);
        // "picture" is an alias's whole part, so image has an equal part.
        assert_eq!(search("picture", &set, &[]), ["image", "picture-frame"]);
    }

    #[test]
    fn search_skips_a_curated_row_for_a_missing_icon() {
        let set = set(&["film"], &[]);
        assert!(search("photo", &set, &[("photo", "gone")]).is_empty());
    }
}
