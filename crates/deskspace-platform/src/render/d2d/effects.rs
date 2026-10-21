//! The device resources a `TransitionFrame` is drawn with (ROADMAP D12): a
//! layer, the gradient, noise, and burn masks, and the offscreen target
//! Pixelate draws small. Each is made the first time a frame needs it and kept, so a
//! frame only updates them; they go with the device.

use std::mem::ManuallyDrop;

use deskspace_core::animation::Edge;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_U, D2D_SIZE_F, D2D1_GRADIENT_STOP};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES, D2D1_BITMAP_INTERPOLATION_MODE,
    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
    D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, ID2D1BitmapBrush, ID2D1BitmapRenderTarget, ID2D1Brush,
    ID2D1Layer, ID2D1LinearGradientBrush,
};

use super::*;
use crate::render::burn::{self, BurnField};
use crate::render::smoke::{self, SmokeField};
use crate::render::transition::Mask;

/// Dissolve's grain, in pixels per step of detail.
const NOISE_CELL_PER_DETAIL: u32 = 2;

#[derive(Default)]
pub(super) struct Effects {
    layer: Option<ID2D1Layer>,
    gradient: Option<ID2D1LinearGradientBrush>,
    noise: Option<NoiseMask>,
    burn: Option<BurnMask>,
    smoke: Option<SmokeMask>,
    pixels: Option<PixelTarget>,
}

/// A mask of grain-sized cells, one pixel per cell, stretched over the space
/// by its brush's transform.
struct NoiseMask {
    cols: u32,
    rows: u32,
    cell: u32,
    /// Each cell's fixed noise value.
    values: Vec<u8>,
    /// The cells still shown.
    shown: CellMask,
}

/// Burn's paper: one pixel per `burn::CELL`-sized cell, drawn smoothly.
struct BurnMask {
    field: BurnField,
    detail: u8,
    /// Where the paper still shows: the contents' opacity mask.
    paper: CellMask,
    /// The glowing edge, the char, and the scorch, laid over the paper.
    marks: CellMask,
}

/// Smoke's paper: one pixel per `smoke::CELL`-sized cell, drawn smoothly, so
/// the window fades where its plumes pass.
struct SmokeMask {
    field: SmokeField,
    detail: u8,
    seed: u32,
    paper: CellMask,
}

/// One pixel per cell, rewritten each frame and stretched over the space by
/// its brush.
struct CellMask {
    /// The mask's pixels for the current frame, written in place.
    pixels: Vec<u32>,
    bitmap: ID2D1Bitmap,
    brush: ID2D1BitmapBrush,
}

struct PixelTarget {
    target: ID2D1BitmapRenderTarget,
    size: (u32, u32),
}

impl Effects {
    pub(super) fn release(&mut self) {
        *self = Self::default();
    }

    /// The layer every transition draws through.
    pub(super) fn layer(
        &mut self,
        target: &ID2D1RenderTarget,
    ) -> Result<ID2D1Layer, PlatformError> {
        if let Some(layer) = &self.layer {
            return Ok(layer.clone());
        }
        // SAFETY: a live render target; `None` lets the layer size itself.
        let layer = unsafe { target.CreateLayer(None)? };
        self.layer = Some(layer.clone());
        Ok(layer)
    }

    /// The opacity brush for `mask` over a space of `size`, updated for this
    /// frame; `None` for no mask.
    pub(super) fn mask_brush(
        &mut self,
        target: &ID2D1RenderTarget,
        mask: Mask,
        size: (f32, f32),
    ) -> Result<Option<ID2D1Brush>, PlatformError> {
        match mask {
            Mask::None => Ok(None),
            Mask::Gradient {
                edge,
                position,
                feather,
            } => {
                let brush = self.gradient(target)?;
                let (start, end) = gradient_points(edge, position, feather, size);
                // SAFETY: a live brush owned by this painter.
                unsafe {
                    brush.SetStartPoint(Vector2::new(start.0, start.1));
                    brush.SetEndPoint(Vector2::new(end.0, end.1));
                }
                Ok(Some(brush.into()))
            }
            Mask::Noise {
                threshold, grain, ..
            } => {
                let noise = self.noise(target, grain, size)?;
                fill_noise_mask(&noise.values, threshold, &mut noise.shown.pixels);
                noise.shown.upload(noise.cols, noise.rows)?;
                Ok(Some(noise.shown.brush.clone().into()))
            }
            Mask::Burn { front, detail } => {
                let burn = self.burn(target, detail, size)?;
                fill_burn(
                    burn.field.values(),
                    front,
                    &mut burn.paper.pixels,
                    &mut burn.marks.pixels,
                );
                let (cols, rows) = (burn.field.cols(), burn.field.rows());
                burn.paper.upload(cols, rows)?;
                burn.marks.upload(cols, rows)?;
                Ok(Some(burn.paper.brush.clone().into()))
            }
            Mask::Smoke {
                front,
                detail,
                seed,
            } => {
                let smoke = self.smoke(target, detail, seed, size)?;
                fill_smoke(smoke.field.values(), front, &mut smoke.paper.pixels);
                let (cols, rows) = (smoke.field.cols(), smoke.field.rows());
                smoke.paper.upload(cols, rows)?;
                Ok(Some(smoke.paper.brush.clone().into()))
            }
        }
    }

