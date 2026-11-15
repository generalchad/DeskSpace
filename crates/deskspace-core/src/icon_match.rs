//! Picking an icon for a space's title, and searching a set (D16).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use crate::icon_set::IconSet;

/// Words that never pick an icon.
pub const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "new", "space", "stuff", "misc", "other",
];

/// Shortest word that picks an icon.
const MIN_WORD_LEN: usize = 3;

const SCORE_CURATED: u32 = 100;
const SCORE_CURATED_SINGULAR: u32 = 95;
const SCORE_CURATED_STEM: u32 = 90;
const SCORE_NAME: u32 = 80;
const SCORE_NAME_SINGULAR: u32 = 75;
const SCORE_NAME_STEM: u32 = 70;
const SCORE_FIRST_PART: u32 = 60;
const SCORE_STEM_FIRST_PART: u32 = 55;
const SCORE_IN_WORD: u32 = 50;

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

/// The stems an "-ing" word may come from, each at least `MIN_WORD_LEN`
/// letters: the word without its ending, an undoubled consonant ("running" to
/// "run"), and the silent "e" put back ("browsing" to "browse"). Only one of
/// the last two is tried, because a word does not both double a consonant and
/// drop an "e".
fn ing_stems(word: &str) -> Vec<String> {
    let Some(base) = word.strip_suffix("ing") else {
        return Vec::new();
    };
    let letters: Vec<char> = base.chars().collect();
    let last = letters.last().copied();
    let doubled = letters.len() >= 2
        && letters[letters.len() - 1] == letters[letters.len() - 2]
        && !"aeiou".contains(letters[letters.len() - 1]);
    let mut stems = vec![base.to_owned()];
    if doubled {
        stems.push(letters[..letters.len() - 1].iter().collect());
    } else if !last.is_some_and(|c| "aeiouy".contains(c)) {
        stems.push(format!("{base}e"));
    }
    stems.retain(|stem| stem.chars().count() >= MIN_WORD_LEN);
    stems.sort_unstable();
    stems.dedup();
    stems
}

/// Whether `word` begins or ends with `candidate`, a word long enough to
/// mean something on its own.
fn starts_or_ends_with(word: &str, candidate: &str) -> bool {
    candidate.chars().count() >= MIN_WORD_LEN
        && (word.starts_with(candidate) || word.ends_with(candidate))
}

/// The icon a curated row names for `word`, if the set has it.
fn curated<'a>(word: &str, set: &'a IconSet, words: &[(&str, &str)]) -> Option<&'a str> {
    let (_, icon) = words.iter().find(|(w, _)| *w == word)?;
    set.canonical(icon)
}

/// What decides between two candidates: the score, then the earlier word,
/// then the shorter name, then alphabetical order.
type Rank<'a> = (u32, Reverse<usize>, Reverse<usize>, Reverse<&'a str>);

/// Keeps `icon` when its score and tie-breaks beat the best so far.
fn consider<'a>(
    best: &mut Option<(Rank<'a>, &'a str)>,
    position: usize,
    score: u32,
    icon: &'a str,
) {
    let rank = (score, Reverse(position), Reverse(icon.len()), Reverse(icon));
    if best.as_ref().is_none_or(|(top, _)| rank > *top) {
        *best = Some((rank, icon));
    }
}

/// Scores every form in `forms` against the set, and says whether any of them
/// found an icon at all, even one another title word beats.
fn score_forms<'a>(
    best: &mut Option<(Rank<'a>, &'a str)>,
    position: usize,
    forms: &[(String, u32, u32, u32)],
    set: &'a IconSet,
    words: &[(&str, &str)],
    names: &[&'a str],
) -> bool {
    let mut found = false;
    for (form, curated_score, name_score, first_score) in forms {
        if let Some(icon) = curated(form, set, words) {
            found = true;
            consider(best, position, *curated_score, icon);
        }
        for name in names {
            let Some(icon) = set.canonical(name) else {
                continue;
            };
            if *name == form.as_str() {
                found = true;
                consider(best, position, *name_score, icon);
            } else if name.split('-').next() == Some(form.as_str()) {
                found = true;
                consider(best, position, *first_score, icon);
            }
        }
    }
    found
}

