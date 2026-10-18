//! Spaces shown through `Windows.UI.Composition` (ROADMAP D12, TASKS T12).
//!
//! A space window made with `WS_EX_NOREDIRECTIONBITMAP` has no bitmap of its
//! own; DWM shows its visual tree instead: a root visual clipped to the
//! space's corners, a backdrop sprite filled with the host backdrop brush
//! (the blur, for Acrylic and Blur), and a content sprite showing the
//! drawing surface the painter draws into. The blur is then a brush
//! DeskSpace owns, so it can take an animation's opacity and masks (T13),
//! which the accent policy (`dwm`) cannot. It renders on a window that is
//! never activated, as spaces never are (T1, seen on build 26300).
//!
//! The choice is made once, at startup (`start`). Windows 10, "Transparency
//! effects" off, desktop-hosted or click-through spaces, and a failure to
//! start keep the accent policy, as does the desktop layer always.
//! https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/using-the-visual-layer-with-win32

use std::cell::RefCell;

use windows::Foundation::{Size, TypedEventHandler};
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use windows::System::DispatcherQueueController;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{
    CompositionDrawingSurface, CompositionGraphicsDevice, CompositionRoundedRectangleGeometry,
    CompositionStretch, Compositor, ContainerVisual, SpriteVisual,
};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::WinRT::Composition::{ICompositorDesktopInterop, ICompositorInterop};
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::Interface;
use windows_numerics::Vector2;

use deskspace_core::theme::Backdrop;

use crate::error::PlatformError;
use crate::render::{Painter, SpaceStyle, d2d};
use crate::win32::debug_info::transparency_enabled;
use crate::win32::runtime;
use crate::win32::system::windows_build;

/// The first Windows 11 build: the host backdrop brush needs it.
const WINDOWS_11: u32 = 22000;

/// What the UI thread's spaces are composed with, while composition runs.
struct Composition {
    // Fields drop in order: the devices and the compositor go before the
    // dispatcher queue that runs them.
    graphics: CompositionGraphicsDevice,
    compositor: Compositor,
    _queue: DispatcherQueueController,
}

thread_local! {
    static COMPOSITION: RefCell<Option<Composition>> = const { RefCell::new(None) };
}

/// Starts composition for this thread's spaces unless something rules it
/// out, and logs which way spaces get their blur. Call once, on the UI
/// thread, after COM is initialised and before any space window exists.
pub(crate) fn start(hosted: bool, click_through: bool) -> bool {
    let reason = ruled_out(
        hosted,
        click_through,
        windows_build(),
        transparency_enabled(),
    );
    if let Some(reason) = reason {
        tracing::info!(reason, "spaces use the accent blur");
        return false;
    }
    match Composition::new() {
        Ok(composition) => {
            COMPOSITION.with(|cell| *cell.borrow_mut() = Some(composition));
            tracing::info!("spaces use composition");
            true
        }
        Err(error) => {
            tracing::warn!(%error, "composition failed to start; spaces use the accent blur");
            false
        }
    }
}

/// Why spaces keep the accent blur, if anything says so. An unknown build
/// counts as Windows 10, and "Transparency effects" counts as on when it is
/// unset, as Windows has it.
fn ruled_out(
    hosted: bool,
    click_through: bool,
    build: Option<u32>,
    transparency: Option<bool>,
) -> Option<&'static str> {
    if hosted {
        Some("spaces are hosted in the desktop")
    } else if click_through {
        Some("spaces are click-through, so layered")
    } else if build.is_none_or(|build| build < WINDOWS_11) {
        Some("this Windows has no host backdrop brush")
    } else if transparency == Some(false) {
        Some("Windows' Transparency effects is off")
    } else {
        None
    }
}

/// Releases the compositor and its devices. Must run after every space
/// window is destroyed and before `OleUninitialize`, as the Direct2D
/// factories do.
pub(crate) fn release() {
    COMPOSITION.with(|cell| cell.borrow_mut().take());
}

impl Composition {
    fn new() -> Result<Self, PlatformError> {
        // A compositor needs a dispatcher queue on its thread, run by the
        // thread's message loop.
        // SAFETY: called on the UI thread with a fully initialised options
        // struct.
        let queue = unsafe {
            CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_STA,
            })?
        };
        let compositor = Compositor::new()?;
        let device = d2d::create_device()?;
        let interop: ICompositorInterop = compositor.cast()?;
        // SAFETY: a live compositor and a Direct2D device from the shared
        // factory.
        let graphics = unsafe { interop.CreateGraphicsDevice(&device)? };
        // A lost device is replaced by the first painter to find it
        // (`render::d2d`), and every surface's content goes with it.
        graphics.RenderingDeviceReplaced(&TypedEventHandler::new(|_, _| {
            runtime::invalidate_spaces();
            Ok(())
        }))?;
        Ok(Self {
            graphics,
            compositor,
            _queue: queue,
        })
    }
}

/// A space window's visual tree.
pub(crate) struct SpaceVisuals {
    root: ContainerVisual,
    corners: CompositionRoundedRectangleGeometry,
    backdrop: SpriteVisual,
    /// The style asks for the blur (Acrylic, Blur).
    blurred: bool,
    /// The blur is not stepped aside (`show_material`).
    material_shown: bool,
    surface: CompositionDrawingSurface,
    graphics: CompositionGraphicsDevice,
    // Kept alive for as long as the window shows the tree.
    _target: DesktopWindowTarget,
}

