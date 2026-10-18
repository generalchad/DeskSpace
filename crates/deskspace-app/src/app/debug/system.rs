//! The debug window's System section: what Windows reports and what
//! DeskSpace hooks, read anew every refresh.

use deskspace_platform::{Ui, system_accent_color, system_uses_dark_mode, transparency_enabled};

use crate::app::App;
use crate::settings::HOTKEY_BINDINGS;

impl App {
    /// Windows' color mode, accent, Animation effects, and transparency.
    pub(super) fn theme_text(&self) -> String {
        let accent = system_accent_color()
            .map(|color| format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b))
            .unwrap_or_else(|| "unknown".to_string());
        let transparency = match transparency_enabled() {
            Some(true) => "on",
            Some(false) => "off",
            None => "unknown",
        };
        format!(
            "Windows mode: {}\nAccent: {accent}\nAnimation effects: {}\nTransparency effects: {transparency}",
            if system_uses_dark_mode() {
                "dark"
            } else {
                "light"
            },
            on_off(self.system.animations()),
        )
    }

    /// What DeskSpace last read from Windows and Explorer, as it holds it.
    pub(super) fn explorer_text(&self) -> String {
        format!("{:#?}", self.system)
    }

    /// Each global hotkey: its binding and whether Windows accepted it.
    pub(super) fn hotkeys_text(&self) -> String {
        HOTKEY_BINDINGS
            .iter()
            .enumerate()
            .map(|(slot, binding)| {
                let text = binding.text(&self.config.window);
                let state = match (text.is_empty(), self.hotkeys.is_registered(slot)) {
                    (true, _) => "unbound".to_string(),
                    (false, true) => format!("{text} (registered)"),
                    (false, false) => format!("{text} (not registered)"),
                };
                format!("{}: {state}", binding.label)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The keyboard and mouse hooks' flags.
    pub(super) fn hooks_text(&self, ui: &Ui) -> String {
        let hooks = ui.hook_state();
        format!(
            "Editing: {}\nWatch selection: {}\nWatch space keys: {}\nOutside clicks: {}",
            on_off(hooks.editing),
            on_off(hooks.watch_selection),
            on_off(hooks.watch_paste),
            on_off(hooks.outside_clicks),
        )
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}