/// The best icon for a space's title, or `None` when nothing scores.
///
/// Each word of the title (without stop words and short words) scores as
/// written and in its singular form: 100 and 95 for a curated `words` row, 80
/// and 75 for an icon or plain alias named exactly so, 60 for an icon whose
/// first hyphen-separated part is the form. A word that finds no icon that way
/// also tries its "-ing" stem (90, 70, or 55, as a curated row, an exact name,
/// or a first part), so "Browsing" reaches "browse". A word still finding
/// nothing tries a curated word or icon name of at least `MIN_WORD_LEN`
/// letters at its start or end, at 50, so "Artcraft" reaches "art". The highest
/// score wins; ties go to the earlier word, then the shorter name, then
/// alphabetical order.
pub fn best_icon<'a>(title: &str, set: &'a IconSet, words: &[(&str, &str)]) -> Option<&'a str> {
    let names = set.names();
    let mut best: Option<(Rank<'a>, &'a str)> = None;
    let title_words = split_words(title)
        .into_iter()
        .filter(|w| w.chars().count() >= MIN_WORD_LEN && !STOP_WORDS.contains(&w.as_str()));
    for (position, word) in title_words.enumerate() {
        let mut forms = vec![(word.clone(), SCORE_CURATED, SCORE_NAME, SCORE_FIRST_PART)];
        if let Some(single) = singular(&word) {
            forms.push((
                single,
                SCORE_CURATED_SINGULAR,
                SCORE_NAME_SINGULAR,
                SCORE_FIRST_PART,
            ));
        }
        let mut found = score_forms(&mut best, position, &forms, set, words, &names);
        if !found {
            let stems: Vec<(String, u32, u32, u32)> = ing_stems(&word)
                .into_iter()
                .map(|stem| {
                    (
                        stem,
                        SCORE_CURATED_STEM,
                        SCORE_NAME_STEM,
                        SCORE_STEM_FIRST_PART,
                    )
                })
                .collect();
            found = score_forms(&mut best, position, &stems, set, words, &names);
        }
        if found {
            continue;
        }
        // The fallback scores below every form above, so it can add a match
        // where a word had none but never changes one it already had.
        for (curated_word, icon) in words {
            if starts_or_ends_with(&word, curated_word)
                && let Some(icon) = set.canonical(icon)
            {
                consider(&mut best, position, SCORE_IN_WORD, icon);
            }
        }
        for name in &names {
            if starts_or_ends_with(&word, name)
                && let Some(icon) = set.canonical(name)
            {
                consider(&mut best, position, SCORE_IN_WORD, icon);
            }
        }
    }
    best.map(|(_, icon)| icon)
}

/// An icon set prepared for the picker's search box.
///
/// Building it reads every icon's name parts once, so a query compares them
/// instead of rebuilding them, and one index answers any number of queries.
pub struct SearchIndex<'a> {
    /// Each icon, alphabetically, with the hyphen-separated parts of its own
    /// name and of its aliases' names.
    icons: Vec<(&'a str, Vec<&'a str>)>,
    set: &'a IconSet,
    words: &'a [(&'a str, &'a str)],
}

impl<'a> SearchIndex<'a> {
    /// Builds the index of `set`, whose curated words are `words`.
    pub fn new(set: &'a IconSet, words: &'a [(&'a str, &'a str)]) -> Self {
        let mut parts: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for name in set.names() {
            if let Some(icon) = set.canonical(name) {
                parts.entry(icon).or_default().extend(name.split('-'));
            }
        }
        Self {
            icons: parts.into_iter().collect(),
            set,
            words,
        }
    }

