//! Environment dump for bug-report logs (2026-09-26): the game exe's file
//! version + PE timestamp, and every non-system DLL loaded into the process
//! (name, file version, base, size) - modeled on the header MapForGoblins
//! prints at startup.
//!
//! Added after a WeightMultiplier report where the log only said "anchor
//! pattern not found" - with no way to tell from it whether the user was on a
//! different game version or had another weight mod patching the same code.
//! With this, the log answers both questions itself.
//!
//! The list skips Windows' own DLLs (anything under the Windows directory),
//! the exe itself (already on the `Game:` line) and [HIDDEN_MODULES] - the
//! game's bundled runtime DLLs and what the Steam client injects - so what's
//! left is proxy loaders (dinput8/winmm/dxgi), mod loaders and mods: exactly
//! what matters when two mods collide.
//!
//! Privacy (2026-09-26): users are asked to post this log publicly (Nexus
//! comments), so it must not reveal more than needed. A module inside the
//! game folder is printed relative to it (`modengine2\bin\lua.dll`, which
//! also shows which loader a DLL belongs to); one outside it only by file
//! name - its full path could contain the Windows account name
//! (`C:\Users\<name>\...`). `steam_api64.dll` and `OnlineFix64.dll` are
//! hidden too, even though they'd hint at a cracked install: that's not
//! something a user should have to disclose to get help with a mod.

use std::ffi::c_void;

use crate::logger;
use crate::module_path;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn K32EnumProcessModules(process: *mut c_void, modules: *mut *mut c_void, cb: u32, needed: *mut u32) -> i32;
    fn GetSystemWindowsDirectoryW(buf: *mut u16, size: u32) -> u32;
}

#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(filename: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(filename: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
    fn VerQueryValueW(block: *const c_void, sub_block: *const u16, buf: *mut *mut c_void, len: *mut u32) -> i32;
}

