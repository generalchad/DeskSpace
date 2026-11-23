//! What Windows reports and DeskSpace follows: dark mode, accent, shell options, and animations.

use deskspace_core::Color;
use deskspace_platform::{
    ShellDisplay, system_accent_color, system_animations, system_uses_dark_mode,
};

/// What Windows reports and DeskSpace follows: dark mode, the accent color,
/// and Explorer's desktop display options.
#[derive(Debug)]
pub(crate) struct SystemLook {
    dark: bool,
    /// Windows' accent color; `None` when it cannot be read.
    accent: Option<Color>,
    /// Explorer's display options, refreshed by `App::refresh_shell_display`,
    /// which also reacts to their change.
    pub(super) shell: ShellDisplay,
    /// Windows' "Animation effects" switch; while it is off nothing animates
    /// (ROADMAP D12).
    animations: bool,
}

impl SystemLook {
    /// Reads dark mode and the accent now. Explorer's display is read once
    /// OLE is up, on `Started`.
    pub(crate) fn read() -> Self {
        Self {
            dark: system_uses_dark_mode(),
            accent: system_accent_color(),
            shell: ShellDisplay::default(),
            animations: system_animations(),
        }
    }

    /// Re-reads dark mode and the accent after a Windows theme change.
    /// Returns whether either changed.
    pub(in crate::app) fn refresh(&mut self) -> bool {
        let dark = std::mem::replace(&mut self.dark, system_uses_dark_mode());
        let accent = std::mem::replace(&mut self.accent, system_accent_color());
        (dark, accent) != (self.dark, self.accent)
    }

    pub(in crate::app) fn dark(&self) -> bool {
        self.dark
    }

    pub(in crate::app) fn animations(&self) -> bool {
        self.animations
    }

    /// Re-reads Windows' "Animation effects" switch after it changed.
    pub(in crate::app) fn refresh_animations(&mut self) {
        self.animations = system_animations();
    }

    pub(in crate::app) fn accent(&self) -> Option<Color> {
        self.accent
    }

    pub(in crate::app) fn shell(&self) -> &ShellDisplay {
        &self.shell
    }

    pub(in crate::app) fn set_shell(&mut self, shell: ShellDisplay) {
        self.shell = shell;
    }

    #[cfg(test)]
    pub(in crate::app) fn set_accent(&mut self, accent: Option<Color>) {
        self.accent = accent;
    }
}
