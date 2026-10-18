//! TASKS.md T1 (ROADMAP D12): does a `Windows.UI.Composition` host backdrop
//! blur a window that is never activated, as spaces never are, and can that
//! blur be faded, swept, and dissolved? A throwaway check, never shipped;
//! delete it with T12-T14 if the answer is no.
//!
//! Run it with `cargo run -p deskspace-platform --example backdrop_spike`
//! over a high-contrast window. The window is never activated, so it takes
//! no keys; these global hotkeys drive it instead:
//!
//! - Ctrl+Alt+Shift+F: fade the whole window out and back (opacity).
//! - Ctrl+Alt+Shift+G: sweep a soft edge down over the blur and back.
//! - Ctrl+Alt+Shift+M: the same sweep through an effect brush (T13).
//! - Ctrl+Alt+Shift+D: dissolve the blur cell by cell and back.
//! - Ctrl+Alt+Shift+T: toggle topmost.
//! - Ctrl+Alt+Shift+Q: quit.
//!
//! Found while building it: `CompositionMaskBrush` refuses a host backdrop
//! brush as its source ("Unsupported source brush type"), so G sweeps thin
//! strips of backdrop at graded opacities. M is the way T13 chose to mask
//! the blur itself: a `CompositionEffectBrush` whose graph is Direct2D's
//! AlphaMask, with the backdrop as its source and a gradient as its mask.

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    spike::run()
}

#[cfg(windows)]
mod spike {
    use std::cell::RefCell;

