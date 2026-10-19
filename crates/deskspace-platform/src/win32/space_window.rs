//! One window per space.
//!
//! Top-level spaces are `WS_POPUP` windows kept at the bottom of the z-order,
//! styled by DWM (`dwm::apply_style`) with the Windows 11 backdrop and rounded
//! corners, and painted with per-pixel alpha by `render::d2d::Painter`. Where
//! composition runs, they show a visual tree instead (`composition`).
//!
//! Desktop-hosted spaces are `WS_CHILD` windows of the desktop's Progman or
//! WorkerW. Child windows get neither DWM backdrops nor composited alpha, so
//! they are painted opaque and rounded with a window region.
//!
//! Window state lives behind a raw `*mut SpaceState` in `GWLP_USERDATA`
//! (`window_state`) and is freed exactly once, in `WM_NCDESTROY`.

use deskspace_core::geometry::Rect;
use deskspace_core::theme::Backdrop;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateRoundRectRgn, InvalidateRect, MapWindowPoints, SetWindowRgn, ValidateRect,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::{MK_CONTROL, MK_SHIFT};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GW_HWNDPREV, GWL_EXSTYLE,
    GetClientRect, GetParent, GetWindow, GetWindowLongW, GetWindowRect, HTCLIENT, HWND_BOTTOM,
    HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, IDC_ARROW, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE,
    IDC_SIZEWE, IsWindowVisible, KillTimer, LWA_ALPHA, LoadCursorW, MA_NOACTIVATE, SC_MINIMIZE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetCursor,
    SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WINDOWPOS,
    WM_CAPTURECHANGED, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SETCURSOR, WM_SIZE, WM_SYSCOMMAND, WM_TIMER, WM_WINDOWPOSCHANGED, WM_WINDOWPOSCHANGING,
    WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, MouseButton, SpaceEvent};
use crate::render::shadow::ShadowLook;
use crate::render::{
    BorderStroke, PaintFrame, Painter, PreparedScene, ShadowStyle, SpaceScene, SpaceStyle,
};
use crate::ui::CursorShape;
use crate::win32::composition::SpaceVisuals;
use crate::win32::dpi::{scale_for, window_dpi};
use crate::win32::shadow_window::{ShadowPlacement, ShadowWindow};
use crate::win32::space_animation::{self, Animation};
use crate::win32::system::{hiword, is_windows_11, loword};
use crate::win32::window_class::{self, WindowClass};
use crate::win32::{drop_target, dwm, messages, runtime, window_state};

const CLASS_NAME: PCWSTR = w!("DeskSpaceSpace");

/// The id of a space window's one-shot timer that paints it again after
/// `show`. It is set on the window itself, so it never reaches the app's
/// `Timer` ids; the value is kept from when animations had timer 1.
const REPAINT_TIMER_ID: usize = 2;
/// Long enough for DWM to have composed the shown window a few times.
const REPAINT_DELAY_MS: u32 = 100;

/// Mouse-button bits of the `wParam` key state on mouse messages. Defined here
/// to avoid pulling in `Win32_System_SystemServices` for three constants.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttonup
const MK_LBUTTON: usize = 0x0001;
const MK_RBUTTON: usize = 0x0002;
const MK_MBUTTON: usize = 0x0010;
const MK_ANY_BUTTON: usize = MK_LBUTTON | MK_RBUTTON | MK_MBUTTON;

pub(crate) struct SpaceWindowOptions {
    pub id: u64,
    pub rect: Rect,
    pub style: SpaceStyle,
    pub host: Option<HWND>,
    pub click_through: bool,
    pub on_top: bool,
    pub visible: bool,
    /// Blur through composition rather than the accent policy; top-level
    /// spaces only, and only once `composition::start` said so.
    pub composition: bool,
}

pub(super) struct SpaceState {
    id: u64,
    hosted: bool,
    /// Development placement above other windows instead of at the bottom.
    on_top: bool,
    /// The style last applied, so re-applying an unchanged one (every space
    /// on every settings edit) skips the DWM calls and region rebuild.
    style: SpaceStyle,
    frame: Frame,
    /// `None` while `WM_PAINT` has it checked out.
    painter: Option<Painter>,
    pub(super) scene: Option<PreparedScene>,
    tracking_leave: bool,
    cursor: CursorShape,
    /// The animation playing on the window, if any (`space_animation`).
    pub(super) animation: Option<Animation>,
    /// The window's visual tree, when composition gives it its blur.
    visuals: Option<SpaceVisuals>,
    /// What of the DWM backing an animation has left in place
    /// (`show_backing`).
    backing: Backing,
    /// The window the space's shadow shows on, once it has cast one
    /// (`sync_shadow`).
    shadow: Option<ShadowWindow>,
}

/// What DWM draws beside a space's own pixels: an animation steps it aside
/// while it leaves the window see-through, since it cannot fade with the
/// pixels (`space_animation`). The border is not here: the painter draws
/// it, so it animates with the rest (`dwm::BorderOwner`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Backing {
    /// The blur, or the flat fill Windows draws instead of it.
    pub material: bool,
}

impl Backing {
    /// Everything in place, as at rest.
    pub(crate) const WHOLE: Self = Self { material: true };
}

