//! Draws one item cell (highlight, icon, label, and check box) and the
//! rubber-band marquee.

use super::edit::{EDIT_MAX_HEIGHT, draw_edit_buffer};
use super::geometry::{color_f, draw_text, inset, rect_f, to_rect_f};
use super::*;
use crate::render::ItemMetrics;

/// The selection box fill's alpha, a translucent shade of its color like
/// Explorer's rubber band.
const MARQUEE_FILL_ALPHA: u8 = 70;
/// The selection box border's alpha, close to solid.
const MARQUEE_BORDER_ALPHA: u8 = 200;
/// How far an item's highlight and focus rectangle sit inside its cell.
const CELL_INSET: f32 = 2.0;
const HIGHLIGHT_RADIUS: f32 = 4.0;
/// The space between a label and its cell's sides.
const LABEL_INSET: f32 = 4.0;
/// The focus rectangle's alpha.
const FOCUS_ALPHA: u8 = 220;
/// The check box's corner radius and tick width, as shares of its side.
const CHECK_RADIUS_SHARE: f32 = 0.19;
const CHECK_STROKE_SHARE: f32 = 0.12;
/// The tick's three points, as shares of the box's side from its top-left.
const TICK: [(f32, f32); 3] = [(0.22, 0.52), (0.43, 0.74), (0.80, 0.26)];
const TICK_COLOR: Color = Color::rgb(255, 255, 255);
/// An unchecked box: light, with a dark border that shows on any backdrop.
const UNCHECKED_FILL: Color = Color::rgba(255, 255, 255, 210);
const UNCHECKED_BORDER: Color = Color::rgba(0, 0, 0, 150);

/// Draws the rubber-band selection rectangle over empty space.
///
/// # Safety
/// Call between `BeginDraw` and `EndDraw` on `target`.
pub(super) unsafe fn draw_marquee(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    marquee: Rect,
    color: Color,
    border: bool,
) {
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        let rect = to_rect_f(marquee);
        brush.SetColor(&color_f(color.with_alpha(MARQUEE_FILL_ALPHA)));
        target.FillRectangle(&rect, brush);
        if border {
            brush.SetColor(&color_f(color.with_alpha(MARQUEE_BORDER_ALPHA)));
            target.DrawRectangle(&rect, brush, 1.0, None);
        }
    }
}

/// Draws one item cell: highlight, icon, and label. Shared by the space
/// painter and `render_items`, so a drag image is exactly what the space shows.
pub(super) unsafe fn draw_item(
    ctx: DrawContext,
    scene: &SpaceScene,
    item: &ItemVisual,
    label: &[u16],
    icon: Option<&ID2D1Bitmap>,
    edit: Option<&[u16]>,
) {
    let DrawContext {
        graphics,
        target,
        brush,
    } = ctx;
    let layout = ItemLayout::new(
        to_rect_f(item.rect),
        scene.metrics,
        scene.label_size,
        scene.label_lines,
    );
    let highlight = match item.state {
        ItemState::Normal | ItemState::Dragged => None,
        ItemState::Hovered => Some(scene.hover_fill),
        ItemState::Selected => Some(scene.selected_fill),
    };
    let alpha = if item.state == ItemState::Dragged {
        DRAGGED_ALPHA
    } else if item.cut {
        CUT_ALPHA
    } else {
        255
    };
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        if let Some(color) = highlight {
            brush.SetColor(&color_f(color));
            target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: layout.frame,
                    radiusX: HIGHLIGHT_RADIUS,
                    radiusY: HIGHLIGHT_RADIUS,
                },
                brush,
            );
        }

        if let Some(bitmap) = icon {
            target.DrawBitmap(
                bitmap,
                Some(&layout.icon),
                alpha as f32 / 255.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
        // Explorer shows a check box while pointing at the item, and keeps it
        // while the item stays selected (independent item check boxes are
        // shown always); a dragged slot is faded and shows no box.
        let show_box = item.state != ItemState::Dragged
            && (scene.check_boxes_persistent || item.checked || item.hovered);
        if scene.show_check_boxes && show_box {
            draw_check_box(ctx, layout.check_box, item.checked, scene.check_fill);
        }
        if let Some(label_rect) = layout.label {
            let format = graphics.label_format_for(scene.label_size);
            if let (Some(buffer), Some((_, text_edit))) = (edit, &scene.editing_item) {
                // Explorer's rename box wraps the whole name and grows downward
                // over the items below it, so lay it out in the label's width
                // but a tall box; `draw_edit_buffer` hugs the measured lines.
                draw_edit_buffer(
                    ctx,
                    text_edit,
                    buffer,
                    &format,
                    layout.edit,
                    scene.edit,
                    scene.caret_visible,
                );
            } else {
                draw_text(
                    target,
                    brush,
                    label,
                    &format,
                    label_rect,
                    item.label_color.unwrap_or(scene.item_text).fade(alpha),
                    scene.text_shadow.map(|shadow| shadow.fade(alpha)),
                );
            }
        }

        // A dotted rectangle marks the item with keyboard focus, as Explorer
        // draws on a focused view item. A faded dragged slot keeps none.
        if scene.focused_item == Some(item.id) && item.state != ItemState::Dragged {
            brush.SetColor(&color_f(scene.item_text.with_alpha(FOCUS_ALPHA)));
            target.DrawRectangle(&layout.frame, brush, 1.0, Some(&graphics.focus_stroke));
        }
    }
}

