//! `App` zoom: Ctrl+wheel, the zoom pop-up's buttons, and the zoom keys
//! change the icon size (ROADMAP D9), and the pop-up shows the result
//! (ROADMAP F-57).

use deskspace_core::Backdrop;
use deskspace_core::theme::{ICON_SIZE_DEFAULT, ICON_SIZE_RANGE, Palette, Theme};
use deskspace_platform::{
    DESKTOP_ICON_RANGE, ITEM_ICON_SIZE, Ui, ZoomAction, ZoomBar, next_desktop_icon_size,
    set_desktop_icon_size,
};

use super::{App, ICON_SIZE_WHEEL_STEP, Surface, Timer, WHEEL_DELTA};
use crate::scene::space_style;

/// How long the zoom pop-up stays after the last zoom, unless the pointer is
/// over it.
const ZOOM_BAR_MS: u32 = 2_000;

/// The least tint opacity the pop-up is drawn with, so its text stays readable
/// over any wallpaper even when the spaces are nearly see-through.
const ZOOM_BAR_MIN_TINT_ALPHA: u8 = 160;

/// The icon size that zooming changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ZoomLevel {
    /// Under "Match Explorer's display": the desktop's own icon size, in
    /// pixels, which the matching spaces follow.
    Desktop(i32),
    /// Otherwise: DeskSpace's global icon size, in percent.
    Custom(u8),
}

impl ZoomLevel {
    /// The size as a percentage of the default. The desktop's 48 px ("Medium
    /// icons") is 100 %, as DeskSpace's own icon size is.
    fn percent(self) -> u32 {
        match self {
            Self::Desktop(pixels) => {
                ((pixels.max(0) * 100 + ITEM_ICON_SIZE / 2) / ITEM_ICON_SIZE) as u32
            }
            Self::Custom(percent) => percent.into(),
        }
    }

    fn can_zoom_out(self) -> bool {
        match self {
            Self::Desktop(pixels) => pixels > DESKTOP_ICON_RANGE.0,
            Self::Custom(percent) => percent > ICON_SIZE_RANGE.0,
        }
    }

    fn can_zoom_in(self) -> bool {
        match self {
            Self::Desktop(pixels) => pixels < DESKTOP_ICON_RANGE.1,
            Self::Custom(percent) => percent < ICON_SIZE_RANGE.1,
        }
    }

    fn is_default(self) -> bool {
        match self {
            Self::Desktop(pixels) => pixels == ITEM_ICON_SIZE,
            Self::Custom(percent) => percent == ICON_SIZE_DEFAULT,
        }
    }
}

/// The pop-up for `level`, in the global look. A clear backdrop would leave
/// nothing behind the text, so the pop-up uses acrylic instead.
fn zoom_bar(level: ZoomLevel, theme: &Theme, palette: &Palette, system_dark: bool) -> ZoomBar {
    let mut look = theme.clone();
    if look.backdrop == Backdrop::Clear {
        look.backdrop = Backdrop::Acrylic;
    }
    ZoomBar {
        percent: level.percent(),
        can_zoom_out: level.can_zoom_out(),
        can_zoom_in: level.can_zoom_in(),
        can_reset: !level.is_default(),
        style: space_style(&look, system_dark),
        tint: palette
            .tint
            .with_alpha(palette.tint.a.max(ZOOM_BAR_MIN_TINT_ALPHA)),
        text: palette.title_text,
        hover: palette.hover_fill,
        border: palette.border,
    }
}

impl App {
    pub(super) fn zoom_level(&self) -> ZoomLevel {
        match self.shell_display.icon_size {
            Some(pixels) if self.config.theme.match_explorer() => ZoomLevel::Desktop(pixels),
            _ => ZoomLevel::Custom(
                self.config
                    .theme
                    .icon_size
                    .clamp(ICON_SIZE_RANGE.0, ICON_SIZE_RANGE.1),
            ),
        }
    }

    /// A zoom pop-up button or zoom key: one notch either way, or back to
    /// 100 %.
    pub(super) fn zoom(&mut self, ui: &mut Ui, action: ZoomAction) {
        match action {
            ZoomAction::Out => self.adjust_icon_size(ui, -WHEEL_DELTA),
            ZoomAction::In => self.adjust_icon_size(ui, WHEEL_DELTA),
            ZoomAction::Reset => self.reset_icon_size(ui),
        }
    }

