//! Draws Explorer's inline rename field: an opaque box with the selected
//! text highlighted and a blinking caret.

use super::geometry::{color_f, draw_text, rect_f};
use super::*;

/// Draws the title rename buffer in the title bar.
pub(super) unsafe fn draw_edit_title(
    ctx: DrawContext,
    prepared: &PreparedScene,
    left: f32,
    right: f32,
    title_height: f32,
) {
    let scene = &prepared.scene;
    let Some(edit) = &scene.editing else {
        return;
    };
    let Some(buffer) = prepared.edit.as_ref() else {
        return;
    };
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        draw_edit_buffer(
            ctx,
            edit,
            buffer,
            &ctx.graphics.edit_format,
            EditArea {
                rect: rect_f(left, 0.0, right, title_height),
                extent: EditWidth::Text,
            },
            scene.edit,
            scene.caret_visible,
        );
    }
}

/// Padding between the text and its rename box, matching Explorer's small,
/// even margin.
const EDIT_PAD_X: f32 = 3.0;
const EDIT_PAD_Y: f32 = 2.0;

/// How a rename box takes its width: from the text it measures, or from the
/// label it covers. Explorer spans an icon cell's whole label, so a short name
/// still fills the box; a row's name and a title bar's title get a box that
/// hugs their text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditWidth {
    Text,
    Label,
}

/// The area an editable buffer is laid out in: the box its height may grow
/// to, and how its width is taken.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct EditArea {
    pub(super) rect: D2D_RECT_F,
    pub(super) extent: EditWidth,
}

/// How tall the rename box's layout is allowed to grow. A name that wraps to
/// more lines than this is clipped by the space's own content clip; the value
/// only needs to exceed any plausible number of wrapped label lines.
pub(super) const EDIT_MAX_HEIGHT: f32 = 1024.0;

/// One content line of a laid-out editable buffer, in layout coordinates.
#[derive(Clone, Copy)]
struct EditLine {
    /// Text positions of the line's content, excluding trailing whitespace and
    /// the newline, in UTF-16 code units.
    start: u32,
    end: u32,
    top: f32,
    bottom: f32,
    /// Horizontal extent of the content.
    left: f32,
    right: f32,
}

/// Measures the content lines of `layout`. `DWRITE_TEXT_METRICS` returns the
/// layout box for center-aligned text, so the lines are hit-tested to hug the
/// text with `edit_field`.
unsafe fn edit_lines(layout: &IDWriteTextLayout) -> Vec<EditLine> {
    let mut count = 0u32;
    // SAFETY: caller is inside BeginDraw/EndDraw; a null array asks only for
    // the line count.
    unsafe {
        let _ = layout.GetLineMetrics(None, &mut count);
    }
    if count == 0 {
        return Vec::new();
    }
    let mut metrics = vec![DWRITE_LINE_METRICS::default(); count as usize];
    // SAFETY: the slice is sized to the count just reported.
    unsafe {
        let _ = layout.GetLineMetrics(Some(&mut metrics), &mut count);
    }
    let mut lines = Vec::with_capacity(count as usize);
    let mut position = 0u32;
    for line in &metrics[..count as usize] {
        let content = line
            .length
            .saturating_sub(line.trailingWhitespaceLength)
            .saturating_sub(line.newlineLength);
        let (start, end) = (position, position + content);
        let mut left = 0.0f32;
        let mut right = 0.0f32;
        let mut top = 0.0f32;
        let mut other = 0.0f32;
        let mut hit = DWRITE_HIT_TEST_METRICS::default();
        // SAFETY: both positions lie within the layout's text. The hit-test
        // y is the line top, which includes any vertical centering.
        unsafe {
            let _ = layout.HitTestTextPosition(start, false, &mut left, &mut top, &mut hit);
            let _ = layout.HitTestTextPosition(end, false, &mut right, &mut other, &mut hit);
        }
        lines.push(EditLine {
            start,
            end,
            top,
            bottom: top + line.height,
            left,
            right,
        });
        position += line.length;
    }
    lines
}

