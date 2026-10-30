//! The settings tables: option lists, the item-display presets, the
//! navigation options, the global hotkey bindings, and the animation pages.

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

/// Whether an icon set may be downloaded, in dropdown order.
pub(super) const ICON_DOWNLOAD_OPTIONS: [(IconDownloads, &str); 3] = [
    (IconDownloads::Ask, "Ask first"),
    (IconDownloads::Allowed, "Allowed"),
    (IconDownloads::Declined, "Never"),
];

/// The icon a new space starts with, in dropdown order.
pub(super) const NEW_SPACE_ICON_OPTIONS: [(NewSpaceIcon, &str); 2] = [
    (NewSpaceIcon::Automatic, "Automatic"),
    (NewSpaceIcon::None, "No icon"),
];

/// How a lens's title follows its folder, in dropdown order (D17).
pub(super) const LENS_TITLES: [(LensTitle, &str); 4] = [
    (LensTitle::FolderName, "Folder name"),
    (LensTitle::ProfilePath, "Path from your user folder"),
    (LensTitle::FullPath, "Full path"),
    (LensTitle::Custom, "Custom"),
];

/// Where the border's color comes from, in dropdown order.
pub(super) const BORDER_SOURCES: [(BorderSource, &str); 3] = [
    (
        BorderSource::Mode,
        "Automatic (white in dark mode, black in light)",
    ),
    (BorderSource::Accent, "Windows accent color"),
    (BorderSource::Custom, "Custom"),
];

/// The item-display presets the Appearance page offers, in dropdown order.
/// "Match Explorer's display" mirrors Explorer's icon size, spacing, check
/// boxes, and extensions; "Custom" uses the fields below.
pub(super) const PRESETS: [(Preset, &str); 2] = [
    (Preset::MatchExplorer, "Match Explorer's display"),
    (Preset::Custom, "Custom"),
];

/// Explorer's views in menu order, with the Ctrl+Shift digit W13 adds.
pub const VIEW_MODES: [(ViewMode, &str, Option<u8>); 9] = [
    (ViewMode::Icons, "Settings icon size", None),
    (ViewMode::ExtraLargeIcons, "Extra large icons", Some(1)),
    (ViewMode::LargeIcons, "Large icons", Some(2)),
    (ViewMode::MediumIcons, "Medium icons", Some(3)),
    (ViewMode::SmallIcons, "Small icons", Some(4)),
    (ViewMode::List, "List", Some(5)),
    (ViewMode::Details, "Details", Some(6)),
    (ViewMode::Tiles, "Tiles", Some(7)),
    (ViewMode::Content, "Content", Some(8)),
];

/// The view Explorer's Ctrl+Shift digit chooses (W13), or `None` for a digit
/// no view carries yet.
pub fn view_for_digit(digit: u8) -> Option<ViewMode> {
    VIEW_MODES
        .iter()
        .find(|(.., shortcut)| *shortcut == Some(digit))
        .map(|(mode, ..)| *mode)
}

/// Explorer's Sort by keys, in its order and words (D17). The Sort by menu,
/// the View page, and the Lenses defaults page read it.
pub const SORT_KEYS: [(SortKey, &str); 5] = [
    (SortKey::Name, "Name"),
    (SortKey::Modified, "Date modified"),
    (SortKey::Kind, "Type"),
    (SortKey::Size, "Size"),
    (SortKey::Created, "Date created"),
];

/// Explorer's Details columns in display order (W18): its header label, the
/// sort key a click on the header uses, its default width at 96 DPI, how its
/// values align, and whether a space shows it until the user chooses. Name
/// takes the width left over, down to `NAME_MIN_WIDTH`. Widths are
/// provisional until W18's on-screen check measures Explorer's own.
pub struct DetailsColumn {
    pub label: &'static str,
    pub key: SortKey,
    pub width: i32,
    pub align: ColumnAlign,
    pub shown: bool,
}

pub const DETAILS_COLUMNS: [DetailsColumn; 5] = [
    DetailsColumn {
        label: "Name",
        key: SortKey::Name,
        width: 0,
        align: ColumnAlign::Left,
        shown: true,
    },
    DetailsColumn {
        label: "Date modified",
        key: SortKey::Modified,
        width: 130,
        align: ColumnAlign::Left,
        shown: true,
    },
    DetailsColumn {
        label: "Type",
        key: SortKey::Kind,
        width: 100,
        align: ColumnAlign::Left,
        shown: true,
    },
    DetailsColumn {
        label: "Size",
        key: SortKey::Size,
        width: 80,
        align: ColumnAlign::Right,
        shown: true,
    },
    // Explorer leaves Date created out of a new folder's Details view.
    DetailsColumn {
        label: "Date created",
        key: SortKey::Created,
        width: 130,
        align: ColumnAlign::Left,
        shown: false,
    },
];

