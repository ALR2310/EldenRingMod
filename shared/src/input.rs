//! Virtual-key name parsing shared by every mod that reads a hotkey ini key
//! (e.g. `ReloadKey`, `HotReloadKey`), plus a raw-Win32 key-press poll for
//! mods with no `fromsoftware-rs` dependency (which otherwise use
//! `eldenring::util::input::is_key_pressed`, registered on the game's own
//! per-frame task scheduler - not an option here).

use std::ffi::c_void;
use std::sync::atomic::{AtomicU8, Ordering};

#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(v_key: i32) -> i16;
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowThreadProcessId(hwnd: *mut c_void, process_id: *mut u32) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcessId() -> u32;
}

// Per-VK state kept between polls (one DLL = one copy, like every other
// static in this crate).
const KEY_RESYNC: u8 = 0; // unknown - (re)gained focus, next poll only records
const KEY_UP: u8 = 1;
const KEY_DOWN: u8 = 2;
static KEY_STATE: [AtomicU8; 256] = [const { AtomicU8::new(KEY_RESYNC) }; 256];

fn game_window_has_focus() -> bool {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return false;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid == unsafe { GetCurrentProcessId() }
}

/// Whether `vk` was pressed since the last call to this function for the
/// same key, while this process's own window had focus. Meant to be polled
/// periodically from a plain thread loop (e.g. every 100ms).
///
/// Used to be just `GetAsyncKeyState(vk) & 1` (2026-09-24 fix). That low
/// "pressed since last call" bit is one system-wide flag, cleared by whichever
/// process calls first - with 2 game instances on one PC (Seamless Co-op
/// testing), the host's own poll kept consuming presses meant for the other
/// window, and a hotkey also fired while typing in any other app. Now:
/// - never polls at all while another process's window is focused (so it
///   can't consume a press meant for that window either);
/// - on (re)gaining focus, the first poll only records the key's state, so a
///   press made in another app before alt-tabbing back doesn't fire;
/// - fires on the low bit (catches a tap shorter than the poll interval) or
///   on an up -> down transition seen by our own per-key state.
pub fn is_key_pressed(vk: i32) -> bool {
    let Some(state) = usize::try_from(vk).ok().and_then(|i| KEY_STATE.get(i)) else {
        return false;
    };
    if !game_window_has_focus() {
        state.store(KEY_RESYNC, Ordering::Relaxed);
        return false;
    }
    let raw = unsafe { GetAsyncKeyState(vk) };
    let down = raw < 0; // high bit: held right now
    let tapped = raw & 1 != 0;
    let previous = state.swap(if down { KEY_DOWN } else { KEY_UP }, Ordering::Relaxed);
    match previous {
        KEY_RESYNC => false,
        KEY_UP => down || tapped,
        _ => tapped && !down, // released and pressed again between polls
    }
}

/// Parses, in order of precedence: a raw virtual-key code in hex ("0x2D") or
/// decimal ("112"), "F1".."F24", or a single letter/digit. Returns `fallback`
/// for anything else (empty string, typo, unparseable, out of range).
/// https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
pub fn parse_virtual_key(name: &str, fallback: i32) -> i32 {
    let upper = name.trim().to_ascii_uppercase();

    if let Some(hex) = upper.strip_prefix("0X") {
        if let Ok(vk) = i32::from_str_radix(hex, 16) {
            if vk > 0 && vk <= 0xFF {
                return vk;
            }
        }
    }
    if upper.len() > 1 && upper.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(vk) = upper.parse::<i32>() {
            if vk > 0 && vk <= 0xFF {
                return vk;
            }
        }
    }
    if upper.len() >= 2 && upper.starts_with('F') {
        if let Ok(n) = upper[1..].parse::<i32>() {
            if (1..=24).contains(&n) {
                return 0x70 + (n - 1); // VK_F1 = 0x70, VK_F2 = 0x71, ...
            }
        }
    }
    if upper.len() == 1 {
        let c = upper.chars().next().unwrap();
        if c.is_ascii_alphanumeric() {
            return c as i32; // VK codes for '0'-'9' and 'A'-'Z' match their ASCII value.
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    const FALLBACK: i32 = 0x74; // VK_F5

    #[test]
    fn parses_hex() {
        assert_eq!(parse_virtual_key("0x2D", FALLBACK), 0x2D);
    }

    #[test]
    fn parses_decimal() {
        assert_eq!(parse_virtual_key("112", FALLBACK), 112);
    }

    #[test]
    fn parses_function_keys() {
        assert_eq!(parse_virtual_key("F1", FALLBACK), 0x70);
        assert_eq!(parse_virtual_key("F9", FALLBACK), 0x78);
        assert_eq!(parse_virtual_key("f24", FALLBACK), 0x70 + 23);
    }

    #[test]
    fn parses_single_char() {
        assert_eq!(parse_virtual_key("g", FALLBACK), 'G' as i32);
        assert_eq!(parse_virtual_key("5", FALLBACK), '5' as i32); // VK codes for '0'-'9' match ASCII
    }

    #[test]
    fn falls_back_on_garbage() {
        assert_eq!(parse_virtual_key("", FALLBACK), FALLBACK);
        assert_eq!(parse_virtual_key("???", FALLBACK), FALLBACK);
        assert_eq!(parse_virtual_key("F99", FALLBACK), FALLBACK);
    }
}
