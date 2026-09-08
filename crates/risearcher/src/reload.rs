//! Watches `General.ReloadKey` and reloads the shared ini config on press.
//! Ported from `sometweaks::reload` (2026-09-08) - same reasoning: runs on
//! its own worker thread/tick, independent of `weapon`/`bullet`, and exposes
//! [RELOAD_GENERATION] for them to poll instead of calling
//! `eldenring::util::input::is_key_pressed` themselves (that call debounces
//! per VK code in a single map shared by every caller - see
//! `crates/eldenring/src/util/input.rs`: `DEBOUNCE_MAP` - so more than one
//! caller for the same key would mean only whichever ran first that frame
//! ever saw `true`). This module must stay the ONLY caller for `ReloadKey`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use eldenring::cs::CSTaskGroupIndex;
use eldenring::util::input;

use common::config;
use common::input::parse_virtual_key;
use common::logger;

const VK_F5: i32 = 0x74;

pub static RELOAD_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Watches `General.ReloadKey` on the game's own `FrameBegin` task group for
/// the rest of the DLL's lifetime, reloading `ini_path` into the shared
/// config map and bumping [RELOAD_GENERATION] on every press. Meant to run
/// on its own worker thread spawned from `DllMain`; never returns.
pub fn run(ini_path: String) {
    let cs_task = crate::task::wait_for_cs_task();

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "Reload",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| {
            let reload_key = parse_virtual_key(&config::get_string("ReloadKey", "F5"), VK_F5);
            if input::is_key_pressed(reload_key) {
                config::load(&ini_path);
                RELOAD_GENERATION.fetch_add(1, Ordering::Relaxed);
                logger::log("Reload: config reloaded (hotkey pressed).");
            }
        },
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
