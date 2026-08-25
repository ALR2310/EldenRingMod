//! Shared "is the player actually in the game world" resolution. Every
//! feature that touches a singleton whose data isn't populated until a real
//! player character exists (`GameDataMan`'s save data, `SoloParamRepository`'s
//! param tables) must gate on this, not just on the singleton's own
//! `instance()`/`instance_mut()` returning `Ok` - that alone only means the
//! manager object exists, which happens as soon as a save slot loads, still
//! at the title/loading screen well before its data is actually populated
//! (confirmed the hard way: premature rune grants in `rune::reward`, and a
//! crash in `drop_rate` before `SoloParamRepository` was gated the same way,
//! 2026-08-24/25).

use std::time::Duration;

use eldenring::cs::{SoloParamRepository, WorldChrMan};
use fromsoftware_shared::FromStatic;

/// Returns the main player's `ChrIns` address if currently resolved, `None`
/// otherwise (title screen, loading, no save loaded, ...).
pub fn main_player_chr_ins_ptr() -> Option<*const u8> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    world_chr_man
        .main_player
        .as_ref()
        .map(|p| &p.chr_ins as *const _ as *const u8)
}

/// Waits (up to `timeout`) for `SoloParamRepository` - the live in-memory
/// regulation.bin - AND for the player to actually be in the game world
/// before returning. `SoloParamRepository::instance_mut()` alone can return
/// `Ok` as soon as the manager object exists (title/loading screen, well
/// before its param tables are actually populated) - same
/// premature-ready-singleton class [main_player_chr_ins_ptr] itself exists
/// to guard against. Ported from `drop_rate`'s own helper of the same name
/// (2026-08-24) once a second caller ([crate::misc::unlock_ashes_of_war])
/// needed it too (2026-08-25).
pub fn wait_for_solo_param_repository(timeout: Duration) -> Option<&'static mut SoloParamRepository> {
    let step = Duration::from_millis(200);
    let mut waited = Duration::ZERO;
    loop {
        if main_player_chr_ins_ptr().is_some() {
            if let Ok(repo) = unsafe { SoloParamRepository::instance_mut() } {
                return Some(repo);
            }
        }
        if waited >= timeout {
            return None;
        }
        std::thread::sleep(step);
        waited += step;
    }
}
