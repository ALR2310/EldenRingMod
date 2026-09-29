#![allow(non_snake_case)] // crate name is "SpeedMultiplier" to control the output DLL's filename

mod probe;
mod speed;

use common::{config, dll_dir, logger};

// SpeedMultiplier.ini is embedded verbatim into the binary at compile time
// via include_str! - single source of truth for the default config.
const DEFAULT_INI: &str = include_str!("../SpeedMultiplier.ini");

/// # Safety
/// This is exposed this way so the library loader can call it. Do not call it
/// yourself.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DllMain(hmodule: u64, reason: u32) -> bool {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason != DLL_PROCESS_ATTACH {
        return true;
    }

    std::thread::spawn(move || {
        let dir = dll_dir(hmodule);
        let ini_path = format!("{dir}\\SpeedMultiplier.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // [Logging] LogFile gates the log file entirely - see common::logger.
        logger::init(&dir, "SpeedMultiplier.log");
        logger::install_panic_hook();
        logger::log("Activating SpeedMultiplier...");
        if migrated > 0 {
            logger::log(&format!(
                "SpeedMultiplier.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        std::thread::spawn(move || common::reload::run(ini_path));
        std::thread::spawn(speed::run);

        common::diag::log_environment_when_game_ready();
        probe::run();
    });

    true
}
