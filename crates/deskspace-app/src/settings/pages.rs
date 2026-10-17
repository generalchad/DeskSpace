//! The settings pages: how the configuration becomes the form's pages.

use super::ids::field;
use super::tables::{
    BACKDROPS, HOTKEY_BINDINGS, HOTKEY_HINT, LABEL_LINES_OPTIONS, LABEL_SHADOW_OPTIONS, MODES,
    PRESETS, corner_marks,
};
use super::*;
use super::{index_of, labels};

/// The pages a space can override: Appearance, Labels, Layout, Colors, and
/// Advanced, showing `theme` and `grid`.
pub(super) fn look_pages(
    theme: &Theme,
    grid: &GridSpec,
    system_dark: bool,
    for_space: bool,
) -> Vec<SettingsPage> {
    let palette = theme.palette(system_dark);
    let solid = theme.backdrop == Backdrop::Solid;

    // Appearance is split across several pages so no page is tall enough to
    // clip on a 1080-pixel display.
    let appearance = SettingsPage {
        title: "Appearance".into(),
        fields: vec![
            Field::heading(field::SPACES_HEADING, "Spaces"),
            Field::choice(
                field::MODE,
                "Color mode",
                &labels(&MODES),
                index_of(&MODES, theme.mode),
            ),
            Field::choice(
                field::BACKDROP,
                "Background material",
                &labels(&BACKDROPS),
                index_of(&BACKDROPS, theme.backdrop),
            ),
            Field::slider(
                field::CORNERS,
                "Corner radius",
                (corner::SQUARE.into(), corner::MAX.into()),
                theme.corner_radius.min(corner::MAX).into(),
            )
            .suffix(" px")
            .marks(corner_marks()),
            Field::slider(
                field::TINT_OPACITY,
                "Tint opacity",
                (TINT_OPACITY_RANGE.0.into(), TINT_OPACITY_RANGE.1.into()),
                theme
                    .tint_opacity
                    .clamp(TINT_OPACITY_RANGE.0, TINT_OPACITY_RANGE.1) as i32,
            )
            .enabled(!solid),
            Field::slider(
                field::TITLE_SHADING,
                "Title bar shading",
                (TITLE_SHADING_RANGE.0.into(), TITLE_SHADING_RANGE.1.into()),
                theme
                    .title_shading
                    .clamp(TITLE_SHADING_RANGE.0, TITLE_SHADING_RANGE.1) as i32,
            ),
            Field::toggle(field::BORDER, "Show window border", theme.show_border),
            Field::choice(
                field::PRESET,
                "Item preset",
                &labels(&PRESETS),
                index_of(&PRESETS, theme.preset),
            ),
        ],
    };

    let labels_page = SettingsPage {
        title: "Labels".into(),
        fields: vec![
            Field::heading(field::LABELS_HEADING, "Labels"),
            Field::choice(
                field::LABEL_LINES,
                "Label lines",
                &labels(&LABEL_LINES_OPTIONS),
                index_of(&LABEL_LINES_OPTIONS, theme.label_lines.clamp(1, 2)),
            ),
            Field::slider(
                field::LABEL_SIZE,
                "Label size",
                (LABEL_SIZE_RANGE.0.into(), LABEL_SIZE_RANGE.1.into()),
                theme
                    .label_size
                    .clamp(LABEL_SIZE_RANGE.0, LABEL_SIZE_RANGE.1) as i32,
            ),
            Field::choice(
                field::LABEL_SHADOW,
                "Label shadow",
                &labels(&LABEL_SHADOW_OPTIONS),
                index_of(&LABEL_SHADOW_OPTIONS, theme.label_shadow),
            ),
        ],
    };

    let layout_page = SettingsPage {
        title: "Layout".into(),
        fields: vec![
            Field::heading(field::LAYOUT_HEADING, "Layout"),
            Field::slider(
                field::ICON_SIZE,
                "Icon size (%)",
                (ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into()),
                theme.icon_size.clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1) as i32,
            )
            .editable()
            .marks(vec![
                SliderMark::new(ICON_SIZE_RANGE.0.into(), "Small"),
                SliderMark::new(100, "Default"),
                SliderMark::new(ICON_SIZE_RANGE.1.into(), "Large"),
            ])
            .enabled(!theme.match_explorer()),
            Field::slider(
                field::GRID_GAP,
                "Icon spacing",
                (GAP_RANGE.0, GAP_RANGE.1),
                grid.gap.clamp(GAP_RANGE.0, GAP_RANGE.1),
            )
            .suffix(" px")
            .enabled(!theme.match_explorer()),
            Field::slider(
                field::GRID_PADDING,
                "Edge padding",
                (GAP_RANGE.0, GAP_RANGE.1),
                grid.padding.clamp(GAP_RANGE.0, GAP_RANGE.1),
            )
            .suffix(" px")
            .enabled(!theme.match_explorer()),
        ],
    };

    let reset = if for_space {
        "Match global appearance"
    } else {
        "Restore default appearance"
    };
    let colors_page = SettingsPage {
        title: "Colors".into(),
        fields: vec![
            Field::heading(field::COLORS_HEADING, "Colors"),
            Field::toggle(
                field::CUSTOM_COLORS,
                "Use custom colors instead of the color mode's",
                theme.custom_colors,
            ),
            Field::color(field::TINT, "Background tint", palette.tint.with_alpha(255))
                .enabled(theme.custom_colors),
            Field::color(field::TEXT, "Text", palette.title_text).enabled(theme.custom_colors),
            Field::toggle(
                field::USE_ACCENT,
                "Use Windows accent color for the selection and border",
                theme.use_accent,
            ),
            Field::color(
                field::MARQUEE_COLOR,
                "Selection box color",
                theme.marquee_color.with_alpha(255),
            )
            .enabled(!theme.use_accent),
            Field::toggle(
                field::MARQUEE_BORDER,
                "Show selection box border",
                theme.show_marquee_border,
            ),
            Field::button(field::RESET, "", reset),
            Field::note(
                field::COMPATIBILITY_NOTE,
                "Clear glass at 0% tint and 0% title shading, without a border, \
                 shows only the icons and labels. Acrylic and blur use Windows 11's \
                 own corners, up to 8 px; larger radii apply to clear glass and \
                 solid. Desktop-hosted spaces are drawn opaque.",
            ),
        ],
    };

    let advanced_page = SettingsPage {
        title: "Advanced".into(),
        fields: vec![
            Field::heading(field::ADVANCED_HEADING, "Advanced"),
            Field::slider(
                field::OVERLAY_SIZE,
                "Shortcut overlay size (%)",
                (OVERLAY_SIZE_RANGE.0.into(), OVERLAY_SIZE_RANGE.1.into()),
                theme
                    .overlay_size
                    .clamp(OVERLAY_SIZE_RANGE.0, OVERLAY_SIZE_RANGE.1)
                    .into(),
            )
            .editable()
            .marks(vec![
                SliderMark::new(OVERLAY_SIZE_RANGE.0.into(), "Small"),
                SliderMark::new(OVERLAY_SIZE_DEFAULT.into(), "Default"),
                SliderMark::new(OVERLAY_SIZE_RANGE.1.into(), "Explorer"),
            ]),
            Field::note(
                field::OVERLAY_NOTE,
                "How large the shell's corner badge (the shortcut arrow, a sync \
                 badge) is drawn relative to the icon. Explorer uses 100%; \
                 smaller is often easier on the eye.",
            ),
        ],
    };

    vec![
        appearance,
        labels_page,
        layout_page,
        colors_page,
        advanced_page,
    ]
}

