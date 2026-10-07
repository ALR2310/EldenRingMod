//! Debug-only probes, both off by default:
//! - `SpeedProbe`: logs the player's and Torrent's anim id, the group
//!   `speed.rs` puts it in and the `animation_speed` actually set, every
//!   time the anim changes. Used to find which anim ids belong to which
//!   group - see README.
//! - `EffectProbe` (2026-10-02, split out of `SpeedProbe` the same day):
//!   logs the player's SpEffect ids (added/removed) when they change - for
//!   testing `[[Override]]`.
//! - `SpeedProbe` also logs `ChrCtrl.weight_type` and the player's
//!   `max_equip_load` when either changes (2026-10-06) - to find which
//!   `weight_type` value is which equip load class, for a `Load`
//!   condition in `[[Override]]` (see TODO).
//! - `SpeedProbe` also logs every character whose `CSChrThrowModule`
//!   `throw_state` changes (2026-10-07; every value, 0 and 1-2 included):
//!   `InThrowAttacker` 3 / `InThrowTarget` 4 / `DeathAttacker` 5 /
//!   `DeathTarget` 6 - to find when the victim of a backstab or riposte
//!   gets up (see TODO).
//!
//! Until 2026-09-29 this also force-wrote 4 candidate speed fields at 3
//! points in the frame (`ProbeForceKey`/`ProbeForceValue`/
//! `ProbeForceStage`); removed once `animation_speed` was settled on, see
//! README "Gỡ phần ép field của SpeedProbe".

use std::collections::HashMap;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, ChrIns, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::logger;

use crate::config;

use crate::speed::{current_anim_id, group_of, throw_state, torrent, torrent_group_of};

fn fmt_anim(id: i32) -> String {
    if id < 0 {
        return format!("{id}");
    }
    format!("a{:03}_{:06}", id / 1_000_000, id % 1_000_000)
}

fn log_throw(last: &mut HashMap<usize, u32>, who: &str, chr: &ChrIns) {
    let Some(state) = throw_state(chr) else {
        return;
    };
    let key = chr as *const ChrIns as usize;
    // First sight of a character: remember it, only log when it isn't idle.
    let before = last.insert(key, state).unwrap_or(0);
    if state == before {
        return;
    }
    let anim = current_anim_id(chr);
    logger::log(&format!(
        "THROW {who} {key:#x} npc={} state {before}->{state} flags={:#x} {} speed={:.2}",
        chr.npc_param_id,
        chr.modules.throw.flags.0,
        fmt_anim(anim),
        chr.modules.behavior.animation_speed,
    ));
}

pub fn run() {
    let (speed_probe, effect_probe) = {
        let logging = &config::get().logging;
        (logging.speed_probe, logging.effect_probe)
    };
    if !speed_probe && !effect_probe {
        return;
    }

    let cs_task = common::task::wait_for_cs_task();

    let mut last_player = i32::MIN;
    let mut last_torrent = i32::MIN;
    let mut last_sp_effects: Vec<i32> = Vec::new();
    let mut last_weight: (u32, f32) = (u32::MAX, f32::NAN);
    let mut last_throw: HashMap<usize, u32> = HashMap::new();
    // PostPhysics: after `speed.rs` (PreBehavior) has set this frame's value.
    common::task::run_recurring_safe(
        cs_task,
        "Probe",
        CSTaskGroupIndex::ChrIns_PostPhysics,
        move |_data: &eldenring::fd4::FD4TaskData| {
            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }
            let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
                return;
            };
            if let Some(player) = world_chr_man.main_player.as_ref() {
                let chr = &player.chr_ins;

                if effect_probe {
                    let mut sp_effects: Vec<i32> =
                        chr.special_effect.entries().map(|e| e.param_id).collect();
                    sp_effects.sort_unstable();
                    sp_effects.dedup();
                    if sp_effects != last_sp_effects {
                        let added: Vec<i32> = sp_effects
                            .iter()
                            .filter(|id| !last_sp_effects.contains(id))
                            .copied()
                            .collect();
                        let removed: Vec<i32> = last_sp_effects
                            .iter()
                            .filter(|id| !sp_effects.contains(id))
                            .copied()
                            .collect();
                        logger::log(&format!(
                            "P SpEffect +{added:?} -{removed:?} now {sp_effects:?}"
                        ));
                        last_sp_effects = sp_effects;
                    }
                }

                if speed_probe {
                    let weight_type = chr.chr_ctrl.weight_type;
                    let max_load = unsafe { player.player_game_data.as_ref() }.max_equip_load;
                    if weight_type != last_weight.0 || max_load != last_weight.1 {
                        last_weight = (weight_type, max_load);
                        logger::log(&format!(
                            "P weight_type={weight_type} max_equip_load={max_load:.1}"
                        ));
                    }
                }

                let anim = current_anim_id(chr);
                if speed_probe && anim != last_player {
                    last_player = anim;
                    logger::log(&format!(
                        "P {} ({anim}) {:?} speed={:.2} fp={}/{}",
                        fmt_anim(anim),
                        group_of(anim),
                        chr.modules.behavior.animation_speed,
                        chr.modules.data.fp,
                        chr.modules.data.max_fp,
                    ));
                }
            }
            if speed_probe {
                if let Some(player) = world_chr_man.main_player.as_ref() {
                    log_throw(&mut last_throw, "player", &player.chr_ins);
                }
                for set in world_chr_man.chr_sets.iter().flatten() {
                    for chr in set.characters() {
                        log_throw(&mut last_throw, "chr", chr);
                    }
                }
            }
            if let Some(chr) = torrent(world_chr_man) {
                let anim = current_anim_id(chr);
                if speed_probe && anim != last_torrent {
                    last_torrent = anim;
                    logger::log(&format!(
                        "T {} ({anim}) {:?} speed={:.2}",
                        fmt_anim(anim),
                        torrent_group_of(anim),
                        chr.modules.behavior.animation_speed,
                    ));
                }
            }
        },
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
