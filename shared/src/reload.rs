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

use crate::config;
use crate::input::parse_virtual_key;
use crate::logger;

const VK_F5: i32 = 0x74;

pub static RELOAD_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Watches `ReloadKey` on the game's own `FrameBegin` task group for the
/// rest of the process's life, reloading `ini_path` into the shared config
/// map and bumping [RELOAD_GENERATION] on every press, plus showing a
/// generic "Config reloaded" in-game banner (see [`crate::announce`]) so
/// the reload is visible without checking the log file (`ReloadBanner=false`
/// turns the banner off). Meant to run on its
/// own worker thread spawned from `DllMain`; never returns.
pub fn run(ini_path: String) {
    run_with(
        || config::get_string("ReloadKey", "F5"),
        || config::get_bool("ReloadBanner", true),
        move || {
            config::load(&ini_path);
            Ok(())
        },
    );
}

/// [run] for a config that isn't the ini map ([`crate::toml_config`]):
/// `reload_key` returns the current key name (re-read every frame, so a
/// reload that changes it takes effect at once), `reload` re-reads the
/// config. An `Err` (e.g. a TOML syntax error, the old config still in use)
/// is logged and shown as "Config error - see log" instead of "Config
/// reloaded"; [RELOAD_GENERATION] is only bumped on success.
/// `banner` (read after the reload, so a reload that turns it off takes
/// effect at once) says whether to show the "Config reloaded" banner;
/// the error banner always shows - without it a failed reload would be
/// silent in game.
pub fn run_with<K, B, R>(reload_key: K, banner: B, mut reload: R)
where
    K: Fn() -> String + Send + 'static,
    B: Fn() -> bool + Send + 'static,
    R: FnMut() -> Result<(), String> + Send + 'static,
{
    let cs_task = crate::task::wait_for_cs_task();

    crate::task::run_recurring_safe(
        cs_task,
        "Reload",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| {
            let key = parse_virtual_key(&reload_key(), VK_F5);
            if !input::is_key_pressed(key) {
                return;
            }
            match reload() {
                Ok(()) => {
                    RELOAD_GENERATION.fetch_add(1, Ordering::Relaxed);
                    logger::log("Config reloaded");
                    if banner() {
                        crate::announce::show_announcement("Config reloaded");
                    }
                }
                Err(err) => {
                    logger::error(&format!("Reload: config not reloaded, the previous one stays in use:
{err}"));
                    crate::announce::show_announcement("Config error - see log");
                }
            }
        },
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
