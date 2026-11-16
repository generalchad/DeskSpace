//! A contact sheet of Burn's frames, composed on the CPU as the space's
//! window and its flame overlay would show them, so a look can be judged
//! from a PNG without a person at the screen.
//!
//! Only what a frame changes is modelled: a sample space's contents through
//! the burn's paper mask, the glow, char, and scorch over them, and the
//! flames above both, on a mid-gray backdrop. The masks are stretched the way
//! the painter stretches them, bilinear between cells, so an edge looks as
//! soft here as it does on screen.

use super::burn::{self, BurnField};
use super::flames::{self, MARGIN_SIDE, MARGIN_TOP, Rows, SPACING};
use super::math::Grid;
use deskspace_core::animation::DETAIL_DEFAULT;

/// The backdrop the sheets sit on, so paper and flames both read at their
/// edges.
const BACKDROP: u32 = 0xFF80_8080;
/// The sample space: an opaque tint, a title band, and item squares, so the
/// burning edge has something to cut through.
const TINT: u32 = 0xFF20_2020;
const TITLE: u32 = 0xFF2B_2B2B;
const ITEM: u32 = 0xFF50_5A6A;
/// The title band's height, and an item square's size and the step between
/// them, in the 96-DPI pixels the sheets are composed at.
const TITLE_HEIGHT: u32 = 32;
const ITEM_SIZE: u32 = 48;
const ITEM_STEP: u32 = 75;
const ITEM_MARGIN: u32 = 12;
/// How many item squares the sample space holds, at most: a few, so the
/// burning edge and its bands stay easy to see.
const ITEM_COLUMNS: u32 = 3;
const ITEM_ROWS: u32 = 3;
/// Burn's detail and seed, so two runs of a sheet compare.
const DETAIL: u8 = DETAIL_DEFAULT;
const SEED: u32 = 1;
/// Frames per sheet, three to a row, and the animation clock's step between
/// them. The first frame is at gone 1 / (`FRAMES` + 1).
const FRAMES: usize = 9;
const FRAME_SECONDS: f32 = 0.05;
const COLUMNS: usize = 3;

/// A composed picture: `size.0 * size.1` premultiplied BGRA pixels
/// (`0xAARRGGBB`), top-down.
struct Picture {
    pixels: Vec<u32>,
    size: (u32, u32),
}

/// One frame of a burn on a space of `space` pixels at `front`, `time`
/// seconds into the play: the backdrop, the sample space masked by the
/// paper, the marks over it, and the flames over both at the overlay's
/// margins.
fn frame(field: &BurnField, space: (u32, u32), front: f32, time: f32) -> Picture {
    let grid = field.grid();
    let (mut paper, mut marks) = (vec![0; grid.cell_count()], vec![0; grid.cell_count()]);
    field.fill_rows(front, 0..grid.rows, &mut paper, &mut marks);

    let (width, height) = flames::overlay_size(space.0, space.1);
    let mut flame_pixels = vec![0; (width * height) as usize];
    let mut bases = Vec::new();
    let mut tongues = Vec::new();
    field.flame_bases(front, SPACING, &mut bases);
    flames::tongues(&bases, space.1 as f32, time, &mut tongues);
    let mut drawn = Rows::ALL;
    flames::paint(&mut flame_pixels, width, height, &tongues, time, &mut drawn);

    let sample = sample(space);
    let mut pixels = vec![BACKDROP; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let mut pixel = BACKDROP;
            let inside = x >= MARGIN_SIDE
                && y >= MARGIN_TOP
                && x - MARGIN_SIDE < space.0
                && y - MARGIN_TOP < space.1;
            if inside {
                let (sx, sy) = (x - MARGIN_SIDE, y - MARGIN_TOP);
                let paper = sample_brush(&paper, grid, burn::CELL, sx, sy);
                let under = over(
                    masked(sample[(sy * space.0 + sx) as usize], paper >> 24),
                    BACKDROP,
                );
                let mark = sample_stretched(&marks, grid, space, sx, sy);
                pixel = over(mark, under);
            }
            let index = (y * width + x) as usize;
            pixels[index] = over(flame_pixels[index], pixel);
        }
    }
    Picture {
        pixels,
        size: (width, height),
    }
}

