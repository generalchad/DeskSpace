//! What the debug window's log console shows (ROADMAP D14): a bounded list
//! of log lines, a text filter, and a pause that holds new lines back. Pure,
//! so it can be tested; the window reads rows from it on demand.
//!
//! It never logs: a log line from here would feed the console itself.

use std::collections::VecDeque;

use crate::ui::LogLine;

/// The most lines the console keeps; the oldest is dropped past it.
pub(crate) const CONSOLE_CAPACITY: usize = 10_000;

#[derive(Debug, Default)]
pub(crate) struct ConsoleModel {
    lines: VecDeque<LogLine>,
    /// How many lines have been dropped from the front since the last clear,
    /// so `shown` can hold positions that stay valid as the front moves.
    dropped: usize,
    /// The positions (counting dropped lines) of the lines the filter shows,
    /// in order.
    shown: Vec<usize>,
    /// Lowercased; empty shows every line.
    filter: String,
    paused: bool,
    /// Lines pushed while paused, shown on resume.
    held: VecDeque<LogLine>,
}

impl ConsoleModel {
    /// Adds `lines`, or holds them while paused.
    pub(crate) fn push(&mut self, lines: &[LogLine]) {
        if self.paused {
            self.held.extend(lines.iter().cloned());
            let excess = self.held.len().saturating_sub(CONSOLE_CAPACITY);
            self.held.drain(..excess);
            return;
        }
        for line in lines {
            self.add(line.clone());
        }
    }

    /// Shows only lines whose target or message contains `filter`, ignoring
    /// case; an empty filter shows every line.
    pub(crate) fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_lowercase();
        self.shown = (0..self.lines.len())
            .filter(|&index| self.matches(&self.lines[index]))
            .map(|index| index + self.dropped)
            .collect();
    }

    /// Pauses or resumes. On resume, the lines held meanwhile are added.
    pub(crate) fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if !paused {
            for line in std::mem::take(&mut self.held) {
                self.add(line);
            }
        }
    }

    /// Forgets every line, held ones included. The filter and pause stay.
    pub(crate) fn clear(&mut self) {
        self.lines.clear();
        self.held.clear();
        self.shown.clear();
        self.dropped = 0;
    }

    /// How many lines the filter shows.
    pub(crate) fn shown_len(&self) -> usize {
        self.shown.len()
    }

    /// The `index`-th shown line. Panics past `shown_len`.
    pub(crate) fn shown(&self, index: usize) -> &LogLine {
        &self.lines[self.shown[index] - self.dropped]
    }

    /// Every shown line, in order.
    pub(crate) fn shown_lines(&self) -> impl Iterator<Item = &LogLine> {
        (0..self.shown_len()).map(|index| self.shown(index))
    }

    fn add(&mut self, line: LogLine) {
        if self.lines.len() == CONSOLE_CAPACITY {
            self.lines.pop_front();
            if self.shown.first() == Some(&self.dropped) {
                self.shown.remove(0);
            }
            self.dropped += 1;
        }
        if self.matches(&line) {
            self.shown.push(self.dropped + self.lines.len());
        }
        self.lines.push_back(line);
    }

    fn matches(&self, line: &LogLine) -> bool {
        self.filter.is_empty()
            || line.target.to_lowercase().contains(&self.filter)
            || line.message.to_lowercase().contains(&self.filter)
    }
}

/// `millis` as `mm:ss.mmm`; the minutes keep counting past 59.
pub(crate) fn format_time(millis: u64) -> String {
    let seconds = millis / 1000;
    format!(
        "{:02}:{:02}.{:03}",
        seconds / 60,
        seconds % 60,
        millis % 1000
    )
}

/// One line as text: `mm:ss.mmm LEVEL target message`.
pub fn format_line(line: &LogLine) -> String {
    format!(
        "{} {:<5} {} {}",
        format_time(line.millis),
        line.level.label().to_uppercase(),
        line.target,
        line.message
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::LogLevel;

    fn line(seq: u64, target: &str, message: &str) -> LogLine {
        LogLine {
            seq,
            millis: seq * 10,
            level: LogLevel::Info,
            target: target.into(),
            message: message.into(),
        }
    }

    fn shown_seqs(model: &ConsoleModel) -> Vec<u64> {
        model.shown_lines().map(|line| line.seq).collect()
    }

    #[test]
    fn pushed_lines_are_shown_in_order() {
        let mut model = ConsoleModel::default();
        model.push(&[line(1, "a", "one"), line(2, "b", "two")]);
        assert_eq!(shown_seqs(&model), vec![1, 2]);
        assert_eq!(model.shown(1).message, "two");
    }

    #[test]
    fn the_oldest_line_is_dropped_past_the_capacity() {
        let mut model = ConsoleModel::default();
        let lines: Vec<_> = (1..=CONSOLE_CAPACITY as u64 + 3)
            .map(|seq| line(seq, "t", "m"))
            .collect();
        model.push(&lines);
        assert_eq!(model.shown_len(), CONSOLE_CAPACITY);
        assert_eq!(model.shown(0).seq, 4);
        assert_eq!(
            model.shown(CONSOLE_CAPACITY - 1).seq,
            CONSOLE_CAPACITY as u64 + 3
        );
    }

    #[test]
    fn a_filtered_view_survives_drops() {
        let mut model = ConsoleModel::default();
        model.set_filter("keep");
        let lines: Vec<_> = (1..=CONSOLE_CAPACITY as u64 + 2)
            .map(|seq| line(seq, "t", if seq % 2 == 0 { "keep" } else { "skip" }))
            .collect();
        model.push(&lines);
        assert_eq!(model.shown_len(), CONSOLE_CAPACITY / 2);
        assert_eq!(model.shown(0).seq, 4, "line 2 was dropped");
    }

    #[test]
    fn the_filter_ignores_case_and_matches_target_or_message() {
        let mut model = ConsoleModel::default();
        model.push(&[
            line(1, "deskspace_app::view", "refresh"),
            line(2, "other", "Saved VIEW"),
            line(3, "other", "nothing"),
        ]);
        model.set_filter("View");
        assert_eq!(shown_seqs(&model), vec![1, 2]);
        model.set_filter("");
        assert_eq!(shown_seqs(&model), vec![1, 2, 3]);
    }

    #[test]
    fn paused_lines_wait_for_resume_and_meet_the_filter_then() {
        let mut model = ConsoleModel::default();
        model.push(&[line(1, "t", "before")]);
        model.set_paused(true);
        model.push(&[line(2, "t", "match"), line(3, "t", "other")]);
        assert_eq!(shown_seqs(&model), vec![1]);
        model.set_filter("match");
        model.set_paused(false);
        assert_eq!(shown_seqs(&model), vec![2]);
    }

    #[test]
    fn clear_forgets_shown_and_held_lines() {
        let mut model = ConsoleModel::default();
        model.push(&[line(1, "t", "m")]);
        model.set_paused(true);
        model.push(&[line(2, "t", "m")]);
        model.clear();
        model.set_paused(false);
        assert_eq!(model.shown_len(), 0);
        model.push(&[line(3, "t", "m")]);
        assert_eq!(shown_seqs(&model), vec![3]);
    }

    #[test]
    fn lines_format_as_time_level_target_message() {
        let mut warn = line(1, "deskspace_app", "could not save");
        warn.millis = 61_005;
        warn.level = LogLevel::Warn;
        assert_eq!(
            format_line(&warn),
            "01:01.005 WARN  deskspace_app could not save"
        );
        assert_eq!(format_time(3_600_000), "60:00.000");
    }
}
