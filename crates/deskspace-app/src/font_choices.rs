//! A font's three choices, family, size, and weight: the options each shows
//! and the font a picked option makes. The Debug window's typography box and
//! the settings window's font pages both read them, so the two always offer
//! the same lists.

use deskspace_core::typography::{FONT_WEIGHTS, Font};

/// The family option for `""`, Windows' message font.
pub(crate) const DEFAULT_FAMILY: &str = "Windows default";

/// The sizes offered, in 96-DPI pixels, within `FONT_SIZE_RANGE`.
const SIZE_PRESETS: &[f32] = &[
    8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 40.0, 48.0,
];

/// One of a font's choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Family,
    Size,
    Weight,
}

impl Part {
    pub(crate) const ALL: [Self; 3] = [Self::Family, Self::Size, Self::Weight];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Family => "Family",
            Self::Size => "Size",
            Self::Weight => "Weight",
        }
    }

    /// The options this choice shows for `font`, and the one selected.
    /// `installed` is every installed family, sorted.
    pub(crate) fn options(self, font: &Font, installed: &[String]) -> (Vec<String>, usize) {
        match self {
            Self::Family => {
                let families = families(&font.family, installed);
                let selected = families
                    .iter()
                    .position(|family| *family == font.family)
                    .unwrap_or(0);
                let labels = families
                    .into_iter()
                    .map(|family| {
                        if family.is_empty() {
                            DEFAULT_FAMILY.to_owned()
                        } else {
                            family
                        }
                    })
                    .collect();
                (labels, selected)
            }
            Self::Size => {
                let sizes = sizes(font.size);
                let selected = sizes
                    .iter()
                    .position(|size| same_size(*size, font.size))
                    .unwrap_or(0);
                (
                    sizes.iter().map(|size| format!("{size} px")).collect(),
                    selected,
                )
            }
            Self::Weight => {
                let labels = FONT_WEIGHTS
                    .iter()
                    .map(|(_, name)| (*name).to_owned())
                    .collect();
                let selected = FONT_WEIGHTS
                    .iter()
                    .position(|(value, _)| *value == font.weight)
                    .unwrap_or(0);
                (labels, selected)
            }
        }
    }

    /// `font` with option `index` of this choice picked, read against the
    /// same lists `options` showed. An index past the list keeps `font`.
    pub(crate) fn pick(self, font: &Font, index: usize, installed: &[String]) -> Font {
        let mut picked = font.clone();
        match self {
            Self::Family => {
                if let Some(family) = families(&font.family, installed).into_iter().nth(index) {
                    picked.family = family;
                }
            }
            Self::Size => {
                if let Some(size) = sizes(font.size).get(index) {
                    picked.size = *size;
                }
            }
            Self::Weight => {
                if let Some((weight, _)) = FONT_WEIGHTS.get(index) {
                    picked.weight = *weight;
                }
            }
        }
        picked
    }
}

/// Windows' default (`""`), then every installed family, with `current` in
/// its sorted place when it is not installed, so a saved family that was
/// uninstalled still shows as itself.
fn families(current: &str, installed: &[String]) -> Vec<String> {
    let mut families: Vec<String> = installed.to_vec();
    if !current.is_empty() && !families.iter().any(|family| family == current) {
        let at = families
            .iter()
            .position(|family| family.to_lowercase() > current.to_lowercase())
            .unwrap_or(families.len());
        families.insert(at, current.to_owned());
    }
    families.insert(0, String::new());
    families
}

/// The size presets, with `current` in its place when it is between them.
fn sizes(current: f32) -> Vec<f32> {
    let mut sizes = SIZE_PRESETS.to_vec();
    if !sizes.iter().any(|size| same_size(*size, current)) {
        sizes.push(current);
        sizes.sort_by(f32::total_cmp);
    }
    sizes
}

fn same_size(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[cfg(test)]
mod tests {
    use deskspace_core::typography::FONT_SIZE_RANGE;

    use super::*;

    fn installed() -> Vec<String> {
        ["Arial", "Consolas", "Segoe UI"].map(String::from).to_vec()
    }

    #[test]
    fn every_size_preset_is_in_range() {
        assert!(
            SIZE_PRESETS
                .iter()
                .all(|size| (FONT_SIZE_RANGE.0..=FONT_SIZE_RANGE.1).contains(size))
        );
    }

    #[test]
    fn the_default_family_comes_first_and_is_selected() {
        let font = Font::new("", 12.0, 400);
        let (options, selected) = Part::Family.options(&font, &installed());
        assert_eq!(
            options,
            ["Windows default", "Arial", "Consolas", "Segoe UI"]
        );
        assert_eq!(selected, 0);
        assert_eq!(Part::Family.pick(&font, 2, &installed()).family, "Consolas");
        assert_eq!(Part::Family.pick(&font, 0, &installed()).family, "");
    }

    #[test]
    fn a_family_that_is_not_installed_shows_as_itself() {
        let font = Font::new("Bahnschrift", 12.0, 400);
        let (options, selected) = Part::Family.options(&font, &installed());
        assert_eq!(options[selected], "Bahnschrift");
        assert_eq!(options[1..=2], ["Arial", "Bahnschrift"]);
        let again = Part::Family.pick(&font, selected, &installed());
        assert_eq!(again, font, "picking it again changes nothing");
    }

    #[test]
    fn a_size_between_presets_is_offered_in_its_place() {
        let font = Font::new("", 12.5, 400);
        let (options, selected) = Part::Size.options(&font, &[]);
        assert_eq!(options[selected], "12.5 px");
        assert_eq!(options[selected - 1], "12 px");
        assert_eq!(Part::Size.pick(&font, selected + 1, &[]).size, 13.0);
    }

    #[test]
    fn a_weight_picks_its_preset() {
        let font = Font::new("", 12.0, 600);
        let (options, selected) = Part::Weight.options(&font, &[]);
        assert_eq!(options[selected], "Semibold");
        assert_eq!(Part::Weight.pick(&font, 2, &[]).weight, 700);
    }

    #[test]
    fn an_index_past_the_list_keeps_the_font() {
        let font = Font::new("Arial", 12.0, 400);
        for part in Part::ALL {
            assert_eq!(part.pick(&font, 999, &installed()), font, "{part:?}");
        }
    }
}
