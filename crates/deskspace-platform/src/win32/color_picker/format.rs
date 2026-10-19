//! The value box's text: a color written as hex, RGB, HSL, or HSB, and typed
//! text read back. Reading recognizes the notation, so typing `#3B82F6` or
//! `hsl(217, 91%, 60%)` works whichever radio button is on, and the picker
//! switches to it.

use deskspace_core::theme::{Color, Hsv};

use crate::render::color_picker::ValueFormat;

/// A value read from the box: the notation it was in, the color, and the
/// alpha when the text gave one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Parsed {
    pub format: ValueFormat,
    pub hsv: Hsv,
    pub alpha: Option<u8>,
}

/// Why typed text is not a color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ValueError {
    Empty,
    /// No notation matches the text.
    Unrecognized,
    /// The text is in this notation, but a number is out of its range.
    OutOfRange(ValueFormat),
}

impl ValueError {
    /// What to tell the user under the box.
    pub(super) fn message(self) -> &'static str {
        match self {
            ValueError::Empty => "Type a color value",
            ValueError::Unrecognized => "Not a hex, RGB, HSL, or HSB value",
            ValueError::OutOfRange(ValueFormat::Hex) => "A hex code has 3, 4, 6, or 8 digits",
            ValueError::OutOfRange(ValueFormat::Rgb) => "RGB values run from 0 to 255",
            ValueError::OutOfRange(ValueFormat::Hsl | ValueFormat::Hsb) => {
                "Hue runs 0–360, the rest 0–100 %"
            }
        }
    }
}

/// Writes `hsv` with `alpha` in `format`. Alpha is written only when the color
/// is translucent: as a hex code's last two digits, otherwise as a fourth
/// value in percent.
pub(super) fn format_value(hsv: Hsv, alpha: u8, format: ValueFormat) -> String {
    let color = hsv.to_color(alpha);
    let translucent = alpha < 255;
    let alpha_suffix = if translucent {
        format!(", {}%", percent(alpha as f32 / 255.0))
    } else {
        String::new()
    };
    match format {
        ValueFormat::Hex if translucent => color.to_hex(),
        ValueFormat::Hex => format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b),
        ValueFormat::Rgb => format!("{}, {}, {}{alpha_suffix}", color.r, color.g, color.b),
        ValueFormat::Hsl => {
            let (s, l) = hsv_to_hsl(hsv.s, hsv.v);
            format!(
                "{}\u{B0}, {}%, {}%{alpha_suffix}",
                degrees(hsv.h),
                percent(s),
                percent(l)
            )
        }
        ValueFormat::Hsb => format!(
            "{}\u{B0}, {}%, {}%{alpha_suffix}",
            degrees(hsv.h),
            percent(hsv.s),
            percent(hsv.v)
        ),
    }
}

/// Reads `text` as a color. A `#` or a function name (`rgb(...)`, `hsl ...`)
/// names the notation. Bare numbers are read in `preferred` when they fit it,
/// else in the first of RGB, HSL, and HSB that they fit.
pub(super) fn parse_value(text: &str, preferred: ValueFormat) -> Result<Parsed, ValueError> {
    let text = text.trim().to_ascii_lowercase();
    if text.is_empty() {
        return Err(ValueError::Empty);
    }
    if let Some(digits) = text.strip_prefix('#') {
        return parse_hex(digits.trim());
    }
    let (named, body) = split_name(&text);
    if named.is_none() && is_bare_hex(&text) {
        return parse_hex(&text);
    }
    let parts = components(body).ok_or(ValueError::Unrecognized)?;
    let candidates = match named {
        Some(format) => vec![format],
        None => guesses(preferred),
    };
    let mut error = ValueError::Unrecognized;
    for format in candidates {
        match from_components(format, &parts) {
            Ok(parsed) => return Ok(parsed),
            // The first range error is the most useful: it is from the
            // notation the user most likely meant.
            Err(found @ ValueError::OutOfRange(_)) if error == ValueError::Unrecognized => {
                error = found;
            }
            Err(_) => {}
        }
    }
    Err(error)
}

/// The notations bare numbers are tried in: `preferred` first.
fn guesses(preferred: ValueFormat) -> Vec<ValueFormat> {
    let mut order = vec![preferred];
    for format in [ValueFormat::Rgb, ValueFormat::Hsl, ValueFormat::Hsb] {
        if !order.contains(&format) {
            order.push(format);
        }
    }
    order.retain(|format| *format != ValueFormat::Hex);
    order
}