pub(super) fn general_page(config: &Config) -> SettingsPage {
    SettingsPage {
        title: "General".into(),
        fields: vec![
            Field::heading(field::BEHAVIOR_HEADING, "Behavior"),
            Field::toggle(
                field::START_HIDDEN,
                "Start with spaces hidden",
                config.window.start_hidden,
            ),
            Field::slider(
                field::SNAP,
                "Snap to grid",
                (SNAP_RANGE.0, SNAP_RANGE.1),
                config.desktop.layout.snap.clamp(SNAP_RANGE.0, SNAP_RANGE.1),
            )
            .suffix(" px"),
            Field::toggle(
                field::FIT_TO_GRID,
                "Snap resized spaces to whole columns and rows",
                config.desktop.layout.fit_to_grid,
            ),
            Field::toggle(
                field::START_WITH_WINDOWS,
                "Start DeskSpace when you sign in",
                config.window.start_with_windows,
            ),
            Field::toggle(
                field::TAKE_OVER_DESKTOP,
                "Take over the desktop (hide Explorer's icons)",
                config.window.take_over_desktop,
            ),
            Field::choice(
                field::UI_MODE,
                "Settings and menus",
                &labels(&MODES),
                index_of(&MODES, config.theme.ui_mode),
            ),
            Field::heading(field::UPDATES_HEADING, "Updates"),
            Field::toggle(
                field::CHECK_UPDATES,
                "Check for updates automatically",
                config.window.check_for_updates,
            )
            .enabled(deskspace_update::is_configured()),
            Field::toggle(
                field::INSTALL_UPDATES,
                "Install updates automatically",
                config.window.install_updates,
            )
            .enabled(can_install_automatically(config)),
            Field::note(field::CHECK_UPDATES_NOTE, updates_note()),
            Field::button(field::CHECK_UPDATES_NOW, "", "Check for updates now")
                .enabled(deskspace_update::is_configured()),
        ],
    }
}