/// The Details columns a space shows, in display order. Name is always first.
pub fn shown_details_columns() -> impl Iterator<Item = &'static DetailsColumn> {
    DETAILS_COLUMNS.iter().filter(|column| column.shown)
}

/// The narrowest Name column, so a small space keeps a readable name.
pub const NAME_MIN_WIDTH: i32 = 120;

/// Explorer's Sort by directions, in the View page's order (W20).
pub const VIEW_ORDERS: [&str; 2] = ["Ascending", "Descending"];

/// A view button in Explorer's status bar (W19): the view it switches to, its
/// glyph, and Explorer's tip. The tip gains its Ctrl+Shift digit from
/// `VIEW_MODES`.
pub struct StatusButton {
    pub mode: ViewMode,
    pub glyph: &'static [u16],
    pub tip: &'static str,
}

/// Explorer's two status-bar view buttons, Details and Large icons. The glyphs
/// are Segoe Fluent Icons / Segoe MDL2 Assets "BulletedList" and "ViewAll",
/// which both fonts have.
/// https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
pub const STATUS_BUTTONS: [StatusButton; 2] = [
    StatusButton {
        mode: ViewMode::Details,
        glyph: &[0xE8FD],
        tip: "Displays information about each item in the window.",
    },
    StatusButton {
        mode: ViewMode::LargeIcons,
        glyph: &[0xE8A9],
        tip: "Displays items by using large thumbnails.",
    },
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

/// The labels of `deskspace_core::animation::EFFECTS`, in its order.
pub(super) const EFFECT_LABELS: [(AnimationEffect, &str); 9] = [
    (AnimationEffect::None, "None"),
    (AnimationEffect::Roll, "Roll"),
    (AnimationEffect::Fade, "Fade"),
    (AnimationEffect::Pixelate, "Pixelate"),
    (AnimationEffect::Gradient, "Gradient"),
    (AnimationEffect::Dissolve, "Dissolve"),
    (AnimationEffect::Fly, "Fly"),
    (AnimationEffect::Crt, "CRT"),
    (AnimationEffect::Burn, "Burn"),
];
pub const EDGES: [(Edge, &str); 5] = [
    (Edge::Auto, "Automatic"),
    (Edge::Top, "Top"),
    (Edge::Bottom, "Bottom"),
    (Edge::Left, "Left"),
    (Edge::Right, "Right"),
];
pub const STARTUP_ORDERS: [(StartupOrder, &str); 2] = [
    (StartupOrder::OneAfterAnother, "One after another"),
    (StartupOrder::AllAtOnce, "All at once"),
];

/// One animation kind's settings page (ROADMAP D12): its title, the name a
/// "Same as" choice uses for it, and its field ids. The page shows the
/// options its kind offers (`KindSpec`), greying those the chosen effect does
/// not use. A new kind is a row here, its `KINDS` row, and its ids.
#[derive(Clone, Copy, Debug)]
pub struct AnimationPage {
    pub kind: Kind,
    pub title: &'static str,
    pub heading: &'static str,
    /// How a "Same as" choice names this kind ("Same as startup").
    pub name: &'static str,
    pub heading_id: u32,
    pub note: u32,
    pub open_settings: u32,
    pub effect: u32,
    pub duration: u32,
    pub detail: u32,
    pub direction: u32,
}

/// The animation pages, in `KINDS` order and in build order from
/// `page::ANIMATIONS`.
pub(super) const ANIMATION_PAGES: [AnimationPage; 3] = [
    AnimationPage {
        kind: Kind::Startup,
        title: "Startup",
        heading: "When spaces come and go",
        name: "startup",
        heading_id: field::ANIM_STARTUP_HEADING,
        note: field::ANIM_STARTUP_NOTE,
        open_settings: field::ANIM_STARTUP_OPEN_SETTINGS,
        effect: field::ANIM_STARTUP_EFFECT,
        duration: field::ANIM_STARTUP_DURATION,
        detail: field::ANIM_STARTUP_DETAIL,
        direction: field::ANIM_STARTUP_DIRECTION,
    },
    AnimationPage {
        kind: Kind::Delete,
        title: "Delete",
        heading: "When a space is deleted",
        name: "delete",
        heading_id: field::ANIM_DELETE_HEADING,
        note: field::ANIM_DELETE_NOTE,
        open_settings: field::ANIM_DELETE_OPEN_SETTINGS,
        effect: field::ANIM_DELETE_EFFECT,
        duration: field::ANIM_DELETE_DURATION,
        detail: field::ANIM_DELETE_DETAIL,
        direction: field::ANIM_DELETE_DIRECTION,
    },
    AnimationPage {
        kind: Kind::Fold,
        title: "Fold",
        heading: "When a space folds and unfolds",
        name: "fold",
        heading_id: field::ANIM_FOLD_HEADING,
        note: field::ANIM_FOLD_NOTE,
        open_settings: field::ANIM_FOLD_OPEN_SETTINGS,
        effect: field::ANIM_FOLD_EFFECT,
        duration: field::ANIM_FOLD_DURATION,
        detail: field::ANIM_FOLD_DETAIL,
        direction: field::ANIM_FOLD_DIRECTION,
    },
];

impl AnimationPage {
    pub fn of(kind: Kind) -> &'static AnimationPage {
        ANIMATION_PAGES
            .iter()
            .find(|page| page.kind == kind)
            .expect("every kind has an animation page")
    }

    /// The choices the Effect dropdown lists, in order: "Same as" first when
    /// the kind can follow another, then its effects.
    pub fn choices(self) -> Vec<EffectChoice> {
        let spec = self.kind.spec();
        spec.follows
            .map(EffectChoice::SameAs)
            .into_iter()
            .chain(spec.offers.effects().map(EffectChoice::Effect))
            .collect()
    }

    pub fn choice_label(choice: EffectChoice) -> String {
        match choice {
            EffectChoice::Effect(effect) => EFFECT_LABELS[index_of(&EFFECT_LABELS, effect)]
                .1
                .to_string(),
            EffectChoice::SameAs(kind) => format!("Same as {}", AnimationPage::of(kind).name),
            EffectChoice::Unknown => String::new(),
        }
    }
}

