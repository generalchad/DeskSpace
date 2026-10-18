//! The settings window's content and how edits map onto the configuration.
//! Pure functions over `Config`, so the form logic is testable without Win32.
//!
//! The window has three panes: the scopes (Global, then each space indented
//! under it), the pages of the selected scope, and the page itself. Global
//! edits write the config directly, as a live preview. A space follows Global
//! until its "Customize this space" switch is on; its edits then become
//! `Command`s the app dispatches, so `Desktop` is only changed through them.

mod apply;
mod ids;
mod pages;
mod session;
mod tables;

pub use apply::{
    apply_animation_edit, apply_field, customize_space, match_global, reset_appearance, space_edit,
};
pub use ids::{field, page};
pub use session::{Session, SpaceSnapshot, needs_relayout, needs_restyle};
pub use tables::{
    ANIMATION_PAGES, AnimationPage, EDGES, HOTKEY_BINDINGS, HotkeyAction, STARTUP_ORDERS,
    windows_animation_settings_uri,
};

use std::path::Path;

use deskspace_config::{Config, NEW_SPACE_SIZE_RANGE, WindowSettings};
use deskspace_core::animation::{
    Animations, DETAIL_RANGE, DURATION_MS_RANGE, Edge, Effect as AnimationEffect, EffectChoice,
    EffectOption, Kind, STAGGER_MS_RANGE, StartupOrder,
};
use deskspace_core::layout::{GAP_RANGE, SNAP_RANGE};
use deskspace_core::theme::{
    ICON_SIZE_RANGE, LABEL_SIZE_RANGE, OVERLAY_SIZE_DEFAULT, OVERLAY_SIZE_RANGE,
    TINT_OPACITY_RANGE, TITLE_SHADING_RANGE, corner,
};
use deskspace_core::{
    Backdrop, Color, Command, GridSpec, LabelShadow, Palette, Preset, Space, SpaceId, SpaceLook,
    Theme, ThemeMode,
};
use deskspace_platform::{
    Field, FieldValue, SettingsButton, SettingsForm, SettingsHeader, SettingsNav, SettingsNavEntry,
    SettingsNavGroup, SettingsPage, SettingsScope, SettingsSwitch, SliderMark, SpacePreview,
};

use pages::{
    about_page, animation_pages, general_page, hotkeys_page, look_pages, new_spaces_page,
    shortcuts_page,
};

/// How many pages hold look settings (Appearance through Advanced).
pub const LOOK_PAGE_COUNT: usize = 5;

fn index_of<T: PartialEq + Copy>(table: &[(T, &str)], value: T) -> usize {
    table
        .iter()
        .position(|(entry, _)| *entry == value)
        .unwrap_or(0)
}