/// Automatic installs need the automatic check, and an installed build that
/// can replace itself (a portable copy only notifies).
fn can_install_automatically(config: &Config) -> bool {
    config.window.check_for_updates
        && deskspace_update::is_configured()
        && deskspace_update::is_installed()
}

fn updates_note() -> &'static str {
    if !deskspace_update::is_configured() {
        "This build cannot check for updates. Download new versions from the releases page."
    } else if !deskspace_update::is_installed() {
        "The automatic check contacts GitHub about once a day. This portable copy only \
         tells you about new versions; the installed version can update itself."
    } else {
        "The automatic check contacts GitHub about once a day. An automatic update \
         downloads in the background and installs after you have been away for 10 minutes."
    }
}

pub(super) fn new_spaces_page(config: &Config) -> SettingsPage {
    let window = &config.window;
    let size_slider = |id, label, value: i32| {
        Field::slider(
            id,
            label,
            (NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1),
            value.clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1),
        )
        .editable()
        .suffix(" px")
    };
    SettingsPage {
        title: "New spaces".into(),
        fields: vec![
            Field::heading(field::NEW_SPACES_HEADING, "Defaults for new spaces"),
            Field::text(
                field::NEW_SPACE_TITLE,
                "Default title",
                &window.new_space_title,
            ),
            size_slider(
                field::NEW_SPACE_WIDTH,
                "Default width",
                window.new_space_width,
            ),
            size_slider(
                field::NEW_SPACE_HEIGHT,
                "Default height",
                window.new_space_height,
            ),
            Field::toggle(
                field::NEW_SPACE_LOCKED,
                "Start new spaces locked",
                window.new_space_locked,
            ),
            Field::note(
                field::NEW_SPACES_NOTE,
                "New spaces otherwise follow the Global appearance and layout. \
                 A locked space keeps its size and scrolls instead of growing.",
            ),
        ],
    }
}

pub(super) fn hotkeys_page(session: &Session) -> SettingsPage {
    let mut fields = vec![Field::heading(field::HOTKEYS_HEADING, "Global hotkeys")];
    for (slot, binding) in HOTKEY_BINDINGS.iter().enumerate() {
        fields.push(Field::text(
            binding.field,
            binding.label,
            &session.hotkey_texts[slot],
        ));
        fields.push(Field::note(
            binding.note,
            session.hotkey_errors[slot]
                .as_deref()
                .unwrap_or(HOTKEY_HINT),
        ));
    }
    SettingsPage {
        title: "Hotkeys".into(),
        fields,
    }
}

pub(super) fn about_page(config_path: &Path) -> SettingsPage {
    SettingsPage {
        title: "About".into(),
        fields: vec![
            Field::heading(
                field::ABOUT_HEADING,
                format!("DeskSpace {}", env!("CARGO_PKG_VERSION")),
            ),
            Field::note(
                field::ABOUT_SUMMARY,
                "A free and open-source desktop space organizer for Windows.",
            ),
            Field::note(
                field::ABOUT_LICENSE,
                "Copyright 2026 Timothy W. Brown. Licensed under the Apache License 2.0.",
            ),
            Field::read_only_text(
                field::CONFIG_PATH,
                "Configuration file",
                config_path.display().to_string(),
            ),
        ],
    }
}

/// A space's Shortcuts page: a preview of the space whose items can be
/// deleted (the X on each) and added to (the + tile; F-53).
pub(super) fn shortcuts_page(preview: SpacePreview) -> SettingsPage {
    SettingsPage {
        title: "Shortcuts".into(),
        fields: vec![
            Field::note(
                field::SHORTCUTS_NOTE,
                "Click \u{2715} to delete an item: a file in the space goes to the Recycle \
                 Bin, and an item shown in place is only removed from the space. Click + to \
                 add a shortcut to an installed app or to anything else.",
            ),
            Field::space_preview(field::SPACE_PREVIEW, preview),
        ],
    }
}
