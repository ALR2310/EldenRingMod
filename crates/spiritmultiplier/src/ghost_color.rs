//! `GhostColor` (default `true` = vanilla): `false` removes the ghostly
//! tint every summoned spirit ash carries, so spirits look like ordinary
//! characters.
//!
//! The tint is a resident SpEffect listed in the spirit's own NpcParam row:
//! `NpcParam.spEffectID26` (offset 0x210, same slot `er10x.dll` clears via
//! its `ghost_slot = 528`) holds 295000 or 295200 ("[Spirit Summon] Color"
//! in Smithbox - verified by the user on NpcParam 140700000, Lone Wolf,
//! 2026-09-27). The game applies it when the spirit spawns, so editing the
//! param in memory is enough - no per-frame work, and spirits spawn
//! untinted instead of losing the tint a frame late.
//!
//! First version (same day) copied `sometweaks`'s `spirit/color.rs`
//! instead: strip the SpEffect from live spirits every frame. Replaced at
//! the user's suggestion (the tint is just a param).
//!
//! Which rows: only NpcParam rows some `BuddyParam` row spawns
//! (`npcParamId` / `npcParamId_ridden`) - `er10x.ini` counts 129 NpcParam
//! rows carrying a 295xxx effect but only 106 are spirit ashes - and only
//! when slot 26 holds a value in [GHOST_SPEFFECT_MIN, GHOST_SPEFFECT_MAX]
//! (`er10x.ini`'s range; anything else a param mod put there is left
//! alone). Rows are addressed by row index, with IDs read through
//! `common::params::row_ids` - never through the runtime lookup table
//! (`get_mut` / `rows_mut`), which can misbehave on modded regulations.
//!
//! Original values are captured once, before the first write, and every
//! later change is applied from that snapshot, so F5 works both ways
//! (spirits summoned after the change get the new look; spirits already out
//! keep theirs until re-summoned).

use std::collections::HashSet;
use std::time::Duration;

use eldenring::cs::{BuddyParam, CSTaskGroupIndex, NpcParam, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

const GHOST_SPEFFECT_MIN: i32 = 295000;
const GHOST_SPEFFECT_MAX: i32 = 295999;
const REMOVED: i32 = -1;

const TICK_INTERVAL_MS: f64 = 1000.0;

/// `(NpcParam row index, original spEffectID26)` for every tinted spirit
/// row, or `None` if the params couldn't be read safely.
fn capture(repo: &mut SoloParamRepository) -> Option<Vec<(usize, i32)>> {
    for check in [common::params::check::<BuddyParam>(repo), common::params::check::<NpcParam>(repo)] {
        if let Err(err) = check {
            logger::error(&format!("GhostColor: {err} - not touching params."));
            return None;
        }
    }

    let mut spirit_npc_ids: HashSet<u32> = HashSet::new();
    common::params::for_each_row_mut::<BuddyParam>(repo, |_, row| {
        for id in [row.npc_param_id(), row.npc_param_id_ridden()] {
            if id > 0 {
                spirit_npc_ids.insert(id as u32);
            }
        }
    });

    let Some(npc_ids) = common::params::row_ids::<NpcParam>(repo) else {
        logger::error("GhostColor: NpcParam row IDs could not be read safely - not touching params.");
        return None;
    };

    let mut tinted = Vec::new();
    common::params::for_each_row_mut::<NpcParam>(repo, |index, row| {
        let tint = row.sp_effect_id26();
        if spirit_npc_ids.contains(&npc_ids[index]) && (GHOST_SPEFFECT_MIN..=GHOST_SPEFFECT_MAX).contains(&tint) {
            tinted.push((index, tint));
        }
    });
    logger::log(&format!(
        "GhostColor: {} spirit NpcParam row(s) from {} BuddyParam NPC id(s) carry the ghost tint in spEffectID26.",
        tinted.len(),
        spirit_npc_ids.len()
    ));
    Some(tinted)
}

pub fn run() {
    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    // None = not captured yet; Some(None) = capture failed, stay off.
    let mut tinted: Option<Option<Vec<(usize, i32)>>> = None;
    let mut applied: Option<bool> = None;

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "GhostColor",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            // Param reads before the player is in the world can panic (see
            // `common::player`'s doc comment) - same gate as every other
            // param-touching feature.
            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }
            let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
                return;
            };

            let ghost = config::get_bool("GhostColor", true);
            let Some(rows) = tinted.get_or_insert_with(|| capture(repo)) else {
                return;
            };
            if applied == Some(ghost) {
                return;
            }

            for &(index, original) in rows.iter() {
                if let Some(row) = repo.get_row_by_index_mut::<NpcParam>(index) {
                    row.set_sp_effect_id26(if ghost { original } else { REMOVED });
                }
            }
            logger::log(&format!(
                "GhostColor={ghost}: ghost tint {} on {} spirit row(s).",
                if ghost { "restored" } else { "removed" },
                rows.len()
            ));
            applied = Some(ghost);
        },
    );

    logger::log("GhostColor: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
