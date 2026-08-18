//! Shared plumbing reused by every mod DLL in this workspace: ini config,
//! file logging, and AOB pattern scanning. Each mod crate is a separate
//! `cdylib`, so this crate gets statically linked into (and its statics
//! duplicated across) every one of them independently - there is no shared
//! runtime state between mods, only shared source code.

pub mod codepatch;
pub mod config;
pub mod input;
pub mod logger;
pub mod memscan;

use std::ffi::c_void;

unsafe extern "system" {
    fn GetModuleFileNameA(hmodule: *mut c_void, lp_filename: *mut u8, n_size: u32) -> u32;
}

/// Directory the calling DLL itself was loaded from - not the process's
/// current working directory, which mod loaders don't always set to the game
/// folder. Falls back to "." if the WinAPI call fails for any reason.
pub fn dll_dir(hmodule: u64) -> String {
    let mut buf = [0u8; 260]; // MAX_PATH
    let len = unsafe { GetModuleFileNameA(hmodule as *mut c_void, buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 {
        return ".".to_string();
    }
    let path = String::from_utf8_lossy(&buf[..len as usize]).into_owned();
    match path.rfind('\\') {
        Some(idx) => path[..idx].to_string(),
        None => ".".to_string(),
    }
}