/// The nine frames of a burn on a space of `space` pixels, three to a row:
/// gone 0.1 to 0.9, with the clock 0.05 s further on at each frame.
fn sheet(space: (u32, u32)) -> Picture {
    let (cell_width, cell_height) = flames::overlay_size(space.0, space.1);
    let rows = FRAMES.div_ceil(COLUMNS) as u32;
    let width = cell_width * COLUMNS as u32;
    let height = cell_height * rows;
    let mut pixels = vec![BACKDROP; (width * height) as usize];
    let field = BurnField::for_space((space.0 as f32, space.1 as f32), DETAIL, SEED);
    for frame_index in 0..FRAMES {
        let picture = frame(&field, space, gone(frame_index), time(frame_index));
        let left = (frame_index % COLUMNS) as u32 * cell_width;
        let top = (frame_index / COLUMNS) as u32 * cell_height;
        for y in 0..cell_height {
            let destination = ((top + y) * width + left) as usize;
            let source = (y * cell_width) as usize;
            pixels[destination..destination + cell_width as usize]
                .copy_from_slice(&picture.pixels[source..source + cell_width as usize]);
        }
    }
    Picture {
        pixels,
        size: (width, height),
    }
}

/// The sample space's pixels, `size.0` by `size.1`, opaque: its tint, a title
/// band along the top, and a few item squares under it.
fn sample(size: (u32, u32)) -> Vec<u32> {
    let (width, height) = size;
    let mut pixels = vec![TINT; (width * height) as usize];
    for y in 0..TITLE_HEIGHT.min(height) {
        pixels[(y * width) as usize..((y + 1) * width) as usize].fill(TITLE);
    }
    for row in 0..ITEM_ROWS {
        for column in 0..ITEM_COLUMNS {
            let left = ITEM_MARGIN + column * ITEM_STEP;
            let top = TITLE_HEIGHT + ITEM_MARGIN + row * ITEM_STEP;
            if left + ITEM_SIZE > width || top + ITEM_SIZE > height {
                continue;
            }
            for y in top..top + ITEM_SIZE {
                pixels[(y * width + left) as usize..(y * width + left + ITEM_SIZE) as usize]
                    .fill(ITEM);
            }
        }
    }
    pixels
}

/// The burn progress of a sheet's frame `frame_index`: 0.1 to 0.9, one step
/// per frame.
fn gone(frame_index: usize) -> f32 {
    (frame_index + 1) as f32 / (FRAMES + 1) as f32
}

/// The animation clock at `frame_index`, in seconds.
fn time(frame_index: usize) -> f32 {
    frame_index as f32 * FRAME_SECONDS
}

/// A cell grid's pixel at a space pixel, as the paper mask draws it: one mask
/// pixel per `cell`-pixel cell, sampled bilinearly. `cells` holds
/// premultiplied white, so its alpha is the covered share.
fn sample_brush(cells: &[u32], grid: Grid, cell: u32, x: u32, y: u32) -> u32 {
    let u = x as f32 / cell.max(1) as f32;
    let v = y as f32 / cell.max(1) as f32;
    bilinear(cells, grid, u, v)
}

/// A cell grid's pixel at a space pixel of `size`, as `DrawBitmap` draws the
/// marks: the whole grid stretched over the whole space, sampled bilinearly.
fn sample_stretched(cells: &[u32], grid: Grid, size: (u32, u32), x: u32, y: u32) -> u32 {
    let u = (x as f32 + 0.5) * grid.cols.max(1) as f32 / size.0.max(1) as f32;
    let v = (y as f32 + 0.5) * grid.rows.max(1) as f32 / size.1.max(1) as f32;
    bilinear(cells, grid, u, v)
}

/// A bilinear sample of `cells` at (`u`, `v`) in cell coordinates, where a
/// cell's center is at (`col` + 0.5, `row` + 0.5), as Direct2D samples a
/// bitmap. Cells outside the grid repeat the edge, as its clamped extend mode
/// does.
fn bilinear(cells: &[u32], grid: Grid, u: f32, v: f32) -> u32 {
    let at = |col: i64, row: i64| -> u32 {
        let cols = grid.cols.max(1);
        let rows = grid.rows.max(1);
        let col = col.clamp(0, i64::from(cols) - 1) as u32;
        let row = row.clamp(0, i64::from(rows) - 1) as u32;
        cells.get((row * cols + col) as usize).copied().unwrap_or(0)
    };
    let (fu, fv) = (u - 0.5, v - 0.5);
    let (col, row) = (fu.floor() as i64, fv.floor() as i64);
    let (tx, ty) = (fu - fu.floor(), fv - fv.floor());
    let top = mix_pixel(at(col, row), at(col + 1, row), tx);
    let bottom = mix_pixel(at(col, row + 1), at(col + 1, row + 1), tx);
    mix_pixel(top, bottom, ty)
}

/// `b` mixed into `a` by `t` (0 to 1), channel by channel.
fn mix_pixel(a: u32, b: u32, t: f32) -> u32 {
    let channel = |shift: u32| {
        let a = ((a >> shift) & 0xFF) as f32;
        let b = ((b >> shift) & 0xFF) as f32;
        (a + (b - a) * t).round() as u32
    };
    channel(0) | channel(8) << 8 | channel(16) << 16 | channel(24) << 24
}

