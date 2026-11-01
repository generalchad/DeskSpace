//! The settings pages: how the configuration becomes the form's pages.

use deskspace_config::ICON_SETS;
use deskspace_core::source::FolderView;

use super::ids::field;
use super::tables::{
    ANIMATION_PAGES, AnimationPage, BACKDROPS, BORDER_SOURCES, EDGES, EXTENSIONS_OPTIONS,
    HOTKEY_BINDINGS, HOTKEY_HINT, ICON_DOWNLOAD_OPTIONS, LABEL_LINES_OPTIONS, LABEL_SHADOW_OPTIONS,
    LENS_TITLES, MODES, NEW_SPACE_ICON_OPTIONS, PRESETS, STARTUP_ORDERS, corner_marks,
};
use super::view_fields::{LENSES_PAGE_IDS, NEW_SPACES_PAGE_IDS, VIEW_PAGE_IDS, view_fields};
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
            Field::slider(
                field::BORDER_WIDTH,
                "Border thickness",
                (BORDER_WIDTH_RANGE.0.into(), BORDER_WIDTH_RANGE.1.into()),
                theme
                    .border_width
                    .clamp(BORDER_WIDTH_RANGE.0, BORDER_WIDTH_RANGE.1) as i32,
            )
            .suffix(" px")
            .enabled(theme.show_border),
            Field::toggle(field::SHADOW, "Show shadow", theme.show_shadow),
            Field::slider(
                field::SHADOW_SIZE,
                "Shadow size",
                (SHADOW_SIZE_RANGE.0.into(), SHADOW_SIZE_RANGE.1.into()),
                theme
                    .shadow_size
                    .clamp(SHADOW_SIZE_RANGE.0, SHADOW_SIZE_RANGE.1) as i32,
            )
            .suffix(" px")
            .enabled(theme.show_shadow),
            Field::slider(
                field::SHADOW_OPACITY,
                "Shadow opacity",
                (SHADOW_OPACITY_RANGE.0.into(), SHADOW_OPACITY_RANGE.1.into()),
                theme
                    .shadow_opacity
                    .clamp(SHADOW_OPACITY_RANGE.0, SHADOW_OPACITY_RANGE.1) as i32,
            )
            .suffix(" %")
            .enabled(theme.show_shadow),
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
                "Use Windows accent color for the selection",
                theme.use_accent,
            ),
            Field::choice(
                field::BORDER_SOURCE,
                "Border color",
                &labels(&BORDER_SOURCES),
                index_of(&BORDER_SOURCES, theme.border_source),
            )
            .enabled(theme.show_border),
            Field::color(
                field::BORDER_COLOR,
                "Custom border color",
                palette.border.with_alpha(255),
            )
            .enabled(theme.show_border && theme.border_source == BorderSource::Custom),
            Field::slider(
                field::BORDER_OPACITY,
                "Border opacity",
                (BORDER_OPACITY_RANGE.0.into(), BORDER_OPACITY_RANGE.1.into()),
                theme
                    .border_opacity
                    .clamp(BORDER_OPACITY_RANGE.0, BORDER_OPACITY_RANGE.1) as i32,
            )
            .suffix(" %")
            .enabled(theme.show_border),
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
            Field::choice(
                field::ICON_DOWNLOADS,
                "Icon downloads",
                &labels(&ICON_DOWNLOAD_OPTIONS),
                index_of(&ICON_DOWNLOAD_OPTIONS, config.window.icon_downloads),
            )
            .enabled(deskspace_update::is_configured()),
            Field::note(field::ICON_DOWNLOADS_NOTE, icon_downloads_note()),
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

fn icon_downloads_note() -> &'static str {
    if deskspace_update::is_configured() {
        "Space icons come from an icon set downloaded once from DeskSpace's releases \
         on GitHub, and only after you agree. Nothing about you or your files is sent."
    } else {
        "This build cannot download space icons."
    }
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
            Field::choice(
                field::NEW_SPACE_ICON,
                "Icon",
                &labels(&NEW_SPACE_ICON_OPTIONS),
                index_of(&NEW_SPACE_ICON_OPTIONS, window.new_space_icon),
            ),
            Field::note(
                field::NEW_SPACES_NOTE,
                "New spaces otherwise follow the Global appearance and layout. \
                 A locked space keeps its size and scrolls instead of growing.",
            ),
        ]
        .into_iter()
        .chain(view_fields(
            &window.new_space_view,
            &NEW_SPACES_PAGE_IDS,
            false,
        ))
        .collect(),
    }
}