    /// Ctrl+wheel changes the icon size, as it does on the desktop, and shows
    /// the zoom pop-up. A space with its own `Custom` look keeps its size
    /// either way.
    ///
    /// Under "Match Explorer's display" (ROADMAP D9) it changes the desktop's
    /// own icon size, which the spaces that match Explorer then follow, so
    /// they stay 1:1 with the desktop. Otherwise, or when the desktop's size
    /// cannot be read, it changes DeskSpace's global icon size, persisted
    /// through the settings rather than the config's base grid.
    pub(super) fn adjust_icon_size(&mut self, ui: &mut Ui, delta: i32) {
        match self.zoom_level() {
            ZoomLevel::Desktop(current) => self.adjust_desktop_icon_size(ui, current, delta),
            ZoomLevel::Custom(current) => {
                let step = delta * ICON_SIZE_WHEEL_STEP / WHEEL_DELTA;
                let next = (i32::from(current) + step)
                    .clamp(ICON_SIZE_RANGE.0.into(), ICON_SIZE_RANGE.1.into());
                self.set_global_icon_size(ui, next as u8);
            }
        }
        self.show_zoom_bar(ui);
    }

    fn reset_icon_size(&mut self, ui: &mut Ui) {
        match self.zoom_level() {
            ZoomLevel::Desktop(_) => {
                if set_desktop_icon_size(ITEM_ICON_SIZE) {
                    self.refresh_shell_display(ui);
                }
            }
            ZoomLevel::Custom(_) => self.set_global_icon_size(ui, ICON_SIZE_DEFAULT),
        }
        self.show_zoom_bar(ui);
    }

    fn adjust_desktop_icon_size(&mut self, ui: &mut Ui, current: i32, delta: i32) {
        let notches = delta / WHEEL_DELTA;
        if notches == 0 {
            return;
        }
        if set_desktop_icon_size(next_desktop_icon_size(current, notches)) {
            self.refresh_shell_display(ui);
        }
    }

    fn set_global_icon_size(&mut self, ui: &mut Ui, size: u8) {
        if size == self.config.theme.icon_size {
            return;
        }
        self.config.theme.icon_size = size;
        self.relayout_all(ui);
        self.schedule_save(ui);
        self.refresh_settings(ui);
    }

    /// Shows the pop-up with the current size and (re)starts its hide timer.
    fn show_zoom_bar(&mut self, ui: &mut Ui) {
        let palette = self.surface_palette(Surface::Desktop);
        ui.show_zoom_bar(zoom_bar(
            self.zoom_level(),
            &self.config.theme,
            &palette,
            self.system_dark,
        ));
        Self::set_timer(ui, Timer::ZoomBar, ZOOM_BAR_MS);
    }

    /// Hides the pop-up once the pointer is off it. The timer repeats, so a
    /// hovered pop-up is checked again until the pointer leaves.
    pub(super) fn on_zoom_bar_timer(&mut self, ui: &mut Ui) {
        if ui.is_zoom_bar_hovered() {
            return;
        }
        ui.hide_zoom_bar();
        Self::clear_timer(ui, Timer::ZoomBar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_size_reads_as_a_percent_of_medium_icons() {
        assert_eq!(ZoomLevel::Desktop(48).percent(), 100);
        assert_eq!(ZoomLevel::Desktop(96).percent(), 200);
        assert_eq!(ZoomLevel::Desktop(16).percent(), 33);
        assert_eq!(ZoomLevel::Desktop(56).percent(), 117);
        assert_eq!(ZoomLevel::Custom(110).percent(), 110);
    }

    #[test]
    fn zooming_stops_at_both_ends_of_its_range() {
        let (small, large) = DESKTOP_ICON_RANGE;
        assert!(!ZoomLevel::Desktop(small).can_zoom_out());
        assert!(ZoomLevel::Desktop(small).can_zoom_in());
        assert!(!ZoomLevel::Desktop(large).can_zoom_in());
        let (small, large) = ICON_SIZE_RANGE;
        assert!(!ZoomLevel::Custom(small).can_zoom_out());
        assert!(!ZoomLevel::Custom(large).can_zoom_in());
        assert!(ZoomLevel::Custom(ICON_SIZE_DEFAULT).can_zoom_in());
    }

    #[test]
    fn reset_applies_only_away_from_the_default() {
        assert!(ZoomLevel::Desktop(ITEM_ICON_SIZE).is_default());
        assert!(!ZoomLevel::Desktop(64).is_default());
        assert!(ZoomLevel::Custom(ICON_SIZE_DEFAULT).is_default());
        let theme = Theme::default();
        let bar = zoom_bar(ZoomLevel::Custom(120), &theme, &theme.palette(false), false);
        assert!(bar.can_reset);
        assert_eq!(bar.percent, 120);
    }

    #[test]
    fn a_clear_theme_still_gets_a_material_and_a_readable_tint() {
        let theme = Theme {
            backdrop: Backdrop::Clear,
            ..Theme::default()
        };
        let mut palette = theme.palette(false);
        palette.tint = palette.tint.with_alpha(20);
        let bar = zoom_bar(ZoomLevel::Custom(100), &theme, &palette, false);
        assert_eq!(bar.style.backdrop, Backdrop::Acrylic);
        assert_eq!(bar.tint.a, ZOOM_BAR_MIN_TINT_ALPHA);
    }
}
