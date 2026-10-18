//! The device resources a `TransitionFrame` is drawn with (ROADMAP D12): a
//! layer, the gradient and noise masks, and the offscreen target Pixelate
//! draws small. Each is made the first time a frame needs it and kept, so a
//! frame only updates them; they go with the device.

use std::mem::ManuallyDrop;

use deskspace_core::animation::Edge;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_U, D2D_SIZE_F, D2D1_GRADIENT_STOP};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2,
    D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    ID2D1BitmapBrush, ID2D1BitmapRenderTarget, ID2D1Brush, ID2D1Layer, ID2D1LinearGradientBrush,
};

use super::*;
use crate::render::transition::Mask;

/// Dissolve's and Burn's grain, in pixels per step of detail.
const NOISE_CELL_PER_DETAIL: u32 = 2;

#[derive(Default)]
pub(super) struct Effects {
    layer: Option<ID2D1Layer>,
    gradient: Option<ID2D1LinearGradientBrush>,
    noise: Option<NoiseMask>,
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
    /// The cells burning right now: Burn's glowing edge.
    embers: CellMask,
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
        }
    }

    /// The opacity brush for Burn's glowing edge this frame: the cells whose
    /// noise is within the mask's glow above its threshold. `None` for any
    /// other mask. Call after `mask_brush` for the same frame.
    pub(super) fn ember_brush(
        &mut self,
        target: &ID2D1RenderTarget,
        mask: Mask,
        size: (f32, f32),
    ) -> Result<Option<ID2D1Brush>, PlatformError> {
        let Mask::Noise {
            threshold,
            grain,
            glow,
        } = mask
        else {
            return Ok(None);
        };
        if glow <= 0.0 {
            return Ok(None);
        }
        let noise = self.noise(target, grain, size)?;
        fill_ember_mask(&noise.values, threshold, glow, &mut noise.embers.pixels);
        noise.embers.upload(noise.cols, noise.rows)?;
        Ok(Some(noise.embers.brush.clone().into()))
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
        // SAFETY: a live render target and a fully initialised size.
        let offscreen = unsafe {
            let offscreen = target.CreateCompatibleRenderTarget(
                Some(&D2D_SIZE_F {
                    width: size.0,
                    height: size.1,
                }),
                None,
                None,
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
            shown: CellMask::new(target, cols, rows, cell)?,
            embers: CellMask::new(target, cols, rows, cell)?,
        })
    }
}

impl CellMask {
    fn new(
        target: &ID2D1RenderTarget,
        cols: u32,
        rows: u32,
        cell: u32,
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
                    interpolationMode: D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
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

/// Writes each cell's ember pixel: opaque while its noise is at or above
/// `threshold` but within `glow` of it (both 0 to 1), the band about to burn.
fn fill_ember_mask(values: &[u8], threshold: f32, glow: f32, pixels: &mut [u32]) {
    let (low, high) = (threshold * 256.0, (threshold + glow) * 256.0);
    for (pixel, value) in pixels.iter_mut().zip(values) {
        let value = f32::from(*value);
        *pixel = if value >= low && value < high {
            u32::MAX
        } else {
            0
        };
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
    fn embers_are_the_band_just_above_the_threshold() {
        let values = [0, 100, 128, 140, 255];
        let mut pixels = [7; 5];
        fill_ember_mask(&values, 0.5, 0.1, &mut pixels);
        assert_eq!(pixels, [0, 0, u32::MAX, u32::MAX, 0]);
        fill_ember_mask(&values, 1.0, 0.1, &mut pixels);
        assert_eq!(pixels, [0; 5], "nothing left to burn");
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
