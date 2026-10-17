//! `App` effects (R17): what an event means, as plain data, and `App::run`,
//! the thin shell that carries it out against `Ui`.
//!
//! A decision is a method that changes `App`'s own state and returns
//! `Vec<Effect>` without touching `Ui`, so a test asserts the effects an
//! event produces. Queries the decision needs (`ui.is_zoom_bar_hovered()`)
//! are read by the caller and passed in. `run` executes the list in order,
//! inside the same `on_event` call that produced it, so the runtime's
//! re-entrancy rules (`ENGINEERING.md` section 6) hold exactly as for a
//! direct call. Grow the enum one event family at a time.

use deskspace_core::Touched;
use deskspace_platform::{Ui, set_desktop_icon_size};

use super::{App, Timer};

/// One thing an event asks the shell to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Effect {
    /// Repaint the spaces and the desktop layer a command changed.
    Refresh(Touched),
    /// Re-flow every space and repaint the desktop layer.
    RelayoutAll,
    /// Defer a configuration write (`App::schedule_save`).
    ScheduleSave,
    /// Push the current form to an open settings window.
    RefreshSettings,
    /// Re-read Explorer's display options and repaint if they changed.
    RefreshShellDisplay,
    /// Show the zoom pop-up. It carries no data: the pop-up shows the size
    /// read after the earlier effects ran, because Explorer may refuse or
    /// round a `SetDesktopIconSize`.
    ShowZoomBar,
    HideZoomBar,
    SetTimer(Timer, u32),
    ClearTimer(Timer),
    /// Ask Explorer to set the desktop's own icon size, in pixels.
    SetDesktopIconSize(i32),
}

impl App {
    /// Carries out `effects` in order.
    pub(super) fn run(&mut self, ui: &mut Ui, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Refresh(touched) => {
                    // A deleted space is gone; `refresh_space` skips it.
                    for id in touched.spaces {
                        self.refresh_space(ui, id);
                    }
                    if touched.layer {
                        self.refresh_desktop_layer(ui);
                    }
                }
                Effect::RelayoutAll => self.relayout_all(ui),
                Effect::ScheduleSave => self.schedule_save(ui),
                Effect::RefreshSettings => self.refresh_settings(ui),
                Effect::RefreshShellDisplay => self.refresh_shell_display(ui),
                Effect::ShowZoomBar => self.show_zoom_bar(ui),
                Effect::HideZoomBar => ui.hide_zoom_bar(),
                Effect::SetTimer(timer, millis) => Self::set_timer(ui, timer, millis),
                Effect::ClearTimer(timer) => Self::clear_timer(ui, timer),
                // A refused size leaves the desktop as it was; the
                // `RefreshShellDisplay` that follows reads whichever size
                // Explorer has.
                Effect::SetDesktopIconSize(pixels) => {
                    set_desktop_icon_size(pixels);
                }
            }
        }
    }
}
