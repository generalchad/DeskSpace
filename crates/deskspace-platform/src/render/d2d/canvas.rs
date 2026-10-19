//! Where a window's painter draws: its own Direct2D render target, or the
//! composition drawing surface the window shows (`win32::composition`),
//! with a second one for the blur's mask. Either one, with its one brush, is
//! made on first paint, resized with the window, and dropped when the device
//! is lost.

use windows::UI::Composition::{CompositionDrawingSurface, CompositionGraphicsDevice};
use windows::Win32::Foundation::{E_POINTER, POINT, SIZE};
use windows::Win32::Graphics::Direct2D::{D2D1_DEVICE_CONTEXT_OPTIONS_NONE, ID2D1DeviceContext};
use windows::Win32::System::WinRT::Composition::{
    ICompositionDrawingSurfaceInterop, ICompositionGraphicsDeviceInterop,
};
use windows::core::Interface;

use super::*;

/// A space painter's canvas.
pub(super) enum Canvas {
    Hwnd(HwndCanvas),
    Surface(SurfaceCanvas),
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas::Hwnd(HwndCanvas::default())
    }
}

impl Canvas {
    /// The client size in pixels as of the last `ensure`.
    pub(super) fn size(&self) -> (u32, u32) {
        match self {
            Canvas::Hwnd(canvas) => canvas.size,
            Canvas::Surface(canvas) => canvas.size,
        }
    }

    /// A target to make device resources with, and the brush, sized to
    /// `hwnd`'s client area. What they make draws on `begin`'s target too.
    pub(super) fn ensure(
        &mut self,
        graphics: &Graphics,
        hwnd: HWND,
    ) -> Result<(ID2D1RenderTarget, ID2D1SolidColorBrush), PlatformError> {
        match self {
            Canvas::Hwnd(canvas) => canvas.ensure(graphics, hwnd),
            Canvas::Surface(canvas) => canvas.ensure(hwnd),
        }
    }

    /// Starts a draw: the target to draw on, and where the window's top-left
    /// corner is on it. Call after `ensure`, and `end` after drawing.
    pub(super) fn begin(&mut self) -> windows::core::Result<(ID2D1RenderTarget, Vector2)> {
        match self {
            Canvas::Hwnd(canvas) => canvas.begin(),
            Canvas::Surface(canvas) => canvas.begin(),
        }
    }

    pub(super) fn end(&mut self) -> windows::core::Result<()> {
        match self {
            Canvas::Hwnd(canvas) => canvas.end(),
            Canvas::Surface(canvas) => canvas.end(),
        }
    }

    /// Draws the blur's mask (T13) with `draw`, given the target and where
    /// the window's top-left corner is on it, over a transparent clear. A
    /// window target has no mask, since the accent blur takes none, so
    /// nothing is drawn. Call after `ensure`, outside `begin` and `end`.
    pub(super) fn draw_blur_mask(
        &mut self,
        draw: impl FnOnce(&ID2D1RenderTarget, Vector2),
    ) -> windows::core::Result<()> {
        match self {
            Canvas::Hwnd(_) => Ok(()),
            Canvas::Surface(canvas) => {
                let size = canvas.size;
                let (target, origin) = canvas.blur_mask.begin(size)?;
                // SAFETY: between the mask surface's begin and end; the clip
                // `begin` pushed keeps the clear in this window's part.
                unsafe { target.Clear(Some(&color_f(Color::TRANSPARENT))) };
                draw(&target, origin);
                canvas.blur_mask.end()
            }
        }
    }

    /// Drops the device resources after the device was lost, and has `hwnd`
    /// painted again with new ones.
    pub(super) fn release(&mut self, hwnd: HWND) {
        match self {
            Canvas::Hwnd(canvas) => canvas.release(hwnd),
            Canvas::Surface(canvas) => canvas.release(hwnd),
        }
    }
}

#[derive(Default)]
pub(super) struct HwndCanvas {
    target: Option<ID2D1HwndRenderTarget>,
    brush: Option<ID2D1SolidColorBrush>,
    size: (u32, u32),
}

