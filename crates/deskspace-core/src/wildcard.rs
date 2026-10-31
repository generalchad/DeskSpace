//! `*` and `?` wildcards for a lens's "Show only" and "Hide" lists (D18).
//!
//! No regular expressions: a pattern comes from the config, which the user
//! may edit by hand, so matching must stay cheap whatever it holds.

/// Whether `name` matches `pattern`, where `*` stands for any run of
/// characters (none included) and `?` for exactly one, without case.
///
/// The greedy matcher that remembers only the last `*`: on a mismatch it
/// retries from that star one character further along `name`, and never
/// backtracks to an earlier star, which the later one makes unnecessary. It
/// takes at most `pattern` × `name` steps, however many stars the pattern
/// holds, and allocates nothing.
/// Adapted from Russ Cox, "Glob Matching Can Be Simple And Fast Too" (2017):
/// https://research.swtch.com/glob
pub fn matches(pattern: &str, name: &str) -> bool {
    // The text after the last star must end the name, so check it first: it
    // rejects most names (`*.pdf` against `report.txt`) before any retry.
    if let Some(star) = pattern.rfind('*')
        && !ends_with(name, &pattern[star + 1..])
    {
        return false;
    }
    let (mut p, mut n) = (0, 0);
    // After the last `*` seen: where the pattern resumes, and where in
    // `name` the star's run would end on the next retry.
    let mut retry: Option<(usize, usize)> = None;
    while n < name.len() {
        let wanted = next_char(pattern, p);
        let got = next_char(name, n).expect("n is inside name");
        match wanted {
            Some('*') => {
                p += 1;
                retry = Some((p, n));
                continue;
            }
            Some(want) if want == '?' || same_char(want, got) => {
                p += want.len_utf8();
                n += got.len_utf8();
                continue;
            }
            _ => {}
        }
        let Some((star_p, star_n)) = retry else {
            return false;
        };
        let skipped = next_char(name, star_n).expect("star_n is inside name");
        let resume = star_n + skipped.len_utf8();
        retry = Some((star_p, resume));
        p = star_p;
        n = resume;
    }
    pattern[p..].chars().all(|ch| ch == '*')
}

/// Whether `name` ends with `tail`, a run of pattern text without stars, in
/// which `?` stands for any one character.
fn ends_with(name: &str, tail: &str) -> bool {
    let mut name = name.chars().rev();
    tail.chars().rev().all(|want| {
        name.next()
            .is_some_and(|got| want == '?' || same_char(want, got))
    })
}

/// The `char` that starts at byte `at` of `text`, or `None` at its end.
fn next_char(text: &str, at: usize) -> Option<char> {
    text[at..].chars().next()
}

/// Whether two characters are the same without case, folding each with
/// `to_lowercase` (which may yield several characters) and comparing the
/// results, so nothing is allocated. Two ASCII characters skip the general
/// folding, which is most of a name's characters.
fn same_char(a: char, b: char) -> bool {
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(&b);
    }
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_matches_itself_without_case() {
        assert!(matches("report.pdf", "report.pdf"));
        assert!(matches("REPORT.PDF", "report.pdf"));
        assert!(!matches("report.pdf", "report.pd"));
        assert!(!matches("report.pd", "report.pdf"));
        assert!(matches("", ""));
        assert!(!matches("", "a"));
    }

    #[test]
    fn a_star_matches_any_run_including_none() {
        assert!(matches("*.pdf", "report.pdf"));
        assert!(matches("*.pdf", ".pdf"));
        assert!(!matches("*.pdf", "report.pdf.tmp"));
        assert!(matches("*", ""));
        assert!(matches("*", "anything"));
        assert!(matches("a*b*c", "aXXbYYc"));
        assert!(matches("a*b*c", "abc"));
        assert!(!matches("a*b*c", "acb"));
        assert!(matches("*report*", "Q3 report final.docx"));
    }

    #[test]
    fn a_question_mark_matches_exactly_one_character() {
        assert!(matches("file?.txt", "file1.txt"));
        assert!(!matches("file?.txt", "file.txt"));
        assert!(!matches("file?.txt", "file12.txt"));
        assert!(matches("??", "é漢"), "one character, not one byte");
    }

    #[test]
    fn non_ascii_names_match_without_case() {
        assert!(matches("ÉTÉ*", "été 2026.jpg"));
        assert!(matches("*ΣΟΦΊΑ*", "η σοφία.txt"));
        assert!(matches("*.jpg", "写真.JPG"));
    }

    #[test]
    fn a_later_star_retries_without_going_back_to_an_earlier_one() {
        // The run for the first star must not swallow the "b" the second
        // part needs.
        assert!(matches("*ab*cd", "xxabyycd"));
        assert!(matches("*aab", "aaaab"));
        assert!(!matches("*aab", "aaaa"));
    }

    #[test]
    fn hostile_patterns_stay_fast() {
        let start = std::time::Instant::now();
        // Thousands of stars against a long name with no match at the end.
        let stars = "*".repeat(4_000) + "x";
        let name = "a".repeat(260);
        assert!(!matches(&stars, &name));
        // A pattern at the length cap against a name at the length cap,
        // built to retry from its one star at every position.
        let pattern = "*".to_owned() + &"a".repeat(258) + "b";
        assert!(!matches(&pattern, &name));
        assert!(matches(&("*".to_owned() + &"a".repeat(259)), &name));
        // Generous for a debug build; the work is about 68,000 steps.
        assert!(
            start.elapsed() < std::time::Duration::from_millis(250),
            "{:?}",
            start.elapsed()
        );
    }
}
