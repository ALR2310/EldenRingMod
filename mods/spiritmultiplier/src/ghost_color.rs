//! `GhostColor` (default `true` = vanilla): `false` removes the ghostly
//! tint every summoned spirit ash carries, so spirits look like ordinary
//! characters.
//!
//! What the tint is: a SpEffect on the spirit whose visual (`vfxId` ..
//! `vfxId7` -> SpEffectVfxParam) forces a PhantomParam onto the model
//! (`phantomParamOverwriteType` != 0, `phantomParamOverwriteId` = the
//! colour). Vanilla: NpcParam `spEffectID26` = 295000 "[Spirit Summon]
//! Color" -> vfx 57000 -> PhantomParam 200 (Puppet: 295200 -> 57100 ->
//! 201). ELDEN RING Reforged 2.3.5.3 (checked on its param exports,
//! 2026-09-30): no 295000; each Ash's own balance SpEffect in
//! `spEffectID4` (200100, 201100, ... 292100) carries vfx 70000 / 70010 /
//! 70020 -> PhantomParam 2000 / 2010 / 420 - 121 of its 125 spirits, and
//! those vfx rows are used by no other NpcParam row.
//!
//! So this edits the vfx, not the SpEffect: `phantomParamOverwriteType`
//! is set to 0 (no overwrite) on every SpEffectVfx row reached from a
//! spirit, keeping the SpEffect itself - in Reforged it also carries the
//! Ash's stat adjustments and effect chains, which removing it would lose.
//!
//! Which rows: spirits = NpcParam rows some `BuddyParam` row spawns
//! (`npcParamId` / `npcParamId_ridden`, not row names - a param mod's
//! names can be wrong), all 32 `spEffectID` slots, every `vfxId` of those
//! SpEffects. IDs come from `common::params::row_ids`, rows are addressed
//! by index - never through the runtime lookup table (`get_mut` /
//! `rows_mut`), which can misbehave on modded regulations.
//!
//! History: first version (2026-09-27) copied `sometweaks`'s
//! `spirit/color.rs` (strip the SpEffect from live spirits every frame);
//! then (same day) cleared `spEffectID26` when it held 295000-295999 -
//! which found nothing in Reforged (Nexus report, 2026-09-30).
//!
//! Original values are captured once, before the first write, and every
//! later change is applied from that snapshot, so F5 works both ways.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use eldenring::cs::{BuddyParam, CSTaskGroupIndex, NpcParam, SoloParamRepository, SpEffectParam, SpEffectVfxParam};
use eldenring::param::{NPC_PARAM_ST, SP_EFFECT_PARAM_ST};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

/// "No PhantomParam overwrite".
const NO_OVERWRITE: u8 = 0;

const TICK_INTERVAL_MS: f64 = 1000.0;

fn sp_effect_slots(row: &NPC_PARAM_ST) -> [i32; 32] {
    [
        row.sp_effect_id0(),
        row.sp_effect_id1(),
        row.sp_effect_id2(),
        row.sp_effect_id3(),
        row.sp_effect_id4(),
        row.sp_effect_id5(),
        row.sp_effect_id6(),
        row.sp_effect_id7(),
        row.sp_effect_id8(),
        row.sp_effect_id9(),
        row.sp_effect_id10(),
        row.sp_effect_id11(),
        row.sp_effect_id12(),
        row.sp_effect_id13(),
        row.sp_effect_id14(),
        row.sp_effect_id15(),
        row.sp_effect_id16(),
        row.sp_effect_id17(),
        row.sp_effect_id18(),
        row.sp_effect_id19(),
        row.sp_effect_id20(),
        row.sp_effect_id21(),
        row.sp_effect_id22(),
        row.sp_effect_id23(),
        row.sp_effect_id24(),
        row.sp_effect_id25(),
        row.sp_effect_id26(),
        row.sp_effect_id27(),
        row.sp_effect_id28(),
        row.sp_effect_id29(),
        row.sp_effect_id30(),
        row.sp_effect_id31(),
    ]
}

fn vfx_ids(row: &SP_EFFECT_PARAM_ST) -> [i32; 8] {
    [
        row.vfx_id(),
        row.vfx_id1(),
        row.vfx_id2(),
        row.vfx_id3(),
        row.vfx_id4(),
        row.vfx_id5(),
        row.vfx_id6(),
        row.vfx_id7(),
    ]
}

/// ID -> row index for `P`, or `None` if the IDs couldn't be read safely.
fn index_by_id<P: eldenring::cs::SoloParam>(repo: &SoloParamRepository) -> Option<HashMap<u32, usize>> {
    let ids = common::params::row_ids::<P>(repo)?;
    Some(ids.into_iter().enumerate().map(|(index, id)| (id, index)).collect())
}

