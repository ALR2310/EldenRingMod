//! Debug-only (`EnemyProbe`, off by default) diagnostics for "enemies near
//! the player vanish when ~60 spirits are summoned" (reported at Stormgate,
//! 2026-09-26) - no gameplay change, only logging.
//!
//! Candidates, from `fromsoftware-rs`'s reflected types:
//! - Omission (update LOD): `ChrIns::omission_mode` (Normal / 30 / 20 / 5 /
//!   1 FPS / NoUpdate), handed out per frame from
//!   `WorldChrMan::omission_update_budget_near/far`, whose size depends on
//!   `WorldChrManDbg::omission_update_num_type` (Normal / Overload /
//!   Emergency, "changed depending on current performance load").
//! - Activation: `ChrIns::chr_activate_threshold` compared against
//!   `CSOpenChrActivateThresholdRegionMan` (open-field characters), result
//!   in `chr_flags1ca.activate_threshold_exceeded`; plus
//!   `chr_activation_flags.activation_enabled`.
//! - Unloading: `ChrSetEntry::chr_load_status`, or the character simply
//!   disappearing from its ChrSet.
//!
//! - Activation COUNT limit (found in IDA 2026-09-26, ER 2.7.1.0):
//!   `sub_14050F9E0` sorts open-field activation candidates and only
//!   activates `limit - already_active` of them (the rest get
//!   `sub_1403E93A0(chr, 2)` = cut, `load_state.evaluation_value_cutoff`);
//!   `limit` is 60 (hard-coded in the singleton ctor `sub_145AE6B75`,
//!   `+0xEC`/`+0xF0`; 40 in an alternate mode) and `already_active` is
//!   the counter at `WorldChrMan+0x1E618` (124440), bumped for every
//!   always-active character - logged here as `active_count`.
//!
//! Every tick this collects every non-summon character within
//! [RADIUS_M] of the player across all of `WorldChrMan.chr_sets`, and logs
//! a summary line (plus the per-character list) whenever the summary
//! changes, together with the live summon count - so summoning at a spot
//! with enemies shows which of the above actually happens to them.

use std::collections::BTreeMap;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, ChrIns, ChrLoadStatus, WorldChrMan, WorldChrManDbg};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

const TICK_INTERVAL_MS: f64 = 1000.0;
const RADIUS_M: f32 = 100.0;

struct Seen {
    set_index: i32,
    slot: usize,
    npc_param_id: i32,
    dist: f32,
    load: String,
    omission: String,
    threshold_exceeded: bool,
    activation_enabled: bool,
    cutoff: bool,
}

fn describe(chr: &ChrIns, set_index: i32, slot: usize, load: String) -> Seen {
    Seen {
        set_index,
        slot,
        npc_param_id: chr.npc_param_id,
        dist: chr.distance_to_player_sqr.max(0.0).sqrt(),
        load,
        omission: format!("{:?}", chr.omission_mode),
        threshold_exceeded: chr.chr_flags1ca.activate_threshold_exceeded(),
        activation_enabled: chr.chr_activation_flags.activation_enabled(),
        cutoff: chr.load_state.evaluation_value_cutoff(),
    }
}

fn count<'a>(items: impl Iterator<Item = &'a str>) -> BTreeMap<&'a str, usize> {
    let mut map = BTreeMap::new();
    for item in items {
        *map.entry(item).or_default() += 1;
    }
    map
}

pub fn run() {
    if !config::get_bool("EnemyProbe", false) {
        return;
    }

    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    let mut last_summary = String::new();

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "EnemyProbe",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }
            let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
                return;
            };

            let summon_set_index = world_chr_man.summon_buddy_chr_set.index;
            let summons = world_chr_man.summon_buddy_chr_set.characters().count();
            let radius_sqr = RADIUS_M * RADIUS_M;

            let mut seen: Vec<Seen> = Vec::new();
            // Non-live entries, counted by status only: their ChrIns pointer
            // can be stale (freed while loading/unloading) - dereferencing
            // one crashed the game on world entry (2026-09-26), so only
            // Active / ReadyForActivation characters are ever read.
            let mut not_live: BTreeMap<String, usize> = BTreeMap::new();
            for chr_set in world_chr_man.chr_sets.iter().flatten() {
                if chr_set.index == summon_set_index {
                    continue;
                }
                for slot in 0..chr_set.capacity as usize {
                    let entry = unsafe { chr_set.entries.add(slot).as_ref() };
                    let Some(chr) = entry.chr_ins else { continue };
                    if !matches!(
                        entry.chr_load_status,
                        ChrLoadStatus::Active | ChrLoadStatus::ReadyForActivation
                    ) {
                        *not_live.entry(format!("{:?}", entry.chr_load_status)).or_default() += 1;
                        continue;
                    }
                    let chr = unsafe { chr.as_ref() };
                    if chr.distance_to_player_sqr > radius_sqr {
                        continue;
                    }
                    seen.push(describe(chr, chr_set.index, slot, format!("{:?}", entry.chr_load_status)));
                }
            }
            seen.sort_by(|a, b| a.dist.total_cmp(&b.dist));

            let (budget_type, near_budget, far_budget) = match unsafe { WorldChrManDbg::instance() } {
                Ok(dbg) => {
                    let ty = unsafe { *(&dbg.omission_update_num_type as *const _ as *const i32) };
                    (ty, world_chr_man.omission_update_budget_near, world_chr_man.omission_update_budget_far)
                }
                Err(_) => (-99, world_chr_man.omission_update_budget_near, world_chr_man.omission_update_budget_far),
            };

            // WorldChrMan+0x1E618: per-frame count of already-active
            // characters that sub_14050F9E0 subtracts from its limit of 60.
            let active_count = unsafe {
                *((world_chr_man as *const WorldChrMan as *const u8).add(0x1E618) as *const u32)
            };

            let summary = format!(
                "summons={summons} active_count={active_count} nearby={} cutoff={} load={:?} omission={:?} threshold_exceeded={} activation_disabled={} | omission_type={budget_type} (0=Normal,1=Overload,2=Emergency)",
                seen.len(),
                seen.iter().filter(|s| s.cutoff).count(),
                count(seen.iter().map(|s| s.load.as_str())),
                count(seen.iter().map(|s| s.omission.as_str())),
                seen.iter().filter(|s| s.threshold_exceeded).count(),
                seen.iter().filter(|s| !s.activation_enabled).count(),
            );
            if summary == last_summary {
                return;
            }

            let list = seen
                .iter()
                .map(|s| {
                    format!(
                        "{}/{}:npc{}@{:.0}m,{},{}{}{}{}",
                        s.set_index,
                        s.slot,
                        s.npc_param_id,
                        s.dist,
                        s.load,
                        s.omission,
                        if s.threshold_exceeded { ",EXCEEDED" } else { "" },
                        if s.activation_enabled { "" } else { ",NOACT" },
                        if s.cutoff { ",CUTOFF" } else { "" },
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            logger::log(&format!(
                "EnemyProbe: {summary} | not_live(all sets)={not_live:?} | budget near={near_budget} far={far_budget} | {list}"
            ));
            last_summary = summary;
        },
    );

    logger::log("EnemyProbe: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
