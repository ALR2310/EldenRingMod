//! Removes the ghostly tint from summoned spirit ashes (and Torrent, which
//! shares the same `summon_buddy_chr_set`, but is never given the tint
//! SpEffect so this never touches it), ported from `.docs/x10_summon/
//! er10x.dll`'s `ghost_colour` option - but using a live per-tick SpEffect
//! removal instead of that mod's static `NpcParam` row edit:
//! `er10x.dll` has to hunt through up to 194 param tables at startup to
//! find the right `NpcParam` and clear a fixed slot on every summon-able
//! NPC row (only 106 of 129 candidate rows are real ashes, so it has to be
//! careful not to clobber the rest) - here, `fromsoftware-rs` already
//! exposes the currently-summoned characters directly
//! (`WorldChrMan::summon_buddy_chr_set`) and their live SpEffect list
//! (`ChrIns::special_effect`), so this just checks each active summon's
//! own SpEffects for the tint's ID range and removes it - no param table
//! search needed, and nothing is touched on NPCs that were never
//! actually summoned.
//!
//! The tint IDs (295000-295999) come from `er10x.ini`'s own documented
//! range for this game build.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, ChrInsExt, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

const GHOST_SPEFFECT_MIN: i32 = 295000;
const GHOST_SPEFFECT_MAX: i32 = 295999;

/// Removes any active SpEffect on `chr_ins` whose ID falls in the ghost
/// tint range. Collects matching IDs first since [ChrInsExt::remove_speffect]
/// needs `&mut ChrIns` while `special_effect.entries()` borrows it
/// immutably.
fn strip_ghost_tint(chr_ins: &mut eldenring::cs::ChrIns) {
    let tint_ids: Vec<i32> = chr_ins
        .special_effect
        .entries()
        .map(|entry| entry.param_id)
        .filter(|id| (GHOST_SPEFFECT_MIN..=GHOST_SPEFFECT_MAX).contains(id))
        .collect();
    for id in tint_ids {
        chr_ins.remove_speffect(id);
    }
}

/// Watches every currently-summoned spirit ash on the game's own
/// `FrameBegin` task group for the rest of the DLL's lifetime, stripping
/// the ghostly tint SpEffect the moment one appears. Meant to run on its
/// own worker thread spawned from `DllMain`; never returns (except early,
/// if `Spirit.Color=false` at startup).
pub fn run() {
    if !config::get_bool("Spirit.Enabled", true) || !config::get_bool("Spirit.Color", false) {
        logger::log("Spirit.Color=false (or Spirit.Enabled=false) - skipping entirely at startup.");
        return;
    }

    let cs_task = crate::task::wait_for_cs_task();

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "Spirit.Color",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| {
            if !config::get_bool("Spirit.Enabled", true) || !config::get_bool("Spirit.Color", false) {
                return;
            }
            let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
                return;
            };
            for chr_ins in world_chr_man.summon_buddy_chr_set.characters() {
                strip_ghost_tint(chr_ins);
            }
        },
    );

    logger::log("Spirit.Color: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
