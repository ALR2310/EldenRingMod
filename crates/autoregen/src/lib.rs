#![allow(non_snake_case)] // crate name is "AutoRegen" to control the output DLL's filename

mod attack_hook;
mod regen;

use common::{config, dll_dir, logger};

const DEFAULT_INI: &str = include_str!("../AutoRegen.ini");

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
        let ini_path = format!("{dir}\\AutoRegen.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // The log is always on (overwritten every run) - AutoRegen's
        // original convention: no separate flag to remember to flip, and no
        // accumulation across play sessions. RegenLog only gates the extra
        // per-hit damage/heal dump in attack_hook, not this base log.
        logger::init(&dir, "AutoRegen.log");
        logger::install_panic_hook();
        logger::log("Activating AutoRegen...");
        if migrated > 0 {
            logger::log(&format!(
                "AutoRegen.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        regen::run(ini_path);
    });

    true
}
