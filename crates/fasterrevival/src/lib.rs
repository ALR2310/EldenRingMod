#![allow(non_snake_case)] // crate name is "FasterRevival" to control the output DLL's filename

mod death;

use common::{config, dll_dir, logger};

// FasterRevival.ini is embedded verbatim into the binary at compile time
// via include_str! - no resource compiler step needed. This is the single
// source of truth for the default config: edit FasterRevival.ini, rebuild,
// done.
const DEFAULT_INI: &str = include_str!("../FasterRevival.ini");

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
        let ini_path = format!("{dir}\\FasterRevival.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        logger::init(&dir, "FasterRevival.log");
        logger::install_panic_hook();
        logger::log("Activating FasterRevival...");
        if migrated > 0 {
            logger::log(&format!(
                "FasterRevival.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        // `common::reload` owns `General.ReloadKey` watching for the whole
        // DLL (see its module doc comment for why only one caller may poll
        // that key) - `death::run` below just re-reads its keys every frame.
        std::thread::spawn(move || common::reload::run(ini_path));

        common::diag::log_environment_when_game_ready();
        death::run();
    });

    true
}
