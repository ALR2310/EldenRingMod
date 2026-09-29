//! Debug-only (`SlotProbe`, off by default) diagnostics that located the
//! 10-spirit cap (see `band.rs`) - no gameplay
//! change at all, only logging.
//!
//! `sometweaks`'s `spirit::summon_count` (and `.docs/x10_summon/er10x.dll`
//! before it) can lengthen an Ash's SpEffect -> BuddyParam chain freely, but
//! the engine only ever spawns 10: per `er10x.ini`, each player gets a fixed
//! "band" of 10 slots inside `WorldChrMan.summon_buddy_chr_set`, whose start
//! and length are engine constants. Before deciding between widening that
//! band inside the ChrSet's existing capacity or reallocating the ChrSet's
//! `entries` array outright, we need two facts this module logs:
//!
//! 1. The ChrSet's real capacity (both the `capacity` field and the vtable's
//!    own `get_capacity()`, in case they disagree) - logged once, the first
//!    time the player is in the world.
//! 2. Which slot indices summoned spirits (and Torrent) actually land in -
//!    logged every time the occupied-slot layout changes, so summoning a
//!    multi-creature Ash shows where the local player's band starts.
//!
//! Also logs the SpEffect -> BuddyParam chain-length histogram once (how
//! many vanilla Ashes summon 1, 2, 3... creatures), plus a few
//! `SummonBuddyManager` fields that look slot-related (`last_buddy_slot`,
//! `active_summon_speffect_id`) alongside each layout change.

use std::collections::BTreeMap;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

const TICK_INTERVAL_MS: f64 = 500.0;

/// One occupied slot: (index, npc_param_id, load status).
type SlotInfo = (u32, i32, String);

pub fn run() {
    if !config::get_bool("SlotProbe", false) {
        logger::log("SlotProbe=false - probe disabled.");
        return;
    }

    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    let mut logged_static_info = false;
    let mut last_layout: Option<Vec<SlotInfo>> = None;

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "SlotProbe",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            // Same "actually in the world" gate as sometweaks' summon_count:
            // `summon_buddy_manager`'s storage isn't populated before this.
            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }
            let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
                return;
            };

            let chr_set = &world_chr_man.summon_buddy_chr_set;
            let manager = &world_chr_man.summon_buddy_manager;

            if !logged_static_info {
                logged_static_info = true;
                logger::log(&format!(
                    "summon_buddy_chr_set: index={} capacity(field)={} capacity(vtable)={}",
                    chr_set.index,
                    chr_set.capacity,
                    chr_set.get_capacity(),
                ));

                let mut histogram: BTreeMap<usize, usize> = BTreeMap::new();
                for (_, head) in manager.trigger_speffect_to_buddy_map.iter_chains() {
                    *histogram.entry(head.chain_len()).or_default() += 1;
                }
                logger::log(&format!(
                    "trigger_speffect_to_buddy_map: {} chain(s), length histogram (len: count) = {:?}",
                    histogram.values().sum::<usize>(),
                    histogram,
                ));
            }

            let mut layout: Vec<SlotInfo> = Vec::new();
            for i in 0..chr_set.capacity {
                let entry = unsafe { chr_set.entries.add(i as usize).as_ref() };
                if let Some(chr_ins) = entry.chr_ins {
                    let npc_param_id = unsafe { chr_ins.as_ref() }.npc_param_id;
                    layout.push((i, npc_param_id, format!("{:?}", entry.chr_load_status)));
                }
            }

            if last_layout.as_ref() != Some(&layout) {
                let slots = layout
                    .iter()
                    .map(|(i, npc, status)| format!("#{i}(npc={npc},{status})"))
                    .collect::<Vec<_>>()
                    .join(" ");
                logger::log(&format!(
                    "slots changed: {} occupied [{}] | active_summon_speffect_id={} last_buddy_slot={} player_has_alive_summon={}",
                    layout.len(),
                    slots,
                    manager.active_summon_speffect_id,
                    manager.last_buddy_slot,
                    manager.player_has_alive_summon,
                ));
                last_layout = Some(layout);
            }
        },
    );

    logger::log("SlotProbe: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
