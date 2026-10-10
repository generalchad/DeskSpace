use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};

use crate::error::PlatformError;

const HOTKEY_BASE_ID: i32 = 0x100;

/// The id `Event::Hotkey` reports for the hotkey registered at `index`.
pub fn hotkey_id(index: i32) -> i32 {
    HOTKEY_BASE_ID + index
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeySpec {
    pub modifiers: HOT_KEY_MODIFIERS,
    pub virtual_key: u32,
}

pub fn register(hwnd: HWND, id: i32, spec: HotkeySpec) -> Result<Hotkey, PlatformError> {
    // SAFETY: `hwnd` is a live window owned by this thread; registration is
    // paired with `unregister` on the same window.
    unsafe {
        RegisterHotKey(Some(hwnd), id, spec.modifiers, spec.virtual_key)?;
    }
    Ok(Hotkey { id })
}

pub fn unregister(hwnd: HWND, hotkey: Hotkey) {
    // SAFETY: releases the registration made by `register` on the same window.
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), hotkey.id);
    }
}

pub fn parse(spec: &str) -> Option<HotkeySpec> {
    let mut modifiers = HOT_KEY_MODIFIERS(0);
    let mut virtual_key = None;

    for part in spec.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let upper = part.to_ascii_uppercase();
        match upper.as_str() {
            "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL,
            "ALT" => modifiers |= MOD_ALT,
            "SHIFT" => modifiers |= MOD_SHIFT,
            "WIN" | "SUPER" | "META" => modifiers |= MOD_WIN,
            other => {
                if virtual_key.is_some() {
                    return None;
                }
                virtual_key = Some(key_code(other)?);
            }
        }
    }

    Some(HotkeySpec {
        modifiers,
        virtual_key: virtual_key?,
    })
}

fn key_code(token: &str) -> Option<u32> {
    let bytes = token.as_bytes();
    if bytes.len() == 1 {
        let ch = bytes[0].to_ascii_uppercase();
        if ch.is_ascii_alphanumeric() {
            return Some(ch as u32);
        }
    }
    if let Some(rest) = token.strip_prefix('F') {
        if let Ok(number) = rest.parse::<u32>() {
            if (1..=24).contains(&number) {
                return Some(0x70 + number - 1);
            }
        }
    }
    match token {
        "SPACE" => Some(0x20),
        "TAB" => Some(0x09),
        "ENTER" | "RETURN" => Some(0x0D),
        "ESC" | "ESCAPE" => Some(0x1B),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ctrl_alt_d() {
        let spec = parse("Ctrl+Alt+D").unwrap();
        assert_eq!(spec.virtual_key, b'D' as u32);
        assert!(spec.modifiers.contains(MOD_CONTROL));
        assert!(spec.modifiers.contains(MOD_ALT));
    }

    #[test]
    fn parses_function_key() {
        let spec = parse("Ctrl+F5").unwrap();
        assert_eq!(spec.virtual_key, 0x74);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("Bogus").is_none());
    }
}
