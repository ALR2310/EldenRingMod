//! Passive rune gain over time, ported from PassiveRunes' RuneEngine.cpp -
//! same feature, same milestone semantics, but adding runes through
//! fromsoftware-rs's real `GameDataMan::main_player_game_data.rune_count`
//! field instead of a hand-decoded AOB pointer chain (`GAMEDATAMAN_PATTERN` +
//! `OFFSET_1`/`OFFSET_2`, reverse engineered byte-by-byte from a third-party
//! cheat DLL), and running as a task on the game's own per-frame scheduler
//! (`CSTaskImp`) instead of a separate sleeping OS thread.
//!
//! Ini keys/units follow the project-wide `Rune.Passive.*`/`Rune.Milestone`
//! convention (also used by `SomeTweaks`' own Rune Reward module) - millisecond
//! `Interval`, matching `Regen.PerTick.Interval` - rather than the original's
//! own `IntervalSeconds`/`RunesPerInterval`/`Milestones` seconds-based keys.
//!
//! Milestone bonuses are granted by threshold-crossing (`session_elapsed_ms >=
//! milestone.at_ms`) rather than the original's exact `elapsed == at_seconds`
//! check - the original assumed the ticking interval always divides every
//! milestone time evenly and that no tick is ever skipped; a single frame
//! hitch, or an `Interval` that doesn't evenly divide a milestone, would
//! silently skip that bonus forever. Threshold-crossing can't miss one: it
//! fires as soon as the elapsed time reaches or passes it.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp, GameDataMan, WorldChrMan};
use eldenring::util::input;
use eldenring::util::system::wait_for_system_init;
use fromsoftware_shared::{FromStatic, Program, RecurringTaskHandle, SharedTaskImpExt};

use common::config;
use common::input::parse_virtual_key;
use common::logger;

const VK_F5: i32 = 0x74;

struct Milestone {
    at_ms: f64,
    bonus: u32,
}

const DEFAULT_MILESTONES: &str =
    "1800:5000,3600:10000,7200:25000,10800:40000,21600:100000,43200:500000,86400:1000000";

/// Parses "seconds:bonus" pairs separated by commas (the ini's own unit is
/// seconds, converted here to ms to match `session_elapsed_ms`), silently
/// skipping any entry that doesn't parse - `Rune.Milestone=` (empty) in
/// particular has no `:`, so it naturally parses to an empty list, same "0/
/// empty disables" convention as every other key in this ini without needing
/// a special case. Sorted ascending by `at_ms` so the threshold-crossing
/// check in [run] can track "next milestone to grant" as a simple advancing
/// index, regardless of the order they're listed in the ini.
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
/// (same gate `autoregen`/`sometweaks` use for their heal-over-time ticks)
/// confirms a live player character actually exists.
fn add_runes(amount: u32) -> bool {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
        return false;
    };
    if world_chr_man.main_player.is_none() {
        return false;
    }
    let Ok(game_data_man) = (unsafe { GameDataMan::instance_mut() }) else {
        return false;
    };
    let data = &mut game_data_man.main_player_game_data;
    data.rune_count = data.rune_count.saturating_add(amount);
    true
}

