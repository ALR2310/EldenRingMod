//! Shared plumbing reused by every mod DLL in this workspace: ini config,
//! file logging, AOB pattern scanning, and (in `task_hook`/`alloc_hook`/
//! `task`/`player`/`reload`) code that talks to the game's own reflected
//! engine types (`eldenring`/`fromsoftware-shared`). Each mod crate is a
//! separate `cdylib`, so this crate gets statically linked into (and its
//! statics duplicated across) every one of them independently - there is no
//! shared runtime state between mods, only shared source code.
//!
//! The `eldenring`-dependent modules were briefly a separate crate (`engine`,
//! 2026-09-14) so a mod like `weightmultiplier` - which patches raw code and
//! never touches a reflected engine struct - wouldn't need to depend on
//! `eldenring`/`fromsoftware-shared` at all. Merged back into this one crate
//! the same day: LTO already strips the unused code from any mod's final
//! `.dll` regardless (this workspace's `[profile.release]` has `lto = true`,
//! `codegen-units = 1`), so the only real cost of the split was organizational
//! (two folders to keep straight) against a real but modest downside (a
//! from-clean build of a mod that doesn't use the engine modules would still
//! have to compile `eldenring` once) - not worth two crates for.
//!
//! - `task_hook`/`alloc_hook`: resolve the game's task-registration function
//!   and its global heap allocator via AOB instead of `fromsoftware-rs`'s
//!   version-gated `eldenring::rva::get()` table, so a mod doesn't panic
//!   outright the moment the game updates past whatever single version that
//!   table was last published for.
//! - `task`: acquires `CSTaskImp` (also without `rva::get()`) and wraps a
//!   per-frame closure with `catch_unwind`, so a panic there can't unwind
//!   into the game's own (non-Rust) call stack.
//! - `player`: the "is the player actually in the game world yet" gate that
//!   `GameDataMan`/`SoloParamRepository`-touching features need before their
//!   data is safe to read.
//! - `reload`: watches a `ReloadKey` ini hotkey and reloads config,
//!   independent of any feature module.
//! - `announce`: shows text in the game's own top-of-screen system
//!   announcement banner (e.g. "config reloaded"), for in-game confirmation
//!   without needing to check a log file.

pub mod alloc_hook;
pub mod announce;
pub mod codepatch;
pub mod config;
pub mod input;
pub mod logger;
pub mod memscan;
pub mod player;
pub mod reload;
pub mod task;
pub mod task_hook;

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
