//! The spaces "Create test spaces" makes (ROADMAP D14): ordinary spaces
//! holding shortcuts that cover a program, a program with arguments, a
//! folder, and a web address.

use deskspace_platform::{CustomShortcut, ShortcutTarget, resolve_path};

use crate::app::files::ShortcutSource;

pub(super) struct TestSpace {
    pub title: &'static str,
    pub shortcuts: &'static [TestShortcut],
}

pub(super) struct TestShortcut {
    pub name: &'static str,
    /// Read as the Custom shortcut dialog reads a typed target.
    pub target: &'static str,
    pub arguments: &'static str,
}

const fn shortcut(name: &'static str, target: &'static str) -> TestShortcut {
    TestShortcut {
        name,
        target,
        arguments: "",
    }
}

pub(super) const TEST_SPACES: &[TestSpace] = &[
    TestSpace {
        title: "Test: Apps",
        shortcuts: &[
            shortcut("Notepad", r"%WINDIR%\System32\notepad.exe"),
            shortcut("Calculator", r"%WINDIR%\System32\calc.exe"),
            shortcut("Character Map", r"%WINDIR%\System32\charmap.exe"),
        ],
    },
    TestSpace {
        title: "Test: Tools",
        shortcuts: &[
            TestShortcut {
                name: "Command Prompt",
                target: r"%WINDIR%\System32\cmd.exe",
                arguments: "/k echo DeskSpace test shortcut",
            },
            TestShortcut {
                name: "PowerShell",
                target: r"%WINDIR%\System32\WindowsPowerShell\v1.0\powershell.exe",
                arguments: r#"-NoExit -Command "Write-Output 'DeskSpace test shortcut'""#,
            },
            shortcut("Task Manager", r"%WINDIR%\System32\Taskmgr.exe"),
        ],
    },
    TestSpace {
        title: "Test: Places",
        shortcuts: &[
            shortcut("Documents", r"%USERPROFILE%\Documents"),
            shortcut("Downloads", r"%USERPROFILE%\Downloads"),
            shortcut("Windows", "%WINDIR%"),
            shortcut("Temp", "%TEMP%"),
        ],
    },
    TestSpace {
        title: "Test: Web",
        shortcuts: &[
            shortcut(
                "DeskSpace on GitHub",
                "https://github.com/generalchad/DeskSpace",
            ),
            shortcut("Example", "https://example.com"),
        ],
    },
];

/// The shortcut counts "Create a stress space" offers, smallest first.
pub(super) const STRESS_SIZES: [usize; 3] = [50, 200, 1000];

/// `count` shortcuts to Notepad, named `Item 0001` upward. Empty, logged,
/// when Notepad is not found.
pub(super) fn stress_shortcuts(count: usize) -> Vec<ShortcutSource> {
    let Some(ShortcutSource::Custom(notepad)) =
        shortcut("Notepad", r"%WINDIR%\System32\notepad.exe").source()
    else {
        return Vec::new();
    };
    (1..=count)
        .map(|index| {
            ShortcutSource::Custom(CustomShortcut {
                name: format!("Item {index:04}"),
                ..notepad.clone()
            })
        })
        .collect()
}

impl TestShortcut {
    /// The shortcut to write, with a path target resolved as the Custom
    /// shortcut dialog resolves one. `None`, logged, when the target does
    /// not exist on this machine.
    pub(super) fn source(&self) -> Option<ShortcutSource> {
        let target = match ShortcutTarget::parse(self.target)? {
            ShortcutTarget::Path(typed) => match resolve_path(&typed) {
                Some(path) => ShortcutTarget::Path(path),
                None => {
                    tracing::warn!(
                        target = self.target,
                        "debug: test shortcut target not found"
                    );
                    return None;
                }
            },
            url => url,
        };
        Some(ShortcutSource::Custom(CustomShortcut {
            target,
            arguments: self.arguments.to_string(),
            name: self.name.to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_parses_as_a_url_or_a_path() {
        for space in TEST_SPACES {
            for shortcut in space.shortcuts {
                let parsed = ShortcutTarget::parse(shortcut.target);
                let web = shortcut.target.starts_with("https://");
                match parsed {
                    Some(ShortcutTarget::Url(_)) => assert!(web, "{}", shortcut.target),
                    Some(ShortcutTarget::Path(_)) => assert!(!web, "{}", shortcut.target),
                    None => panic!("{} does not parse", shortcut.target),
                }
            }
        }
    }

    #[test]
    fn stress_sizes_are_ascending() {
        assert!(STRESS_SIZES.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn titles_are_unique() {
        let mut titles: Vec<_> = TEST_SPACES.iter().map(|space| space.title).collect();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), TEST_SPACES.len());
    }
}
