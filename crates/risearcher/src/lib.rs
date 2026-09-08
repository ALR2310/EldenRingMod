#![allow(non_snake_case)] // crate name is "RiseArcher" to control the output DLL's filename

mod bullet;
mod reload;
mod task;
mod weapon;

use std::sync::atomic::Ordering;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use common::{config, dll_dir, logger};

// RiseArcher.ini is embedded verbatim into the binary at compile time via
// include_str! - no resource compiler step needed. This is the single
// source of truth for the default config: edit RiseArcher.ini, rebuild,
// done.
const DEFAULT_INI: &str = include_str!("../RiseArcher.ini");

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
        let ini_path = format!("{dir}\\RiseArcher.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        if config::get_bool("LogFile", false) {
            logger::init(&dir, "RiseArcher.log");
        }
        logger::log("Activating RiseArcher...");
        if migrated > 0 {
            logger::log(&format!(
                "RiseArcher.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        // `reload` owns General.ReloadKey watching for the whole DLL (see its
        // module doc comment for why only one module may call
        // eldenring::util::input::is_key_pressed for the same key) - `run`
        // below only polls `reload::RELOAD_GENERATION`.
        std::thread::spawn(move || reload::run(ini_path));

        run();
    });

    true
}

/// Waits (up to `timeout`) for `SoloParamRepository` - the live in-memory
/// regulation.bin - to become available, applies every weapon/bullet buff,
/// then watches `reload::RELOAD_GENERATION` for the rest of the DLL's
/// lifetime, reapplying from each row's cached original values (see
/// `weapon::Baseline`/`bullet::Baseline`) on every `ReloadKey` press.
///
/// Unlike `autoregen`, this doesn't need to run on the game's own frame
/// scheduler for the buff itself: regulation param rows are loaded once at
/// startup and never reloaded mid-session, so a single edit right after they
/// become available is all a normal run ever needs. The per-frame task below
/// exists only to detect the reload hotkey, same as every other
/// hot-reloadable feature in `sometweaks` (2026-09-08).
fn run() {
    let mut last_seen_generation = reload::RELOAD_GENERATION.load(Ordering::Relaxed);

    let Some(repo) = wait_for_repository(Duration::from_secs(60)) else {
        logger::log("ERROR: SoloParamRepository never became available - RiseArcher disabled for this session.");
        return;
    };

    let weapon_changed = weapon::apply(repo);
    let bullet_changed = bullet::apply(repo);
    logger::log(&format!(
        "Applied to {weapon_changed} EquipParamWeapon row(s) and {bullet_changed} Bullet row(s)."
    ));

    let cs_task = task::wait_for_cs_task();
    let _handle = task::run_recurring_safe(
        cs_task,
        "RiseArcher",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| {
            let generation = reload::RELOAD_GENERATION.load(Ordering::Relaxed);
            if generation == last_seen_generation {
                return;
            }
            last_seen_generation = generation;

            let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
                logger::warn("RiseArcher: SoloParamRepository not available on reload, skipped.");
                return;
            };
            let weapon_changed = weapon::apply(repo);
            let bullet_changed = bullet::apply(repo);
            logger::log(&format!(
                "Reload: reapplied to {weapon_changed} EquipParamWeapon row(s) and {bullet_changed} Bullet row(s) (hotkey pressed)."
            ));
        },
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn wait_for_repository(timeout: Duration) -> Option<&'static mut SoloParamRepository> {
    let step = Duration::from_millis(200);
    let mut waited = Duration::ZERO;
    loop {
        if let Ok(repo) = unsafe { SoloParamRepository::instance_mut() } {
            return Some(repo);
        }
        if waited >= timeout {
            return None;
        }
        std::thread::sleep(step);
        waited += step;
    }
}
