//! The picker's pure geometry: where each part sits and what a point hits.

use super::*;

/// Which page of the picker is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Palette,
    Custom,
}

/// Which interactive part of the picker a point falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Region {
    /// A swatch in the palette, by index into the recent-then-standard list.
    Swatch(usize),
    Saturation,
    Hue,
    Alpha,
    Toggle,
    Ok,
    Cancel,
}

impl Region {
    pub(super) fn is_value(self) -> bool {
        matches!(self, Region::Saturation | Region::Hue | Region::Alpha)
    }
}

/// The picker's rectangles, in window-local coordinates. Both panels share one
/// window size, so toggling does not resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PickerLayout {
    pub(super) size: (i32, i32),
    pub(super) recent_label: Rect,
    pub(super) recent_grid: Rect,
    pub(super) windows_label: Rect,
    pub(super) windows_grid: Rect,
    pub(super) saturation: Rect,
    pub(super) hue: Rect,
    pub(super) alpha: Rect,
    pub(super) preview: Rect,
    pub(super) hex: Rect,
    pub(super) toggle: Rect,
    pub(super) ok: Rect,
    pub(super) cancel: Rect,
    pub(super) swatch: i32,
    pub(super) swatch_gap: i32,
    pub(super) swatch_radius: i32,
}

impl PickerLayout {
    /// Lays the picker out at `scale` (1.0 at 96 DPI).
    pub(super) fn compute(scale: f32) -> Self {
        let s = |value: i32| ((value as f32) * scale).round() as i32;
        let pad = s(PAD);
        let swatch = s(SWATCH);
        let swatch_gap = s(SWATCH_GAP);
        let label = s(LABEL_HEIGHT);
        let button_gap = s(BUTTON_GAP);
        let button_w = s(BUTTON_WIDTH);
        let toggle_w = s(TOGGLE_WIDTH);
        let grid_width = COLS as i32 * swatch + (COLS as i32 - 1) * swatch_gap;
        // The bar of three buttons is wider than the grid, so the window fits
        // both and the grid is centered in the extra width.
        let buttons_width = toggle_w + button_gap + button_w + button_gap + button_w;
        let content_width = grid_width.max(buttons_width);
        let width = content_width + pad * 2;
        let grid_x = pad + (content_width - grid_width) / 2;

        let recent_label = Rect::new(grid_x, pad, grid_width, label);
        let recent_grid = Rect::new(grid_x, recent_label.y + label + s(4), grid_width, swatch);
        let windows_label = Rect::new(
            grid_x,
            recent_grid.y + swatch + s(SECTION_GAP),
            grid_width,
            label,
        );
        let grid_height = ROWS as i32 * swatch + (ROWS as i32 - 1) * swatch_gap;
        let windows_grid = Rect::new(
            grid_x,
            windows_label.y + label + s(4),
            grid_width,
            grid_height,
        );

        let button_y = windows_grid.y + grid_height + s(16);
        let toggle = Rect::new(pad, button_y, toggle_w, s(BUTTON_HEIGHT));
        let ok = Rect::new(width - pad - button_w, button_y, button_w, s(BUTTON_HEIGHT));
        let cancel = Rect::new(
            ok.x - button_gap - button_w,
            button_y,
            button_w,
            s(BUTTON_HEIGHT),
        );
        let size = (width, button_y + s(BUTTON_HEIGHT) + pad);

        let saturation = Rect::new(pad, pad, content_width, s(SQUARE_HEIGHT));
        let hue = Rect::new(
            pad,
            saturation.y + saturation.height + s(FIELD_GAP),
            content_width,
            s(STRIP_HEIGHT),
        );
        let alpha = Rect::new(
            pad,
            hue.y + hue.height + s(FIELD_GAP),
            content_width,
            s(STRIP_HEIGHT),
        );
        let preview = Rect::new(
            pad,
            alpha.y + alpha.height + s(ROW_GAP),
            s(PREVIEW_WIDTH),
            s(PREVIEW_HEIGHT),
        );
        let hex_x = preview.x + preview.width + s(ROW_GAP);
        let hex = Rect::new(
            hex_x,
            preview.y,
            (content_width - (hex_x - pad)).max(0),
            preview.height,
        );

        Self {
            size,
            recent_label,
            recent_grid,
            windows_label,
            windows_grid,
            saturation,
            hue,
            alpha,
            preview,
            hex,
            toggle,
            ok,
            cancel,
            swatch,
            swatch_gap,
            swatch_radius: s(SWATCH_RADIUS),
        }
    }

