//! The settings tables: option lists, the item-display presets, the
//! navigation options, and the global hotkey bindings.

use super::ids::field;
use super::*;

pub(super) const MODES: [(ThemeMode, &str); 3] = [
    (ThemeMode::System, "Use Windows setting"),
    (ThemeMode::Dark, "Dark"),
    (ThemeMode::Light, "Light"),
];
pub(super) const BACKDROPS: [(Backdrop, &str); 4] = [
    (Backdrop::Acrylic, "Acrylic (frosted)"),
    (Backdrop::Blur, "Blur"),
    (Backdrop::Clear, "Clear glass (no blur)"),
    (Backdrop::Solid, "Solid"),
];
pub(super) const LABEL_LINES_OPTIONS: [(u8, &str); 2] = [(1, "One line"), (2, "Two lines")];
pub(super) const LABEL_SHADOW_OPTIONS: [(LabelShadow, &str); 3] = [
    (LabelShadow::Auto, "Automatic (when nearly transparent)"),
    (LabelShadow::On, "Always"),
    (LabelShadow::Off, "Never"),
];

/// The item-display presets the Appearance page offers, in dropdown order.
/// "Match Explorer's display" mirrors Explorer's icon size, spacing, check
/// boxes, and extensions; "Custom" uses the fields below.
pub(super) const PRESETS: [(Preset, &str); 2] = [
    (Preset::MatchExplorer, "Match Explorer's display"),
    (Preset::Custom, "Custom"),
];

pub(super) fn corner_marks() -> Vec<SliderMark> {
    vec![
        SliderMark::new(corner::SQUARE.into(), "Square"),
        SliderMark::new(corner::WINDOWS.into(), "Windows 11"),
        SliderMark::new(corner::ROUND.into(), "Round"),
        SliderMark::new(corner::MAX.into(), "Max"),
    ]
}

pub(super) const HOTKEY_HINT: &str = "Combine Ctrl, Alt, Shift, or Win with a letter, digit, or F1-F24, \
     for example Ctrl+Alt+D. Empty leaves the action unbound.";

/// What a global hotkey does when pressed. The app matches on it, so a new
/// action cannot be added without a handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    ToggleSpaces,
    NewSpace,
    OpenSettings,
}

/// A global hotkey the Hotkeys page can bind: what it does, the field that
/// edits it, the note field below it, its label, and where its text is
/// stored. Its position in `HOTKEY_BINDINGS` is its registration index.
#[derive(Clone, Copy, Debug)]
pub struct HotkeyBinding {
    pub action: HotkeyAction,
    pub field: u32,
    pub note: u32,
    pub label: &'static str,
    stored: fn(&WindowSettings) -> &String,
    stored_mut: fn(&mut WindowSettings) -> &mut String,
}

impl HotkeyBinding {
    /// The binding's saved text, for example "Ctrl+Alt+D"; empty is unbound.
    pub fn text(self, window: &WindowSettings) -> &str {
        (self.stored)(window)
    }

    pub fn set_text(self, window: &mut WindowSettings, text: String) {
        *(self.stored_mut)(window) = text;
    }
}

/// The global hotkeys, in registration order. A new binding is one entry here,
/// its `WindowSettings` field, its two `field::` ids, and (for a new action)
/// a `HotkeyAction` variant; the app registers, applies, and dispatches every
/// binding from this table.
pub const HOTKEY_BINDINGS: [HotkeyBinding; 3] = [
    HotkeyBinding {
        action: HotkeyAction::ToggleSpaces,
        field: field::HOTKEY,
        note: field::HOTKEY_NOTE,
        label: "Show or hide spaces",
        stored: |window| &window.toggle_hotkey,
        stored_mut: |window| &mut window.toggle_hotkey,
    },
    HotkeyBinding {
        action: HotkeyAction::NewSpace,
        field: field::HOTKEY_NEW_SPACE,
        note: field::HOTKEY_NEW_SPACE_NOTE,
        label: "Create a new space",
        stored: |window| &window.new_space_hotkey,
        stored_mut: |window| &mut window.new_space_hotkey,
    },
    HotkeyBinding {
        action: HotkeyAction::OpenSettings,
        field: field::HOTKEY_SETTINGS,
        note: field::HOTKEY_SETTINGS_NOTE,
        label: "Open settings",
        stored: |window| &window.settings_hotkey,
        stored_mut: |window| &mut window.settings_hotkey,
    },
];

pub(super) fn is_hotkey_field(id: u32) -> bool {
    HOTKEY_BINDINGS.iter().any(|binding| binding.field == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Config, Session) {
        let config = Config::new();
        let session = Session::new(&config);
        (config, session)
    }

    #[test]
    fn every_hotkey_binding_has_an_editable_field() {
        let (config, session) = setup();
        let form = build_form(
            &config,
            &session,
            true,
            Path::new("x"),
            deskspace_platform::SpacePreview::default(),
        );
        assert_eq!(session.hotkey_texts.len(), HOTKEY_BINDINGS.len());
        for binding in HOTKEY_BINDINGS {
            let field = form.field(binding.field).expect("its text field");
            assert!(field.enabled);
            assert!(form.field(binding.note).is_some(), "its hint note");
        }
    }

    #[test]
    fn each_binding_stores_its_own_setting() {
        let mut window = WindowSettings::default();
        for (slot, binding) in HOTKEY_BINDINGS.iter().enumerate() {
            binding.set_text(&mut window, format!("Ctrl+F{}", slot + 1));
        }
        assert_eq!(window.toggle_hotkey, "Ctrl+F1");
        assert_eq!(window.new_space_hotkey, "Ctrl+F2");
        assert_eq!(window.settings_hotkey, "Ctrl+F3");
        for (slot, binding) in HOTKEY_BINDINGS.iter().enumerate() {
            assert_eq!(binding.text(&window), format!("Ctrl+F{}", slot + 1));
        }
    }

    #[test]
    fn each_action_has_one_binding() {
        for action in [
            HotkeyAction::ToggleSpaces,
            HotkeyAction::NewSpace,
            HotkeyAction::OpenSettings,
        ] {
            let count = HOTKEY_BINDINGS
                .iter()
                .filter(|binding| binding.action == action)
                .count();
            assert_eq!(count, 1, "{action:?}");
        }
    }
}