pub(crate) fn create(options: SpaceWindowOptions) -> Result<HWND, PlatformError> {
    let hosted = options.host.is_some();
    let state = Box::into_raw(Box::new(SpaceState {
        id: options.id,
        hosted,
        on_top: options.on_top,
        style: options.style,
        frame: Frame::for_style(
            options.style,
            (!hosted).then(|| material(options.composition)),
            is_windows_11(),
        ),
        painter: Some(Painter::new()),
        scene: None,
        tracking_leave: false,
        cursor: CursorShape::Arrow,
        animation: None,
        visuals: None,
        backing: Backing::WHOLE,
        shadow: None,
    }));

    // An OLE drop target (registered below) makes the space accept file drags
    // and choose their effect; WS_EX_ACCEPTFILES and WM_DROPFILES are not used.
    let mut ex_style: WINDOW_EX_STYLE = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    if options.on_top {
        ex_style |= WS_EX_TOPMOST;
    }
    if options.composition {
        // The visual tree is the window's only content.
        ex_style |= WS_EX_NOREDIRECTIONBITMAP;
    }
    if options.click_through {
        // WS_EX_TRANSPARENT only makes a window click-through when it is also
        // layered; on its own it merely changes paint order.
        // https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles
        ex_style |= WS_EX_LAYERED | WS_EX_TRANSPARENT;
    }
    let style = if hosted {
        WS_CHILD | WS_CLIPSIBLINGS
    } else {
        WS_POPUP
    };
    let rect = options.rect;

    // SAFETY: plain Win32 window creation. Ownership of `state` passes to the
    // window in WM_NCCREATE and it is freed in WM_NCDESTROY. If creation fails
    // after WM_NCCREATE, WM_NCDESTROY has already freed it, so it is
    // deliberately not freed here.
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        ensure_class()?;
        let hwnd = CreateWindowExW(
            ex_style,
            CLASS_NAME,
            w!("DeskSpace space"),
            style,
            rect.x,
            rect.y,
            rect.width.max(1),
            rect.height.max(1),
            options.host,
            None,
            Some(instance),
            Some(state as *const core::ffi::c_void),
        )?;

        if options.click_through {
            let _ = SetLayeredWindowAttributes(hwnd, Default::default(), 255, LWA_ALPHA);
        }
        update_region(hwnd);
        if options.composition
            && let Err(error) = attach_visuals(hwnd, options.style)
        {
            // Without its tree the window shows nothing at all, so it is
            // made again with the accent blur.
            tracing::warn!(%error, "space composition failed; using the accent blur");
            let _ = DestroyWindow(hwnd);
            return create(SpaceWindowOptions {
                composition: false,
                ..options
            });
        }
        if !hosted {
            dwm::apply_style(
                hwnd,
                options.style,
                material(options.composition),
                dwm::BorderOwner::Painter,
            );
            let _ = SetWindowPos(
                hwnd,
                Some(if options.on_top {
                    HWND_TOPMOST
                } else {
                    HWND_BOTTOM
                }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        if options.visible {
            show(hwnd);
        }
        if let Err(error) = drop_target::register(hwnd, options.id, hosted) {
            tracing::warn!(%error, "space will not accept dropped files");
        }
        Ok(hwnd)
    }
}

/// Builds `hwnd`'s visual tree and points its painter at the tree's surface.
///
/// # Safety
/// `hwnd` is a live space window made with `WS_EX_NOREDIRECTIONBITMAP`.
unsafe fn attach_visuals(hwnd: HWND, style: SpaceStyle) -> Result<(), PlatformError> {
    // SAFETY: the caller's contract.
    let visuals = unsafe { SpaceVisuals::create(hwnd, style)? };
    let painter = visuals.painter()?;
    // SAFETY: the borrow does not re-enter.
    unsafe {
        with_state(hwnd, |state| {
            state.painter = Some(painter);
            state.visuals = Some(visuals);
        })
    };
    Ok(())
}

fn material(composition: bool) -> dwm::Material {
    if composition {
        dwm::Material::Composition
    } else {
        dwm::Material::Accent
    }
}

/// Shows `hwnd` without activating it, and paints it again a moment later.
///
/// The first frame a space presents right after it is shown sometimes never
/// reaches the screen: the window shows only its DWM blur, with no tint,
/// title, or items, until something repaints it, such as a hover. An
/// animation's later frames paint over a lost first frame, but a window
/// shown with one paint (an instant play, or a startup play whose frames the
/// UI thread was too busy to draw, as with `--debug` opening its window) kept
/// it (seen 2026-10-18 with Roll and `--debug`). The second paint costs one
/// redraw per show.
///
/// # Safety
/// `hwnd` is a live space window of this thread; call it outside any state
/// borrow.
pub(crate) unsafe fn show(hwnd: HWND) {
    // SAFETY: the caller's contract.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetTimer(Some(hwnd), REPAINT_TIMER_ID, REPAINT_DELAY_MS, None);
    }
}

/// Moves a top-level space between its two placements: above every window
/// (`--on-top`), or at the bottom, where `on_window_pos_changing` pins it as
/// at creation. A desktop-hosted space has only one placement.
pub(crate) fn set_on_top(hwnd: HWND, on_top: bool) {
    // SAFETY: `hwnd` is a live space window; the state borrow is short and
    // ends before SetWindowPos, whose WM_WINDOWPOSCHANGING reads the state.
    unsafe {
        let hosted = with_state(hwnd, |state| {
            state.on_top = on_top;
            state.hosted
        });
        if hosted != Some(false) {
            return;
        }
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        if on_top {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
        } else {
            let _ = SetWindowPos(hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0, flags);
            let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, flags);
        }
    }
}

/// Puts a desktop-hosted window below its sibling spaces. Windows creates a
/// child above its siblings, so a layer made after the hosted spaces (takeover
/// turned on from Settings) would cover them. A top-level window is already
/// placed by `on_window_pos_changing`.
pub(crate) fn sink_hosted(hwnd: HWND) {
    // SAFETY: `hwnd` is a live space window; the state borrow is short and
    // ends before SetWindowPos.
    unsafe {
        if with_state(hwnd, |state| state.hosted) != Some(true) {
            return;
        }
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, flags);
    }
}