    use windows::Foundation::IPropertyValue;
    use windows::Graphics::Effects::{
        IGraphicsEffect, IGraphicsEffect_Impl, IGraphicsEffectSource, IGraphicsEffectSource_Impl,
    };
    use windows::UI::Color;
    use windows::UI::Composition::{
        CompositionBackdropBrush, CompositionEffectSourceParameter, CompositionLinearGradientBrush,
        Compositor, ContainerVisual, Desktop::DesktopWindowTarget, SpriteVisual,
    };
    use windows::Win32::Foundation::{E_INVALIDARG, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Direct2D::CLSID_D2D1AlphaMask;
    use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
    use windows::Win32::System::WinRT::Graphics::Direct2D::{
        GRAPHICS_EFFECT_PROPERTY_MAPPING, IGraphicsEffectD2D1Interop,
        IGraphicsEffectD2D1Interop_Impl,
    };
    use windows::Win32::System::WinRT::{
        CreateDispatcherQueueController, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT,
        DispatcherQueueOptions, RO_INIT_SINGLETHREADED, RoInitialize,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_NOTOPMOST,
        HWND_TOPMOST, KillTimer, MA_NOACTIVATE, MSG, PostQuitMessage, RegisterClassW,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetTimer, SetWindowPos, WM_DESTROY, WM_HOTKEY,
        WM_MOUSEACTIVATE, WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
        WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };
    use windows::core::{BOOL, GUID, HSTRING, Interface, PCWSTR, Result, w};
    use windows_core::implement;
    use windows_numerics::{Vector2, Vector3};

    const WIDTH: f32 = 480.0;
    const HEIGHT: f32 = 320.0;
    const CORNER: f32 = 12.0;
    /// The dissolve grid.
    const COLS: usize = 24;
    const ROWS: usize = 16;
    /// The sweep's strips, and how much of the height its soft edge spans.
    const STRIPS: usize = 80;
    const FEATHER: f32 = 0.25;
    /// `DWMWA_USE_HOSTBACKDROPBRUSH` (Windows 11): lets the window's
    /// composition tree use a host backdrop brush.
    /// https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
    const DWMWA_USE_HOSTBACKDROPBRUSH: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(17);
    const TIMER: usize = 1;
    /// Each demo runs out for this many ticks and back for as many.
    const HALF_TICKS: u32 = 60;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Demo {
        Fade,
        Sweep,
        MaskedSweep,
        Dissolve,
    }

    /// The backdrop masked by an effect graph, for MaskedSweep.
    struct Masked {
        sprite: SpriteVisual,
        /// Transparent above the edge, opaque below; moved per frame.
        mask: CompositionLinearGradientBrush,
    }

    struct Spike {
        window: HWND,
        root: ContainerVisual,
        /// The backdrop in horizontal strips, for Fade and Sweep.
        strips: ContainerVisual,
        strip_visuals: Vec<SpriteVisual>,
        /// `None` when the compositor refused the effect graph.
        masked: Option<Masked>,
        /// The same backdrop in cells, shown only for Dissolve.
        cells: ContainerVisual,
        cell_visuals: Vec<SpriteVisual>,
        /// The order cells dissolve in.
        order: Vec<usize>,
        topmost: bool,
        running: Option<(Demo, u32)>,
        // Kept alive for as long as the window shows the tree.
        _target: DesktopWindowTarget,
        _compositor: Compositor,
    }

    thread_local! {
        static SPIKE: RefCell<Option<Spike>> = const { RefCell::new(None) };
    }

    pub fn run() -> Result<()> {
        // SAFETY: plain Win32 and WinRT calls on this thread, in order; every
        // pointer passed points at a live local.
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED)?;
            // A composition tree needs a dispatcher queue on its thread.
            let _queue = CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_STA,
            })?;
            let window = create_window()?;
            let host = BOOL::from(true);
            let attribute = DwmSetWindowAttribute(
                window,
                DWMWA_USE_HOSTBACKDROPBRUSH,
                &host as *const BOOL as *const core::ffi::c_void,
                std::mem::size_of::<BOOL>() as u32,
            );
            println!("DWMWA_USE_HOSTBACKDROPBRUSH: {attribute:?}");
            let spike = build(window)?;
            SPIKE.with(|cell| *cell.borrow_mut() = Some(spike));
            register_hotkeys(window);
            println!(
                "Ctrl+Alt+Shift+ F fade, G sweep, M masked sweep, D dissolve, \
                 T topmost, Q quit. \
                 Never click into the window to activate it: it must blur without."
            );
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                DispatchMessageW(&message);
            }
        }
        Ok(())
    }

    unsafe fn create_window() -> Result<HWND> {
        // SAFETY: plain window class registration and creation.
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("DeskSpaceBackdropSpike");
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            });
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP,
                class,
                w!("Backdrop spike"),
                WS_POPUP | WS_VISIBLE,
                200,
                200,
                WIDTH as i32,
                HEIGHT as i32,
                None,
                None,
                Some(instance.into()),
                None,
            )
        }
    }

    fn build(window: HWND) -> Result<Spike> {
        let compositor = Compositor::new()?;
        let interop: ICompositorDesktopInterop = compositor.cast()?;
        // SAFETY: `window` is our live top-level window.
        let target = unsafe { interop.CreateDesktopWindowTarget(window, false)? };
        let root = compositor.CreateContainerVisual()?;
        root.SetSize(Vector2::new(WIDTH, HEIGHT))?;
        target.SetRoot(&root)?;

        let corners = compositor.CreateRoundedRectangleGeometry()?;
        corners.SetSize(Vector2::new(WIDTH, HEIGHT))?;
        corners.SetCornerRadius(Vector2::new(CORNER, CORNER))?;
        root.SetClip(&compositor.CreateGeometricClipWithGeometry(&corners)?)?;

        let blur = compositor.CreateHostBackdropBrush()?;

        let strips = compositor.CreateContainerVisual()?;
        strips.SetSize(Vector2::new(WIDTH, HEIGHT))?;
        let strip_height = HEIGHT / STRIPS as f32;
        let mut strip_visuals = Vec::with_capacity(STRIPS);
        for row in 0..STRIPS {
            let strip = compositor.CreateSpriteVisual()?;
            strip.SetSize(Vector2::new(WIDTH, strip_height))?;
            strip.SetOffset(Vector3::new(0.0, row as f32 * strip_height, 0.0))?;
            strip.SetBrush(&blur)?;
            strips.Children()?.InsertAtTop(&strip)?;
            strip_visuals.push(strip);
        }
        root.Children()?.InsertAtTop(&strips)?;

        let cells = compositor.CreateContainerVisual()?;
        cells.SetSize(Vector2::new(WIDTH, HEIGHT))?;
        cells.SetIsVisible(false)?;
        let (cell_width, cell_height) = (WIDTH / COLS as f32, HEIGHT / ROWS as f32);
        let mut cell_visuals = Vec::with_capacity(COLS * ROWS);
        for row in 0..ROWS {
            for col in 0..COLS {
                let cell = compositor.CreateSpriteVisual()?;
                cell.SetSize(Vector2::new(cell_width, cell_height))?;
                cell.SetOffset(Vector3::new(
                    col as f32 * cell_width,
                    row as f32 * cell_height,
                    0.0,
                ))?;
                cell.SetBrush(&blur)?;
                cells.Children()?.InsertAtTop(&cell)?;
                cell_visuals.push(cell);
            }
        }
        root.Children()?.InsertAtTop(&cells)?;

        let masked = match build_masked(&compositor, &blur) {
            Ok(masked) => {
                root.Children()?.InsertAtTop(&masked.sprite)?;
                Some(masked)
            }
            Err(error) => {
                println!("The effect brush failed, so M does nothing: {error:?}");
                None
            }
        };

        // A translucent tint over the blur, as a space draws its own.
        let tint = compositor.CreateSpriteVisual()?;
        tint.SetSize(Vector2::new(WIDTH, HEIGHT))?;
        tint.SetBrush(&compositor.CreateColorBrushWithColor(color(0x50, 0x20, 0x20, 0x20))?)?;
        root.Children()?.InsertAtTop(&tint)?;

        let order = shuffled(COLS * ROWS);
        Ok(Spike {
            window,
            root,
            strips,
            strip_visuals,
            masked,
            cells,
            cell_visuals,
            order,
            topmost: false,
            running: None,
            _target: target,
            _compositor: compositor,
        })
    }

    fn build_masked(compositor: &Compositor, blur: &CompositionBackdropBrush) -> Result<Masked> {
        let effect: IGraphicsEffect = AlphaMask::new(["backdrop", "mask"])?.into();
        let brush = compositor.CreateEffectFactory(&effect)?.CreateBrush()?;
        brush.SetSourceParameter(&HSTRING::from("backdrop"), blur)?;

        // Relative coordinates, so the start and end points are fractions
        // of the height; past the ends the gradient holds its end colors.
        let mask = compositor.CreateLinearGradientBrush()?;
        mask.SetStartPoint(Vector2::new(0.0, 0.0))?;
        mask.SetEndPoint(Vector2::new(0.0, 1.0))?;
        let stops = mask.ColorStops()?;
        stops.Append(
            &compositor.CreateColorGradientStopWithOffsetAndColor(0.0, color(0, 0, 0, 0))?,
        )?;
        stops.Append(
            &compositor.CreateColorGradientStopWithOffsetAndColor(1.0, color(0xFF, 0, 0, 0))?,
        )?;
        brush.SetSourceParameter(&HSTRING::from("mask"), &mask)?;

        let sprite = compositor.CreateSpriteVisual()?;
        sprite.SetSize(Vector2::new(WIDTH, HEIGHT))?;
        sprite.SetBrush(&brush)?;
        sprite.SetIsVisible(false)?;
        Ok(Masked { sprite, mask })
    }

    /// Direct2D's AlphaMask effect described to the compositor: the first
    /// source's pixels times the second's alpha. Composition takes effect
    /// graphs only through these interfaces; Win2D usually supplies them.
    /// https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.effects.interop/nn-windows-graphics-effects-interop-igraphicseffectd2d1interop
    /// https://learn.microsoft.com/en-us/windows/win32/direct2d/alpha-mask-effect
    #[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
    struct AlphaMask {
        name: RefCell<HSTRING>,
        sources: [IGraphicsEffectSource; 2],
    }

    impl AlphaMask {
        fn new([source, mask]: [&str; 2]) -> Result<Self> {
            let parameter = |name: &str| -> Result<IGraphicsEffectSource> {
                CompositionEffectSourceParameter::Create(&HSTRING::from(name))?.cast()
            };
            Ok(Self {
                name: RefCell::new(HSTRING::from("AlphaMask")),
                sources: [parameter(source)?, parameter(mask)?],
            })
        }
    }

    impl IGraphicsEffect_Impl for AlphaMask_Impl {
        fn Name(&self) -> Result<HSTRING> {
            Ok(self.name.borrow().clone())
        }

        fn SetName(&self, name: &HSTRING) -> Result<()> {
            *self.name.borrow_mut() = name.clone();
            Ok(())
        }
    }

    impl IGraphicsEffectSource_Impl for AlphaMask_Impl {}

    impl IGraphicsEffectD2D1Interop_Impl for AlphaMask_Impl {
        fn GetEffectId(&self) -> Result<GUID> {
            Ok(CLSID_D2D1AlphaMask)
        }

        // AlphaMask has no properties to animate or map.
        fn GetNamedPropertyMapping(
            &self,
            _name: &PCWSTR,
            _index: *mut u32,
            _mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
        ) -> Result<()> {
            Err(E_INVALIDARG.into())
        }

        fn GetPropertyCount(&self) -> Result<u32> {
            Ok(0)
        }

        fn GetProperty(&self, _index: u32) -> Result<IPropertyValue> {
            Err(E_INVALIDARG.into())
        }

        fn GetSource(&self, index: u32) -> Result<IGraphicsEffectSource> {
            self.sources
                .get(index as usize)
                .cloned()
                .ok_or_else(|| E_INVALIDARG.into())
        }

        fn GetSourceCount(&self) -> Result<u32> {
            Ok(self.sources.len() as u32)
        }
    }

    fn color(a: u8, r: u8, g: u8, b: u8) -> Color {
        Color {
            A: a,
            R: r,
            G: g,
            B: b,
        }
    }

    /// 0..count in a fixed scrambled order (a multiplicative step coprime
    /// with `count`), so no random number crate is needed.
    fn shuffled(count: usize) -> Vec<usize> {
        let step = (1..count)
            .rev()
            .find(|step| gcd(*step, count) == 1 && *step < count * 2 / 3)
            .unwrap_or(1);
        (0..count).map(|index| (index * step + 7) % count).collect()
    }

    fn gcd(a: usize, b: usize) -> usize {
        if b == 0 { a } else { gcd(b, a % b) }
    }

    unsafe fn register_hotkeys(window: HWND) {
        let modifiers: HOT_KEY_MODIFIERS = MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT;
        for key in *b"FGMDTQ" {
            // SAFETY: registering hotkeys for our own window.
            if unsafe { RegisterHotKey(Some(window), i32::from(key), modifiers, u32::from(key)) }
                .is_err()
            {
                println!("Ctrl+Alt+Shift+{} is taken by another app", key as char);
            }
        }
    }

    fn on_hotkey(key: u8) {
        SPIKE.with(|cell| {
            let mut spike = cell.borrow_mut();
            let Some(spike) = spike.as_mut() else {
                return;
            };
            let demo = match key {
                b'F' => Demo::Fade,
                b'G' => Demo::Sweep,
                b'M' if spike.masked.is_some() => Demo::MaskedSweep,
                b'D' => Demo::Dissolve,
                b'T' => {
                    spike.topmost = !spike.topmost;
                    // SAFETY: re-ordering our own window.
                    unsafe {
                        let _ = SetWindowPos(
                            spike.window,
                            Some(if spike.topmost {
                                HWND_TOPMOST
                            } else {
                                HWND_NOTOPMOST
                            }),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                    return;
                }
                // SAFETY: ending our own message loop.
                b'Q' => return unsafe { PostQuitMessage(0) },
                _ => return,
            };
            let _ = spike.cells.SetIsVisible(demo == Demo::Dissolve);
            let _ = spike
                .strips
                .SetIsVisible(matches!(demo, Demo::Fade | Demo::Sweep));
            if let Some(masked) = &spike.masked {
                let _ = masked.sprite.SetIsVisible(demo == Demo::MaskedSweep);
            }
            spike.running = Some((demo, 0));
            // SAFETY: a timer on our own window.
            unsafe {
                let _ = SetTimer(Some(spike.window), TIMER, 16, None);
            }
        });
    }

    fn on_timer() {
        SPIKE.with(|cell| {
            let mut spike = cell.borrow_mut();
            let Some(spike) = spike.as_mut() else {
                return;
            };
            let Some((demo, tick)) = spike.running else {
                return;
            };
            // 1 shown, 0 gone: out for HALF_TICKS, back for as many.
            let shown = if tick < HALF_TICKS {
                1.0 - tick as f32 / HALF_TICKS as f32
            } else {
                (tick - HALF_TICKS) as f32 / HALF_TICKS as f32
            };
            let _ = step(spike, demo, shown);
            if tick >= 2 * HALF_TICKS {
                spike.running = None;
                // SAFETY: our own window's timer.
                unsafe {
                    let _ = KillTimer(Some(spike.window), TIMER);
                }
            } else {
                spike.running = Some((demo, tick + 1));
            }
        });
    }

    fn step(spike: &Spike, demo: Demo, shown: f32) -> Result<()> {
        match demo {
            Demo::Fade => spike.root.SetOpacity(shown),
            Demo::Sweep => {
                // Hidden above `position - FEATHER`, fading in down to
                // `position`, shown below it.
                let position = (1.0 - shown) * (1.0 + FEATHER);
                for (row, strip) in spike.strip_visuals.iter().enumerate() {
                    let y = (row as f32 + 0.5) / STRIPS as f32;
                    let opacity = ((y - (position - FEATHER)) / FEATHER).clamp(0.0, 1.0);
                    strip.SetOpacity(opacity)?;
                }
                Ok(())
            }
            Demo::MaskedSweep => {
                let Some(masked) = &spike.masked else {
                    return Ok(());
                };
                let position = (1.0 - shown) * (1.0 + FEATHER);
                masked
                    .mask
                    .SetStartPoint(Vector2::new(0.0, position - FEATHER))?;
                masked.mask.SetEndPoint(Vector2::new(0.0, position))
            }
            Demo::Dissolve => {
                let gone = ((1.0 - shown) * spike.order.len() as f32).round() as usize;
                for (turn, index) in spike.order.iter().enumerate() {
                    spike.cell_visuals[*index].SetIsVisible(turn >= gone)?;
                }
                Ok(())
            }
        }
    }

    unsafe extern "system" fn wndproc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            // Clicking must not activate the window: the point is that the
            // blur renders on a window that never is.
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_HOTKEY => {
                on_hotkey(wparam.0 as u8);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER => {
                on_timer();
                LRESULT(0)
            }
            WM_DESTROY => {
                // SAFETY: ending our own message loop.
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            // SAFETY: default handling for everything else.
            _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
        }
    }
}
