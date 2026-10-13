//! Draws Explorer's inline rename field: an opaque box with the selected
//! text highlighted and a blinking caret.

use super::geometry::{color_f, draw_text, rect_f};
use super::*;

/// Draws the title rename buffer in the title bar.
pub(super) unsafe fn draw_edit_title(
    graphics: &Graphics,
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
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
            graphics,
            target,
            brush,
            edit,
            buffer,
            &graphics.edit_format,
            rect_f(left, 0.0, right, title_height),
            scene.edit,
            scene.caret_visible,
        );
    }
}

/// Padding between the text and its rename box, matching Explorer's small,
/// even margin.
const EDIT_PAD_X: f32 = 3.0;
const EDIT_PAD_Y: f32 = 2.0;

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
/// and a blinking caret. The measured lines place the box, selection, and
/// caret, so all three follow DirectWrite's own shaping.
#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn draw_edit_buffer(
    graphics: &Graphics,
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    edit: &TextEdit,
    buffer: &[u16],
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    colors: EditColors,
    caret_visible: bool,
) {
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
        let field = edit_field(rect, &lines);

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

/// The rename box, hugging the measured text with an even margin and clamped
/// to the label or title area it is drawn in. An empty buffer falls back to a
/// small caret-sized box.
fn edit_field(rect: D2D_RECT_F, lines: &[EditLine]) -> D2D_RECT_F {
    let (left, right, top, bottom) = if lines.is_empty() {
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
    let field_left = (rect.left + left - EDIT_PAD_X).max(rect.left);
    let field_top = (rect.top + top - EDIT_PAD_Y).max(rect.top);
    let field_right = (rect.left + right + EDIT_PAD_X).min(rect.right);
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
    use super::*;

    #[test]
    fn edit_field_hugs_all_wrapped_lines() {
        let line = |start, top, left, right| EditLine {
            start,
            end: start + 3,
            top,
            bottom: top + 12.0,
            left,
            right,
        };
        let lines = [
            line(0, 0.0, 10.0, 40.0),
            line(3, 12.0, 5.0, 45.0),
            line(6, 24.0, 8.0, 30.0),
        ];
        let field = edit_field(rect_f(0.0, 0.0, 100.0, 1000.0), &lines);
        assert_eq!(
            (field.left, field.top, field.right, field.bottom),
            (2.0, 0.0, 48.0, 38.0)
        );
    }
}
