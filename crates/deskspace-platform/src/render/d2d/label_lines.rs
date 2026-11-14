//! How many lines an icon cell's label takes, so that its highlight can hug
//! the label as Explorer's does: shorter around a one-line name than around
//! one that wraps.

use std::cell::RefCell;
use std::collections::HashMap;

use super::item::grid_label_width;
use super::*;
use crate::render::ItemLayout;

/// The labels remembered before the memo starts over. A space shows a few
/// hundred icon cells at most, so this holds many spaces' worth.
const MEMO_LIMIT: usize = 8192;
/// The height labels are laid out in to count their lines: room for more
/// lines than any name wraps to, so none is trimmed away uncounted.
const MEASURE_HEIGHT: f32 = 4096.0;

/// The line counts already measured. A scene is pushed again for every hover
/// and selection change, and laying each label out again for those would cost
/// the UI thread a text layout per visible item.
#[derive(Default)]
pub(super) struct LabelLines {
    memo: RefCell<Memo>,
}

/// A label box: its width in pixels, and the label size percent.
type LabelBox = (u32, u8);

#[derive(Default)]
struct Memo {
    boxes: HashMap<LabelBox, HashMap<Vec<u16>, u8>>,
    count: usize,
}

impl Memo {
    fn get(&self, shape: LabelBox, label: &[u16]) -> Option<u8> {
        self.boxes.get(&shape)?.get(label).copied()
    }

    fn insert(&mut self, shape: LabelBox, label: &[u16], lines: u8) {
        if self.count >= MEMO_LIMIT {
            self.boxes.clear();
            self.count = 0;
        }
        let known = self.boxes.entry(shape).or_default();
        if known.insert(label.to_vec(), lines).is_none() {
            self.count += 1;
        }
    }
}

impl Graphics {
    /// How many lines `label` takes as `item`'s in `scene`: one or two in an
    /// icon grid, by how the label wraps and how many lines the look allows.
    /// Every other view draws a name on one line.
    pub(super) fn item_label_lines(
        &self,
        scene: &SpaceScene,
        item: &ItemVisual,
        label: &[u16],
    ) -> u8 {
        if scene.item_layout != ItemLayout::Grid {
            return 1;
        }
        let cell_width = item.rect.width as f32;
        self.grid_label_lines(label, cell_width, scene.label_size, scene.label_lines)
    }

    /// How many lines `label` takes in an icon cell `cell_width` pixels wide
    /// at `size` percent, when the look allows `allowed` lines.
    fn grid_label_lines(&self, label: &[u16], cell_width: f32, size: u8, allowed: u8) -> u8 {
        let allowed = allowed.clamp(1, 2);
        if allowed == 1 || label.is_empty() {
            return 1;
        }
        let width = grid_label_width(cell_width);
        let shape = (width.max(0.0).round() as u32, size);
        if let Some(lines) = self.label_lines.memo.borrow().get(shape, label) {
            return lines;
        }
        // A label that cannot be laid out keeps the tallest highlight, which
        // holds whatever is drawn.
        let lines = self
            .wrapped_lines(label, width, size)
            .map_or(allowed, |count| count.clamp(1, 2) as u8);
        self.label_lines
            .memo
            .borrow_mut()
            .insert(shape, label, lines);
        lines
    }

    /// The lines `label` wraps to in a box `width` pixels wide at `size`
    /// percent, or `None` when DirectWrite cannot lay it out.
    fn wrapped_lines(&self, label: &[u16], width: f32, size: u8) -> Option<u32> {
        let format = self.item_label_format_for(size, ItemLayout::Grid);
        let mut count = 0u32;
        // SAFETY: `label` and `format` are live for the calls. A null array
        // asks `GetLineMetrics` only for the line count, which it reports
        // along with the error for a buffer too small.
        // https://learn.microsoft.com/en-us/windows/win32/api/dwrite/nf-dwrite-idwritetextlayout-getlinemetrics
        unsafe {
            let layout = self
                .dwrite
                .CreateTextLayout(label, &format, width.max(1.0), MEASURE_HEIGHT)
                .ok()?;
            let _ = layout.GetLineMetrics(None, &mut count);
        }
        (count > 0).then_some(count)
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    use super::*;

    fn text(label: &str) -> Vec<u16> {
        label.encode_utf16().collect()
    }

    #[test]
    fn a_short_name_takes_one_line_and_a_long_one_wraps_to_two() {
        // SAFETY: initialises COM on the test thread; a repeat call is
        // harmless.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let graphics = graphics().expect("graphics");
        // Explorer's 75 px desktop step.
        let lines =
            |label: &str, allowed| graphics.grid_label_lines(&text(label), 75.0, 100, allowed);
        assert_eq!(lines("Word", 2), 1);
        assert_eq!(lines("Quarterly budget review", 2), 2);
        assert_eq!(
            lines("A name long enough to need three lines or more", 2),
            2
        );
        assert_eq!(
            lines("Quarterly budget review", 1),
            1,
            "the look allows one"
        );
        assert_eq!(lines("", 2), 1);
        // A wide cell holds on one line what a narrow one wraps.
        let wide = graphics.grid_label_lines(&text("Quarterly budget review"), 400.0, 100, 2);
        assert_eq!(wide, 1);
        assert_eq!(
            lines("Quarterly budget review", 2),
            2,
            "remembered per width"
        );
    }

    #[test]
    fn a_label_is_remembered_for_its_own_box_only() {
        let mut memo = Memo::default();
        memo.insert((66, 100), &text("Word"), 1);
        memo.insert((66, 100), &text("A long name that wraps"), 2);
        assert_eq!(memo.get((66, 100), &text("Word")), Some(1));
        assert_eq!(
            memo.get((66, 100), &text("A long name that wraps")),
            Some(2)
        );
        assert_eq!(memo.get((66, 100), &text("Excel")), None);
        // A wider box, or a larger label, wraps the same name differently.
        assert_eq!(memo.get((98, 100), &text("Word")), None);
        assert_eq!(memo.get((66, 150), &text("Word")), None);
    }

    #[test]
    fn the_memo_counts_each_label_once_and_starts_over_when_full() {
        let mut memo = Memo::default();
        memo.insert((66, 100), &text("Word"), 1);
        memo.insert((66, 100), &text("Word"), 1);
        assert_eq!(memo.count, 1);
        for index in 0..MEMO_LIMIT {
            memo.insert((66, 100), &text(&format!("item {index}")), 1);
        }
        assert!(memo.count <= MEMO_LIMIT, "{}", memo.count);
        assert_eq!(memo.get((66, 100), &text("Word")), None, "started over");
        let last = format!("item {}", MEMO_LIMIT - 1);
        assert_eq!(memo.get((66, 100), &text(&last)), Some(1));
    }
}