/// Where an item cell's parts go, in the scene's coordinates. Pure, so the
/// layout is tested apart from the drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ItemLayout {
    /// Where the highlight and the focus rectangle go.
    frame: D2D_RECT_F,
    /// The icon, centred on whole pixels.
    icon: D2D_RECT_F,
    check_box: CheckBoxRect,
    /// The label's box; `None` when the cell has no room below the icon.
    label: Option<D2D_RECT_F>,
    /// The rename box's: the label's width, and tall enough to grow over
    /// the items below.
    edit: D2D_RECT_F,
}

impl ItemLayout {
    /// The layout of `cell` at `metrics`, with labels at `label_size`
    /// percent (`LABEL_SIZE_RANGE`) and `label_lines` (one or two) tall.
    fn new(cell: D2D_RECT_F, metrics: ItemMetrics, label_size: u8, label_lines: u8) -> Self {
        let icon_size = metrics.icon_size as f32;
        let icon_top = cell.top + metrics.icon_top as f32;
        let icon_left = ((cell.left + cell.right) / 2.0 - icon_size / 2.0).round();
        let margin = metrics.check_box_margin as f32;
        let scale = label_size.clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1) as f32 / 100.0;
        let lines = label_lines.clamp(1, 2) as f32;
        let label_top = icon_top + icon_size + metrics.label_gap as f32;
        let label_bottom = (label_top + lines * (LABEL_LINE_HEIGHT * scale)).min(cell.bottom);
        let (left, right) = (cell.left + LABEL_INSET, cell.right - LABEL_INSET);
        Self {
            frame: inset(cell, CELL_INSET),
            icon: rect_f(
                icon_left,
                icon_top,
                icon_left + icon_size,
                icon_top + icon_size,
            ),
            check_box: CheckBoxRect {
                left: icon_left + margin,
                top: icon_top + margin,
                size: metrics.check_box_size as f32,
            },
            label: (label_bottom > label_top).then(|| rect_f(left, label_top, right, label_bottom)),
            edit: rect_f(left, label_top, right, label_top + EDIT_MAX_HEIGHT),
        }
    }
}

/// Where an item's check box is drawn: its top-left corner and side length.
/// The painter keeps the icon's float position, so this can sit a rounding
/// pixel away from `item_check_box`, the whole-pixel rect hit tests use.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CheckBoxRect {
    left: f32,
    top: f32,
    size: f32,
}