fn labels<T>(table: &[(T, &'static str)]) -> Vec<&'static str> {
    table.iter().map(|(_, label)| *label).collect()
}

/// The space the session edits, if it still exists; `None` is Global.
fn scope_space<'a>(config: &'a Config, session: &Session) -> Option<&'a Space> {
    session.scope.and_then(|id| config.desktop.space(id))
}

/// The scope index (in the scopes pane) of the session's scope: 0 for
/// Global, then the spaces in order.
pub fn scope_index(config: &Config, session: &Session) -> usize {
    scope_space(config, session)
        .and_then(|space| {
            config
                .desktop
                .spaces()
                .iter()
                .position(|other| other.id == space.id)
        })
        .map_or(0, |index| index + 1)
}

/// The scope at `index` in the scopes pane: `None` for Global (0) or an index
/// past the spaces.
pub fn scope_at(config: &Config, index: usize) -> Option<SpaceId> {
    index
        .checked_sub(1)
        .and_then(|index| config.desktop.spaces().get(index))
        .map(|space| space.id)
}

/// The pages the categories pane lists for `scope`, in order: every Global
/// page for Global (`None`), the animation pages last, or a space's look
/// pages and its Shortcuts page.
pub fn listed_pages(scope: Option<SpaceId>) -> Vec<usize> {
    match scope {
        None => (0..=page::ABOUT)
            .chain(page::ANIMATIONS..page::ANIMATIONS + tables::ANIMATION_PAGES.len())
            .collect(),
        Some(_) => (0..LOOK_PAGE_COUNT)
            .chain(std::iter::once(page::SHORTCUTS))
            .collect(),
    }
}

/// The opaque colors a settings preview draws a space with: the tint over a
/// stand-in for the backdrop (Windows 11's dark or light base), and the title
/// shading over that. The blur cannot be shown, so this is an approximation.
pub fn space_preview_colors(palette: &Palette) -> (Color, Color, Color) {
    let base = if palette.dark {
        Color::rgb(0x20, 0x20, 0x20)
    } else {
        Color::rgb(0xF3, 0xF3, 0xF3)
    };
    let fill = over(base, palette.tint);
    let title = over(fill, palette.title_fill);
    (fill, title, palette.title_text.with_alpha(255))
}

/// `top` composited over the opaque `base` by `top`'s alpha.
fn over(base: Color, top: Color) -> Color {
    let alpha = u32::from(top.a);
    let mix =
        |b: u8, t: u8| ((u32::from(b) * (255 - alpha) + u32::from(t) * alpha + 127) / 255) as u8;
    Color::rgb(mix(base.r, top.r), mix(base.g, top.g), mix(base.b, top.b))
}

/// Builds the settings form. `preview` is what the Shortcuts page shows for
/// the selected space (it is built for every scope, so the form keeps its
/// shape; Global never lists the page).
pub fn build_form(
    config: &Config,
    session: &Session,
    system_dark: bool,
    system_animations: bool,
    config_path: &Path,
    preview: SpacePreview,
) -> SettingsForm {
    let space = scope_space(config, session);
    let global = &config.theme;
    // A space shows what it is drawn with: its own look while customized,
    // else the global values, greyed until the switch is turned on.
    let theme = space.map_or(global, |space| space.theme(global));
    let grid = space.map_or(config.desktop.layout, |space| {
        space.grid(config.desktop.layout)
    });
    let editable = space.is_none_or(|space| space.custom());

    let mut pages = look_pages(theme, &grid, system_dark, space.is_some());
    if !editable {
        for field in pages.iter_mut().flat_map(|page| page.fields.iter_mut()) {
            field.enabled = false;
        }
    }
    // Always built, so the form keeps its shape when the scope changes; it
    // applies only to a resized, unlocked space and is grey for Global. Fitting
    // is geometry, not a look setting, so it is not caught by the `editable`
    // disable above and stays available while a space follows Global, matching
    // the space menu's "Fit to contents".
    pages[page::LAYOUT].fields.push(
        Field::button(field::FIT_CONTENTS, "", "Fit to contents")
            .enabled(space.is_some_and(|space| space.user_sized() && !space.locked())),
    );
    pages.push(general_page(config));
    pages.push(new_spaces_page(config));
    pages.push(hotkeys_page(session));
    pages.push(about_page(config_path));
    pages.push(shortcuts_page(preview));
    debug_assert_eq!(pages.len(), page::ANIMATIONS);
    pages.extend(animation_pages(
        &config.window.animations,
        system_animations,
    ));

    let listed = listed_pages(space.map(|space| space.id));
    let selected = listed
        .iter()
        .position(|page| *page == session.page)
        .unwrap_or(0);
    let entry = |&index: &usize| SettingsNavEntry {
        label: pages[index].title.clone(),
        page: index,
        marked: false,
    };
    // The animation pages get a group of their own, after the rest, so the
    // flattened entries keep `listed`'s order and `selected` stays valid.
    let (settings_pages, animation_pages): (Vec<usize>, Vec<usize>) =
        listed.iter().partition(|&&index| index < page::ANIMATIONS);
    let mut groups = vec![SettingsNavGroup {
        title: String::new(),
        entries: settings_pages.iter().map(entry).collect(),
    }];
    if !animation_pages.is_empty() {
        groups.push(SettingsNavGroup {
            title: "Animations".into(),
            entries: animation_pages.iter().map(entry).collect(),
        });
    }

    let mut scopes = vec![SettingsScope {
        label: "Global".into(),
        indent: false,
        dimmed: false,
    }];
    scopes.extend(config.desktop.spaces().iter().map(|space| SettingsScope {
        label: space.title().to_owned(),
        indent: true,
        dimmed: !space.custom(),
    }));
    let header = match space {
        None => SettingsHeader {
            title: "Global settings".into(),
            switch: None,
            button: None,
        },
        Some(space) => SettingsHeader {
            title: space.title().to_owned(),
            switch: Some(SettingsSwitch {
                label: "Customize this space".into(),
                on: space.custom(),
            }),
            button: Some(SettingsButton {
                // The ellipsis promises a confirmation dialog, which only a
                // non-empty space shows (matching the space menu).
                label: if space.items().is_empty() {
                    "Delete space".into()
                } else {
                    "Delete space\u{2026}".into()
                },
            }),
        },
    };

    SettingsForm {
        title: "DeskSpace Settings".into(),
        pages,
        apply_enabled: session.dirty,
        nav: SettingsNav {
            scopes,
            selected_scope: scope_index(config, session),
            groups,
            selected,
            header: Some(header),
        },
    }
}

/// What a settings button does. Every `Field::button` id maps to one, so a new
/// button cannot be built without a handler (`button_action` and the app's
/// `on_settings_event` match on it exhaustively).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAction {
    /// The Colors page's reset: restore the default look (Global) or copy the
    /// global look (a customized space).
    ResetAppearance,
    /// The Layout page's "Fit to contents" for the selected space.
    FitToContents,
    /// The General page's "Check for updates now".
    CheckForUpdates,
    /// An animation page's "Open Windows animation settings", which opens
    /// the page holding Windows' "Animation effects" switch.
    OpenWindowsAnimationSettings,
}

