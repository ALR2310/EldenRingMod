//! Waiting for "the player is actually in the game world", needed because
//! `SoloParamRepository::instance_mut()` alone can return `Ok` as soon as the
//! manager object exists - title/loading screen, well before its param
//! tables (`EquipParamWeapon`, `Bullet`, ...) are actually populated.
//! Confirmed the hard way in this exact crate (2026-09-08): RiseArcher's
//! first in-game test silently lost its own worker thread to a panic deep
//! inside `fromsoftware-rs` (`SoloParamRepository::get_param_file_mut`'s
//! "Expected param holder to have exactly one res cap" `.expect()`), with no
//! crash and no log line - a plain `std::thread` panic under this
//! workspace's panic=unwind profile just kills that one thread quietly.
//! Ported from `sometweaks::player`'s helper of the same name, which exists
//! for the identical reason (see its own doc comment: a crash in
//! `sometweaks::drop_rate` before it was gated this way, 2026-08-24/25).

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
/// before returning.
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
