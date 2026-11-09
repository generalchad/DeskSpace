//! Fails when a doc that describes the code names something the code does
//! not have: a backticked path that resolves to no file, a backticked Rust
//! name no source contains, or a relative Markdown link to a missing file or
//! heading (`ENGINEERING.md` section 7: when code and docs disagree, the docs
//! are wrong).
//!
//! The checked docs describe what exists. Task bodies and `ROADMAP.md` are
//! plans that name files and types still to be written, so only the
//! `TASKS.md` text above its first `---` (the rules, the file map, and the
//! recipes) is checked.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The docs checked whole.
const DOCS: &[&str] = &[
    "README.md",
    "CONTRIBUTING.md",
    "AGENTS.md",
    "RELEASING.md",
    "SECURITY.md",
    "docs/ENGINEERING.md",
    "docs/CONTEXT.md",
    "docs/TOOLS.md",
];

/// Checked up to its first `---` line.
const TASKS: &str = "docs/TASKS.md";

/// Backticked names that are not in this repository, and why a doc names them.
const EXTERNAL: &[(&str, &str)] = &[
    (
        "background_alpha",
        "a removed config key that loading ignores",
    ),
    (
        "icons.json",
        "the upstream Iconify file the icon-set trimmer reads",
    ),
    ("latest.json", "the update feed each release publishes"),
    (
        "IExplorerCommand",
        "the shell interface a Windows 11 menu entry needs",
    ),
    (
        "GetOverlappedResult",
        "the Win32 call whose wait hung the folder watch",
    ),
    (
        "NtUserGetMessage",
        "the frame an idle UI thread's stack shows",
    ),
    (
        "FreeConsole",
        "a Win32 call the Ctrl+C recipe makes from PowerShell",
    ),
    ("MN_GETHMENU", "a message the no-screen menu recipe sends"),
    ("MN_SELECTITEM", "a message the no-screen menu recipe sends"),
    (
        "GetMenuString",
        "a Win32 call the no-screen menu recipe makes",
    ),
    ("locking_mode", "an input of the rust MCP server's tools"),
];

const THIS_FILE: &str = "crates/deskspace-app/tests/doc_refs.rs";

/// Directories never walked: build output, git, and agent worktrees.
const SKIPPED_DIRS: &[&str] = &["target", ".git", "worktrees", "node_modules"];

const CODE_EXTENSIONS: &[&str] = &[
    "rs", "ps1", "psm1", "psd1", "toml", "yml", "yaml", "nsi", "json",
];
const PATH_EXTENSIONS: &[&str] = &[
    "rs", "md", "toml", "ps1", "psm1", "psd1", "yml", "yaml", "json", "nsi", "txt", "lock", "ico",
];

#[test]
fn every_doc_reference_resolves() {
    let root = workspace_root();
    let repo = Repo::scan(&root);
    let mut dangling = Vec::new();
    for (doc, text) in checked_docs(&root) {
        let mut in_fence = false;
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            for token in backticked(line) {
                if let Some(problem) = repo.check_token(token) {
                    dangling.push(format!("  {doc}:{}: `{token}` {problem}", index + 1));
                }
            }
            for target in links(line) {
                if let Some(problem) = check_link(&root, doc, target) {
                    dangling.push(format!("  {doc}:{}: ({target}) {problem}", index + 1));
                }
            }
        }
    }
    assert!(
        dangling.is_empty(),
        "these doc references name nothing in the repository:\n{}\n\
         Fix the doc in the same commit as the code it describes, or, for a \
         name from outside the repository, add it to EXTERNAL with why.",
        dangling.join("\n")
    );
}

#[test]
fn every_external_name_is_still_external_and_still_named() {
    let root = workspace_root();
    let repo = Repo::scan(&root);
    let docs: String = checked_docs(&root)
        .into_iter()
        .map(|(_, text)| text)
        .collect();
    for (name, why) in EXTERNAL {
        assert!(!why.is_empty(), "{name}: EXTERNAL needs a reason");
        assert!(
            !repo.words.contains(*name),
            "{name} is in the code now; remove it from EXTERNAL"
        );
        assert!(
            docs.contains(&format!("`{name}`")),
            "no checked doc names {name}; remove it from EXTERNAL"
        );
    }
}