pub(crate) fn set_rect(hwnd: HWND, rect: Rect) {
    // SAFETY: `hwnd` is a live space window, outside any state borrow.
    if unsafe { space_animation::retarget(hwnd, rect) } || current_rect(hwnd) == Some(rect) {
        return;
    }
    // SAFETY: `hwnd` is a live space window. Messages sent synchronously by
    // SetWindowPos (WM_WINDOWPOSCHANGING, WM_SIZE) do not touch the runtime.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            rect.x,
            rect.y,
            rect.width.max(1),
            rect.height.max(1),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// The window's rect in the coordinates `set_rect` takes: the parent's client
/// coordinates for a hosted child (Progman/WorkerW spans the desktop), screen
/// coordinates otherwise.
pub(super) fn current_rect(hwnd: HWND) -> Option<Rect> {
    let mut rect = RECT::default();
    // SAFETY: `hwnd` is a live space window; neither call sends messages.
    unsafe {
        GetWindowRect(hwnd, &mut rect).ok()?;
        if let Ok(parent) = GetParent(hwnd) {
            let mut corners = [
                POINT {
                    x: rect.left,
                    y: rect.top,
                },
                POINT {
                    x: rect.right,
                    y: rect.bottom,
                },
            ];
            MapWindowPoints(None, Some(parent), &mut corners);
            rect = RECT {
                left: corners[0].x,
                top: corners[0].y,
                right: corners[1].x,
                bottom: corners[1].y,
            };
        }
    }
    Some(Rect::new(
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    ))
}

pub(crate) fn set_scene(hwnd: HWND, scene: SpaceScene) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let unchanged = unsafe {
        with_state(hwnd, |state| {
            state
                .scene
                .as_ref()
                .is_some_and(|current| *current.scene() == scene)
        })
    };
    if unchanged == Some(true) {
        return;
    }
    let prepared = PreparedScene::new(scene);
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    unsafe {
        with_state(hwnd, |state| {
            // A folding window keeps drawing its unfolded scene until the
            // fold ends (`Animation::defer`).
            let prepared = match state.animation.as_mut() {
                Some(animation) => animation.defer(prepared),
                None => Some(prepared),
            };
            let Some(prepared) = prepared else {
                return;
            };
            if let Some(painter) = state.painter.as_mut() {
                painter.retain_icons(prepared.scene());
            }
            state.scene = Some(prepared);
        });
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

/// Sets the pointer shown over the space. The app calls this while handling
/// a mouse event on the space, so the cursor is over it (or captured by it)
/// and is updated at once. WM_SETCURSOR is not sent while the mouse is
/// captured, so waiting for it would leave the old shape during a drag.
pub(crate) fn set_cursor(hwnd: HWND, shape: CursorShape) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let changed = unsafe {
        with_state(hwnd, |state| {
            std::mem::replace(&mut state.cursor, shape) != shape
        })
    };
    if changed == Some(true) {
        apply_cursor(shape);
    }
}

fn apply_cursor(shape: CursorShape) {
    let id = match shape {
        CursorShape::Arrow => IDC_ARROW,
        CursorShape::SizeWestEast => IDC_SIZEWE,
        CursorShape::SizeNorthSouth => IDC_SIZENS,
        CursorShape::SizeNorthWestSouthEast => IDC_SIZENWSE,
        CursorShape::SizeNorthEastSouthWest => IDC_SIZENESW,
    };
    // SAFETY: stock cursors are shared system resources and are never freed.
    unsafe {
        if let Ok(cursor) = LoadCursorW(None, id) {
            SetCursor(Some(cursor));
        }
    }
}

pub(crate) fn set_style(hwnd: HWND, style: SpaceStyle) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter
    // (composition property changes send no window messages).
    let Some(Some((hosted, composition, backing))) = (unsafe {
        with_state(hwnd, |state| {
            if state.style == style {
                return None;
            }
            state.style = style;
            let backed = (!state.hosted).then(|| material(state.visuals.is_some()));
            state.frame = Frame::for_style(style, backed, is_windows_11());
            if let Some(visuals) = &mut state.visuals
                && let Err(error) = visuals.set_style(style)
            {
                tracing::warn!(%error, "space composition not restyled");
            }
            Some((state.hosted, state.visuals.is_some(), state.backing))
        })
    }) else {
        return;
    };
    if !hosted {
        dwm::apply_style(
            hwnd,
            style,
            material(composition),
            dwm::BorderOwner::Painter,
        );
        // A restyle mid-animation keeps what the animation stepped aside.
        if !composition && !backing.material {
            dwm::show_accent(hwnd, style.backdrop, false);
        }
    }
    // SAFETY: `hwnd` is a live space window.
    unsafe {
        update_region(hwnd);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
    sync_shadow(hwnd);
}

/// Brings the space's shadow in line with the window: its rect, whether it
/// shows, its look at the window's DPI, and the running frame's fade. It
/// makes the shadow window the first time one is cast, and changes only
/// what differs, so calling it again costs little.
pub(crate) fn sync_shadow(hwnd: HWND) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter
    // (reading the window's DPI sends no messages).
    let cast = unsafe {
        with_state(hwnd, |state| {
            let (look, opacity) = state.frame.shadow(window_dpi(hwnd))?;
            let strength = state
                .animation
                .as_ref()
                .map_or(1.0, |animation| animation.frame().shadow_strength());
            Some((look, opacity, strength))
        })
    }
    .flatten();
    // SAFETY: a plain visibility query on a live window.
    let shown = unsafe { IsWindowVisible(hwnd) }.as_bool();
    let placement =
        cast.filter(|_| shown)
            .zip(current_rect(hwnd))
            .map(|((look, opacity, strength), space)| ShadowPlacement {
                look,
                space,
                alpha: shadow_alpha(opacity, strength),
            });
    // Taken out while it syncs, so no borrow is held across its Win32 calls.
    // SAFETY: as above.
    let shadow = unsafe { with_state(hwnd, |state| state.shadow.take()) }.flatten();
    let Some(mut shadow) = shadow.or_else(|| placement.and_then(|_| ShadowWindow::create())) else {
        return;
    };
    shadow.sync(hwnd, placement);
    // SAFETY: as above.
    unsafe { with_state(hwnd, |state| state.shadow = Some(shadow)) };
}