/// Splits a leading function name off, longest names first so `rgba` is not
/// read as `rgb` and a stray `a`.
fn split_name(text: &str) -> (Option<ValueFormat>, &str) {
    const NAMES: [(&str, ValueFormat); 8] = [
        ("rgba", ValueFormat::Rgb),
        ("rgb", ValueFormat::Rgb),
        ("hsla", ValueFormat::Hsl),
        ("hsl", ValueFormat::Hsl),
        ("hsba", ValueFormat::Hsb),
        ("hsb", ValueFormat::Hsb),
        ("hsva", ValueFormat::Hsb),
        ("hsv", ValueFormat::Hsb),
    ];
    for (name, format) in NAMES {
        if let Some(rest) = text.strip_prefix(name) {
            return (Some(format), rest);
        }
    }
    (None, text)
}

/// Hex digits with no `#`: any 3, 4, 6, or 8 digits with a letter among them,
/// or 6 or 8 plain digits. Three plain digits stay numbers, so typing the
/// start of `123, 45, 6` does not flip the picker to hex.
fn is_bare_hex(text: &str) -> bool {
    let all_hex = text.chars().all(|c| c.is_ascii_hexdigit());
    let has_letter = text.chars().any(|c| c.is_ascii_alphabetic());
    all_hex
        && match text.len() {
            3 | 4 => has_letter,
            6 | 8 => true,
            _ => false,
        }
}

fn parse_hex(digits: &str) -> Result<Parsed, ValueError> {
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ValueError::Unrecognized);
    }
    let nibble = |index: usize| u8::from_str_radix(&digits[index..=index], 16).unwrap_or(0);
    let byte = |index: usize| u8::from_str_radix(&digits[index..index + 2], 16).unwrap_or(0);
    let (color, alpha) = match digits.len() {
        3 | 4 => {
            let short = |index: usize| nibble(index) * 17;
            let alpha = (digits.len() == 4).then(|| short(3));
            (Color::rgb(short(0), short(1), short(2)), alpha)
        }
        6 | 8 => {
            let alpha = (digits.len() == 8).then(|| byte(6));
            (Color::rgb(byte(0), byte(2), byte(4)), alpha)
        }
        _ => return Err(ValueError::OutOfRange(ValueFormat::Hex)),
    };
    Ok(Parsed {
        format: ValueFormat::Hex,
        hsv: color.to_hsv(),
        alpha,
    })
}

/// A number as typed, with its unit.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Component {
    Number(f32),
    Percent(f32),
    Degrees(f32),
}

/// The three or four numbers of `body`, which may be wrapped in parentheses
/// and separated by commas, spaces, or a slash before alpha.
fn components(body: &str) -> Option<Vec<Component>> {
    let body = body.trim();
    let body = body.strip_prefix('(').unwrap_or(body);
    let body = body.strip_suffix(')').unwrap_or(body);
    let parts: Vec<Component> = body
        .split([',', ' ', '/'])
        .filter(|part| !part.is_empty())
        .map(component)
        .collect::<Option<_>>()?;
    (3..=4).contains(&parts.len()).then_some(parts)
}

fn component(text: &str) -> Option<Component> {
    let (number, kind): (&str, fn(f32) -> Component) = if let Some(n) = text.strip_suffix('%') {
        (n, Component::Percent)
    } else if let Some(n) = text.strip_suffix('\u{B0}') {
        (n, Component::Degrees)
    } else if let Some(n) = text.strip_suffix("deg") {
        (n, Component::Degrees)
    } else {
        (text, Component::Number)
    };
    let value: f32 = number.parse().ok()?;
    value.is_finite().then(|| kind(value))
}

fn from_components(format: ValueFormat, parts: &[Component]) -> Result<Parsed, ValueError> {
    let out_of_range = ValueError::OutOfRange(format);
    let in_range = |value: f32, max: f32| {
        if (0.0..=max).contains(&value) {
            Ok(value)
        } else {
            Err(out_of_range)
        }
    };
    let alpha = match parts.get(3) {
        None => None,
        Some(Component::Number(fraction)) => Some(in_range(*fraction, 1.0)?),
        Some(Component::Percent(percent)) => Some(in_range(*percent, 100.0)? / 100.0),
        Some(Component::Degrees(_)) => return Err(ValueError::Unrecognized),
    }
    .map(|fraction| (fraction * 255.0).round() as u8);

    let hsv = match format {
        ValueFormat::Rgb => {
            let mut channels = [0u8; 3];
            for (channel, part) in channels.iter_mut().zip(parts) {
                let Component::Number(value) = part else {
                    return Err(ValueError::Unrecognized);
                };
                *channel = in_range(*value, 255.0)?.round() as u8;
            }
            Color::rgb(channels[0], channels[1], channels[2]).to_hsv()
        }
        ValueFormat::Hsl | ValueFormat::Hsb => {
            let hue = match parts[0] {
                Component::Number(value) | Component::Degrees(value) => in_range(value, 360.0)?,
                Component::Percent(_) => return Err(ValueError::Unrecognized),
            };
            let mut fractions = [0.0f32; 2];
            for (fraction, part) in fractions.iter_mut().zip(&parts[1..3]) {
                *fraction = match part {
                    Component::Number(value) | Component::Percent(value) => {
                        in_range(*value, 100.0)? / 100.0
                    }
                    Component::Degrees(_) => return Err(ValueError::Unrecognized),
                };
            }
            let [s, second] = fractions;
            let (s, v) = if format == ValueFormat::Hsl {
                hsl_to_hsv(s, second)
            } else {
                (s, second)
            };
            Hsv {
                h: hue % 360.0,
                s,
                v,
            }
        }
        ValueFormat::Hex => return Err(ValueError::Unrecognized),
    };
    Ok(Parsed { format, hsv, alpha })
}

