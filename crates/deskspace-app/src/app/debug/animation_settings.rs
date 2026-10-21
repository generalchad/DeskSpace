//! The debug window's animation settings (ROADMAP D14): one "Animation
//! settings" box in the Animations section showing the kind chosen in the
//! Kind box, edited as the settings window edits it, and saved.
//!
//! The box keeps its rows whatever the kind, so the window keeps its shape
//! and never rebuilds; a row the kind or its effect does not use is greyed,
//! as on the settings window's pages (D12). Each row is a choice, since the
//! debug window has no number fields; a value between presets is listed as
//! it is.

use deskspace_core::animation::{
    Animations, DETAIL_RANGE, EffectChoice, EffectOption, Kind, STAGGER_MS_RANGE, StartupOrder,
};
use deskspace_platform::{DebugControl, DebugControlKind, FieldValue, Ui};

use crate::app::App;
use crate::settings::{self, AnimationPage, EDGES, STARTUP_ORDERS, field};

/// The box's title.
const GROUP: &str = "Animation settings";
/// Where these controls' ids start, far above every `DebugId`.
const ANIMATION_ID_BASE: u32 = 10_000;

/// The durations offered, in milliseconds (`DURATION_MS_RANGE`).
const DURATION_PRESETS: [u16; 9] = [100, 150, 200, 250, 300, 400, 500, 750, 1000];
/// The startup delays offered, in milliseconds (`STAGGER_MS_RANGE`).
const DELAY_PRESETS: [u16; 8] = [10, 25, 50, 75, 100, 200, 300, 500];

/// A row of the box. Its id is its place in `ALL`, so the window keeps
/// the same controls whichever kind is shown; never reorder them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setting {
    Effect,
    Duration,
    Detail,
    Direction,
    Order,
    Delay,
    Smoke,
}

impl Setting {
    const ALL: [Self; 7] = [
        Self::Effect,
        Self::Duration,
        Self::Detail,
        Self::Direction,
        Self::Order,
        Self::Delay,
        Self::Smoke,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Effect => "Effect",
            Self::Duration => "Duration",
            Self::Detail => "Detail",
            Self::Direction => "Direction",
            Self::Order => "Order",
            Self::Delay => "Delay",
            Self::Smoke => "Smoke",
        }
    }

    fn id(self) -> u32 {
        let index = Self::ALL
            .iter()
            .position(|setting| *setting == self)
            .expect("every setting is in ALL");
        ANIMATION_ID_BASE + index as u32
    }

    fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|setting| setting.id() == id)
    }

    /// The row an effect option is edited in. A new option fails to compile
    /// here until it has a row.
    fn of_option(option: EffectOption) -> Self {
        match option {
            EffectOption::Detail => Self::Detail,
            EffectOption::Direction => Self::Direction,
            EffectOption::Smoke => Self::Smoke,
        }
    }
}

/// One row as shown for a kind.
#[derive(Debug, Clone, PartialEq)]
struct AnimationRow {
    setting: Setting,
    /// The settings field it edits (`settings::field`).
    field: u32,
    /// Each option's text, and the edit choosing it makes.
    options: Vec<(String, FieldValue)>,
    selected: usize,
    enabled: bool,
}

