#![allow(non_snake_case)] // crate name is "SomeTweaks" to control the output DLL's filename

mod regen;

use common::{config, dll_dir, logger};

// SomeTweaks.ini is embedded verbatim into the binary at compile time via
// include_str! - no resource compiler step needed. This is the single source
// of truth for the default config: edit SomeTweaks.ini, rebuild, done.
const DEFAULT_INI: &str = include_str!("../SomeTweaks.ini");

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
        let ini_path = format!("{dir}\\SomeTweaks.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // The log is always on (overwritten every run) so per-module flags
        // like Regen.PerTick/PerHit's RegenLog have something to write to
        // even with DebugLog=false - same convention as AutoRegen.
        logger::init(&dir, "SomeTweaks.log");
        logger::log("Activating SomeTweaks...");
        if migrated > 0 {
            logger::log(&format!(
                "SomeTweaks.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        regen::run(ini_path);
    });

    true
}