/// One module loaded in this process.
pub struct ModuleInfo {
    /// File name only, e.g. `eldenring.exe`.
    pub name: String,
    pub path: String,
    pub base: usize,
    /// `SizeOfImage` from the in-memory PE header.
    pub size: u32,
    /// `FileHeader.TimeDateStamp` - identifies the exact build even when the
    /// file version string wasn't bumped.
    pub timestamp: u32,
    /// `a.b.c.d` from the file's version resource; `0.0.0.0` if it has none
    /// (most mod DLLs don't).
    pub version: String,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn file_version(path: &str) -> String {
    let path_w = wide(path);
    unsafe {
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(path_w.as_ptr(), &mut handle);
        if size == 0 {
            return "0.0.0.0".to_string();
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(path_w.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) == 0 {
            return "0.0.0.0".to_string();
        }
        let mut info: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let root = wide("\\");
        // VS_FIXEDFILEINFO: dwSignature, dwStrucVersion, dwFileVersionMS, dwFileVersionLS, ...
        if VerQueryValueW(data.as_ptr() as *const c_void, root.as_ptr(), &mut info, &mut len) == 0 || len < 16 {
            return "0.0.0.0".to_string();
        }
        let fixed = info as *const u32;
        let (ms, ls) = (*fixed.add(2), *fixed.add(3));
        format!("{}.{}.{}.{}", ms >> 16, ms & 0xFFFF, ls >> 16, ls & 0xFFFF)
    }
}

/// Reads `(SizeOfImage, TimeDateStamp)` from a loaded module's PE header.
fn pe_size_and_timestamp(base: *const u8) -> (u32, u32) {
    unsafe {
        let e_lfanew = *(base.add(0x3C) as *const i32);
        let nt = base.add(e_lfanew as usize);
        // NT headers: Signature(4) + FileHeader(20: .., TimeDateStamp at +4) + OptionalHeader(SizeOfImage at +56)
        let timestamp = *(nt.add(4 + 4) as *const u32);
        let size = *(nt.add(4 + 20 + 56) as *const u32);
        (size, timestamp)
    }
}

fn module_info(hmodule: *mut c_void) -> Option<ModuleInfo> {
    let path = module_path(hmodule)?;
    let name = path.rsplit('\\').next().unwrap_or(&path).to_string();
    let (size, timestamp) = pe_size_and_timestamp(hmodule as *const u8);
    Some(ModuleInfo { version: file_version(&path), name, path, base: hmodule as usize, size, timestamp })
}

/// Every module currently loaded in this process, in load order (the main
/// exe first).
pub fn loaded_modules() -> Vec<ModuleInfo> {
    let mut handles: Vec<*mut c_void> = vec![std::ptr::null_mut(); 512];
    loop {
        let mut needed = 0u32;
        let cb = (handles.len() * size_of::<*mut c_void>()) as u32;
        let ok = unsafe { K32EnumProcessModules(GetCurrentProcess(), handles.as_mut_ptr(), cb, &mut needed) };
        if ok == 0 {
            return Vec::new();
        }
        let count = needed as usize / size_of::<*mut c_void>();
        if count <= handles.len() {
            handles.truncate(count);
            break;
        }
        handles.resize(count, std::ptr::null_mut());
    }
    handles.into_iter().filter_map(module_info).collect()
}

/// Game-bundled and Steam-client DLLs left out of the logged list (compared
/// case-insensitively) - see the module doc.
const HIDDEN_MODULES: &[&str] = &[
    // Shipped with the game
    "bink2w64.dll",
    "amd_ags_x64.dll",
    "oo2core_6_win64.dll",
    "eossdk-win64-shipping.dll",
    "steam_api64.dll",
    // Injected by the Steam client
    "steamclient64.dll",
    "tier0_s64.dll",
    "vstdlib_s64.dll",
    "gameoverlayrenderer64.dll",
    // Online-play crack - same reasoning as steam_api64 (see module doc)
    "onlinefix64.dll",
];

/// `m`'s path relative to `game_dir` if it's inside it, else just its file
/// name - never a full path (see module doc).
fn display_path(m: &ModuleInfo, game_dir: &str) -> String {
    // Byte-length compare, not `to_lowercase()` on both sides: lowercasing a
    // non-ASCII path can change its byte length, and slicing by the wrong
    // length would panic on exactly the paths that broke `dll_dir` before.
    let n = game_dir.len();
    let inside = n > 0
        && m.path.is_char_boundary(n)
        && m.path[..n].eq_ignore_ascii_case(game_dir)
        && m.path[n..].starts_with('\\');
    if inside { m.path[n + 1..].to_string() } else { m.name.clone() }
}

fn windows_dir() -> String {
    let mut buf = vec![0u16; 260];
    let n = unsafe { GetSystemWindowsDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n >= buf.len() {
        return "c:\\windows".to_string();
    }
    String::from_utf16_lossy(&buf[..n]).to_lowercase()
}

/// The loaded module whose image contains `addr`, if any - e.g. to name
/// which DLL a foreign `jmp` patched into game code points into.
pub fn module_containing(addr: usize) -> Option<ModuleInfo> {
    loaded_modules().into_iter().find(|m| addr >= m.base && addr < m.base + m.size as usize)
}

/// Logs the game exe line and the non-system module list (see module doc).
/// Call it late enough that the other mods have loaded too - e.g. right
/// before the mod's own AOB scan/hook install, not straight from `DllMain`.
pub fn log_environment() {
    let exe = unsafe { GetModuleHandleW(std::ptr::null()) };
    if let Some(game) = module_info(exe) {
        logger::log(&format!(
            "Game: {} v{} base=0x{:X} size=0x{:X} ts=0x{:08X}",
            game.name, game.version, game.base, game.size, game.timestamp
        ));
    }

    let game_dir = module_path(exe)
        .and_then(|p| p.rfind('\\').map(|i| p[..i].to_string()))
        .unwrap_or_default();
    let win_dir = windows_dir();
    let mut hidden = 0;
    let modules: Vec<ModuleInfo> = loaded_modules()
        .into_iter()
        // The exe is already fully described by the "Game:" line above.
        .filter(|m| m.base != exe as usize && !m.path.to_lowercase().starts_with(&win_dir))
        .filter(|m| {
            let hide = HIDDEN_MODULES.iter().any(|h| m.name.eq_ignore_ascii_case(h));
            hidden += hide as usize;
            !hide
        })
        .collect();
    logger::log(&format!("Loaded modules ({}/{}):", modules.len(), modules.len() + hidden));
    for m in &modules {
        logger::log(&format!(
            "  {:<40} v{:<12} base=0x{:X} size=0x{:X}",
            display_path(m, &game_dir),
            m.version,
            m.base,
            m.size
        ));
    }
}

/// Waits for `CSTaskImp` (the game has finished initializing - every mod
/// loader has loaded its DLLs by then), then [log_environment]. For a mod's
/// startup thread, right before its feature starts: several features only
/// reach their own `wait_for_cs_task` after the player is in the world,
/// which would be too late for a "mod never started" report. The wait is
/// cached (see `task::wait_for_cs_task`), so the feature's own later call
/// returns immediately.
pub fn log_environment_when_game_ready() {
    crate::task::wait_for_cs_task();
    log_environment();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_own_process_modules() {
        let modules = loaded_modules();
        let exe = &modules[0];
        assert!(exe.name.to_lowercase().ends_with(".exe"), "{}", exe.name);
        assert!(exe.size > 0 && exe.timestamp != 0);
        assert!(modules.iter().any(|m| m.name.eq_ignore_ascii_case("kernel32.dll") && m.version != "0.0.0.0"));
        // An address inside this test binary's own code resolves to the exe.
        let here = lists_own_process_modules as usize;
        assert_eq!(module_containing(here).map(|m| m.base), Some(exe.base));
    }

    fn module(path: &str) -> ModuleInfo {
        let name = path.rsplit('\\').next().unwrap().to_string();
        ModuleInfo { name, path: path.to_string(), base: 0, size: 0, timestamp: 0, version: String::new() }
    }

    #[test]
    fn display_path_never_leaks_outside_game_dir() {
        let game = r"E:\SteamLibrary\steamapps\common\ELDEN RING\Game";
        assert_eq!(display_path(&module(r"E:\SteamLibrary\steamapps\common\ELDEN RING\Game\modengine2\bin\lua.dll"), game), r"modengine2\bin\lua.dll");
        // Case differs from the exe's path - still inside.
        assert_eq!(display_path(&module(r"e:\steamlibrary\steamapps\common\elden ring\game\mod\AutoRegen.dll"), game), r"mod\AutoRegen.dll");
        // Outside the game folder: file name only, no user name.
        assert_eq!(display_path(&module(r"C:\Users\Nguyễn Văn A\ME3\mods\X.dll"), game), "X.dll");
        // Sibling folder sharing the prefix isn't "inside".
        assert_eq!(display_path(&module(r"E:\SteamLibrary\steamapps\common\ELDEN RING\GameBackup\Y.dll"), game), "Y.dll");
        // Non-ASCII game folder: no panic, still relative.
        let vn = r"D:\Trò chơi\ELDEN RING\Game";
        assert_eq!(display_path(&module(r"D:\Trò chơi\ELDEN RING\Game\mod\Z.dll"), vn), r"mod\Z.dll");
        assert_eq!(display_path(&module(r"D:\x.dll"), ""), "x.dll");
    }
}
