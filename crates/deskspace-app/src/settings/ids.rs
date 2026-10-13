//! The settings field and page ids.

pub mod field {
    pub const SPACES_HEADING: u32 = 1;
    pub const MODE: u32 = 2;
    pub const BACKDROP: u32 = 3;
    pub const CORNERS: u32 = 4;
    pub const TINT_OPACITY: u32 = 5;
    pub const TITLE_SHADING: u32 = 6;
    pub const BORDER: u32 = 7;
    pub const COLORS_HEADING: u32 = 8;
    pub const CUSTOM_COLORS: u32 = 9;
    pub const TINT: u32 = 10;
    pub const TEXT: u32 = 11;
    pub const RESET: u32 = 12;
    pub const COMPATIBILITY_NOTE: u32 = 13;
    pub const LABELS_HEADING: u32 = 14;
    pub const LABEL_LINES: u32 = 15;
    pub const LABEL_SIZE: u32 = 16;
    pub const LABEL_SHADOW: u32 = 17;
    pub const LAYOUT_HEADING: u32 = 18;
    pub const GRID_GAP: u32 = 19;
    pub const GRID_PADDING: u32 = 41;
    pub const PRESET: u32 = 42;
    pub const ICON_SIZE: u32 = 43;

    pub const ADVANCED_HEADING: u32 = 44;
    pub const OVERLAY_SIZE: u32 = 45;
    pub const OVERLAY_NOTE: u32 = 46;

    pub const BEHAVIOR_HEADING: u32 = 20;
    pub const HOTKEY: u32 = 21;
    pub const HOTKEY_NOTE: u32 = 22;
    pub const START_HIDDEN: u32 = 23;
    pub const SNAP: u32 = 24;

    pub const ABOUT_HEADING: u32 = 30;
    pub const ABOUT_SUMMARY: u32 = 31;
    pub const ABOUT_LICENSE: u32 = 32;
    pub const CONFIG_PATH: u32 = 33;
    pub const USE_ACCENT: u32 = 47;
    pub const UI_MODE: u32 = 48;
    pub const START_WITH_WINDOWS: u32 = 49;
    pub const TAKE_OVER_DESKTOP: u32 = 50;

    pub const HOTKEYS_HEADING: u32 = 51;
    pub const HOTKEY_NEW_SPACE: u32 = 52;
    pub const HOTKEY_NEW_SPACE_NOTE: u32 = 53;
    pub const HOTKEY_SETTINGS: u32 = 54;
    pub const HOTKEY_SETTINGS_NOTE: u32 = 55;

    pub const NEW_SPACES_HEADING: u32 = 56;
    pub const NEW_SPACE_TITLE: u32 = 57;
    pub const NEW_SPACE_WIDTH: u32 = 58;
    pub const NEW_SPACE_HEIGHT: u32 = 59;
    pub const NEW_SPACE_LOCKED: u32 = 60;
    pub const NEW_SPACES_NOTE: u32 = 61;
    pub const FIT_CONTENTS: u32 = 62;
}

/// The pages, in rail order. The first `LOOK_PAGE_COUNT` hold look settings,
/// which a space can override; General, New spaces, Hotkeys, and About are
/// Global only.
pub mod page {
    pub const APPEARANCE: usize = 0;
    pub const LAYOUT: usize = 2;
    pub const ABOUT: usize = 8;
}
