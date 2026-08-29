#![allow(non_snake_case)] // crate name is "SomeTweaks" to control the output DLL's filename

mod drop_rate;
mod grace_menu;
mod misc;
mod player;
mod regen;
mod reload;
mod rune;
mod spirit;
mod task;

use common::{config, dll_dir, logger};
use misc::{torrent_anywhere, unlock_ashes_of_war, unlock_enchantments, warp_anywhere, weight_multiplier};
use rune::{keep_on_death as rune_keep_on_death, multiplier as rune_multiplier, reward as rune_reward};
use spirit::{
    color as spirit_color, regen as spirit_regen, summon_anywhere as spirit_summon_anywhere,
    summon_count as spirit_summon_count,
};

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

        // The log is always on (overwritten every run) regardless of
        // DebugLog - same convention as AutoRegen.
        logger::init(&dir, "SomeTweaks.log");
        logger::log("Activating SomeTweaks...");
        if migrated > 0 {
            logger::log(&format!(
                "SomeTweaks.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        // One single wait for the game engine, here, before any feature
        // thread exists (2026-08-28). Every feature ultimately needs
        // `CSTaskImp` (directly for a tick, or indirectly because its code
        // patch targets memory the game hasn't finished relocating yet), and
        // when each of them waited on its own they all raced the same
        // `InvalidRva` window and each logged its own retry line. Waiting
        // once here means the log reads "engine not ready -> engine ready ->
        // features" instead of interleaving ten copies of the same fact.
        // The features' own `task::wait_for_cs_task()` calls now return the
        // cached instance immediately.
        crate::task::wait_for_cs_task();

        // `reload` owns General.ReloadKey watching for the whole DLL (see its
        // module doc comment for why only one module may call
        // eldenring::util::input::is_key_pressed for the same key) - every
        // other feature below runs independently, on its own worker thread,
        // reading the same shared config map.
        std::thread::spawn(move || reload::run(ini_path));

        std::thread::spawn(rune_reward::run);
        std::thread::spawn(rune_multiplier::run);
        std::thread::spawn(rune_keep_on_death::run);
        std::thread::spawn(weight_multiplier::run);
        std::thread::spawn(drop_rate::run);
        std::thread::spawn(torrent_anywhere::run);
        std::thread::spawn(unlock_ashes_of_war::run);
        std::thread::spawn(unlock_enchantments::run);
        std::thread::spawn(warp_anywhere::run);
        std::thread::spawn(spirit_color::run);
        std::thread::spawn(spirit_regen::run);
        std::thread::spawn(spirit_summon_anywhere::run);
        std::thread::spawn(spirit_summon_count::run);
        std::thread::spawn(grace_menu::run);

        regen::run();
    });

    true
}
