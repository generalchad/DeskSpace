//! The picker's state and what each input does to it. No Win32 calls: the
//! window procedure turns messages into these calls and carries out the
//! returned `Outcome`, so the behavior is unit-tested.

use deskspace_core::geometry::Point;
use deskspace_core::theme::{Color, Hsv};

use super::format::{Parsed, format_value, parse_value};
use crate::edit::{EditKey, EditOutcome, MoveDir, TextEdit};
use crate::render::color_picker::{
    FieldStatus, Grid, PALETTE, PickerLayout, PickerView, Region, ValueFormat, fraction_x,
    saturation_value,
};

/// What the window must do after an input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    Nothing,
    Redraw,
    /// Close the picker; `result` says whether a color was chosen.
    Done,
    /// Start the eyedropper: capture the mouse and sample the screen.
    Sample,
    /// Put this text on the clipboard.
    Copy(String),
}

/// A key the picker handles, decoded from `WM_KEYDOWN`; typed characters
/// arrive as `Edit(EditKey::Char)` from `WM_CHAR`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PickerKey {
    Edit(EditKey),
    Tab,
    Enter,
    Escape,
    Copy,
    Paste,
}

/// Virtual-key codes the picker reads.
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
pub(super) mod vk {
    pub const BACK: u16 = 0x08;
    pub const TAB: u16 = 0x09;
    pub const RETURN: u16 = 0x0D;
    pub const ESCAPE: u16 = 0x1B;
    pub const END: u16 = 0x23;
    pub const HOME: u16 = 0x24;
    pub const LEFT: u16 = 0x25;
    pub const RIGHT: u16 = 0x27;
    pub const INSERT: u16 = 0x2D;
    pub const DELETE: u16 = 0x2E;
    pub const A: u16 = 0x41;
    pub const C: u16 = 0x43;
    pub const V: u16 = 0x56;
}

/// Decodes a key press with the Ctrl and Shift state.
pub(super) fn picker_key(key: u16, ctrl: bool, shift: bool) -> Option<PickerKey> {
    let moving = |dir: MoveDir| Some(PickerKey::Edit(EditKey::Move { dir, select: shift }));
    match key {
        vk::ESCAPE => Some(PickerKey::Escape),
        vk::RETURN => Some(PickerKey::Enter),
        vk::TAB => Some(PickerKey::Tab),
        vk::BACK if ctrl => Some(PickerKey::Edit(EditKey::BackspaceWord)),
        vk::BACK => Some(PickerKey::Edit(EditKey::Backspace)),
        vk::DELETE if ctrl => Some(PickerKey::Edit(EditKey::DeleteWord)),
        vk::DELETE => Some(PickerKey::Edit(EditKey::Delete)),
        vk::LEFT if ctrl => moving(MoveDir::WordLeft),
        vk::LEFT => moving(MoveDir::Left),
        vk::RIGHT if ctrl => moving(MoveDir::WordRight),
        vk::RIGHT => moving(MoveDir::Right),
        vk::HOME => moving(MoveDir::Home),
        vk::END => moving(MoveDir::End),
        vk::A if ctrl => Some(PickerKey::Edit(EditKey::SelectAll)),
        vk::C if ctrl => Some(PickerKey::Copy),
        vk::INSERT if ctrl => Some(PickerKey::Copy),
        vk::V if ctrl => Some(PickerKey::Paste),
        vk::INSERT if shift => Some(PickerKey::Paste),
        _ => None,
    }
}

pub(super) struct PickerState {
    pub(super) layout: PickerLayout,
    /// The window's DPI factor (1.0 at 96 DPI).
    pub(super) scale: f32,
    pub(super) dark: bool,
    pub(super) acrylic: bool,
    accent: Color,
    initial: Color,
    recent: Vec<Color>,
    // The color in HSV, so dragging saturation or value keeps the hue.
    hsv: Hsv,
    alpha: u8,
    format: ValueFormat,
    field: TextEdit,
    field_focused: bool,
    /// The box holds typed text not yet rewritten in its format's own form.
    field_dirty: bool,
    /// The opacity when the box was last written, for typed text that names
    /// none.
    alpha_before_typing: u8,
    pub(super) caret_visible: bool,
    status: FieldStatus,
    hover: Option<Region>,
    /// The button the press went down on; it acts when released over it.
    pressed: Option<Region>,
    /// The square or slider being dragged.
    drag: Option<Region>,
    /// While the eyedropper samples, the color to go back to if it is
    /// cancelled.
    sampling: Option<(Hsv, u8)>,
    pub(super) result: Option<Color>,
}

