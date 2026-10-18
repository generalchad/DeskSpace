use std::path::PathBuf;

use anyhow::Context;
use deskspace_config::{Config, ConfigStore};
use deskspace_core::{Command, Desktop, ItemFacts, Rect};
use deskspace_platform::{
    Placement, RunOptions, enumerate_desktop_items, item_facts, system_uses_dark_mode, work_area_at,
};

use crate::app::{App, DebugTools, LayerState, tray_tooltip};
use crate::cli::Cli;
use crate::debug_log::DebugLog;

const INITIAL_SPACE: &str = "Desktop";
const INITIAL_RECT: Rect = Rect::new(60, 60, 440, 320);

/// One desktop item to seed: its path, what the file system says about it,
/// and the shell's display name for it.
struct SeedEntry {
    path: PathBuf,
    facts: ItemFacts,
    label: String,
}

pub fn seed_default_desktop(desktop: &mut Desktop, with_icons: bool) {
    let entries = if with_icons {
        desktop_entries()
    } else {
        Vec::new()
    };
    seed_space(desktop, entries, work_area_at(INITIAL_RECT.center()));
}

fn desktop_entries() -> Vec<SeedEntry> {
    match enumerate_desktop_items() {
        Ok(entries) => entries
            .into_iter()
            .map(|entry| SeedEntry {
                facts: item_facts(&entry.path),
                path: entry.path,
                label: entry.label,
            })
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "could not enumerate desktop items");
            Vec::new()
        }
    }
}

/// Creates the initial space holding `entries`, arranged, through `Command`s
/// alone. `AddItem` labels an item with its file stem, so a `RenameItem`
/// follows it to keep the shell's display name, as Explorer shows it.
fn seed_space(desktop: &mut Desktop, entries: Vec<SeedEntry>, bounds: Rect) {
    let id = desktop.next_space_id();
    let create = Command::CreateSpace {
        title: INITIAL_SPACE.to_owned(),
        at: INITIAL_RECT.origin(),
        size: INITIAL_RECT.size(),
        locked: false,
    };
    if !apply_seed(desktop, create, bounds) {
        return;
    }
    for entry in entries {
        let item = desktop.next_item_id();
        let add = Command::AddItem {
            space: id,
            path: entry.path.clone(),
            facts: entry.facts,
        };
        if apply_seed(desktop, add, bounds) {
            let label = Command::RenameItem {
                id: item,
                path: entry.path,
                label: entry.label,
            };
            apply_seed(desktop, label, bounds);
        }
    }
    apply_seed(desktop, Command::AutoArrange { id }, bounds);
}

fn apply_seed(desktop: &mut Desktop, command: Command, bounds: Rect) -> bool {
    let outcome = desktop.apply(command, bounds);
    if !outcome.is_applied() {
        tracing::warn!(
            ?outcome,
            "a command seeding the initial space did not apply"
        );
    }
    outcome.is_applied()
}

pub fn run(cli: Cli, debug_log: Option<DebugLog>) -> anyhow::Result<()> {
    deskspace_platform::enable_per_monitor_awareness();

    // `--show` from the desktop menu (or a shortcut): hand it to a running
    // instance and exit, rather than starting a second copy.
    if cli.show && deskspace_platform::signal_show_spaces() {
        return Ok(());
    }

    let store = match &cli.config {
        Some(path) => ConfigStore::with_path(PathBuf::from(path)),
        None => ConfigStore::discover()
            .context("could not find the per-user configuration directory")?,
    };

    let mut config = if cli.reset {
        if store.exists() {
            match store.backup("reset") {
                Ok(path) => {
                    tracing::info!(backup = %path.display(), "previous configuration backed up")
                }
                Err(error) => {
                    return Err(error).context(
                        "refusing to reset: could not back up the existing configuration",
                    );
                }
            }
        }
        Config::new()
    } else {
        store.load_or_default()
    };

    if config.desktop.spaces().is_empty() {
        seed_default_desktop(&mut config.desktop, !cli.no_icons);
    }
    // Per-space overrides from before customized looks become a look now.
    // The palette they were shown with depends on the Windows color mode.
    let theme = config.theme.clone();
    if config
        .desktop
        .merge_legacy_overrides(&theme, system_uses_dark_mode())
        && let Err(error) = store.save(&config)
    {
        tracing::warn!(%error, "could not save the converted space appearance");
    }

    // `--show` also forces them shown when it started a fresh instance, so a
    // stale desktop entry still works after an unclean exit.
    let hidden = !cli.show && (cli.hidden || config.window.start_hidden);
    let placement = cli.placement().unwrap_or(Placement::TopLevel);
    let options = RunOptions {
        placement,
        click_through: cli.click_through,
        on_top: cli.on_top,
        tray_tooltip: tray_tooltip(&config.desktop),
    };

    let mut app = App::new(config, store, hidden, cli.settings);
    app.layer = LayerState::new(cli.takeover_layer, cli.takeover);
    app.debug = debug_log.map(|log| DebugTools::new(log, cli.config.is_none()));
    tracing::info!(
        path = %app.store.path().display(),
        storage = %app.storage.root().display(),
        "starting DeskSpace"
    );
    let install_on_exit = app.install_on_exit();
    deskspace_platform::run(options, Box::new(app)).context("the DeskSpace UI runtime failed")?;
    // An update queued by the app runs only now, after the teardown has
    // restored the desktop; the installer then relaunches DeskSpace.
    install_on_exit.install();
    Ok(())
}

#[cfg(test)]
mod tests {
    use deskspace_core::ItemKind;

    use super::*;

    const BOUNDS: Rect = Rect::new(0, 0, 1920, 1040);

    fn entry(path: &str, label: &str, is_folder: bool) -> SeedEntry {
        SeedEntry {
            path: PathBuf::from(path),
            facts: ItemFacts {
                is_folder,
                ..ItemFacts::default()
            },
            label: label.to_owned(),
        }
    }

    #[test]
    fn a_fresh_desktop_gets_one_arranged_desktop_space() {
        let mut desktop = Desktop::new();
        let entries = vec![
            entry(r"C:\Users\me\Desktop\report.docx", "report.docx", false),
            entry(r"C:\Users\me\Desktop\Projects", "Projects", true),
            entry(r"C:\Users\me\Desktop\Notes.lnk", "Notes", false),
        ];

        seed_space(&mut desktop, entries, BOUNDS);

        let [space] = desktop.spaces() else {
            panic!("expected one space, got {}", desktop.spaces().len());
        };
        assert_eq!(space.title(), INITIAL_SPACE);
        assert_eq!(space.rect().origin(), INITIAL_RECT.origin());
        let seeded: Vec<(&str, ItemKind)> = space
            .items()
            .iter()
            .map(|item| (item.label.as_str(), item.kind))
            .collect();
        assert_eq!(
            seeded,
            [
                ("report.docx", ItemKind::File),
                ("Projects", ItemKind::Folder),
                ("Notes", ItemKind::Shortcut),
            ]
        );

        let arranged = desktop.clone();
        let id = space.id;
        assert!(
            desktop
                .apply(Command::AutoArrange { id }, BOUNDS)
                .is_applied()
        );
        assert_eq!(desktop, arranged, "the seeded space is already arranged");
    }

    #[test]
    fn seeding_without_icons_gives_an_empty_desktop_space() {
        let mut desktop = Desktop::new();

        seed_space(&mut desktop, Vec::new(), BOUNDS);

        let [space] = desktop.spaces() else {
            panic!("expected one space, got {}", desktop.spaces().len());
        };
        assert_eq!(space.title(), INITIAL_SPACE);
        assert!(space.items().is_empty());
    }
}