#[test]
fn tokens_are_classified() {
    let repo = Repo {
        files: vec![
            "crates/deskspace-app/src/app/view.rs".into(),
            "docs/TASKS.md".into(),
        ],
        dirs: HashSet::from(["crates".into(), "crates/deskspace-app".into()]),
        words: HashSet::from(["Command".into(), "apply".into(), "theme_for".into()]),
    };
    for ok in [
        "app/view.rs",
        "deskspace-app/src/app/{view,view}.rs",
        "docs/TASKS.md",
        "crates/",
        "Command::apply",
        "theme_for(id)",
        "fn theme_for",
        "cargo test --workspace",
        "%TEMP%\\deskspace-dev",
        "<prefix>:<name>",
        "deskspace.exe",
        "Ctrl+Shift+1",
        "plain",
        "-Fresh",
    ] {
        assert_eq!(repo.check_token(ok), None, "{ok:?}");
    }
    for bad in [
        "app/gone.rs",
        "settings.rs",
        "Command::vanish",
        "gone_function",
    ] {
        assert!(repo.check_token(bad).is_some(), "{bad:?}");
    }
}

struct Repo {
    /// Every file, relative to the root with `/` separators.
    files: Vec<String>,
    dirs: HashSet<String>,
    /// Every identifier-like word in the code and scripts.
    words: HashSet<String>,
}

impl Repo {
    fn scan(root: &Path) -> Self {
        let mut repo = Repo {
            files: Vec::new(),
            dirs: HashSet::new(),
            words: HashSet::new(),
        };
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            let entries = fs::read_dir(&dir)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()));
            for entry in entries {
                let entry = entry.unwrap_or_else(|error| panic!("cannot read an entry: {error}"));
                let path = entry.path();
                let relative = relative(root, &path);
                if path.is_dir() {
                    let name = entry.file_name();
                    if !SKIPPED_DIRS.iter().any(|skipped| name == *skipped) {
                        repo.dirs.insert(relative);
                        pending.push(path);
                    }
                    continue;
                }
                let is_code = path
                    .extension()
                    .is_some_and(|extension| CODE_EXTENSIONS.iter().any(|code| extension == *code));
                // This file names the external names; they must not count as code.
                if is_code && relative != THIS_FILE {
                    let text = fs::read_to_string(&path).unwrap_or_default();
                    repo.words.extend(words(&text).map(str::to_string));
                }
                repo.files.push(relative);
            }
        }
        repo
    }

    /// Why `token` names nothing here, or `None` when it resolves or is not
    /// a reference this test checks.
    fn check_token(&self, token: &str) -> Option<&'static str> {
        match classify(token)? {
            Reference::Path(path) => (!expand_braces(path)
                .iter()
                .all(|path| self.path_exists(path)))
            .then_some("is no file or folder"),
            Reference::Name(name) => {
                let known = EXTERNAL.iter().any(|(external, _)| *external == name)
                    || name.split("::").all(|part| self.words.contains(part));
                (!known).then_some("is in no source")
            }
        }
    }

    fn path_exists(&self, path: &str) -> bool {
        let path = path.trim_start_matches("./").trim_end_matches('/');
        let path = path
            .strip_prefix("archive/")
            .map_or(path.to_string(), |rest| format!("docs/archive/{rest}"));
        let suffix = format!("/{path}");
        ["", "crates/", "docs/"].iter().any(|prefix| {
            let full = format!("{prefix}{path}");
            self.dirs.contains(&full) || self.files.contains(&full)
        }) || self.files.iter().any(|file| file.ends_with(&suffix))
            || self.dirs.iter().any(|dir| dir.ends_with(&suffix))
    }
}