    /// The rectangle of swatch `index` in `grid`.
    pub(super) fn swatch_rect(&self, grid: Rect, index: usize) -> Rect {
        let col = (index % COLS) as i32;
        let row = (index / COLS) as i32;
        Rect::new(
            grid.x + col * (self.swatch + self.swatch_gap),
            grid.y + row * (self.swatch + self.swatch_gap),
            self.swatch,
            self.swatch,
        )
    }

    pub(super) fn region_at(
        &self,
        panel: Panel,
        recent_len: usize,
        point: Point,
    ) -> Option<Region> {
        if self.ok.contains(point) {
            return Some(Region::Ok);
        }
        if self.cancel.contains(point) {
            return Some(Region::Cancel);
        }
        if self.toggle.contains(point) {
            return Some(Region::Toggle);
        }
        match panel {
            Panel::Palette => {
                for index in 0..recent_len.min(COLS) {
                    if self.swatch_rect(self.recent_grid, index).contains(point) {
                        return Some(Region::Swatch(index));
                    }
                }
                for index in 0..WINDOWS_COLORS.len() {
                    if self.swatch_rect(self.windows_grid, index).contains(point) {
                        return Some(Region::Swatch(recent_len + index));
                    }
                }
                None
            }
            Panel::Custom => {
                if self.saturation.contains(point) {
                    Some(Region::Saturation)
                } else if self.hue.contains(point) {
                    Some(Region::Hue)
                } else if self.alpha.contains(point) {
                    Some(Region::Alpha)
                } else {
                    None
                }
            }
        }
    }
}

/// The color of the swatch with `index` in the recent-then-standard list.
pub(super) fn swatch_color(recent: &[Color], index: usize) -> Option<Color> {
    if index < recent.len() {
        recent.get(index).copied()
    } else {
        WINDOWS_COLORS.get(index - recent.len()).copied()
    }
}

/// The saturation and value (`0.0..=1.0`) a point maps to in the square. The
/// top edge is value 1 and the left edge is saturation 0.
pub(super) fn saturation_value(rect: Rect, point: Point) -> (f32, f32) {
    let width = (rect.width - 1).max(1) as f32;
    let height = (rect.height - 1).max(1) as f32;
    (
        ((point.x - rect.x) as f32 / width).clamp(0.0, 1.0),
        1.0 - ((point.y - rect.y) as f32 / height).clamp(0.0, 1.0),
    )
}

