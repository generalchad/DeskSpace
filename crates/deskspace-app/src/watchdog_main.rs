//! The crash watchdog for the desktop takeover (ROADMAP D8).
//!
//! A separate executable so a bulk kill of `deskspace.exe` (by image name, a
//! closed console, or an ended process tree) does not take it with the main
//! process. The main app launches it detached as `deskspace-watchdog <pid>`;
//! it waits on that process and re-shows Explorer's icons unless the app
//! signaled a clean exit. See `deskspace_platform::arm_watchdog`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    match deskspace_platform::watchdog_pid(&args) {
        Some(pid) => {
            deskspace_platform::run_watchdog(pid);
            ExitCode::SUCCESS
        }
        None => ExitCode::FAILURE,
    }
}
