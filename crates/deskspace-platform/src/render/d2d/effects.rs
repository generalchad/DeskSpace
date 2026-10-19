//! The device resources a `TransitionFrame` is drawn with (ROADMAP D12): a
//! layer, the gradient, noise, and burn masks, and the offscreen target
//! Pixelate draws small. Each is made the first time a frame needs it and kept, so a
//! frame only updates them; they go with the device.

use std::mem::ManuallyDrop;

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
use crate::render::dissolve::DissolveField;
use crate::render::math::Grid;
use crate::render::transition::{Mask, Side};

#[derive(Default)]
pub(super) struct Effects {
    layer: Option<ID2D1Layer>,
    gradient: Option<ID2D1LinearGradientBrush>,
    noise: Option<NoiseMask>,
    burn: Option<BurnMask>,
    pixels: Option<PixelTarget>,
}

/// Dissolve's mask: one pixel per grain-sized cell, stretched over the space
/// by its brush's transform.
struct NoiseMask {
    field: DissolveField,
    /// The space size and grain it was made for.
    made_for: ((f32, f32), u8),
    /// The cells still shown.
    shown: CellMask,
}

/// Burn's paper: one pixel per `burn::CELL`-sized cell, drawn smoothly.
struct BurnMask {
    field: BurnField,
    /// The space size and detail it was made for.
    made_for: ((f32, f32), u8),
    /// Where the paper still shows: the contents' opacity mask.
    paper: CellMask,
    /// The glowing edge, the char, and the scorch, laid over the paper.
    marks: CellMask,
}

