//! Allows interacting with (resting at) a Site of Grace without dismounting
//! Torrent, by clearing 2 bits on `ActionButtonParam` row `6100` ("Touch
//! grace") - `is_invalid_for_ride` (hides the prompt entirely while mounted)
//! and `is_grayout_for_ride` (shows it but grays it out, same effect) - both
//! confirmed via a real exported `ActionButtonParam.csv` (SmithBox): row
//! `6100` ships with `isGrayoutForRide=1`, `isInvalidForRide=0`, and
//! `overrideActionButtonIdForRide=-1` (no redirect to a different row while
//! mounted, so editing this row directly is enough - no other row to chase).
//!
//! Unlike [`EldenConvenienceMod`](../../.docs/EldenConvenienceMod) (a
//! reference-only, non-Rust tool in this repo), which had to clone row 6100
//! into a brand new action ID and hand-write a whole new EMEVD event for it
//! (their new ID had nothing listening for it yet), this edits the vanilla
//! row in place - the game's existing event logic already listens for action
//! ID 6100, so no event-script authoring is needed at all, just 2 param
//! bits.
//!
//! One-shot, same as `risearcher`: regulation.bin param rows load once at
//! startup and are never touched again by the game afterward, so there's no
//! reason to watch `General.ReloadKey` here.

use std::time::Duration;

use eldenring::cs::{ActionButtonParam, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

// "Touch grace" - the action button players interact with to sit at any
// Site of Grace. Same ID across the whole game (see the exported CSV);
// `overrideActionButtonIdForRide=-1` on this row confirms no other row
// takes over while mounted.
const TOUCH_GRACE_ACTION_ID: u32 = 6100;

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

/// Waits for `SoloParamRepository`, then (if `Misc.GraceOnTorrent` is
/// enabled) clears row 6100's ride-restriction bits once. Meant to run on
/// its own worker thread spawned from `DllMain`; returns once done (no
/// per-tick or hotkey-watching loop for this module).
pub fn run() {
    if !config::get_bool("Misc.GraceOnTorrent", true) {
        return;
    }

    let Some(repo) = wait_for_repository(Duration::from_secs(60)) else {
        logger::log("ERROR: SoloParamRepository never became available - Misc.GraceOnTorrent disabled for this session.");
        return;
    };

    let Some(row) = repo.get_mut::<ActionButtonParam>(TOUCH_GRACE_ACTION_ID) else {
        logger::log(&format!(
            "Misc.GraceOnTorrent: ERROR - ActionButtonParam row {TOUCH_GRACE_ACTION_ID} (Touch grace) not found."
        ));
        return;
    };
    row.set_is_invalid_for_ride(false);
    row.set_is_grayout_for_ride(false);

    logger::log("Misc.GraceOnTorrent: enabled - Sites of Grace can now be used while mounted on Torrent.");
}