impl PickerState {
    pub(super) fn new(initial: Color, recent: &[Color], dark: bool, accent: Color) -> Self {
        let hsv = initial.to_hsv();
        let format = ValueFormat::Hex;
        Self {
            layout: PickerLayout::new(),
            scale: 1.0,
            dark,
            acrylic: false,
            accent,
            initial,
            recent: recent.to_vec(),
            hsv,
            alpha: initial.a,
            format,
            field: TextEdit::new(format_value(hsv, initial.a, format)),
            field_focused: false,
            field_dirty: false,
            alpha_before_typing: initial.a,
            caret_visible: true,
            status: FieldStatus::None,
            hover: None,
            pressed: None,
            drag: None,
            sampling: None,
            result: None,
        }
    }

    pub(super) fn color(&self) -> Color {
        self.hsv.to_color(self.alpha)
    }

    pub(super) fn is_sampling(&self) -> bool {
        self.sampling.is_some()
    }

    pub(super) fn view(&self) -> PickerView {
        PickerView {
            hsv: self.hsv,
            alpha: self.alpha,
            initial: self.initial,
            recent: self.recent.clone(),
            format: self.format,
            field: self.field.clone(),
            field_focused: self.field_focused,
            caret_visible: self.caret_visible,
            status: self.status,
            hover: self.hover,
            sampling: self.is_sampling(),
            dark: self.dark,
            acrylic: self.acrylic,
            accent: self.accent,
        }
    }

    /// The part under a client point in physical pixels.
    pub(super) fn region_at(&self, x: f32, y: f32) -> Option<Region> {
        let (x, y) = (x / self.scale, y / self.scale);
        self.layout.region_at(
            Point::new(x.floor() as i32, y.floor() as i32),
            self.recent.len(),
        )
    }

    fn confirm(&mut self) -> Outcome {
        self.result = Some(self.color());
        Outcome::Done
    }

    /// Rewrites the box from the color in the current format.
    fn refresh_field(&mut self) {
        self.field = TextEdit::new(format_value(self.hsv, self.alpha, self.format));
        self.field_dirty = false;
        self.status = FieldStatus::None;
        self.alpha_before_typing = self.alpha;
    }

    /// Sets the color from outside the box: a swatch, the eyedropper, or
    /// Current.
    fn set_color(&mut self, color: Color) {
        self.store(color.to_hsv(), color.a);
        self.refresh_field();
    }

    /// Stores a color. A gray has no hue, so the hue the picker had is kept
    /// for when saturation comes back.
    fn store(&mut self, hsv: Hsv, alpha: u8) {
        let gray = hsv.s == 0.0 || hsv.v == 0.0;
        self.hsv = Hsv {
            h: if gray { self.hsv.h } else { hsv.h },
            ..hsv
        };
        self.alpha = alpha;
    }

    /// Takes a value typed in the box: the color follows it and the radio
    /// buttons switch to the notation it was recognized in. Text without an
    /// alpha keeps the opacity the box had before typing, so passing through
    /// `#00ff` on the way to `#00ff00` does not change it.
    fn apply_parsed(&mut self, parsed: Parsed) {
        self.store(parsed.hsv, parsed.alpha.unwrap_or(self.alpha_before_typing));
        self.format = parsed.format;
        self.field_dirty = true;
        self.status = FieldStatus::None;
    }

    /// Re-reads the box after an edit.
    fn field_edited(&mut self) {
        match parse_value(self.field.text(), self.format) {
            Ok(parsed) => self.apply_parsed(parsed),
            Err(error) => {
                self.field_dirty = true;
                self.status = FieldStatus::Invalid(error.message());
            }
        }
    }

    fn focus_field(&mut self) {
        if !self.field_focused {
            self.field_focused = true;
            self.caret_visible = true;
            let end = self.field.text().len();
            self.field.select_range(0, end);
        }
    }

    /// Leaves the box, rewriting it from the color: an invalid entry is
    /// dropped and a valid one is tidied.
    fn blur_field(&mut self) {
        if self.field_focused {
            self.field_focused = false;
            self.refresh_field();
        }
    }

    fn select_format(&mut self, format: ValueFormat) {
        self.format = format;
        self.refresh_field();
    }

