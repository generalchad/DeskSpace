//! Choosing which labels are worth measuring: a wide label is usually a long
//! one, so character count finds the candidates without a font.

/// The `count` labels with the most characters, longest first. Ties keep their
/// input order. A caller measures only these, so it is cheap enough to run on
/// every item.
pub fn longest_by_chars<'a>(labels: &[&'a str], count: usize) -> Vec<&'a str> {
    let mut candidates = labels.to_vec();
    candidates.sort_by_cached_key(|label| std::cmp::Reverse(label.chars().count()));
    candidates.truncate(count);
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_keeps_the_longest_labels_longest_first() {
        assert_eq!(
            longest_by_chars(&["a", "ccc", "bb", "dddd"], 2),
            ["dddd", "ccc"]
        );
    }

    #[test]
    fn ties_keep_their_input_order() {
        assert_eq!(longest_by_chars(&["aa", "bb", "cc"], 2), ["aa", "bb"]);
    }

    #[test]
    fn it_returns_everything_when_count_is_larger() {
        assert_eq!(longest_by_chars(&["b", "a"], 10), ["b", "a"]);
    }

    #[test]
    fn it_handles_no_labels_and_no_count() {
        assert!(longest_by_chars(&[], 10).is_empty());
        assert!(longest_by_chars(&["a"], 0).is_empty());
    }

    #[test]
    fn it_counts_characters_not_bytes() {
        assert_eq!(longest_by_chars(&["é", "ab"], 1), ["ab"]);
    }
}