/// The window `hwnd`'s shadow shows on, if it has cast one.
pub(crate) fn shadow_of(hwnd: HWND) -> Option<HWND> {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    unsafe { with_state(hwnd, |state| state.shadow.as_ref().map(ShadowWindow::hwnd)) }.flatten()
}

/// A shadow's whole opacity, 0 to 255: its setting, in percent, times how
/// strongly the frame casts it.
fn shadow_alpha(opacity: u8, strength: f32) -> u8 {
    (f32::from(opacity.min(100)) / 100.0 * strength.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Leaves `backing` of the window's DWM backing in place, stepping the rest
/// aside (`Backing`). Changes only what differs, and nothing for a
/// desktop-hosted window, which has none.
pub(crate) fn show_backing(hwnd: HWND, backing: Backing) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter
    // (composition property changes send no window messages).
    let change = unsafe {
        with_state(hwnd, |state| {
            if state.hosted {
                return None;
            }
            let before = std::mem::replace(&mut state.backing, backing);
            if before.material == backing.material {
                return None;
            }
            match &mut state.visuals {
                Some(visuals) => {
                    if let Err(error) = visuals.show_material(backing.material) {
                        tracing::warn!(%error, "space composition backdrop not changed");
                    }
                    None
                }
                None => Some(state.style),
            }
        })
    };
    if let Some(Some(style)) = change {
        dwm::show_accent(hwnd, style.backdrop, backing.material);
    }
}

/// Who draws a space's corners, and how its window is clipped. The painter
/// always draws the border (`dwm::BorderOwner::Painter`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    /// Desktop-hosted windows get no per-pixel alpha; paint them opaque.
    opaque: bool,
    /// The radius the painter rounds the body to; 0 where DWM rounds the
    /// window or the corners are square.
    radius: i32,
    /// The border's thickness at 96 DPI, 0 for none.
    border_width: i32,
    /// The radius DWM rounds the window to, at 96 DPI: 0 where it does not
    /// round (its own square corners, or Windows 10, which has none). DWM
    /// scales it with the window's DPI, so the border follows it there.
    dwm_radius: i32,
    /// Clip the window to a rounded region of this radius. Needed for opaque
    /// desktop-hosted windows, whose pixels outside the rounded corners
    /// would otherwise show. Regions are not anti-aliased, so the painter
    /// still draws smooth corners inside them. Accent-blurred top-level
    /// spaces use DWM's corners instead, because the blur material ignores
    /// regions (`Theme::effective_corner_radius`); composition spaces clip
    /// themselves in their visual tree (`dwm::Material::dwm_rounds`).
    region_radius: Option<i32>,
    /// The shadow the window draws itself (ROADMAP D15): the style's, unless
    /// DWM casts the Windows 11 shadow with its corners, or the space has
    /// no edge to cast from (Clear, chrome-less) or is desktop-hosted.
    shadow: Option<ShadowStyle>,
}

impl Frame {
    /// `material` is what draws the window's blur, `None` for a
    /// desktop-hosted window, which has none.
    fn for_style(style: SpaceStyle, material: Option<dwm::Material>, windows_11: bool) -> Self {
        let hosted = material.is_none();
        // A chrome-less window (the desktop layer) has no rounded corners or
        // border; it fills its whole rectangle.
        let radius = if style.chrome {
            style.corner_radius.max(0)
        } else {
            0
        };
        let border_width = if style.chrome && style.border {
            style.border_width.max(1)
        } else {
            0
        };
        let shadow = style
            .shadow
            .filter(|_| style.chrome && style.backdrop != Backdrop::Clear && !hosted);
        if material.is_some_and(dwm::Material::dwm_rounds) && style.dwm_draws_frame() {
            // `dwm_draws_frame` allows only radii DWM has (0, 4, 8).
            let dwm_radius = if windows_11 { radius } else { 0 };
            return Self {
                opaque: false,
                radius: 0,
                border_width,
                dwm_radius,
                region_radius: None,
                // DWM's rounded corners bring its own shadow.
                shadow: shadow.filter(|_| dwm_radius == 0),
            };
        }
        Self {
            opaque: hosted,
            radius,
            border_width,
            dwm_radius: 0,
            region_radius: (radius > 0 && hosted).then_some(radius),
            shadow,
        }
    }

    /// What the painter draws, for a window at `dpi`.
    fn paint(&self, dpi: u32) -> PaintFrame {
        let scale = scale_for(dpi);
        // Whole pixels keep the line crisp at any scale.
        let width = (self.border_width as f32 * scale).round().max(1.0);
        PaintFrame {
            opaque: self.opaque,
            radius: self.radius,
            border: (self.border_width > 0).then_some(BorderStroke {
                radius: self.outline_radius(scale),
                width,
            }),
        }
    }

    /// The shadow the window casts at `dpi`, and its opacity in percent.
    fn shadow(&self, dpi: u32) -> Option<(ShadowLook, u8)> {
        let shadow = self.shadow?;
        let scale = scale_for(dpi);
        let look = ShadowLook {
            reach: ((shadow.size as f32 * scale).round() as i32).max(1),
            radius: self.outline_radius(scale),
        };
        Some((look, shadow.opacity))
    }

    /// The radius of the window's visible corners at `scale`: DWM's, which
    /// it scales with the window's DPI, or the one the space rounds itself
    /// to.
    fn outline_radius(&self, scale: f32) -> f32 {
        if self.dwm_radius > 0 {
            self.dwm_radius as f32 * scale
        } else {
            self.radius as f32
        }
    }
}