    /// The icons for the picker's search box, best first.
    ///
    /// An icon matches when every query word is a prefix of one of its
    /// hyphen-separated name parts (or an alias's), or a curated row maps the
    /// word to it. Curated hits come first, then icons with a part equal to a
    /// query word, then the rest, each group alphabetical. An empty query
    /// lists every icon.
    pub fn search(&self, query: &str) -> Vec<&'a str> {
        let query = split_words(query);
        // Each query word is answered once, rather than once per icon.
        let answers: Vec<Option<&str>> = query
            .iter()
            .map(|word| curated(word, self.set, self.words))
            .collect();
        let mut groups: [BTreeSet<&str>; 3] = Default::default();
        for (icon, icon_parts) in &self.icons {
            let mut by_curated = false;
            let mut by_equal = false;
            let all_match = query.iter().zip(&answers).all(|(word, answer)| {
                if *answer == Some(*icon) {
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
                groups[group].insert(*icon);
            }
        }
        groups.into_iter().flatten().collect()
    }
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
    fn ing_words_lose_their_ending_or_double_letter() {
        assert_eq!(ing_stems("browsing"), ["brows", "browse"]);
        assert_eq!(ing_stems("running"), ["run", "runn"]);
        assert_eq!(ing_stems("shopping"), ["shop", "shopp"]);
        assert_eq!(ing_stems("reading"), ["read", "reade"]);
        assert_eq!(ing_stems("studying"), ["study"]);
        assert!(ing_stems("being").is_empty(), "no stem of 3 letters");
        assert!(
            ing_stems("sing").is_empty(),
            "a stem under 3 letters is dropped"
        );
        assert!(ing_stems("browse").is_empty(), "not an -ing word");
    }

    #[test]
    fn an_ing_word_tries_its_stem_only_when_it_finds_nothing() {
        let words = [("browse", "globe")];
        let globe = set(&["globe"], &[]);
        assert_eq!(best_icon("Web Browsing", &globe, &words), Some("globe"));
        assert_eq!(best_icon("Browsing", &globe, &words), Some("globe"));
        // A word that finds any icon keeps it, even a first part.
        let named = set(&["browsing-list", "globe"], &[]);
        assert_eq!(best_icon("Browsing", &named, &words), Some("browsing-list"));
        let playing = set(&["playing-cards", "play"], &[]);
        assert_eq!(
            best_icon("Playing Cards", &playing, &[]),
            Some("playing-cards")
        );
        // The word's own curated row still wins.
        let both = set(&["globe"], &[]);
        assert_eq!(
            best_icon(
                "Browsing",
                &both,
                &[("browsing", "chat"), ("browse", "globe")]
            ),
            Some("globe"),
            "chat is not in this set, so the stem's curated row wins"
        );
        let chat_and_globe = set(&["chat", "globe"], &[]);
        assert_eq!(
            best_icon(
                "Browsing",
                &chat_and_globe,
                &[("browsing", "chat"), ("browse", "globe")],
            ),
            Some("chat"),
            "a curated row for the word as written beats its stem"
        );
    }

    #[test]
    fn an_undoubled_stem_reaches_an_exact_name() {
        let run = set(&["run"], &[]);
        assert_eq!(best_icon("Running", &run, &[]), Some("run"));
        let coded = set(&["code"], &[]);
        assert_eq!(best_icon("Coding", &coded, &[]), Some("code"));
        let designed = set(&["design-ideas"], &[]);
        assert_eq!(
            best_icon("Designing", &designed, &[("design", "design-ideas")]),
            Some("design-ideas")
        );
    }

    #[test]
    fn a_word_falls_back_to_a_curated_word_inside_it() {
        let words = [("art", "paint-brush")];
        let set = set(&["paint-brush"], &[]);
        assert_eq!(best_icon("Artcraft", &set, &words), Some("paint-brush"));
        let words = [("craft", "paint-brush")];
        assert_eq!(best_icon("Artcraft", &set, &words), Some("paint-brush"));
    }

    #[test]
    fn a_word_falls_back_to_an_icon_name_inside_it() {
        let start = set(&["art"], &[]);
        assert_eq!(best_icon("Artcraft", &start, &[]), Some("art"));
        let end = set(&["craft"], &[]);
        assert_eq!(best_icon("Artcraft", &end, &[]), Some("craft"));
        let short = set(&["an"], &[]);
        assert_eq!(best_icon("Banana", &short, &[]), None);
    }

    #[test]
    fn a_fallback_scores_below_every_form() {
        let set = set(&["art", "artcraft-album"], &[]);
        assert_eq!(
            best_icon("Artcraft", &set, &[]),
            Some("artcraft-album"),
            "a first part beats a name inside the word"
        );
    }

    #[test]
    fn titles_that_gain_an_icon() {
        let set = set(
            &[
                "mail",
                "chat",
                "globe",
                "paint-brush",
                "document",
                "code",
                "run",
                "notebook",
                "calendar",
            ],
            &[],
        );
        let words = [
            ("email", "mail"),
            ("inbox", "mail"),
            ("message", "chat"),
            ("browse", "globe"),
            ("internet", "globe"),
            ("doc", "document"),
            ("art", "paint-brush"),
            ("journal", "notebook"),
            ("schedule", "calendar"),
        ];
        for (title, expected) in [
            ("Email & Communication", "mail"),
            ("Inbox", "mail"),
            ("Team Messages", "chat"),
            ("Web Browsing", "globe"),
            ("Internet", "globe"),
            ("Coding", "code"),
            ("Running", "run"),
            ("My Docs", "document"),
            ("Artcraft", "paint-brush"),
            ("Journal", "notebook"),
            ("Weekly Schedule", "calendar"),
        ] {
            assert_eq!(
                best_icon(title, &set, &words),
                Some(expected),
                "title {title:?}"
            );
        }
        assert_eq!(best_icon("Stuff", &set, &words), None);
    }

    #[test]
    fn titles_that_pick_an_icon_today_keep_it() {
        let words = [("shopping", "cart"), ("writing", "pen")];
        let set = set(
            &[
                "photos",
                "shopping-bag",
                "cart",
                "reading-list",
                "writing",
                "pen",
                "games",
            ],
            &[],
        );
        assert_eq!(best_icon("Photos", &set, &words), Some("photos"));
        assert_eq!(best_icon("Shopping", &set, &words), Some("cart"));
        assert_eq!(
            best_icon("Reading", &set, &words),
            Some("reading-list"),
            "its -ing stem finds no other icon"
        );
        assert_eq!(best_icon("Writing", &set, &words), Some("pen"));
        assert_eq!(best_icon("Games", &set, &words), Some("games"));
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
        let index = SearchIndex::new(&set, &[]);
        assert_eq!(index.search(""), ["a", "b"]);
        assert_eq!(index.search("  "), ["a", "b"]);
    }

    #[test]
    fn search_matches_name_part_prefixes() {
        let set = set(&["photo-album", "camera-photo", "phone", "film"], &[]);
        let index = SearchIndex::new(&set, &[]);
        assert_eq!(
            index.search("pho"),
            ["camera-photo", "phone", "photo-album"]
        );
        assert_eq!(index.search("photo"), ["camera-photo", "photo-album"]);
    }

    #[test]
    fn search_needs_every_word_to_match() {
        let set = set(&["photo-album", "photo-camera", "music-album"], &[]);
        let index = SearchIndex::new(&set, &[]);
        assert_eq!(index.search("photo album"), ["photo-album"]);
        assert!(index.search("photo zebra").is_empty());
    }

    #[test]
    fn search_orders_curated_then_equal_part_then_prefix() {
        let set = set(&["camera", "photo-album", "photograph", "film"], &[]);
        let words = [("photo", "film")];
        let index = SearchIndex::new(&set, &words);
        assert_eq!(index.search("photo"), ["film", "photo-album", "photograph"]);
    }

    #[test]
    fn search_finds_an_icon_by_its_alias_and_lists_it_once() {
        let set = set(
            &["image", "picture-frame"],
            &[("pic", "image"), ("picture", "image")],
        );
        let index = SearchIndex::new(&set, &[]);
        assert_eq!(index.search("pic"), ["image", "picture-frame"]);
        // "picture" is an alias's whole part, so image has an equal part.
        assert_eq!(index.search("picture"), ["image", "picture-frame"]);
    }

    #[test]
    fn search_skips_a_curated_row_for_a_missing_icon() {
        let set = set(&["film"], &[]);
        let index = SearchIndex::new(&set, &[("photo", "gone")]);
        assert!(index.search("photo").is_empty());
    }

    #[test]
    fn one_index_answers_more_than_one_query() {
        let set = set(&["photo-album", "camera-photo", "phone", "film"], &[]);
        let index = SearchIndex::new(&set, &[]);
        assert_eq!(
            index.search("pho"),
            ["camera-photo", "phone", "photo-album"]
        );
        assert_eq!(index.search("film"), ["film"]);
        assert!(index.search("zebra").is_empty());
    }

    #[test]
    fn a_deprecated_icon_is_never_picked() {
        let mut set = set(&["camera", "photo"], &[]);
        set.icons.get_mut("photo").unwrap().hidden = true;
        let index = SearchIndex::new(&set, &[]);
        assert_eq!(best_icon("Photos", &set, &[]), None);
        assert!(index.search("photo").is_empty());
        assert_eq!(index.search(""), ["camera"]);
    }
}
