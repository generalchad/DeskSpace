//! The startup clock: one zero for the startup timeline and for the debug
//! log's line times, so the two can be compared. `start` is called at the top
//! of `main`; anything logged before then starts the clock itself.

use std::sync::OnceLock;
use std::time::Instant;

// Process state: the clock's zero. Every startup phase and every log line
// reads it, so it cannot be passed down, and it lives for the process.
static START: OnceLock<Instant> = OnceLock::new();

/// Starts the clock unless it is already running.
pub fn start() {
    START.get_or_init(Instant::now);
}

/// Milliseconds since `start`, or since the first call when nothing started
/// it.
pub fn millis() -> u128 {
    START.get_or_init(Instant::now).elapsed().as_millis()
}
