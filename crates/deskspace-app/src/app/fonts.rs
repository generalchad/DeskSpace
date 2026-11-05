//! `App`'s type scale: which `Typography` is in force, and handing it to the
//! platform. The saved one comes from the settings window's font pages; the
//! Debug window's typography box can override it for the session.

use deskspace_core::typography::Typography;
use deskspace_platform::Ui;

use super::App;

/// The type scale last handed to the platform. Each hand-off rebuilds every
/// DirectWrite format and repaints every space, so an unchanged scale is not
/// sent again.
#[derive(Debug, Default)]
pub(super) struct Fonts {
    shown: Option<Typography>,
}

impl App {
    /// The type scale in force: the Debug window's session override when it
    /// has one, else the saved one.
    pub(super) fn typography(&self) -> &Typography {
        in_force(
            &self.config.typography,
            self.debug
                .as_ref()
                .and_then(|tools| tools.typography.as_ref()),
        )
    }

    /// Hands the type scale in force to the platform when it changed since
    /// the last hand-off.
    pub(super) fn sync_typography(&mut self, ui: &mut Ui) {
        if self.fonts.shown.as_ref() == Some(self.typography()) {
            return;
        }
        let typography = self.typography().clone();
        ui.set_typography(&typography);
        self.fonts.shown = Some(typography);
    }
}

fn in_force<'a>(saved: &'a Typography, session: Option<&'a Typography>) -> &'a Typography {
    session.unwrap_or(saved)
}

#[cfg(test)]
mod tests {
    use deskspace_core::typography::{Font, Role};

    use super::*;

    #[test]
    fn a_session_override_wins_over_the_saved_scale() {
        let mut saved = Typography::new();
        saved.set(Role::Body, Font::new("Georgia", 13.0, 400));
        let mut session = Typography::new();
        session.set(Role::Body, Font::new("Consolas", 12.0, 400));
        assert_eq!(in_force(&saved, None), &saved);
        assert_eq!(in_force(&saved, Some(&session)), &session);
    }
}