    /// The color Burn lays over the paper this frame (`render::burn`), for a
    /// space of `size`; `None` for any other mask. Call after `mask_brush`
    /// for the same frame, which writes it.
    pub(super) fn burn_marks(&self, mask: Mask) -> Option<ID2D1Bitmap> {
        let Mask::Burn { .. } = mask else {
            return None;
        };
        self.burn.as_ref().map(|burn| burn.marks.bitmap.clone())
    }

    fn burn(
        &mut self,
        target: &ID2D1RenderTarget,
        detail: u8,
        size: (f32, f32),
    ) -> Result<&mut BurnMask, PlatformError> {
        let cols = (size.0 / burn::CELL as f32).ceil().max(1.0) as u32;
        let rows = (size.1 / burn::CELL as f32).ceil().max(1.0) as u32;
        let fits = self.burn.as_ref().is_some_and(|burn| {
            (burn.field.cols(), burn.field.rows(), burn.detail) == (cols, rows, detail)
        });
        if !fits {
            self.burn = Some(BurnMask {
                field: BurnField::new(cols, rows, detail),
                detail,
                paper: CellMask::new(target, cols, rows, burn::CELL, SMOOTH)?,
                marks: CellMask::new(target, cols, rows, burn::CELL, SMOOTH)?,
            });
        }
        Ok(self.burn.as_mut().expect("made above"))
    }

    fn smoke(
        &mut self,
        target: &ID2D1RenderTarget,
        detail: u8,
        seed: u32,
        size: (f32, f32),
    ) -> Result<&mut SmokeMask, PlatformError> {
        let width = size.0.ceil().max(1.0) as u32;
        let height = size.1.ceil().max(1.0) as u32;
        let fits = self.smoke.as_ref().is_some_and(|smoke| {
            (
                smoke.field.cols(),
                smoke.field.rows(),
                smoke.detail,
                smoke.seed,
            ) == (
                width.div_ceil(smoke::CELL),
                height.div_ceil(smoke::CELL),
                detail,
                seed,
            )
        });
        if !fits {
            let field = SmokeField::new(width, height, detail, seed);
            let (cols, rows) = (field.cols(), field.rows());
            self.smoke = Some(SmokeMask {
                field,
                detail,
                seed,
                paper: CellMask::new(target, cols, rows, smoke::CELL, SMOOTH)?,
            });
        }
        Ok(self.smoke.as_mut().expect("made above"))
    }

