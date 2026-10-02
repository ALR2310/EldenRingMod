//! Debug-only probes, both off by default:
//! - `SpeedProbe`: logs the player's and Torrent's anim id, the group
//!   `speed.rs` puts it in and the `animation_speed` actually set, every
//!   time the anim changes. Used to find which anim ids belong to which
//!   group - see README.
//! - `EffectProbe` (2026-10-02, split out of `SpeedProbe` the same day):
//!   logs the player's SpEffect ids (added/removed) when they change - for
//!   testing `[[Override]]`.
//!
//! Until 2026-09-29 this also force-wrote 4 candidate speed fields at 3
//! points in the frame (`ProbeForceKey`/`ProbeForceValue`/
//! `ProbeForceStage`); removed once `animation_speed` was settled on, see
//! README "Gỡ phần ép field của SpeedProbe".

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::logger;

use crate::config;

use crate::speed::{current_anim_id, group_of, torrent};

fn fmt_anim(id: i32) -> String {
    if id < 0 {
        return format!("{id}");
    }
    format!("a{:03}_{:06}", id / 1_000_000, id % 1_000_000)
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
            if let Some(chr) = torrent(world_chr_man) {
                let anim = current_anim_id(chr);
                if speed_probe && anim != last_torrent {
                    last_torrent = anim;
                    logger::log(&format!(
                        "T {} ({anim}) speed={:.2}",
                        fmt_anim(anim),
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
