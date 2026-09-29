#![allow(non_snake_case)] // crate name is "SpiritMultiplier" to control the output DLL's filename

mod activate_limit;
mod band;
mod buddy_stone;
mod chain;
mod enemy_probe;
mod ghost_color;
mod multi_spirit;
mod probe;
mod regen;

use common::{config, dll_dir, logger};

// SpiritMultiplier.ini is embedded verbatim into the binary at compile time
// via include_str! - single source of truth for the default config.
const DEFAULT_INI: &str = include_str!("../SpiritMultiplier.ini");

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
        let ini_path = format!("{dir}\\SpiritMultiplier.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // [Logging] LogFile gates the log file entirely - see common::logger.
        logger::init(&dir, "SpiritMultiplier.log");
        logger::install_panic_hook();
        logger::log("Activating SpiritMultiplier...");
        if migrated > 0 {
            logger::log(&format!(
                "SpiritMultiplier.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        // Band first: without it, chains longer than 10 are harmless (the
        // engine just drops the extra spirits) - so a failed band patch
        // still leaves the chain half running, it just can't exceed 10.
        // MaxSpirits sizes the summon ChrSet itself, so it's read once here
        // and only takes effect on the next game start (unlike Amount/
        // Multiplier, which chain.rs re-reads every tick).
        let max_spirits = config::get_int("MaxSpirits", band::DEFAULT_MAX_SPIRITS as i32).max(0) as u32;
        if !band::install(max_spirits) {
            logger::error("Band patch not installed - spirits stay capped at 10 per cast.");
        }

        std::thread::spawn(move || common::reload::run(ini_path));
        std::thread::spawn(probe::run);
        std::thread::spawn(enemy_probe::run);
        std::thread::spawn(activate_limit::run);
        std::thread::spawn(ghost_color::run);
        std::thread::spawn(regen::run);
        std::thread::spawn(multi_spirit::run);
        std::thread::spawn(buddy_stone::run);

        common::diag::log_environment_when_game_ready();
        chain::run();
    });

    true
}

