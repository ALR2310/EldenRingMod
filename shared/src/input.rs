//! Virtual-key name parsing shared by every mod that reads a hotkey ini key
//! (e.g. `ReloadKey`, `HotReloadKey`).

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