enum Reference<'a> {
    Path(&'a str),
    Name(&'a str),
}

/// What a backticked token refers to, if this test checks it: a path, or a
/// Rust name that reads as one (`snake_case`, `CamelCase`, `SCREAMING`, or
/// with a `::`).
fn classify(token: &str) -> Option<Reference<'_>> {
    let token = token.trim();
    if token.is_empty()
        || token.contains(['<', '>', '*', '%', '$', '@', '\\'])
        || token.starts_with("http")
        || token.starts_with('-')
        || token.starts_with(['.', '/'])
        || token.starts_with("target")
    {
        return None;
    }
    if EXTERNAL.iter().any(|(external, _)| *external == token) {
        return None;
    }
    let first = token.split_whitespace().next()?;
    if is_path_like(first) {
        let artifact = [".exe", ".zip", ".sha256", ".sig"]
            .iter()
            .any(|end| first.ends_with(end));
        return (!artifact).then_some(Reference::Path(first));
    }
    if token.contains(' ')
        && ![
            "fn ", "struct ", "enum ", "const ", "trait ", "type ", "mod ",
        ]
        .iter()
        .any(|keyword| token.starts_with(keyword))
    {
        return None;
    }
    let name = token.rsplit(' ').next()?;
    let name = name.split('(').next()?;
    let is_name = !name.is_empty()
        && name.split("::").all(|part| {
            part.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
    let reads_as_code = name.contains('_')
        || name.contains("::")
        || name.chars().skip(1).any(|c| c.is_ascii_uppercase());
    (is_name && reads_as_code).then_some(Reference::Name(name))
}

fn is_path_like(token: &str) -> bool {
    token.contains('/')
        || Path::new(token)
            .extension()
            .is_some_and(|extension| PATH_EXTENSIONS.iter().any(|known| extension == *known))
}

/// `a/{b,c}.rs` as `a/b.rs` and `a/c.rs`.
fn expand_braces(token: &str) -> Vec<String> {
    let (Some(open), Some(close)) = (token.find('{'), token.find('}')) else {
        return vec![token.to_string()];
    };
    token[open + 1..close]
        .split(',')
        .flat_map(|choice| {
            expand_braces(&format!(
                "{}{choice}{}",
                &token[..open],
                &token[close + 1..]
            ))
        })
        .collect()
}

fn checked_docs(root: &Path) -> Vec<(&'static str, String)> {
    let read = |doc: &str| {
        fs::read_to_string(root.join(doc))
            .unwrap_or_else(|error| panic!("cannot read {doc}: {error}"))
    };
    let mut docs: Vec<(&'static str, String)> = DOCS.iter().map(|doc| (*doc, read(doc))).collect();
    let tasks = read(TASKS);
    let preamble = tasks
        .lines()
        .take_while(|line| line.trim() != "---")
        .collect::<Vec<_>>()
        .join("\n");
    docs.push((TASKS, preamble));
    docs
}

/// The single-backticked spans of a line.
fn backticked(line: &str) -> Vec<&str> {
    line.split('`')
        .enumerate()
        .filter(|(index, part)| index % 2 == 1 && !part.is_empty())
        .map(|(_, part)| part)
        .collect()
}

/// The targets of the line's Markdown links that stay in the repository.
fn links(line: &str) -> Vec<&str> {
    line.match_indices("](")
        .filter_map(|(start, _)| {
            let rest = &line[start + 2..];
            let target = &rest[..rest.find(')')?];
            let external = target.starts_with("http") || target.starts_with("mailto:");
            (!external && !target.contains(' ')).then_some(target)
        })
        .collect()
}

fn check_link(root: &Path, doc: &str, target: &str) -> Option<&'static str> {
    let (file, anchor) = target.split_once('#').unwrap_or((target, ""));
    let base = Path::new(doc).parent().unwrap_or(Path::new(""));
    let path = if file.is_empty() {
        root.join(doc)
    } else {
        root.join(base).join(file)
    };
    if !path.exists() {
        return Some("links to a missing file");
    }
    if anchor.is_empty() || path.extension().is_none_or(|extension| extension != "md") {
        return None;
    }
    let text = fs::read_to_string(&path).unwrap_or_default();
    let found = text
        .lines()
        .filter_map(|line| {
            line.trim_start_matches('#')
                .strip_prefix(' ')
                .filter(|_| line.starts_with('#'))
        })
        .any(|heading| slug(heading) == anchor);
    (!found).then_some("links to a missing heading")
}

/// A heading's GitHub anchor: lowercase, spaces to `-`, punctuation dropped.
fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("a path found under the root")
        .to_string_lossy()
        .replace('\\', "/")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the app crate lives under crates/ in the workspace")
        .to_path_buf()
}