    fn set_from_point(&mut self, region: Region, x: f32, y: f32) {
        let (x, y) = (x / self.scale, y / self.scale);
        let layout = self.layout;
        let mut hsv = self.hsv;
        let mut alpha = self.alpha;
        match region {
            Region::Saturation => (hsv.s, hsv.v) = saturation_value(layout.saturation, x, y),
            Region::Hue => hsv.h = fraction_x(layout.hue, x) * 360.0,
            Region::Alpha => alpha = (fraction_x(layout.alpha, x) * 255.0).round() as u8,
            _ => return,
        }
        // Set directly: `set_hsv` would keep the old hue at the gray edges.
        self.hsv = hsv;
        self.alpha = alpha;
        self.refresh_field();
    }

    /// A left press at a client point in physical pixels.
    pub(super) fn pressed(&mut self, x: f32, y: f32) -> Outcome {
        let region = self.region_at(x, y);
        if region == Some(Region::Field) {
            self.focus_field();
            return Outcome::Redraw;
        }
        self.blur_field();
        match region {
            Some(region) if region.is_value() => {
                self.drag = Some(region);
                self.set_from_point(region, x, y);
            }
            Some(region) if region.is_button() => self.pressed = Some(region),
            _ => {}
        }
        Outcome::Redraw
    }

    /// The pointer moved: drags a value control and tracks hover.
    pub(super) fn moved(&mut self, x: f32, y: f32) -> Outcome {
        let mut outcome = Outcome::Nothing;
        if let Some(region) = self.drag {
            self.set_from_point(region, x, y);
            outcome = Outcome::Redraw;
        }
        let hover = self.region_at(x, y).filter(|region| region.is_button());
        if hover != self.hover {
            self.hover = hover;
            outcome = Outcome::Redraw;
        }
        outcome
    }

    /// The pointer left the window.
    pub(super) fn left(&mut self) -> Outcome {
        if self.hover.take().is_some() {
            Outcome::Redraw
        } else {
            Outcome::Nothing
        }
    }

    /// A left release: ends a drag, or presses the button the press started
    /// on.
    pub(super) fn released(&mut self, x: f32, y: f32) -> Outcome {
        self.drag = None;
        let pressed = self.pressed.take();
        match self.region_at(x, y) {
            Some(region) if Some(region) == pressed => self.activate(region),
            _ => Outcome::Redraw,
        }
    }

    /// A double-click: on a swatch it picks that color and closes.
    pub(super) fn double_clicked(&mut self, x: f32, y: f32) -> Outcome {
        match self.region_at(x, y) {
            Some(Region::Swatch(grid, index)) => match self.swatch(grid, index) {
                Some(color) => {
                    self.set_color(color);
                    self.confirm()
                }
                None => Outcome::Nothing,
            },
            _ => self.pressed(x, y),
        }
    }

    fn swatch(&self, grid: Grid, index: usize) -> Option<Color> {
        match grid {
            Grid::Recent => self.recent.get(index).copied(),
            Grid::Palette => PALETTE.get(index).copied(),
        }
    }

    fn activate(&mut self, region: Region) -> Outcome {
        match region {
            Region::Close | Region::Cancel => Outcome::Done,
            Region::Apply => self.confirm(),
            Region::Eyedropper => Outcome::Sample,
            Region::Current => {
                self.set_color(self.initial);
                Outcome::Redraw
            }
            Region::Swatch(grid, index) => {
                if let Some(color) = self.swatch(grid, index) {
                    self.set_color(color);
                }
                Outcome::Redraw
            }
            Region::Format(format) => {
                self.select_format(format);
                Outcome::Redraw
            }
            Region::Copy => {
                let text = self.field.text().to_string();
                self.status = FieldStatus::Copied;
                Outcome::Copy(text)
            }
            Region::Title | Region::Saturation | Region::Hue | Region::Alpha | Region::Field => {
                Outcome::Nothing
            }
        }
    }