    fn gradient(
        &mut self,
        target: &ID2D1RenderTarget,
    ) -> Result<ID2D1LinearGradientBrush, PlatformError> {
        if let Some(brush) = &self.gradient {
            return Ok(brush.clone());
        }
        // Hidden before the start point and shown after the end point; the
        // points move every frame.
        let stops = [
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: color_f(Color::TRANSPARENT),
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: color_f(Color::rgba(255, 255, 255, 255)),
            },
        ];
        // SAFETY: a live render target and fully initialised locals.
        let brush = unsafe {
            let stops = target.CreateGradientStopCollection(
                &stops,
                D2D1_GAMMA_2_2,
                D2D1_EXTEND_MODE_CLAMP,
            )?;
            target.CreateLinearGradientBrush(
                &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: Vector2::new(0.0, 0.0),
                    endPoint: Vector2::new(0.0, 1.0),
                },
                None,
                &stops,
            )?
        };
        self.gradient = Some(brush.clone());
        Ok(brush)
    }

    fn noise(
        &mut self,
        target: &ID2D1RenderTarget,
        grain: u8,
        size: (f32, f32),
    ) -> Result<&mut NoiseMask, PlatformError> {
        let cell = u32::from(grain.max(1)) * NOISE_CELL_PER_DETAIL;
        let cols = (size.0 / cell as f32).ceil().max(1.0) as u32;
        let rows = (size.1 / cell as f32).ceil().max(1.0) as u32;
        let fits = self
            .noise
            .as_ref()
            .is_some_and(|noise| (noise.cols, noise.rows, noise.cell) == (cols, rows, cell));
        if !fits {
            self.noise = Some(NoiseMask::new(target, cols, rows, cell)?);
        }
        Ok(self.noise.as_mut().expect("made above"))
    }

    /// The offscreen target Pixelate draws the scene on, small, for a space
    /// of `size`.
    pub(super) fn pixel_target(
        &mut self,
        target: &ID2D1RenderTarget,
        size: (f32, f32),
    ) -> Result<ID2D1BitmapRenderTarget, PlatformError> {
        let pixel_size = (size.0.ceil() as u32, size.1.ceil() as u32);
        if let Some(pixels) = &self.pixels
            && pixels.size == pixel_size
        {
            return Ok(pixels.target.clone());
        }
        // The format is named rather than inherited: a composition surface's
        // resources come from a device context with no target bound, whose
        // format is unknown, and Direct2D refused it with
        // WINCODEC_ERR_UNSUPPORTEDPIXELFORMAT on every Pixelate frame (seen
        // 2026-10-18).
        let format = D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        };
        // SAFETY: a live render target and fully initialised locals.
        let offscreen = unsafe {
            let offscreen = target.CreateCompatibleRenderTarget(
                Some(&D2D_SIZE_F {
                    width: size.0,
                    height: size.1,
                }),
                None,
                Some(&format),
                D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
            )?;
            offscreen.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            // Transparent background: ClearType needs an opaque one.
            offscreen.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            offscreen
        };
        self.pixels = Some(PixelTarget {
            target: offscreen.clone(),
            size: pixel_size,
        });
        Ok(offscreen)
    }
}

/// The scale Pixelate draws the scene at, and the part of the offscreen
/// target that scene then covers, for a space of `size`.
pub(super) fn pixelated(size: (f32, f32), block: u32) -> (f32, D2D_RECT_F) {
    let scale = 1.0 / block.max(1) as f32;
    let source = rect_f(0.0, 0.0, (size.0 * scale).ceil(), (size.1 * scale).ceil());
    (scale, source)
}

impl NoiseMask {
    fn new(
        target: &ID2D1RenderTarget,
        cols: u32,
        rows: u32,
        cell: u32,
    ) -> Result<Self, PlatformError> {
        let values = (0..rows)
            .flat_map(|row| (0..cols).map(move |col| noise_value(col, row)))
            .collect();
        Ok(Self {
            cols,
            rows,
            cell,
            values,
            shown: CellMask::new(target, cols, rows, cell, BLOCKY)?,
        })
    }
}

/// How a cell mask stretches over the space: hard-edged cells (Dissolve's
/// grain) or smooth ones (Burn's paper).
const BLOCKY: D2D1_BITMAP_INTERPOLATION_MODE = D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR;
const SMOOTH: D2D1_BITMAP_INTERPOLATION_MODE = D2D1_BITMAP_INTERPOLATION_MODE_LINEAR;

