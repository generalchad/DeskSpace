use std::path::PathBuf;

use anyhow::Context;
use deskspace_config::{Config, ConfigStore};
use deskspace_core::{Desktop, Item, Rect, layout::arrange_space};
use deskspace_platform::{
    Placement, RunOptions, enumerate_desktop_items, item_facts, system_uses_dark_mode,
};

use crate::app::{App, tray_tooltip};
use crate::cli::Cli;

const INITIAL_SPACE: &str = "Desktop";
const INITIAL_RECT: Rect = Rect::new(60, 60, 440, 320);

pub fn seed_default_desktop(desktop: &mut Desktop, with_icons: bool) {
    let id = desktop.add_space(INITIAL_SPACE, INITIAL_RECT);

    if with_icons {
        match enumerate_desktop_items() {
            Ok(entries) => {
                for entry in entries {
                    let item =
                        Item::new(desktop.next_item_id(), &entry.path, item_facts(&entry.path))
                            .with_label(entry.label)
                            .with_kind(entry.kind);
                    desktop.add_item(id, item);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "could not enumerate desktop items");
            }
        }
    }

    let layout = desktop.layout;
    if let Some(space) = desktop.space_mut(id) {
        arrange_space(space, layout);
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
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

    if config.desktop.spaces.is_empty() {
        seed_default_desktop(&mut config.desktop, !cli.no_icons);
    }
    // Per-space overrides from before customized looks become a look now.
    // The palette they were shown with depends on the Windows color mode.
    let theme = config.theme.clone();
    if config
        .desktop
        .fold_legacy_overrides(&theme, system_uses_dark_mode())
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
    app.takeover_layer = cli.takeover_layer;
    app.takeover = cli.takeover;
    tracing::info!(
        path = %app.store.path().display(),
        storage = %app.storage.root().display(),
        "starting DeskSpace"
    );
    deskspace_platform::run(options, Box::new(app)).context("the DeskSpace UI runtime failed")?;
    Ok(())
}
