//! Adjusts enemy item-drop weights in `ItemLotParam_enemy`, two mutually
//! exclusive ways picked by `DropRate.Mode` (never both at once, unlike an
//! earlier version of this module that let `DropChancePercent > 0` silently
//! override `DropRateMultiplier`):
//! - `DropRate.Mode=0`: `DropRate.Multiplier` scales every real item's own
//!   weight by a factor.
//! - `DropRate.Mode=1`: `DropRate.ChancePercent` forces every row's combined
//!   "any real item" chance to exactly this %, preserving each item's
//!   relative share against its row-mates.
//!
//! Unlike `RiseArcher`'s one-shot-only param edits, this module supports
//! `General.ReloadKey` hot reload, so re-reads the ini and recomputes on
//! every press instead of only once at startup.
//!
//! FromSoft's item-lot roll is a weighted lottery **relative to the row's own
//! total**: each of a row's up to 8 slots holds a weight
//! (`lot_item_base_point0N`), and the chance of slot N is
//! `lot_item_base_point0N / sum(lot_item_base_point01..08)`. A dedicated
//! "nothing drops" outcome, when a row has one, is just another explicit
//! slot (`lot_item_id0N == 0`) that participates in the same sum - confirmed
//! against a real exported `ItemLotParam_enemy.csv` row (SmithBox), whose
//! own displayed "% chance to occur" per slot matches this formula exactly
//! (e.g. `985/(985+15) = 98.5%`, and after editing the second slot's weight
//! `15 -> 30`: `30/(985+30) = 2.96%`, `985/(985+30) = 97.04%` - the first
//! slot's % necessarily drops since it didn't get scaled but the row's total
//! did, not a bug).
//!
//! `cumulate_lot_point0N` is deliberately left untouched: the same real
//! export shows it shipping as `0` on every slot of every row, and SmithBox
//! computes its "% chance" purely from `lot_item_base_point`, never reading
//! `cumulate_lot_point` - so despite `ITEMLOT_PARAM_ST`'s field name (and
//! older Souls-game documentation describing a precomputed running-sum
//! roll), this field is not what Elden Ring's own roll consults. Writing
//! non-vanilla values into it would risk relying on behavior no released
//! version of the game has ever actually exercised, for zero known benefit.
//!
//! Scaling every slot in a row uniformly (including a "nothing" slot) would
//! change nothing - the ratios that decide the roll are unaffected when
//! numerator and denominator scale together. Only slots holding a real item
//! (`lot_item_id0N != 0`) are scaled; a "nothing" slot's weight is left
//! untouched (`DropRateMultiplier`) or used as the fixed pivot to solve for
//! the new item weight (`DropChancePercent`).
//!
//! `DropChancePercent`'s algebra, per row: with `itemSum` = the row's own
//! total real-item weight and `otherSum` = its "nothing" slot's weight
//! (unchanged), solving `target = itemSum_new / (itemSum_new + otherSum)`
//! for `itemSum_new` gives `itemSum_new = target * otherSum / (1 - target)`;
//! every real item's weight is then scaled by `itemSum_new / itemSum`,
//! preserving each item's share relative to its row-mates while the group's
//! combined chance lands on exactly `target`. Two edges need explicit
//! handling: `target >= 100%` divides by zero in that formula, so instead
//! every "nothing" slot's weight is zeroed directly (guarantees something
//! drops, without needing to know what "infinite" item weight would mean);
//! and a row with no "nothing" slot at all (`otherSum == 0`) can't have its
//! combined chance reduced below 100% - always drops something already -
//! so `target < 100%` is a no-op for that row (nothing to redistribute the
//! removed chance into).
//!
//! Hot reload needs the game's own *original* weights, captured once before
//! any edit, to recompute from on every `ReloadKey` press - recomputing from
//! the already-modified live values would compound (2x then reload at 2x
//! again would become 4x, not stay at 2x).

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp, ItemLotParam_enemy, SoloParamRepository};
use eldenring::param::ITEMLOT_PARAM_ST;
use fromsoftware_shared::{FromStatic, SharedTaskImpExt};

