//! Passive rune gain over time, ported from PassiveRunes' rune.rs - same
//! `GameDataMan::main_player_game_data.rune_count` write and threshold-
//! crossing milestone logic, but under SomeTweaks' `Rune.Passive.*`/
//! `Rune.Milestone` key names and millisecond `Interval` convention (matching
//! `Regen.PerTick.Interval`) instead of PassiveRunes' own `IntervalSeconds`/
//! `RunesPerInterval`/`Milestones` seconds-based keys.
//!
//! `add_runes` also gates on `player::main_player_chr_ins_ptr` (not just
//! `GameDataMan`) - `GameDataMan` resolves as soon as a save slot loads,
//! still at the title/loading screen well before control passes to the
//! player, so runes (including milestone bonuses) were being granted before
//! actually entering the world. Same bug, same fix as PassiveRunes' own
//! `add_runes` (2026-08-24).

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, GameDataMan};
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

struct Milestone {
    at_ms: f64,
    bonus: u32,
}

const DEFAULT_MILESTONES: &str =
    "1800:5000,3600:10000,7200:25000,10800:40000,21600:100000,43200:500000,86400:1000000";

/// Parses "seconds:bonus" pairs separated by commas (the ini's own unit is
/// seconds, converted here to ms to match `session_elapsed`), silently
/// skipping any entry that doesn't parse - `Rune.Milestone=0` in particular
/// has no `:`, so it naturally parses to an empty list, same "0 disables"
/// convention as every other key in this ini without needing a special case.
/// Sorted ascending by `at_ms` so the threshold-crossing check in [run] can
/// track "next milestone to grant" as a simple advancing index, regardless of
/// the order they're listed in the ini.
fn parse_milestones(spec: &str) -> Vec<Milestone> {
    let mut result: Vec<Milestone> = spec
        .split(',')
        .filter_map(|entry| {
            let (secs, bonus) = entry.trim().split_once(':')?;
            let at_seconds: f64 = secs.trim().parse().ok()?;
            Some(Milestone {
                at_ms: at_seconds * 1000.0,
                bonus: bonus.trim().parse().ok()?,
            })
        })
        .collect();
    result.sort_by(|a, b| a.at_ms.total_cmp(&b.at_ms));
    result
}

/// Adds `amount` runes to the main player's save data. Returns `false` (a
/// no-op) if `GameDataMan` isn't resolved yet, or if the player isn't
/// actually in the game world yet. `GameDataMan` alone resolves as soon as a
/// save slot is loaded - while still sitting at the title/loading screen,
/// well before control passes to the player - so it's not enough on its own
/// to gate a passive gameplay effect; checking `WorldChrMan.main_player`
/// (same gate `player::main_player_chr_ins_ptr` exists for) confirms a live
/// player character actually exists.
fn add_runes(amount: u32) -> bool {
    if crate::player::main_player_chr_ins_ptr().is_none() {
        return false;
    }
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
    let cs_task = crate::task::wait_for_cs_task();

    // Milestones are parsed once at startup rather than re-read every tick
    // like Enabled/Interval/Amount below - re-parsing would also require
    // deciding what happens to `next_milestone`'s progress on a config
    // change, which isn't worth it (same tradeoff PassiveRunes made, and this
    // module has no hot-reload key of its own to begin with - General.ReloadKey
    // only re-reads the ini file, it doesn't restart this thread).
    let milestones = parse_milestones(&config::get_string("Rune.Milestone", DEFAULT_MILESTONES));
    let mut next_milestone: usize = 0;

    // `session_elapsed_ms` never resets (drives milestones); `interval_elapsed_ms`
    // resets every time it crosses `Rune.Passive.Interval` (drives the
    // per-tick rune grant) - two independent clocks.
    let mut session_elapsed_ms: f64 = 0.0;
    let mut interval_elapsed_ms: f64 = 0.0;

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "Rune.Passive",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            // Rune.Passive.Enabled=false turns off both the per-interval
            // grant and the milestone bonuses below - same "whole section"
            // scope as Regen.PerTick.Enabled/Regen.PerHit.Enabled.
            if !config::get_bool("Rune.Passive.Enabled", true) {
                return;
            }

            let dt_ms = (data.delta_time.time as f64) * 1000.0;
            session_elapsed_ms += dt_ms;

            while next_milestone < milestones.len() && session_elapsed_ms >= milestones[next_milestone].at_ms {
                let milestone = &milestones[next_milestone];
                if milestone.bonus > 0 {
                    // Not in the game world yet (still at the title/loading
                    // screen past this milestone's threshold) - leave
                    // `next_milestone` as-is and retry next tick instead of
                    // silently skipping the bonus forever.
                    if !add_runes(milestone.bonus) {
                        break;
                    }
                    if config::get_bool("RuneLog", false) {
                        logger::log(&format!(
                            "Rune.Passive: milestone bonus +{} at {}s.",
                            milestone.bonus,
                            milestone.at_ms / 1000.0
                        ));
                    }
                }
                next_milestone += 1;
            }

            // Rune.Passive.Interval=0 disables the per-tick rune grant, same
            // "0 disables" convention as every other key in this section.
            let interval_ms = config::get_int("Rune.Passive.Interval", 10000).max(0) as f64;
            if interval_ms <= 0.0 {
                return;
            }

            interval_elapsed_ms += dt_ms;
            if interval_elapsed_ms < interval_ms {
                return;
            }
            interval_elapsed_ms = 0.0;

            // Deliberately not logged (2026-08-28): the per-interval grant
            // fires every `Rune.Passive.Interval` ms for the whole session
            // (~360 lines/hour at the default 10s) and says nothing the
            // player can't see on their own rune counter - it only inflated
            // the log file. Milestone bonuses above stay logged: they are
            // one-off, rare, and easy to miss in-game.
            let amount = config::get_int("Rune.Passive.Amount", 100).max(0) as u32;
            if amount > 0 {
                add_runes(amount);
            }
        },
    );

    logger::log("Rune.Passive: tick registered on CSTaskGroupIndex::FrameBegin.");

    // `_handle` cancels the recurring task if dropped - park this thread
    // forever so it stays alive for the lifetime of the DLL.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
