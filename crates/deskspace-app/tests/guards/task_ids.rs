//! Keeps task ids unique, so an id in a commit message names one task.
//!
//! A task is defined where a list item in `docs/TASKS.md` opens with its id
//! in bold: `- [ ] **K10. Split ...**`, `- **R14a (senior, design).**`, or a
//! range such as `**T35-T39.**`. A finished task is deleted from `TASKS.md`
//! and retired as one such line at the end of `docs/archive/TASKS-done.md`
//! (`TASKS.md` rule 7), so the two files together hold every id ever used.
//! `docs/ROADMAP.md` defines its decisions (`D12`) and the ideas waiting for
//! one (`Y3`) the same way, so they are checked with the tasks.

use std::collections::BTreeMap;
use std::fs;

use super::support::workspace_root;

/// Ids that two tasks shared before this test existed (2026-11-09): an old
/// section G and N were archived and their letters reused. They stay as
/// history; delete an entry if its ids ever stop colliding.
const REUSED_BEFORE_THE_TEST: &[&str] = &[
    "G1", "G2", "G3", "G4", "G6", "G7", "K9", "N1", "N2", "N3", "N4", "N5",
];

const SOURCES: &[&str] = &[
    "docs/TASKS.md",
    "docs/archive/TASKS-done.md",
    "docs/ROADMAP.md",
];

#[test]
fn every_task_id_is_defined_once() {
    let definitions = definitions();
    let reused: Vec<String> = definitions
        .iter()
        .filter(|(id, places)| places.len() > 1 && !REUSED_BEFORE_THE_TEST.contains(&id.as_str()))
        .map(|(id, places)| format!("  {id}: {}", places.join(", ")))
        .collect();
    assert!(
        reused.is_empty(),
        "these task ids name more than one task:\n{}\n\
         Give the newer task an id no line in docs/TASKS.md or \
         docs/archive/TASKS-done.md uses (TASKS.md rule 7).",
        reused.join("\n")
    );
}

#[test]
fn the_reused_list_holds_only_ids_that_still_collide() {
    let definitions = definitions();
    for id in REUSED_BEFORE_THE_TEST {
        let count = definitions.get(*id).map_or(0, Vec::len);
        assert!(
            count > 1,
            "{id} is defined {count} time(s); remove it from REUSED_BEFORE_THE_TEST"
        );
    }
}

#[test]
fn definitions_are_recognized() {
    let lines = [
        "- [ ] **K10. Split `win32/folder_watch.rs`.** 1,278 lines.",
        "  - [ ] **Z5a (intern). Lens files.**",
        "- [x] **R14b… (junior, one page per commit).**",
        "    - **R17a:** `app/effect.rs`",
        "- [ ] **T35-T39. Move one built-in effect to a recipe each.**",
        "- **P43.** Let clippy keep closures that only call a method out.",
    ];
    let found: Vec<Vec<String>> = lines.iter().map(|line| defined_ids(line)).collect();
    assert_eq!(
        found,
        [
            vec!["K10"],
            vec!["Z5a"],
            vec!["R14b"],
            vec!["R17a"],
            vec!["T35", "T36", "T37", "T38", "T39"],
            vec!["P43"],
        ]
    );
    for prose in [
        "- **Watch.** Windows' `ReadDirectoryChangesW` tells us",
        "| P50 (v0.3) (part script) | Ctrl+click two items |",
        "- [ ] **F1-F5** are references, not a task",
        "  F2 renames the selected item",
    ] {
        assert!(defined_ids(prose).is_empty(), "{prose:?} defines no id");
    }
}

/// Every defined id, with the `file:line` of each definition.
fn definitions() -> BTreeMap<String, Vec<String>> {
    let root = workspace_root();
    let mut definitions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for source in SOURCES {
        let text = fs::read_to_string(root.join(source))
            .unwrap_or_else(|error| panic!("cannot read {source}: {error}"));
        for (index, line) in text.lines().enumerate() {
            for id in defined_ids(line) {
                definitions
                    .entry(id)
                    .or_default()
                    .push(format!("{source}:{}", index + 1));
            }
        }
    }
    definitions
}

/// The ids a line defines: none unless it is a list item whose bold text
/// opens with an id followed by `.`, ` (`, `:`, `…`, or `-` and a second id.
fn defined_ids(line: &str) -> Vec<String> {
    let Some(item) = line.trim_start().strip_prefix("- ") else {
        return Vec::new();
    };
    let item = item
        .strip_prefix("[ ] ")
        .or_else(|| item.strip_prefix("[x] "))
        .unwrap_or(item);
    let Some(rest) = item.strip_prefix("**") else {
        return Vec::new();
    };
    let Some((first, rest)) = split_id(rest) else {
        return Vec::new();
    };
    if let Some(range) = rest.strip_prefix('-')
        && let Some((last, after)) = split_id(range)
        && after.starts_with('.')
    {
        return expand_range(&first, &last);
    }
    if [".", " (", ":", "…"]
        .iter()
        .any(|end| rest.starts_with(end))
    {
        vec![first]
    } else {
        Vec::new()
    }
}

/// Splits a leading id (`K10`, `Z5a`) off `text`.
fn split_id(text: &str) -> Option<(String, &str)> {
    let mut chars = text.char_indices();
    let (_, letter) = chars.next()?;
    if !letter.is_ascii_uppercase() {
        return None;
    }
    let digits_end = text[1..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(text.len(), |end| end + 1);
    if digits_end == 1 {
        return None;
    }
    let end = match text[digits_end..].chars().next() {
        Some(suffix) if suffix.is_ascii_lowercase() => digits_end + 1,
        _ => digits_end,
    };
    // A lowercase letter followed by more letters is a word, not a suffix.
    if end > digits_end && text[end..].starts_with(|c: char| c.is_ascii_alphabetic()) {
        return Some((text[..digits_end].to_string(), &text[digits_end..]));
    }
    Some((text[..end].to_string(), &text[end..]))
}

fn expand_range(first: &str, last: &str) -> Vec<String> {
    let letter = &first[..1];
    let number = |id: &str| id[1..].parse::<u32>().ok();
    match (number(first), number(last)) {
        (Some(from), Some(to)) if last.starts_with(letter) && from <= to => {
            (from..=to).map(|n| format!("{letter}{n}")).collect()
        }
        _ => vec![first.to_string()],
    }
}
