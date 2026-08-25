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

use eldenring::cs::WorldChrMan;
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
