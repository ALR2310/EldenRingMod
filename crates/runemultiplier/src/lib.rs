#![allow(non_snake_case)] // crate name is "RuneMultiplier" to control the output DLL's filename

mod hook;

use common::{config, dll_dir, logger};

// RuneMultiplier.ini is embedded verbatim into the binary at compile time via
// include_str! - no resource compiler step needed (the original C++ used a
// .rc RCDATA resource for the same purpose). This is the single source of
// truth for the default config: edit RuneMultiplier.ini, rebuild, done.
const DEFAULT_INI: &str = include_str!("../RuneMultiplier.ini");

/// # Safety
/// This is exposed this way so the library loader can call it. Do not call it
/// yourself. Mirrors the entry point shape used across fromsoftware-rs's own
/// examples (e.g. examples/apply-speffect).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DllMain(hmodule: u64, reason: u32) -> bool {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason != DLL_PROCESS_ATTACH {
        return true;
    }

    std::thread::spawn(move || {
        let dir = dll_dir(hmodule);
        let ini_path = format!("{dir}\\RuneMultiplier.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // DebugLog gates the log file entirely (off by default) - same
        // convention as the original RuneMultiplier's Logger::Init(dir).
        if config::get_bool("DebugLog", false) {
            logger::init(&dir, "RuneMultiplier.log");
        }
        logger::log("Activating RuneMultiplier...");
        if migrated > 0 {
            logger::log(&format!(
                "RuneMultiplier.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        hook::run(ini_path, dir);
    });

    true
}
