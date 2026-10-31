//! Fails when a source file outgrows the size guideline in
//! `docs/ENGINEERING.md` section 2.
//!
//! A file is measured in non-test lines: everything above its test module,
//! the first top-level `#[cfg(test)] mod tests`. A file with no test module
//! is measured whole.
//!
//! A file that is already over the limit sits in `ALLOWED` with the reason
//! it is tolerated and the task that will split it. The entry is deleted
//! when the split lands; the test fails on a stale entry too, so the table
//! cannot outlive the debt.

use std::fs;
use std::path::{Path, PathBuf};

/// The guideline's trigger, in non-test lines (`ENGINEERING.md` section 2).
const LIMIT: usize = 800;

/// Marks an exception that has no split task yet. These are the backlog the
/// test surfaced; each needs a K task in `TASKS.md` or a decision to leave
/// it whole.
const NO_TASK: &str = "none yet";

struct Allowed {
    /// Path from `crates/`, with `/` separators.
    path: &'static str,
    /// What the file holds, and why it is still whole.
    why: &'static str,
    /// The K task that splits it, or `NO_TASK`.
    task: &'static str,
}

/// Files over the limit when this test was added (2026-10-29). Delete an
/// entry when its file drops below the limit, and name the split task when
/// one is written.
const ALLOWED: &[Allowed] = &[
    Allowed {
        path: "deskspace-platform/src/win32/space_window.rs",
        why: "The space window's procedure, painting, z-order, and mouse handling.",
        task: "K8",
    },
    Allowed {
        path: "deskspace-platform/src/win32/folder_watch.rs",
        why: "The watch worker, its port, and its listings share one thread.",
        task: NO_TASK,
    },
    Allowed {
        path: "deskspace-app/src/scene.rs",
        why: "One concern (model to SpaceScene) built from small helpers.",
        task: NO_TASK,
    },
    Allowed {
        path: "deskspace-app/src/app/menus.rs",
        why: "The space, item, and tray menus share their builders.",
        task: NO_TASK,
    },
    Allowed {
        path: "deskspace-app/src/app.rs",
        why: "App, on_event, MenuId, and the preset tables live here by design.",
        task: NO_TASK,
    },
    Allowed {
        path: "deskspace-app/src/app/files.rs",
        why: "Drops, delete, prune, and the layer sync share one file path.",
        task: NO_TASK,
    },
    Allowed {
        path: "deskspace-app/src/app/view.rs",
        why: "Look resolution, refresh, reflow, and the layer cache share state.",
        task: NO_TASK,
    },
    Allowed {
        path: "deskspace-platform/src/render/d2d/mod.rs",
        why: "One impl, the Painter's paint path.",
        task: NO_TASK,
    },
];

#[test]
fn every_source_file_stays_within_the_size_guideline() {
    let crates = workspace_root().join("crates");
    let over: Vec<(String, usize)> = rust_files(&crates)
        .into_iter()
        .map(|path| {
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            (relative(&crates, &path), non_test_lines(&source))
        })
        .filter(|(_, lines)| *lines > LIMIT)
        .collect();

    let unlisted: Vec<String> = over
        .iter()
        .filter(|(path, _)| !ALLOWED.iter().any(|allowed| allowed.path == path.as_str()))
        .map(|(path, lines)| format!("  {path}: {lines} lines"))
        .collect();
    assert!(
        unlisted.is_empty(),
        "these files are past the {LIMIT}-line guideline \
         (docs/ENGINEERING.md section 2):\n{}\n\
         Split each along its seams (docs/TASKS.md section K), or add it to \
         ALLOWED here with a reason and a task.",
        unlisted.join("\n")
    );

    for (index, allowed) in ALLOWED.iter().enumerate() {
        assert!(
            !allowed.why.trim().is_empty(),
            "{}: ALLOWED needs a reason",
            allowed.path
        );
        assert!(
            allowed.task == NO_TASK || is_task_id(allowed.task),
            "{}: task must be a K task id or {NO_TASK}",
            allowed.path
        );
        for other in &ALLOWED[index + 1..] {
            assert_ne!(
                allowed.path, other.path,
                "ALLOWED names {} twice",
                allowed.path
            );
        }

        let path = crates.join(allowed.path);
        assert!(
            path.is_file(),
            "{}: ALLOWED names a file that does not exist; remove the entry",
            allowed.path
        );
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let lines = non_test_lines(&source);
        assert!(
            lines > LIMIT,
            "{} is {lines} non-test lines, at or under the {LIMIT}-line \
             guideline; remove its ALLOWED entry",
            allowed.path
        );
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the app crate lives under crates/ in the workspace")
        .to_path_buf()
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| panic!("cannot read an entry: {error}"));
            let path = entry.path();
            if entry
                .file_type()
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
                .is_dir()
            {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("a file found under the walked root")
        .to_string_lossy()
        .replace('\\', "/")
}

/// Lines above the file's test module, or all of them when it has none.
fn non_test_lines(source: &str) -> usize {
    let lines: Vec<&str> = source.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if line.trim_end() != "#[cfg(test)]" {
            continue;
        }
        let next = lines[index + 1..]
            .iter()
            .find(|line| !line.trim().is_empty());
        if next.is_some_and(|line| is_tests_module(line)) {
            return index;
        }
    }
    lines.len()
}

/// `mod tests`, also `pub mod tests` / `pub(crate) mod tests`.
fn is_tests_module(line: &str) -> bool {
    let line = line.trim_start();
    let line = line
        .strip_prefix("pub(crate) ")
        .or_else(|| line.strip_prefix("pub "))
        .unwrap_or(line);
    line.starts_with("mod tests")
}

fn is_task_id(value: &str) -> bool {
    value
        .strip_prefix('K')
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
}
