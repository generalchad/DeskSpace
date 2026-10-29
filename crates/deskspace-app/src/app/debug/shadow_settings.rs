//! The debug window's shadow settings (ROADMAP D15): one "Shadow settings"
//! box in the Windows section editing the global shadow look as the
//! settings window does, and saving it at once.
//!
//! "Show shadow" is a toggle; "Shadow size" and "Shadow opacity" are
//! choices (since the debug window has no sliders), greyed while "Show
//! shadow" is off, with a value between presets listed in place.

use deskspace_core::Theme;
use deskspace_core::theme::{SHADOW_OPACITY_RANGE, SHADOW_SIZE_RANGE};
use deskspace_platform::{DebugControl, DebugControlKind, FieldValue, Ui};

use super::DebugInput;
use crate::app::App;
use crate::settings::{self, field};

/// The box's title.
const GROUP: &str = "Shadow settings";
/// Where these controls' ids start, above `DebugId` and `ANIMATION_ID_BASE`.
const SHADOW_ID_BASE: u32 = 10_100;

/// The shadow sizes offered, in 96-DPI pixels (`SHADOW_SIZE_RANGE`).
const SIZE_PRESETS: [u8; 8] = [2, 4, 8, 12, 16, 24, 32, 48];
/// The shadow opacities offered, in percent (`SHADOW_OPACITY_RANGE`).
const OPACITY_PRESETS: [u8; 8] = [0, 15, 25, 35, 50, 70, 85, 100];

/// A row of the box. Its id is its place in `ALL`; never reorder them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setting {
    Show,
    Size,
    Opacity,
}

impl Setting {
    const ALL: [Self; 3] = [Self::Show, Self::Size, Self::Opacity];

    fn label(self) -> &'static str {
        match self {
            Self::Show => "Show shadow",
            Self::Size => "Shadow size",
            Self::Opacity => "Shadow opacity",
        }
    }

    fn field(self) -> u32 {
        match self {
            Self::Show => field::SHADOW,
            Self::Size => field::SHADOW_SIZE,
            Self::Opacity => field::SHADOW_OPACITY,
        }
    }

    fn id(self) -> u32 {
        let index = Self::ALL
            .iter()
            .position(|setting| *setting == self)
            .expect("every setting is in ALL");
        SHADOW_ID_BASE + index as u32
    }

    fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|setting| setting.id() == id)
    }
}

/// One row of the box.
#[derive(Debug, Clone, PartialEq)]
struct ShadowRow {
    setting: Setting,
    field: u32,
    kind: DebugControlKind,
    /// For a choice row, the edit each option makes.
    options: Vec<(String, FieldValue)>,
}

/// `theme`'s shadow settings as the box's rows, in `Setting::ALL` order.
fn shadow_rows(theme: &Theme) -> Vec<ShadowRow> {
    let show = theme.show_shadow;
    let size = theme
        .shadow_size
        .clamp(SHADOW_SIZE_RANGE.0, SHADOW_SIZE_RANGE.1);
    let opacity = theme
        .shadow_opacity
        .clamp(SHADOW_OPACITY_RANGE.0, SHADOW_OPACITY_RANGE.1);
    Setting::ALL
        .into_iter()
        .map(|setting| {
            let (kind, options) = match setting {
                Setting::Show => (DebugControlKind::Toggle { on: show }, Vec::new()),
                Setting::Size => choice(&SIZE_PRESETS, size, "px", show),
                Setting::Opacity => choice(&OPACITY_PRESETS, opacity, "%", show),
            };
            ShadowRow {
                setting,
                field: setting.field(),
                kind,
                options,
            }
        })
        .collect()
}

/// `presets` as a `Choice` with `unit`, including `current` in order when
/// it lies between presets.
fn choice(
    presets: &[u8],
    current: u8,
    unit: &str,
    enabled: bool,
) -> (DebugControlKind, Vec<(String, FieldValue)>) {
    let mut values = presets.to_vec();
    if !values.contains(&current) {
        values.push(current);
        values.sort_unstable();
    }
    let selected = values
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    let options: Vec<(String, FieldValue)> = values
        .into_iter()
        .map(|value| (format!("{value} {unit}"), FieldValue::Slider(value.into())))
        .collect();
    let kind = DebugControlKind::Choice {
        options: options.iter().map(|(label, _)| label.clone()).collect(),
        selected,
        enabled,
    };
    (kind, options)
}

impl App {
    /// The Shadow settings box, showing the global theme's shadow look.
    pub(super) fn shadow_setting_controls(&self) -> Vec<DebugControl> {
        shadow_rows(&self.config.theme)
            .into_iter()
            .map(|row| DebugControl {
                id: row.setting.id(),
                label: row.setting.label().to_string(),
                kind: row.kind,
                group: Some(GROUP.to_string()),
            })
            .collect()
    }

