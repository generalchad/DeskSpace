//! The color picker (ROADMAP F-51): what it shows and where each part sits.
//! Pure, so the layout and hit test are unit-tested; `win32/color_picker`
//! runs the window and its input, and `d2d::ColorPickerPainter` paints this.

use deskspace_core::geometry::{Point, Rect};
use deskspace_core::theme::{Color, Hsv};

use crate::edit::TextEdit;

/// A notation the value box reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueFormat {
    Hex,
    Rgb,
    Hsl,
    Hsb,
}

impl ValueFormat {
    /// In the order the radio buttons show them.
    pub(crate) const ALL: [ValueFormat; 4] = [Self::Hex, Self::Rgb, Self::Hsl, Self::Hsb];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::Rgb => "RGB",
            Self::Hsl => "HSL",
            Self::Hsb => "HSB",
        }
    }
}

/// One of the two swatch grids under Presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grid {
    Recent,
    Palette,
}

/// The default palette, two rows of vivid hues.
pub(crate) const PALETTE: [Color; GRID_COLUMNS * GRID_ROWS] = [
    Color::rgb(0xEF, 0x44, 0x44),
    Color::rgb(0xF9, 0x73, 0x16),
    Color::rgb(0xFA, 0xCC, 0x15),
    Color::rgb(0x22, 0xC5, 0x5E),
    Color::rgb(0x38, 0xBD, 0xF8),
    Color::rgb(0x81, 0x8C, 0xF8),
    Color::rgb(0xF4, 0x72, 0xB6),
    Color::rgb(0xF5, 0x9E, 0x0B),
    Color::rgb(0x84, 0xCC, 0x16),
    Color::rgb(0x14, 0xB8, 0xA6),
    Color::rgb(0x3B, 0x82, 0xF6),
    Color::rgb(0x63, 0x66, 0xF1),
    Color::rgb(0xA8, 0x55, 0xF7),
    Color::rgb(0xEC, 0x48, 0x99),
];

pub(crate) const GRID_COLUMNS: usize = 7;
pub(crate) const GRID_ROWS: usize = 2;

/// Which interactive part of the picker a point falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Region {
    /// The title strip, which drags the window.
    Title,
    Close,
    Eyedropper,
    Swatch(Grid, usize),
    Saturation,
    Hue,
    Alpha,
    Format(ValueFormat),
    Field,
    Copy,
    /// The "Current" half of the comparison, which goes back to the color
    /// the picker opened with.
    Current,
    Cancel,
    Apply,
}

impl Region {
    /// Whether dragging over this region keeps setting a value.
    pub(crate) fn is_value(self) -> bool {
        matches!(self, Region::Saturation | Region::Hue | Region::Alpha)
    }

    /// Whether the region is a button that shows hover and acts on release.
    pub(crate) fn is_button(self) -> bool {
        matches!(
            self,
            Region::Close
                | Region::Eyedropper
                | Region::Swatch(..)
                | Region::Format(_)
                | Region::Copy
                | Region::Current
                | Region::Cancel
                | Region::Apply
        )
    }
}

/// What the line under the value box says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldStatus {
    None,
    /// The typed text is not a color; the message says what is expected.
    Invalid(&'static str),
    /// The copy button put the value on the clipboard.
    Copied,
}

/// Everything the painter draws, copied from the picker's state so painting
/// holds no borrow of it.
#[derive(Debug, Clone)]
pub(crate) struct PickerView {
    pub hsv: Hsv,
    pub alpha: u8,
    /// The color the picker opened with.
    pub initial: Color,
    pub recent: Vec<Color>,
    pub format: ValueFormat,
    pub field: TextEdit,
    pub field_focused: bool,
    pub caret_visible: bool,
    pub status: FieldStatus,
    pub hover: Option<Region>,
    /// The eyedropper is sampling the screen.
    pub sampling: bool,
    pub dark: bool,
    /// DWM draws acrylic behind the window, so the body is translucent.
    pub acrylic: bool,
    /// Fills the Apply button and the focus and selection rings.
    pub accent: Color,
}

impl PickerView {
    /// The color Apply would return.
    pub(crate) fn color(&self) -> Color {
        self.hsv.to_color(self.alpha)
    }
}