/// Draws an editable buffer the way Explorer draws an inline rename field: an
/// opaque box with an even margin, the selected text in the system highlight,
/// and a blinking caret. The measured lines place the box's height, selection,
/// and caret, so all three follow DirectWrite's own shaping; `area` says
/// whether the box hugs the text or spans the label it is drawn in, as
/// Explorer's does in an icon cell.
pub(super) unsafe fn draw_edit_buffer(
    ctx: DrawContext,
    edit: &TextEdit,
    buffer: &[u16],
    format: &IDWriteTextFormat,
    area: EditArea,
    colors: EditColors,
    caret_visible: bool,
) {
    let DrawContext {
        graphics,
        target,
        brush,
    } = ctx;
    let rect = area.rect;
    let width = (rect.right - rect.left).max(1.0);
    let height = (rect.bottom - rect.top).max(1.0);
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        let Ok(layout) = graphics
            .dwrite
            .CreateTextLayout(buffer, format, width, height)
        else {
            draw_text(target, brush, buffer, format, rect, colors.text, None);
            return;
        };
        let lines = edit_lines(&layout);
        let field = edit_field(rect, &lines, area.extent);

        brush.SetColor(&color_f(colors.fill));
        target.FillRectangle(&field, brush);
        brush.SetColor(&color_f(colors.border));
        target.DrawRectangle(&field, brush, 1.0, None);
        target.PushAxisAlignedClip(&field, D2D1_ANTIALIAS_MODE_ALIASED);

        let (start, end) = edit.selection();
        let start16 = utf16_index(edit.text(), start);
        let end16 = utf16_index(edit.text(), end);
        let mut hit = DWRITE_HIT_TEST_METRICS::default();
        if start != end {
            brush.SetColor(&color_f(colors.selection));
            for line in &lines {
                let from = start16.max(line.start);
                let to = end16.min(line.end);
                if from >= to {
                    continue;
                }
                let (mut x0, mut x1) = (0.0f32, 0.0f32);
                let mut line_top = 0.0f32;
                let _ = layout.HitTestTextPosition(from, false, &mut x0, &mut line_top, &mut hit);
                let _ = layout.HitTestTextPosition(to, false, &mut x1, &mut line_top, &mut hit);
                target.FillRectangle(
                    &rect_f(
                        rect.left + x0,
                        rect.top + line.top,
                        rect.left + x1,
                        rect.top + line.bottom,
                    ),
                    brush,
                );
            }
        }
        draw_edit_text(
            target,
            &layout,
            buffer,
            rect,
            brush,
            colors,
            (start16, end16),
        );
        if caret_visible {
            let caret16 = utf16_index(edit.text(), edit.caret());
            let (mut cx, mut cy) = (0.0f32, 0.0f32);
            let _ = layout.HitTestTextPosition(caret16, false, &mut cx, &mut cy, &mut hit);
            let caret_height = hit.height.max(1.0);
            brush.SetColor(&color_f(colors.text));
            target.FillRectangle(
                &rect_f(
                    rect.left + cx,
                    rect.top + cy,
                    rect.left + cx + 1.0,
                    rect.top + cy + caret_height,
                ),
                brush,
            );
        }
        target.PopAxisAlignedClip();
    }
}

