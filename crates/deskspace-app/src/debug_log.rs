//! The debug window's log feed (ROADMAP D14): a `tracing` layer, installed
//! only with `--debug`, that records every event into a bounded ring. The
//! app drains the ring into the window's console on a timer. The layer has
//! its own level, changed live from the console, and leaves the stderr
//! layer and `RUST_LOG` alone.

use std::collections::VecDeque;
use std::fmt::{Debug, Write as _};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use deskspace_platform::{LogLevel, LogLine};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Registry;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::reload;

/// The most lines the ring keeps; the oldest is dropped past it.
pub const CAPACITY: usize = 10_000;

/// When the first line was recorded: the console's zero.
static START: OnceLock<Instant> = OnceLock::new();

/// A shared handle to the ring of recorded lines.
#[derive(Clone, Default)]
pub struct LogBuffer {
    ring: Arc<Mutex<Ring>>,
}

#[derive(Default)]
struct Ring {
    lines: VecDeque<LogLine>,
    /// The `seq` the last line was given.
    last_seq: u64,
}

impl LogBuffer {
    /// Records one line, stamping its `seq` and time, and drops the oldest
    /// past `CAPACITY`.
    pub fn push(&self, level: LogLevel, target: &str, message: String) {
        let millis = START.get_or_init(Instant::now).elapsed().as_millis();
        let mut ring = self.lock();
        ring.last_seq += 1;
        let line = LogLine {
            seq: ring.last_seq,
            millis: u64::try_from(millis).unwrap_or(u64::MAX),
            level,
            target: target.to_string(),
            message,
        };
        ring.lines.push_back(line);
        if ring.lines.len() > CAPACITY {
            ring.lines.pop_front();
        }
    }

    /// The lines recorded after the one numbered `seq` (0 for all).
    pub fn since(&self, seq: u64) -> Vec<LogLine> {
        let ring = self.lock();
        let first = ring.lines.partition_point(|line| line.seq <= seq);
        ring.lines.range(first..).cloned().collect()
    }

    /// A panic while the lock was held leaves the ring usable: at worst one
    /// line is missing.
    fn lock(&self) -> MutexGuard<'_, Ring> {
        self.ring
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The layer that records into a `LogBuffer`.
pub struct ConsoleLayer {
    buffer: LogBuffer,
}

impl ConsoleLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

impl<S: Subscriber> Layer<S> for ConsoleLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut visitor = LineVisitor::default();
        event.record(&mut visitor);
        let metadata = event.metadata();
        self.buffer.push(
            log_level(*metadata.level()),
            metadata.target(),
            visitor.finish(),
        );
    }
}

fn log_level(level: Level) -> LogLevel {
    match level {
        Level::ERROR => LogLevel::Error,
        Level::WARN => LogLevel::Warn,
        Level::INFO => LogLevel::Info,
        Level::DEBUG => LogLevel::Debug,
        Level::TRACE => LogLevel::Trace,
    }
}

/// Collects an event's fields as `message key=value key=value`.
#[derive(Default)]
struct LineVisitor {
    message: String,
    fields: String,
}

impl LineVisitor {
    fn finish(self) -> String {
        let mut line = self.message;
        line.push_str(&self.fields);
        line.trim_start().to_string()
    }
}

impl Visit for LineVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            let _ = write!(self.fields, " {}={value}", field.name());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            let _ = write!(self.fields, " {}={value:?}", field.name());
        }
    }
}

/// The console layer's level, which the console's Level box changes live.
pub struct ConsoleLevel {
    handle: reload::Handle<LevelFilter, Registry>,
}

impl ConsoleLevel {
    pub fn new(handle: reload::Handle<LevelFilter, Registry>) -> Self {
        Self { handle }
    }

    pub fn set(&self, level: LogLevel) {
        let filter = match level {
            LogLevel::Error => LevelFilter::ERROR,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Trace => LevelFilter::TRACE,
        };
        if let Err(error) = self.handle.reload(filter) {
            tracing::warn!(%error, "could not change the console's log level");
        }
    }
}

/// What `--debug` hands the app: the ring and the console layer's level.
pub struct DebugLog {
    pub buffer: LogBuffer,
    pub level: ConsoleLevel,
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    #[test]
    fn the_ring_drops_the_oldest_and_keeps_counting() {
        let buffer = LogBuffer::default();
        for index in 0..CAPACITY + 2 {
            buffer.push(LogLevel::Info, "t", index.to_string());
        }
        let lines = buffer.since(0);
        assert_eq!(lines.len(), CAPACITY);
        assert_eq!(lines[0].seq, 3);
        assert_eq!(lines[CAPACITY - 1].seq, CAPACITY as u64 + 2);
    }

    #[test]
    fn since_returns_only_newer_lines() {
        let buffer = LogBuffer::default();
        for message in ["one", "two", "three"] {
            buffer.push(LogLevel::Info, "t", message.into());
        }
        let newer: Vec<_> = buffer
            .since(1)
            .into_iter()
            .map(|line| line.message)
            .collect();
        assert_eq!(newer, vec!["two", "three"]);
        assert!(buffer.since(3).is_empty());
    }

    #[test]
    fn events_record_their_level_target_and_fields() {
        let buffer = LogBuffer::default();
        let subscriber = tracing_subscriber::registry().with(ConsoleLayer::new(buffer.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(count = 2, "hello");
        });
        let lines = buffer.since(0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].level, LogLevel::Info);
        assert_eq!(lines[0].target, module_path!());
        assert_eq!(lines[0].message, "hello count=2");
    }
}
