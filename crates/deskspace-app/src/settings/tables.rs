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

/// A global hotkey the Hotkeys page can bind: the field that edits it, the note
/// field below it, its label, and the registration index the app passes to
/// `Ui::register_hotkey`.
#[derive(Clone, Copy, Debug)]
pub struct HotkeyBinding {
    pub field: u32,
    pub note: u32,
    pub label: &'static str,
    pub index: i32,
}

/// The global hotkeys, in registration order. The app iterates this to register
/// and to apply each binding, so a new binding is one entry here plus its
/// `WindowSettings` field.
pub const HOTKEY_BINDINGS: [HotkeyBinding; 3] = [
    HotkeyBinding {
        field: field::HOTKEY,
        note: field::HOTKEY_NOTE,
        label: "Show or hide spaces",
        index: 0,
    },
    HotkeyBinding {
        field: field::HOTKEY_NEW_SPACE,
        note: field::HOTKEY_NEW_SPACE_NOTE,
        label: "Create a new space",
        index: 1,
    },
    HotkeyBinding {
        field: field::HOTKEY_SETTINGS,
        note: field::HOTKEY_SETTINGS_NOTE,
        label: "Open settings",
        index: 2,
    },
];

/// The stored text of the binding at `index`.
pub fn hotkey_setting(window: &WindowSettings, index: i32) -> &str {
    match index {
        0 => &window.toggle_hotkey,
        1 => &window.new_space_hotkey,
        2 => &window.settings_hotkey,
        _ => "",
    }
}

/// Sets the stored text of the binding at `index`.
pub fn set_hotkey_setting(window: &mut WindowSettings, index: i32, text: String) {
    match index {
        0 => window.toggle_hotkey = text,
        1 => window.new_space_hotkey = text,
        2 => window.settings_hotkey = text,
        _ => {}
    }
}

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
}