/// The action of the button with field id `id`, or `None` for any other id.
pub fn button_action(id: u32) -> Option<ButtonAction> {
    match id {
        field::RESET => Some(ButtonAction::ResetAppearance),
        field::FIT_CONTENTS => Some(ButtonAction::FitToContents),
        field::CHECK_UPDATES_NOW => Some(ButtonAction::CheckForUpdates),
        id if tables::ANIMATION_PAGES
            .iter()
            .any(|page| page.open_settings == id) =>
        {
            Some(ButtonAction::OpenWindowsAnimationSettings)
        }
        _ => None,
    }
}

/// What an edit changed, so the caller knows what to refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    /// The look changed; restyle and repaint.
    Appearance,
    /// The grid changed; re-flow and repaint.
    Layout,
    /// Only pending (unapplied) settings changed.
    Pending,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{apply, create_space};
    use deskspace_core::Rect;
    use deskspace_platform::FieldKind;

    fn setup() -> (Config, Session) {
        let config = Config::new();
        let session = Session::new(&config);
        (config, session)
    }

    fn config_with_space(title: &str) -> (Config, SpaceId) {
        let mut config = Config::new();
        let id = create_space(&mut config.desktop, title, Rect::new(0, 0, 200, 200));
        (config, id)
    }

    fn nav(form: &SettingsForm) -> &SettingsNav {
        &form.nav
    }

    fn slider_value(form: &SettingsForm, id: u32) -> i32 {
        match &form.field(id).unwrap().kind {
            FieldKind::Slider { value, .. } => *value,
            other => panic!("expected a slider, got {other:?}"),
        }
    }

    fn form(config: &Config, session: &Session) -> SettingsForm {
        build_form(
            config,
            session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        )
    }

    /// A value of the kind the control for `kind` reports, or `None` for a
    /// field the user cannot edit (headings, notes, buttons, previews, and
    /// read-only text).
    fn edit_value(kind: &FieldKind) -> Option<FieldValue> {
        match kind {
            FieldKind::Choice { options, .. } => Some(FieldValue::Choice(options.len() - 1)),
            FieldKind::Toggle { on } => Some(FieldValue::Toggle(!on)),
            FieldKind::Slider { max, .. } => Some(FieldValue::Slider(*max)),
            FieldKind::Color { .. } => Some(FieldValue::Color(Color::rgb(1, 2, 3))),
            FieldKind::Text {
                read_only: false, ..
            } => Some(FieldValue::Text("Ctrl+Alt+Q".into())),
            FieldKind::Heading
            | FieldKind::Note
            | FieldKind::Button { .. }
            | FieldKind::SpacePreview(_)
            | FieldKind::Text {
                read_only: true, ..
            } => None,
        }
    }

    #[test]
    fn field_ids_are_unique() {
        let (config, session) = setup();
        let mut ids: Vec<u32> = form(&config, &session)
            .pages
            .iter()
            .flat_map(|page| page.fields.iter().map(|field| field.id))
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "two settings fields share an id");
    }

    #[test]
    fn every_editable_global_field_has_a_handler() {
        let (config, session) = setup();
        for page in form(&config, &session).pages {
            for field in page.fields {
                let Some(value) = edit_value(&field.kind) else {
                    continue;
                };
                let (mut config, mut session) = setup();
                let effect = apply_field(&mut config, &mut session, field.id, value, true);
                assert_ne!(
                    effect,
                    Effect::None,
                    "field {} ({:?}) on the {} page has no arm in `apply_field`",
                    field.id,
                    field.label,
                    page.title
                );
            }
        }
    }

    #[test]
    fn every_editable_space_field_has_a_handler() {
        let (mut config, id) = config_with_space("A");
        let seed = SpaceLook::from_global(&config.theme, &config.desktop.layout);
        apply(
            &mut config.desktop,
            Command::CustomizeSpace { id, on: true, seed },
        );
        let mut session = Session::new(&config);
        session.select_scope(Some(id));
        let pages = form(&config, &session).pages;
        for page in pages.iter().take(LOOK_PAGE_COUNT) {
            for field in &page.fields {
                let Some(value) = edit_value(&field.kind) else {
                    continue;
                };
                assert!(
                    space_edit(&config, &mut session, field.id, value, true).is_some(),
                    "look field {} ({:?}) has no arm in `apply_look_field`",
                    field.id,
                    field.label
                );
            }
        }
    }

    #[test]
    fn every_button_has_an_action() {
        let (config, session) = setup();
        for field in form(&config, &session)
            .pages
            .iter()
            .flat_map(|page| &page.fields)
        {
            if matches!(field.kind, FieldKind::Button { .. }) {
                assert!(
                    button_action(field.id).is_some(),
                    "button {} ({:?}) has no `ButtonAction`",
                    field.id,
                    field.label
                );
            }
        }
    }

    #[test]
    fn form_reflects_the_theme_and_disables_dependent_fields() {
        let (mut config, session) = setup();
        config.theme.backdrop = Backdrop::Solid;
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("C:/cfg.toml"),
            SpacePreview::default(),
        );
        let opacity = form.field(field::TINT_OPACITY).unwrap();
        assert!(!opacity.enabled);
        assert!(!form.field(field::TINT).unwrap().enabled);
        assert!(matches!(
            form.field(field::BACKDROP).unwrap().kind,
            FieldKind::Choice { selected: 3, .. }
        ));
        assert!(!form.apply_enabled);
    }

    #[test]
    fn layout_fields_are_disabled_while_matching_explorer() {
        let (mut config, session) = setup();
        // `match_explorer` defaults on, so Explorer owns the icon size and
        // spacing and the Layout controls are inert.
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("C:/cfg.toml"),
            SpacePreview::default(),
        );
        for id in [field::ICON_SIZE, field::GRID_GAP, field::GRID_PADDING] {
            assert!(!form.field(id).unwrap().enabled);
        }
        config.theme.preset = Preset::Custom;
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("C:/cfg.toml"),
            SpacePreview::default(),
        );
        for id in [field::ICON_SIZE, field::GRID_GAP, field::GRID_PADDING] {
            assert!(form.field(id).unwrap().enabled);
        }
    }

    #[test]
    fn new_spaces_page_shows_the_configured_defaults() {
        let (mut config, session) = setup();
        config.window.new_space_title = "Inbox".into();
        config.window.new_space_width = 300;
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert!(form.field(field::NEW_SPACE_TITLE).unwrap().enabled);
        assert_eq!(slider_value(&form, field::NEW_SPACE_WIDTH), 300);
        assert_eq!(
            form.field(field::NEW_SPACE_TITLE).unwrap().label,
            "Default title"
        );
    }

    #[test]
    fn the_general_page_offers_update_controls() {
        let (mut config, session) = setup();
        config.window.check_for_updates = true;
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        let toggle = form.field(field::CHECK_UPDATES).unwrap();
        assert!(matches!(toggle.kind, FieldKind::Toggle { on: true }));
        let button = form.field(field::CHECK_UPDATES_NOW).unwrap();
        // A build without an update key offers the controls, greyed out.
        let available = deskspace_update::is_configured();
        assert_eq!(toggle.enabled, available);
        assert_eq!(button.enabled, available);
        let install = form.field(field::INSTALL_UPDATES).unwrap();
        assert!(matches!(install.kind, FieldKind::Toggle { on: false }));
        assert_eq!(
            install.enabled,
            available && deskspace_update::is_installed()
        );
    }

    #[test]
    fn automatic_installs_need_the_automatic_check() {
        let (config, session) = setup();
        assert!(!config.window.check_for_updates);
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert!(!form.field(field::INSTALL_UPDATES).unwrap().enabled);
    }

    #[test]
    fn overlay_slider_marks_the_default_and_explorer_sizes() {
        let (config, session) = setup();
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        match &form.field(field::OVERLAY_SIZE).unwrap().kind {
            FieldKind::Slider { value, marks, .. } => {
                assert_eq!(*value, i32::from(OVERLAY_SIZE_DEFAULT));
                assert!(marks.iter().any(|mark| {
                    mark.value == i32::from(OVERLAY_SIZE_DEFAULT) && mark.label == "Default"
                }));
                assert!(
                    marks
                        .iter()
                        .any(|mark| mark.value == i32::from(OVERLAY_SIZE_RANGE.1))
                );
            }
            other => panic!("expected a slider, got {other:?}"),
        }
    }

    #[test]
    fn corner_radius_is_a_marked_slider() {
        let (mut config, mut session) = setup();
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        match &form.field(field::CORNERS).unwrap().kind {
            FieldKind::Slider {
                value, max, marks, ..
            } => {
                assert_eq!(*value, 8);
                assert_eq!(*max, 24);
                assert!(marks.iter().any(|mark| mark.value == 8));
            }
            other => panic!("expected a slider, got {other:?}"),
        }
        apply_field(
            &mut config,
            &mut session,
            field::CORNERS,
            FieldValue::Slider(40),
            true,
        );
        assert_eq!(config.theme.corner_radius, corner::MAX);
    }

    #[test]
    fn hotkey_errors_replace_the_hint() {
        let (config, mut session) = setup();
        session.hotkey_errors[0] = Some("bad".into());
        session.hotkey_errors[2] = Some("worse".into());
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert_eq!(form.field(field::HOTKEY_NOTE).unwrap().label, "bad");
        assert_eq!(
            form.field(field::HOTKEY_SETTINGS_NOTE).unwrap().label,
            "worse"
        );
    }

    #[test]
    fn global_lists_every_space_dimmed_until_customized() {
        let (mut config, _) = config_with_space("Work");
        let games = create_space(&mut config.desktop, "Games", Rect::new(0, 0, 200, 200));
        let seed = SpaceLook::from_global(&config.theme, &config.desktop.layout);
        apply(
            &mut config.desktop,
            Command::CustomizeSpace {
                id: games,
                on: true,
                seed,
            },
        );
        let session = Session::new(&config);
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        let nav = nav(&form);
        let scopes: Vec<(&str, bool, bool)> = nav
            .scopes
            .iter()
            .map(|scope| (scope.label.as_str(), scope.indent, scope.dimmed))
            .collect();
        assert_eq!(
            scopes,
            [
                ("Global", false, false),
                ("Work", true, true),
                ("Games", true, false)
            ]
        );
        assert_eq!(nav.selected_scope, 0);
        assert_eq!(
            nav.entries().count(),
            page::ABOUT + 1 + tables::ANIMATION_PAGES.len()
        );
        let header = nav.header.as_ref().unwrap();
        assert_eq!(header.title, "Global settings");
        assert!(header.switch.is_none());
        assert!(header.button.is_none(), "Global cannot be deleted");
    }

    #[test]
    fn a_space_shows_only_look_pages_and_a_customize_switch() {
        let (config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.page = page::ABOUT;
        session.select_scope(Some(id));
        assert_eq!(session.page, page::APPEARANCE, "About is Global only");
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        let nav = nav(&form);
        assert_eq!(nav.selected_scope, 1);
        let labels: Vec<&str> = nav.entries().map(|entry| entry.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Appearance",
                "Labels",
                "Layout",
                "Colors",
                "Advanced",
                "Shortcuts"
            ]
        );
        let switch = nav.header.as_ref().unwrap().switch.as_ref().unwrap();
        assert_eq!(switch.label, "Customize this space");
        assert!(!switch.on);
        let button = nav.header.as_ref().unwrap().button.as_ref().unwrap();
        assert_eq!(button.label, "Delete space");
        // The page shape never changes: General and About are still built.
        assert_eq!(
            form.pages.len(),
            page::ANIMATIONS + tables::ANIMATION_PAGES.len()
        );
    }

    fn animation_form(config: &Config, system_on: bool) -> SettingsForm {
        build_form(
            config,
            &Session::new(config),
            true,
            system_on,
            Path::new("x"),
            SpacePreview::default(),
        )
    }

    fn enabled(form: &SettingsForm, id: u32) -> bool {
        form.field(id).unwrap().enabled
    }

    #[test]
    fn the_animation_pages_are_a_group_of_their_own() {
        let (config, session) = setup();
        let form = form(&config, &session);
        let groups = &nav(&form).groups;
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[1].title, "Animations");
        let titles: Vec<&str> = groups[1]
            .entries
            .iter()
            .map(|entry| entry.label.as_str())
            .collect();
        assert_eq!(titles, ["Startup", "Delete", "Fold"]);
        assert_eq!(groups[1].entries[0].page, page::ANIMATIONS);
    }

    #[test]
    fn an_animation_page_greys_the_options_its_effect_does_not_use() {
        let mut config = Config::new();
        let hide_show = &mut config.window.animations.startup_exit;
        hide_show.effect = EffectChoice::Effect(AnimationEffect::Pixelate);
        let form = animation_form(&config, true);
        assert!(enabled(&form, field::ANIM_STARTUP_EFFECT));
        assert!(enabled(&form, field::ANIM_STARTUP_DURATION));
        assert!(enabled(&form, field::ANIM_STARTUP_DETAIL));
        assert!(!enabled(&form, field::ANIM_STARTUP_DIRECTION));

        config.window.animations.startup_exit.effect = EffectChoice::Effect(AnimationEffect::None);
        let form = animation_form(&config, true);
        assert!(enabled(&form, field::ANIM_STARTUP_EFFECT));
        assert!(!enabled(&form, field::ANIM_STARTUP_DURATION));
        assert!(!enabled(&form, field::ANIM_STARTUP_DETAIL));
    }

    #[test]
    fn following_another_kind_greys_its_options_but_not_the_duration() {
        let config = Config::new();
        let form = animation_form(&config, true);
        assert!(enabled(&form, field::ANIM_DELETE_DURATION));
        assert!(
            !enabled(&form, field::ANIM_DELETE_DIRECTION),
            "edited on Startup"
        );
        assert!(enabled(&form, field::ANIM_STARTUP_DIRECTION));
    }

    #[test]
    fn fold_offers_only_its_duration() {
        let form = animation_form(&Config::new(), true);
        assert!(form.field(field::ANIM_FOLD_DURATION).is_some());
        assert!(form.field(field::ANIM_FOLD_DETAIL).is_none());
        assert!(form.field(field::ANIM_FOLD_DIRECTION).is_none());
    }

    #[test]
    fn windows_switch_off_greys_every_animation_control_and_says_why() {
        let config = Config::new();
        let on = animation_form(&config, true);
        let off = animation_form(&config, false);
        assert!(on.same_shape(&off), "the switch must not change the shape");
        for page in tables::ANIMATION_PAGES {
            for id in [page.effect, page.duration, page.detail, page.direction] {
                if let Some(field) = off.field(id) {
                    assert!(!field.enabled, "field {id} on {}", page.title);
                }
            }
            assert!(off.field(page.note).unwrap().label.contains("is off"));
            assert!(!on.field(page.note).unwrap().label.contains("is off"));
            assert!(enabled(&off, page.open_settings), "the way out stays open");
        }
        assert!(!enabled(&off, field::ANIM_STARTUP_ORDER));
        assert!(!enabled(&off, field::ANIM_STAGGER));
    }

    #[test]
    fn the_startup_delay_is_grey_when_spaces_appear_all_at_once() {
        let mut config = Config::new();
        assert!(enabled(&animation_form(&config, true), field::ANIM_STAGGER));
        config.window.animations.startup = StartupOrder::AllAtOnce;
        assert!(!enabled(
            &animation_form(&config, true),
            field::ANIM_STAGGER
        ));
    }

    #[test]
    fn the_open_settings_buttons_open_windows_settings() {
        for page in tables::ANIMATION_PAGES {
            assert_eq!(
                button_action(page.open_settings),
                Some(ButtonAction::OpenWindowsAnimationSettings)
            );
        }
    }

    #[test]
    fn only_a_space_lists_its_shortcuts_page() {
        let (config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        assert!(!listed_pages(None).contains(&page::SHORTCUTS));
        assert!(listed_pages(Some(id)).contains(&page::SHORTCUTS));

        // The sixth entry of a space is its Shortcuts page, which stays
        // selected while the space is, is always enabled, and is left for
        // Appearance when the scope returns to Global.
        session.select_scope(Some(id));
        session.select_entry(5);
        assert_eq!(session.page, page::SHORTCUTS);
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert_eq!(form.nav.selected, 5);
        assert_eq!(form.nav.page_of(5), Some(page::SHORTCUTS));
        assert!(form.field(field::SPACE_PREVIEW).unwrap().enabled);
        session.select_scope(None);
        assert_eq!(session.page, page::APPEARANCE);
    }

    #[test]
    fn preview_colors_composite_the_tint_over_the_base() {
        let mut palette = Theme::default().palette(true);
        palette.tint = Color::rgba(0xFF, 0xFF, 0xFF, 0);
        palette.title_fill = Color::rgba(0, 0, 0, 255);
        let (fill, title, text) = space_preview_colors(&palette);
        assert_eq!(
            fill,
            Color::rgb(0x20, 0x20, 0x20),
            "a clear tint shows the base"
        );
        assert_eq!(title, Color::rgb(0, 0, 0), "an opaque shade covers it");
        assert_eq!(text.a, 255);
    }

    #[test]
    fn a_space_that_follows_global_shows_global_values_greyed() {
        let (mut config, id) = config_with_space("Work");
        config.theme.tint_opacity = 70;
        let mut kept = SpaceLook::from_global(&config.theme, &config.desktop.layout);
        kept.theme.tint_opacity = 10;
        apply(
            &mut config.desktop,
            Command::SetSpaceLook {
                id,
                custom: false,
                look: Some(kept),
            },
        );
        let mut session = Session::new(&config);
        session.select_scope(Some(id));

        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert_eq!(slider_value(&form, field::TINT_OPACITY), 70);
        assert!(!form.field(field::TINT_OPACITY).unwrap().enabled);
        assert!(!form.field(field::MODE).unwrap().enabled);

        let seed = SpaceLook::from_global(&config.theme, &config.desktop.layout);
        apply(
            &mut config.desktop,
            Command::CustomizeSpace { id, on: true, seed },
        );
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert_eq!(slider_value(&form, field::TINT_OPACITY), 10, "its own look");
        assert!(form.field(field::TINT_OPACITY).unwrap().enabled);
    }

    #[test]
    fn a_space_offers_fit_to_contents_on_the_layout_page() {
        let (mut config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.select_scope(Some(id));

        // An auto-sized space already fits, so the button is present but grey.
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert!(
            form.pages[page::LAYOUT]
                .fields
                .iter()
                .any(|field| field.id == field::FIT_CONTENTS)
        );
        assert!(!form.field(field::FIT_CONTENTS).unwrap().enabled);

        apply(&mut config.desktop, Command::KeepSize { id });
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert!(form.field(field::FIT_CONTENTS).unwrap().enabled);

        // A locked space cannot be fit, matching the space menu.
        apply(&mut config.desktop, Command::ToggleLock { id });
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert!(!form.field(field::FIT_CONTENTS).unwrap().enabled);
    }

    #[test]
    fn global_fit_to_contents_is_present_but_disabled() {
        let (config, _) = config_with_space("Work");
        let session = Session::new(&config);
        let form = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        // Present for Global too, so the page shape does not change with the
        // scope, but greyed because no space is being edited.
        let field = form.field(field::FIT_CONTENTS).expect("present");
        assert!(!field.enabled);
    }

    #[test]
    fn the_form_shape_is_stable_across_scopes() {
        // The settings window diffs a new form onto the live controls and
        // panics if the page shape changes, so Global and a space must build
        // the same pages and fields (selecting a space must not add a field).
        let (config, id) = config_with_space("Work");
        let global = build_form(
            &config,
            &Session::new(&config),
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        let mut session = Session::new(&config);
        session.select_scope(Some(id));
        let space = build_form(
            &config,
            &session,
            true,
            true,
            Path::new("x"),
            SpacePreview::default(),
        );
        assert!(global.same_shape(&space));
    }

    #[test]
    fn scope_indices_map_to_spaces() {
        let (config, id) = config_with_space("Work");
        assert_eq!(scope_at(&config, 0), None);
        assert_eq!(scope_at(&config, 1), Some(id));
        assert_eq!(scope_at(&config, 2), None);
        let mut session = Session::new(&config);
        assert_eq!(scope_index(&config, &session), 0);
        session.select_scope(Some(id));
        assert_eq!(scope_index(&config, &session), 1);
        session.select_scope(Some(SpaceId(999)));
        assert_eq!(scope_index(&config, &session), 0, "a deleted space");
    }
}
