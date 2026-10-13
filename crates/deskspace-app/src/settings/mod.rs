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

pub use apply::{apply_field, customize_space, match_global, reset_appearance, space_edit};
pub use ids::{field, page};
pub use session::{Session, SpaceSnapshot, needs_relayout, needs_restyle};
pub use tables::{HOTKEY_BINDINGS, hotkey_setting, set_hotkey_setting};

use std::path::Path;

use deskspace_config::{Config, NEW_SPACE_SIZE_RANGE, WindowSettings};
use deskspace_core::layout::{GAP_RANGE, SNAP_RANGE};
use deskspace_core::theme::{
    ICON_SIZE_RANGE, LABEL_SIZE_RANGE, OVERLAY_SIZE_DEFAULT, OVERLAY_SIZE_RANGE, corner,
};
use deskspace_core::{
    Backdrop, Command, GridSpec, LabelShadow, Preset, Space, SpaceId, SpaceLook, Theme, ThemeMode,
};
use deskspace_platform::{
    Field, FieldValue, SettingsButton, SettingsForm, SettingsHeader, SettingsNav, SettingsNavEntry,
    SettingsNavGroup, SettingsPage, SettingsScope, SettingsSwitch, SliderMark,
};

use pages::{about_page, general_page, hotkeys_page, look_pages, new_spaces_page};

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
                .spaces
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
        .and_then(|index| config.desktop.spaces.get(index))
        .map(|space| space.id)
}

