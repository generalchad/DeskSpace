//! How wide an item's name is drawn, so a column can be sized to its labels
//! (ROADMAP D25): Details' "Size all columns to fit" and List's columns.

use windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS;

use super::*;
use crate::render::ItemLayout;

/// The height a label is laid out in to measure it: any line fits.
const MEASURE_HEIGHT: f32 = 4096.0;

impl Graphics {
    /// The width in pixels each of `labels` takes on one line, drawn as an
    /// item's name in `layout` at `size` percent: in the format the painter
    /// draws it with (`item_label_format_for`), rounded up to whole pixels.
    /// 0 for an empty label, or one DirectWrite cannot lay out.
    fn label_widths(&self, labels: &[&str], layout: ItemLayout, size: u8) -> Vec<i32> {
        let format = self.item_label_format_for(size, layout);
        labels
            .iter()
            .map(|label| self.one_line_width(label, &format).unwrap_or(0))
            .collect()
    }

    /// `label`'s width on one line in `format`, or `None` when DirectWrite
    /// cannot lay it out.
    fn one_line_width(&self, label: &str, format: &IDWriteTextFormat) -> Option<i32> {
        let text: Vec<u16> = label.encode_utf16().collect();
        // An unbounded box never wraps or trims, whatever `format` does in a
        // cell. Text is drawn at one pixel per DIP, so DIPs are pixels here.
        // SAFETY: `text` and `format` are live for the calls; the metrics
        // are a plain out-struct.
        // https://learn.microsoft.com/en-us/windows/win32/api/dwrite/nf-dwrite-idwritetextlayout-getmetrics
        let metrics = unsafe {
            let layout = self
                .dwrite
                .CreateTextLayout(&text, format, f32::MAX, MEASURE_HEIGHT)
                .ok()?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).ok()?;
            metrics
        };
        Some(metrics.width.max(0.0).ceil() as i32)
    }
}

/// `Graphics::label_widths` on this thread's factories; every width is 0
/// when they cannot be made.
pub(crate) fn measure_label_widths(labels: &[&str], layout: ItemLayout, size: u8) -> Vec<i32> {
    match graphics() {
        Ok(graphics) => graphics.label_widths(labels, layout, size),
        Err(error) => {
            tracing::warn!(%error, "could not measure labels");
            vec![0; labels.len()]
        }
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    use super::*;

    fn widths(labels: &[&str], layout: ItemLayout) -> Vec<i32> {
        // SAFETY: initialises COM on the test thread; a repeat call is
        // harmless.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        measure_label_widths(labels, layout, 100)
    }

    #[test]
    fn wide_letters_measure_wider_than_narrow_ones() {
        for layout in [ItemLayout::Details, ItemLayout::Inline] {
            let [wide, narrow] = widths(&["WWW", "iii"], layout)[..] else {
                panic!("one width per label");
            };
            assert!(narrow > 0, "{layout:?}");
            assert!(wide > narrow, "{layout:?}: {wide} <= {narrow}");
        }
    }

    #[test]
    fn a_long_name_measures_on_one_line_in_order_and_an_empty_one_is_zero() {
        let long = "A name long enough to wrap in any icon cell there is";
        let measured = widths(&["Word", "", long], ItemLayout::Grid);
        assert_eq!(measured.len(), 3);
        assert_eq!(measured[1], 0);
        assert!(
            measured[2] > 4 * measured[0],
            "laid out on one line, never wrapped: {measured:?}"
        );
    }
}