/// A space's View page (W20): its view mode, status bar, extension choice
/// (W28), and sort rule. Built for every scope so the form keeps its shape,
/// but only a space lists it.
pub(super) fn view_page(space: Option<&Space>) -> SettingsPage {
    let view = space.map_or_else(SpaceView::default, |space| space.view().clone());
    let mut fields = view_fields(&view, &VIEW_PAGE_IDS, space.is_some_and(Space::is_lens));
    // The extension choice sits after the status bar, as in the View menu.
    let after_status = fields
        .iter()
        .position(|field| field.id == field::VIEW_STATUS_BAR)
        .map_or(fields.len(), |index| index + 1);
    fields.insert(
        after_status,
        Field::toggle(field::VIEW_COMPACT, "Compact view", view.compact),
    );
    fields.insert(
        after_status + 1,
        Field::choice(
            field::VIEW_EXTENSIONS,
            "File name extensions",
            &labels(&EXTENSIONS_OPTIONS),
            index_of(&EXTENSIONS_OPTIONS, view.extensions),
        ),
    );
    SettingsPage {
        title: "View".into(),
        fields,
    }
}

/// Global's Lenses page (W23): the defaults a new lens starts with.
pub(super) fn lenses_page(config: &Config) -> SettingsPage {
    let defaults = &config.window.new_lens;
    let mut fields = view_fields(&defaults.view, &LENSES_PAGE_IDS, true);
    fields.extend([
        Field::heading(field::LENSES_TITLE_HEADING, "Lens"),
        Field::toggle(
            field::LENSES_SHOW_HIDDEN,
            "Show hidden files",
            defaults.show_hidden,
        ),
        Field::choice(
            field::LENSES_TITLE_MODE,
            "Show as",
            &labels(&LENS_TITLES),
            index_of(&LENS_TITLES, defaults.title),
        ),
    ]);
    SettingsPage {
        title: "Lenses".into(),
        fields,
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
            Field::note(field::ABOUT_ICONS, icons_note()),
            Field::read_only_text(
                field::CONFIG_PATH,
                "Configuration file",
                config_path.display().to_string(),
            ),
        ],
    }
}