// The layout in 96-DPI pixels; the window scales it for its monitor.
const PAD: i32 = 20;
const WIDTH: i32 = 560;
const TITLE_TOP: i32 = 16;
const TITLE_HEIGHT: i32 = 28;
const CONTENT_TOP: i32 = 60;
const LEFT_WIDTH: i32 = 264;
const COLUMN_GAP: i32 = 24;
const SQUARE_HEIGHT: i32 = 236;
const TRACK_HEIGHT: i32 = 12;
/// Extra height above and below a slider track that still grabs it.
const TRACK_SLOP: i32 = 6;
const LABEL_HEIGHT: i32 = 16;
const ICON_BUTTON: i32 = 28;
pub(crate) const SWATCH: i32 = 28;
const SWATCH_GAP: i32 = 6;
const RADIO_HEIGHT: i32 = 20;
const FIELD_HEIGHT: i32 = 32;
const FOOTER_HEIGHT: i32 = 40;
const BUTTON_WIDTH: i32 = 100;
const BUTTON_GAP: i32 = 10;

/// Where each part of the picker is, in 96-DPI pixels from its top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PickerLayout {
    pub size: (i32, i32),
    pub title: Rect,
    pub close: Rect,
    pub saturation: Rect,
    pub hue: Rect,
    pub alpha_label: Rect,
    pub alpha: Rect,
    pub presets_label: Rect,
    pub eyedropper_label: Rect,
    pub eyedropper: Rect,
    pub recent_label: Rect,
    pub recent_grid: Rect,
    pub palette_label: Rect,
    pub palette_grid: Rect,
    /// The row of format radio buttons.
    pub formats: Rect,
    pub field: Rect,
    pub copy: Rect,
    pub status: Rect,
    /// The New / Current comparison.
    pub compare: Rect,
    pub cancel: Rect,
    pub apply: Rect,
}

impl PickerLayout {
    pub(crate) fn new() -> Self {
        let right_x = PAD + LEFT_WIDTH + COLUMN_GAP;
        let grid_height = GRID_ROWS as i32 * SWATCH + (GRID_ROWS as i32 - 1) * SWATCH_GAP;
        let right_width = GRID_COLUMNS as i32 * SWATCH + (GRID_COLUMNS as i32 - 1) * SWATCH_GAP;
        let width = right_x + right_width + PAD;
        debug_assert_eq!(width, WIDTH);

        let title = Rect::new(PAD, TITLE_TOP, width - PAD * 2 - ICON_BUTTON, TITLE_HEIGHT);
        let close = Rect::new(
            width - PAD - ICON_BUTTON,
            TITLE_TOP,
            ICON_BUTTON,
            ICON_BUTTON,
        );

        // The right column, top to bottom.
        let presets_label = Rect::new(right_x, CONTENT_TOP, right_width / 2, ICON_BUTTON);
        let eyedropper = Rect::new(
            right_x + right_width - ICON_BUTTON,
            CONTENT_TOP,
            ICON_BUTTON,
            ICON_BUTTON,
        );
        let eyedropper_label = Rect::new(
            right_x + right_width / 2,
            CONTENT_TOP,
            right_width / 2 - ICON_BUTTON - 8,
            ICON_BUTTON,
        );
        let recent_label = Rect::new(
            right_x,
            presets_label.bottom() + 10,
            right_width,
            LABEL_HEIGHT,
        );
        let recent_grid = Rect::new(right_x, recent_label.bottom() + 6, right_width, grid_height);
        let palette_label = Rect::new(
            right_x,
            recent_grid.bottom() + 12,
            right_width,
            LABEL_HEIGHT,
        );
        let palette_grid = Rect::new(
            right_x,
            palette_label.bottom() + 6,
            right_width,
            grid_height,
        );
        let formats = Rect::new(
            right_x,
            palette_grid.bottom() + 16,
            right_width,
            RADIO_HEIGHT,
        );
        let field = Rect::new(right_x, formats.bottom() + 8, right_width, FIELD_HEIGHT);
        let copy_inset = (FIELD_HEIGHT - ICON_BUTTON) / 2;
        let copy = Rect::new(
            field.right() - copy_inset - ICON_BUTTON,
            field.y + copy_inset,
            ICON_BUTTON,
            ICON_BUTTON,
        );
        let status = Rect::new(right_x, field.bottom() + 4, right_width, LABEL_HEIGHT);

        // The footer sits under the taller column; the left column's
        // square takes whatever height the sliders leave it.
        let footer_y = status.bottom() + 14;
        let alpha = Rect::new(PAD, footer_y - 18 - TRACK_HEIGHT, LEFT_WIDTH, TRACK_HEIGHT);
        let alpha_label = Rect::new(PAD, alpha.y - 6 - LABEL_HEIGHT, LEFT_WIDTH, LABEL_HEIGHT);
        let hue = Rect::new(
            PAD,
            alpha_label.y - 12 - TRACK_HEIGHT,
            LEFT_WIDTH,
            TRACK_HEIGHT,
        );
        let saturation = Rect::new(PAD, CONTENT_TOP, LEFT_WIDTH, hue.y - 16 - CONTENT_TOP);
        debug_assert_eq!(saturation.height, SQUARE_HEIGHT);

        let compare = Rect::new(PAD, footer_y, LEFT_WIDTH, FOOTER_HEIGHT);
        let apply = Rect::new(
            width - PAD - BUTTON_WIDTH,
            footer_y,
            BUTTON_WIDTH,
            FOOTER_HEIGHT,
        );
        let cancel = Rect::new(
            apply.x - BUTTON_GAP - BUTTON_WIDTH,
            footer_y,
            BUTTON_WIDTH,
            FOOTER_HEIGHT,
        );

        Self {
            size: (width, footer_y + FOOTER_HEIGHT + PAD),
            title,
            close,
            saturation,
            hue,
            alpha_label,
            alpha,
            presets_label,
            eyedropper_label,
            eyedropper,
            recent_label,
            recent_grid,
            palette_label,
            palette_grid,
            formats,
            field,
            copy,
            status,
            compare,
            cancel,
            apply,
        }
    }

