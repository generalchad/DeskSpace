//! Ash flakes lifting and tumbling off Burn's glowing edge.
//!
//! Flakes are deterministic functions of `(field, front, time, scale)`, so
//! reversing a burn mid-way lowers them back into the paper along the same
//! path. Painted onto the flame overlay after flames using source-over blending.

use std::f32::consts::TAU;

use super::burn::{self, BurnField};
use super::flames::{self, Rows};
use super::noise;

/// Field units after the front passes a flake cell during which its flake
/// stays alive before fading out completely.
const ASH_LIFE: f32 = 0.12;
/// How far an ash flake rises in 96-DPI pixels over its lifetime.
const ASH_RISE_PX: f32 = 90.0;
const DRIFT_AMP_PX: f32 = 10.0;
const DRIFT_RATE: f32 = 1.7;
const MAX_SPIN_RAD: f32 = 4.0;
const RIM_LIFE_MAX: f32 = 0.25;
const RIM_HEAT: f32 = 0.6;
const PHASE_SALT: u32 = 0x51A5_7001;
const SPIN_SALT: u32 = 0x51A5_7002;

/// One ash flake in overlay pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flake {
    pub x: f32,
    pub y: f32,
    pub rx: f32,
    pub ry: f32,
    pub body: u32,
    pub rim: Option<u32>,
    pub(crate) rim_px: f32,
}

/// Computes every active ash flake for `field` at `front` and `time`
/// (seconds) at display `scale`, writing them into `out`.
pub fn flakes(field: &BurnField, front: f32, time: f32, scale: f32, out: &mut Vec<Flake>) {
    out.clear();
    let values = field.values();
    let cols = field.grid().cols.max(1);
    let (side, top) = flames::margins(field.dpi());
    let cell = burn::cell_px(field.dpi()) as f32;
    let seed = field.seed();
    let rim_px = (1.0 * scale).max(1.0);

    for &index in field.flake_cells() {
        let Some(&value) = values.get(index as usize) else {
            continue;
        };
        let delta = front - value;
        if delta <= 0.0 || delta >= ASH_LIFE {
            continue;
        }
        let life = delta / ASH_LIFE;
        let col = index % cols;
        let row = index / cols;
        let base_x = side as f32 + (col as f32 + 0.5) * cell;
        let base_y = top as f32 + (row as f32 + 0.5) * cell;

        let phase = noise::random(seed ^ PHASE_SALT, index) * TAU;
        let spin = (noise::random(seed ^ SPIN_SALT, index) * 2.0 - 1.0) * MAX_SPIN_RAD;
        let angle = time * spin + phase;

        let y = base_y - life * ASH_RISE_PX * scale;
        let x = base_x + (time * DRIFT_RATE + phase).sin() * DRIFT_AMP_PX * scale * life;
        let height_px = (3.0 * scale).max(0.5);
        let width_px = 3.0 * scale * angle.cos().abs() + 0.5;
        let rx = width_px * 0.5;
        let ry = height_px * 0.5;

        let alpha = ((1.0 - life).powf(0.7) * 230.0).round().clamp(0.0, 230.0) as u8;
        let body = premultiplied((22, 18, 16), alpha);
        let rim = (life < RIM_LIFE_MAX).then(|| flames::fire(RIM_HEAT));

        out.push(Flake {
            x,
            y,
            rx,
            ry,
            body,
            rim,
            rim_px,
        });
    }
}

/// Paints `flakes` onto `pixels` (`size.0` by `size.1`, premultiplied BGRA)
/// using source-over blending, skipping any flake outside the buffer, and
/// returns the row run written.
pub fn paint(flakes: &[Flake], size: (u32, u32), pixels: &mut [u32]) -> Rows {
    let (width, height) = size;
    if width == 0 || height == 0 || pixels.len() < (width * height) as usize {
        return Rows::NONE;
    }
    let mut top_row = u32::MAX;
    let mut bottom_row = 0u32;

    for flake in flakes {
        let extra = if flake.rim.is_some() {
            flake.rim_px
        } else {
            0.0
        };
        let rx_eff = flake.rx.max(0.5);
        let ry_eff = flake.ry.max(0.5);
        if flake.x + rx_eff + extra < 0.0
            || flake.x - rx_eff - extra >= width as f32
            || flake.y + ry_eff + extra < 0.0
            || flake.y - ry_eff - extra >= height as f32
        {
            continue;
        }
        let min_x = ((flake.x - rx_eff - extra).floor() as i32).max(0) as u32;
        let max_x = ((flake.x + rx_eff + extra).ceil() as i32).min(width as i32) as u32;
        let min_y = ((flake.y - ry_eff - extra).floor() as i32).max(0) as u32;
        let max_y = ((flake.y + ry_eff + extra).ceil() as i32).min(height as i32) as u32;
        if min_x >= max_x || min_y >= max_y {
            continue;
        }

        // Sub-pixel edge-on width attenuates body alpha instead of vanishing
        // between integer pixel centers.
        let coverage = (flake.rx / rx_eff).min(1.0);
        let body = scale_premul(flake.body, coverage);
        let inv_rx2 = 1.0 / (rx_eff * rx_eff);
        let inv_ry2 = 1.0 / (ry_eff * ry_eff);
        let rx_out = rx_eff + flake.rim_px;
        let ry_out = ry_eff + flake.rim_px;
        let inv_rx_out2 = 1.0 / (rx_out * rx_out);
        let inv_ry_out2 = 1.0 / (ry_out * ry_out);

        for py in min_y..max_y {
            let dy = py as f32 + 0.5 - flake.y;
            let dy_in2 = dy * dy * inv_ry2;
            let dy_out2 = dy * dy * inv_ry_out2;
            let row_start = (py * width) as usize;
            let mut row_hit = false;
            for px in min_x..max_x {
                let dx = px as f32 + 0.5 - flake.x;
                let dx2 = dx * dx;
                let src = if dx2 * inv_rx2 + dy_in2 <= 1.0 {
                    body
                } else if let Some(rim) = flake.rim
                    && dx2 * inv_rx_out2 + dy_out2 <= 1.0
                {
                    rim
                } else {
                    continue;
                };
                if src == 0 {
                    continue;
                }
                let idx = row_start + px as usize;
                pixels[idx] = over(src, pixels[idx]);
                row_hit = true;
            }
            if row_hit {
                top_row = top_row.min(py);
                bottom_row = bottom_row.max(py + 1);
            }
        }
    }

    if top_row >= bottom_row {
        Rows::NONE
    } else {
        Rows::new(top_row, bottom_row)
    }
}