use common::config;
use common::logger;

type Points = [u16; 8];
type ItemIds = [i32; 8];

// The game's own original `lot_item_base_point0N` values, keyed by row ID -
// captured once (lazily, on the first call to `apply`) before any edit, so
// every later `ReloadKey` press rescales from the true baseline instead of
// compounding on top of an already-scaled value. Guarded by a `Mutex`
// because the initial capture runs on this module's own worker thread while
// later reloads run on whichever thread the game's `FrameBegin` task group
// executes the recurring closure on.
static ORIGINAL_BASE_POINTS: Mutex<Option<HashMap<u32, Points>>> = Mutex::new(None);

fn get_base_points(row: &ITEMLOT_PARAM_ST) -> Points {
    [
        row.lot_item_base_point01(),
        row.lot_item_base_point02(),
        row.lot_item_base_point03(),
        row.lot_item_base_point04(),
        row.lot_item_base_point05(),
        row.lot_item_base_point06(),
        row.lot_item_base_point07(),
        row.lot_item_base_point08(),
    ]
}

fn get_item_ids(row: &ITEMLOT_PARAM_ST) -> ItemIds {
    [
        row.lot_item_id01(),
        row.lot_item_id02(),
        row.lot_item_id03(),
        row.lot_item_id04(),
        row.lot_item_id05(),
        row.lot_item_id06(),
        row.lot_item_id07(),
        row.lot_item_id08(),
    ]
}

fn set_base_points(row: &mut ITEMLOT_PARAM_ST, points: Points) {
    row.set_lot_item_base_point01(points[0]);
    row.set_lot_item_base_point02(points[1]);
    row.set_lot_item_base_point03(points[2]);
    row.set_lot_item_base_point04(points[3]);
    row.set_lot_item_base_point05(points[4]);
    row.set_lot_item_base_point06(points[5]);
    row.set_lot_item_base_point07(points[6]);
    row.set_lot_item_base_point08(points[7]);
}

fn clamp_u16(value: f64) -> u16 {
    value.round().clamp(0.0, u16::MAX as f64) as u16
}

/// The two mutually exclusive ways to adjust drop weights, selected by
/// `DropRate.Mode` - see the module doc comment for the algebra behind
/// [Mode::FixedPercent].
enum Mode {
    /// `DropRate.Mode=0`: scale every real item's own weight by `factor`
    /// (`DropRate.Multiplier`).
    Multiplier(f64),
    /// `DropRate.Mode=1`: force every row's combined "any real item" chance
    /// to exactly this percent (`DropRate.ChancePercent`, `0.0..=100.0`),
    /// preserving each item's share relative to its row-mates.
    FixedPercent(f64),
}

/// `DropRate.Enabled=false` is treated as `Multiplier(1.0)` - a true no-op
/// that reverts every row to its cached original weights (see [apply]),
/// rather than merely skipping the rest of this run - values already
/// scaled by a previous `Enabled=true`/`ReloadKey` press must actually be
/// undone, not just left as they were.
fn build_mode() -> Mode {
    if !config::get_bool("DropRate.Enabled", true) {
        return Mode::Multiplier(1.0);
    }
    match config::get_int("DropRate.Mode", 0) {
        1 => Mode::FixedPercent(config::get_double("DropRate.ChancePercent", 50.0).max(0.0)),
        _ => Mode::Multiplier(config::get_double("DropRate.Multiplier", 2.0).max(0.0)),
    }
}