/// `kind`'s settings as the box's rows, in `Setting::ALL` order.
fn animation_rows(animations: &Animations, kind: Kind) -> Vec<AnimationRow> {
    let page = AnimationPage::of(kind);
    let spec = kind.spec();
    let settings = animations.get(kind);
    // An option's row is live when the kind offers the option and its own
    // effect uses it; a kind following another ("Same as startup") takes
    // that one's.
    let live = |setting: Setting| {
        spec.options.iter().any(|option| {
            Setting::of_option(*option) == setting
                && matches!(settings.effect, EffectChoice::Effect(effect) if effect.uses(*option))
        })
    };
    Setting::ALL
        .into_iter()
        .map(|setting| {
            let (field, (options, selected), enabled) = match setting {
                Setting::Effect => {
                    let choices = page.choices();
                    let options = choices
                        .iter()
                        .enumerate()
                        .map(|(index, choice)| {
                            (
                                AnimationPage::choice_label(*choice),
                                FieldValue::Choice(index),
                            )
                        })
                        .collect();
                    let selected = choices
                        .iter()
                        .position(|choice| *choice == settings.effect)
                        .unwrap_or(0);
                    (page.effect, (options, selected), true)
                }
                Setting::Duration => (
                    page.duration,
                    milliseconds(&DURATION_PRESETS, settings.duration_ms),
                    true,
                ),
                Setting::Detail => (page.detail, details(settings.detail), live(setting)),
                Setting::Direction => (
                    page.direction,
                    labelled(&EDGES, |edge| *edge == settings.direction),
                    live(setting),
                ),
                Setting::Order => (
                    field::ANIM_STARTUP_ORDER,
                    labelled(&STARTUP_ORDERS, |order| *order == animations.startup),
                    spec.startup,
                ),
                Setting::Delay => (
                    field::ANIM_STAGGER,
                    milliseconds(
                        &DELAY_PRESETS,
                        animations
                            .stagger_ms
                            .clamp(STAGGER_MS_RANGE.0, STAGGER_MS_RANGE.1),
                    ),
                    spec.startup && animations.startup == StartupOrder::OneAfterAnother,
                ),
                Setting::Smoke => (page.smoke, toggled(settings.smoke), live(setting)),
            };
            AnimationRow {
                setting,
                field,
                options,
                selected,
                enabled,
            }
        })
        .collect()
}

/// A table's labels as options, each choosing its index, and the first
/// entry `current` accepts.
fn labelled<T>(
    table: &[(T, &str)],
    current: impl Fn(&T) -> bool,
) -> (Vec<(String, FieldValue)>, usize) {
    let options = table
        .iter()
        .enumerate()
        .map(|(index, (_, label))| (label.to_string(), FieldValue::Choice(index)))
        .collect();
    let selected = table
        .iter()
        .position(|(value, _)| current(value))
        .unwrap_or(0);
    (options, selected)
}

/// Off and On, with the one `on` picks.
fn toggled(on: bool) -> (Vec<(String, FieldValue)>, usize) {
    (
        vec![
            ("Off".to_string(), FieldValue::Toggle(false)),
            ("On".to_string(), FieldValue::Toggle(true)),
        ],
        if on { 1 } else { 0 },
    )
}

fn details(current: u8) -> (Vec<(String, FieldValue)>, usize) {
    let range = DETAIL_RANGE.0..=DETAIL_RANGE.1;
    let options = range
        .clone()
        .map(|detail| (detail.to_string(), FieldValue::Slider(detail.into())))
        .collect();
    let selected = range.into_iter().position(|detail| detail == current);
    (options, selected.unwrap_or(0))
}

/// `presets` as options in milliseconds, with `current` among them in
/// order when it is not one, and where `current` is.
fn milliseconds(presets: &[u16], current: u16) -> (Vec<(String, FieldValue)>, usize) {
    let mut values = presets.to_vec();
    if !values.contains(&current) {
        values.push(current);
        values.sort_unstable();
    }
    let selected = values
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    let options = values
        .into_iter()
        .map(|value| (format!("{value} ms"), FieldValue::Slider(value.into())))
        .collect();
    (options, selected)
}

impl App {
    /// The Animation settings box, showing the kind chosen in Kind.
    pub(super) fn animation_setting_controls(&self) -> Vec<DebugControl> {
        animation_rows(&self.config.window.animations, self.chosen_kind())
            .into_iter()
            .map(|row| DebugControl {
                id: row.setting.id(),
                label: row.setting.label().to_string(),
                kind: DebugControlKind::Choice {
                    options: row.options.into_iter().map(|(label, _)| label).collect(),
                    selected: row.selected,
                    enabled: row.enabled,
                },
                group: Some(GROUP.to_string()),
            })
            .collect()
    }

    /// Handles a choice on control `id` if it is an animation setting, for
    /// the kind chosen in Kind. The edit is the settings window's own, saved
    /// at once; an open settings window shows it, and its Cancel keeps it.
    pub(super) fn on_animation_setting(&mut self, ui: &mut Ui, id: u32, index: usize) {
        let Some(setting) = Setting::from_id(id) else {
            return;
        };
        let kind = self.chosen_kind();
        let rows = animation_rows(&self.config.window.animations, kind);
        let Some(row) = rows.iter().find(|row| row.setting == setting) else {
            return;
        };
        let Some((label, value)) = row.options.get(index).filter(|_| row.enabled) else {
            return;
        };
        let edit = |animations: &mut Animations| {
            settings::apply_animation_edit(animations, row.field, value.clone())
        };
        if !edit(&mut self.config.window.animations) {
            return;
        }
        if let Some(session) = self.settings_ui.session_mut() {
            edit(&mut session.saved_window.animations);
        }
        self.refresh_settings(ui);
        self.flush_save(ui);
        tracing::info!(
            kind = AnimationPage::of(kind).title,
            setting = setting.label(),
            value = %label,
            "debug: animation setting changed"
        );
    }
}

