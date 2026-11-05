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

mod frame;
mod highlight;
mod mouse;
mod zorder;

use deskspace_core::geometry::Rect;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, MapWindowPoints, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GetParent, GetWindowRect, HTCLIENT,
    HWND_BOTTOM, HWND_TOPMOST, IsWindowVisible, KillTimer, LWA_ALPHA, MA_NOACTIVATE, SC_MINIMIZE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow, WINDOW_EX_STYLE,
    WM_CAPTURECHANGED, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEHWHEEL, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SETCURSOR, WM_SIZE, WM_SYSCOMMAND, WM_TIMER, WM_WINDOWPOSCHANGED,
    WM_WINDOWPOSCHANGING, WM_XBUTTONUP, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    XBUTTON1,
};
use windows::core::{PCWSTR, w};

use crate::error::PlatformError;
use crate::input::{Event, SpaceEvent};
use crate::render::{DropInto, Painter, PreparedScene, SpaceScene, SpaceStyle};
use crate::ui::CursorShape;
use crate::win32::composition::SpaceVisuals;
use crate::win32::dpi::window_dpi;
use crate::win32::shadow_window::{ShadowPlacement, ShadowWindow};
use crate::win32::space_animation::{self, Animation};
use crate::win32::system::{hiword, is_windows_11, loword};
use crate::win32::window_class::{self, WindowClass};
use crate::win32::{drop_target, dwm, frame_clock, messages, runtime, window_state};

use frame::{Frame, shadow_alpha, update_region};
pub(crate) use highlight::store_scene;
use mouse::{
    apply_cursor, on_button_down, on_button_up, on_mouse_leave, on_mouse_move, on_mouse_wheel,
    to_desktop,
};
use zorder::on_window_pos_changing;
pub(crate) use zorder::{set_on_top, sink, window_above};

const CLASS_NAME: PCWSTR = w!("DeskSpaceSpace");

/// The id of a space window's one-shot timer that paints it again after
/// `show`. It is set on the window itself, so it never reaches the app's
/// `Timer` ids; the value is kept from when animations had timer 1.
const REPAINT_TIMER_ID: usize = 2;
/// Long enough for DWM to have composed the shown window a few times.
const REPAINT_DELAY_MS: u32 = 100;

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

impl SpaceWindowOptions {
    /// The desktop layer's window, which spans every monitor. It is never
    /// topmost, whatever `--on-top` says: a topmost window that size covers
    /// every screen and takes every click, and on 2026-10-21 that left the PC
    /// unusable until the user signed out. A top-level space places itself
    /// above the layer anyway, so `--on-top` spaces still show.
    pub(crate) fn desktop_layer(
        rect: Rect,
        style: SpaceStyle,
        host: Option<HWND>,
        click_through: bool,
    ) -> Self {
        Self {
            id: 0,
            rect,
            style,
            host,
            click_through,
            on_top: false,
            visible: true,
            // The layer never animates, so it keeps the accent blur (T12).
            composition: false,
        }
    }
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
    /// The settings window's highlight outline and its fade.
    highlight: highlight::Highlight,
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
        highlight: highlight::Highlight::new(),
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
            let previous = state.scene.as_ref().map(|scene| scene.scene().highlight);
            state.highlight.follow(previous, prepared.scene(), hwnd);
            state.scene = Some(prepared);
        });
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

/// Sets the pointer shown over the space. The app calls this while handling
/// a mouse event on the space, so the cursor is over it (or captured by it)
/// and is updated at once. WM_SETCURSOR is not sent while the mouse is
/// captured, so waiting for it would leave the old shape during a drag.
pub(crate) use mouse::set_cursor;

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

unsafe fn paint(window: HWND) {
    // SAFETY: for every call, `window` is live and the state borrows are
    // short.
    unsafe {
        let checked_out = with_state(window, |state| {
            let mut frame = state.frame.paint(window_dpi(window));
            frame.highlight = state.highlight.level();
            (
                state.painter.take(),
                state.scene.take(),
                frame,
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
                frame_clock::delivered(window);
                space_animation::on_frame(window);
                highlight::on_frame(window);
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
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                on_mouse_wheel(window, wparam, lparam, message == WM_MOUSEHWHEEL);
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
            // The high word of wParam names the X button; a handled
            // WM_XBUTTONUP returns TRUE.
            // https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-xbuttonup
            WM_XBUTTONUP if hiword(wparam.0 as isize) == i32::from(XBUTTON1) => {
                space_event(window, SpaceEvent::BackButton);
                LRESULT(1)
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

/// What a drop on `window` lands in, from the scene the app last pushed. A
/// window with no scene yet takes drops as a space does.
pub(crate) fn drop_into(window: HWND) -> DropInto {
    // SAFETY: space windows and the desktop layer share this state; the
    // borrow does not re-enter.
    unsafe {
        with_state(window, |state: &mut SpaceState| {
            state
                .scene
                .as_ref()
                .map(|prepared| prepared.scene().drop_into)
        })
    }
    .flatten()
    .unwrap_or_default()
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
            shadow: None,
        }
    }

    #[test]
    fn the_desktop_layer_is_never_topmost() {
        let style = style(deskspace_core::Backdrop::Blur, 8);
        let layer =
            SpaceWindowOptions::desktop_layer(Rect::new(-1920, 0, 3840, 1080), style, None, false);
        assert!(!layer.on_top);
    }
}