    pub(crate) fn grid(&self, grid: Grid) -> Rect {
        match grid {
            Grid::Recent => self.recent_grid,
            Grid::Palette => self.palette_grid,
        }
    }

    /// The rectangle of swatch `index` in `grid`.
    pub(crate) fn swatch_rect(&self, grid: Grid, index: usize) -> Rect {
        let origin = self.grid(grid);
        let column = (index % GRID_COLUMNS) as i32;
        let row = (index / GRID_COLUMNS) as i32;
        Rect::new(
            origin.x + column * (SWATCH + SWATCH_GAP),
            origin.y + row * (SWATCH + SWATCH_GAP),
            SWATCH,
            SWATCH,
        )
    }

    /// The radio button for `format`, its circle and label together.
    pub(crate) fn format_rect(&self, format: ValueFormat) -> Rect {
        let count = ValueFormat::ALL.len() as i32;
        let index = ValueFormat::ALL
            .iter()
            .position(|candidate| *candidate == format)
            .unwrap_or(0) as i32;
        let width = self.formats.width / count;
        Rect::new(
            self.formats.x + index * width,
            self.formats.y,
            width,
            self.formats.height,
        )
    }

    /// The "New" and "Current" halves of the comparison.
    pub(crate) fn compare_halves(&self) -> (Rect, Rect) {
        let half = self.compare.width / 2;
        (
            Rect::new(self.compare.x, self.compare.y, half, self.compare.height),
            Rect::new(
                self.compare.x + half,
                self.compare.y,
                self.compare.width - half,
                self.compare.height,
            ),
        )
    }

    /// The part under `point`, given how many recent colors are shown.
    pub(crate) fn region_at(&self, point: Point, recent: usize) -> Option<Region> {
        let grab = |track: Rect| {
            Rect::new(
                track.x,
                track.y - TRACK_SLOP,
                track.width,
                track.height + TRACK_SLOP * 2,
            )
        };
        let fixed = [
            (self.close, Region::Close),
            (self.eyedropper, Region::Eyedropper),
            (self.saturation, Region::Saturation),
            (grab(self.hue), Region::Hue),
            (grab(self.alpha), Region::Alpha),
            (self.copy, Region::Copy),
            (self.field, Region::Field),
            (self.compare_halves().1, Region::Current),
            (self.cancel, Region::Cancel),
            (self.apply, Region::Apply),
        ];
        if let Some((_, region)) = fixed.iter().find(|(rect, _)| rect.contains(point)) {
            return Some(*region);
        }
        if let Some(format) = ValueFormat::ALL
            .into_iter()
            .find(|format| self.format_rect(*format).contains(point))
        {
            return Some(Region::Format(format));
        }
        let shown = |grid: Grid| match grid {
            Grid::Recent => recent.min(GRID_COLUMNS * GRID_ROWS),
            Grid::Palette => PALETTE.len(),
        };
        for grid in [Grid::Recent, Grid::Palette] {
            if let Some(index) =
                (0..shown(grid)).find(|index| self.swatch_rect(grid, *index).contains(point))
            {
                return Some(Region::Swatch(grid, index));
            }
        }
        (point.y < CONTENT_TOP).then_some(Region::Title)
    }
}

/// The horizontal fraction (`0.0..=1.0`) `x` falls at across `rect`.
pub(crate) fn fraction_x(rect: Rect, x: f32) -> f32 {
    ((x - rect.x as f32) / rect.width.max(1) as f32).clamp(0.0, 1.0)
}