/// The page Windows' "Animation effects" switch is on: Accessibility >
/// Visual effects on Windows 11 (build 22000 and later, and when the build is
/// unknown), Ease of Access > Display on Windows 10.
/// https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings-app
pub fn windows_animation_settings_uri(build: Option<u32>) -> &'static str {
    const WINDOWS_11: u32 = 22000;
    match build {
        Some(build) if build < WINDOWS_11 => "ms-settings:easeofaccess-display",
        _ => "ms-settings:easeofaccess-visualeffects",
    }
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
    fn each_view_shortcut_digit_names_one_view() {
        assert_eq!(view_for_digit(1), Some(ViewMode::ExtraLargeIcons));
        assert_eq!(view_for_digit(2), Some(ViewMode::LargeIcons));
        assert_eq!(view_for_digit(3), Some(ViewMode::MediumIcons));
        assert_eq!(view_for_digit(4), Some(ViewMode::SmallIcons));
        assert_eq!(view_for_digit(5), Some(ViewMode::List));
        assert_eq!(view_for_digit(0), None);

        let mut digits: Vec<u8> = VIEW_MODES.iter().filter_map(|(.., digit)| *digit).collect();
        let count = digits.len();
        digits.sort_unstable();
        digits.dedup();
        assert_eq!(digits.len(), count, "one view per shortcut digit");
    }

    #[test]
    fn every_effect_and_kind_has_its_row() {
        use deskspace_core::animation::{EFFECTS, KINDS};
        let effects: Vec<AnimationEffect> = EFFECTS.iter().map(|spec| spec.effect).collect();
        let labelled: Vec<AnimationEffect> =
            EFFECT_LABELS.iter().map(|(effect, _)| *effect).collect();
        assert_eq!(labelled, effects, "EFFECT_LABELS follows EFFECTS");
        let kinds: Vec<Kind> = KINDS.iter().map(|spec| spec.kind).collect();
        let pages: Vec<Kind> = ANIMATION_PAGES.iter().map(|page| page.kind).collect();
        assert_eq!(pages, kinds, "ANIMATION_PAGES follows KINDS");
    }

    #[test]
    fn delete_offers_same_as_startup_first() {
        let choices = AnimationPage::of(Kind::Delete).choices();
        assert_eq!(choices[0], EffectChoice::SameAs(Kind::Startup));
        assert_eq!(AnimationPage::choice_label(choices[0]), "Same as startup");
        assert_eq!(
            AnimationPage::of(Kind::Fold).choices(),
            [
                EffectChoice::Effect(AnimationEffect::None),
                EffectChoice::Effect(AnimationEffect::Roll)
            ]
        );
    }

    #[test]
    fn the_animation_switch_is_on_its_own_page_per_version() {
        assert_eq!(
            windows_animation_settings_uri(Some(19045)),
            "ms-settings:easeofaccess-display"
        );
        assert_eq!(
            windows_animation_settings_uri(Some(26300)),
            "ms-settings:easeofaccess-visualeffects"
        );
        assert_eq!(
            windows_animation_settings_uri(None),
            "ms-settings:easeofaccess-visualeffects"
        );
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
