//! Fails on a `pub` item in a library crate that nothing uses.
//!
//! The compiler warns about an unused private item, but not about a `pub`
//! one in a library, since another crate might call it. This workspace
//! publishes nothing, so its own crates are every caller there is: a `pub`
//! function, type, constant, or static whose name appears in no Rust source
//! but its own declaration is dead code (`ENGINEERING.md` section 2, delete
//! dead code instead of keeping it "just in case").
//!
//! Names are matched as whole words in every `.rs` file under `crates/`,
//! comments and strings included, so a name that is only mentioned counts as
//! used. The test can miss a dead item; it never flags a used one.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::support::{relative, rust_files, workspace_root};

const LIBRARIES: &[&str] = &[
    "deskspace-core",
    "deskspace-config",
    "deskspace-platform",
    "deskspace-update",
];

#[test]
fn every_pub_item_in_a_library_has_a_user() {
    let crates = workspace_root().join("crates");
    let sources: Vec<(PathBuf, String)> = rust_files(&crates)
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            (path, text)
        })
        .collect();

    let mut uses: HashMap<&str, usize> = HashMap::new();
    for (_, text) in &sources {
        for word in words(text) {
            *uses.entry(word).or_default() += 1;
        }
    }

    let mut dead = Vec::new();
    for (path, text) in &sources {
        let relative = relative(&crates, path);
        if !LIBRARIES
            .iter()
            .any(|library| relative.starts_with(&format!("{library}/src/")))
        {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if let Some(name) = pub_item_name(line)
                && uses.get(name).copied().unwrap_or(0) <= 1
            {
                dead.push(format!("  {relative}:{}: {name}", index + 1));
            }
        }
    }
    assert!(
        dead.is_empty(),
        "these pub items have no user anywhere in the workspace:\n{}\n\
         Delete each one, or use it in the commit that needs it.",
        dead.join("\n")
    );
}

#[test]
fn declarations_are_recognized() {
    for (line, name) in [
        ("pub fn apply(&mut self) {", Some("apply")),
        ("    pub const fn new(x: i32) -> Self {", Some("new")),
        (
            "pub unsafe fn pick_folder(owner: HWND)",
            Some("pick_folder"),
        ),
        ("pub struct Space {", Some("Space")),
        ("pub enum Command {", Some("Command")),
        (
            "pub const FONT_SIZE_RANGE: (f32, f32) = (6.0, 48.0);",
            Some("FONT_SIZE_RANGE"),
        ),
        ("pub static NAMES: &[&str] = &[];", Some("NAMES")),
        ("pub trait EventHandler {", Some("EventHandler")),
        (
            "pub type Result<T> = std::result::Result<T, Error>;",
            Some("Result"),
        ),
        ("pub(crate) fn private_to_the_crate() {", None),
        ("pub mod render;", None),
        ("pub use geometry::Rect;", None),
        ("    pub width: i32,", None),
        ("// pub fn in_a_comment()", None),
    ] {
        assert_eq!(pub_item_name(line), name, "{line:?}");
    }
}

/// The name a line declares as a `pub` item, if it declares one.
fn pub_item_name(line: &str) -> Option<&str> {
    let mut rest = line.trim_start().strip_prefix("pub ")?;
    for qualifier in ["const fn ", "unsafe fn ", "async fn "] {
        if rest.starts_with(qualifier) {
            rest = &rest[qualifier.len() - "fn ".len()..];
        }
    }
    let rest = [
        "fn ", "struct ", "enum ", "const ", "static ", "trait ", "type ",
    ]
    .iter()
    .find_map(|kind| rest.strip_prefix(kind))?;
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
}