    pub(super) fn key(&mut self, key: PickerKey) -> Outcome {
        match key {
            PickerKey::Escape if self.field_focused && self.field_dirty => {
                self.refresh_field();
                self.focus_again();
                Outcome::Redraw
            }
            PickerKey::Escape => Outcome::Done,
            PickerKey::Enter if self.field_focused && self.field_dirty => {
                if !matches!(self.status, FieldStatus::Invalid(_)) {
                    self.refresh_field();
                    self.focus_again();
                }
                Outcome::Redraw
            }
            PickerKey::Enter => self.confirm(),
            PickerKey::Tab => {
                if self.field_focused {
                    self.blur_field();
                } else {
                    self.focus_field();
                }
                Outcome::Redraw
            }
            PickerKey::Copy => Outcome::Copy(self.copied_text()),
            // The window reads the clipboard and calls `paste`.
            PickerKey::Paste => Outcome::Nothing,
            PickerKey::Edit(EditKey::Char(c)) => {
                self.type_text(c.encode_utf8(&mut [0; 4]));
                Outcome::Redraw
            }
            PickerKey::Edit(edit) if self.field_focused => {
                self.caret_visible = true;
                let before = self.field.text().to_string();
                // A caret move or selection changes nothing to re-read.
                if self.field.apply(edit) == EditOutcome::Changed && self.field.text() != before {
                    self.field_edited();
                }
                Outcome::Redraw
            }
            PickerKey::Edit(_) => Outcome::Nothing,
        }
    }

    /// `refresh_field` selects the whole box, as `TextEdit::new` does; the
    /// box keeps the focus.
    fn focus_again(&mut self) {
        self.field_focused = true;
        self.caret_visible = true;
    }

    /// The selection when the focused box has one, else the whole value.
    fn copied_text(&self) -> String {
        let (start, end) = self.field.selection();
        if self.field_focused && start != end {
            self.field.text()[start..end].to_string()
        } else {
            self.field.text().to_string()
        }
    }

    /// Types `text` into the box, focusing it first (selecting all, so typing
    /// while it is unfocused replaces the value).
    fn type_text(&mut self, text: &str) {
        self.focus_field();
        self.caret_visible = true;
        let mut typed = false;
        for c in text.chars().filter(|c| !c.is_control()) {
            self.field.apply(EditKey::Char(c));
            typed = true;
        }
        if typed {
            self.field_edited();
        }
    }

    /// Pastes clipboard text into the box, without the spaces and line break
    /// a copied value often brings along.
    pub(super) fn paste(&mut self, text: &str) {
        self.type_text(text.trim());
    }

    /// Blinks the caret; returns whether to redraw.
    pub(super) fn blink(&mut self) -> bool {
        if self.field_focused {
            self.caret_visible = !self.caret_visible;
        }
        self.field_focused
    }

    pub(super) fn start_sampling(&mut self) {
        self.blur_field();
        self.sampling = Some((self.hsv, self.alpha));
    }

    /// Shows the color under the eyedropper, keeping the opacity.
    pub(super) fn sample(&mut self, color: Color) {
        if self.is_sampling() {
            self.set_color(color.with_alpha(self.alpha));
        }
    }

