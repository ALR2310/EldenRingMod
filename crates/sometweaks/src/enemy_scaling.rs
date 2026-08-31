//! Increases every enemy's max HP and (optionally) attack power - two
//! independent knobs, both toggled by `Enemy.Enabled`:
//!
//! - **`Enemy.Health.Multiplier`**: scales every `NpcParam.hp` row by a
//!   factor. Hot-reloadable, same snapshot+restore pattern `drop_rate` uses
//!   (captures the game's own original `hp` once, before any edit, so every
//!   later `ReloadKey` press recomputes from the true baseline instead of
//!   compounding on top of an already-scaled value).
//! - **`Enemy.Damage.SpEffectId`**: an existing SpEffect ID (from the game's
//!   own data) re-applied every tick to every enemy currently loaded in
//!   `WorldChrMan.open_field_chr_set` - see [is_enemy] for exactly which
//!   characters that includes. `0` disables this half entirely. Unlike HP,
//!   there is no generic "attack power" scalar on `NpcParam` itself -
//!   damage is resolved from `AtkParam` plus the attacker's own stats at
//!   hit time, not a single field - so applying a real attack-boosting
//!   SpEffect (the same mechanism buff items/consumables use) is the only
//!   generic, hook-free way to raise every enemy type's damage at once.
//!   This module ships no default ID: picking a `SpEffectParam` row to
//!   reuse without confirming nothing else in the game references it can
//!   silently change that other thing's behavior too - the exact class of
//!   bug `grace_menu`'s own history avoided by never hijacking existing
//!   ESD content. Find a suitable ID via a param editor
//!   (SmithBox/DSMapStudio) or `Paramdex/SpEffect.xml` before enabling.
//!
//! Both keep reading the ini fresh every tick rather than relying on
//! `General.ReloadKey`'s own generation counter for the damage half - the
//! SpEffect re-apply already runs continuously (matching
//! `torrent_anywhere`'s own reapply-every-tick pattern for its dismount-fix
//! SpEffect), so picking up an ini change is just "next tick uses the new
//! value" with no extra plumbing. [apply_health] still needs
//! `reload::RELOAD_GENERATION` explicitly, since it is NOT a per-tick loop -
//! it only runs once at startup and once per `ReloadKey` press.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

use eldenring::cs::{ChrInsExt, ChrType, CSTaskGroupIndex, NpcParam, SoloParamRepository, WorldChrMan};
use eldenring::fd4::FD4TaskData;
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

const DAMAGE_APPLY_INTERVAL_MS: f64 = 1000.0;

// The game's own original `hp` values, keyed by row ID - captured once
// (lazily, on the first call to `apply_health`) before any edit, so every
// later `ReloadKey` press rescales from the true baseline instead of
// compounding. Guarded by a `Mutex` for the same reason `drop_rate`'s own
// snapshot is: the initial capture runs on this module's own worker
// thread, later reloads run on whichever thread the game's `FrameBegin`
// task group executes the recurring closure on.
static ORIGINAL_HP: Mutex<Option<HashMap<u32, u32>>> = Mutex::new(None);

fn health_multiplier() -> f64 {
    if !config::get_bool("Enemy.Enabled", true) {
        return 1.0; // true no-op - reverts any previous multiplier back to vanilla
    }
    config::get_double("Enemy.Health.Multiplier", 1.0).max(0.0)
}

/// Scales every `NpcParam.hp` row by `factor`, always derived from the
/// cached original snapshot (never from an already-scaled value). Returns
/// how many rows were touched, for logging.
fn apply_health(repo: &mut SoloParamRepository, factor: f64) -> usize {
    let mut snapshot_guard = ORIGINAL_HP.lock().unwrap();
    let is_first_call = snapshot_guard.is_none();
    let snapshot = snapshot_guard.get_or_insert_with(|| {
        repo.rows::<NpcParam>().map(|(id, row)| (id, row.hp())).collect()
    });
    if is_first_call {
        logger::log(&format!("Enemy.Health: snapshotted {} NpcParam row(s).", snapshot.len()));
    }

    let mut changed = 0;
    for (id, row) in repo.rows_mut::<NpcParam>() {
        let Some(&original) = snapshot.get(&id) else {
            continue; // shouldn't happen - every row was snapshotted above
        };
        row.set_hp((original as f64 * factor).round().clamp(0.0, u32::MAX as f64) as u32);
        changed += 1;
    }
    changed
}