/// HSV saturation and value to HSL saturation and lightness.
/// https://en.wikipedia.org/wiki/HSL_and_HSV#HSV_to_HSL
fn hsv_to_hsl(s: f32, v: f32) -> (f32, f32) {
    let l = v * (1.0 - s / 2.0);
    let s = if l <= 0.0 || l >= 1.0 {
        0.0
    } else {
        (v - l) / l.min(1.0 - l)
    };
    (s, l)
}

/// HSL saturation and lightness to HSV saturation and value.
/// https://en.wikipedia.org/wiki/HSL_and_HSV#HSL_to_HSV
fn hsl_to_hsv(s: f32, l: f32) -> (f32, f32) {
    let v = l + s * l.min(1.0 - l);
    let s = if v <= 0.0 { 0.0 } else { 2.0 * (1.0 - l / v) };
    (s, v)
}

fn percent(fraction: f32) -> u32 {
    (fraction * 100.0).round() as u32
}

fn degrees(hue: f32) -> u32 {
    hue.round() as u32 % 360
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLUE: Color = Color::rgb(59, 130, 246);

    fn color_of(parsed: Parsed) -> Color {
        parsed.hsv.to_color(parsed.alpha.unwrap_or(255))
    }

    fn read(text: &str, preferred: ValueFormat) -> (ValueFormat, Color) {
        let parsed = parse_value(text, preferred).unwrap();
        (parsed.format, color_of(parsed))
    }

    #[test]
    fn writes_each_format() {
        let hsv = BLUE.to_hsv();
        assert_eq!(format_value(hsv, 255, ValueFormat::Hex), "#3B82F6");
        assert_eq!(format_value(hsv, 255, ValueFormat::Rgb), "59, 130, 246");
        assert_eq!(format_value(hsv, 255, ValueFormat::Hsb), "217°, 76%, 96%");
        assert_eq!(format_value(hsv, 255, ValueFormat::Hsl), "217°, 91%, 60%");
        assert_eq!(format_value(hsv, 128, ValueFormat::Hex), "#3B82F680");
        assert_eq!(
            format_value(hsv, 128, ValueFormat::Rgb),
            "59, 130, 246, 50%"
        );
    }

    #[test]
    fn every_format_reads_back_what_it_writes() {
        for color in [
            BLUE,
            Color::rgb(255, 0, 0),
            Color::rgb(0, 0, 0),
            Color::rgb(255, 255, 255),
        ] {
            for format in ValueFormat::ALL {
                let text = format_value(color.to_hsv(), 255, format);
                let (read_format, read_color) = read(&text, format);
                assert_eq!(read_format, format, "{text}");
                // HSL and HSB are written in whole percents, so allow a step.
                for (a, b) in [
                    (read_color.r, color.r),
                    (read_color.g, color.g),
                    (read_color.b, color.b),
                ] {
                    assert!(a.abs_diff(b) <= 3, "{text}: {read_color:?} vs {color:?}");
                }
            }
        }
    }

    #[test]
    fn hex_is_recognized_from_any_format() {
        for text in ["#3b82f6", "#3B82F6", "3b82f6", " #3B82F6 "] {
            assert_eq!(
                read(text, ValueFormat::Hsl),
                (ValueFormat::Hex, BLUE),
                "{text}"
            );
        }
        assert_eq!(read("#fff", ValueFormat::Rgb).1, Color::rgb(255, 255, 255));
        assert_eq!(read("abc", ValueFormat::Rgb).0, ValueFormat::Hex);
        let parsed = parse_value("#3B82F680", ValueFormat::Rgb).unwrap();
        assert_eq!(parsed.alpha, Some(0x80));
    }

    #[test]
    fn function_names_pick_the_format() {
        assert_eq!(
            read("rgb(59, 130, 246)", ValueFormat::Hex),
            (ValueFormat::Rgb, BLUE)
        );
        assert_eq!(
            read("hsl(217, 91%, 60%)", ValueFormat::Hex).0,
            ValueFormat::Hsl
        );
        assert_eq!(read("hsv(217 76 96)", ValueFormat::Rgb).0, ValueFormat::Hsb);
        assert_eq!(
            read("HSB 217, 76%, 96%", ValueFormat::Rgb).0,
            ValueFormat::Hsb
        );
        let parsed = parse_value("rgba(59, 130, 246, 0.5)", ValueFormat::Hex).unwrap();
        assert_eq!(parsed.alpha, Some(128));
    }

    #[test]
    fn bare_numbers_keep_the_current_format_when_they_fit() {
        assert_eq!(read("40, 50, 60", ValueFormat::Rgb).0, ValueFormat::Rgb);
        assert_eq!(read("40, 50, 60", ValueFormat::Hsl).0, ValueFormat::Hsl);
        assert_eq!(read("40, 50, 60", ValueFormat::Hsb).0, ValueFormat::Hsb);
        assert_eq!(read("40, 50, 60", ValueFormat::Hex).0, ValueFormat::Rgb);
    }

    #[test]
    fn bare_numbers_switch_to_the_format_they_fit() {
        // Over 100: only RGB takes it.
        assert_eq!(
            read("59, 130, 246", ValueFormat::Hsl),
            (ValueFormat::Rgb, BLUE)
        );
        // A degree sign or percents rule RGB out.
        assert_eq!(read("217°, 91%, 60%", ValueFormat::Rgb).0, ValueFormat::Hsl);
        assert_eq!(read("217deg 76% 96%", ValueFormat::Hsb).0, ValueFormat::Hsb);
        // Over 255 but a valid hue: HSL.
        assert_eq!(read("300, 50, 50", ValueFormat::Rgb).0, ValueFormat::Hsl);
    }

    #[test]
    fn three_plain_digits_are_not_hex() {
        assert_eq!(
            parse_value("123", ValueFormat::Rgb),
            Err(ValueError::Unrecognized)
        );
    }

    #[test]
    fn invalid_text_says_why() {
        assert_eq!(parse_value("   ", ValueFormat::Rgb), Err(ValueError::Empty));
        assert_eq!(
            parse_value("blue", ValueFormat::Rgb),
            Err(ValueError::Unrecognized)
        );
        assert_eq!(
            parse_value("#12345", ValueFormat::Hex),
            Err(ValueError::OutOfRange(ValueFormat::Hex))
        );
        assert_eq!(
            parse_value("#12345g", ValueFormat::Hex),
            Err(ValueError::Unrecognized)
        );
        assert_eq!(
            parse_value("400, 400, 400", ValueFormat::Rgb),
            Err(ValueError::OutOfRange(ValueFormat::Rgb))
        );
        assert_eq!(
            parse_value("rgb(256, 0, 0)", ValueFormat::Hex),
            Err(ValueError::OutOfRange(ValueFormat::Rgb))
        );
        assert_eq!(
            parse_value("hsl(200, 120%, 50%)", ValueFormat::Rgb),
            Err(ValueError::OutOfRange(ValueFormat::Hsl))
        );
        assert_eq!(
            parse_value("1, 2", ValueFormat::Rgb),
            Err(ValueError::Unrecognized)
        );
        assert_eq!(
            parse_value("1, 2, 3, 4, 5", ValueFormat::Rgb),
            Err(ValueError::Unrecognized)
        );
    }

    #[test]
    fn messages_fit_the_status_line() {
        // About 38 characters fit under the box at 12 px.
        let errors = [ValueError::Empty, ValueError::Unrecognized]
            .into_iter()
            .chain(ValueFormat::ALL.map(ValueError::OutOfRange));
        for error in errors {
            let message = error.message();
            assert!(
                !message.is_empty() && message.chars().count() <= 36,
                "{message}"
            );
        }
    }

    #[test]
    fn hsl_and_hsv_convert_both_ways() {
        for (s, v) in [(0.0, 0.0), (1.0, 1.0), (0.76, 0.96), (0.3, 0.4), (0.0, 1.0)] {
            let (hs, l) = hsv_to_hsl(s, v);
            let (s2, v2) = hsl_to_hsv(hs, l);
            assert!((v - v2).abs() < 1e-4, "{s} {v}");
            if v > 0.0 {
                assert!((s - s2).abs() < 1e-4, "{s} {v}");
            }
        }
    }
}