    /// Ends the eyedropper, keeping the sampled color or going back.
    pub(super) fn end_sampling(&mut self, keep: bool) {
        if let Some((hsv, alpha)) = self.sampling.take()
            && !keep
        {
            self.hsv = hsv;
            self.alpha = alpha;
            self.refresh_field();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskspace_core::geometry::Rect;

    const BLUE: Color = Color::rgb(59, 130, 246);
    const ACCENT: Color = Color::rgb(0, 120, 212);

    fn state() -> PickerState {
        PickerState::new(BLUE, &[Color::rgb(1, 2, 3)], true, ACCENT)
    }

    fn center(rect: Rect) -> (f32, f32) {
        (
            rect.x as f32 + rect.width as f32 / 2.0,
            rect.y as f32 + rect.height as f32 / 2.0,
        )
    }

    fn click(state: &mut PickerState, part: impl FnOnce(&PickerLayout) -> Rect) -> Outcome {
        let (x, y) = center(part(&state.layout));
        state.pressed(x, y);
        state.released(x, y)
    }

    fn type_keys(state: &mut PickerState, text: &str) {
        for c in text.chars() {
            state.key(PickerKey::Edit(EditKey::Char(c)));
        }
    }

    #[test]
    fn opens_on_the_initial_color_in_hex() {
        let state = state();
        assert_eq!(state.color(), BLUE);
        assert_eq!(state.view().field.text(), "#3B82F6");
        assert_eq!(state.view().format, ValueFormat::Hex);
    }

    #[test]
    fn apply_returns_the_color_and_cancel_returns_none() {
        let mut picker = state();
        assert_eq!(click(&mut picker, |layout| layout.cancel), Outcome::Done);
        assert_eq!(picker.result, None);
        let mut picker = state();
        assert_eq!(click(&mut picker, |layout| layout.close), Outcome::Done);
        assert_eq!(picker.result, None);
        let mut picker = state();
        assert_eq!(click(&mut picker, |layout| layout.apply), Outcome::Done);
        assert_eq!(picker.result, Some(BLUE));
    }

    #[test]
    fn a_button_acts_only_when_released_over_it() {
        let mut picker = state();
        let (x, y) = center(picker.layout.apply);
        picker.pressed(x, y);
        let (away_x, away_y) = center(picker.layout.field);
        assert_eq!(picker.released(away_x, away_y), Outcome::Redraw);
        assert_eq!(picker.result, None);
    }

    #[test]
    fn swatches_set_the_color_and_a_double_click_picks() {
        let mut picker = state();
        click(&mut picker, |layout| layout.swatch_rect(Grid::Palette, 0));
        assert_eq!(picker.color(), PALETTE[0]);
        click(&mut picker, |layout| layout.swatch_rect(Grid::Recent, 0));
        assert_eq!(picker.color(), Color::rgb(1, 2, 3));
        let (x, y) = center(picker.layout.swatch_rect(Grid::Palette, 3));
        assert_eq!(picker.double_clicked(x, y), Outcome::Done);
        assert_eq!(picker.result, Some(PALETTE[3]));
    }

    #[test]
    fn current_goes_back_to_the_initial_color() {
        let mut picker = state();
        click(&mut picker, |layout| layout.swatch_rect(Grid::Palette, 0));
        click(&mut picker, |layout| layout.compare_halves().1);
        assert_eq!(picker.color(), BLUE);
    }

    #[test]
    fn dragging_the_hue_slider_sets_the_hue() {
        let mut picker = state();
        let hue = picker.layout.hue;
        let y = hue.y as f32 + 1.0;
        picker.pressed(hue.x as f32, y);
        assert_eq!(picker.hsv.h, 0.0);
        picker.moved(hue.right() as f32, y);
        assert_eq!(picker.hsv.h, 360.0);
        picker.released(0.0, 0.0);
        assert_eq!(picker.moved(hue.x as f32, y), Outcome::Nothing);
    }

    #[test]
    fn input_is_scaled_to_the_layout() {
        let mut picker = state();
        picker.scale = 2.0;
        let (x, y) = center(picker.layout.apply);
        assert_eq!(picker.region_at(x * 2.0, y * 2.0), Some(Region::Apply));
    }

    #[test]
    fn the_square_keeps_the_hue_at_its_gray_edge() {
        let mut picker = state();
        let hue = picker.hsv.h;
        let square = picker.layout.saturation;
        picker.pressed(square.x as f32, square.y as f32);
        assert_eq!(picker.hsv.s, 0.0);
        assert_eq!(picker.hsv.h, hue);
        click(&mut picker, |layout| layout.swatch_rect(Grid::Recent, 0));
        picker.set_color(Color::rgb(128, 128, 128));
        assert!(picker.hsv.h > 0.0, "a gray keeps the previous hue");
    }

    #[test]
    fn typing_a_value_in_another_format_switches_the_radio() {
        let mut picker = state();
        click(&mut picker, |layout| layout.field);
        type_keys(&mut picker, "rgb(255, 0, 0)");
        assert_eq!(picker.view().format, ValueFormat::Rgb);
        assert_eq!(picker.color(), Color::rgb(255, 0, 0));
        // The text stays as typed until Enter tidies it.
        assert_eq!(picker.view().field.text(), "rgb(255, 0, 0)");
        assert_eq!(picker.key(PickerKey::Enter), Outcome::Redraw);
        assert_eq!(picker.view().field.text(), "255, 0, 0");
        // A second Enter applies.
        assert_eq!(picker.key(PickerKey::Enter), Outcome::Done);
        assert_eq!(picker.result, Some(Color::rgb(255, 0, 0)));
    }

    #[test]
    fn typing_while_unfocused_replaces_the_value() {
        let mut picker = state();
        type_keys(&mut picker, "#00ff00");
        assert!(picker.field_focused);
        assert_eq!(picker.color(), Color::rgb(0, 255, 0));
        assert_eq!(picker.view().format, ValueFormat::Hex);
    }

    #[test]
    fn typing_a_hex_code_keeps_the_opacity() {
        let mut picker = PickerState::new(BLUE.with_alpha(128), &[], true, ACCENT);
        type_keys(&mut picker, "#00ff00");
        assert_eq!(picker.color(), Color::rgba(0, 255, 0, 128));
    }

    #[test]
    fn invalid_text_shows_an_error_and_keeps_the_color() {
        let mut picker = state();
        type_keys(&mut picker, "300, 300, 300");
        assert_eq!(picker.color(), BLUE);
        assert!(matches!(picker.view().status, FieldStatus::Invalid(_)));
        // Enter does not apply invalid text, and Esc puts the value back.
        assert_eq!(picker.key(PickerKey::Enter), Outcome::Redraw);
        assert_eq!(picker.result, None);
        assert_eq!(picker.key(PickerKey::Escape), Outcome::Redraw);
        assert_eq!(picker.view().field.text(), "#3B82F6");
        assert_eq!(picker.view().status, FieldStatus::None);
        // With nothing typed, Esc cancels.
        assert_eq!(picker.key(PickerKey::Escape), Outcome::Done);
    }

    #[test]
    fn leaving_the_box_tidies_or_drops_what_was_typed() {
        let mut picker = state();
        type_keys(&mut picker, "nonsense");
        click(&mut picker, |layout| layout.saturation);
        assert!(!picker.field_focused);
        assert_eq!(picker.view().status, FieldStatus::None);
        assert!(picker.view().field.text().starts_with('#'));
    }

    #[test]
    fn choosing_a_format_rewrites_the_value() {
        let mut picker = state();
        click(&mut picker, |layout| layout.format_rect(ValueFormat::Hsb));
        assert_eq!(picker.view().field.text(), "217°, 76%, 96%");
        click(&mut picker, |layout| layout.format_rect(ValueFormat::Rgb));
        assert_eq!(picker.view().field.text(), "59, 130, 246");
    }

    #[test]
    fn caret_moves_do_not_rewrite_the_color() {
        let mut picker = state();
        click(&mut picker, |layout| layout.field);
        let moved = picker.key(PickerKey::Edit(EditKey::Move {
            dir: MoveDir::Home,
            select: false,
        }));
        assert_eq!(moved, Outcome::Redraw);
        assert_eq!(picker.color(), BLUE);
        assert!(!picker.field_dirty);
    }

    #[test]
    fn copy_puts_the_value_or_selection_on_the_clipboard() {
        let mut picker = state();
        assert_eq!(
            click(&mut picker, |layout| layout.copy),
            Outcome::Copy("#3B82F6".to_string())
        );
        assert_eq!(picker.view().status, FieldStatus::Copied);
        click(&mut picker, |layout| layout.field);
        picker.field.select_range(1, 3);
        assert_eq!(picker.key(PickerKey::Copy), Outcome::Copy("3B".to_string()));
    }

    #[test]
    fn pasting_reads_the_value() {
        let mut picker = state();
        picker.paste("  hsl(0, 100%, 50%)\r\n");
        assert_eq!(picker.color(), Color::rgb(255, 0, 0));
        assert_eq!(picker.view().format, ValueFormat::Hsl);
    }

    #[test]
    fn the_eyedropper_previews_and_can_be_cancelled() {
        let mut picker = state();
        assert_eq!(
            click(&mut picker, |layout| layout.eyedropper),
            Outcome::Sample
        );
        picker.start_sampling();
        picker.sample(Color::rgb(10, 20, 30));
        assert_eq!(picker.color(), Color::rgb(10, 20, 30));
        picker.end_sampling(false);
        assert_eq!(picker.color(), BLUE);
        picker.start_sampling();
        picker.sample(Color::rgb(10, 20, 30));
        picker.end_sampling(true);
        assert_eq!(picker.color(), Color::rgb(10, 20, 30));
        assert!(!picker.is_sampling());
    }

    #[test]
    fn hover_tracks_buttons_and_redraws_once() {
        let mut picker = state();
        let (x, y) = center(picker.layout.apply);
        assert_eq!(picker.moved(x, y), Outcome::Redraw);
        assert_eq!(picker.moved(x, y), Outcome::Nothing);
        assert_eq!(picker.left(), Outcome::Redraw);
    }

    #[test]
    fn keys_decode_with_modifiers() {
        assert_eq!(
            picker_key(vk::ESCAPE, false, false),
            Some(PickerKey::Escape)
        );
        assert_eq!(picker_key(vk::V, true, false), Some(PickerKey::Paste));
        assert_eq!(picker_key(vk::INSERT, false, true), Some(PickerKey::Paste));
        assert_eq!(picker_key(vk::C, true, false), Some(PickerKey::Copy));
        assert_eq!(
            picker_key(vk::LEFT, true, true),
            Some(PickerKey::Edit(EditKey::Move {
                dir: MoveDir::WordLeft,
                select: true
            }))
        );
        assert_eq!(picker_key(vk::V, false, false), None);
    }
}