fn ensure_class() -> Result<(), PlatformError> {
    // Without CS_DBLCLKS Windows never sends WM_LBUTTONDBLCLK, and a
    // double-click arrives as two WM_LBUTTONDOWNs.
    // https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttondblclk
    window_class::register(
        WindowClass::new(CLASS_NAME, Some(wndproc))
            .style(CS_DBLCLKS)
            .arrow_cursor(),
    )
}

/// Sizes the window's visual tree, if it has one, to its new client area.
unsafe fn fit_visuals(hwnd: HWND) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let fitted = unsafe {
        with_state(hwnd, |state| {
            state.visuals.as_ref().map(|visuals| visuals.fit(hwnd))
        })
    };
    if let Some(Some(Err(error))) = fitted {
        tracing::warn!(%error, "space composition not resized");
    }
}

/// Applies the frame's rounded window region, or removes it.
unsafe fn update_region(hwnd: HWND) {
    // SAFETY: `hwnd` is a live space window; the borrow does not re-enter.
    let Some(radius) = (unsafe { with_state(hwnd, |state| state.frame.region_radius) }) else {
        return;
    };
    let mut client = Default::default();
    // SAFETY: `hwnd` is live; the system owns the region after SetWindowRgn.
    unsafe {
        let Some(radius) = radius else {
            let _ = SetWindowRgn(hwnd, None, true);
            return;
        };
        if GetClientRect(hwnd, &mut client).is_err() {
            return;
        }
        // CreateRoundRectRgn excludes the right and bottom edges, hence +1.
        let region = CreateRoundRectRgn(
            0,
            0,
            client.right + 1,
            client.bottom + 1,
            radius * 2,
            radius * 2,
        );
        let _ = SetWindowRgn(hwnd, Some(region), true);
    }
}

/// Runs `f` with a short-lived borrow of the window's state. `f` must not call
/// anything that can re-enter `wndproc`.
pub(super) unsafe fn with_state<R>(
    window: HWND,
    f: impl FnOnce(&mut SpaceState) -> R,
) -> Option<R> {
    // SAFETY: space windows always hold a `SpaceState`; the caller rules out
    // re-entry.
    unsafe { window_state::with_state(window, f) }
}

/// Converts a client point to the space's desktop coordinates: screen
/// coordinates for top-level spaces, the host's client coordinates for hosted
/// ones.
unsafe fn to_desktop(window: HWND, lparam: LPARAM, hosted: bool) -> (i32, i32) {
    let mut point = [POINT {
        x: loword(lparam.0),
        y: hiword(lparam.0),
    }];
    // SAFETY: `window` is live; `point` is a valid one-element buffer.
    unsafe {
        let target = if hosted { GetParent(window).ok() } else { None };
        MapWindowPoints(Some(window), target, &mut point);
    }
    (point[0].x, point[0].y)
}

/// Ctrl and Shift as a mouse message reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Modifiers {
    ctrl: bool,
    shift: bool,
}

/// Reads the `MK_*` flags that every mouse message carries in the low word of
/// wParam (for `WM_MOUSEWHEEL`, GET_KEYSTATE_WPARAM).
///
/// `GetKeyState` cannot be used here: it reports the key state as of the last
/// keyboard message this thread read, and a space is never activated, so its
/// thread never sees Ctrl or Shift go down. That is why Ctrl+wheel did nothing
/// over a space.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
fn modifiers(wparam: WPARAM) -> Modifiers {
    let keys = (wparam.0 & 0xFFFF) as u32;
    Modifiers {
        ctrl: keys & MK_CONTROL.0 != 0,
        shift: keys & MK_SHIFT.0 != 0,
    }
}

