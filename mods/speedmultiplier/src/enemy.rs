//! `[Enemy] All` (2026-10-07): one speed for hostile enemies and bosses, by
//! writing their `animation_speed` like the player's and Torrent's. See
//! `docs/enemy_speed.md`.
//!
//! Which characters count: `chr_type` Npc or Unk7 = boss (not the player, nor
//! other players' phantoms) whose `team_type` isn't one of the friendly / neutral / summon
//! teams below, and whose `NpcParam.threatLv` is above 0 (it is 0 for
//! scarabs, deer, goats, birds, wandering nobles, dummies...). `threatLv`
//! is a param column, not a `ChrIns` field, so the id -> threat table is
//! built once from the param rows (by row index, not the lookup table - see
//! `common::params`).

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{ChrIns, ChrLoadStatus, ChrType, NpcParam, SoloParamRepository, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::logger;

/// `TEAM_TYPE` values (Smithbox enum, 2026-10-07) that are not enemies: None,
/// Live, White / Black / Grey / Wandering Ghost, Ally, Decoy, Battle Ally,
/// Invader (13, 16-18), Neutral, Charmed, Host, Co-op, Friendly NPC,
/// Co-op NPC, Object, the Mad Phantoms, Spirit Summon. Torrent is team 10.
const NOT_ENEMY_TEAMS: [u8; 23] = [
    0, 1, 2, 3, 4, 5, 8, 10, 12, 13, 14, 15, 16, 17, 18, 19, 20, 26, 28, 30, 31, 32, 47,
];

/// `NpcParam` id -> `threatLv`; None until built, empty if it couldn't be.
static THREAT: Mutex<Option<HashMap<i32, u32>>> = Mutex::new(None);

fn build_threat_table() -> HashMap<i32, u32> {
    let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
        return HashMap::new();
    };
    let Some(ids) = common::params::row_ids::<NpcParam>(repo) else {
        logger::warn("Enemy: couldn't read the NpcParam row ids, enemy speed is off.");
        return HashMap::new();
    };
    let mut table = HashMap::with_capacity(ids.len());
    for (index, id) in ids.into_iter().enumerate() {
        if let Some(row) = repo.get_row_by_index::<NpcParam>(index) {
            table.insert(id as i32, row.threat_lv());
        }
    }
    logger::log(&format!("Enemy: read the threat level of {} NpcParam rows.", table.len()));
    table
}

/// Bosses are `Unk7`, not `Npc` (lock-on log 2026-10-07: Margit and Godrick
/// style bosses, Death Bird, Ulcerated Tree Spirit), while Tree Sentinel is
/// an `Npc`.
fn is_enemy_type(chr_type: ChrType) -> bool {
    matches!(chr_type, ChrType::Npc | ChrType::Unk7)
}

fn is_enemy(chr: &ChrIns, threat: &HashMap<i32, u32>) -> bool {
    is_enemy_type(chr.chr_type)
        && !NOT_ENEMY_TEAMS.contains(&chr.team_type)
        && threat.get(&chr.npc_param_id).is_some_and(|&level| level > 0)
}

/// `f(chr)` for every active character in `WorldChrMan::chr_sets`. Only
/// `Active` / `ReadyForActivation` entries are dereferenced (anything else
/// can be stale, see spiritmultiplier's `regen.rs`).
pub fn for_each_active(world_chr_man: &WorldChrMan, mut f: impl FnMut(&mut ChrIns)) {
    for set in world_chr_man.chr_sets.iter().flatten() {
        for slot in 0..set.capacity {
            let entry = unsafe { set.entries.add(slot as usize).as_ref() };
            let Some(mut chr) = entry.chr_ins else {
                continue;
            };
            if !matches!(
                entry.chr_load_status,
                ChrLoadStatus::Active | ChrLoadStatus::ReadyForActivation
            ) {
                continue;
            }
            f(unsafe { chr.as_mut() });
        }
    }
}

/// One frame. `speed` is `[Enemy] All`; `sped` holds the enemies written to,
/// so they go back to 1 when it's set back to 1 or they stop counting.
/// Victims of a backstab / riposte are left to `speed::sync_victims`.
pub fn apply(world_chr_man: &WorldChrMan, speed: f32, sped: &mut Vec<usize>) {
    let off = (speed - 1.0).abs() < 0.0001;
    if off && sped.is_empty() {
        return;
    }
    let mut guard = THREAT.lock().unwrap_or_else(|e| e.into_inner());
    let threat = guard.get_or_insert_with(build_threat_table);
    let mut seen = Vec::new();
    for_each_active(world_chr_man, |chr| {
        let key = chr as *const ChrIns as usize;
        let enemy = !off && is_enemy(chr, threat);
        if enemy {
            if !crate::speed::is_throw_victim_now(chr) {
                crate::speed::set_animation_speed(chr, speed);
            }
            seen.push(key);
        } else if sped.contains(&key) {
            crate::speed::set_animation_speed(chr, 1.0);
        }
    });
    *sped = seen;
}
