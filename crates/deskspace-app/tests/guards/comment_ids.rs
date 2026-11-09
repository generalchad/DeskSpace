//! Fails on a task id in a `//` or `///` comment (`ENGINEERING.md` section
//! 4, rule 10): an id sends a reader to the task list, and ages as tasks
//! finish, so a comment says the why in its own words and the commit message
//! carries the id.
//!
//! A task id is a section letter `docs/TASKS.md`, its archive, or
//! `docs/ROADMAP.md` defines ids under, and a number, as a whole word: `W19`,
//! `R14a`. ROADMAP's decision ids (`D12`) pass, and so does `F`, whose
//! archived ids cannot be told from key names such as F2; `F-45` and `M4`
//! never match.

use std::collections::BTreeSet;
use std::fs;

use super::support::{relative, rust_files, workspace_root};

/// The files that define ids, read for their section letters.
const ID_SOURCES: &[&str] = &[
    "docs/TASKS.md",
    "docs/archive/TASKS-done.md",
    "docs/ROADMAP.md",
];

/// Letters a comment may use with a number: ROADMAP's decisions, and F.
const ALLOWED_LETTERS: &[char] = &['D', 'F'];

/// The guards that describe the id format and quote ids as examples.
const EXEMPT: &[&str] = &[
    "deskspace-app/tests/guards/task_ids.rs",
    "deskspace-app/tests/guards/comment_ids.rs",
];

#[test]
fn no_comment_carries_a_task_id() {
    let letters = task_letters();
    let crates = workspace_root().join("crates");
    let mut found = Vec::new();
    for path in rust_files(&crates) {
        let relative = relative(&crates, &path);
        if EXEMPT.contains(&relative.as_str()) {
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        for (index, line) in text.lines().enumerate() {
            let Some(comment) = comment_of(line) else {
                continue;
            };
            for id in task_ids(comment, &letters) {
                found.push(format!("  {relative}:{}: {id}", index + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "these comments carry a task id:\n{}\n\
         Say the why in the comment's own words and put the id in the commit \
         message (docs/ENGINEERING.md section 4, rule 10).",
        found.join("\n")
    );
}

#[test]
fn ids_and_comments_are_recognized() {
    let letters: BTreeSet<char> = ['B', 'R', 'W'].into();
    assert_eq!(
        task_ids("// lens settings (W19, W23).", &letters),
        ["W19", "W23"]
    );
    assert_eq!(task_ids("// split by R14a", &letters), ["R14a"]);
    for clean in [
        "// decided in D17 (F-45, M4 phase 2)",
        "// Shift+F10 opens the menu",
        "// DXGI_FORMAT_B8G8R8A8_UNORM",
        "// WM_APP + 12",
        "// WORD19, W1234, and W19ab",
    ] {
        assert!(task_ids(clean, &letters).is_empty(), "{clean:?}");
    }
    assert_eq!(comment_of("let x = 1; // the why"), Some("// the why"));
    assert_eq!(comment_of(r#"let url = "https://example.com/W19";"#), None);
    assert_eq!(comment_of("    /// docs"), Some("/// docs"));
}

/// The section letters the id sources define ids under, less the allowed.
fn task_letters() -> BTreeSet<char> {
    let root = workspace_root();
    let mut letters = BTreeSet::new();
    for source in ID_SOURCES {
        let text = fs::read_to_string(root.join(source))
            .unwrap_or_else(|error| panic!("cannot read {source}: {error}"));
        for line in text.lines() {
            let Some(item) = line.trim_start().strip_prefix("- ") else {
                continue;
            };
            let item = item
                .strip_prefix("[ ] ")
                .or_else(|| item.strip_prefix("[x] "))
                .unwrap_or(item);
            let mut chars = item.strip_prefix("**").unwrap_or("").chars();
            if let (Some(letter), Some(digit)) = (chars.next(), chars.next())
                && letter.is_ascii_uppercase()
                && digit.is_ascii_digit()
            {
                letters.insert(letter);
            }
        }
    }
    letters.retain(|letter| !ALLOWED_LETTERS.contains(letter));
    assert!(
        letters.contains(&'W'),
        "the id sources define no W ids; did a file move?"
    );
    letters
}

/// The comment part of a line: from the first `//` outside a string.
fn comment_of(line: &str) -> Option<&str> {
    let mut in_string = false;
    let mut previous = '\0';
    for (index, c) in line.char_indices() {
        match c {
            '"' if previous != '\\' => in_string = !in_string,
            '/' if !in_string && line[index + 1..].starts_with('/') => {
                return Some(&line[index..]);
            }
            _ => {}
        }
        previous = c;
    }
    None
}

/// The task ids in `comment`: a letter from `letters`, one to three digits,
/// and an optional lowercase letter, as a whole word.
fn task_ids(comment: &str, letters: &BTreeSet<char>) -> Vec<String> {
    comment
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|word| {
            let mut chars = word.chars();
            let Some(letter) = chars.next() else {
                return false;
            };
            let rest = chars.as_str();
            let digits = rest.trim_end_matches(|c: char| c.is_ascii_lowercase());
            letters.contains(&letter)
                && (1..=3).contains(&digits.len())
                && digits.chars().all(|c| c.is_ascii_digit())
                && rest.len() - digits.len() <= 1
        })
        .map(str::to_string)
        .collect()
}