#[cfg(test)]
mod tests {
    use deskspace_core::animation::{Effect, KINDS};

    use super::*;

    fn row(rows: &[AnimationRow], setting: Setting) -> &AnimationRow {
        rows.iter()
            .find(|row| row.setting == setting)
            .expect("every setting has a row")
    }

    fn enabled(animations: &Animations, kind: Kind) -> Vec<&'static str> {
        animation_rows(animations, kind)
            .iter()
            .filter(|row| row.enabled)
            .map(|row| row.setting.label())
            .collect()
    }

    #[test]
    fn every_kind_shows_the_same_rows() {
        let animations = Animations::default();
        for spec in KINDS {
            let settings: Vec<Setting> = animation_rows(&animations, spec.kind)
                .iter()
                .map(|row| row.setting)
                .collect();
            assert_eq!(settings, Setting::ALL, "{:?}", spec.kind);
        }
    }

    #[test]
    fn rows_a_kind_or_its_effect_does_not_use_are_greyed() {
        let mut animations = Animations::default();
        // Startup's default, Roll, uses only Direction.
        assert_eq!(
            enabled(&animations, Kind::Startup),
            ["Effect", "Duration", "Direction", "Order", "Delay"]
        );
        // "Same as startup" greys Delete's own options, as its page does.
        assert_eq!(enabled(&animations, Kind::Delete), ["Effect", "Duration"]);
        assert_eq!(enabled(&animations, Kind::Fold), ["Effect", "Duration"]);
        animations.startup_exit.effect = EffectChoice::Effect(Effect::Burn);
        animations.startup = StartupOrder::AllAtOnce;
        assert_eq!(
            enabled(&animations, Kind::Startup),
            ["Effect", "Duration", "Detail", "Order", "Smoke"]
        );
    }

    #[test]
    fn every_option_is_an_edit_the_settings_accept() {
        let animations = Animations::default();
        for spec in KINDS {
            for row in animation_rows(&animations, spec.kind) {
                assert!(row.selected < row.options.len(), "{:?}", row.setting);
                for (label, value) in &row.options {
                    let mut edited = animations;
                    assert!(
                        settings::apply_animation_edit(&mut edited, row.field, value.clone()),
                        "{:?} {:?} {label}",
                        spec.kind,
                        row.setting
                    );
                }
            }
        }
    }

    #[test]
    fn a_row_edits_the_shown_kind_and_selects_the_choice() {
        let mut animations = Animations::default();
        let rows = animation_rows(&animations, Kind::Fold);
        let effect = row(&rows, Setting::Effect);
        let none = effect
            .options
            .iter()
            .position(|(label, _)| label == "None")
            .expect("None is offered");
        settings::apply_animation_edit(
            &mut animations,
            effect.field,
            effect.options[none].1.clone(),
        );
        assert_eq!(
            animations.get(Kind::Fold).effect,
            EffectChoice::Effect(Effect::None)
        );
        assert_ne!(
            animations.get(Kind::Startup).effect,
            EffectChoice::Effect(Effect::None),
            "the other kinds are untouched"
        );
        let again = animation_rows(&animations, Kind::Fold);
        assert_eq!(row(&again, Setting::Effect).selected, none);
    }

    #[test]
    fn a_value_between_presets_is_listed_as_it_is() {
        let (options, selected) = milliseconds(&DURATION_PRESETS, 333);
        assert_eq!(options[selected].0, "333 ms");
        assert_eq!(options.len(), DURATION_PRESETS.len() + 1);
        let (options, selected) = milliseconds(&DURATION_PRESETS, 250);
        assert_eq!((options.len(), options[selected].0.as_str()), (9, "250 ms"));
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
        assert!(highest < ANIMATION_ID_BASE);
    }
}