/// The horizontal fraction (`0.0..=1.0`) a point maps to within `rect`.
pub(super) fn fraction_x(rect: Rect, point: Point) -> f32 {
    let width = (rect.width - 1).max(1) as f32;
    ((point.x - rect.x) as f32 / width).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_places_every_part_inside_the_window() {
        let layout = PickerLayout::compute(1.0);
        for rect in [
            layout.recent_label,
            layout.recent_grid,
            layout.windows_label,
            layout.windows_grid,
            layout.saturation,
            layout.hue,
            layout.alpha,
            layout.preview,
            layout.hex,
            layout.toggle,
            layout.ok,
            layout.cancel,
        ] {
            assert!(rect.x >= 0 && rect.y >= 0, "{rect:?}");
            assert!(rect.x + rect.width <= layout.size.0, "{rect:?}");
            assert!(rect.y + rect.height <= layout.size.1, "{rect:?}");
        }
        assert!(layout.recent_grid.y < layout.windows_grid.y);
        assert!(layout.windows_grid.y < layout.ok.y);
    }

    #[test]
    fn layout_scales_with_dpi() {
        let one = PickerLayout::compute(1.0);
        let two = PickerLayout::compute(2.0);
        assert_eq!(two.size, (one.size.0 * 2, one.size.1 * 2));
        assert_eq!(two.swatch, one.swatch * 2);
    }

    #[test]
    fn the_button_bar_fits_without_overlap() {
        let layout = PickerLayout::compute(1.0);
        assert!(
            layout.toggle.x + layout.toggle.width <= layout.cancel.x,
            "the toggle must not overlap Cancel"
        );
        assert!(layout.cancel.x + layout.cancel.width <= layout.ok.x);
        assert!(layout.ok.x + layout.ok.width <= layout.size.0);
        // The narrow grid is centered in the wider button bar.
        let left = layout.windows_grid.x;
        let right = layout.size.0 - (layout.windows_grid.x + layout.windows_grid.width);
        assert!((left - right).abs() <= 1);
    }

    #[test]
    fn swatch_rects_tile_the_grid() {
        let layout = PickerLayout::compute(1.0);
        let first = layout.swatch_rect(layout.windows_grid, 0);
        let next = layout.swatch_rect(layout.windows_grid, 1);
        assert_eq!(next.x, first.x + layout.swatch + layout.swatch_gap);
        assert_eq!(next.y, first.y);
        let second_row = layout.swatch_rect(layout.windows_grid, COLS);
        assert_eq!(second_row.x, first.x);
        assert_eq!(second_row.y, first.y + layout.swatch + layout.swatch_gap);
    }

    #[test]
    fn region_at_maps_the_palette_and_buttons() {
        let layout = PickerLayout::compute(1.0);
        let center = |rect: Rect| Point::new(rect.x + rect.width / 2, rect.y + rect.height / 2);
        let recent = 2;
        assert_eq!(
            layout.region_at(
                Panel::Palette,
                recent,
                center(layout.swatch_rect(layout.recent_grid, 0))
            ),
            Some(Region::Swatch(0))
        );
        assert_eq!(
            layout.region_at(
                Panel::Palette,
                recent,
                center(layout.swatch_rect(layout.windows_grid, 3))
            ),
            Some(Region::Swatch(recent + 3))
        );
        assert_eq!(
            layout.region_at(Panel::Palette, recent, center(layout.ok)),
            Some(Region::Ok)
        );
        assert_eq!(
            layout.region_at(Panel::Palette, recent, center(layout.cancel)),
            Some(Region::Cancel)
        );
        assert_eq!(
            layout.region_at(Panel::Palette, recent, center(layout.toggle)),
            Some(Region::Toggle)
        );
        assert_eq!(
            layout.region_at(Panel::Palette, recent, Point::new(0, 0)),
            None
        );
    }

    #[test]
    fn region_at_maps_the_custom_controls() {
        let layout = PickerLayout::compute(1.0);
        let center = |rect: Rect| Point::new(rect.x + rect.width / 2, rect.y + rect.height / 2);
        assert_eq!(
            layout.region_at(Panel::Custom, 0, center(layout.saturation)),
            Some(Region::Saturation)
        );
        assert_eq!(
            layout.region_at(Panel::Custom, 0, center(layout.hue)),
            Some(Region::Hue)
        );
        assert_eq!(
            layout.region_at(Panel::Custom, 0, center(layout.alpha)),
            Some(Region::Alpha)
        );
    }

    #[test]
    fn swatch_color_reads_recent_then_standard() {
        let recent = vec![Color::rgb(1, 2, 3), Color::rgb(4, 5, 6)];
        assert_eq!(swatch_color(&recent, 0), Some(Color::rgb(1, 2, 3)));
        assert_eq!(swatch_color(&recent, 1), Some(Color::rgb(4, 5, 6)));
        assert_eq!(swatch_color(&recent, 2), Some(WINDOWS_COLORS[0]));
        assert_eq!(swatch_color(&recent, 999), None);
    }

    #[test]
    fn saturation_value_maps_the_square_corners() {
        let rect = Rect::new(10, 20, 201, 101);
        assert_eq!(saturation_value(rect, Point::new(10, 20)), (0.0, 1.0));
        assert_eq!(saturation_value(rect, Point::new(210, 120)), (1.0, 0.0));
        assert_eq!(saturation_value(rect, Point::new(-50, -50)), (0.0, 1.0));
        assert_eq!(saturation_value(rect, Point::new(9999, 9999)), (1.0, 0.0));
    }

    #[test]
    fn fraction_x_maps_and_clamps() {
        let rect = Rect::new(10, 0, 101, 16);
        assert_eq!(fraction_x(rect, Point::new(10, 5)), 0.0);
        assert_eq!(fraction_x(rect, Point::new(110, 5)), 1.0);
        assert_eq!(fraction_x(rect, Point::new(60, 5)), 0.5);
        assert_eq!(fraction_x(rect, Point::new(-1, 5)), 0.0);
    }
}
