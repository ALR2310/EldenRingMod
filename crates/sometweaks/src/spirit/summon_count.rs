//! Changes how many spirits an Ash of War summons - ported from
//! `.docs/x10_summon/er10x.dll`'s "chain rebuild" (its own `worker`
//! thread's stage 3), but using `fromsoftware-rs`'s own typed structure
//! instead of hand-derived raw offsets:
//!
//! `WorldChrMan.summon_buddy_manager.trigger_speffect_to_buddy_map` is a
//! `ChainingMap<i32, i32>` mapping "summon SpEffect ID" -> "BuddyParam ID"
//! - and per its own doc comment, "because multiple BuddyParams can share
//! the same SpEffect, this is a chaining tree": an Ash that summons N
//! creatures (e.g. 3 Mad Pumpkin Heads) stores N `BuddyParam` IDs under
//! the SAME SpEffect key, linked via `ChainingMapBucketEntry::next` - this
//! IS the "chain" `er10x.dll` reverse engineers by hand (its own node
//! layout, `data` then `next` 8 bytes later, matches
//! `ChainingMapBucketEntry<T>` byte-for-byte).
//!
//! The backing storage for these chain nodes is a fixed-size array
//! (`ChainingMap::buckets`, sized once at load for the game's own vanilla
//! entry count) - there's no room to grow a chain in place, which is
//! exactly why `er10x.dll` allocates its own memory for a longer chain
//! and splices it in rather than trying to insert new nodes into the
//! game's array. This does the same thing, but leaves the game's own head
//! node for each SpEffect completely untouched and only redirects its
//! `next` pointer - `iter_chains_mut()` already hands out a safe `&mut
//! ChainingMapBucketEntry` for exactly that head node, so no raw
//! offset/pointer-into-private-fields access is needed at all (unlike
//! `er10x.dll`, which resolves the map's internal tree node layout by
//! hand since it has no access to typed source).
//!
//! Extension nodes are `Box::leak`'d (never freed) - same "never free,
//! just leak old extensions" choice `er10x.dll` makes for its own link
//! pool: freeing risks a use-after-free if the game ever re-reads an
//! already-replaced chain, and the total memory involved over a whole
//! session is negligible (a handful of small allocations, one per Ash
//! actually summoned).
//!
//! Applied on a periodic tick rather than once - `er10x.dll` re-checks
//! every chain's length on every tick (not just once), which suggests the
//! game resets these chains back to vanilla under some circumstances
//! (observed by that project, not independently confirmed here); redoing
//! the check is cheap (~100-200 short chains) and idempotent when nothing
//! changed.
//!
//! **Original chain contents are snapshotted once per SpEffect ID**
//! (`ORIGINAL_CHAIN_IDS`, first tick they're seen) and every later
//! rebuild always computes `target_count` and the cycled id list from
//! that snapshot - never from the chain's current (possibly
//! already-rebuilt) contents. Same bug class `drop_rate` already hit and
//! fixed the same way (2026-08-24): without a snapshot,
//! `Spirit.Summon.Multiplier` would read back its OWN previous rebuild's
//! now-longer chain as if it were the original vanilla one and multiply
//! it again next tick - compounding every tick instead of converging
//! (confirmed in-game 2026-08-26: `Multiplier=2` ballooned to the 10-slot
//! cap instead of just doubling).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use eldenring::ChainingMapBucketEntry;
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

const TICK_INTERVAL_MS: f64 = 1000.0;
const MAX_SUMMONS: usize = 10; // hard engine limit - see module doc comment on `misc`/`spirit` for the same cap elsewhere

/// The game's own original creature-id chain for each SpEffect ID,
/// captured the first time this module sees it (before any rebuild) -
/// `target_count`/cycling must always be derived from this, never from a
/// chain's current (possibly already-rebuilt) contents.
static ORIGINAL_CHAIN_IDS: Mutex<Option<HashMap<i32, Vec<i32>>>> = Mutex::new(None);

