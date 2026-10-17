//! Applying form edits to the configuration: global edits write it
//! directly, and a space's edits become Commands.

use super::ids::field;
use super::session::Session;
use super::tables::{
    BACKDROPS, HOTKEY_BINDINGS, LABEL_LINES_OPTIONS, LABEL_SHADOW_OPTIONS, MODES, PRESETS,
    is_hotkey_field,
};
use super::*;
use super::{Effect, scope_space};

/// Applies a look-page edit to `look`. `None` when `id` is not a look field.
fn apply_look_field(
    look: &mut SpaceLook,
    id: u32,
    value: &FieldValue,
    system_dark: bool,
) -> Option<Effect> {
    let theme = &mut look.theme;
    Some(match (id, value) {
        (field::MODE, FieldValue::Choice(index)) => {
            theme.mode = MODES.get(*index).map_or(theme.mode, |entry| entry.0);
            Effect::Appearance
        }
        (field::BACKDROP, FieldValue::Choice(index)) => {
            theme.backdrop = BACKDROPS
                .get(*index)
                .map_or(theme.backdrop, |entry| entry.0);
            Effect::Appearance
        }
        (field::CORNERS, FieldValue::Slider(radius)) => {
            theme.corner_radius = (*radius).clamp(0, corner::MAX.into()) as u8;
            Effect::Appearance
        }
        (field::TINT_OPACITY, FieldValue::Slider(value)) => {
            theme.tint_opacity =
                (*value).clamp(TINT_OPACITY_RANGE.0.into(), TINT_OPACITY_RANGE.1.into()) as u8;
            Effect::Appearance
        }
        (field::TITLE_SHADING, FieldValue::Slider(value)) => {
            theme.title_shading =
                (*value).clamp(TITLE_SHADING_RANGE.0.into(), TITLE_SHADING_RANGE.1.into()) as u8;
            Effect::Appearance
        }
        (field::BORDER, FieldValue::Toggle(on)) => {
            theme.show_border = *on;
            Effect::Appearance
        }
        (field::PRESET, FieldValue::Choice(index)) => {
            theme.preset = PRESETS.get(*index).map_or(theme.preset, |entry| entry.0);
            // Mirroring also imports Explorer's icon size and item spacing, so
            // the grid has to re-flow, not just restyle.
            Effect::Layout
        }
        (field::LABEL_LINES, FieldValue::Choice(index)) => {
            theme.label_lines = LABEL_LINES_OPTIONS
                .get(*index)
                .map_or(theme.label_lines, |entry| entry.0);
            Effect::Appearance
        }
        (field::LABEL_SIZE, FieldValue::Slider(value)) => {
            theme.label_size =
                (*value).clamp(LABEL_SIZE_RANGE.0.into(), LABEL_SIZE_RANGE.1.into()) as u8;
            Effect::Appearance
        }
        (field::LABEL_SHADOW, FieldValue::Choice(index)) => {
            theme.label_shadow = LABEL_SHADOW_OPTIONS
                .get(*index)
                .map_or(theme.label_shadow, |entry| entry.0);
            Effect::Appearance
        }
        (field::ICON_SIZE, FieldValue::Slider(value)) => {
            theme.icon_size =
                (*value).clamp(ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into()) as u8;
            Effect::Layout
        }
        (field::OVERLAY_SIZE, FieldValue::Slider(value)) => {
            theme.overlay_size =
                (*value).clamp(OVERLAY_SIZE_RANGE.0.into(), OVERLAY_SIZE_RANGE.1.into()) as u8;
            Effect::Appearance
        }
        (field::GRID_GAP, FieldValue::Slider(value)) => {
            look.gap = (*value).clamp(GAP_RANGE.0, GAP_RANGE.1);
            Effect::Layout
        }
        (field::GRID_PADDING, FieldValue::Slider(value)) => {
            look.padding = (*value).clamp(GAP_RANGE.0, GAP_RANGE.1);
            Effect::Layout
        }
        (field::CUSTOM_COLORS, FieldValue::Toggle(on)) => {
            if *on && !theme.custom_colors {
                // Start from what is on screen rather than a stale color.
                let palette = theme.palette(system_dark);
                theme.tint = palette.tint.with_alpha(255);
                theme.text = palette.title_text;
            }
            theme.custom_colors = *on;
            Effect::Appearance
        }
        (field::TINT, FieldValue::Color(color)) => {
            theme.tint = color.with_alpha(255);
            Effect::Appearance
        }
        (field::TEXT, FieldValue::Color(color)) => {
            theme.text = color.with_alpha(255);
            Effect::Appearance
        }
        (field::USE_ACCENT, FieldValue::Toggle(on)) => {
            theme.use_accent = *on;
            Effect::Appearance
        }
        (field::MARQUEE_COLOR, FieldValue::Color(color)) => {
            theme.marquee_color = color.with_alpha(255);
            Effect::Appearance
        }
        (field::MARQUEE_BORDER, FieldValue::Toggle(on)) => {
            theme.show_marquee_border = *on;
            Effect::Appearance
        }
        _ => return None,
    })
}