/// Whether `chr_ins` should be treated as an "enemy" for
/// `Enemy.Damage.SpEffectId` - anything in `WorldChrMan.open_field_chr_set`
/// (map-based characters) whose `chr_type` is `Npc`. This excludes the
/// player, other players/phantoms, the player's own spirit-ash summons
/// (tracked in a different `ChrSet`, `summon_buddy_chr_set`, never iterated
/// here), and cosmetic ghosts - but does NOT distinguish a real boss from a
/// regular enemy or a passive/non-hostile NPC, matching
/// `Enemy.Health.Multiplier`'s own "every `NpcParam` row" scope: both knobs
/// affect every enemy uniformly, not bosses specifically.
fn is_enemy(chr_ins: &eldenring::cs::ChrIns) -> bool {
    chr_ins.chr_type == ChrType::Npc
}

/// Re-applies `sp_effect_id` to every enemy currently loaded (see
/// [is_enemy]). Returns how many characters were touched, for logging.
fn apply_damage(world_chr_man: &mut WorldChrMan, sp_effect_id: i32) -> usize {
    let mut touched = 0;
    for chr_ins in world_chr_man.open_field_chr_set.characters() {
        if is_enemy(chr_ins) {
            chr_ins.apply_speffect(sp_effect_id, true);
            touched += 1;
        }
    }
    touched
}

/// Removes `sp_effect_id` from every enemy currently loaded (see
/// [is_enemy]) - used when `Enemy.Damage.SpEffectId` changes or gets
/// disabled, so the previous ID doesn't linger forever on top of whatever
/// replaces it.
fn remove_damage(world_chr_man: &mut WorldChrMan, sp_effect_id: i32) {
    for chr_ins in world_chr_man.open_field_chr_set.characters() {
        if is_enemy(chr_ins) {
            chr_ins.remove_speffect(sp_effect_id);
        }
    }
}

/// Applies `Enemy.Health.Multiplier` once, then registers a tick on the
/// game's own `FrameBegin` task group that:
/// - watches `General.ReloadKey` to recompute the HP multiplier from the
///   cached original snapshot (same reasoning as `drop_rate`'s own tick);
/// - every [DAMAGE_APPLY_INTERVAL_MS], re-applies `Enemy.Damage.SpEffectId`
///   to every enemy currently loaded, removing the previous ID first if it
///   changed since the last tick.
///
/// Meant to run on its own worker thread spawned from `DllMain`; never
/// returns (except early, if `SoloParamRepository` never becomes
/// available).
pub fn run() {
    let mut last_seen_generation = crate::reload::RELOAD_GENERATION.load(Ordering::Relaxed);

    if config::get_bool("Enemy.Enabled", true) {
        match crate::player::wait_for_solo_param_repository(Duration::from_secs(300)) {
            Some(repo) => {
                logger::log("Enemy.Health: SoloParamRepository instance acquired.");
                let factor = health_multiplier();
                let changed = apply_health(repo, factor);
                logger::log(&format!("Enemy.Health.Multiplier={factor:.3} applied to {changed} NpcParam row(s)."));
            }
            None => {
                logger::error("Enemy.Health: SoloParamRepository never became available, disabled for this session.");
            }
        }
    } else {
        logger::log("Enemy.Enabled=false - skipping Enemy.Health.Multiplier entirely at startup.");
    }

    let cs_task = crate::task::wait_for_cs_task();
    let mut elapsed_ms: f64 = 0.0;
    let mut last_sp_effect_id: i32 = 0;

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "Enemy",
        CSTaskGroupIndex::FrameBegin,
        move |data: &FD4TaskData| {
            let generation = crate::reload::RELOAD_GENERATION.load(Ordering::Relaxed);
            if generation != last_seen_generation {
                last_seen_generation = generation;

                if crate::player::main_player_chr_ins_ptr().is_none() {
                    logger::warn("Enemy.Health: not in-world yet, reload skipped.");
                } else if let Ok(repo) = unsafe { SoloParamRepository::instance_mut() } {
                    let factor = health_multiplier();
                    let changed = apply_health(repo, factor);
                    logger::log(&format!(
                        "Enemy.Health.Multiplier={factor:.3} applied to {changed} NpcParam row(s) (hotkey pressed)."
                    ));
                } else {
                    logger::warn("Enemy.Health: SoloParamRepository not available on reload, skipped.");
                }
            }

            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < DAMAGE_APPLY_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            let sp_effect_id = if config::get_bool("Enemy.Enabled", true) {
                config::get_int("Enemy.Damage.SpEffectId", 0)
            } else {
                0
            };

            let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
                return;
            };

            if sp_effect_id != last_sp_effect_id && last_sp_effect_id != 0 {
                remove_damage(world_chr_man, last_sp_effect_id);
            }
            if sp_effect_id != 0 {
                apply_damage(world_chr_man, sp_effect_id);
            }
            last_sp_effect_id = sp_effect_id;
        },
    );

    logger::log("Enemy: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
