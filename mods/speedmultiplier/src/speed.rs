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

use crate::config::{self, Player, Torrent};

const TORRENT_NPC_PARAM_ID: i32 = 80020000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Walk,
    Run,
    Sneak,
    Jump,
    Roll,
    /// Everything on a ladder (`PlayerLadder`), see [group_of].
    Ladder,
    Attack,
    /// Backstabs and ripostes (`PlayerCritical`), see [group_of].
    Critical,
    Skill,
    Cast,
    Item,
    Other,
}

impl Group {
    /// This group's multiplier in `[Player]`.
    fn speed(self, player: &Player) -> f32 {
        match self {
            Group::Walk => player.walk,
            Group::Run => player.run,
            Group::Sneak => player.sneak,
            Group::Jump => player.jump,
            Group::Roll => player.roll,
            Group::Ladder => player.ladder,
            Group::Attack => player.attack,
            Group::Critical => player.critical,
            Group::Skill => player.skill,
            Group::Cast => player.cast,
            Group::Item => player.item,
            Group::Other => player.other,
        }
    }
}

/// Torrent's own anims (2026-10-02, `SpeedProbe` while riding): walk
/// 0021xx (002100 start, 002110), run - the dash key, a second press
/// included - 0022xx (002220, 002200, 002221, slowing down 002210).
/// Jump: taking off 0061xx (006110 standing, 006130 walking or running -
/// the same id for both) and landing 0074xx (007400, 007451). Everything
/// else - standing 000000, turning 0051xx - is `Other`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TorrentGroup {
    Walk,
    Run,
    Jump,
    Other,
}

impl TorrentGroup {
    /// This group's multiplier in `[Torrent]`.
    fn speed(self, torrent: &Torrent) -> f32 {
        match self {
            TorrentGroup::Walk => torrent.walk,
            TorrentGroup::Run => torrent.run,
            TorrentGroup::Jump => torrent.jump,
            TorrentGroup::Other => torrent.other,
        }
    }
}