/// Draws Explorer's item check box at the top-left of an icon. Checked, it is
/// filled with the accent (`check_fill`) and shows a white tick, as Windows
/// draws a checked box; unchecked, it is a light box with a dark border that
/// stays visible on any backdrop.
unsafe fn draw_check_box(
    ctx: DrawContext,
    check_box: CheckBoxRect,
    checked: bool,
    check_fill: Color,
) {
    let DrawContext { target, brush, .. } = ctx;
    let CheckBoxRect { left, top, size } = check_box;
    let rect = rect_f(left, top, left + size, top + size);
    let radius = (size * CHECK_RADIUS_SHARE).max(1.0);
    let rounded = D2D1_ROUNDED_RECT {
        rect,
        radiusX: radius,
        radiusY: radius,
    };
    let point = |(x, y): (f32, f32)| Vector2::new(left + size * x, top + size * y);
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        if checked {
            // Explorer fills a checked box with the accent and no border.
            brush.SetColor(&color_f(check_fill.with_alpha(255)));
            target.FillRoundedRectangle(&rounded, brush);
            brush.SetColor(&color_f(TICK_COLOR));
            let stroke = (size * CHECK_STROKE_SHARE).max(1.0);
            for [from, to] in [[TICK[0], TICK[1]], [TICK[1], TICK[2]]] {
                target.DrawLine(point(from), point(to), brush, stroke, None);
            }
        } else {
            brush.SetColor(&color_f(UNCHECKED_FILL));
            target.FillRoundedRectangle(&rounded, brush);
            brush.SetColor(&color_f(UNCHECKED_BORDER));
            target.DrawRoundedRectangle(&rounded, brush, 1.0, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(cell: D2D_RECT_F) -> ItemLayout {
        ItemLayout::new(cell, ItemMetrics::for_scale(1.0), 100, 2)
    }

    #[test]
    fn the_icon_is_centred_on_whole_pixels_with_the_check_box_at_its_corner() {
        let metrics = ItemMetrics::for_scale(1.0);
        let cell = rect_f(100.0, 200.0, 197.0, 304.0);
        let layout = layout(cell);
        let size = metrics.icon_size as f32;
        assert_eq!(layout.icon.left, layout.icon.left.round());
        assert!(((layout.icon.left + size / 2.0) - 148.5).abs() <= 0.5);
        assert_eq!(layout.icon.top, cell.top + metrics.icon_top as f32);
        assert_eq!(layout.icon.right - layout.icon.left, size);
        let margin = metrics.check_box_margin as f32;
        assert_eq!(
            (layout.check_box.left, layout.check_box.top),
            (layout.icon.left + margin, layout.icon.top + margin)
        );
    }

    #[test]
    fn the_label_sits_below_the_icon_inside_the_cell() {
        let metrics = ItemMetrics::for_scale(1.0);
        let cell = rect_f(0.0, 0.0, 96.0, 400.0);
        let layout = layout(cell);
        let label = layout.label.expect("room for a label");
        assert_eq!(label.top, layout.icon.bottom + metrics.label_gap as f32);
        assert_eq!((label.left, label.right), (LABEL_INSET, 96.0 - LABEL_INSET));
        assert_eq!(
            label.bottom - label.top,
            2.0 * LABEL_LINE_HEIGHT,
            "two lines"
        );
        assert!(layout.edit.bottom > label.bottom, "the rename box can grow");
        assert_eq!(layout.frame, inset(cell, CELL_INSET));
    }

    #[test]
    fn a_short_cell_clips_the_label_or_leaves_none() {
        let short = layout(rect_f(0.0, 0.0, 96.0, 80.0));
        assert_eq!(short.label.map(|label| label.bottom), Some(80.0));
        assert_eq!(layout(rect_f(0.0, 0.0, 96.0, 50.0)).label, None);
    }

    #[test]
    fn the_label_follows_its_size_and_lines_within_their_ranges() {
        let cell = rect_f(0.0, 0.0, 96.0, 400.0);
        let height = |size, lines| {
            let label = ItemLayout::new(cell, ItemMetrics::for_scale(1.0), size, lines)
                .label
                .unwrap();
            label.bottom - label.top
        };
        assert_eq!(height(100, 1), LABEL_LINE_HEIGHT);
        assert_eq!(height(100, 9), height(100, 2), "two lines at most");
        assert_eq!(height(255, 1), height(LABEL_SIZE_RANGE.1, 1));
    }
}
