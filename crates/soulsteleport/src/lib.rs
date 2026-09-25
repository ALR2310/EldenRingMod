#![allow(non_snake_case)] // crate name is "SoulsTeleport" to control the output DLL's filename

mod input_block;
mod net;
mod party;
mod steam;
mod ui;
mod warp;

use common::{config, dll_dir, logger};

// SoulsTeleport.ini is embedded verbatim into the binary at compile time via
// include_str! - no resource compiler step needed. This is the single source
// of truth for the default config: edit SoulsTeleport.ini, rebuild, done.
const DEFAULT_INI: &str = include_str!("../SoulsTeleport.ini");

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
        let ini_path = format!("{dir}\\SoulsTeleport.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // [Logging] LogFile gates the log file entirely - see common::logger.
        logger::init(&dir, "SoulsTeleport.log");
        logger::install_panic_hook();
        logger::log("Activating SoulsTeleport...");
        if migrated > 0 {
            logger::log(&format!(
                "SoulsTeleport.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        ui::set_ini_path(ini_path.clone());
        ui::install();

        warp::run();
    });

    true
}