pub fn torrent_group_of(anim_id: i32) -> TorrentGroup {
    match anim_id {
        2_100..=2_199 => TorrentGroup::Walk,
        2_200..=2_299 => TorrentGroup::Run,
        6_100..=6_199 | 7_400..=7_499 => TorrentGroup::Jump,
        _ => TorrentGroup::Other,
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
    // Placidusax's Ruin (a451, only that spell - TAE list): the beam phase
    // 045110 spawns a laser on the game's own clock, so a sped-up anim
    // desyncs from it (Nexus, bloodaxis, 2026-10-04; probe 2026-10-06:
    // 045100 -> 045110, ~6 s). Out of Cast, into Other (1 by default); the
    // wind-up 045100 stays Cast. Not tested in game yet.
    if prefix == 451 && (45_110..=45_119).contains(&(anim_id % 1_000_000)) {
        return Group::Other;
    }
    if (400..600).contains(&prefix) {
        return Group::Cast;
    }
    if (600..1000).contains(&prefix) && !WEAPON_TAES_IN_SKILL_RANGE.contains(&prefix) {
        return Group::Skill;
    }
    match anim_id % 1_000_000 {
        // Walk / run / sneak (probe, 2026-10-02 - the 3rd suffix digit is
        // the gait, the same standing or sneaking): walk 0201xx (stop
        // 0221xx), run - holding the dash key - 0202xx (stop 0222xx),
        // sneaking 3xxxxx (idle 300000, sneak walk 3201xx, sneak run
        // 3202xx, going into the sneak stance 390000). The rest of the old Movement range
        // (standing 000000, 0200xx = the legs while using an item on the
        // move, jumps...) is Other since `PlayerMovement` was split.
        20_100..=20_199 | 22_100..=22_199 => Group::Walk,
        20_200..=20_299 | 22_200..=22_299 => Group::Run,
        300_000..=399_999 => Group::Sneak,
        // Jumps (probe, 2026-10-02): taking off 2020xx (202000 standing,
        // 202020 walking, 202030/202040 running), landing 2021xx (202100,
        // 202115, 202126). Only the range seen - other 20xxxx anims were
        // noted around Torrent before.
        202_000..=202_199 => Group::Jump,
        27_000..=27_999 => Group::Roll,
        // Ladders (probe, 2026-10-06), one range for all of it: grabbing
        // 028999, idle on the ladder 028030, climbing up 0280 1x (+ 028100
        // from the bottom), down 0280 2x, sliding down 028000-028002, attack
        // / kick 028040, 028045, 028036 (transition).
        28_000..=28_999 => Group::Ladder,
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
fn apply(last_active: &mut Vec<usize>, seamless_fix: &mut crate::seamless::GetterFix) {
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
    let load = world_chr_man
        .main_player
        .as_ref()
        .and_then(|player| config::LoadClass::from_weight_type(player.chr_ins.chr_ctrl.weight_type));
    let (speeds, active) = config.effective_speed(|id| sp_effects.contains(&id), load);
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

    // Seamless Co-op hooks the game's read of animation_speed - see
    // `seamless.rs`. Tell its stub which characters are ours, then check it.
    let player_ptr = world_chr_man.main_player.as_ref().map_or(0, |p| &p.chr_ins as *const ChrIns as usize);
    let torrent_ptr = torrent(world_chr_man).map_or(0, |t| t as *const ChrIns as usize);
    crate::seamless::set_own(player_ptr, torrent_ptr);
    seamless_fix.check();

    if let Some(player) = world_chr_man.main_player.as_mut() {
        let chr = &mut player.chr_ins;
        let group = group_of(current_anim_id(chr));
        let master = clamped(speeds.player.all);
        let value = if (master - 1.0).abs() > 0.0001 {
            master
        } else {
            clamped(group.speed(&speeds.player))
        };
        set_animation_speed(chr, value);
    }
    if let Some(torrent) = torrent(world_chr_man) {
        let master = clamped(speeds.torrent.all);
        let value = if (master - 1.0).abs() > 0.0001 {
            master
        } else {
            clamped(torrent_group_of(current_anim_id(torrent)).speed(&speeds.torrent))
        };
        set_animation_speed(torrent, value);
    }
}

pub fn run() {
    let cs_task = common::task::wait_for_cs_task();
    common::task::run_recurring_safe(cs_task, "Speed", CSTaskGroupIndex::ChrIns_PreBehavior, {
        let mut last_active = Vec::new();
        let mut seamless_fix = crate::seamless::GetterFix::new();
        move |_data: &eldenring::fd4::FD4TaskData| apply(&mut last_active, &mut seamless_fix)
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
    fn torrent_gaits_are_split() {
        assert_eq!(torrent_group_of(2_100), TorrentGroup::Walk);
        assert_eq!(torrent_group_of(2_110), TorrentGroup::Walk);
        assert_eq!(torrent_group_of(2_220), TorrentGroup::Run);
        assert_eq!(torrent_group_of(2_210), TorrentGroup::Run); // slowing down
        assert_eq!(torrent_group_of(0), TorrentGroup::Other); // standing
        assert_eq!(torrent_group_of(5_102), TorrentGroup::Other); // turning
        assert_eq!(torrent_group_of(6_110), TorrentGroup::Jump); // standing jump
        assert_eq!(torrent_group_of(6_130), TorrentGroup::Jump); // moving jump
        assert_eq!(torrent_group_of(7_400), TorrentGroup::Jump); // landing
        assert_eq!(torrent_group_of(7_451), TorrentGroup::Jump); // moving landing
    }

    #[test]
    fn jumps_are_their_own_group() {
        assert_eq!(group_of(202_000), Group::Jump); // standing jump
        assert_eq!(group_of(202_020), Group::Jump); // walking jump
        assert_eq!(group_of(202_040), Group::Jump); // running jump
        assert_eq!(group_of(202_100), Group::Jump); // landing
        assert_eq!(group_of(202_126), Group::Jump); // running landing
        assert_eq!(group_of(202_200), Group::Other);
    }

    #[test]
    fn gaits_are_split() {
        assert_eq!(group_of(20_110), Group::Walk);
        assert_eq!(group_of(22_100), Group::Walk); // walk stop
        assert_eq!(group_of(20_210), Group::Run);
        assert_eq!(group_of(22_200), Group::Run); // run stop
        assert_eq!(group_of(300_000), Group::Sneak); // sneak idle
        assert_eq!(group_of(320_110), Group::Sneak); // sneak walk
        assert_eq!(group_of(320_210), Group::Sneak); // sneak run
        assert_eq!(group_of(390_000), Group::Sneak); // going into the sneak stance
        assert_eq!(group_of(0), Group::Other); // standing
        assert_eq!(group_of(20_010), Group::Other); // legs while using an item
    }

    #[test]
    fn placidusax_ruin_beam_is_not_cast() {
        assert_eq!(group_of(451_045_100), Group::Cast); // wind-up
        assert_eq!(group_of(451_045_110), Group::Other); // beam
        assert_eq!(group_of(435_045_110), Group::Cast); // other incantations
    }

    #[test]
    fn ladder_anims_are_their_own_group() {
        for tail in [28_999, 28_100, 28_030, 28_011, 28_012, 28_013, 28_020, 28_023, 28_000, 28_002, 28_036, 28_040, 28_045] {
            assert_eq!(group_of(tail), Group::Ladder, "{tail}");
        }
        assert_eq!(group_of(27_999), Group::Roll);
        assert_eq!(group_of(29_000), Group::Other);
        let player = Player { ladder: 1.5, ..crate::config::template().player };
        assert_eq!(Group::Ladder.speed(&player), 1.5);
    }

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
        let defaults = crate::config::template().player;
        let player = Player { attack: 3.0, ..defaults.clone() };
        assert_eq!(Group::Critical.speed(&player), defaults.critical);
        let player = Player { critical: 1.5, ..crate::config::template().player };
        assert_eq!(Group::Critical.speed(&player), 1.5);
        assert_eq!(Group::Attack.speed(&player), defaults.attack);
    }
}
