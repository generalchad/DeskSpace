mod app;
mod bootstrap;
mod cli;
mod scene;
mod settings;
mod storage;

use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::Cli;

fn main() -> ExitCode {
    // The takeover watchdog is the same executable, launched as
    // `deskspace --watchdog <pid>`. Handle it before `Cli::parse`, which would
    // reject the flag. See `deskspace_platform::arm_watchdog`.
    let args: Vec<String> = std::env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    if let Some(pid) = deskspace_platform::watchdog_pid(&args) {
        deskspace_platform::run_watchdog(pid);
        return ExitCode::SUCCESS;
    }

    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match app::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "DeskSpace exited with an error");
            ExitCode::FAILURE
        }
    }
}

fn init_tracing(verbosity: u8) {
    let default_level = match verbosity {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}