impl SpaceVisuals {
    /// Builds `window`'s tree, styled for `style` and sized to its client
    /// area.
    ///
    /// # Safety
    /// `window` is a live top-level window of this thread made with
    /// `WS_EX_NOREDIRECTIONBITMAP`.
    pub(crate) unsafe fn create(window: HWND, style: SpaceStyle) -> Result<Self, PlatformError> {
        let (compositor, graphics) = COMPOSITION
            .with(|cell| {
                cell.borrow().as_ref().map(|composition| {
                    (composition.compositor.clone(), composition.graphics.clone())
                })
            })
            .ok_or(PlatformError::NotRunning)?;
        let interop: ICompositorDesktopInterop = compositor.cast()?;
        // SAFETY: the caller's contract. Not topmost: the tree is the
        // window's only content.
        let target = unsafe { interop.CreateDesktopWindowTarget(window, false)? };

        let root = compositor.CreateContainerVisual()?;
        let corners = compositor.CreateRoundedRectangleGeometry()?;
        root.SetClip(&compositor.CreateGeometricClipWithGeometry(&corners)?)?;
        target.SetRoot(&root)?;

        let whole = Vector2::new(1.0, 1.0);
        let backdrop = compositor.CreateSpriteVisual()?;
        backdrop.SetRelativeSizeAdjustment(whole)?;
        backdrop.SetBrush(&compositor.CreateHostBackdropBrush()?)?;

        // The surface is resized with the window by its painter; drawn at
        // its own size from the top-left corner, never stretched.
        let surface = graphics.CreateDrawingSurface(
            Size {
                Width: 1.0,
                Height: 1.0,
            },
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            DirectXAlphaMode::Premultiplied,
        )?;
        let content_brush = compositor.CreateSurfaceBrushWithSurface(&surface)?;
        content_brush.SetStretch(CompositionStretch::None)?;
        content_brush.SetHorizontalAlignmentRatio(0.0)?;
        content_brush.SetVerticalAlignmentRatio(0.0)?;
        let content = compositor.CreateSpriteVisual()?;
        content.SetRelativeSizeAdjustment(whole)?;
        content.SetBrush(&content_brush)?;

        let children = root.Children()?;
        children.InsertAtBottom(&backdrop)?;
        children.InsertAtTop(&content)?;

        let mut visuals = Self {
            root,
            corners,
            backdrop,
            blurred: false,
            material_shown: true,
            surface,
            graphics,
            _target: target,
        };
        visuals.set_style(style)?;
        visuals.fit(window)?;
        Ok(visuals)
    }

    /// A painter that draws into this tree's content.
    pub(crate) fn painter(&self) -> Result<Painter, PlatformError> {
        Painter::on_surface(&self.surface, &self.graphics)
    }

    /// Shows the blur for Acrylic and Blur, and rounds the corners as the
    /// window's frame does (`space_window::Frame`).
    pub(crate) fn set_style(&mut self, style: SpaceStyle) -> Result<(), PlatformError> {
        self.blurred = matches!(style.backdrop, Backdrop::Acrylic | Backdrop::Blur);
        self.sync_backdrop()?;
        let radius = if style.chrome {
            style.corner_radius.max(0) as f32
        } else {
            0.0
        };
        self.corners.SetCornerRadius(Vector2::new(radius, radius))?;
        Ok(())
    }

    /// Steps the blur aside, or brings it back (`space_window::show_material`).
    pub(crate) fn show_material(&mut self, shown: bool) -> Result<(), PlatformError> {
        self.material_shown = shown;
        self.sync_backdrop()
    }

    fn sync_backdrop(&self) -> Result<(), PlatformError> {
        Ok(self
            .backdrop
            .SetIsVisible(self.blurred && self.material_shown)?)
    }

    /// Sizes the tree to `window`'s client area.
    pub(crate) fn fit(&self, window: HWND) -> Result<(), PlatformError> {
        let mut client = RECT::default();
        // SAFETY: `window` is the live window this tree belongs to.
        unsafe { GetClientRect(window, &mut client)? };
        let size = Vector2::new(
            (client.right - client.left) as f32,
            (client.bottom - client.top) as f32,
        );
        self.root.SetSize(size)?;
        self.corners.SetSize(size)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ELEVEN: Option<u32> = Some(26300);

    #[test]
    fn windows_11_with_transparency_composes() {
        assert_eq!(ruled_out(false, false, ELEVEN, Some(true)), None);
        assert_eq!(ruled_out(false, false, ELEVEN, None), None, "unset is on");
    }

    #[test]
    fn everything_else_keeps_the_accent_blur() {
        assert!(ruled_out(true, false, ELEVEN, Some(true)).is_some());
        assert!(ruled_out(false, true, ELEVEN, Some(true)).is_some());
        assert!(ruled_out(false, false, Some(19045), Some(true)).is_some());
        assert!(ruled_out(false, false, None, Some(true)).is_some());
        assert!(ruled_out(false, false, ELEVEN, Some(false)).is_some());
    }
}