impl HwndCanvas {
    /// The target and brush for `hwnd`, created or resized to its client
    /// area. The painters draw on any `ID2D1RenderTarget`, so the HWND
    /// target's own type stays here.
    pub(super) fn ensure(
        &mut self,
        graphics: &Graphics,
        hwnd: HWND,
    ) -> Result<(ID2D1RenderTarget, ID2D1SolidColorBrush), PlatformError> {
        let mut client = RECT::default();
        // SAFETY: `hwnd` is the window being painted.
        unsafe { GetClientRect(hwnd, &mut client)? };
        let size = (
            (client.right - client.left).max(1) as u32,
            (client.bottom - client.top).max(1) as u32,
        );

        if let (Some(target), Some(brush)) = (&self.target, &self.brush) {
            if size != self.size {
                // SAFETY: resizing a live render target.
                unsafe {
                    target.Resize(&D2D_SIZE_U {
                        width: size.0,
                        height: size.1,
                    })?
                };
                self.size = size;
            }
            return Ok((target.clone().into(), brush.clone()));
        }

        let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: size.0,
                height: size.1,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        // SAFETY: property structs are fully initialised locals.
        let (target, brush) = unsafe {
            let target = graphics
                .d2d
                .CreateHwndRenderTarget(&render_target_properties(), &hwnd_properties)?;
            target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            let brush = target.CreateSolidColorBrush(&color_f(Color::TRANSPARENT), None)?;
            (target, brush)
        };
        self.target = Some(target.clone());
        self.brush = Some(brush.clone());
        self.size = size;
        Ok((target.into(), brush))
    }

    fn begin(&self) -> windows::core::Result<(ID2D1RenderTarget, Vector2)> {
        let target = self.target.as_ref().ok_or(E_POINTER)?;
        // SAFETY: a live render target; `end` closes the draw.
        unsafe { target.BeginDraw() };
        Ok((target.clone().into(), Vector2::zero()))
    }

    fn end(&self) -> windows::core::Result<()> {
        let target = self.target.as_ref().ok_or(E_POINTER)?;
        // SAFETY: closes the draw `begin` opened.
        unsafe { target.EndDraw(None, None) }
    }

