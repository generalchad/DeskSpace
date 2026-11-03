//! The reference theme files must keep loading (TASKS.md N1).
//!
//! A shared pack file must keep loading forever: once a format is shared, a
//! renamed field breaks other people's files. Never edit or delete a
//! reference file; add a new one when the format changes.

use std::path::PathBuf;

use deskspace_config::ThemeFile;
use deskspace_core::layout::GAP_RANGE;
use deskspace_core::theme::{
    BORDER_OPACITY_RANGE, BORDER_WIDTH_RANGE, ICON_SIZE_RANGE, LABEL_SIZE_RANGE,
    OVERLAY_SIZE_RANGE, SHADOW_OPACITY_RANGE, SHADOW_SIZE_RANGE, TINT_OPACITY_RANGE,
    TITLE_SHADING_RANGE, corner,
};
use deskspace_core::{
    Backdrop, BorderSource, Color, LabelShadow, Preset, SpaceLook, Theme, ThemeMode,
};

fn reference(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/packs/themes")
        .join(name)
}

#[test]
fn minimal_loads_as_the_defaults() {
    let theme = ThemeFile::load(&reference("minimal.deskspace-theme.toml")).unwrap();
    assert_eq!(
        theme,
        ThemeFile {
            name: "Minimal".to_owned(),
            ..ThemeFile::default()
        }
    );
}

#[test]
fn full_loads_with_exactly_the_values_written() {
    let theme = ThemeFile::load(&reference("full.deskspace-theme.toml")).unwrap();
    assert_eq!(theme.format, 1);
    assert_eq!(theme.name, "Reference full");
    assert_eq!(theme.author.as_deref(), Some("DeskSpace contributors"));
    assert_eq!(
        theme.look,
        SpaceLook {
            theme: Theme {
                mode: ThemeMode::Dark,
                ui_mode: ThemeMode::Light,
                backdrop: Backdrop::Solid,
                corner_radius: 16,
                tint_opacity: 60,
                title_shading: 45,
                show_border: false,
                border_width: 3,
                border_source: BorderSource::Custom,
                border_color: Color::rgb(0x11, 0x22, 0x33),
                border_opacity: 80,
                show_shadow: false,
                shadow_size: 30,
                shadow_opacity: 60,
                label_lines: 1,
                label_size: 130,
                icon_size: 150,
                overlay_size: 60,
                label_shadow: LabelShadow::On,
                custom_colors: true,
                tint: Color::rgb(0x20, 0x40, 0x60),
                text: Color::rgb(0xF0, 0xE0, 0xD0),
                marquee_color: Color::rgb(0x33, 0x66, 0x99),
                show_marquee_border: false,
                preset: Preset::Custom,
                use_accent: true,
            },
            gap: 4,
            padding: 20,
        }
    );
}

#[test]
fn hostile_loads_with_each_value_at_the_edge_of_its_range() {
    let theme = ThemeFile::load(&reference("hostile.deskspace-theme.toml")).unwrap();
    assert!(
        theme.name.len() > 200,
        "the long name still loads without truncation"
    );
    let look = theme.look;
    assert_eq!(look.gap, GAP_RANGE.1);
    assert_eq!(look.padding, GAP_RANGE.0);
    assert_eq!(look.theme.corner_radius, corner::MAX);
    assert_eq!(look.theme.tint_opacity, TINT_OPACITY_RANGE.1);
    assert_eq!(look.theme.title_shading, TITLE_SHADING_RANGE.1);
    assert_eq!(look.theme.border_width, BORDER_WIDTH_RANGE.0);
    assert_eq!(look.theme.border_opacity, BORDER_OPACITY_RANGE.1);
    assert_eq!(look.theme.shadow_size, SHADOW_SIZE_RANGE.0);
    assert_eq!(look.theme.shadow_opacity, SHADOW_OPACITY_RANGE.1);
    assert_eq!(look.theme.label_lines, 1);
    assert_eq!(look.theme.label_size, LABEL_SIZE_RANGE.0);
    assert_eq!(look.theme.icon_size, ICON_SIZE_RANGE.0);
    assert_eq!(look.theme.overlay_size, OVERLAY_SIZE_RANGE.1);
}

#[test]
fn a_newer_format_still_loads() {
    let theme = ThemeFile::load(&reference("newer.deskspace-theme.toml")).unwrap();
    assert_eq!(theme.format, 99);
    assert_eq!(theme.name, "Future format");
    assert_eq!(theme.look.gap, 8);
    assert_eq!(theme.look.theme.mode, ThemeMode::Light);
}

#[test]
fn a_wrong_type_is_an_error_and_does_not_panic() {
    let result = ThemeFile::load(&reference("wrong-type.deskspace-theme.toml"));
    assert!(result.is_err());
}