/// Applies an edit to the global settings.
pub fn apply_field(
    config: &mut Config,
    session: &mut Session,
    id: u32,
    value: FieldValue,
    system_dark: bool,
) -> Effect {
    let mut look = SpaceLook::from_global(&config.theme, &config.desktop.layout);
    let effect = match apply_look_field(&mut look, id, &value, system_dark) {
        Some(effect) => {
            config.theme = look.theme;
            config.desktop.layout.gap = look.gap;
            config.desktop.layout.padding = look.padding;
            effect
        }
        None => match (id, value) {
            (field::UI_MODE, FieldValue::Choice(index)) => {
                config.theme.ui_mode = MODES
                    .get(index)
                    .map_or(config.theme.ui_mode, |entry| entry.0);
                Effect::Appearance
            }
            (field::START_WITH_WINDOWS, FieldValue::Toggle(on)) => {
                config.window.start_with_windows = on;
                Effect::Pending
            }
            (field::TAKE_OVER_DESKTOP, FieldValue::Toggle(on)) => {
                config.window.take_over_desktop = on;
                Effect::Pending
            }
            (id, FieldValue::Text(text)) if is_hotkey_field(id) => {
                let slot = HOTKEY_BINDINGS
                    .iter()
                    .position(|binding| binding.field == id)
                    .expect("a hotkey field is in the table");
                session.hotkey_texts[slot] = text;
                session.hotkey_errors[slot] = None;
                Effect::Pending
            }
            (field::START_HIDDEN, FieldValue::Toggle(on)) => {
                config.window.start_hidden = on;
                Effect::Pending
            }
            (field::CHECK_UPDATES, FieldValue::Toggle(on)) => {
                config.window.check_for_updates = on;
                Effect::Pending
            }
            (field::INSTALL_UPDATES, FieldValue::Toggle(on)) => {
                config.window.install_updates = on;
                Effect::Pending
            }
            (field::SNAP, FieldValue::Slider(value)) => {
                config.desktop.layout.snap = value.clamp(SNAP_RANGE.0, SNAP_RANGE.1);
                Effect::Pending
            }
            (field::FIT_TO_GRID, FieldValue::Toggle(on)) => {
                config.desktop.layout.fit_to_grid = on;
                Effect::Pending
            }
            (field::NEW_SPACE_TITLE, FieldValue::Text(text)) => {
                config.window.new_space_title = text;
                Effect::Pending
            }
            (field::NEW_SPACE_WIDTH, FieldValue::Slider(value)) => {
                config.window.new_space_width =
                    value.clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1);
                Effect::Pending
            }
            (field::NEW_SPACE_HEIGHT, FieldValue::Slider(value)) => {
                config.window.new_space_height =
                    value.clamp(NEW_SPACE_SIZE_RANGE.0, NEW_SPACE_SIZE_RANGE.1);
                Effect::Pending
            }
            (field::NEW_SPACE_LOCKED, FieldValue::Toggle(on)) => {
                config.window.new_space_locked = on;
                Effect::Pending
            }
            _ => Effect::None,
        },
    };
    if effect != Effect::None {
        session.dirty = true;
    }
    effect
}

