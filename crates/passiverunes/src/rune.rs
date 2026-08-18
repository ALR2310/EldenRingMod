//! Passive rune gain over time, ported from PassiveRunes' RuneEngine.cpp -
//! same feature, same ini keys, but adding runes through fromsoftware-rs's
//! real `GameDataMan::main_player_game_data.rune_count` field instead of a
//! hand-decoded AOB pointer chain (`GAMEDATAMAN_PATTERN` + `OFFSET_1`/
//! `OFFSET_2`, reverse engineered byte-by-byte from a third-party cheat DLL),
//! and running as a task on the game's own per-frame scheduler (`CSTaskImp`)
//! instead of a separate sleeping OS thread.
//!
//! Milestone bonuses are granted by threshold-crossing (`session_elapsed >=
//! milestone.at_seconds`) rather than the original's exact `elapsed ==
//! at_seconds` check - the original assumed the ticking interval always
//! divides every milestone time evenly and that no tick is ever skipped; a
//! single frame hitch, or an `IntervalSeconds` that doesn't evenly divide a
//! milestone, would silently skip that bonus forever. Threshold-crossing
//! can't miss one: it fires as soon as the elapsed time reaches or passes it.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp, GameDataMan};
use fromsoftware_shared::{FromStatic, SharedTaskImpExt};

use common::config;
use common::logger;

struct Milestone {
    at_seconds: f64,
    bonus: u32,
}

const DEFAULT_MILESTONES: &str =
    "1800:5000,3600:10000,7200:25000,10800:40000,21600:100000,43200:500000,86400:1000000";

/// Parses "seconds:bonus" pairs separated by commas, silently skipping any
/// entry that doesn't parse - same tolerant behavior as the original C++
/// `ParseMilestones`. Sorted ascending by `at_seconds` so the threshold-
/// crossing check in [run] can track "next milestone to grant" as a simple
/// advancing index, regardless of the order they're listed in the ini.
fn parse_milestones(spec: &str) -> Vec<Milestone> {
    let mut result: Vec<Milestone> = spec
        .split(',')
        .filter_map(|entry| {
            let (secs, bonus) = entry.trim().split_once(':')?;
            Some(Milestone {
                at_seconds: secs.trim().parse().ok()?,
                bonus: bonus.trim().parse().ok()?,
            })
        })
        .collect();
    result.sort_by(|a, b| a.at_seconds.total_cmp(&b.at_seconds));
    result
}

/// Adds `amount` runes to the main player's save data. Returns `false` (a
/// no-op) if `GameDataMan` isn't resolved yet (title screen, loading, ...).
fn add_runes(amount: u32) -> bool {
    let Ok(game_data_man) = (unsafe { GameDataMan::instance_mut() }) else {
        return false;
    };
    let data = &mut game_data_man.main_player_game_data;
    data.rune_count = data.rune_count.saturating_add(amount);
    true
}

/// Registers the passive-rune tick as a recurring task on the game's own
/// `FrameBegin` task group. Meant to run on its own worker thread spawned
/// from `DllMain`; never returns.
pub fn run() {
    let cs_task = match CSTaskImp::wait_for_instance(Duration::MAX) {
        Ok(instance) => instance,
        Err(err) => {
            logger::log(&format!("ERROR: CSTaskImp never became available ({err:?}) - PassiveRunes disabled for this session."));
            return;
        }
    };

    // Milestones are parsed once at startup rather than re-read every tick
    // like the other ini keys below - re-parsing would also require deciding
    // what happens to `next_milestone`'s progress on a config change, which
    // isn't worth it for a mod with no hot-reload key to begin with.
    let milestones = parse_milestones(&config::get_string("Milestones", DEFAULT_MILESTONES));
    let mut next_milestone: usize = 0;

    // `session_elapsed` never resets (drives milestones); `interval_elapsed`
    // resets every time it crosses `IntervalSeconds` (drives the per-tick
    // rune grant) - two independent clocks, same as the original's `elapsed`
    // (session total) vs. the implicit per-Sleep interval.
    let mut session_elapsed: f64 = 0.0;
    let mut interval_elapsed: f64 = 0.0;

    let _handle = cs_task.run_recurring(
        move |data: &eldenring::fd4::FD4TaskData| {
            let dt = data.delta_time.time as f64;
            session_elapsed += dt;

            if config::get_bool("EnableMilestones", true) {
                while next_milestone < milestones.len() && session_elapsed >= milestones[next_milestone].at_seconds {
                    let milestone = &milestones[next_milestone];
                    if milestone.bonus > 0 && add_runes(milestone.bonus) {
                        logger::log(&format!(
                            "Milestone bonus +{} at {}s",
                            milestone.bonus, milestone.at_seconds
                        ));
                    }
                    next_milestone += 1;
                }
            }

            // IntervalSeconds=0 disables the per-tick rune grant, same "0
            // disables" convention as every other key in this mod.
            let interval_seconds = config::get_int("IntervalSeconds", 5).max(0) as f64;
            if interval_seconds <= 0.0 {
                return;
            }

            interval_elapsed += dt;
            if interval_elapsed < interval_seconds {
                return;
            }
            interval_elapsed = 0.0;

            let runes_per_interval = config::get_int("RunesPerInterval", 25).max(0) as u32;
            if runes_per_interval > 0 && add_runes(runes_per_interval) {
                logger::log(&format!("+{runes_per_interval} runes (interval tick, session {session_elapsed:.0}s)"));
            }
        },
        CSTaskGroupIndex::FrameBegin,
    );

    logger::log("PassiveRunes tick registered on CSTaskGroupIndex::FrameBegin.");

    // `_handle` cancels the recurring task if dropped - park this thread
    // forever so it stays alive for the lifetime of the DLL.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