/// The rename box for `lines` laid out in `rect`: their height plus an even
/// margin, and either the text's own width or all of `rect`'s. Clamped to
/// `rect`, so a line that reaches past it still draws inside. A buffer with
/// no measured lines gets a small box at the layout's top.
fn edit_field(rect: D2D_RECT_F, lines: &[EditLine], extent: EditWidth) -> D2D_RECT_F {
    let (text_left, text_right, top, bottom) = if lines.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (
            lines
                .iter()
                .map(|line| line.left)
                .fold(f32::INFINITY, f32::min),
            lines
                .iter()
                .map(|line| line.right)
                .fold(f32::NEG_INFINITY, f32::max),
            lines[0].top,
            lines[lines.len() - 1].bottom,
        )
    };
    let (field_left, field_right) = match extent {
        EditWidth::Text => (
            (rect.left + text_left - EDIT_PAD_X).max(rect.left),
            (rect.left + text_right + EDIT_PAD_X).min(rect.right),
        ),
        EditWidth::Label => (rect.left, rect.right),
    };
    let field_top = (rect.top + top - EDIT_PAD_Y).max(rect.top);
    let field_bottom = (rect.top + bottom + EDIT_PAD_Y).min(rect.bottom);
    rect_f(
        field_left,
        field_top,
        field_right.max(field_left + 1.0),
        field_bottom.max(field_top + 1.0),
    )
}

/// Draws `buffer` in the field's text color, with the selected run in the
/// highlight-text color, using drawing effects so the runs match the layout.
fn draw_edit_text(
    target: &ID2D1RenderTarget,
    layout: &IDWriteTextLayout,
    buffer: &[u16],
    rect: D2D_RECT_F,
    brush: &ID2D1SolidColorBrush,
    colors: EditColors,
    (start16, end16): (u32, u32),
) {
    let origin = Vector2::new(rect.left, rect.top);
    // SAFETY: caller is inside BeginDraw/EndDraw, and the range endpoints are
    // clamped to the buffer length.
    unsafe {
        brush.SetColor(&color_f(colors.text));
        let len = buffer.len() as u32;
        let start16 = start16.min(len);
        let end16 = end16.min(len).max(start16);
        if start16 == end16 {
            target.DrawTextLayout(origin, layout, brush, D2D1_DRAW_TEXT_OPTIONS_CLIP);
            return;
        }
        let Ok(selected) = target.CreateSolidColorBrush(&color_f(colors.selection_text), None)
        else {
            target.DrawTextLayout(origin, layout, brush, D2D1_DRAW_TEXT_OPTIONS_CLIP);
            return;
        };
        let _ = layout.SetDrawingEffect(
            brush,
            DWRITE_TEXT_RANGE {
                startPosition: 0,
                length: start16,
            },
        );
        let _ = layout.SetDrawingEffect(
            &selected,
            DWRITE_TEXT_RANGE {
                startPosition: start16,
                length: end16 - start16,
            },
        );
        let _ = layout.SetDrawingEffect(
            brush,
            DWRITE_TEXT_RANGE {
                startPosition: end16,
                length: len - end16,
            },
        );
        target.DrawTextLayout(origin, layout, brush, D2D1_DRAW_TEXT_OPTIONS_CLIP);
    }
}

/// A byte offset in the text, counted in UTF-16 code units for DirectWrite.
/// Clamps to the text length if the offset is not on a boundary.
fn utf16_index(text: &str, byte: usize) -> u32 {
    text.get(..byte).unwrap_or(text).encode_utf16().count() as u32
}

#[cfg(test)]
mod tests {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    use super::item::grid_label_width;
    use super::*;
    use crate::render::ItemLayout;

    fn line(start: u32, top: f32, left: f32, right: f32) -> EditLine {
        EditLine {
            start,
            end: start + 3,
            top,
            bottom: top + 16.0,
            left,
            right,
        }
    }

    #[test]
    fn a_box_that_hugs_its_text_spans_all_wrapped_lines() {
        let lines = [
            line(0, 0.0, 10.0, 40.0),
            line(3, 16.0, 5.0, 45.0),
            line(6, 32.0, 8.0, 30.0),
        ];
        let field = edit_field(rect_f(0.0, 0.0, 100.0, 1000.0), &lines, EditWidth::Text);
        assert_eq!(
            (field.left, field.top, field.right, field.bottom),
            (2.0, 0.0, 48.0, 50.0)
        );
    }

