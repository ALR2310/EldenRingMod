//! Lengthens every Ash's SpEffect -> BuddyParam chain so one cast asks for
//! more spirits - the other half of this mod, next to `band.rs` (which
//! makes room for them).
//!
//! Same technique as `sometweaks`'s `spirit/summon_count.rs` (itself
//! ported from `.docs/x10_summon/er10x.dll`'s "chain rebuild"), kept as a
//! separate copy here rather than moved to `common` at the user's request
//! (2026-09-25). The only behavioral difference is the cap:
//! [crate::band::band_len] (`MaxSpirits`) instead of 10. See `summon_count.rs`'s module
//! doc comment for the full reasoning; in short:
//!
//! - `WorldChrMan.summon_buddy_manager.trigger_speffect_to_buddy_map` maps
//!   an Ash's summon SpEffect to a linked chain of BuddyParam IDs, one per
//!   creature. The game's own head node is left untouched; only its `next`
//!   is redirected to a `Box::leak`'d extension that cycles through the
//!   original IDs (never freed - the game may still hold the old chain).
//! - Original chains are snapshotted once per SpEffect ID and every rebuild
//!   is computed from that snapshot, never from the (possibly already
//!   rebuilt) current chain - otherwise `Multiplier` compounds every tick
//!   (the bug `summon_count.rs` hit on 2026-08-26).
//! - Re-checked on a periodic tick (cheap and idempotent), which also
//!   picks up ini changes after a hot reload.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use eldenring::ChainingMapBucketEntry;
use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

const TICK_INTERVAL_MS: f64 = 1000.0;

static ORIGINAL_CHAIN_IDS: Mutex<Option<HashMap<i32, Vec<i32>>>> = Mutex::new(None);

enum Mode {
    /// `Amount` - every Ash summons exactly this many.
    Fixed(usize),
    /// `Multiplier` - every Ash summons its vanilla count times this.
    Multiplier(f64),
}

fn read_mode() -> Option<Mode> {
    let amount = config::get_int("Amount", 0);
    if amount > 0 {
        return Some(Mode::Fixed(amount as usize));
    }
    let multiplier = config::get_double("Multiplier", 1.0);
    if multiplier > 0.0 && (multiplier - 1.0).abs() > f64::EPSILON {
        return Some(Mode::Multiplier(multiplier));
    }
    None
}

fn target_count(mode: &Mode, original_len: usize) -> usize {
    let raw = match mode {
        Mode::Fixed(amount) => *amount,
        Mode::Multiplier(factor) => ((original_len as f64) * factor).round() as usize,
    };
    raw.clamp(1, crate::band::band_len() as usize)
}

/// Builds a `target_len - 1` node extension cycling through `original_ids`
/// (index 0 is the game's own head node, left untouched).
fn build_extension(original_ids: &[i32], target_len: usize) -> Option<*mut ChainingMapBucketEntry<i32>> {
    if target_len <= 1 {
        return None;
    }
    let nodes: Vec<ChainingMapBucketEntry<i32>> = (1..target_len)
        .map(|i| ChainingMapBucketEntry {
            data: original_ids[i % original_ids.len()],
            next: None,
        })
        .collect();
    let leaked = Box::leak(nodes.into_boxed_slice());
    let base = leaked.as_mut_ptr();
    for i in 0..leaked.len() {
        let next = if i + 1 < leaked.len() {
            std::ptr::NonNull::new(unsafe { base.add(i + 1) })
        } else {
            None
        };
        unsafe {
            (*base.add(i)).next = next;
        }
    }
    Some(base)
}

/// Rebuilds every chain whose length doesn't match what `mode` asks for -
/// or, with `mode == None` (both keys at their defaults after a reload),
/// restores every chain this module has touched back to its vanilla
/// length. Returns the number of chains changed.
fn apply(mode: Option<&Mode>) -> usize {
    // `summon_buddy_manager`'s storage isn't populated until the player is
    // actually in the world - see `summon_count.rs` (crash 2026-08-29).
    if common::player::main_player_chr_ins_ptr().is_none() {
        return 0;
    }
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return 0;
    };
    let manager = &mut world_chr_man.summon_buddy_manager;

    let mut snapshot_guard = ORIGINAL_CHAIN_IDS.lock().unwrap();
    let snapshot = snapshot_guard.get_or_insert_with(HashMap::new);

    let mut changed = 0;
    for (speffect_id, head) in manager.trigger_speffect_to_buddy_map.iter_chains_mut() {
        let original_ids = snapshot
            .entry(*speffect_id)
            .or_insert_with(|| head.iter().copied().collect());
        if original_ids.is_empty() {
            continue;
        }
        let target = match mode {
            Some(mode) => target_count(mode, original_ids.len()),
            None => original_ids.len(),
        };
        if head.chain_len() == target {
            continue;
        }
        head.next = build_extension(original_ids, target).and_then(std::ptr::NonNull::new);
        changed += 1;
    }
    changed
}

/// Registers the chain check on `FrameBegin` (the game's main thread - the
/// only safe place to mutate `WorldChrMan`). Never returns.
pub fn run() {
    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "Chain",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            let mode = read_mode();
            let changed = apply(mode.as_ref());
            if changed > 0 {
                logger::log(&format!("Chain: rebuilt {changed} chain(s)."));
            }
        },
    );

    logger::log("Chain: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