    /// Drops the device-dependent resources after `D2DERR_RECREATE_TARGET`
    /// and invalidates `hwnd`, so a new paint recreates them and draws the
    /// frame that was lost.
    pub(super) fn release(&mut self, hwnd: HWND) {
        self.target = None;
        self.brush = None;
        // The lost frame was never presented, and EndPaint validates the
        // window, so without this it stays blank (only its DWM blur shows)
        // until something else invalidates it, such as a click. Spaces
        // painted while the device was lost at startup were left that way
        // (seen 2026-10-18). Invalidating between BeginPaint and EndPaint
        // queues one more WM_PAINT.
        // SAFETY: `hwnd` is the window being painted.
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

/// A window's composition drawing surfaces: its content, and the blur's mask
/// (T13). Resources are made on a device context of their own over the
/// surfaces' device, since a surface's context exists only during a draw.
pub(super) struct SurfaceCanvas {
    content: DrawingSurface,
    blur_mask: DrawingSurface,
    /// The device the surfaces were made on, which says what Direct2D device
    /// draws on them.
    device: ICompositionGraphicsDeviceInterop,
    resources: Option<SurfaceResources>,
    size: (u32, u32),
}

struct SurfaceResources {
    /// The device they were made on, to notice when it is replaced.
    device: ID2D1Device,
    context: ID2D1DeviceContext,
    brush: ID2D1SolidColorBrush,
}

impl SurfaceCanvas {
    pub(super) fn new(
        content: &CompositionDrawingSurface,
        blur_mask: &CompositionDrawingSurface,
        device: &CompositionGraphicsDevice,
    ) -> windows::core::Result<Self> {
        Ok(Self {
            content: DrawingSurface::new(content)?,
            blur_mask: DrawingSurface::new(blur_mask)?,
            device: device.cast()?,
            resources: None,
            size: (0, 0),
        })
    }

    fn ensure(
        &mut self,
        hwnd: HWND,
    ) -> Result<(ID2D1RenderTarget, ID2D1SolidColorBrush), PlatformError> {
        let device = self.device()?;
        if let Some(resources) = &self.resources
            && resources.device != device
        {
            // Another window found the device lost and replaced it (see
            // `release`); everything made on the old one is useless now.
            return Err(windows::core::Error::from(D2DERR_RECREATE_TARGET).into());
        }
        let resources = match self.resources.take() {
            Some(resources) => resources,
            // SAFETY: a live device; the brush is made on its context.
            None => unsafe {
                let context = device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
                let brush = context.CreateSolidColorBrush(&color_f(Color::TRANSPARENT), None)?;
                SurfaceResources {
                    device,
                    context,
                    brush,
                }
            },
        };
        let made = (resources.context.clone().into(), resources.brush.clone());
        self.resources = Some(resources);
        let size = client_size(hwnd)?;
        if size != self.size {
            self.content.resize(size)?;
            self.blur_mask.resize(size)?;
            self.size = size;
        }
        Ok(made)
    }

    /// The Direct2D device the surfaces' composition graphics device draws
    /// with now.
    fn device(&self) -> windows::core::Result<ID2D1Device> {
        // SAFETY: a live graphics device; it hands out its own reference.
        unsafe { self.device.GetRenderingDevice()? }.cast()
    }

    fn begin(&mut self) -> windows::core::Result<(ID2D1RenderTarget, Vector2)> {
        self.content.begin(self.size)
    }

    fn end(&mut self) -> windows::core::Result<()> {
        self.content.end()
    }

    /// Drops the resources and, if no other window has yet, gives the
    /// composition graphics device a new Direct2D device: every surface's
    /// content went with the old one, and `win32::composition` repaints them
    /// all when the device is replaced.
    fn release(&mut self, hwnd: HWND) {
        let stale = self.resources.take().map(|resources| resources.device);
        let current = self.device().ok();
        if (stale.is_none() || stale == current)
            && let Err(error) = self.replace_device()
        {
            tracing::warn!(%error, "could not replace the lost Direct2D device");
        }
        self.size = (0, 0);
        // SAFETY: `hwnd` is the window being painted.
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }

    fn replace_device(&self) -> Result<(), PlatformError> {
        let device = graphics()?.create_device()?;
        // SAFETY: a live graphics device, given a new device from the shared
        // factory.
        unsafe { self.device.SetRenderingDevice(&device)? };
        tracing::debug!("direct2d device lost; replaced it for every surface");
        Ok(())
    }
}

/// One composition drawing surface. Its Direct2D context exists only between
/// the surface's `BeginDraw` and `EndDraw`, and sits at an offset in a
/// shared atlas, so each draw is clipped to the surface's part of the atlas.
/// https://learn.microsoft.com/en-us/windows/win32/api/windows.ui.composition.interop/nn-windows-ui-composition-interop-icompositiondrawingsurfaceinterop
struct DrawingSurface {
    surface: ICompositionDrawingSurfaceInterop,
    /// The context `begin` handed out, until `end`.
    drawing: Option<ID2D1DeviceContext>,
}

impl DrawingSurface {
    fn new(surface: &CompositionDrawingSurface) -> windows::core::Result<Self> {
        Ok(Self {
            surface: surface.cast()?,
            drawing: None,
        })
    }

    fn resize(&self, (width, height): (u32, u32)) -> windows::core::Result<()> {
        // SAFETY: resizing a live surface outside a draw; its content is
        // undefined afterwards, and the next draw redraws all of it.
        unsafe {
            self.surface.Resize(SIZE {
                cx: width as i32,
                cy: height as i32,
            })
        }
    }

    /// Starts a draw on the surface, `size` large: the target, and where the
    /// surface's top-left corner is on it. `end` closes it.
    fn begin(&mut self, size: (u32, u32)) -> windows::core::Result<(ID2D1RenderTarget, Vector2)> {
        let mut offset = POINT::default();
        // SAFETY: `offset` is a valid out-pointer; no update rect means the
        // whole surface. `end` closes the draw.
        let context: ID2D1DeviceContext = unsafe { self.surface.BeginDraw(None, &mut offset)? };
        let origin = Vector2::new(offset.x as f32, offset.y as f32);
        // SAFETY: drawing on the context the surface just handed out. Clear
        // ignores the transform, so the clip keeps it inside this surface's
        // part of the atlas.
        unsafe {
            context.SetDpi(PIXEL_DPI, PIXEL_DPI);
            context.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            context.SetTransform(&Matrix3x2::identity());
            context.PushAxisAlignedClip(
                &rect_f(
                    origin.X,
                    origin.Y,
                    origin.X + size.0 as f32,
                    origin.Y + size.1 as f32,
                ),
                D2D1_ANTIALIAS_MODE_ALIASED,
            );
        }
        self.drawing = Some(context.clone());
        Ok((context.into(), origin))
    }

    fn end(&mut self) -> windows::core::Result<()> {
        // SAFETY: closes the draw `begin` opened, popping its clip first.
        unsafe {
            if let Some(context) = self.drawing.take() {
                context.SetTransform(&Matrix3x2::identity());
                context.PopAxisAlignedClip();
            }
            self.surface.EndDraw()
        }
    }
}

fn client_size(hwnd: HWND) -> windows::core::Result<(u32, u32)> {
    let mut client = RECT::default();
    // SAFETY: `hwnd` is the window being painted.
    unsafe { GetClientRect(hwnd, &mut client)? };
    Ok((
        (client.right - client.left).max(1) as u32,
        (client.bottom - client.top).max(1) as u32,
    ))
}