impl CellMask {
    fn new(
        target: &ID2D1RenderTarget,
        cols: u32,
        rows: u32,
        cell: u32,
        interpolation: D2D1_BITMAP_INTERPOLATION_MODE,
    ) -> Result<Self, PlatformError> {
        let properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };
        // SAFETY: a live render target and fully initialised locals; the
        // bitmap starts uninitialised and is written before every use.
        let (bitmap, brush) = unsafe {
            let bitmap = target.CreateBitmap(
                D2D_SIZE_U {
                    width: cols,
                    height: rows,
                },
                None,
                0,
                &properties,
            )?;
            let brush = target.CreateBitmapBrush(
                &bitmap,
                Some(&D2D1_BITMAP_BRUSH_PROPERTIES {
                    extendModeX: D2D1_EXTEND_MODE_CLAMP,
                    extendModeY: D2D1_EXTEND_MODE_CLAMP,
                    interpolationMode: interpolation,
                }),
                None,
            )?;
            // One mask pixel covers one cell.
            brush.SetTransform(&Matrix3x2 {
                M11: cell as f32,
                M12: 0.0,
                M21: 0.0,
                M22: cell as f32,
                M31: 0.0,
                M32: 0.0,
            });
            (bitmap, brush)
        };
        Ok(Self {
            pixels: vec![0; (cols * rows) as usize],
            bitmap,
            brush,
        })
    }

    /// Copies this frame's pixels to the bitmap.
    fn upload(&self, cols: u32, rows: u32) -> Result<(), PlatformError> {
        // SAFETY: `pixels` holds `cols * rows` BGRA pixels, so `cols * 4` is
        // its row pitch; D2D copies the data.
        unsafe {
            self.bitmap.CopyFromMemory(
                Some(&D2D_RECT_U {
                    left: 0,
                    top: 0,
                    right: cols,
                    bottom: rows,
                }),
                self.pixels.as_ptr() as *const core::ffi::c_void,
                cols * 4,
            )?;
        }
        Ok(())
    }
}

/// Pushes `layer` with `opacity` and `mask`, runs `draw`, and pops it.
///
/// # Safety
/// Call between `BeginDraw` and `EndDraw` on `target`.
pub(super) unsafe fn with_layer(
    target: &ID2D1RenderTarget,
    layer: &ID2D1Layer,
    opacity: f32,
    mask: Option<ID2D1Brush>,
    draw: impl FnOnce(),
) {
    let parameters = D2D1_LAYER_PARAMETERS {
        contentBounds: rect_f(-f32::MAX, -f32::MAX, f32::MAX, f32::MAX),
        geometricMask: ManuallyDrop::new(None),
        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        maskTransform: Matrix3x2::identity(),
        opacity,
        opacityBrush: ManuallyDrop::new(mask),
        layerOptions: D2D1_LAYER_OPTIONS_NONE,
    };
    // SAFETY: the caller is inside BeginDraw/EndDraw; D2D holds its own
    // reference to the brush while the layer is pushed.
    unsafe {
        target.PushLayer(&parameters, layer);
        draw();
        target.PopLayer();
    }
    // The `ManuallyDrop` fields are not dropped with `parameters`; release
    // the brush reference this function was given.
    drop(ManuallyDrop::into_inner(parameters.opacityBrush));
}

/// Where a gradient mask's brush starts (hidden) and ends (shown) over a
/// space of `size`, sweeping in from `edge`; see `Mask::Gradient`.
fn gradient_points(
    edge: Edge,
    position: f32,
    feather: f32,
    (width, height): (f32, f32),
) -> ((f32, f32), (f32, f32)) {
    let (hidden, shown) = (position - feather, position);
    match edge {
        Edge::Auto | Edge::Top => ((0.0, hidden * height), (0.0, shown * height)),
        Edge::Bottom => (
            (0.0, (1.0 - hidden) * height),
            (0.0, (1.0 - shown) * height),
        ),
        Edge::Left => ((hidden * width, 0.0), (shown * width, 0.0)),
        Edge::Right => (((1.0 - hidden) * width, 0.0), ((1.0 - shown) * width, 0.0)),
    }
}

/// A cell's fixed noise value: an integer hash, so the same cell always
/// dissolves at the same moment and a reversed dissolve retraces itself.
fn noise_value(col: u32, row: u32) -> u8 {
    let mut hash = col.wrapping_mul(0x9E37_79B1) ^ row.wrapping_mul(0x85EB_CA77);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2C1B_3C6D);
    hash ^= hash >> 12;
    (hash >> 24) as u8
}

/// Writes each cell's mask pixel: opaque (premultiplied white) while its
/// noise is at or above `threshold` (0 to 1), else transparent.
fn fill_noise_mask(values: &[u8], threshold: f32, pixels: &mut [u32]) {
    let cutoff = threshold * 256.0;
    for (pixel, value) in pixels.iter_mut().zip(values) {
        *pixel = if f32::from(*value) >= cutoff {
            u32::MAX
        } else {
            0
        };
    }
}

