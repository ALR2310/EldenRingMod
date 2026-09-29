//! Debug-only `SpeedProbe` (off by default): logs the player's and
//! Torrent's anim id, the group `speed.rs` puts it in and the
//! `animation_speed` actually set, every time the anim changes. Used to find
//! which anim ids belong to which group - see README.
//!
//! Until 2026-09-29 this also force-wrote 4 candidate speed fields at 3
//! points in the frame (`ProbeForceKey`/`ProbeForceValue`/
//! `ProbeForceStage`); removed once `animation_speed` was settled on, see
//! README "Gỡ phần ép field của SpeedProbe".

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

use crate::speed::{current_anim_id, group_of, torrent};

fn fmt_anim(id: i32) -> String {
    if id < 0 {
        return format!("{id}");
    }
    format!("a{:03}_{:06}", id / 1_000_000, id % 1_000_000)
}

pub fn run() {
    if !config::get_bool("SpeedProbe", false) {
        return;
    }

    let cs_task = common::task::wait_for_cs_task();

    let mut last_player = i32::MIN;
    let mut last_torrent = i32::MIN;
    // PostPhysics: after `speed.rs` (PreBehavior) has set this frame's value.
    common::task::run_recurring_safe(
        cs_task,
        "SpeedProbe",
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
                let anim = current_anim_id(chr);
                if anim != last_player {
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
                if anim != last_torrent {
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