/// Applies `mode` to a single row's weights, always derived from `original`
/// (never from an already-scaled value), so this is safe to call repeatedly
/// with a different `mode` (hot reload) without compounding.
fn scale_row(original: &Points, item_ids: &ItemIds, mode: &Mode) -> Points {
    let mut scaled = *original;
    match *mode {
        Mode::Multiplier(factor) => {
            for i in 0..8 {
                if item_ids[i] != 0 {
                    scaled[i] = clamp_u16(original[i] as f64 * factor);
                }
            }
        }
        Mode::FixedPercent(percent) => {
            let target = (percent / 100.0).min(1.0);
            let item_sum: u32 = (0..8).filter(|&i| item_ids[i] != 0).map(|i| original[i] as u32).sum();
            let other_sum: u32 = (0..8).filter(|&i| item_ids[i] == 0).map(|i| original[i] as u32).sum();
            if item_sum == 0 {
                return scaled; // no real item in this row - nothing to force a chance onto
            }
            if target >= 1.0 {
                // 100%: guarantee a drop by zeroing the "nothing" slot(s)
                // instead of dividing by zero below - item weights need no
                // change, since they'll be the only nonzero weight left.
                for i in 0..8 {
                    if item_ids[i] == 0 {
                        scaled[i] = 0;
                    }
                }
                return scaled;
            }
            if other_sum == 0 {
                return scaled; // no "nothing" slot to shrink - already drops 100% of the time
            }
            let factor = target * other_sum as f64 / ((1.0 - target) * item_sum as f64);
            for i in 0..8 {
                if item_ids[i] != 0 {
                    scaled[i] = clamp_u16(original[i] as f64 * factor);
                }
            }
        }
    }
    scaled
}

/// Applies `mode` to every `ItemLotParam_enemy` row, from a cached snapshot
/// of the game's own original weights (captured on the first call, before
/// any mutation). Returns how many rows were touched, for logging.
fn apply(repo: &mut SoloParamRepository, mode: &Mode) -> usize {
    let mut snapshot_guard = ORIGINAL_BASE_POINTS.lock().unwrap();
    let is_first_call = snapshot_guard.is_none();
    if is_first_call {
        logger::log("DropRate: SoloParamRepository ready, snapshotting ItemLotParam_enemy...");
    }
    let snapshot = snapshot_guard.get_or_insert_with(|| {
        repo.rows_mut::<ItemLotParam_enemy>()
            .map(|(id, row)| (id, get_base_points(row)))
            .collect()
    });
    if is_first_call {
        logger::log(&format!("DropRate: snapshotted {} row(s), applying...", snapshot.len()));
    }

    let mut changed = 0;
    for (id, row) in repo.rows_mut::<ItemLotParam_enemy>() {
        let Some(original) = snapshot.get(&id) else {
            continue; // shouldn't happen - every row was snapshotted above
        };
        let item_ids = get_item_ids(row);
        let scaled = scale_row(original, &item_ids, mode);
        set_base_points(row, scaled);
        changed += 1;
    }
    changed
}

/// Waits (up to `timeout`) for `SoloParamRepository` - the live in-memory
/// regulation.bin - to become available. Ported from `risearcher`'s own
/// helper of the same name.
/// Waits for both `SoloParamRepository` to resolve AND the player to
/// actually be in the game world (`regen::main_player_chr_ins_ptr`) before
/// returning - `SoloParamRepository::instance_mut()` alone can return `Ok`
/// as soon as the manager object exists (title/loading screen, well before
/// its param tables are actually populated), same class of premature-ready
/// singleton that `rune_reward::add_runes` already had to guard against for
/// `GameDataMan` (2026-08-24) - crashed in-game here instead of just
/// granting runes too early.
fn wait_for_repository(timeout: Duration) -> Option<&'static mut SoloParamRepository> {
    let step = Duration::from_millis(200);
    let mut waited = Duration::ZERO;
    loop {
        if crate::regen::main_player_chr_ins_ptr().is_some() {
            if let Ok(repo) = unsafe { SoloParamRepository::instance_mut() } {
                return Some(repo);
            }
        }
        if waited >= timeout {
            return None;
        }
        std::thread::sleep(step);
        waited += step;
    }
}