/// Writes each cell's two Burn pixels for a burn at `front`: whether the
/// paper still shows, and the glow, char, or scorch over it.
fn fill_burn(values: &[f32], front: f32, paper: &mut [u32], marks: &mut [u32]) {
    for ((value, paper), marks) in values.iter().zip(paper.iter_mut()).zip(marks.iter_mut()) {
        (*paper, *marks) = burn::cell_pixels(burn::stage(value - front));
    }
}

/// Writes each cell's mask pixel: opaque (premultiplied white) while the
/// smoke has not reached it, fading as its plume passes.
fn fill_smoke(values: &[f32], front: f32, paper: &mut [u32]) {
    for (value, paper) in values.iter().zip(paper.iter_mut()) {
        let alpha = (smoke::cover(*value, front) * 255.0).round() as u32;
        *paper = alpha << 24 | alpha << 16 | alpha << 8 | alpha;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gradient_sweeps_from_its_edge() {
        let size = (200.0, 100.0);
        assert_eq!(
            gradient_points(Edge::Top, 0.5, 0.25, size),
            ((0.0, 25.0), (0.0, 50.0))
        );
        assert_eq!(
            gradient_points(Edge::Bottom, 0.5, 0.25, size),
            ((0.0, 75.0), (0.0, 50.0))
        );
        assert_eq!(
            gradient_points(Edge::Right, 0.5, 0.25, size),
            ((150.0, 0.0), (100.0, 0.0))
        );
    }

    #[test]
    fn pixelate_draws_small_and_reads_back_what_it_drew() {
        let (scale, source) = pixelated((200.0, 101.0), 4);
        assert_eq!(scale, 0.25);
        assert_eq!((source.right, source.bottom), (50.0, 26.0));
        assert_eq!(pixelated((200.0, 100.0), 1).0, 1.0);
    }

    #[test]
    fn a_burn_writes_both_pixels_of_every_cell() {
        let values = [0.0, 0.5, 1.0];
        let (mut paper, mut marks) = ([7; 3], [7; 3]);
        fill_burn(&values, 0.5, &mut paper, &mut marks);
        assert_eq!(paper[0], 0, "below the front is gone");
        assert_eq!(paper[1], u32::MAX, "the front itself still burns");
        assert_eq!(marks[1] >> 24, 255, "and glows");
        assert_eq!(
            (paper[2], marks[2]),
            (u32::MAX, 0),
            "far above is untouched"
        );
    }

    #[test]
    fn smoke_fades_where_its_plume_passes() {
        let values = [0.0, 0.45, 0.9];
        let mut paper = [7; 3];
        fill_smoke(&values, smoke::front(0.5), &mut paper);
        assert_eq!(paper[0], 0, "the bottom is gone");
        assert!((1..u32::MAX).contains(&paper[1]), "the edge is fading");
        assert_eq!(paper[2], u32::MAX, "the top still shows");
        fill_smoke(&values, smoke::front(0.0), &mut paper);
        assert_eq!(paper, [u32::MAX; 3], "nothing hides at the start");
        fill_smoke(&values, smoke::front(1.0), &mut paper);
        assert_eq!(paper, [0; 3], "all hides at the end");
    }

    #[test]
    fn noise_is_fixed_and_spread_out() {
        assert_eq!(noise_value(3, 7), noise_value(3, 7));
        let values: Vec<u8> = (0..64)
            .flat_map(|row| (0..64).map(move |col| noise_value(col, row)))
            .collect();
        let low = values.iter().filter(|&&value| value < 128).count();
        // Roughly half below the middle, so a dissolve is even.
        assert!((1600..2500).contains(&low), "{low}");
    }

    #[test]
    fn the_noise_mask_shows_cells_at_or_above_the_threshold() {
        let values = [0, 64, 128, 255];
        let mut pixels = [7; 4];
        fill_noise_mask(&values, 0.0, &mut pixels);
        assert_eq!(pixels, [u32::MAX; 4], "all shown at 0");
        fill_noise_mask(&values, 0.5, &mut pixels);
        assert_eq!(pixels, [0, 0, u32::MAX, u32::MAX]);
        fill_noise_mask(&values, 1.0, &mut pixels);
        assert_eq!(pixels, [0; 4], "all hidden at 1");
    }
}
