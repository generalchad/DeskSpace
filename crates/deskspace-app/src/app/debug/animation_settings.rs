//! The debug window's animation settings (ROADMAP D14): each kind's
//! settings in a box of its own in the Animations section, edited as the
//! settings window edits them, and saved.
//!
//! The rows come from `KINDS`: a kind shows the options it offers, so an
//! option or kind added there shows up here too. Each row is a choice, since
//! the debug window has no number fields; a value between presets is listed
//! as it is.

use deskspace_core::animation::{Animations, DETAIL_RANGE, EffectOption, STAGGER_MS_RANGE};
use deskspace_platform::{DebugControl, DebugControlKind, FieldValue, Ui};

use crate::app::App;
use crate::settings::{self, ANIMATION_PAGES, AnimationPage, EDGES, STARTUP_ORDERS, field};

/// Where these controls' ids start: each is this plus its settings field
/// id, far above every `DebugId`.
const ANIMATION_ID_BASE: u32 = 10_000;

/// The durations offered, in milliseconds (`DURATION_MS_RANGE`).
const DURATION_PRESETS: [u16; 9] = [100, 150, 200, 250, 300, 400, 500, 750, 1000];
/// The startup delays offered, in milliseconds (`STAGGER_MS_RANGE`).
const DELAY_PRESETS: [u16; 8] = [10, 25, 50, 75, 100, 200, 300, 500];

/// One setting as a choice.
#[derive(Debug, Clone, PartialEq)]
struct AnimationRow {
    /// The settings field it edits (`settings::field`).
    field: u32,
    /// The kind's name, which titles its box.
    group: &'static str,
    label: &'static str,
    /// Each option's text, and the edit choosing it makes.
    options: Vec<(String, FieldValue)>,
    selected: usize,
}

/// Every kind's settings as rows, kind by kind in `KINDS` order: the effect
/// and the duration, the options the kind offers, and for Startup the order
/// and the delay.
fn animation_rows(animations: &Animations) -> Vec<AnimationRow> {
    let mut rows = Vec::new();
    for page in &ANIMATION_PAGES {
        let spec = page.kind.spec();
        let settings = animations.get(page.kind);
        let row = |field, label, options, selected| AnimationRow {
            field,
            group: page.title,
            label,
            options,
            selected,
        };
        let choices = page.choices();
        rows.push(row(
            page.effect,
            "Effect",
            choices
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    (
                        AnimationPage::choice_label(*choice),
                        FieldValue::Choice(index),
                    )
                })
                .collect(),
            choices
                .iter()
                .position(|choice| *choice == settings.effect)
                .unwrap_or(0),
        ));
        let (options, selected) = milliseconds(&DURATION_PRESETS, settings.duration_ms);
        rows.push(row(page.duration, "Duration", options, selected));
        for option in spec.options {
            match option {
                EffectOption::Detail => {
                    let details = DETAIL_RANGE.0..=DETAIL_RANGE.1;
                    rows.push(row(
                        page.detail,
                        "Detail",
                        details
                            .clone()
                            .map(|detail| (detail.to_string(), FieldValue::Slider(detail.into())))
                            .collect(),
                        details
                            .into_iter()
                            .position(|detail| detail == settings.detail)
                            .unwrap_or(0),
                    ));
                }
                EffectOption::Direction => rows.push(row(
                    page.direction,
                    "Direction",
                    labelled(&EDGES),
                    EDGES
                        .iter()
                        .position(|(edge, _)| *edge == settings.direction)
                        .unwrap_or(0),
                )),
            }
        }
        if spec.startup {
            rows.push(row(
                field::ANIM_STARTUP_ORDER,
                "Order",
                labelled(&STARTUP_ORDERS),
                STARTUP_ORDERS
                    .iter()
                    .position(|(order, _)| *order == animations.startup)
                    .unwrap_or(0),
            ));
            let delay = animations
                .stagger_ms
                .clamp(STAGGER_MS_RANGE.0, STAGGER_MS_RANGE.1);
            let (options, selected) = milliseconds(&DELAY_PRESETS, delay);
            rows.push(row(field::ANIM_STAGGER, "Delay", options, selected));
        }
    }
    rows
}