/// The command for an edit on a customized space's page, with what it changes.
/// `None` when Global is selected, the space follows Global, or `id` is not
/// a look field.
pub fn space_edit(
    config: &Config,
    session: &mut Session,
    id: u32,
    value: FieldValue,
    system_dark: bool,
) -> Option<(SpaceId, Command, Effect)> {
    let space = scope_space(config, session).filter(|space| space.custom())?;
    let mut look = space
        .look()
        .cloned()
        .unwrap_or_else(|| SpaceLook::from_global(&config.theme, &config.desktop.layout));
    let effect = apply_look_field(&mut look, id, &value, system_dark)?;
    session.dirty = true;
    let command = Command::SetSpaceLook {
        id: space.id,
        custom: true,
        look: Some(look),
    };
    Some((space.id, command, effect))
}

/// The command for the header switch: customize the selected space, or let
/// it follow Global again (keeping its look for later).
pub fn customize_space(config: &Config, session: &mut Session, on: bool) -> Option<Command> {
    let space = scope_space(config, session)?;
    session.dirty = true;
    Some(Command::CustomizeSpace {
        id: space.id,
        on,
        seed: SpaceLook::from_global(&config.theme, &config.desktop.layout),
    })
}

/// The command for a customized space's "Match global appearance": a look
/// copied from the global settings.
pub fn match_global(config: &Config, session: &mut Session) -> Option<Command> {
    let space = scope_space(config, session).filter(|space| space.custom())?;
    session.dirty = true;
    Some(Command::SetSpaceLook {
        id: space.id,
        custom: true,
        look: Some(SpaceLook::from_global(
            &config.theme,
            &config.desktop.layout,
        )),
    })
}