/// `src` drawn over `dst`, both premultiplied: `src + dst * (1 - src.a)`.
fn over(src: u32, dst: u32) -> u32 {
    let rest = 255 - (src >> 24);
    let channel = |shift: u32| {
        let src = (src >> shift) & 0xFF;
        let dst = (dst >> shift) & 0xFF;
        (src + (dst * rest + 127) / 255).min(255)
    };
    channel(0) | channel(8) << 8 | channel(16) << 16 | channel(24) << 24
}

/// An opaque color at the `coverage` (0 to 255) an opacity mask gives it.
fn masked(color: u32, coverage: u32) -> u32 {
    let channel = |shift: u32| (((color >> shift) & 0xFF) * coverage + 127) / 255;
    channel(0) | channel(8) << 8 | channel(16) << 16 | (coverage << 24)
}

#[cfg(test)]
mod tests {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

    use super::*;

    const SPACE: (u32, u32) = (100, 100);

    fn field(space: (u32, u32)) -> BurnField {
        BurnField::for_space((space.0 as f32, space.1 as f32), DETAIL, SEED)
    }

    #[test]
    fn a_gone_frame_shows_only_the_backdrop() {
        let field = field(SPACE);
        let picture = frame(&field, SPACE, 2.0, 0.0);
        assert_eq!(picture.size, flames::overlay_size(SPACE.0, SPACE.1));
        assert!(
            picture.pixels.iter().all(|&pixel| pixel == BACKDROP),
            "paper gone and no flames: nothing but the backdrop shows"
        );
    }

    #[test]
    fn a_whole_frame_shows_the_space_inside_the_margins() {
        let field = field(SPACE);
        // Below every cell's value: every cell is untouched paper, and
        // nothing burns.
        let picture = frame(&field, SPACE, -1.0, 0.0);
        let sample = sample(SPACE);
        let (width, height) = picture.size;
        for y in 0..height {
            for x in 0..width {
                let pixel = picture.pixels[(y * width + x) as usize];
                let inside = x >= MARGIN_SIDE
                    && y >= MARGIN_TOP
                    && x - MARGIN_SIDE < SPACE.0
                    && y - MARGIN_TOP < SPACE.1;
                let expected = if inside {
                    sample[((y - MARGIN_TOP) * SPACE.0 + x - MARGIN_SIDE) as usize]
                } else {
                    BACKDROP
                };
                assert_eq!(pixel, expected, "at ({x}, {y})");
            }
        }
    }

    #[test]
    fn the_paper_mask_fades_across_a_cell_edge() {
        let grid = Grid { cols: 2, rows: 1 };
        let cells = [u32::MAX, 0];
        assert_eq!(sample_brush(&cells, grid, burn::CELL, 0, 0), u32::MAX);
        let seam = sample_brush(&cells, grid, burn::CELL, burn::CELL, 0) >> 24;
        assert!(
            (127..=128).contains(&seam),
            "half covered at the seam: {seam}"
        );
    }

    #[test]
    fn a_mark_lies_over_what_is_under_it() {
        let red = 0xFFFF_0000;
        assert_eq!(over(red, BACKDROP), red, "an opaque mark hides the rest");
        let lightened = over(0x8080_8080, BACKDROP);
        assert!(
            lightened & 0xFF > BACKDROP & 0xFF,
            "a half-white mark lightens"
        );
        assert_eq!(lightened >> 24, 255, "the backdrop stays opaque");
    }

    #[test]
    fn the_sheet_holds_three_frames_to_a_row() {
        let space = (60, 40);
        let (cell_width, cell_height) = flames::overlay_size(space.0, space.1);
        let picture = sheet(space);
        assert_eq!(picture.size, (cell_width * 3, cell_height * 3));
        assert_eq!(
            picture.pixels.len(),
            (picture.size.0 * picture.size.1) as usize
        );
    }

    /// Writes one sheet per case to the temp folder, for judging Burn's look
    /// before and after a change. Run it by hand:
    /// `cargo test -p deskspace-platform burn_sheet -- --ignored`.
    /// The 144-DPI case joins it once Burn scales with the display's DPI.
    #[test]
    #[ignore = "writes contact sheets; run by hand"]
    fn burn_sheet() {
        // SAFETY: COM on this test thread, as the other offscreen tests do.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        for (name, space) in [("320x240", (320, 240)), ("1200x900", (1200, 900))] {
            let picture = sheet(space);
            let path = std::env::temp_dir().join(format!("deskspace-burn-{name}.png"));
            crate::render::d2d::png::write_pixels(
                &picture.pixels,
                picture.size.0,
                picture.size.1,
                &path,
            )
            .unwrap_or_else(|error| panic!("could not write {}: {error}", path.display()));
            println!("{}", path.display());
        }
    }
}
