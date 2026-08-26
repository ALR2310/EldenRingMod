//! Heals every currently-summoned spirit ash's HP by a percent of its own
//! max HP every second, `0` disables it - same tick-based idea as
//! `regen`'s own `Regen.PerTick` (under `Unit=1`), just scoped to
//! `WorldChrMan::summon_buddy_chr_set` instead of the main player.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

const TICK_INTERVAL_MS: f64 = 1000.0;

/// Heals `percent_fraction` (e.g. 0.01 = 1%) of each summon's own max HP,
/// clamped to max, at least 1 point restored if the percent alone would
/// round to 0 (no-op if dead).
fn heal_summons(percent_fraction: f64) {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return;
    };
    for chr_ins in world_chr_man.summon_buddy_chr_set.characters() {
        let data = &mut chr_ins.modules.data;
        if data.hp <= 0 {
            continue; // dead - don't touch anything
        }
        let heal = ((percent_fraction * data.max_hp as f64) as i32).max(1);
        data.hp = (data.hp + heal).min(data.max_hp);
    }
}

/// Registers the Spirit.Regen tick as a recurring task on the game's own
/// `FrameBegin` task group. Meant to run on its own worker thread spawned
/// from `DllMain`; never returns.
pub fn run() {
    let cs_task = crate::task::wait_for_cs_task("Spirit.Regen");

    let mut elapsed_ms: f64 = 0.0;

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "Spirit.Regen",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            if !config::get_bool("Spirit.Enabled", true) {
                return;
            }
            let percent = config::get_double("Spirit.Regen", 1.0);
            if percent <= 0.0 {
                return;
            }

            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            heal_summons(percent / 100.0);
        },
    );

    logger::log("Spirit.Regen tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