fn button_for(message: u32) -> MouseButton {
    match message {
        WM_LBUTTONDOWN | WM_LBUTTONUP => MouseButton::Left,
        WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

unsafe fn paint(window: HWND) {
    // SAFETY: for every call, `window` is live and the state borrows are
    // short.
    unsafe {
        let checked_out = with_state(window, |state| {
            (
                state.painter.take(),
                state.scene.take(),
                state.frame.paint(window_dpi(window)),
                state.animation.as_ref().map(Animation::frame),
            )
        });
        let Some((Some(mut painter), scene, frame, transition)) = checked_out else {
            let _ = ValidateRect(Some(window), None);
            return;
        };
        let painted = painter.paint(window, scene.as_ref(), frame, transition.as_ref());
        if let Err(error) = &painted {
            tracing::error!(%error, "space paint failed");
        }
        // The painter drew the blur's mask with a frame that leaves the
        // window see-through; only then does the blur show through it.
        let masked = painted.is_ok()
            && transition
                .as_ref()
                .is_some_and(|transition| !transition.covers_window());
        with_state(window, |state| {
            state.painter = Some(painter);
            if state.scene.is_none() {
                state.scene = scene;
            }
            if let Some(visuals) = &mut state.visuals
                && let Err(error) = visuals.mask_blur(masked)
            {
                tracing::warn!(%error, "space composition backdrop not masked");
            }
        });
    }
}

/// Raises `event` for this window's space. Same contract as `with_state`.
unsafe fn space_event(window: HWND, event: SpaceEvent) {
    // A deleted space's window, still animating away, is not the app's any
    // more.
    if space_animation::is_dying(window) {
        return;
    }
    // SAFETY: the caller's contract; the borrow ends before the dispatch.
    if let Some(id) = unsafe { with_state(window, |state| state.id) } {
        runtime::dispatch(Event::Space { id, event });
    }
}

/// Whether the space is hosted in the desktop. Same contract as `with_state`.
unsafe fn is_hosted(window: HWND) -> bool {
    // SAFETY: the caller's contract.
    unsafe { with_state(window, |state| state.hosted) }.unwrap_or(false)
}

/// Keeps top-level spaces under every other window so they behave like part
/// of the desktop, and above the desktop layer (`insert_after`).
unsafe fn on_window_pos_changing(window: HWND, lparam: LPARAM) {
    // SAFETY: for WM_WINDOWPOSCHANGING lParam is null or a live WINDOWPOS the
    // system lets us edit; the state borrow is short.
    unsafe {
        let position = lparam.0 as *mut WINDOWPOS;
        let pinned = with_state(window, |state| {
            let shadow = state.shadow.as_ref().map(ShadowWindow::hwnd);
            (!state.hosted && !state.on_top).then_some(shadow)
        })
        .flatten();
        let Some(shadow) = pinned else {
            return;
        };
        if position.is_null() || (*position).flags.contains(SWP_NOZORDER) {
            return;
        }
        let layer = runtime::desktop_layer_window();
        // The space's own shadow sits right under it, so the space is
        // directly above the layer when only its shadow is between them.
        let above_layer = layer.and_then(window_above).and_then(|above| {
            if Some(above) == shadow {
                window_above(above)
            } else {
                Some(above)
            }
        });
        match insert_after(window, layer, above_layer) {
            Some(after) => (*position).hwndInsertAfter = after,
            None => (*position).flags |= SWP_NOZORDER,
        }
    }
}

/// Where a pinned top-level window goes when its z-order changes, as the
/// `hwndInsertAfter` to use, or `None` to stay where it is. `above_layer` is
/// the non-topmost window directly above the desktop `layer`, if any.
///
/// The layer and the spaces all live at the bottom, and the layer spans the
/// whole desktop. When every one of them went to `HWND_BOTTOM`, the last to
/// move ended up lowest, so an outside z-order change (Show desktop, a task
/// switch) could leave a space under the layer: still visible through it,
/// but its clicks and drops went to the layer, which refuses drops.
/// Spaces therefore go directly above the layer, which alone takes the
/// bottom.
fn insert_after(window: HWND, layer: Option<HWND>, above_layer: Option<HWND>) -> Option<HWND> {
    match layer {
        None => Some(HWND_BOTTOM),
        Some(layer) if layer == window => Some(HWND_BOTTOM),
        Some(_) => match above_layer {
            Some(above) if above == window => None,
            Some(above) => Some(above),
            // Only topmost windows are above the layer, so the top of the
            // non-topmost band is the slot directly above it.
            None => Some(HWND_TOP),
        },
    }
}

/// The window directly above `window` in the z-order, unless it is topmost.
/// Then `HWND_TOP` names the same slot for a non-topmost window, without
/// positioning it relative to a topmost one.
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos#remarks
pub(crate) fn window_above(window: HWND) -> Option<HWND> {
    // SAFETY: plain z-order and style queries; neither sends a message.
    unsafe {
        let above = GetWindow(window, GW_HWNDPREV).ok()?;
        let style = WINDOW_EX_STYLE(GetWindowLongW(above, GWL_EXSTYLE) as u32);
        (!style.contains(WS_EX_TOPMOST)).then_some(above)
    }
}

unsafe fn on_mouse_move(window: HWND, lparam: LPARAM) {
    // SAFETY: `window` is a live space window; the state borrows are short.
    unsafe {
        let start_tracking = with_state(window, |state| {
            !std::mem::replace(&mut state.tracking_leave, true)
        });
        if start_tracking == Some(true) {
            // Ask for WM_MOUSELEAVE so hover highlights clear when the cursor
            // leaves the space.
            // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-trackmouseevent
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: window,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);
        }
        let (x, y) = to_desktop(window, lparam, is_hosted(window));
        space_event(window, SpaceEvent::MouseMove { x, y });
    }
}

unsafe fn on_mouse_leave(window: HWND) {
    // SAFETY: `window` is a live space window; the state borrow is short.
    unsafe {
        with_state(window, |state| {
            state.tracking_leave = false;
            state.cursor = CursorShape::Arrow;
        });
        space_event(window, SpaceEvent::MouseLeave);
    }
}

unsafe fn on_mouse_wheel(window: HWND, wparam: WPARAM, lparam: LPARAM) {
    // Unlike other mouse messages, the wheel position is in screen
    // coordinates, and the delta is the signed high word of wParam
    // (GET_WHEEL_DELTA_WPARAM).
    // https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousewheel
    let delta = hiword(wparam.0 as isize);
    let mut point = [POINT {
        x: loword(lparam.0),
        y: hiword(lparam.0),
    }];
    let Modifiers { ctrl, .. } = modifiers(wparam);
    // SAFETY: `window` is a live space window and `point` a valid one-element
    // buffer.
    unsafe {
        if is_hosted(window) {
            MapWindowPoints(None, GetParent(window).ok(), &mut point);
        }
        space_event(
            window,
            SpaceEvent::Wheel {
                x: point[0].x,
                y: point[0].y,
                delta,
                ctrl,
            },
        );
    }
}

unsafe fn on_button_down(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) {
    let Modifiers { ctrl, shift } = modifiers(wparam);
    // SAFETY: `window` is a live space window.
    unsafe {
        // Capture so the matching button-up arrives even if the cursor leaves
        // the window mid-drag.
        // https://learn.microsoft.com/en-us/windows/win32/inputdev/about-mouse-input#mouse-capture
        SetCapture(window);
        let (x, y) = to_desktop(window, lparam, is_hosted(window));
        space_event(
            window,
            SpaceEvent::MouseDown {
                x,
                y,
                button: button_for(message),
                ctrl,
                shift,
            },
        );
    }
}

unsafe fn on_button_up(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) {
    // SAFETY: `window` is a live space window. ReleaseCapture is not tied to
    // the window, so it is sound even if the handler destroyed it.
    unsafe {
        let (x, y) = to_desktop(window, lparam, is_hosted(window));
        space_event(
            window,
            SpaceEvent::MouseUp {
                x,
                y,
                button: button_for(message),
            },
        );
        if wparam.0 & MK_ANY_BUTTON == 0 {
            let _ = ReleaseCapture();
        }
    }
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: called by Windows with valid arguments for `message`; state
    // access follows the rules on `with_state`.
    unsafe {
        match message {
            WM_NCCREATE => {
                window_state::attach_create_param::<SpaceState>(window, lparam);
                DefWindowProcW(window, message, wparam, lparam)
            }
            WM_PAINT => {
                paint(window);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            messages::FRAME => {
                space_animation::on_frame(window);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == REPAINT_TIMER_ID => {
                let _ = KillTimer(Some(window), REPAINT_TIMER_ID);
                let _ = InvalidateRect(Some(window), None, false);
                LRESULT(0)
            }
            WM_SIZE => {
                fit_visuals(window);
                update_region(window);
                let _ = InvalidateRect(Some(window), None, false);
                LRESULT(0)
            }
            WM_SYSCOMMAND if (wparam.0 as u32 & 0xFFF0) == SC_MINIMIZE => {
                // A space is part of the desktop and must stay on it: Show
                // Desktop (`MinimizeAll`) minimizes every normal window, and a
                // space has to survive with its blur. `WS_EX_TOOLWINDOW` makes
                // the shell skip tool windows, and this ignores the command if
                // one is sent directly, so survival never depends on that
                // undocumented exclusion. Minimizing would also tear down the
                // accent blur, which only survives while the window does.
                // https://learn.microsoft.com/en-us/windows/win32/menurc/wm-syscommand
                LRESULT(0)
            }
            WM_DPICHANGED => {
                // The suggested rectangle in lParam is deliberately ignored:
                // DeskSpace owns a space's rectangle in the model and reflows
                // it for the new DPI, so applying the system's default would
                // fight the model. LOWORD(wParam) is the new horizontal DPI.
                // https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows
                let dpi = loword(wparam.0 as isize).max(0) as u32;
                space_event(window, SpaceEvent::DpiChanged { dpi });
                LRESULT(0)
            }
            WM_WINDOWPOSCHANGING => {
                on_window_pos_changing(window, lparam);
                LRESULT(0)
            }
            WM_WINDOWPOSCHANGED => {
                // DefWindowProcW sends WM_SIZE and WM_MOVE from here.
                let result = DefWindowProcW(window, message, wparam, lparam);
                sync_shadow(window);
                result
            }
            // Clicking a space must not activate it (and pull it above other
            // apps); WS_EX_NOACTIVATE alone does not cover every click path.
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_MOUSEMOVE => {
                on_mouse_move(window, lparam);
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                on_mouse_leave(window);
                LRESULT(0)
            }
            WM_SETCURSOR if loword(lparam.0) == HTCLIENT as i32 => {
                let shape = with_state(window, |state| state.cursor).unwrap_or_default();
                apply_cursor(shape);
                LRESULT(1)
            }
            WM_MOUSEWHEEL => {
                on_mouse_wheel(window, wparam, lparam);
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDBLCLK => {
                on_button_down(window, message, wparam, lparam);
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                on_button_up(window, message, wparam, lparam);
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                let (x, y) = to_desktop(window, lparam, is_hosted(window));
                space_event(window, SpaceEvent::DoubleClick { x, y });
                LRESULT(0)
            }
            WM_DESTROY => {
                // Revoke while the window still exists so OLE releases its
                // reference to the drop target before the HWND is gone.
                drop_target::revoke(window);
                // The shadow goes with its space.
                drop(with_state(window, |state| state.shadow.take()));
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                // lParam is the window gaining capture. Our own ReleaseCapture
                // after the last button-up also lands here, after MouseUp has
                // been delivered, so a CaptureLost then is harmless.
                if lparam.0 != window.0 as isize {
                    space_event(window, SpaceEvent::CaptureLost);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                window_state::free::<SpaceState>(window);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(backdrop: deskspace_core::Backdrop, corner_radius: i32) -> SpaceStyle {
        SpaceStyle {
            backdrop,
            corner_radius,
            dark: false,
            border: true,
            border_width: 1,
            chrome: true,
            shadow: Some(SHADOW),
        }
    }

    fn stroke(radius: f32, width: f32) -> BorderStroke {
        BorderStroke { radius, width }
    }

    const ACCENT: Option<dwm::Material> = Some(dwm::Material::Accent);
    const COMPOSED: Option<dwm::Material> = Some(dwm::Material::Composition);
    const SHADOW: ShadowStyle = ShadowStyle {
        size: 16,
        opacity: 35,
    };

    #[test]
    fn a_composition_space_casts_its_own_shadow_at_its_corners() {
        let frame = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), COMPOSED, true);
        let (look, opacity) = frame.shadow(144).expect("composition casts no DWM shadow");
        assert_eq!(opacity, 35);
        assert_eq!(look.reach, 24, "the reach scales with the DPI");
        assert_eq!(look.radius, 8.0, "the radius the window clips itself to");
    }

    #[test]
    fn where_dwm_rounds_the_corners_its_own_shadow_stands() {
        let rounded = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), ACCENT, true);
        assert_eq!(rounded.shadow(96), None);
        // Square corners and Windows 10 bring no DWM shadow.
        let square = Frame::for_style(style(deskspace_core::Backdrop::Solid, 0), ACCENT, true);
        assert!(square.shadow(96).is_some());
        let ten = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), ACCENT, false);
        assert_eq!(ten.shadow(96).map(|(look, _)| look.radius), Some(0.0));
        // A radius DWM cannot draw: the space rounds itself, and casts.
        let painted = Frame::for_style(style(deskspace_core::Backdrop::Solid, 12), ACCENT, true);
        assert_eq!(painted.shadow(96).map(|(look, _)| look.radius), Some(12.0));
    }

    #[test]
    fn no_shadow_without_the_setting_an_edge_or_a_top_level_window() {
        let off = SpaceStyle {
            shadow: None,
            ..style(deskspace_core::Backdrop::Acrylic, 8)
        };
        assert_eq!(Frame::for_style(off, COMPOSED, true).shadow(96), None);
        let clear = style(deskspace_core::Backdrop::Clear, 8);
        assert_eq!(Frame::for_style(clear, COMPOSED, true).shadow(96), None);
        let layer = SpaceStyle {
            chrome: false,
            ..style(deskspace_core::Backdrop::Acrylic, 0)
        };
        assert_eq!(Frame::for_style(layer, COMPOSED, true).shadow(96), None);
        let hosted = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), None, true);
        assert_eq!(hosted.shadow(96), None);
    }

    #[test]
    fn a_shadow_fades_with_its_setting_and_its_frame() {
        assert_eq!(shadow_alpha(100, 1.0), 255);
        assert_eq!(shadow_alpha(35, 1.0), 89);
        assert_eq!(shadow_alpha(100, 0.5), 128);
        assert_eq!(shadow_alpha(35, 0.0), 0);
        assert_eq!(shadow_alpha(250, 2.0), 255, "both clamped");
    }

    #[test]
    fn a_composition_space_rounds_itself_where_dwm_would_have() {
        let composed = Some(dwm::Material::Composition);
        let frame = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), composed, true);
        let paint = frame.paint(144);
        assert!(!paint.opaque);
        assert_eq!((paint.radius, paint.border), (8, Some(stroke(8.0, 2.0))));
        assert_eq!(frame.region_radius, None);
    }

    #[test]
    fn the_border_thickness_scales_to_whole_pixels() {
        let thick = SpaceStyle {
            border_width: 3,
            ..style(deskspace_core::Backdrop::Clear, 0)
        };
        let frame = Frame::for_style(thick, ACCENT, true);
        assert_eq!(frame.paint(96).border.unwrap().width, 3.0);
        assert_eq!(frame.paint(120).border.unwrap().width, 4.0);
        assert_eq!(frame.paint(144).border.unwrap().width, 5.0);
    }

    #[test]
    fn where_dwm_rounds_the_border_follows_its_corners_at_the_windows_dpi() {
        let frame = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), ACCENT, true);
        assert_eq!(frame.paint(96).radius, 0);
        assert_eq!(frame.paint(96).border, Some(stroke(8.0, 1.0)));
        assert_eq!(frame.paint(144).border, Some(stroke(12.0, 2.0)));
        let small = Frame::for_style(style(deskspace_core::Backdrop::Blur, 4), ACCENT, true);
        assert_eq!(small.paint(120).border, Some(stroke(5.0, 1.0)));
    }

    #[test]
    fn windows_10_squares_the_painted_border_as_it_squares_the_window() {
        let frame = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), ACCENT, false);
        assert_eq!(frame.paint(144).border, Some(stroke(0.0, 2.0)));
    }

    #[test]
    fn a_painter_rounded_space_borders_at_its_own_radius() {
        let clear = Frame::for_style(style(deskspace_core::Backdrop::Clear, 12), ACCENT, true);
        let paint = clear.paint(144);
        assert_eq!((paint.radius, paint.border), (12, Some(stroke(12.0, 2.0))));
        let hosted = Frame::for_style(style(deskspace_core::Backdrop::Acrylic, 8), None, true);
        let paint = hosted.paint(96);
        assert!(paint.opaque);
        assert_eq!((paint.radius, paint.border), (8, Some(stroke(8.0, 1.0))));
        assert_eq!(hosted.region_radius, Some(8));
    }

    #[test]
    fn no_border_without_the_setting_or_chrome() {
        let off = SpaceStyle {
            border: false,
            ..style(deskspace_core::Backdrop::Acrylic, 8)
        };
        assert_eq!(Frame::for_style(off, ACCENT, true).paint(96).border, None);
        let layer = SpaceStyle {
            chrome: false,
            ..style(deskspace_core::Backdrop::Clear, 0)
        };
        assert_eq!(Frame::for_style(layer, ACCENT, true).paint(96).border, None);
    }

    #[test]
    fn modifiers_come_from_the_mouse_message() {
        let wheel_with_ctrl = WPARAM((120usize << 16) | MK_CONTROL.0 as usize);
        assert_eq!(
            modifiers(wheel_with_ctrl),
            Modifiers {
                ctrl: true,
                shift: false
            }
        );
        let click_with_both = WPARAM((MK_CONTROL.0 | MK_SHIFT.0) as usize);
        assert_eq!(
            modifiers(click_with_both),
            Modifiers {
                ctrl: true,
                shift: true
            }
        );
        let wheel_down_alone = WPARAM((-120i16 as u16 as usize) << 16);
        assert_eq!(
            modifiers(wheel_down_alone),
            Modifiers {
                ctrl: false,
                shift: false
            }
        );
    }

    fn window(id: usize) -> HWND {
        HWND(id as *mut core::ffi::c_void)
    }

    #[test]
    fn without_a_layer_a_space_goes_to_the_bottom() {
        assert_eq!(insert_after(window(1), None, None), Some(HWND_BOTTOM));
    }

    #[test]
    fn the_layer_itself_goes_to_the_bottom() {
        let layer = window(9);
        assert_eq!(
            insert_after(layer, Some(layer), Some(window(1))),
            Some(HWND_BOTTOM)
        );
    }

    #[test]
    fn a_space_goes_directly_above_the_layer() {
        let (space, other, layer) = (window(1), window(2), window(9));
        assert_eq!(insert_after(space, Some(layer), Some(other)), Some(other));
        // Already directly above the layer: it stays put.
        assert_eq!(insert_after(space, Some(layer), Some(space)), None);
        // Nothing but topmost windows above the layer.
        assert_eq!(insert_after(space, Some(layer), None), Some(HWND_TOP));
    }
}