pub fn reset_appearance(config: &mut Config, session: &mut Session) {
    config.theme = Theme::default();
    session.dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::Color;
    use deskspace_core::Rect;

    const BOUNDS: Rect = Rect::new(0, 0, 1920, 1040);

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

    #[test]
    fn ui_mode_choice_applies_live() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::UI_MODE,
            FieldValue::Choice(1),
            true,
        );
        assert_eq!(effect, Effect::Appearance);
        assert_eq!(config.theme.ui_mode, ThemeMode::Dark);
    }

    #[test]
    fn start_with_windows_is_pending() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::START_WITH_WINDOWS,
            FieldValue::Toggle(true),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(config.window.start_with_windows);
    }

    #[test]
    fn fit_to_grid_is_pending() {
        let (mut config, mut session) = setup();
        assert!(config.desktop.layout.fit_to_grid);
        let effect = apply_field(
            &mut config,
            &mut session,
            field::FIT_TO_GRID,
            FieldValue::Toggle(false),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(!config.desktop.layout.fit_to_grid);
    }

    #[test]
    fn take_over_desktop_is_pending() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::TAKE_OVER_DESKTOP,
            FieldValue::Toggle(true),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(config.window.take_over_desktop);
    }

    #[test]
    fn check_for_updates_is_pending() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::CHECK_UPDATES,
            FieldValue::Toggle(true),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(config.window.check_for_updates);
    }

    #[test]
    fn install_updates_is_pending() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::INSTALL_UPDATES,
            FieldValue::Toggle(true),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(config.window.install_updates);
    }

    #[test]
    fn preset_choice_applies_live() {
        let (mut config, mut session) = setup();
        assert_eq!(config.theme.preset, Preset::MatchExplorer);
        let effect = apply_field(
            &mut config,
            &mut session,
            field::PRESET,
            FieldValue::Choice(1),
            true,
        );
        assert_eq!(effect, Effect::Layout);
        assert_eq!(config.theme.preset, Preset::Custom);
        assert!(!config.theme.match_explorer());
        apply_field(
            &mut config,
            &mut session,
            field::PRESET,
            FieldValue::Choice(0),
            true,
        );
        assert_eq!(config.theme.preset, Preset::MatchExplorer);
    }

    #[test]
    fn cancel_restores_the_saved_settings() {
        let (mut config, mut session) = setup();
        apply_field(
            &mut config,
            &mut session,
            field::TINT_OPACITY,
            FieldValue::Slider(90),
            true,
        );
        apply_field(
            &mut config,
            &mut session,
            field::START_HIDDEN,
            FieldValue::Toggle(true),
            true,
        );
        session.revert(&mut config);
        assert_eq!(config, Config::new());
    }

    #[test]
    fn enabling_custom_colors_starts_from_the_current_palette() {
        let (mut config, mut session) = setup();
        config.theme.mode = ThemeMode::Light;
        apply_field(
            &mut config,
            &mut session,
            field::CUSTOM_COLORS,
            FieldValue::Toggle(true),
            true,
        );
        assert_eq!(config.theme.tint, Color::rgb(243, 243, 243));
        assert_eq!(config.theme.text, Color::rgb(26, 26, 26));
    }

    #[test]
    fn selection_box_color_and_border_apply_live() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::MARQUEE_COLOR,
                FieldValue::Color(Color::rgba(10, 20, 30, 0)),
                true,
            ),
            Effect::Appearance
        );
        assert_eq!(config.theme.marquee_color, Color::rgb(10, 20, 30));
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::MARQUEE_BORDER,
                FieldValue::Toggle(false),
                true,
            ),
            Effect::Appearance
        );
        assert!(!config.theme.show_marquee_border);
    }

    #[test]
    fn hotkey_text_is_pending_until_applied() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::HOTKEY,
            FieldValue::Text("Ctrl+Shift+F".into()),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert_eq!(config.window.toggle_hotkey, "Ctrl+Alt+D");
        assert_eq!(session.hotkey_texts[0], "Ctrl+Shift+F");
        let form = build_form(
            &config,
            &session,
            true,
            Path::new("x"),
            deskspace_platform::SpacePreview::default(),
        );
        assert!(form.apply_enabled);
    }

    #[test]
    fn a_second_hotkey_is_staged_like_the_first() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::HOTKEY_NEW_SPACE,
            FieldValue::Text("Ctrl+Alt+N".into()),
            true,
        );
        assert_eq!(effect, Effect::Pending);
        assert!(config.window.new_space_hotkey.is_empty());
        assert_eq!(session.hotkey_texts[1], "Ctrl+Alt+N");
    }

    #[test]
    fn new_space_defaults_are_pending_and_update_the_config() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::NEW_SPACE_TITLE,
                FieldValue::Text("Inbox".into()),
                true,
            ),
            Effect::Pending
        );
        assert_eq!(config.window.new_space_title, "Inbox");
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::NEW_SPACE_WIDTH,
                FieldValue::Slider(320),
                true,
            ),
            Effect::Pending
        );
        assert_eq!(config.window.new_space_width, 320);
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::NEW_SPACE_HEIGHT,
                FieldValue::Slider(9_000),
                true,
            ),
            Effect::Pending
        );
        assert_eq!(config.window.new_space_height, NEW_SPACE_SIZE_RANGE.1);
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::NEW_SPACE_LOCKED,
                FieldValue::Toggle(true),
                true,
            ),
            Effect::Pending
        );
        assert!(config.window.new_space_locked);
        assert!(session.dirty);
    }

    #[test]
    fn slider_values_are_clamped() {
        let (mut config, mut session) = setup();
        apply_field(
            &mut config,
            &mut session,
            field::TITLE_SHADING,
            FieldValue::Slider(250),
            true,
        );
        assert_eq!(config.theme.title_shading, 100);
    }

    #[test]
    fn grid_sliders_report_a_layout_effect_and_clamp() {
        let (mut config, mut session) = setup();
        let effect = apply_field(
            &mut config,
            &mut session,
            field::GRID_GAP,
            FieldValue::Slider(30),
            true,
        );
        assert_eq!(effect, Effect::Layout);
        assert_eq!(config.desktop.layout.gap, 24);
        apply_field(
            &mut config,
            &mut session,
            field::GRID_PADDING,
            FieldValue::Slider(4),
            true,
        );
        assert_eq!(config.desktop.layout.padding, 4);
    }

    #[test]
    fn icon_size_reports_a_layout_effect_and_clamps() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::ICON_SIZE,
                FieldValue::Slider(150),
                true,
            ),
            Effect::Layout
        );
        assert_eq!(config.theme.icon_size, 150);
        apply_field(
            &mut config,
            &mut session,
            field::ICON_SIZE,
            FieldValue::Slider(10),
            true,
        );
        assert_eq!(config.theme.icon_size, 50);
        apply_field(
            &mut config,
            &mut session,
            field::ICON_SIZE,
            FieldValue::Slider(500),
            true,
        );
        assert_eq!(config.theme.icon_size, 200);
    }

    #[test]
    fn overlay_size_reports_an_appearance_effect_and_clamps() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::OVERLAY_SIZE,
                FieldValue::Slider(60),
                true,
            ),
            Effect::Appearance
        );
        assert_eq!(config.theme.overlay_size, 60);
        apply_field(
            &mut config,
            &mut session,
            field::OVERLAY_SIZE,
            FieldValue::Slider(0),
            true,
        );
        assert_eq!(config.theme.overlay_size, OVERLAY_SIZE_RANGE.0);
        apply_field(
            &mut config,
            &mut session,
            field::OVERLAY_SIZE,
            FieldValue::Slider(250),
            true,
        );
        assert_eq!(config.theme.overlay_size, OVERLAY_SIZE_RANGE.1);
    }

    #[test]
    fn label_style_edits_apply_live_and_clamp() {
        let (mut config, mut session) = setup();
        assert_eq!(
            apply_field(
                &mut config,
                &mut session,
                field::LABEL_LINES,
                FieldValue::Choice(0),
                true,
            ),
            Effect::Appearance
        );
        assert_eq!(config.theme.label_lines, 1);
        apply_field(
            &mut config,
            &mut session,
            field::LABEL_SIZE,
            FieldValue::Slider(200),
            true,
        );
        assert_eq!(config.theme.label_size, 160);
        apply_field(
            &mut config,
            &mut session,
            field::LABEL_SHADOW,
            FieldValue::Choice(2),
            true,
        );
        assert_eq!(config.theme.label_shadow, LabelShadow::Off);
    }

    #[test]
    fn space_edits_become_commands_and_leave_the_global_theme_alone() {
        let (mut config, id) = config_with_space("Work");
        let mut session = Session::new(&config);
        session.select_scope(Some(id));
        let edit = |config: &Config, session: &mut Session| {
            space_edit(
                config,
                session,
                field::ICON_SIZE,
                FieldValue::Slider(150),
                true,
            )
        };
        assert!(edit(&config, &mut session).is_none(), "follows Global");

        assert!(
            config
                .desktop
                .apply(
                    Command::CustomizeSpace {
                        id,
                        on: true,
                        seed: SpaceLook::from_global(&config.theme, &config.desktop.layout),
                    },
                    BOUNDS,
                )
                .is_applied()
        );
        let (space, command, effect) = edit(&config, &mut session).unwrap();
        assert_eq!(space, id);
        assert_eq!(effect, Effect::Layout);
        assert!(session.dirty);
        match command {
            Command::SetSpaceLook {
                custom: true,
                look: Some(look),
                ..
            } => {
                assert_eq!(look.theme.icon_size, 150);
                assert_eq!(look.theme.tint_opacity, config.theme.tint_opacity);
            }
            other => panic!("expected SetSpaceLook, got {other:?}"),
        }
        assert_eq!(config.theme.icon_size, 100);
        assert!(
            space_edit(
                &config,
                &mut session,
                field::HOTKEY,
                FieldValue::Text("x".into()),
                true
            )
            .is_none(),
            "General is Global only"
        );
    }

    #[test]
    fn customizing_seeds_from_global_and_match_global_copies_it() {
        let (mut config, id) = config_with_space("Work");
        config.theme.corner_radius = 4;
        let mut session = Session::new(&config);
        assert!(
            customize_space(&config, &mut session, true).is_none(),
            "Global"
        );
        session.select_scope(Some(id));
        match customize_space(&config, &mut session, true) {
            Some(Command::CustomizeSpace {
                id: target,
                on,
                seed,
            }) => {
                assert_eq!(target, id);
                assert!(on);
                assert_eq!(seed.theme.corner_radius, 4);
            }
            other => panic!("expected CustomizeSpace, got {other:?}"),
        }
        assert!(
            match_global(&config, &mut session).is_none(),
            "not customized"
        );
        assert!(
            config
                .desktop
                .apply(
                    Command::CustomizeSpace {
                        id,
                        on: true,
                        seed: SpaceLook::from_global(&config.theme, &config.desktop.layout),
                    },
                    BOUNDS,
                )
                .is_applied()
        );
        assert!(matches!(
            match_global(&config, &mut session),
            Some(Command::SetSpaceLook { custom: true, look: Some(look), .. })
                if look.theme.corner_radius == 4
        ));
    }
}