    #[test]
    fn an_icon_cells_box_spans_its_label() {
        let lines = [line(0, 0.0, 20.0, 42.0)];
        let field = edit_field(rect_f(100.0, 40.0, 175.0, 1064.0), &lines, EditWidth::Label);
        assert_eq!((field.left, field.right), (100.0, 175.0));
        assert_eq!(
            (field.top, field.bottom),
            (40.0, 58.0),
            "one line and a margin"
        );
    }

    #[test]
    fn a_two_line_name_grows_an_icon_cells_box_downward() {
        let lines = [line(0, 0.0, 0.0, 75.0), line(3, 16.0, 0.0, 60.0)];
        let field = edit_field(rect_f(100.0, 40.0, 175.0, 1064.0), &lines, EditWidth::Label);
        assert_eq!((field.left, field.right), (100.0, 175.0));
        assert_eq!(
            (field.top, field.bottom),
            (40.0, 74.0),
            "two lines and a margin"
        );
    }

    #[test]
    fn a_name_wider_than_its_label_keeps_the_box_on_the_label() {
        let lines = [line(0, 0.0, 0.0, 120.0)];
        let rect = rect_f(100.0, 40.0, 175.0, 1064.0);
        let field = edit_field(rect, &lines, EditWidth::Label);
        assert_eq!((field.left, field.right), (rect.left, rect.right));
        let hugged = edit_field(rect, &lines, EditWidth::Text);
        assert_eq!(hugged.right, rect.right, "a row clamps to its box too");
    }

    fn layout(
        graphics: &Graphics,
        name: &str,
        width: f32,
        size: u8,
    ) -> (Vec<EditLine>, D2D_RECT_F) {
        let text: Vec<u16> = name.encode_utf16().collect();
        let format = graphics.item_label_format_for(size, ItemLayout::Grid);
        let rect = rect_f(100.0, 40.0, 100.0 + width, 40.0 + EDIT_MAX_HEIGHT);
        // SAFETY: the layout is made from a live format for this call; nothing
        // is drawing, and the COM apartment belongs to this test thread.
        let lines = unsafe {
            let layout = graphics
                .dwrite
                .CreateTextLayout(&text, &format, width, EDIT_MAX_HEIGHT)
                .expect("a text layout");
            edit_lines(&layout)
        };
        (lines, rect)
    }

    #[test]
    fn a_real_name_is_centred_in_an_icon_cells_box() {
        // SAFETY: initialises COM on the test thread; a repeat call is
        // harmless.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let graphics = graphics().expect("graphics");
        // The box is an icon cell's label width: Explorer's 75 px desktop
        // step, and a cell at 150 %.
        for (cell, size) in [(75.0, 100), (113.0, 150)] {
            let width = grid_label_width(cell);
            let (lines, rect) = layout(&graphics, "Firefox", width, size);
            assert_eq!(lines.len(), 1, "{size} %: a short name takes one line");
            let field = edit_field(rect, &lines, EditWidth::Label);
            assert_eq!(
                (field.left, field.right),
                (rect.left, rect.right),
                "{size} %: the box spans the label"
            );
            let text_centre = (lines[0].left + lines[0].right) / 2.0;
            let box_centre = width / 2.0;
            assert!(
                (text_centre - box_centre).abs() <= 1.5,
                "{size} %: the name is centred in the box, at {text_centre} of {box_centre}"
            );
        }
    }

    #[test]
    fn a_real_long_name_wraps_and_grows_the_box_downward() {
        // SAFETY: as above.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let graphics = graphics().expect("graphics");
        let (lines, rect) = layout(&graphics, "Quarterly review", grid_label_width(75.0), 100);
        assert_eq!(lines.len(), 2, "wraps inside the label");
        let field = edit_field(rect, &lines, EditWidth::Label);
        assert_eq!((field.left, field.right), (rect.left, rect.right));
        assert_eq!(field.bottom, rect.top + lines[1].bottom + EDIT_PAD_Y);
        assert!(
            field.bottom - field.top > lines[0].bottom - lines[0].top,
            "taller than a one-line box"
        );
    }
}