    /// Handles a toggle or choice on control `id` if it is a shadow
    /// setting. The edit is the settings window's own, saved at once; an
    /// open settings window shows it, and its Cancel keeps it.
    pub(super) fn on_shadow_setting(&mut self, ui: &mut Ui, id: u32, input: DebugInput) {
        let Some(setting) = Setting::from_id(id) else {
            return;
        };
        let rows = shadow_rows(&self.config.theme);
        let Some(row) = rows.iter().find(|row| row.setting == setting) else {
            return;
        };
        let (label, value) = match (setting, input) {
            (Setting::Show, DebugInput::Toggle(on)) => (on.to_string(), FieldValue::Toggle(on)),
            (Setting::Size | Setting::Opacity, DebugInput::Choose(index))
                if self.config.theme.show_shadow =>
            {
                let Some(entry) = row.options.get(index) else {
                    return;
                };
                entry.clone()
            }
            _ => return,
        };
        let edit = |theme: &mut Theme| settings::apply_shadow_edit(theme, row.field, value.clone());
        if !edit(&mut self.config.theme) {
            return;
        }
        if let Some(session) = self.settings_ui.session_mut() {
            edit(&mut session.saved_theme);
        }
        self.restyle_all(ui);
        self.refresh_settings(ui);
        self.flush_save(ui);
        tracing::info!(
            setting = setting.label(),
            value = %label,
            "debug: shadow setting changed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(rows: &[ShadowRow], setting: Setting) -> &ShadowRow {
        rows.iter()
            .find(|row| row.setting == setting)
            .expect("every setting has a row")
    }

    #[test]
    fn turning_the_shadow_off_greys_its_size_and_opacity() {
        let mut theme = Theme::default();
        let on = shadow_rows(&theme);
        assert_eq!(
            row(&on, Setting::Show).kind,
            DebugControlKind::Toggle { on: true }
        );
        for setting in [Setting::Size, Setting::Opacity] {
            assert!(matches!(
                row(&on, setting).kind,
                DebugControlKind::Choice { enabled: true, .. }
            ));
        }

        theme.show_shadow = false;
        let off = shadow_rows(&theme);
        assert_eq!(
            row(&off, Setting::Show).kind,
            DebugControlKind::Toggle { on: false }
        );
        for setting in [Setting::Size, Setting::Opacity] {
            assert!(matches!(
                row(&off, setting).kind,
                DebugControlKind::Choice { enabled: false, .. }
            ));
        }
    }

    #[test]
    fn every_option_is_an_edit_the_settings_accept() {
        let theme = Theme::default();
        for row in shadow_rows(&theme) {
            for (label, value) in &row.options {
                let mut edited = theme.clone();
                assert!(
                    settings::apply_shadow_edit(&mut edited, row.field, value.clone()),
                    "{:?} {label}",
                    row.setting
                );
            }
        }
        let mut edited = theme;
        assert!(settings::apply_shadow_edit(
            &mut edited,
            Setting::Show.field(),
            FieldValue::Toggle(false)
        ));
        assert!(!edited.show_shadow);
    }

    #[test]
    fn a_row_edits_the_theme_and_selects_the_choice() {
        let mut theme = Theme::default();
        let rows = shadow_rows(&theme);
        let size = row(&rows, Setting::Size);
        let index = size
            .options
            .iter()
            .position(|(label, _)| label == "32 px")
            .expect("32 px is offered");
        settings::apply_shadow_edit(&mut theme, size.field, size.options[index].1.clone());
        assert_eq!(theme.shadow_size, 32);
        let again = shadow_rows(&theme);
        assert!(matches!(
            row(&again, Setting::Size).kind,
            DebugControlKind::Choice { selected, .. } if selected == index
        ));
    }

    #[test]
    fn a_value_between_presets_is_listed_as_it_is() {
        let (kind, options) = choice(&SIZE_PRESETS, 20, "px", true);
        let DebugControlKind::Choice { selected, .. } = kind else {
            panic!("expected a Choice");
        };
        assert_eq!(options[selected].0, "20 px");
        assert_eq!(options.len(), SIZE_PRESETS.len() + 1);
    }

    #[test]
    fn ids_round_trip_and_stay_clear_of_the_debug_ids() {
        for setting in Setting::ALL {
            assert_eq!(Setting::from_id(setting.id()), Some(setting));
        }
        let highest = super::super::ids::DebugId::ALL
            .iter()
            .map(|id| id.id())
            .max()
            .unwrap_or(0);
        assert!(highest < SHADOW_ID_BASE);
    }
}