/// Waits for the earliest reliable "the game process is actually alive"
/// signal (`CSWindow`'s global hInstance, populated right after CRT init -
/// see the crate's own doc comment on this function), retrying past
/// `SystemInitError::InvalidRva`/`Timeout` instead of giving up. Ported
/// from `.docs/UltimatePassiveRegeneration` via `sometweaks::task`
/// (2026-08-26) - `fromsoftware-rs` already ships this helper for exactly
/// this purpose, PassiveRunes just hadn't called it before, going straight
/// for `CSTaskImp` instead.
fn wait_for_system_init_until_ready() {
    let program = Program::current();
    loop {
        if wait_for_system_init(&program, Duration::from_secs(5)).is_ok() {
            return;
        }
        logger::log("System not initialized yet, retrying...");
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// `CSTaskImp::wait_for_instance` treats `SystemInitError::InvalidRva` as
/// immediately fatal and never retries it, even with `Duration::MAX` - it
/// only retries the `Null` case internally. `InvalidRva` fires whenever the
/// version-specific RVA lookup runs before the game executable has finished
/// unpacking/relocating (e.g. Arxan), which is a timing race against how
/// early this DLL's worker thread happens to start - nothing to do with
/// where the DLL itself lives on disk. Retrying here with a short delay
/// rides out that race instead of permanently disabling the mod for the
/// session on a one-off early poll.
fn wait_for_cs_task() -> &'static CSTaskImp {
    wait_for_system_init_until_ready();

    loop {
        match CSTaskImp::wait_for_instance(Duration::MAX) {
            Ok(instance) => return instance,
            Err(err) => {
                logger::log(&format!("CSTaskImp not ready yet ({err:?}), retrying in 1s..."));
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

/// Registers `f` as a recurring task the same way `cs_task.run_recurring`
/// does, but catches any panic `f` raises for a given frame instead of
/// letting it unwind into the game's own call stack. Ported from
/// `.docs/UltimatePassiveRegeneration` via `sometweaks::task` (2026-08-26).
/// Requires `[profile.release]`'s `panic = "abort"` to be off (see
/// workspace `Cargo.toml`).
fn run_recurring_safe<F>(
    cs_task: &'static CSTaskImp,
    group: CSTaskGroupIndex,
    mut f: F,
) -> RecurringTaskHandle<eldenring::fd4::FD4TaskData>
where
    F: FnMut(&eldenring::fd4::FD4TaskData) + 'static + Send,
{
    cs_task.run_recurring(
        move |data: &eldenring::fd4::FD4TaskData| {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(data))).is_err() {
                logger::log("PassiveRunes tick panicked, skipped this frame.");
            }
        },
        group,
    )
}

/// Registers the passive-rune tick as a recurring task on the game's own
/// `FrameBegin` task group and blocks the calling thread forever watching for
/// `General.ReloadKey`. Meant to run on its own worker thread spawned from
/// `DllMain`; never returns.
pub fn run(ini_path: String) {
    let cs_task = wait_for_cs_task();

    // Milestones are parsed once at startup rather than re-read every tick
    // like Enabled/Interval/Amount below - re-parsing would also require
    // deciding what happens to `next_milestone`'s progress on a config
    // change, which isn't worth it (a milestone already granted stays
    // granted across a hot reload; only future thresholds see updated
    // amounts).
    let milestones = parse_milestones(&config::get_string("Rune.Milestone", DEFAULT_MILESTONES));
    let mut next_milestone: usize = 0;

    // `session_elapsed_ms` never resets (drives milestones); `interval_elapsed_ms`
    // resets every time it crosses `Rune.Passive.Interval` (drives the
    // per-tick rune grant) - two independent clocks.
    let mut session_elapsed_ms: f64 = 0.0;
    let mut interval_elapsed_ms: f64 = 0.0;

    let _handle = run_recurring_safe(
        cs_task,
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            // General.ReloadKey - already debounced by eldenring::util::input,
            // so this fires once per physical press regardless of how many
            // frames the key stays down. This module runs as its own DLL, so
            // nothing else re-reads the ini for it - it has to poll the key
            // itself, same as AutoRegen/SomeTweaks do in their own tick.
            let reload_key = parse_virtual_key(&config::get_string("ReloadKey", "F5"), VK_F5);
            if input::is_key_pressed(reload_key) {
                config::load(&ini_path);
                logger::log("Config reloaded (hotkey pressed).");
            }

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
                            "Milestone bonus +{} at {}s",
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

            let amount = config::get_int("Rune.Passive.Amount", 100).max(0) as u32;
            if amount > 0 && add_runes(amount) && config::get_bool("RuneLog", false) {
                logger::log(&format!(
                    "+{amount} runes (interval tick, session {:.0}s)",
                    session_elapsed_ms / 1000.0
                ));
            }
        },
    );

    logger::log("PassiveRunes tick registered on CSTaskGroupIndex::FrameBegin.");

    // `_handle` cancels the recurring task if dropped - park this thread
    // forever so it stays alive for the lifetime of the DLL.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