/// The saturation and value (`0.0..=1.0`) a point maps to in the square:
/// saturation grows to the right and value toward the top.
pub(crate) fn saturation_value(rect: Rect, x: f32, y: f32) -> (f32, f32) {
    let down = ((y - rect.y as f32) / rect.height.max(1) as f32).clamp(0.0, 1.0);
    (fraction_x(rect, x), 1.0 - down)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center(rect: Rect) -> Point {
        Point::new(rect.x + rect.width / 2, rect.y + rect.height / 2)
    }

    #[test]
    fn every_part_fits_in_the_window() {
        let layout = PickerLayout::new();
        for rect in [
            layout.title,
            layout.close,
            layout.saturation,
            layout.hue,
            layout.alpha_label,
            layout.alpha,
            layout.presets_label,
            layout.eyedropper_label,
            layout.eyedropper,
            layout.recent_grid,
            layout.palette_grid,
            layout.formats,
            layout.field,
            layout.copy,
            layout.status,
            layout.compare,
            layout.cancel,
            layout.apply,
        ] {
            assert!(rect.x >= 0 && rect.y >= 0, "{rect:?}");
            assert!(rect.width > 0 && rect.height > 0, "{rect:?}");
            assert!(rect.right() <= layout.size.0, "{rect:?}");
            assert!(rect.bottom() <= layout.size.1, "{rect:?}");
        }
    }

    #[test]
    fn the_columns_and_footer_do_not_overlap() {
        let layout = PickerLayout::new();
        assert!(layout.saturation.right() < layout.field.x);
        assert!(layout.status.bottom() < layout.apply.y);
        assert!(layout.alpha.bottom() < layout.compare.y);
        assert!(layout.compare.right() < layout.cancel.x);
        assert!(layout.cancel.right() < layout.apply.x);
        assert!(layout.copy.x > layout.field.x && layout.copy.right() <= layout.field.right());
    }

    #[test]
    fn swatches_tile_both_grids() {
        let layout = PickerLayout::new();
        for grid in [Grid::Recent, Grid::Palette] {
            let first = layout.swatch_rect(grid, 0);
            let last = layout.swatch_rect(grid, GRID_COLUMNS * GRID_ROWS - 1);
            assert_eq!(
                (first.x, first.y),
                (layout.grid(grid).x, layout.grid(grid).y)
            );
            assert_eq!(last.right(), layout.grid(grid).right());
            assert_eq!(last.bottom(), layout.grid(grid).bottom());
        }
    }

    #[test]
    fn region_at_finds_each_part() {
        let layout = PickerLayout::new();
        let cases = [
            (layout.close, Region::Close),
            (layout.eyedropper, Region::Eyedropper),
            (layout.saturation, Region::Saturation),
            (layout.hue, Region::Hue),
            (layout.alpha, Region::Alpha),
            (layout.copy, Region::Copy),
            (layout.field, Region::Field),
            (layout.compare_halves().1, Region::Current),
            (layout.cancel, Region::Cancel),
            (layout.apply, Region::Apply),
            (layout.title, Region::Title),
            (
                layout.swatch_rect(Grid::Recent, 1),
                Region::Swatch(Grid::Recent, 1),
            ),
            (
                layout.swatch_rect(Grid::Palette, 9),
                Region::Swatch(Grid::Palette, 9),
            ),
        ];
        for (rect, region) in cases {
            assert_eq!(layout.region_at(center(rect), 3), Some(region), "{rect:?}");
        }
        for format in ValueFormat::ALL {
            assert_eq!(
                layout.region_at(center(layout.format_rect(format)), 0),
                Some(Region::Format(format))
            );
        }
    }

    #[test]
    fn empty_recent_slots_and_the_new_half_are_not_buttons() {
        let layout = PickerLayout::new();
        assert_eq!(
            layout.region_at(center(layout.swatch_rect(Grid::Recent, 3)), 3),
            None
        );
        assert_eq!(layout.region_at(center(layout.compare_halves().0), 0), None);
    }

    #[test]
    fn slider_tracks_grab_a_little_above_and_below() {
        let layout = PickerLayout::new();
        let above = Point::new(layout.hue.x + 5, layout.hue.y - TRACK_SLOP + 1);
        assert_eq!(layout.region_at(above, 0), Some(Region::Hue));
    }

    #[test]
    fn the_square_maps_its_corners_and_clamps() {
        let rect = Rect::new(10, 20, 200, 100);
        assert_eq!(saturation_value(rect, 10.0, 20.0), (0.0, 1.0));
        assert_eq!(saturation_value(rect, 210.0, 120.0), (1.0, 0.0));
        assert_eq!(saturation_value(rect, -5.0, 999.0), (0.0, 0.0));
        assert_eq!(fraction_x(rect, 110.0), 0.5);
    }
}
