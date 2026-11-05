//! Turns a space window's events into the app's pointer gestures.

use deskspace_core::SpaceId;
use deskspace_core::geometry::Point;
use deskspace_platform::{MouseButton, SpaceEvent, Ui};

use crate::app::wheel::WheelInput;

use super::{App, Surface};

impl App {
    pub(in crate::app) fn on_space_event(&mut self, ui: &mut Ui, id: SpaceId, event: SpaceEvent) {
        let surface = Surface::of(id);
        if !self.prepare_space_event(ui, surface, &event) {
            return;
        }
        match event {
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ctrl,
                shift,
            } => self.begin_interaction(ui, surface, Point::new(x, y), ctrl, shift),
            SpaceEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
                ..
            } => self.begin_right_press(ui, surface, Point::new(x, y)),
            SpaceEvent::MouseMove { x, y } => self.on_pointer_move(ui, surface, Point::new(x, y)),
            SpaceEvent::Wheel {
                x,
                y,
                delta,
                ctrl,
                shift,
                tilt,
            } => {
                self.on_wheel(
                    ui,
                    surface,
                    WheelInput {
                        point: Point::new(x, y),
                        delta,
                        ctrl,
                        shift,
                        tilt,
                    },
                );
            }
            SpaceEvent::MouseUp {
                button: MouseButton::Left,
                ..
            } => self.end_drag(ui),
            SpaceEvent::CaptureLost => self.on_capture_lost(ui),
            SpaceEvent::MouseUp {
                x,
                y,
                button: MouseButton::Right,
            } => self.end_right_press(ui, surface, Point::new(x, y)),
            SpaceEvent::DoubleClick { x, y } => self.on_double_click(ui, surface, Point::new(x, y)),
            SpaceEvent::BackButton => self.on_back_button(ui, surface),
            SpaceEvent::MouseLeave => self.on_pointer_leave(ui, surface),
            SpaceEvent::FilesDropped {
                x,
                y,
                paths,
                action,
            } => self.on_files_dropped(ui, surface, Point::new(x, y), paths, action),
            SpaceEvent::ShellItemsDropped { x, y, items } => {
                self.on_shell_items_dropped(ui, surface, Point::new(x, y), items);
            }
            SpaceEvent::DpiChanged { .. } => self.on_surface_dpi_changed(ui, surface),
            _ => {}
        }
    }

    /// Prepares a space event, or returns false when its surface is gone. It
    /// commits a rename under the click and makes a press act like a desktop click.
    fn prepare_space_event(&mut self, ui: &mut Ui, surface: Surface, event: &SpaceEvent) -> bool {
        // The chrome-less desktop layer reuses the space window: id 0 is it,
        // any other id a real space. A space window can outlive its space by
        // one queued event, so ignore anything they report.
        let exists = match surface {
            Surface::Space(id) => self.config.desktop.space(id).is_some(),
            Surface::Desktop => self.layer_active(),
        };
        if !exists {
            return false;
        }
        // Clicking anywhere commits a rename in progress before the click is
        // handled, as clicking away from a title does elsewhere; so does the
        // mouse's back button, which empties the lens the rename is in. A
        // press on the title icon button opens the icon picker and leaves the
        // rename open behind it (D16).
        if self.edit.is_active()
            && matches!(
                event,
                SpaceEvent::MouseDown { .. }
                    | SpaceEvent::DoubleClick { .. }
                    | SpaceEvent::BackButton
            )
        {
            if self.title_icon_pressed(ui, surface, event) {
                return false;
            }
            self.finish_rename(ui, true);
        }
        // A space is part of the desktop, and never takes activation, so a
        // click here is made to behave like one on the desktop: focus it and
        // clear its icon selection.
        if matches!(event, SpaceEvent::MouseDown { .. }) {
            // A new press starts over; one on the Back button sets it again.
            self.pointer.back_press = None;
            self.dismiss_info_tip(ui);
            self.refresh_shell_display(ui);
            ui.on_desktop_click();
        }
        true
    }
}