pub fn build_form(
    config: &Config,
    session: &Session,
    system_dark: bool,
    config_path: &Path,
) -> SettingsForm {
    let space = scope_space(config, session);
    let global = &config.theme;
    // A space shows what it is drawn with: its own look while customized,
    // else the global values, greyed until the switch is turned on.
    let theme = space.map_or(global, |space| space.theme(global));
    let grid = space.map_or(config.desktop.layout, |space| {
        space.grid(config.desktop.layout)
    });
    let editable = space.is_none_or(|space| space.custom);

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
            .enabled(space.is_some_and(|space| space.user_sized && !space.locked)),
    );
    pages.push(general_page(config));
    pages.push(new_spaces_page(config));
    pages.push(hotkeys_page(session));
    pages.push(about_page(config_path));
    debug_assert_eq!(pages.len(), page::ABOUT + 1);

    let page_count = if space.is_some() {
        LOOK_PAGE_COUNT
    } else {
        pages.len()
    };
    let entries = pages[..page_count]
        .iter()
        .enumerate()
        .map(|(index, page)| SettingsNavEntry {
            label: page.title.clone(),
            page: index,
            marked: false,
        })
        .collect();

    let mut scopes = vec![SettingsScope {
        label: "Global".into(),
        indent: false,
        dimmed: false,
    }];
    scopes.extend(config.desktop.spaces.iter().map(|space| SettingsScope {
        label: space.title.clone(),
        indent: true,
        dimmed: !space.custom,
    }));
    let header = match space {
        None => SettingsHeader {
            title: "Global settings".into(),
            switch: None,
            button: None,
        },
        Some(space) => SettingsHeader {
            title: space.title.clone(),
            switch: Some(SettingsSwitch {
                label: "Customize this space".into(),
                on: space.custom,
            }),
            button: Some(SettingsButton {
                // The ellipsis promises a confirmation dialog, which only a
                // non-empty space shows (matching the space menu).
                label: if space.items.is_empty() {
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
            groups: vec![SettingsNavGroup {
                title: String::new(),
                entries,
            }],
            selected: session.page.min(page_count - 1),
            header: Some(header),
        },
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
    use deskspace_core::Rect;
    use deskspace_platform::FieldKind;

    fn setup() -> (Config, Session) {
        let config = Config::new();
        let session = Session::new(&config);
        (config, session)
    }

    fn config_with_space(title: &str) -> (Config, SpaceId) {
        let mut config = Config::new();
        let id = config.desktop.add_space(title, Rect::new(0, 0, 200, 200));
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

    #[test]
    fn form_reflects_the_theme_and_disables_dependent_fields() {
        let (mut config, session) = setup();
        config.theme.backdrop = Backdrop::Solid;
        let form = build_form(&config, &session, true, Path::new("C:/cfg.toml"));
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
        let form = build_form(&config, &session, true, Path::new("C:/cfg.toml"));
        for id in [field::ICON_SIZE, field::GRID_GAP, field::GRID_PADDING] {
            assert!(!form.field(id).unwrap().enabled);
        }
        config.theme.preset = Preset::Custom;
        let form = build_form(&config, &session, true, Path::new("C:/cfg.toml"));
        for id in [field::ICON_SIZE, field::GRID_GAP, field::GRID_PADDING] {
            assert!(form.field(id).unwrap().enabled);
        }
    }

    #[test]
    fn new_spaces_page_shows_the_configured_defaults() {
        let (mut config, session) = setup();
        config.window.new_space_title = "Inbox".into();
        config.window.new_space_width = 300;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(form.field(field::NEW_SPACE_TITLE).unwrap().enabled);
        assert_eq!(slider_value(&form, field::NEW_SPACE_WIDTH), 300);
        assert_eq!(
            form.field(field::NEW_SPACE_TITLE).unwrap().label,
            "Default title"
        );
    }

    #[test]
    fn overlay_slider_marks_the_default_and_explorer_sizes() {
        let (config, session) = setup();
        let form = build_form(&config, &session, true, Path::new("x"));
        match &form.field(field::OVERLAY_SIZE).unwrap().kind {
            FieldKind::Slider { value, marks, .. } => {
                assert_eq!(*value, OVERLAY_SIZE_DEFAULT.into());
                assert!(marks.iter().any(
                    |mark| mark.value == OVERLAY_SIZE_DEFAULT.into() && mark.label == "Default"
                ));
                assert!(
                    marks
                        .iter()
                        .any(|mark| mark.value == OVERLAY_SIZE_RANGE.1.into())
                );
            }
            other => panic!("expected a slider, got {other:?}"),
        }
    }

    #[test]
    fn corner_radius_is_a_marked_slider() {
        let (mut config, mut session) = setup();
        let form = build_form(&config, &session, true, Path::new("x"));
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
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(form.field(field::HOTKEY_NOTE).unwrap().label, "bad");
        assert_eq!(
            form.field(field::HOTKEY_SETTINGS_NOTE).unwrap().label,
            "worse"
        );
    }

    #[test]
    fn global_lists_every_space_dimmed_until_customized() {
        let (mut config, _) = config_with_space("Work");
        let games = config.desktop.add_space("Games", Rect::new(0, 0, 200, 200));
        config.desktop.space_mut(games).unwrap().custom = true;
        let session = Session::new(&config);
        let form = build_form(&config, &session, true, Path::new("x"));
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
        assert_eq!(nav.entries().count(), page::ABOUT + 1);
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
        let form = build_form(&config, &session, true, Path::new("x"));
        let nav = nav(&form);
        assert_eq!(nav.selected_scope, 1);
        let labels: Vec<&str> = nav.entries().map(|entry| entry.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Appearance", "Labels", "Layout", "Colors", "Advanced"]
        );
        let switch = nav.header.as_ref().unwrap().switch.as_ref().unwrap();
        assert_eq!(switch.label, "Customize this space");
        assert!(!switch.on);
        let button = nav.header.as_ref().unwrap().button.as_ref().unwrap();
        assert_eq!(button.label, "Delete space");
        // The page shape never changes: General and About are still built.
        assert_eq!(form.pages.len(), page::ABOUT + 1);
    }

    #[test]
    fn a_space_that_follows_global_shows_global_values_greyed() {
        let (mut config, id) = config_with_space("Work");
        config.theme.tint_opacity = 70;
        let mut kept = SpaceLook::from_global(&config.theme, &config.desktop.layout);
        kept.theme.tint_opacity = 10;
        config.desktop.space_mut(id).unwrap().look = Some(kept);
        let mut session = Session::new(&config);
        session.select_scope(Some(id));

        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(slider_value(&form, field::TINT_OPACITY), 70);
        assert!(!form.field(field::TINT_OPACITY).unwrap().enabled);
        assert!(!form.field(field::MODE).unwrap().enabled);

        config.desktop.space_mut(id).unwrap().custom = true;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert_eq!(slider_value(&form, field::TINT_OPACITY), 10, "its own look");
        assert!(form.field(field::TINT_OPACITY).unwrap().enabled);
    }

    #[test]
    fn a_space_offers_fit_to_contents_on_the_layout_page() {
        let (mut config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.select_scope(Some(id));

        // An auto-sized space already fits, so the button is present but grey.
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(
            form.pages[page::LAYOUT]
                .fields
                .iter()
                .any(|field| field.id == field::FIT_CONTENTS)
        );
        assert!(!form.field(field::FIT_CONTENTS).unwrap().enabled);

        config.desktop.space_mut(id).unwrap().user_sized = true;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(form.field(field::FIT_CONTENTS).unwrap().enabled);

        // A locked space cannot be fit, matching the space menu.
        config.desktop.space_mut(id).unwrap().locked = true;
        let form = build_form(&config, &session, true, Path::new("x"));
        assert!(!form.field(field::FIT_CONTENTS).unwrap().enabled);
    }

    #[test]
    fn global_fit_to_contents_is_present_but_disabled() {
        let (config, _) = config_with_space("Work");
        let session = Session::new(&config);
        let form = build_form(&config, &session, true, Path::new("x"));
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
        let global = build_form(&config, &Session::new(&config), true, Path::new("x"));
        let mut session = Session::new(&config);
        session.select_scope(Some(id));
        let space = build_form(&config, &session, true, Path::new("x"));
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