/// What `Spirit.Summon.Amount`/`Spirit.Summon.Multiplier` currently ask
/// for, or `None` if both are left at their "do nothing" defaults
/// (`Amount=0`, `Multiplier=1`).
enum Mode {
    /// `Spirit.Summon.Amount` - every Ash summons exactly this many,
    /// regardless of its own vanilla count.
    Fixed(usize),
    /// `Spirit.Summon.Multiplier` - every Ash summons its own vanilla
    /// count times this factor.
    Multiplier(f64),
}

fn read_mode() -> Option<Mode> {
    if !config::get_bool("Spirit.Enabled", true) {
        return None;
    }
    let amount = config::get_int("Spirit.Summon.Amount", 0);
    if amount > 0 {
        return Some(Mode::Fixed(amount as usize));
    }
    let multiplier = config::get_double("Spirit.Summon.Multiplier", 1.0);
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
    raw.clamp(1, MAX_SUMMONS)
}

/// Builds a `target_len - 1` node extension chain cycling through
/// `original_ids` (skipping index 0, since the game's own head node
/// already holds `original_ids[0]` and is left untouched), and returns
/// its head pointer (or `None` if no extension is needed).
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

/// Walks every SpEffect->BuddyParam chain, rebuilding any whose current
/// length doesn't match `mode`'s target - always computed from
/// [ORIGINAL_CHAIN_IDS]'s snapshot of that chain's true vanilla contents,
/// never from what the chain currently holds. Returns the number
/// rebuilt, for logging.
fn apply(mode: &Mode) -> usize {
    // `WorldChrMan::instance_mut()` alone returns `Ok` as soon as the
    // manager object exists (title/loading screen, mid-transition between
    // Graces), well before `summon_buddy_manager`'s own storage is
    // populated - touching it that early crashed with an access violation
    // (2026-08-29, `SomeTweaks.dll+0xE30D`: null `trigger_speffect_to_buddy_map`
    // storage pointer dereferenced while loading/transitioning). Same
    // "is the player actually in the game world" gate every other feature
    // in this crate already uses - see `player.rs`'s own doc comment.
    if crate::player::main_player_chr_ins_ptr().is_none() {
        return 0;
    }

    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return 0;
    };
    let manager = &mut world_chr_man.summon_buddy_manager;

    let mut snapshot_guard = ORIGINAL_CHAIN_IDS.lock().unwrap();
    let snapshot = snapshot_guard.get_or_insert_with(HashMap::new);

    let mut rebuilt = 0;
    for (speffect_id, head) in manager.trigger_speffect_to_buddy_map.iter_chains_mut() {
        let original_ids = snapshot
            .entry(*speffect_id)
            .or_insert_with(|| head.iter().copied().collect());
        if original_ids.is_empty() {
            continue;
        }
        let target = target_count(mode, original_ids.len());
        if head.chain_len() == target {
            continue; // already correct - nothing to do
        }
        head.next = build_extension(original_ids, target).and_then(std::ptr::NonNull::new);
        rebuilt += 1;
    }
    rebuilt
}

/// Registers the Spirit.Summon.Amount/Multiplier check as a recurring task
/// on the game's own `FrameBegin` task group, same as every other feature
/// in this crate that mutates live game structures (`WorldChrMan` here is
/// only safe to mutate from the game's main thread) - unlike
/// `er10x.dll`'s own independent OS thread + `readable()` safety-net
/// approach. Meant to run on its own worker thread spawned from
/// `DllMain`; never returns.
pub fn run() {
    let cs_task = crate::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "Spirit.Summon",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            let Some(mode) = read_mode() else {
                return;
            };

            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            let rebuilt = apply(&mode);
            if rebuilt > 0 && config::get_bool("DebugLog", false) {
                logger::log(&format!("Spirit.Summon: rebuilt {rebuilt} chain(s)."));
            }
        },
    );

    logger::log("Spirit.Summon: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
