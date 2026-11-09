use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context;
use deskspace_config::{Config, ConfigError, ConfigStore};
use deskspace_core::{Command, Desktop, ItemFacts, Rect};
use deskspace_platform::{
    Placement, RunOptions, claim_instance, enumerate_desktop_items, instance_key,
    system_uses_dark_mode, work_area_at,
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
                facts: entry.facts,
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
        icon: deskspace_core::SpaceIcon::None,
        view: deskspace_core::SpaceView::default(),
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

const LOAD_FAILED_TITLE: &str = "DeskSpace did not start";

/// What the user is told when the configuration exists but could not be read,
/// or could not be backed up before starting over.
fn load_failed_text(path: &Path, error: &ConfigError) -> String {
    format!(
        "DeskSpace could not read its settings file, so it did not start.\n\n\
         {}\n\n{error}\n\n\
         The file was left unchanged. If another program has it open, or it is \
         still syncing, close that program or wait, then start DeskSpace again.",
        path.display()
    )
}

/// How long a second launch waits for the running instance's controller
/// window before it gives up and exits.
const HAND_OFF_TIMEOUT: Duration = Duration::from_secs(5);
const HAND_OFF_RETRY: Duration = Duration::from_millis(100);

/// Asks the running instance for `key` to show its spaces, retrying while it
/// makes its controller window. Exits anyway after `HAND_OFF_TIMEOUT`, so a
/// launch that cannot reach it never starts a second copy.
fn hand_off(key: &str) {
    let deadline = Instant::now() + HAND_OFF_TIMEOUT;
    loop {
        if deskspace_platform::signal_show_spaces(key) {
            return;
        }
        if Instant::now() >= deadline {
            tracing::warn!(
                key,
                "another DeskSpace holds the settings file but did not answer"
            );
            return;
        }
        std::thread::sleep(HAND_OFF_RETRY);
    }
}

pub fn run(cli: Cli, debug_log: Option<DebugLog>) -> anyhow::Result<()> {
    deskspace_platform::enable_per_monitor_awareness();

    let store = match &cli.config {
        Some(path) => ConfigStore::with_path(PathBuf::from(path)),
        None => ConfigStore::discover()
            .context("could not find the per-user configuration directory")?,
    };
    // One instance per settings file: the key names its mutex and titles its
    // controller window.
    let key = instance_key(store.path());

    // `--show` from the desktop menu (or a shortcut): hand it to the running
    // instance for this settings file and exit, rather than starting a second
    // copy.
    if cli.show && deskspace_platform::signal_show_spaces(&key) {
        return Ok(());
    }
    // The other desktop verbs hand off the same way; with no instance running
    // this process starts and opens the space or the window itself.
    if cli.new_space && deskspace_platform::signal_new_space(&key) {
        return Ok(());
    }
    if cli.settings && deskspace_platform::signal_open_settings(&key) {
        return Ok(());
    }

    // A plain launch with the same settings file behaves as `--show`: the
    // running instance shows its spaces and this process exits. The first
    // instance may not have made its controller yet, so keep trying.
    let Some(guard) = claim_instance(store.path()) else {
        hand_off(&key);
        return Ok(());
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
        match store.load_or_default() {
            Ok(config) => config,
            Err(error) => {
                // A release build has no console, so the box is the only way
                // the user learns why nothing started.
                deskspace_platform::alert(
                    LOAD_FAILED_TITLE,
                    &load_failed_text(store.path(), &error),
                );
                return Err(error).context(
                    "refusing to start: the configuration could not be read or backed up",
                );
            }
        }
    };

    tracing::debug!(
        elapsed_ms = deskspace_platform::startup_millis(),
        spaces = config.desktop.spaces().len(),
        items = config
            .desktop
            .spaces()
            .iter()
            .map(|space| space.items().len())
            .sum::<usize>(),
        "startup: config loaded"
    );

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
        instance_key: key,
    };

    let mut app = App::new(config, store, hidden, cli.settings);
    app.layer = LayerState::new(cli.takeover_layer, cli.takeover);
    app.shell_entries = cli.config.is_none();
    app.new_space_on_start = cli.new_space;
    app.debug = debug_log.map(|log| DebugTools::new(log, cli.config.is_none()));
    tracing::info!(
        path = %app.store.path().display(),
        storage = %app.storage.root().display(),
        "starting DeskSpace"
    );
    let install_on_exit = app.install_on_exit();
    let result =
        deskspace_platform::run(options, Box::new(app)).context("the DeskSpace UI runtime failed");
    // Let go of the settings file before the installer relaunches DeskSpace,
    // so the new process does not find the old claim and exit.
    drop(guard);
    // An update queued by the app runs only now, after the teardown has
    // restored the desktop; the installer then relaunches DeskSpace.
    install_on_exit.install();
    result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use deskspace_core::ItemKind;

    use super::*;

    const BOUNDS: Rect = Rect::new(0, 0, 1920, 1040);

    #[test]
    fn a_failed_load_names_the_file_and_says_it_was_left_alone() {
        let path = Path::new(r"C:\Users\me\AppData\Roaming\DeskSpace\deskspace.toml");
        let error = ConfigError::io(
            path,
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );

        let text = load_failed_text(path, &error);

        assert!(text.contains(&path.display().to_string()), "{text}");
        assert!(text.contains(&error.to_string()), "{text}");
        assert!(text.contains("left unchanged"), "{text}");
    }

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
        assert!(space.icon().is_none(), "the first launch never asks");
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
