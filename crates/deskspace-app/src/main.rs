// The released app is a GUI program, so no console window flashes when it is
// started from the installer, a shortcut, or a terminal. Debug builds keep the
// console so `-v` logging and Ctrl+C work while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod app;
mod bootstrap;
mod cli;
mod debug_log;
#[cfg(test)]
mod fixtures;
mod scene;
mod settings;
mod storage;

use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, reload};

use crate::cli::Cli;
use crate::debug_log::{ConsoleLayer, ConsoleLevel, DebugLog, LogBuffer};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let debug_log = init_tracing(cli.verbose, cli.debug);
    log_panics();

    match bootstrap::run(cli, debug_log) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "DeskSpace exited with an error");
            ExitCode::FAILURE
        }
    }
}

/// Logs to the console as before, under `RUST_LOG` or `-v`. With `debug`,
/// also records into the debug window's ring (ROADMAP D14) at a level of its
/// own, which the returned `DebugLog` can change.
fn init_tracing(verbosity: u8, debug: bool) -> Option<DebugLog> {
    let default_level = match verbosity {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    let output = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_filter(filter);
    let (console, debug_log) = if debug {
        let buffer = LogBuffer::default();
        let (level, handle) = reload::Layer::new(LevelFilter::DEBUG);
        let layer = ConsoleLayer::new(buffer.clone()).with_filter(level);
        let log = DebugLog {
            buffer,
            level: ConsoleLevel::new(handle),
        };
        (Some(layer), Some(log))
    } else {
        (None, None)
    };
    // The console layer goes first, straight on the registry, so its
    // reload handle names the plain `Registry` type.
    tracing_subscriber::registry()
        .with(console)
        .with(output)
        .init();
    debug_log
}

/// Logs every panic at error before the default report, in every build, so
/// the debug console and a saved log show it.
fn log_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(%info, "panic");
        previous(info);
    }));
}
