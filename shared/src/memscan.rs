//! Generic IDA-style AOB (array-of-bytes) pattern scanner, restricted to the
//! executable sections of the current process's main module. Used by mods
//! that hook a bespoke call site not covered by fromsoftware-rs's reflected
//! API (e.g. `attack_hook`'s hit-resolution hook, derived from a public
//! Cheat Engine table rather than the game's own reflected engine types).
//!
//! Pattern example: "48 8B 05 ?? ?? ?? ?? 48 85 C0 74 07" - '??' or '?' marks a
//! wildcard byte.

use std::ffi::c_void;

unsafe extern "system" {
    fn GetModuleHandleA(lp_module_name: *const u8) -> *mut c_void;
}

#[repr(C)]
struct ImageDosHeader {
    e_magic: u16,
    _reserved: [u16; 29],
    e_lfanew: i32,
}

#[repr(C)]
struct ImageFileHeader {
    machine: u16,
    number_of_sections: u16,
    _reserved: [u8; 16],
}

#[repr(C)]
struct ImageSectionHeader {
    _name: [u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    _reserved: [u8; 24],
    characteristics: u32,
}

const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;

enum PatternByte {
    Exact(u8),
    Wildcard,
}

fn parse_pattern(pattern: &str) -> Vec<PatternByte> {
    pattern
        .split_whitespace()
        .map(|token| {
            if token == "?" || token == "??" {
                PatternByte::Wildcard
            } else {
                PatternByte::Exact(u8::from_str_radix(token, 16).unwrap_or(0))
            }
        })
        .collect()
}

fn find_pattern(haystack: &[u8], pattern: &[PatternByte]) -> Option<usize> {
    if pattern.is_empty() || haystack.len() < pattern.len() {
        return None;
    }
    let last = haystack.len() - pattern.len();
    'outer: for i in 0..=last {
        for (j, p) in pattern.iter().enumerate() {
            if let PatternByte::Exact(expected) = p {
                if haystack[i + j] != *expected {
                    continue 'outer;
                }
            }
        }
        return Some(i);
    }
    None
}

/// Scans the executable sections of the current process's main module (the
/// game .exe) for `pattern`, returning the absolute address of the first
/// match, or `None` if not found.
pub fn find_pattern_in_module(pattern: &str) -> Option<*mut u8> {
    let bytes = parse_pattern(pattern);
    let base = unsafe { GetModuleHandleA(std::ptr::null()) } as *const u8;
    if base.is_null() {
        return None;
    }

    unsafe {
        let dos = &*(base as *const ImageDosHeader);
        let nt_base = base.add(dos.e_lfanew as usize);
        // IMAGE_NT_HEADERS64: Signature(u32) + FileHeader + OptionalHeader.
        // We only need FileHeader, which starts right after the u32 signature.
        let file_header = &*(nt_base.add(4) as *const ImageFileHeader);
        // OptionalHeader (IMAGE_OPTIONAL_HEADER64) is 0xF0 bytes on x64 - the
        // section table follows it directly.
        let section_table = nt_base.add(4 + size_of::<ImageFileHeader>() + 0xF0) as *const ImageSectionHeader;

        for i in 0..file_header.number_of_sections as usize {
            let section = &*section_table.add(i);
            if section.characteristics & IMAGE_SCN_MEM_EXECUTE == 0 {
                continue;
            }
            let section_start = base.add(section.virtual_address as usize);
            let section_slice = std::slice::from_raw_parts(section_start, section.virtual_size as usize);
            if let Some(offset) = find_pattern(section_slice, &bytes) {
                return Some(section_start.add(offset) as *mut u8);
            }
        }
    }
    None
}