fn premultiplied((r, g, b): (u8, u8, u8), alpha: u8) -> u32 {
    let scale = |channel: u8| (u32::from(channel) * u32::from(alpha) + 127) / 255;
    (u32::from(alpha) << 24) | (scale(r) << 16) | (scale(g) << 8) | scale(b)
}

fn scale_premul(px: u32, factor: f32) -> u32 {
    if factor >= 0.999 {
        return px;
    }
    let scale = |shift: u32| ((((px >> shift) & 0xff) as f32) * factor).round() as u32;
    (scale(24) << 24) | (scale(16) << 16) | (scale(8) << 8) | scale(0)
}

fn over(src: u32, dst: u32) -> u32 {
    let rest = 255 - (src >> 24);
    let channel = |shift: u32| {
        let s = (src >> shift) & 0xff;
        let d = (dst >> shift) & 0xff;
        s + (d * rest + 127) / 255
    };
    (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::animation::DETAIL_DEFAULT;

    #[test]
    fn no_flakes_appear_before_the_burn_begins() {
        let field = BurnField::for_space((400.0, 300.0), DETAIL_DEFAULT, 1, 96);
        let mut out = Vec::new();
        flakes(&field, field.front(0.0), 0.2, 1.0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn active_flakes_rise_with_front_and_match_deterministically() {
        let field = BurnField::for_space((400.0, 300.0), DETAIL_DEFAULT, 7, 96);
        let mut first = Vec::new();
        let mut second = Vec::new();
        let mut later = Vec::new();
        let f1 = field.front(0.5);
        let f2 = f1 + 0.01;
        flakes(&field, f1, 0.4, 1.0, &mut first);
        flakes(&field, f1, 0.4, 1.0, &mut second);
        assert_eq!(first, second, "same inputs must yield identical flakes");
        assert!(!first.is_empty(), "mid-burn should lift at least one flake");

        let values = field.values();
        let active_at = |front: f32| -> Vec<u32> {
            field
                .flake_cells()
                .iter()
                .copied()
                .filter(|&idx| {
                    let d = front - values[idx as usize];
                    d > 0.0 && d < ASH_LIFE
                })
                .collect()
        };
        let ids_1 = active_at(f1);
        let ids_2 = active_at(f2);
        flakes(&field, f2, 0.4, 1.0, &mut later);
        assert_eq!(ids_1.len(), first.len());
        assert_eq!(ids_2.len(), later.len());

        let (pos_1, id) = ids_1
            .iter()
            .enumerate()
            .find(|&(_, id)| ids_2.contains(id))
            .expect("at least one flake survives a 0.01 front advance");
        let pos_2 = ids_2.iter().position(|other| other == id).unwrap();
        assert!(
            later[pos_2].y < first[pos_1].y,
            "flake {id} must rise (smaller y) when front advances: {} -> {}",
            first[pos_1].y,
            later[pos_2].y
        );

        let mut rotated = Vec::new();
        flakes(&field, f1, 0.9, 1.0, &mut rotated);
        assert!(
            rotated.iter().zip(&first).any(|(a, b)| a.rx != b.rx),
            "flakes tumble so width changes with time"
        );
    }

    #[test]
    fn paint_keeps_pixels_premultiplied_and_stays_inside_buffer() {
        let size = (40u32, 30u32);
        let mut pixels = vec![0u32; (size.0 * size.1) as usize];
        let mut copy = vec![premultiplied((20, 40, 80), 120); pixels.len()];
        let sample_flakes = [
            Flake {
                x: 15.0,
                y: 12.0,
                rx: 1.5,
                ry: 1.5,
                body: premultiplied((22, 18, 16), 210),
                rim: Some(flames::fire(0.6)),
                rim_px: 1.0,
            },
            Flake {
                x: -10.0,
                y: 12.0,
                rx: 1.5,
                ry: 1.5,
                body: premultiplied((22, 18, 16), 210),
                rim: None,
                rim_px: 1.0,
            },
            Flake {
                x: 55.0,
                y: 12.0,
                rx: 1.5,
                ry: 1.5,
                body: premultiplied((22, 18, 16), 210),
                rim: None,
                rim_px: 1.0,
            },
        ];

        let rows_blank = paint(&sample_flakes, size, &mut pixels);
        assert!(!rows_blank.is_empty());
        assert!(rows_blank.top() >= 9 && rows_blank.bottom() <= 16);
        assert!(pixels.iter().all(|&px| {
            let a = px >> 24;
            ((px >> 16) & 0xff) <= a && ((px >> 8) & 0xff) <= a && (px & 0xff) <= a
        }));

        paint(&sample_flakes, size, &mut copy);
        assert!(copy.iter().all(|&px| {
            let a = px >> 24;
            ((px >> 16) & 0xff) <= a && ((px >> 8) & 0xff) <= a && (px & 0xff) <= a
        }));
    }
}