/// A table's labels as options, each choosing its index.
fn labelled<T>(table: &[(T, &str)]) -> Vec<(String, FieldValue)> {
    table
        .iter()
        .enumerate()
        .map(|(index, (_, label))| (label.to_string(), FieldValue::Choice(index)))
        .collect()
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
    /// The animation settings' controls, each in its kind's box.
    pub(super) fn animation_setting_controls(&self) -> Vec<DebugControl> {
        animation_rows(&self.config.window.animations)
            .into_iter()
            .map(|row| DebugControl {
                id: ANIMATION_ID_BASE + row.field,
                label: row.label.to_string(),
                kind: DebugControlKind::Choice {
                    options: row.options.into_iter().map(|(label, _)| label).collect(),
                    selected: row.selected,
                },
                group: Some(row.group.to_string()),
            })
            .collect()
    }

    /// Handles a choice on control `id` if it is an animation setting, and
    /// says whether it was. The edit is the settings window's own, saved at
    /// once; an open settings window shows it, and its Cancel keeps it.
    pub(super) fn on_animation_setting(&mut self, ui: &mut Ui, id: u32, index: usize) -> bool {
        let rows = animation_rows(&self.config.window.animations);
        let Some(row) = rows.iter().find(|row| ANIMATION_ID_BASE + row.field == id) else {
            return false;
        };
        let Some((label, value)) = row.options.get(index) else {
            return true;
        };
        let edit = |animations: &mut Animations| {
            settings::apply_animation_edit(animations, row.field, value.clone())
        };
        if !edit(&mut self.config.window.animations) {
            return true;
        }
        if let Some(session) = self.settings_ui.session_mut() {
            edit(&mut session.saved_window.animations);
        }
        self.refresh_settings(ui);
        self.flush_save(ui);
        tracing::info!(
            kind = row.group,
            setting = row.label,
            value = %label,
            "debug: animation setting changed"
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use deskspace_core::animation::{EffectChoice, KINDS, Kind};

    use super::*;

    fn group<'a>(rows: &'a [AnimationRow], title: &str) -> Vec<&'a str> {
        rows.iter()
            .filter(|row| row.group == title)
            .map(|row| row.label)
            .collect()
    }

    #[test]
    fn each_kind_gets_a_box_of_the_settings_it_offers() {
        let rows = animation_rows(&Animations::default());
        assert_eq!(
            group(&rows, "Startup"),
            [
                "Effect",
                "Duration",
                "Detail",
                "Direction",
                "Order",
                "Delay"
            ]
        );
        assert_eq!(
            group(&rows, "Delete"),
            ["Effect", "Duration", "Detail", "Direction"]
        );
        assert_eq!(group(&rows, "Fold"), ["Effect", "Duration"]);
        let titles: Vec<&str> = ANIMATION_PAGES.iter().map(|page| page.title).collect();
        let mut seen: Vec<&str> = rows.iter().map(|row| row.group).collect();
        seen.dedup();
        assert_eq!(seen, titles, "one run per kind, in KINDS order");
        assert_eq!(titles.len(), KINDS.len());
    }

    #[test]
    fn every_option_is_an_edit_the_settings_accept() {
        let animations = Animations::default();
        for row in animation_rows(&animations) {
            assert!(row.selected < row.options.len(), "{}", row.label);
            for (label, value) in &row.options {
                let mut edited = animations;
                assert!(
                    settings::apply_animation_edit(&mut edited, row.field, value.clone()),
                    "{} {label}",
                    row.label
                );
            }
        }
    }

    #[test]
    fn choosing_an_option_selects_it() {
        let mut animations = Animations::default();
        let rows = animation_rows(&animations);
        let effect = rows
            .iter()
            .find(|row| row.group == "Fold" && row.label == "Effect")
            .expect("Fold has an effect");
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
            EffectChoice::Effect(deskspace_core::animation::Effect::None)
        );
        let again = animation_rows(&animations);
        let effect = again
            .iter()
            .find(|row| row.group == "Fold" && row.label == "Effect")
            .expect("Fold has an effect");
        assert_eq!(effect.selected, none);
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
    fn ids_stay_clear_of_the_debug_ids() {
        let highest = super::super::ids::DebugId::ALL
            .iter()
            .map(|id| id.id())
            .max()
            .unwrap_or(0);
        assert!(highest < ANIMATION_ID_BASE);
    }
}
