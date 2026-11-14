//! Details column headers, group headers, and the status bar.

use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE;
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows_numerics::Vector2;

use crate::render::scene::StatusBar;

use super::geometry::{color_f, draw_text, fill_rounded, rect_f, to_rect_f};
use super::prepared::PreparedScene;
use super::{DrawContext, Painter};

/// Segoe Fluent Icons / Segoe MDL2 Assets "ChevronUp" and "ChevronDown", the
/// sorted Details header's arrow.
const SORT_UP_GLYPH: &[u16] = &[0xE70E];
const SORT_DOWN_GLYPH: &[u16] = &[0xE70D];
/// The dragged header's insertion mark, as a share of the header's height, so
/// it scales with the display.
const DROP_MARK_WIDTH_RATIO: f32 = 0.08;

impl Painter {
    /// Draws the Details header row at `top`: each column's label, Explorer's
    /// sort arrow on the sorted column, and a rule along its bottom.
    pub(super) unsafe fn draw_details_header(
        &self,
        ctx: DrawContext,
        prepared: &PreparedScene,
        width: f32,
        top: f32,
    ) {
        let Some(details) = prepared.scene.details.as_ref() else {
            return;
        };
        let DrawContext {
            graphics,
            target,
            brush,
        } = ctx;
        let scene = &prepared.scene;
        let height = details.header_height as f32;
        let pad = details.cell_pad as f32;
        let bottom = top + height;
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            brush.SetColor(&color_f(scene.item_text.fade(80)));
            target.DrawLine(
                Vector2::new(0.0, bottom - 0.5),
                Vector2::new(width, bottom - 0.5),
                brush,
                1.0,
                None,
            );
            for (index, (column, label)) in details.columns.iter().zip(&prepared.header).enumerate()
            {
                let mut right = column.rect.right() as f32 - pad;
                if let Some((_, up)) = details.sorted.filter(|(sorted, _)| *sorted == index)
                    && let Some(format) = &graphics.glyph_format
                {
                    let glyph_width = height * 0.6;
                    brush.SetColor(&color_f(scene.item_text.fade(200)));
                    target.DrawText(
                        if up { SORT_UP_GLYPH } else { SORT_DOWN_GLYPH },
                        format,
                        &rect_f(right - glyph_width, top, right, bottom),
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                    right -= glyph_width;
                }
                draw_text(
                    target,
                    brush,
                    label,
                    &graphics.details_format,
                    rect_f(column.rect.x as f32 + pad, top, right, bottom),
                    scene.item_text.fade(190),
                    scene.text_shadow,
                );
            }
            if let Some(mark) = details.drop_mark {
                let mark = mark as f32;
                brush.SetColor(&color_f(scene.item_text));
                target.DrawLine(
                    Vector2::new(mark, top),
                    Vector2::new(mark, bottom),
                    brush,
                    (height * DROP_MARK_WIDTH_RATIO).round().max(1.0),
                    None,
                );
            }
        }
    }

    /// Draws each group's header row: a chevron, its name and count, and a rule
    /// along its bottom to the edge.
    pub(super) unsafe fn draw_group_headers(
        &self,
        ctx: DrawContext,
        prepared: &PreparedScene,
        width: f32,
    ) {
        let DrawContext {
            graphics,
            target,
            brush,
        } = ctx;
        let scene = &prepared.scene;
        for (header, label) in scene.group_headers.iter().zip(&prepared.group_labels) {
            let rect = to_rect_f(header.rect);
            let glyph_width = (rect.bottom - rect.top) * 0.8;
            // SAFETY: caller is inside BeginDraw/EndDraw.
            unsafe {
                if let Some(format) = &graphics.glyph_format {
                    brush.SetColor(&color_f(scene.item_text.fade(200)));
                    target.DrawText(
                        if header.folded {
                            SORT_DOWN_GLYPH
                        } else {
                            SORT_UP_GLYPH
                        },
                        format,
                        &rect_f(rect.left, rect.top, rect.left + glyph_width, rect.bottom),
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
                draw_text(
                    target,
                    brush,
                    label,
                    &graphics.details_format,
                    rect_f(rect.left + glyph_width, rect.top, rect.right, rect.bottom),
                    scene.item_text,
                    scene.text_shadow,
                );
                brush.SetColor(&color_f(scene.item_text.fade(80)));
                target.DrawLine(
                    Vector2::new(rect.left, rect.bottom - 0.5),
                    Vector2::new(width, rect.bottom - 0.5),
                    brush,
                    1.0,
                    None,
                );
            }
        }
    }

    /// Draws Explorer's status bar: the counts on the left and the view
    /// buttons on the right, with the space's current view marked.
    pub(super) unsafe fn draw_status(
        &self,
        ctx: DrawContext,
        prepared: &PreparedScene,
        status: &StatusBar,
    ) {
        let DrawContext {
            graphics,
            target,
            brush,
        } = ctx;
        let scene = &prepared.scene;
        let rect = to_rect_f(status.rect);
        let height = rect.bottom - rect.top;
        let pad = (height * 0.25).round();
        let text_right = status
            .buttons
            .first()
            .map_or(rect.right - pad, |button| button.rect.x as f32 - pad);
        // SAFETY: caller is inside BeginDraw/EndDraw.
        unsafe {
            brush.SetColor(&color_f(scene.item_text.fade(60)));
            target.DrawLine(
                Vector2::new(rect.left, rect.top + 0.5),
                Vector2::new(rect.right, rect.top + 0.5),
                brush,
                1.0,
                None,
            );
            draw_text(
                target,
                brush,
                &prepared.status_segments,
                &graphics.details_format,
                rect_f(rect.left + pad, rect.top, text_right, rect.bottom),
                scene.item_text.fade(200),
                scene.text_shadow,
            );
            for button in &status.buttons {
                let button_rect = to_rect_f(button.rect);
                if button.checked || button.hovered {
                    let radius = (button_rect.bottom - button_rect.top) * 0.2;
                    fill_rounded(
                        target,
                        brush,
                        button_rect,
                        radius,
                        if button.hovered {
                            scene.hover_fill
                        } else {
                            scene.selected_fill
                        },
                    );
                }
                if let Some(format) = &graphics.glyph_format {
                    brush.SetColor(&color_f(scene.item_text.fade(if button.checked {
                        255
                    } else {
                        200
                    })));
                    target.DrawText(
                        button.glyph,
                        format,
                        &button_rect,
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            }
        }
    }
}
