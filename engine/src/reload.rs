//! Watches a hotkey ini key (`ReloadKey` by convention) and reloads the
//! shared ini config on press, independent of any feature module - so a
//! mod's various features don't have to each poll the key themselves (and
//! don't have to be running for reload to work at all).
//!
//! Exposes [RELOAD_GENERATION], an atomic counter bumped on every reload, for
//! features that need to detect the reload *edge* specifically (e.g. to
//! recompute from a cached original-value baseline rather than compounding)
//! - most features don't need this at all, since `common::config` is
//! already a live shared map and a plain per-tick `config::get_*` call picks
//! up a reload for free.
//!
//! `eldenring::util::input::is_key_pressed` debounces per VK code in a
//! single map shared by every caller (see `crates/eldenring/src/util/
//! input.rs`: `DEBOUNCE_MAP`), not per caller - if more than one module
//! called it for the same key, only whichever happened to run first that
//! frame would ever see `true`. [run] must stay the ONLY caller for a given
//! `ReloadKey` binding in a process; other modules poll [RELOAD_GENERATION]
//! instead.
//!
//! Ported from `sometweaks::reload`/`risearcher::reload` (both carried an
//! identical copy) once a third mod needed the same watcher, now built on
//! this crate's own AOB-resilient [`crate::task`] instead of each mod's own.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use eldenring::cs::CSTaskGroupIndex;
use eldenring::util::input;

use common::config;
use common::input::parse_virtual_key;
use common::logger;

const VK_F5: i32 = 0x74;

pub static RELOAD_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Watches `ReloadKey` on the game's own `FrameBegin` task group for the
/// rest of the process's life, reloading `ini_path` into the shared config
/// map and bumping [RELOAD_GENERATION] on every press. Meant to run on its
/// own worker thread spawned from `DllMain`; never returns.
pub fn run(ini_path: String) {
    let cs_task = crate::task::wait_for_cs_task();

    crate::task::run_recurring_safe(
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
