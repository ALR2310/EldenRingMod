//! Per-action speed multipliers: every frame, sets the player's
//! `CSChrBehaviorModule.animation_speed` to the multiplier of the group its
//! current animation belongs to, and Torrent's to `Torrent`. `PlayerAll`
//! (default 1) is the master key: any value other than 1 applies to every
//! player action and overrides all per-group keys (user's design,
//! 2026-09-29).
//!
//! Groups come from the anim id `aXXX_YYYYYY` (`anim_id / 1_000_000` is
//! the `XXX` prefix = which TAE, `anim_id % 1_000_000` the `YYYYYY`
//! action suffix). Spells and Ashes of War share the `04xxxx` suffix, so
//! they're told apart by prefix first: `a400`-`a599` = sorcery/incantation
//! TAEs, `a600`-`a999` = Sword Arts (minus a few special-weapon movesets
//! numbered in that range), per `.docs/Elden Ring tae list updated for
//! SOTE.txt`. Everything else goes by suffix, ranges from SpeedProbe logs -
//! see README (2026-09-29).
//!
//! `animation_speed` is never reset by the game (probe test 2), so it's
//! only written when the wanted value differs from what's there.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, ChrIns, ChrLoadStatus, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::logger;

use crate::config::{self, Speed};

const TORRENT_NPC_PARAM_ID: i32 = 80020000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Movement,
    Roll,
    Attack,
    /// Backstabs and ripostes (`PlayerCritical`), see [group_of].
    Critical,
    Skill,
    Cast,
    Item,
    Other,
}

impl Group {
    /// This group's multiplier in `[Speed]`.
    fn speed(self, speed: &Speed) -> f32 {
        match self {
            Group::Movement => speed.player_movement,
            Group::Roll => speed.player_roll,
            Group::Attack => speed.player_attack,
            Group::Critical => speed.player_critical,
            Group::Skill => speed.player_skill,
            Group::Cast => speed.player_cast,
            Group::Item => speed.player_item,
            Group::Other => speed.player_other,
        }
    }
}

/// Special-weapon movesets whose TAE number falls inside the Sword Arts
/// range (Axe of Godfrey, Starscourge Greatsword, Ghiza's Wheel, Ornamental
/// Straight Sword, Repeating Crossbow, Rakshasa's Great Katana).
const WEAPON_TAES_IN_SKILL_RANGE: [i32; 6] = [831, 832, 839, 852, 935, 953];

/// Classifies a player anim id: by TAE prefix for spells/Sword Arts, by
/// action suffix for everything else.
pub fn group_of(anim_id: i32) -> Group {
    if anim_id < 0 {
        return Group::Other;
    }
    let prefix = anim_id / 1_000_000;
    if (400..600).contains(&prefix) {
        return Group::Cast;
    }
    if (600..1000).contains(&prefix) && !WEAPON_TAES_IN_SKILL_RANGE.contains(&prefix) {
        return Group::Skill;
    }
    match anim_id % 1_000_000 {
        0 | 20_000..=26_999 => Group::Movement,
        27_000..=27_999 => Group::Roll,
        // Critical hits (riposte 031700, backstab 031719 -> 031710 - probe,
        // 2026-10-02) are paired with the victim's anim, which this mod
        // doesn't speed up: a faster player pulled the blade out while the
        // enemy was still falling (user test). Own key `PlayerCritical`,
        // default 1 (vanilla) - first fixed at 1, made a key the same day
        // at the user's request.
        31_700..=31_799 => Group::Critical,
        30_000..=39_999 => Group::Attack,
        40_000..=49_999 => Group::Skill,
        // Torrent's whistle is an item, but in game it's the first part of
        // mounting (050190 -> 101004 mount -> 100000 riding), so it stays
        // with the other riding anims.
        50_190 => Group::Other,
        50_000..=59_999 => Group::Item,
        _ => Group::Other,
    }
}

/// The anim id currently playing, off `CSChrTimeActModule.anim_queue`.
pub fn current_anim_id(chr: &ChrIns) -> i32 {
    let time_act = &chr.modules.time_act;
    let idx = (time_act.read_idx as usize) % time_act.anim_queue.len();
    time_act.anim_queue[idx].anim_id
}