/// `(SpEffectVfxParam row index, original phantomParamOverwriteType)` for
/// every tinting vfx reached from a spirit, or `None` if the params
/// couldn't be read safely.
fn capture(repo: &mut SoloParamRepository) -> Option<Vec<(usize, u8)>> {
    for check in [
        common::params::check::<BuddyParam>(repo),
        common::params::check::<NpcParam>(repo),
        common::params::check::<SpEffectParam>(repo),
        common::params::check::<SpEffectVfxParam>(repo),
    ] {
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

    let (Some(npc_ids), Some(sp_effect_index), Some(vfx_index)) = (
        common::params::row_ids::<NpcParam>(repo),
        index_by_id::<SpEffectParam>(repo),
        index_by_id::<SpEffectVfxParam>(repo),
    ) else {
        logger::error("GhostColor: param row IDs could not be read safely - not touching params.");
        return None;
    };

    // Every NpcParam row's SpEffects, flagged spirit / not.
    let mut npc_rows: Vec<(bool, [i32; 32])> = Vec::new();
    common::params::for_each_row_mut::<NpcParam>(repo, |index, row| {
        npc_rows.push((spirit_npc_ids.contains(&npc_ids[index]), sp_effect_slots(row)));
    });

    // Per vfx: how many spirit / other NpcParam rows reach it.
    let mut vfx_of_sp_effect: HashMap<i32, Vec<u32>> = HashMap::new();
    let mut uses: HashMap<u32, (u32, u32)> = HashMap::new();
    let mut spirit_rows = 0;
    for (is_spirit, slots) in &npc_rows {
        spirit_rows += *is_spirit as u32;
        let mut row_vfx: HashSet<u32> = HashSet::new();
        for id in slots.iter().copied().filter(|&id| id > 0) {
            let vfx = vfx_of_sp_effect.entry(id).or_insert_with(|| {
                sp_effect_index
                    .get(&(id as u32))
                    .and_then(|&i| repo.get_row_by_index::<SpEffectParam>(i))
                    .map(|row| vfx_ids(row).into_iter().filter(|&v| v > 0).map(|v| v as u32).collect())
                    .unwrap_or_default()
            });
            row_vfx.extend(vfx.iter().copied());
        }
        for v in row_vfx {
            let count = uses.entry(v).or_default();
            if *is_spirit {
                count.0 += 1;
            } else {
                count.1 += 1;
            }
        }
    }

    // A tint vfx is the summon ghost colour if spirits carry it more than
    // other NPCs do (vanilla 57000: ~106 spirit rows vs ~23 unused/NPC-summon
    // rows); one that mostly other NPCs carry is a character's own look -
    // vanilla 54183 (SpEffect 14495, PhantomParam 240) is the Mausoleum
    // Knight's, also on 1 spirit. Clearing that would strip the enemies too.
    let mut tinting: Vec<(usize, u8)> = Vec::new();
    let mut kept: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut ids: Vec<(&u32, &(u32, u32))> = uses.iter().filter(|(_, c)| c.0 > 0).collect();
    ids.sort_unstable();
    for (&id, &(spirits, others)) in ids {
        let Some(&index) = vfx_index.get(&id) else {
            continue;
        };
        let Some(row) = repo.get_row_by_index::<SpEffectVfxParam>(index) else {
            continue;
        };
        let kind = row.phantom_param_overwrite_type();
        if kind == NO_OVERWRITE {
            continue;
        }
        let entry = format!("{id} ({spirits} spirit / {others} other)");
        if spirits > others {
            tinting.push((index, kind));
            kept.push(entry);
        } else {
            skipped.push(entry);
        }
    }
    logger::log(&format!(
        "GhostColor: {spirit_rows} spirit NpcParam row(s) from {} BuddyParam NPC id(s); tint vfx {kept:?}; left alone (mostly non-spirit) {skipped:?}.",
        spirit_npc_ids.len()
    ));
    Some(tinting)
}

pub fn run() {
    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    // None = not captured yet; Some(None) = capture failed, stay off.
    let mut tinting: Option<Option<Vec<(usize, u8)>>> = None;
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
            let Some(rows) = tinting.get_or_insert_with(|| capture(repo)) else {
                return;
            };
            if applied == Some(ghost) {
                return;
            }

            for &(index, original) in rows.iter() {
                if let Some(row) = repo.get_row_by_index_mut::<SpEffectVfxParam>(index) {
                    row.set_phantom_param_overwrite_type(if ghost { original } else { NO_OVERWRITE });
                }
            }
            logger::log(&format!(
                "GhostColor={ghost}: ghost tint {} on {} vfx row(s).",
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
