#![allow(non_snake_case)] // crate name is "RiseArcher" to control the output DLL's filename

mod bullet;
mod weapon;

use std::time::Duration;

use eldenring::cs::SoloParamRepository;
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

        if config::get_bool("DebugLog", false) {
            logger::init(&dir, "RiseArcher.log");
        }
        logger::log("Activating RiseArcher...");
        if migrated > 0 {
            logger::log(&format!(
                "RiseArcher.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        run();
    });

    true
}

/// Waits (up to `timeout`) for `SoloParamRepository` - the live in-memory
/// regulation.bin - to become available, then applies every weapon/bullet
/// buff exactly once. Unlike `autoregen`/`sometweaks`, this doesn't need to
/// run on the game's own frame scheduler: regulation param rows are loaded
/// once at startup and never reloaded mid-session, so a single edit right
/// after they become available is all this mod ever needs to do.
fn run() {
    let Some(repo) = wait_for_repository(Duration::from_secs(60)) else {
        logger::log("ERROR: SoloParamRepository never became available - RiseArcher disabled for this session.");
        return;
    };

    let weapon_changed = weapon::apply(repo);
    let bullet_changed = bullet::apply(repo);
    logger::log(&format!(
        "Applied to {weapon_changed} EquipParamWeapon row(s) and {bullet_changed} Bullet row(s)."
    ));
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