/// One line per icon set: its label, author, and license. A single note
/// whatever the number of sets, so adding a set never adds a field id.
fn icons_note() -> String {
    ICON_SETS
        .iter()
        .map(|set| {
            format!(
                "Icons: {} by {}, {} License.",
                set.label, set.author, set.license
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The note atop every animation page: why the page is grey when Windows'
/// "Animation effects" is off, or what the page follows when it is on.
const ANIMATIONS_ON_NOTE: &str =
    "DeskSpace plays these while Windows' \u{201c}Animation effects\u{201d} is on.";
const ANIMATIONS_OFF_NOTE: &str = "Windows' \u{201c}Animation effects\u{201d} is off, so \
     DeskSpace does not animate. Turn it on in Windows to choose here.";

/// One page per animation kind (ROADMAP D12), in `ANIMATION_PAGES` order.
/// Each shows the options its kind offers, greyed while the chosen effect
/// does not use them, and every control is greyed while Windows' switch is
/// off.
pub(super) fn animation_pages(animations: &Animations, system_on: bool) -> Vec<SettingsPage> {
    ANIMATION_PAGES
        .iter()
        .map(|page| animation_page(page, animations, system_on))
        .collect()
}

fn animation_page(page: &AnimationPage, animations: &Animations, system_on: bool) -> SettingsPage {
    let spec = page.kind.spec();
    let settings = animations.get(page.kind);
    let resolved = animations.resolve(page.kind);
    let animates = system_on && resolved.effect != AnimationEffect::None;
    // A kind that follows another shows that kind's options, greyed: they are
    // edited on its page.
    let own = matches!(settings.effect, EffectChoice::Effect(_));
    let option_enabled = |option| animates && own && resolved.effect.uses(option);

    let choices = page.choices();
    let choice_labels: Vec<String> = choices
        .iter()
        .map(|choice| AnimationPage::choice_label(*choice))
        .collect();
    let choice_refs: Vec<&str> = choice_labels.iter().map(String::as_str).collect();
    let selected = choices
        .iter()
        .position(|choice| *choice == settings.effect)
        .unwrap_or(0);

    let mut fields = vec![
        Field::heading(page.heading_id, page.heading),
        Field::note(
            page.note,
            if system_on {
                ANIMATIONS_ON_NOTE
            } else {
                ANIMATIONS_OFF_NOTE
            },
        ),
        // Buttons have a fixed width; the note above says what this opens.
        Field::button(page.open_settings, "", "Windows settings\u{2026}"),
        Field::choice(page.effect, "Effect", &choice_refs, selected).enabled(system_on),
        Field::slider(
            page.duration,
            "Duration",
            (DURATION_MS_RANGE.0.into(), DURATION_MS_RANGE.1.into()),
            settings.duration_ms.into(),
        )
        .editable()
        .suffix(" ms")
        .enabled(animates),
    ];
    if spec.options.contains(&EffectOption::Detail) {
        fields.push(
            Field::slider(
                page.detail,
                "Detail",
                (DETAIL_RANGE.0.into(), DETAIL_RANGE.1.into()),
                resolved.detail.into(),
            )
            .suffix("")
            .marks(vec![
                SliderMark::new(DETAIL_RANGE.0.into(), "Fine"),
                SliderMark::new(DETAIL_RANGE.1.into(), "Coarse"),
            ])
            .enabled(option_enabled(EffectOption::Detail)),
        );
    }
    if spec.options.contains(&EffectOption::Direction) {
        fields.push(
            Field::choice(
                page.direction,
                "Direction",
                &labels(&EDGES),
                index_of(&EDGES, resolved.direction),
            )
            .enabled(option_enabled(EffectOption::Direction)),
        );
    }
    if spec.startup {
        fields.push(
            Field::choice(
                field::ANIM_STARTUP_ORDER,
                "At startup",
                &labels(&STARTUP_ORDERS),
                index_of(&STARTUP_ORDERS, animations.startup),
            )
            .enabled(animates),
        );
        fields.push(
            Field::slider(
                field::ANIM_STAGGER,
                "Delay between spaces",
                (STAGGER_MS_RANGE.0.into(), STAGGER_MS_RANGE.1.into()),
                animations.stagger_ms.into(),
            )
            .editable()
            .suffix(" ms")
            .enabled(animates && animations.startup == StartupOrder::OneAfterAnother),
        );
    }
    SettingsPage {
        title: page.title.into(),
        fields,
    }
}

/// The separator between a filter field's patterns, as typed and as shown.
const PATTERN_SEPARATOR: char = ';';

/// A filter field's text as patterns: split at `;`, each trimmed, empty ones
/// dropped. The lens caps the list further (`FolderView::clamped`).
pub(crate) fn parse_patterns(text: &str) -> Vec<String> {
    text.split(PATTERN_SEPARATOR)
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Patterns as a filter field shows them: `*.pdf; *.docx`.
pub(crate) fn format_patterns(patterns: &[String]) -> String {
    patterns.join(&format!("{PATTERN_SEPARATOR} "))
}

/// A lens's Lens page: its folder, the buttons that act on it, the
/// hidden-files toggle, and its filters. Built for every scope so the form
/// keeps its shape; only a lens lists it.
pub(super) fn lens_page(space: Option<&Space>, available: bool, session: &Session) -> SettingsPage {
    let view = space.and_then(Space::folder_view);
    // A filter shows the text as typed while the window is open, so the form
    // never rewrites it mid-edit; otherwise the lens's own list.
    let filter_text = |id: u32, patterns: fn(&FolderView) -> &[String]| {
        space
            .and_then(|space| session.filter_texts.get(&(space.id, id)))
            .cloned()
            .or_else(|| view.map(|view| format_patterns(patterns(view))))
            .unwrap_or_default()
    };
    let path = view.map(|view| view.path.display().to_string());
    let folder = match (path, available) {
        (Some(path), true) => path,
        (Some(path), false) => format!("Folder not available\n{path}"),
        (None, _) => String::new(),
    };
    let title = view.map_or(LensTitle::Custom, |view| view.title);
    let custom_text = space.map_or("", |space| space.title());
    SettingsPage {
        title: "Lens".into(),
        fields: vec![
            Field::heading(field::LENS_HEADING, "Folder"),
            Field::note(field::LENS_PATH, folder),
            Field::button(field::LENS_OPEN, "", "Open folder"),
            Field::button(field::LENS_CHANGE, "", "Change folder\u{2026}"),
            Field::heading(field::LENS_TITLE_HEADING, "Title"),
            Field::choice(
                field::LENS_TITLE_MODE,
                "Show as",
                &labels(&LENS_TITLES),
                index_of(&LENS_TITLES, title),
            ),
            Field::text(field::LENS_TITLE_TEXT, "Custom title", custom_text)
                .enabled(title == LensTitle::Custom),
            Field::heading(field::LENS_CONTENTS_HEADING, "Contents"),
            Field::toggle(
                field::LENS_SHOW_HIDDEN,
                "Show hidden files",
                view.is_some_and(|view| view.show_hidden),
            ),
            Field::heading(field::LENS_FILTER_HEADING, "Filter"),
            Field::text(
                field::LENS_SHOW_ONLY,
                "Show only",
                filter_text(field::LENS_SHOW_ONLY, |view| &view.include),
            ),
            Field::text(
                field::LENS_HIDE,
                "Hide",
                filter_text(field::LENS_HIDE, |view| &view.exclude),
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
