//! Draws one item cell (highlight, icon, label, and check box) and the
//! rubber-band marquee.

use super::edit::{EDIT_MAX_HEIGHT, draw_edit_buffer};
use super::geometry::{color_f, draw_text, rect_f, to_rect_f};
use super::*;
use crate::render::{DetailsView, ItemLayout, ItemMetrics};

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
    let details = scene.item_layout == ItemLayout::Details;
    let tiles = scene.item_layout == ItemLayout::Tiles;
    let content = scene.item_layout == ItemLayout::Content;
    let layout = CellLayout::new(
        cell,
        metrics,
        scene.label_size,
        scene.label_lines,
        scene.item_layout,
        scene.details.as_ref(),
    );
    let line_height = layout.line_height;
    let label_top = layout.label_top;
    let name_left = layout.name_left;
    let meta_left = layout.meta_left;
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
            let format = if details || tiles || content {
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

        // Tiles: the type and size, dimmed under the name (W21).
        if tiles {
            for (index, text) in cells.iter().enumerate().take(2) {
                let top = label_top + (index as f32 + 1.0) * line_height;
                draw_text(
                    target,
                    brush,
                    text,
                    &graphics.details_format,
                    rect_f(
                        name_left,
                        top,
                        cell.right - metrics.inline_inset as f32,
                        top + line_height,
                    ),
                    scene.item_text.fade(150),
                    scene.text_shadow.map(|shadow| shadow.fade(alpha)),
                );
            }
        }

        // Content: the labelled details on the right, and the separator that
        // runs under the row (W22).
        if content {
            let mid = meta_left + (cell.right - meta_left) / 2.0;
            let inset = metrics.inline_inset as f32;
            for (index, text) in cells.iter().enumerate().take(2) {
                let (left, right) = if index == 0 {
                    (meta_left, mid - inset)
                } else {
                    (mid, cell.right - inset)
                };
                draw_text(
                    target,
                    brush,
                    text,
                    &graphics.details_right_format,
                    rect_f(left, cell.top, right, cell.bottom),
                    scene.item_text.fade(170),
                    scene.text_shadow.map(|shadow| shadow.fade(alpha)),
                );
            }
            brush.SetColor(&color_f(scene.item_text.fade(40)));
            target.DrawLine(
                Vector2::new(cell.left, cell.bottom - 0.5),
                Vector2::new(cell.right, cell.bottom - 0.5),
                brush,
                1.0,
                None,
            );
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
/// layout is tested apart from the drawing. The cell's shape follows the
/// space's view (`ItemLayout`): a grid centers the icon over a two-line label,
/// while Small, List, Details, Tiles, and Content put the icon beside the name.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CellLayout {
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
    /// The first label line's top and its height, for the extra rows Details,
    /// Tiles, and Content draw below the name.
    label_top: f32,
    line_height: f32,
    /// The name's left edge, and where a Content row's details begin.
    name_left: f32,
    meta_left: f32,
}

impl CellLayout {
    /// The layout of `cell` at `metrics` in `mode`, with labels at
    /// `label_size` percent (`LABEL_SIZE_RANGE`) and `label_lines` (one or two)
    /// tall. `details` gives Details view's name column.
    fn new(
        cell: D2D_RECT_F,
        metrics: ItemMetrics,
        label_size: u8,
        label_lines: u8,
        mode: ItemLayout,
        details: Option<&DetailsView>,
    ) -> Self {
        let icon_size = metrics.icon_size as f32;
        // A grid centers the icon over its label; every other view puts the
        // check box, then the icon, then the name on one line, vertically
        // centered (W16).
        let beside_icon = !matches!(mode, ItemLayout::Grid);
        let (icon_left, icon_top) = if beside_icon {
            let left = cell.left
                + (metrics.inline_inset + metrics.check_box_size + metrics.inline_gap) as f32;
            let top = cell.top + ((cell.bottom - cell.top - icon_size) / 2.0).round();
            (left, top)
        } else {
            let center_x = (cell.left + cell.right) / 2.0;
            (
                (center_x - icon_size / 2.0).round(),
                cell.top + metrics.icon_top as f32,
            )
        };
        let margin = metrics.check_box_margin as f32;
        let scale = label_size.clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1) as f32 / 100.0;
        let line_height = LABEL_LINE_HEIGHT * scale;
        let inset = metrics.inline_inset as f32;
        let center_y = (cell.top + cell.bottom) / 2.0;
        let name_left = icon_left + icon_size + metrics.inline_gap as f32;
        // A content row keeps a right-hand area for its labelled details; the
        // width follows the icon size, so it scales with the display (W22).
        let meta_left = if mode == ItemLayout::Content {
            (cell.right - (360.0 * icon_size / 16.0))
                .max(cell.left + (cell.right - cell.left) * 0.4)
        } else {
            cell.right
        };
        let (label_left, label_top, label_right, lines) = match mode {
            ItemLayout::Details => {
                let name = details.and_then(|details| details.columns.first());
                let right = name.map_or(cell.right - inset, |column| {
                    column.rect.right() as f32 - details.map_or(0, |d| d.cell_pad) as f32
                });
                (
                    name_left,
                    (center_y - line_height / 2.0).round(),
                    right,
                    1.0,
                )
            }
            ItemLayout::Tiles => (
                name_left,
                (center_y - line_height * 1.5).round(),
                cell.right - inset,
                1.0,
            ),
            ItemLayout::Content => (
                name_left,
                (center_y - line_height / 2.0).round(),
                meta_left - metrics.inline_gap as f32,
                1.0,
            ),
            ItemLayout::Inline => (
                name_left,
                (center_y - line_height / 2.0).round(),
                cell.right - inset,
                1.0,
            ),
            ItemLayout::Grid => (
                cell.left + LABEL_INSET,
                icon_top + icon_size + metrics.label_gap as f32,
                cell.right - LABEL_INSET,
                label_lines.clamp(1, 2) as f32,
            ),
        };
        let label_bottom = (label_top + lines * line_height).min(cell.bottom);
        let check_box = if beside_icon {
            CheckBoxRect {
                left: cell.left + inset,
                top: cell.top
                    + (((cell.bottom - cell.top) as i32 - metrics.check_box_size) / 2) as f32,
                size: metrics.check_box_size as f32,
            }
        } else {
            CheckBoxRect {
                left: icon_left + margin,
                top: icon_top + margin,
                size: metrics.check_box_size as f32,
            }
        };
        Self {
            frame: super::geometry::inset(cell, CELL_INSET),
            icon: rect_f(
                icon_left,
                icon_top,
                icon_left + icon_size,
                icon_top + icon_size,
            ),
            check_box,
            label: (label_bottom > label_top)
                .then(|| rect_f(label_left, label_top, label_right, label_bottom)),
            edit: rect_f(
                label_left,
                label_top,
                label_right,
                label_top + EDIT_MAX_HEIGHT,
            ),
            label_top,
            line_height,
            name_left,
            meta_left,
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

    fn layout(cell: D2D_RECT_F) -> CellLayout {
        CellLayout::new(
            cell,
            ItemMetrics::for_scale(1.0),
            100,
            2,
            ItemLayout::Grid,
            None,
        )
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
        assert_eq!(layout.frame, super::geometry::inset(cell, CELL_INSET));
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
            let label = CellLayout::new(
                cell,
                ItemMetrics::for_scale(1.0),
                size,
                lines,
                ItemLayout::Grid,
                None,
            )
            .label
            .unwrap();
            label.bottom - label.top
        };
        assert_eq!(height(100, 1), LABEL_LINE_HEIGHT);
        assert_eq!(height(100, 9), height(100, 2), "two lines at most");
        assert_eq!(height(255, 1), height(LABEL_SIZE_RANGE.1, 1));
    }
}
