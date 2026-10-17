//! A window's Direct2D render target and its one brush, created on first
//! paint, resized with the window, and dropped when the device is lost.

use super::*;

#[derive(Default)]
pub(super) struct HwndCanvas {
    target: Option<ID2D1HwndRenderTarget>,
    brush: Option<ID2D1SolidColorBrush>,
    size: (u32, u32),
}

impl HwndCanvas {
    /// The client size in pixels as of the last `ensure`.
    pub(super) fn size(&self) -> (u32, u32) {
        self.size
    }

    pub(super) fn target(&self) -> Option<&ID2D1HwndRenderTarget> {
        self.target.as_ref()
    }

    /// The target and brush for `hwnd`, created or resized to its client
    /// area.
    pub(super) fn ensure(
        &mut self,
        graphics: &Graphics,
        hwnd: HWND,
    ) -> Result<(ID2D1HwndRenderTarget, ID2D1SolidColorBrush), PlatformError> {
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
            return Ok((target.clone(), brush.clone()));
        }

        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            // 96 DPI makes one DIP one physical pixel, matching the model's
            // pixel coordinates.
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
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
                .CreateHwndRenderTarget(&properties, &hwnd_properties)?;
            target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            let brush = target.CreateSolidColorBrush(&color_f(Color::TRANSPARENT), None)?;
            (target, brush)
        };
        self.target = Some(target.clone());
        self.brush = Some(brush.clone());
        self.size = size;
        Ok((target, brush))
    }

    /// Drops the device-dependent resources, so the next `ensure` recreates
    /// them (after `D2DERR_RECREATE_TARGET`).
    pub(super) fn release(&mut self) {
        self.target = None;
        self.brush = None;
    }
}
