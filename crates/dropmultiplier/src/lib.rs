#![allow(non_snake_case)] // crate name is "DropMultiplier" to control the output DLL's filename

mod drop_rate;

use common::{config, dll_dir, logger};

// DropMultiplier.ini is embedded verbatim into the binary at compile time
// via include_str! - no resource compiler step needed. This is the single
// source of truth for the default config: edit DropMultiplier.ini, rebuild,
// done.
const DEFAULT_INI: &str = include_str!("../DropMultiplier.ini");

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
        let ini_path = format!("{dir}\\DropMultiplier.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // The log is always on (overwritten every run) regardless of
        // [Logging] LogFile - same convention as every other mod in this
        // workspace. LogFile only gates the extra per-row/reload detail in
        // drop_rate.rs.
        logger::init(&dir, "DropMultiplier.log");
        logger::install_panic_hook();
        logger::log("Activating DropMultiplier...");
        if migrated > 0 {
            logger::log(&format!(
                "DropMultiplier.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        // `engine::reload` owns `General.ReloadKey` watching for the whole
        // DLL (see its module doc comment for why only one caller may poll
        // that key) - `drop_rate::run` below only polls
        // `engine::reload::RELOAD_GENERATION`.
        std::thread::spawn(move || engine::reload::run(ini_path));

        drop_rate::run();
    });

    true
}
