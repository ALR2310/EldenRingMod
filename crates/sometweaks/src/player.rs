//! Pad-input reading specific to this crate's own features - the "is the
//! player actually in the game world" gate (`main_player_chr_ins_ptr`/
//! `wait_for_solo_param_repository`) that used to live here too has moved to
//! [`common::player`] (2026-09-17, once `dropmultiplier` needed an identical
//! copy for the third time) - every call site in this crate now goes
//! straight to `common::player::*` instead.

use eldenring::cs::WorldChrMan;
use fromsoftware_shared::FromStatic;

/// The newly-pressed-this-frame R1/R2/L1/L2 pad-input bits, read from the
/// main player's `CSChrActionRequestModule.new_action_presses` - used by
/// `regen`'s `Regen.PerHit.ExcludeAow` to tell an Ash of War hit from a
/// plain attack (confirmed in-game more reliable than any `AtkParam` field,
/// see AutoRegen's README, 2026-09-03: Elden Ring always triggers Skill via
/// L2 regardless of which hand is active, R1/R2/L1 never do).
pub struct NewActionPresses {
    pub r1: bool,
    pub r2: bool,
    pub l1: bool,
    pub l2: bool,
}

/// Reads the main player's `new_action_presses` bits for this frame. `None`
/// if not resolved yet.
pub fn main_player_new_action_presses() -> Option<NewActionPresses> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    let main_player = world_chr_man.main_player.as_ref()?;
    let presses = &main_player.chr_ins.modules.action_request.new_action_presses;
    Some(NewActionPresses {
        r1: presses.r1(),
        r2: presses.r2(),
        l1: presses.l1(),
        l2: presses.l2(),
    })
}
