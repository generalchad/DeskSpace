//! Draws one item cell (highlight, icon, label, and check box) and the
//! rubber-band marquee.

use super::edit::{EDIT_MAX_HEIGHT, draw_edit_buffer};
use super::geometry::{color_f, draw_text, inset, rect_f, to_rect_f};
use super::*;

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
    let cell = to_rect_f(item.rect);
    let metrics = scene.metrics;
    let icon_size = metrics.icon_size as f32;
    let icon_top = metrics.icon_top as f32;
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
                    rect: inset(cell, CELL_INSET),
                    radiusX: HIGHLIGHT_RADIUS,
                    radiusY: HIGHLIGHT_RADIUS,
                },
                brush,
            );
        }

        let center_x = (cell.left + cell.right) / 2.0;
        let icon_left = (center_x - icon_size / 2.0).round();
        if let Some(bitmap) = icon {
            let top = cell.top + icon_top;
            target.DrawBitmap(
                bitmap,
                Some(&rect_f(
                    icon_left,
                    top,
                    icon_left + icon_size,
                    top + icon_size,
                )),
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
            let margin = metrics.check_box_margin as f32;
            let check_box = CheckBoxRect {
                left: icon_left + margin,
                top: cell.top + icon_top + margin,
                size: metrics.check_box_size as f32,
            };
            draw_check_box(ctx, check_box, item.checked, scene.check_fill);
        }
        let label_top = cell.top + icon_top + icon_size + metrics.label_gap as f32;
        let scale = scene
            .label_size
            .clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1) as f32
            / 100.0;
        let lines = scene.label_lines.clamp(1, 2) as f32;
        let line_height = LABEL_LINE_HEIGHT * scale;
        let label_bottom = (label_top + lines * line_height).min(cell.bottom);
        if label_bottom > label_top {
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
                    rect_f(
                        cell.left + LABEL_INSET,
                        label_top,
                        cell.right - LABEL_INSET,
                        label_top + EDIT_MAX_HEIGHT,
                    ),
                    scene.edit,
                    scene.caret_visible,
                );
            } else {
                let label_rect = rect_f(
                    cell.left + LABEL_INSET,
                    label_top,
                    cell.right - LABEL_INSET,
                    label_bottom,
                );
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
            target.DrawRectangle(
                &inset(cell, CELL_INSET),
                brush,
                1.0,
                Some(&graphics.focus_stroke),
            );
        }
    }
}

/// Where an item's check box is drawn: its top-left corner and side length.
/// The painter keeps the icon's float position, so this can sit a rounding
/// pixel away from `item_check_box`, the whole-pixel rect hit tests use.
#[derive(Clone, Copy)]
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
