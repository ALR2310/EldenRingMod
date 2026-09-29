//! `Regen` (default `0.5`, `0` = off, hot reload): every second, heals every
//! character in `WorldChrMan::summon_buddy_chr_set` - spirit ashes and
//! Torrent (user's choice, 2026-09-29) - by that percent of its own max HP.
//!
//! Same technique as `sometweaks`'s `spirit/regen.rs`, kept as a separate
//! copy here rather than moved to `common` at the user's request
//! (2026-09-29). One difference: only `Active` / `ReadyForActivation`
//! entries are dereferenced (a `ChrIns` behind any other load status can be
//! stale - see `enemy_probe.rs`'s crashes), where `regen.rs` walks
//! `characters()`, which reads every occupied entry.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, ChrLoadStatus, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

const TICK_INTERVAL_MS: f64 = 1000.0;

/// Heals `fraction` (e.g. 0.005 = 0.5%) of each live summon's own max HP,
/// clamped to max, at least 1 point if the percent alone rounds to 0.
/// Dead characters (`hp <= 0`) are left alone.
fn heal_summons(fraction: f64) {
    if common::player::main_player_chr_ins_ptr().is_none() {
        return;
    }
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return;
    };
    let chr_set = &world_chr_man.summon_buddy_chr_set;
    for slot in 0..chr_set.capacity {
        let entry = unsafe { chr_set.entries.add(slot as usize).as_ref() };
        let Some(mut chr) = entry.chr_ins else { continue };
        if !matches!(
            entry.chr_load_status,
            ChrLoadStatus::Active | ChrLoadStatus::ReadyForActivation
        ) {
            continue;
        }
        let data = &mut unsafe { chr.as_mut() }.modules.data;
        if data.hp <= 0 {
            continue;
        }
        let heal = ((fraction * data.max_hp as f64) as i32).max(1);
        data.hp = (data.hp + heal).min(data.max_hp);
    }
}

pub fn run() {
    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "Regen",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            let percent = config::get_double("Regen", 0.5);
            if percent > 0.0 {
                heal_summons(percent / 100.0);
            }
        },
    );

    logger::log("Regen: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
