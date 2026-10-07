//! Skips the pause after "YOU DIED" appears: sets
//! `MenuCommonParam[0].soloPlayDeath_ToFadeOutTime` (paramdef/Smithbox name,
//! "[YOU DIED] Fade Out Duration"; fromsoftware-rs accessor
//! `solo_play_death_to_fade_out_time`, vanilla 3.8) to 0, once.
//!
//! Same effect as ImAxel0's FasterRespawn (MIT,
//! https://github.com/ImAxel0/EldenRing-FasterRespawn-Mod), which patches
//! `jz` -> `jmp` (AOB `74 ?? F3 0F 10 19`) in `sub_1405A7E00` (2.7.1.0), a
//! step of `CSDeathRestartEvent::Start`: that jump decides whether the
//! step's wait time (4th arg of `sub_1405942D0`, stage 4) comes from
//! `sub_140D2FE90` - a pointer to this exact param field (param slot 143,
//! row ID 0, first field) - or is `0.0`. Writing the param instead needs no
//! code patch. `partyGhostDeath_ToFadeOutTime` (co-op, vanilla 3.3) is left alone, like
//! the original.

use eldenring::cs::{MenuCommonParam, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use common::logger;

#[derive(Default)]
pub struct FadeState {
    done: bool,
}

/// Writes the param once. Must only be called while the main player
/// exists - `SoloParamRepository` reads before that can panic (see
/// `common::player`).
pub fn apply(state: &mut FadeState) {
    if state.done {
        return;
    }
    let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else { return };
    // Whatever happens below, don't retry every frame.
    state.done = true;
    if let Err(reason) = common::params::check::<MenuCommonParam>(repo) {
        logger::error(&format!("YOU DIED fade: {reason} - disabled."));
        return;
    }
    let Some(row) = repo.get_row_by_index_mut::<MenuCommonParam>(0) else {
        logger::error("YOU DIED fade: MenuCommonParam has no row - disabled.");
        return;
    };
    let vanilla = row.solo_play_death_to_fade_out_time();
    row.set_solo_play_death_to_fade_out_time(0.0);
    logger::log(&format!("YOU DIED fade delay: 0s (was {vanilla}s)."));
}
