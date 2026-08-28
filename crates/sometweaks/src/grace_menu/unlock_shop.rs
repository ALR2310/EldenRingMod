//! Unlocks every row of `ShopLineupParam` (forces
//! `event_flag_for_release = -1`, i.e. `u32::MAX`) so `GraceMenu.Shop`'s
//! combined mega-shop (see `super`) - and every other NPC shop menu in
//! the game - shows items that are normally hidden until some other
//! in-game condition is met (meeting the right NPC, progressing a quest,
//! etc.), instead of only whatever's already unlocked for this
//! playthrough.
//!
//! Ported from the community's own `Elden-Ring-CT-TGA` Cheat Engine
//! table script ("Access all shop inventory.cea",
//! `.docs/Elden-Ring-CT-TGA`): `ParamPatchAll(ShopLineupParam, {
//! param->eventFlag_forRelease = -1; })` - same idea `GraceMenu.Shop`'s
//! own item lot range (`0..9999999`) was borrowed from that project's
//! "All Shops.cea" script.
//!
//! Independent of `GraceMenu.Shop` - can be turned off to only see items
//! actually unlocked through normal play, while still using the combined
//! shop for convenience.
//!
//! Unlike an earlier version of this module, supports `General.ReloadKey`
//! hot reload (same pattern `drop_rate` uses): the game's own original
//! `event_flag_for_release` values are captured once, before any edit, so
//! toggling `GraceMenu.UnlockShop=false` and reloading actually restores
//! each row's real vanilla flag instead of leaving it force-unlocked.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, ShopLineupParam, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

// The game's own original `event_flag_for_release` values, keyed by row
// ID - captured once (lazily, on the first call to `apply`) before any
// edit, so every later `ReloadKey` press can actually restore the real
// vanilla flag when toggled off, not just leave rows force-unlocked
// forever. Guarded by a `Mutex` for the same reason `drop_rate`'s own
// snapshot is: the initial capture runs on this module's own worker
// thread, later reloads run on whichever thread the game's `FrameBegin`
// task group executes the recurring closure on.
static ORIGINAL_FLAGS: Mutex<Option<HashMap<u32, u32>>> = Mutex::new(None);

/// Sets every `ShopLineupParam` row's `event_flag_for_release` to either
/// `u32::MAX` (`GraceMenu.UnlockShop=true` - the param's own `-1`
/// sentinel meaning "always released") or its cached original value
/// (`false` - undoes a previous `true`/`ReloadKey` press). Returns how
/// many rows were touched, for logging.
fn apply(repo: &mut SoloParamRepository, unlock: bool) -> usize {
    let mut snapshot_guard = ORIGINAL_FLAGS.lock().unwrap();
    let is_first_call = snapshot_guard.is_none();
    let snapshot = snapshot_guard.get_or_insert_with(|| {
        repo.rows_mut::<ShopLineupParam>().map(|(id, row)| (id, row.event_flag_for_release())).collect()
    });
    if is_first_call {
        logger::log(&format!("GraceMenu: snapshotted {} ShopLineupParam row(s).", snapshot.len()));
    }

    let mut changed = 0;
    for (id, row) in repo.rows_mut::<ShopLineupParam>() {
        let Some(&original) = snapshot.get(&id) else {
            continue; // shouldn't happen - every row was snapshotted above
        };
        row.set_event_flag_for_release(if unlock { u32::MAX } else { original });
        changed += 1;
    }
    changed
}

/// Applies the param edit once (skipped entirely at startup if
/// `GraceMenu.UnlockShop=false`, same as `drop_rate`'s own
/// `DropRate.Enabled` - no reason to wait up to 300s for
/// `SoloParamRepository` when nothing wants to touch it yet), then
/// watches `General.ReloadKey` on the game's own `FrameBegin` task group
/// for the rest of the DLL's lifetime, re-applying (or restoring, if
/// toggled off) on every press. Meant to run on its own worker thread
/// spawned from `DllMain`; never returns.
pub fn run() {
    let mut last_seen_generation = crate::reload::RELOAD_GENERATION.load(Ordering::Relaxed);

    if config::get_bool("GraceMenu.UnlockShop", false) {
        match crate::player::wait_for_solo_param_repository(Duration::from_secs(300)) {
            Some(repo) => {
                let rows = apply(repo, true);
                logger::log(&format!("GraceMenu.UnlockShop=true applied to {rows} ShopLineupParam row(s)."));
            }
            None => {
                logger::error("GraceMenu: SoloParamRepository never became available, UnlockShop disabled for this session.");
            }
        }
    } else {
        logger::log("GraceMenu.UnlockShop=false - skipping entirely at startup.");
    }

    let cs_task = crate::task::wait_for_cs_task();
    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "GraceMenu.UnlockShop",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| {
            // Same reasoning as `drop_rate`'s own reload tick: poll
            // `reload::RELOAD_GENERATION` instead of watching the key
            // ourselves - `reload` must stay the only caller for
            // `General.ReloadKey` (see that module's doc comment).
            let generation = crate::reload::RELOAD_GENERATION.load(Ordering::Relaxed);
            if generation == last_seen_generation {
                return;
            }
            last_seen_generation = generation;

            let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
                logger::warn("GraceMenu: SoloParamRepository not available on reload, UnlockShop skipped.");
                return;
            };
            let unlock = config::get_bool("GraceMenu.UnlockShop", false);
            let rows = apply(repo, unlock);
            logger::log(&format!("GraceMenu.UnlockShop={unlock} applied to {rows} ShopLineupParam row(s) (hotkey pressed)."));
        },
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
