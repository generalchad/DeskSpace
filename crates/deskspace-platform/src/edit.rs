//! Inline text-editing state for renaming a space in its title bar.
//!
//! The app owns a `TextEdit` while a space is renamed and feeds it `EditKey`s
//! decoded from the keyboard hook (`win32::keyboard`). `caret` and `anchor`
//! are byte offsets into the text at character boundaries; the selection is
//! the range between them and is empty when they are equal.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDir {
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
}

/// One editing action decoded from a keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKey {
    Char(char),
    Backspace,
    Delete,
    BackspaceWord,
    DeleteWord,
    Move { dir: MoveDir, select: bool },
    SelectAll,
    Commit,
    Cancel,
}

/// What applying an `EditKey` did, so the caller knows whether to repaint or
/// to end the edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOutcome {
    /// The text or selection changed; repaint.
    Changed,
    /// Nothing changed.
    Ignored,
    /// Enter: apply the text.
    Commit,
    /// Escape: discard.
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    text: String,
    caret: usize,
    anchor: usize,
}

impl TextEdit {
    /// Starts editing `text` with all of it selected, so the first keystroke
    /// replaces the old name.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.len();
        Self {
            text,
            caret,
            anchor: 0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// Sets the selection to `start..end` (byte offsets), moving the caret to
    /// `end`. Used to preselect the base name, leaving the extension unselected
    /// as Explorer does when it shows extensions.
    pub fn select_range(&mut self, start: usize, end: usize) {
        self.anchor = start.min(self.text.len());
        self.caret = end.min(self.text.len());
    }

    /// The selected range as `(start, end)` byte offsets; empty when collapsed.
    pub fn selection(&self) -> (usize, usize) {
        if self.caret <= self.anchor {
            (self.caret, self.anchor)
        } else {
            (self.anchor, self.caret)
        }
    }

    pub fn apply(&mut self, key: EditKey) -> EditOutcome {
        match key {
            EditKey::Char(c) => {
                let mut buffer = [0u8; 4];
                self.replace_selection(c.encode_utf8(&mut buffer));
                EditOutcome::Changed
            }
            EditKey::Backspace => {
                if self.has_selection() {
                    self.delete_selection();
                } else if let Some(previous) = self.prev_boundary(self.caret) {
                    self.text.replace_range(previous..self.caret, "");
                    self.caret = previous;
                    self.anchor = previous;
                } else {
                    return EditOutcome::Ignored;
                }
                EditOutcome::Changed
            }
            EditKey::Delete => {
                if self.has_selection() {
                    self.delete_selection();
                } else if let Some(next) = self.next_boundary(self.caret) {
                    self.text.replace_range(self.caret..next, "");
                } else {
                    return EditOutcome::Ignored;
                }
                EditOutcome::Changed
            }
            EditKey::BackspaceWord => {
                if self.has_selection() {
                    self.delete_selection();
                } else {
                    let target = self.prev_word_boundary(self.caret);
                    if target == self.caret {
                        return EditOutcome::Ignored;
                    }
                    self.text.replace_range(target..self.caret, "");
                    self.caret = target;
                    self.anchor = target;
                }
                EditOutcome::Changed
            }
            EditKey::DeleteWord => {
                if self.has_selection() {
                    self.delete_selection();
                } else {
                    let target = self.next_word_boundary(self.caret);
                    if target == self.caret {
                        return EditOutcome::Ignored;
                    }
                    self.text.replace_range(self.caret..target, "");
                }
                EditOutcome::Changed
            }
            EditKey::Move { dir, select } => self.move_caret(dir, select),
            EditKey::SelectAll => {
                self.anchor = 0;
                self.caret = self.text.len();
                EditOutcome::Changed
            }
            EditKey::Commit => EditOutcome::Commit,
            EditKey::Cancel => EditOutcome::Cancel,
        }
    }

    fn move_caret(&mut self, dir: MoveDir, select: bool) -> EditOutcome {
        let (start, end) = self.selection();
        let had_selection = start != end;
        let target = match dir {
            MoveDir::Home => 0,
            MoveDir::End => self.text.len(),
            MoveDir::Left if select || !had_selection => {
                self.prev_boundary(self.caret).unwrap_or(0)
            }
            MoveDir::Left => start,
            MoveDir::Right if select || !had_selection => {
                self.next_boundary(self.caret).unwrap_or(self.text.len())
            }
            MoveDir::Right => end,
            MoveDir::WordLeft if select || !had_selection => self.prev_word_boundary(self.caret),
            MoveDir::WordLeft => start,
            MoveDir::WordRight if select || !had_selection => self.next_word_boundary(self.caret),
            MoveDir::WordRight => end,
        };
        if target == self.caret && !had_selection {
            return EditOutcome::Ignored;
        }
        self.caret = target;
        if !select {
            self.anchor = target;
        }
        EditOutcome::Changed
    }

    fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    fn delete_selection(&mut self) {
        let (start, end) = self.selection();
        self.text.replace_range(start..end, "");
        self.caret = start;
        self.anchor = start;
    }

    fn replace_selection(&mut self, insert: &str) {
        let (start, end) = self.selection();
        self.text.replace_range(start..end, insert);
        self.caret = start + insert.len();
        self.anchor = self.caret;
    }

    fn prev_boundary(&self, at: usize) -> Option<usize> {
        self.text[..at].char_indices().next_back().map(|(i, _)| i)
    }

    fn next_boundary(&self, at: usize) -> Option<usize> {
        let rest = self.text.get(at..)?;
        rest.chars().next().map(|c| at + c.len_utf8())
    }

    /// The start of the word before `at`, skipping spaces, as Ctrl+Left and
    /// Ctrl+Backspace do.
    fn prev_word_boundary(&self, at: usize) -> usize {
        let indices: Vec<(usize, char)> = self.text[..at].char_indices().collect();
        let mut i = indices.len();
        while i > 0 && indices[i - 1].1.is_whitespace() {
            i -= 1;
        }
        while i > 0 && !indices[i - 1].1.is_whitespace() {
            i -= 1;
        }
        if i == 0 { 0 } else { indices[i].0 }
    }

    /// The end of the word after `at`, skipping spaces, as Ctrl+Right and
    /// Ctrl+Delete do.
    fn next_word_boundary(&self, at: usize) -> usize {
        let indices: Vec<(usize, char)> = self.text[at..]
            .char_indices()
            .map(|(index, character)| (at + index, character))
            .collect();
        let mut i = 0;
        while i < indices.len() && indices[i].1.is_whitespace() {
            i += 1;
        }
        while i < indices.len() && !indices[i].1.is_whitespace() {
            i += 1;
        }
        if i >= indices.len() {
            self.text.len()
        } else {
            indices[i].0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_selects_the_whole_title() {
        let edit = TextEdit::new("Work");
        assert_eq!(edit.selection(), (0, 4));
        assert_eq!(edit.caret(), 4);
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut edit = TextEdit::new("Work");
        assert_eq!(edit.apply(EditKey::Char('H')), EditOutcome::Changed);
        assert_eq!(edit.text(), "H");
        assert_eq!(edit.selection(), (1, 1));
    }

    #[test]
    fn backspace_and_delete_use_the_caret_when_collapsed() {
        let mut edit = TextEdit::new("abc");
        // Collapse the selection to the end.
        assert_eq!(
            edit.apply(EditKey::Move {
                dir: MoveDir::End,
                select: false
            }),
            EditOutcome::Changed
        );
        edit.apply(EditKey::Backspace);
        assert_eq!(edit.text(), "ab");
        // Move home and delete forward.
        edit.apply(EditKey::Move {
            dir: MoveDir::Home,
            select: false,
        });
        edit.apply(EditKey::Delete);
        assert_eq!(edit.text(), "b");
    }

    #[test]
    fn backspace_at_the_start_is_ignored() {
        let mut edit = TextEdit::new("a");
        edit.apply(EditKey::Move {
            dir: MoveDir::Home,
            select: false,
        });
        assert_eq!(edit.apply(EditKey::Backspace), EditOutcome::Ignored);
    }

    #[test]
    fn shift_extends_the_selection() {
        let mut edit = TextEdit::new("abcd");
        edit.apply(EditKey::Move {
            dir: MoveDir::Home,
            select: false,
        });
        assert_eq!(
            edit.apply(EditKey::Move {
                dir: MoveDir::Right,
                select: true
            }),
            EditOutcome::Changed
        );
        assert_eq!(edit.selection(), (0, 1));
    }

    #[test]
    fn moving_with_a_selection_collapses_to_the_edge() {
        // Left collapses to the start, Right to the end.
        let mut edit = TextEdit::new("abcd");
        edit.apply(EditKey::Move {
            dir: MoveDir::Left,
            select: false,
        });
        assert_eq!(edit.caret(), 0);
        assert_eq!(edit.selection(), (0, 0));
    }

    #[test]
    fn select_all_then_delete_clears_the_text() {
        let mut edit = TextEdit::new("Work");
        edit.apply(EditKey::SelectAll);
        assert_eq!(edit.apply(EditKey::Delete), EditOutcome::Changed);
        assert_eq!(edit.text(), "");
    }

    #[test]
    fn ctrl_move_and_delete_stop_at_word_boundaries() {
        let mut edit = TextEdit::new("alpha beta gamma");
        let move_to = |edit: &mut TextEdit, dir| {
            edit.apply(EditKey::Move { dir, select: false });
        };
        move_to(&mut edit, MoveDir::Home);
        move_to(&mut edit, MoveDir::WordRight);
        assert_eq!(edit.caret(), 5);
        move_to(&mut edit, MoveDir::WordRight);
        assert_eq!(edit.caret(), 10);
        move_to(&mut edit, MoveDir::WordLeft);
        assert_eq!(edit.caret(), 6);
        // Ctrl+Backspace removes the previous word.
        move_to(&mut edit, MoveDir::End);
        edit.apply(EditKey::BackspaceWord);
        assert_eq!(edit.text(), "alpha beta ");
        // Ctrl+Delete removes the next word.
        move_to(&mut edit, MoveDir::Home);
        edit.apply(EditKey::DeleteWord);
        assert_eq!(edit.text(), " beta ");
    }

    #[test]
    fn commit_and_cancel_report_without_changing_text() {
        let mut edit = TextEdit::new("Work");
        assert_eq!(edit.apply(EditKey::Commit), EditOutcome::Commit);
        assert_eq!(edit.text(), "Work");
        assert_eq!(edit.apply(EditKey::Cancel), EditOutcome::Cancel);
    }
}
