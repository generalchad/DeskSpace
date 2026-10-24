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

/// Draws the rubber-band selection rectangle over empty space.
pub(super) fn draw_marquee(
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
    cells: &[Vec<u16>],
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
    let inline = scene.item_layout == ItemLayout::Inline;
    let details = scene.item_layout == ItemLayout::Details;
    let beside_icon = inline || details;
    let icon_size = metrics.icon_size as f32;
    let icon_pad = metrics.icon_top as f32;
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
                    rect: inset(cell, 2.0),
                    radiusX: 4.0,
                    radiusY: 4.0,
                },
                brush,
            );
        }

        // The grid centers the icon over its label; an inline cell puts the
        // check box, then the icon, then one label line, all vertically
        // centered (W16).
        let (icon_left, icon_top) = if beside_icon {
            let left = cell.left
                + (metrics.inline_inset + metrics.check_box_size + metrics.inline_gap) as f32;
            let top = cell.top + ((cell.bottom - cell.top - icon_size) / 2.0).round();
            (left, top)
        } else {
            let center_x = (cell.left + cell.right) / 2.0;
            ((center_x - icon_size / 2.0).round(), cell.top + icon_pad)
        };
        if let Some(bitmap) = icon {
            target.DrawBitmap(
                bitmap,
                Some(&rect_f(
                    icon_left,
                    icon_top,
                    icon_left + icon_size,
                    icon_top + icon_size,
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
            let check_box = if beside_icon {
                CheckBoxRect {
                    left: cell.left + metrics.inline_inset as f32,
                    top: cell.top
                        + (((cell.bottom - cell.top) as i32 - metrics.check_box_size) / 2) as f32,
                    size: metrics.check_box_size as f32,
                }
            } else {
                CheckBoxRect {
                    left: icon_left + margin,
                    top: cell.top + icon_pad + margin,
                    size: metrics.check_box_size as f32,
                }
            };
            draw_check_box(ctx, check_box, item.checked, scene.check_fill);
        }
        let scale = scene
            .label_size
            .clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1) as f32
            / 100.0;
        let line_height = LABEL_LINE_HEIGHT * scale;
        let center_y = (cell.top + cell.bottom) / 2.0;
        let (label_left, label_top, label_right, lines) = if details {
            let name = scene
                .details
                .as_ref()
                .and_then(|details| details.columns.first());
            let right = name.map_or(cell.right - metrics.inline_inset as f32, |column| {
                column.rect.right() as f32 - scene.details.as_ref().map_or(0, |d| d.cell_pad) as f32
            });
            let top = (center_y - line_height / 2.0).round();
            (
                icon_left + icon_size + metrics.inline_gap as f32,
                top,
                right,
                1.0,
            )
        } else if inline {
            // One line beside the icon, vertically centered and ellipsized.
            let top = (center_y - line_height / 2.0).round();
            (
                icon_left + icon_size + metrics.inline_gap as f32,
                top,
                cell.right - metrics.inline_inset as f32,
                1.0,
            )
        } else {
            (
                cell.left + 4.0,
                cell.top + icon_pad + icon_size + metrics.label_gap as f32,
                cell.right - 4.0,
                scene.label_lines.clamp(1, 2) as f32,
            )
        };
        let label_bottom = (label_top + lines * line_height).min(cell.bottom);
        if label_bottom > label_top {
            let format = if details {
                graphics.details_format.clone()
            } else {
                graphics.item_label_format_for(scene.label_size, scene.item_layout)
            };
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
                        label_left,
                        label_top,
                        label_right,
                        label_top + EDIT_MAX_HEIGHT,
                    ),
                    scene.edit,
                    scene.caret_visible,
                );
            } else {
                let label_rect = rect_f(label_left, label_top, label_right, label_bottom);
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

        // Details: the other columns' text, in their header rects (W18).
        if details && let Some(details_view) = &scene.details {
            for (column, text) in details_view.columns.iter().skip(1).zip(cells) {
                let format = match column.align {
                    ColumnAlign::Left => graphics.details_format.clone(),
                    ColumnAlign::Right => graphics.details_right_format.clone(),
                };
                let pad = details_view.cell_pad as f32;
                draw_text(
                    target,
                    brush,
                    text,
                    &format,
                    rect_f(
                        column.rect.x as f32 + pad,
                        cell.top,
                        column.rect.right() as f32 - pad,
                        cell.bottom,
                    ),
                    item.label_color.unwrap_or(scene.item_text).fade(alpha),
                    scene.text_shadow.map(|shadow| shadow.fade(alpha)),
                );
            }
        }

        // A dotted rectangle marks the item with keyboard focus, as Explorer
        // draws on a focused view item. A faded dragged slot keeps none.
        if scene.focused_item == Some(item.id) && item.state != ItemState::Dragged {
            brush.SetColor(&color_f(scene.item_text.with_alpha(220)));
            target.DrawRectangle(&inset(cell, 2.0), brush, 1.0, Some(&graphics.focus_stroke));
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
    let radius = (size * 0.19).max(1.0);
    let rounded = D2D1_ROUNDED_RECT {
        rect,
        radiusX: radius,
        radiusY: radius,
    };
    // SAFETY: caller is inside BeginDraw/EndDraw.
    unsafe {
        if checked {
            // Explorer fills a checked box with the accent and no border.
            brush.SetColor(&color_f(check_fill.with_alpha(255)));
            target.FillRoundedRectangle(&rounded, brush);
            brush.SetColor(&color_f(Color::rgb(255, 255, 255)));
            let s = size;
            let stroke = (size * 0.12).max(1.0);
            target.DrawLine(
                Vector2::new(left + s * 0.22, top + s * 0.52),
                Vector2::new(left + s * 0.43, top + s * 0.74),
                brush,
                stroke,
                None,
            );
            target.DrawLine(
                Vector2::new(left + s * 0.43, top + s * 0.74),
                Vector2::new(left + s * 0.80, top + s * 0.26),
                brush,
                stroke,
                None,
            );
        } else {
            brush.SetColor(&color_f(Color::rgba(255, 255, 255, 210)));
            target.FillRoundedRectangle(&rounded, brush);
            brush.SetColor(&color_f(Color::rgba(0, 0, 0, 150)));
            target.DrawRoundedRectangle(&rounded, brush, 1.0, None);
        }
    }
}