/// Torrent's `ChrIns`: the active `summon_buddy_chr_set` entry with its
/// NpcParam id (slot #0 for the local player's own, see spiritmultiplier's
/// README). Only `Active`/`ReadyForActivation` entries are dereferenced,
/// same rule as spiritmultiplier's `regen.rs`.
pub fn torrent(world_chr_man: &mut WorldChrMan) -> Option<&mut ChrIns> {
    let chr_set = &world_chr_man.summon_buddy_chr_set;
    for slot in 0..chr_set.capacity {
        let entry = unsafe { chr_set.entries.add(slot as usize).as_ref() };
        let Some(mut chr) = entry.chr_ins else {
            continue;
        };
        if !matches!(
            entry.chr_load_status,
            ChrLoadStatus::Active | ChrLoadStatus::ReadyForActivation
        ) {
            continue;
        }
        let chr = unsafe { chr.as_mut() };
        if chr.npc_param_id == TORRENT_NPC_PARAM_ID {
            return Some(chr);
        }
    }
    None
}

/// A multiplier clamped to a sane range (0 or negative would freeze or
/// reverse the animation).
fn clamped(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.1, 10.0)
    } else {
        1.0
    }
}

fn set_animation_speed(chr: &mut ChrIns, value: f32) {
    let behavior = &mut chr.modules.behavior;
    if (behavior.animation_speed - value).abs() > 0.0001 {
        behavior.animation_speed = value;
    }
}

/// One frame. `last_active` is the set of active overrides from the last
/// frame, so a change is logged once instead of every frame.
fn apply(last_active: &mut Vec<usize>) {
    if common::player::main_player_chr_ins_ptr().is_none() {
        return;
    }
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return;
    };
    let config = config::get();

    // The player's SpEffects decide which `[[Override]]`s are on - for
    // Torrent's speed too (a buff on the rider).
    let sp_effects: Vec<i32> = match world_chr_man.main_player.as_ref() {
        Some(player) if !config.overrides.is_empty() => player
            .chr_ins
            .special_effect
            .entries()
            .map(|entry| entry.param_id)
            .collect(),
        _ => Vec::new(),
    };
    let (speed, active) = config.effective_speed(|id| sp_effects.contains(&id));
    if active != *last_active {
        let list = active
            .iter()
            .map(|i| format!("#{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        logger::log(&format!(
            "Speed: active overrides: {}",
            if list.is_empty() {
                "none".to_string()
            } else {
                list
            }
        ));
        *last_active = active;
    }

    if let Some(player) = world_chr_man.main_player.as_mut() {
        let chr = &mut player.chr_ins;
        let group = group_of(current_anim_id(chr));
        let master = clamped(speed.player_all);
        let value = if (master - 1.0).abs() > 0.0001 {
            master
        } else {
            clamped(group.speed(&speed))
        };
        set_animation_speed(chr, value);
    }
    if let Some(torrent) = torrent(world_chr_man) {
        set_animation_speed(torrent, clamped(speed.torrent));
    }
}

pub fn run() {
    let cs_task = common::task::wait_for_cs_task();
    common::task::run_recurring_safe(cs_task, "Speed", CSTaskGroupIndex::ChrIns_PreBehavior, {
        let mut last_active = Vec::new();
        move |_data: &eldenring::fd4::FD4TaskData| apply(&mut last_active)
    });
    logger::log("Speed: per-action multipliers active.");
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_hits_are_their_own_group() {
        assert_eq!(group_of(23_031_700), Group::Critical); // riposte
        assert_eq!(group_of(23_031_719), Group::Critical); // backstab start
        assert_eq!(group_of(23_031_710), Group::Critical); // backstab
        assert_eq!(group_of(23_030_000), Group::Attack);
        assert_eq!(group_of(23_031_699), Group::Attack);
        assert_eq!(group_of(23_031_800), Group::Attack);
    }

    #[test]
    fn critical_hits_use_their_own_key() {
        let speed = Speed { player_attack: 3.0, ..Speed::default() };
        assert_eq!(Group::Critical.speed(&speed), 1.0); // default
        let speed = Speed { player_critical: 1.5, ..Speed::default() };
        assert_eq!(Group::Critical.speed(&speed), 1.5);
        assert_eq!(Group::Attack.speed(&speed), 1.2);
    }
}