/// One pixel per cell, rewritten each frame and stretched over the space by
/// its brush.
struct CellMask {
    cols: u32,
    rows: u32,
    /// The mask's `cols * rows` pixels for the current frame, written in
    /// place; never resized.
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
                side,
                position,
                feather,
            } => {
                let brush = self.gradient(target)?;
                let (start, end) = gradient_points(side, position, feather, size);
                // SAFETY: a live brush owned by this painter.
                unsafe {
                    brush.SetStartPoint(Vector2::new(start.0, start.1));
                    brush.SetEndPoint(Vector2::new(end.0, end.1));
                }
                Ok(Some(brush.into()))
            }
            Mask::Noise { threshold, grain } => {
                let noise = self.noise(target, grain, size)?;
                noise.field.fill_mask(threshold, &mut noise.shown.pixels);
                noise.shown.upload()?;
                Ok(Some(noise.shown.brush.clone().into()))
            }
            Mask::Burn { front, detail } => {
                let burn = self.burn(target, detail, size)?;
                burn.field
                    .fill_pixels(front, &mut burn.paper.pixels, &mut burn.marks.pixels);
                burn.paper.upload()?;
                burn.marks.upload()?;
                Ok(Some(burn.paper.brush.clone().into()))
            }
        }
    }

    /// The color Burn lays over the paper this frame (`render::burn`);
    /// `None` for any other mask. Call after `mask_brush` for the same
    /// frame, which writes it.
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
        let made_for = (size, detail);
        if self
            .burn
            .as_ref()
            .is_none_or(|burn| burn.made_for != made_for)
        {
            let field = BurnField::for_size(size, detail);
            let grid = field.grid();
            self.burn = Some(BurnMask {
                field,
                made_for,
                paper: CellMask::new(target, grid, burn::CELL, SMOOTH)?,
                marks: CellMask::new(target, grid, burn::CELL, SMOOTH)?,
            });
        }
        Ok(self.burn.as_mut().expect("made above"))
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
        let made_for = (size, grain);
        if self
            .noise
            .as_ref()
            .is_none_or(|noise| noise.made_for != made_for)
        {
            let field = DissolveField::for_size(size, grain);
            let shown = CellMask::new(target, field.grid(), field.cell(), BLOCKY)?;
            self.noise = Some(NoiseMask {
                field,
                made_for,
                shown,
            });
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
        // SAFETY: a live render target and fully initialised locals.
        let offscreen = unsafe {
            let offscreen = target.CreateCompatibleRenderTarget(
                Some(&D2D_SIZE_F {
                    width: size.0,
                    height: size.1,
                }),
                None,
                Some(&PREMULTIPLIED_BGRA),
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

/// The scale Pixelate draws the scene at on each axis, and the part of the
/// offscreen target that scene then covers, for a space of `size`.
///
/// The scene fills whole pixels, so its blocks come out a little under
/// `block` where `block` does not divide the size. At exactly 1 / `block`
/// the last column and row were only partly covered, so partly transparent,
/// and drew as a pale strip a block wide down the right and bottom edges
/// (seen 2026-10-19).
pub(super) fn pixelated(size: (f32, f32), block: u32) -> ((f32, f32), D2D_RECT_F) {
    let block = block.max(1) as f32;
    let (width, height) = ((size.0 / block).ceil(), (size.1 / block).ceil());
    let scale = (width / size.0.max(1.0), height / size.1.max(1.0));
    (scale, rect_f(0.0, 0.0, width, height))
}

/// How a cell mask stretches over the space: hard-edged cells (Dissolve's
/// grain) or smooth ones (Burn's paper).
const BLOCKY: D2D1_BITMAP_INTERPOLATION_MODE = D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR;
const SMOOTH: D2D1_BITMAP_INTERPOLATION_MODE = D2D1_BITMAP_INTERPOLATION_MODE_LINEAR;

impl CellMask {
    /// A mask with a pixel for each cell of `grid`, each covering a cell
    /// `cell` pixels square.
    fn new(
        target: &ID2D1RenderTarget,
        grid: Grid,
        cell: u32,
        interpolation: D2D1_BITMAP_INTERPOLATION_MODE,
    ) -> Result<Self, PlatformError> {
        let Grid { cols, rows } = grid;
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
                &bitmap_properties(),
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
            brush.SetTransform(&Matrix3x2::scale(cell as f32, cell as f32));
            (bitmap, brush)
        };
        Ok(Self {
            cols,
            rows,
            pixels: vec![0; grid.cell_count()],
            bitmap,
            brush,
        })
    }

    /// Copies this frame's pixels to the bitmap.
    fn upload(&self) -> Result<(), PlatformError> {
        // SAFETY: `pixels` holds `cols * rows` BGRA pixels (made so in `new`
        // and never resized), so `cols * 4` is its row pitch; D2D copies the
        // data.
        unsafe {
            self.bitmap.CopyFromMemory(
                Some(&D2D_RECT_U {
                    left: 0,
                    top: 0,
                    right: self.cols,
                    bottom: self.rows,
                }),
                self.pixels.as_ptr() as *const core::ffi::c_void,
                self.cols * 4,
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
/// space of `size`, sweeping in from `side`; see `Mask::Gradient`.
fn gradient_points(
    side: Side,
    position: f32,
    feather: f32,
    (width, height): (f32, f32),
) -> ((f32, f32), (f32, f32)) {
    let (hidden, shown) = (position - feather, position);
    match side {
        Side::Top => ((0.0, hidden * height), (0.0, shown * height)),
        Side::Bottom => (
            (0.0, (1.0 - hidden) * height),
            (0.0, (1.0 - shown) * height),
        ),
        Side::Left => ((hidden * width, 0.0), (shown * width, 0.0)),
        Side::Right => (((1.0 - hidden) * width, 0.0), ((1.0 - shown) * width, 0.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gradient_sweeps_from_its_edge() {
        let size = (200.0, 100.0);
        assert_eq!(
            gradient_points(Side::Top, 0.5, 0.25, size),
            ((0.0, 25.0), (0.0, 50.0))
        );
        assert_eq!(
            gradient_points(Side::Bottom, 0.5, 0.25, size),
            ((0.0, 75.0), (0.0, 50.0))
        );
        assert_eq!(
            gradient_points(Side::Right, 0.5, 0.25, size),
            ((150.0, 0.0), (100.0, 0.0))
        );
    }

    #[test]
    fn pixelate_draws_small_and_reads_back_what_it_drew() {
        let (scale, source) = pixelated((200.0, 101.0), 4);
        assert_eq!((source.right, source.bottom), (50.0, 26.0));
        // The scene covers every pixel it reads back, the last row too.
        assert_eq!(200.0 * scale.0, source.right);
        assert!((101.0 * scale.1 - source.bottom).abs() < 1e-4);
        assert_eq!(pixelated((200.0, 100.0), 1).0, (1.0, 1.0));
    }
}
