#![allow(non_snake_case)] // crate name is "WeightMultiplier" to control the output DLL's filename

mod hook;

use common::{config, dll_dir, logger};

// WeightMultiplier.ini is embedded verbatim into the binary at compile time
// via include_str! - no resource compiler step needed. This is the single
// source of truth for the default config: edit WeightMultiplier.ini, rebuild,
// done.
const DEFAULT_INI: &str = include_str!("../WeightMultiplier.ini");

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
        let ini_path = format!("{dir}\\WeightMultiplier.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // The log is always on (overwritten every run), same convention as
        // the original ReductionWeight's unconditional Logger::Init(dir) -
        // there's no DebugLog/EnableLog key in this mod.
        logger::init(&dir, "WeightMultiplier.log");
        logger::log("Activating WeightMultiplier...");
        if migrated > 0 {
            logger::log(&format!(
                "WeightMultiplier.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        hook::run();
    });

    true
}