/// `CSTaskImp::wait_for_instance` treats `SystemInitError::InvalidRva` as
/// immediately fatal and never retries it, even with `Duration::MAX` - it
/// only retries the `Null` case internally. Retrying here with a short delay
/// rides out that race instead of permanently disabling hot reload for the
/// session. Same fix as `regen::wait_for_cs_task`/`multipliers::rune_multiplier`'s
/// `wait_for_cs_task` (2026-08-24).
fn wait_for_cs_task() -> &'static CSTaskImp {
    loop {
        match CSTaskImp::wait_for_instance(Duration::MAX) {
            Ok(instance) => return instance,
            Err(err) => {
                logger::log(&format!("DropRate: CSTaskImp not ready yet ({err:?}), retrying in 1s..."));
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn log_mode(mode: &Mode, changed: usize, suffix: &str) {
    match mode {
        Mode::Multiplier(factor) => logger::log(&format!(
            "DropRate.Multiplier={factor:.3} applied to {changed} ItemLotParam_enemy row(s){suffix}."
        )),
        Mode::FixedPercent(percent) => logger::log(&format!(
            "DropRate.ChancePercent={percent:.3} applied to {changed} ItemLotParam_enemy row(s){suffix}."
        )),
    }
}

/// Applies `DropRate.Mode`'s selected mode once, then watches
/// `General.ReloadKey` on the game's own `FrameBegin` task group for the
/// rest of the DLL's lifetime, recomputing from the cached original weights
/// on every press. Meant to run on its own worker thread spawned from
/// `DllMain`; never returns (except early, if `SoloParamRepository` never
/// becomes available).
///
/// `DropRate.Enabled=false` at startup skips touching
/// `SoloParamRepository`/`ItemLotParam_enemy` entirely, rather than calling
/// `apply` with a no-op `Multiplier(1.0)` - `build_mode`'s own
/// `Multiplier(1.0)` fallback exists for the *hot-reload* case (undoing an
/// already-applied scale when toggled off mid-session, from the cached
/// snapshot), which doesn't apply before this module has ever run once.
pub fn run() {
    let mut last_seen_generation = crate::regen::RELOAD_GENERATION.load(Ordering::Relaxed);

    if config::get_bool("DropRate.Enabled", true) {
        match wait_for_repository(Duration::from_secs(300)) {
            Some(repo) => {
                logger::log("DropRate: SoloParamRepository instance acquired.");
                let mode = build_mode();
                let changed = apply(repo, &mode);
                log_mode(&mode, changed, "");
            }
            None => {
                logger::log("ERROR: SoloParamRepository never became available - DropRate disabled for this session.");
            }
        }
    } else {
        logger::log("DropRate.Enabled=false - skipping ItemLotParam_enemy entirely at startup.");
    }

    let cs_task = wait_for_cs_task();
    let _handle = cs_task.run_recurring(
        move |_data: &eldenring::fd4::FD4TaskData| {
            // Poll `regen::RELOAD_GENERATION` instead of calling
            // `input::is_key_pressed(ReloadKey)` ourselves - see the module
            // doc comment on that static for why: it debounces per VK code
            // in one map shared by every caller, so a second caller checking
            // the same key `regen` already checked this frame would always
            // see `false`, never picking up a reload at all.
            let generation = crate::regen::RELOAD_GENERATION.load(Ordering::Relaxed);
            if generation == last_seen_generation {
                return;
            }
            last_seen_generation = generation;

            if crate::regen::main_player_chr_ins_ptr().is_none() {
                logger::log("DropRate: not in-world yet, reload skipped.");
                return;
            }
            let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
                logger::log("DropRate: SoloParamRepository not available on reload, skipped.");
                return;
            };
            let mode = build_mode();
            let changed = apply(repo, &mode);
            log_mode(&mode, changed, " (hotkey pressed)");
        },
        CSTaskGroupIndex::FrameBegin,
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
