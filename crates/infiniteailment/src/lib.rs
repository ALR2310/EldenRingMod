#![allow(non_snake_case)] // crate name is "InfiniteAilment" to control the output DLL's filename

mod status_effect;

use common::{config, dll_dir, logger};

// InfiniteAilment.ini is embedded verbatim into the binary at compile time
// via include_str! - no resource compiler step needed. This is the single
// source of truth for the default config: edit InfiniteAilment.ini, rebuild,
// done.
const DEFAULT_INI: &str = include_str!("../InfiniteAilment.ini");

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
        let ini_path = format!("{dir}\\InfiniteAilment.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        logger::init(&dir, "InfiniteAilment.log");
        logger::install_panic_hook();
        logger::log("Activating InfiniteAilment...");
        if migrated > 0 {
            logger::log(&format!(
                "InfiniteAilment.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        // `common::reload` owns `General.ReloadKey` watching for the whole
        // DLL (see its module doc comment for why only one caller may poll
        // that key) - `status_effect::run` below only polls
        // `common::reload::RELOAD_GENERATION`.
        std::thread::spawn(move || common::reload::run(ini_path));

        status_effect::run();
    });

    true
}
