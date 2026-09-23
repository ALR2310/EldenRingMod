//! Passive rune gain over time, ported from PassiveRunes' RuneEngine.cpp -
//! same feature, same milestone semantics, but adding runes through
//! fromsoftware-rs's real `PlayerGameData::rune_count` field (reached via
//! `WorldChrMan.main_player.player_game_data`, see [with_player_game_data])
//! instead of a hand-decoded AOB pointer chain (`GAMEDATAMAN_PATTERN` +
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
//!
//! Nothing here goes through `fromsoftware-rs`'s version-gated
//! `eldenring::rva::get()` table (2026-09-23): `CSTaskImp` acquisition and
//! the per-frame task come from `common::task` (name-based singleton + AOB,
//! see its doc comment), and player data is reached through `WorldChrMan`
//! (name-based singleton) rather than `GameDataMan::instance()`, whose
//! `FromStatic` impl is an RVA lookup - so a game patch doesn't break the
//! mod until someone ships a new `fromsoftware-rs` RVA table.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, PlayerGameData, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

/// The game caps held runes at 999,999,999 - `saturating_add` alone would
/// only stop at `u32::MAX`, past what the game itself ever lets you hold.
const MAX_HELD_RUNES: u32 = 999_999_999;

struct Milestone {
    at_ms: f64,
    /// Negative takes runes away (2026-09-23), same as every other amount.
    bonus: i64,
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
/// index, regardless of the order they're listed in the ini. A negative
/// `seconds` is skipped too - it would otherwise fire the instant the
/// player enters the world.
fn parse_milestones(spec: &str) -> Vec<Milestone> {
    let mut result: Vec<Milestone> = spec
        .split(',')
        .filter_map(|entry| {
            let (secs, bonus) = entry.trim().split_once(':')?;
            let at_seconds: f64 = secs.trim().parse().ok()?;
            if at_seconds < 0.0 {
                return None;
            }
            Some(Milestone {
                at_ms: at_seconds * 1000.0,
                bonus: bonus.trim().parse().ok()?,
            })
        })
        .collect();
    result.sort_by(|a, b| a.at_ms.total_cmp(&b.at_ms));
    result
}

/// Gives `f` mutable access to the main player's `PlayerGameData` (level,
/// held runes, ...). `None` (a no-op) if the player isn't actually in the
/// game world yet - `WorldChrMan.main_player` only exists once a live player
/// character does, which is the same gate `autoregen`/`sometweaks` use for
/// their heal-over-time ticks (`GameDataMan` alone resolves as soon as a
/// save slot is loaded, still at the title/loading screen). Reached through
/// `PlayerIns::player_game_data` instead of `GameDataMan::instance()` so no
/// RVA lookup is involved (see the module doc comment).
fn with_player_game_data<R>(f: impl FnOnce(&mut PlayerGameData) -> R) -> Option<R> {
    let world_chr_man = unsafe { WorldChrMan::instance_mut() }.ok()?;
    let main_player = world_chr_man.main_player.as_mut()?;
    let data = unsafe { main_player.player_game_data.as_mut() };
    Some(f(data))
}

/// The main player's state right after a grant, for `LogFile` lines.
struct Granted {
    held: u32,
    level: u32,
}

/// Adds `delta` runes to the main player's save data - negative takes runes
/// away - clamped to `0..=`[MAX_HELD_RUNES] (never below 0). Returns `None`
/// (a no-op) if the player isn't in the game world yet, otherwise the
/// resulting held runes + current level.
fn apply_runes(delta: i64) -> Option<Granted> {
    with_player_game_data(|data| {
        data.rune_count = (data.rune_count as i64 + delta).clamp(0, MAX_HELD_RUNES as i64) as u32;
        Granted {
            held: data.rune_count,
            level: data.level,
        }
    })
}

/// Carried state for `Rune.Passive.Interest` between interval ticks.
#[derive(Default)]
struct InterestState {
    /// Fractional runes left over from previous ticks - `0.1%` of 1,500 is
    /// 1.5, and truncating every tick would pay 1 forever until 2,000 (and
    /// nothing at all below 1,000), which isn't compound interest. Same
    /// sign as the `Interest` that produced it.
    carry: f64,
    /// Held runes right after the last grant - held dropping below this
    /// means runes were spent or lost (death), so `carry` resets. A
    /// negative `Interest` lowering held runes itself doesn't trip this:
    /// `last_held` is recorded after that grant.
    last_held: Option<u32>,
}

/// `Rune.Passive.Interest` portion of one interval tick: `percent`% of the
/// runes currently held (compound interest - negative `percent` is
/// compound decay), with fractional runes carried over to the next tick.
/// No cap (by design - see README) and no minimum - holding 0 runes earns
/// (or loses) 0. Returns `(runes, principal)` for the log line, `(0, 0)` if
/// off or the player isn't in the world.
fn interest_runes(percent: f64, state: &mut InterestState) -> (i64, u32) {
    if percent == 0.0 {
        state.carry = 0.0;
        return (0, 0);
    }
    let Some(held) = with_player_game_data(|data| data.rune_count) else {
        return (0, 0);
    };
    // Also reset on a sign flip (hot reload from +x to -x) so leftover
    // positive fractions don't offset the first negative ticks.
    if state.last_held.is_some_and(|last| held < last) || state.carry * percent < 0.0 {
        state.carry = 0.0;
    }
    let exact = held as f64 * percent / 100.0 + state.carry;
    let runes = exact.trunc();
    state.carry = exact - runes;
    (runes as i64, held)
}

/// Runes needed to go from `level` to `level + 1`. The game has no param for
/// this - it's a hardcoded formula in the exe, long since worked out by the
/// community (level 1 -> 2 = 673, matches the in-game level-up screen).
fn level_up_cost(level: u32) -> f64 {
    let l = level as f64 + 81.0;
    let x = ((l - 92.0) * 0.02).max(0.0);
    ((x + 0.1) * l * l + 1.0).floor()
}

/// `Rune.Passive.Percent` portion of one interval tick: `percent`% of the
/// main player's next level-up cost (negative takes runes away), truncated
/// toward 0, but at least 1 rune either way whenever `percent != 0` so a
/// tiny percent at low level never rounds down to nothing. Returns 0 if the
/// player isn't in the game world yet.
fn percent_runes(percent: f64) -> i64 {
    if percent == 0.0 {
        return 0;
    }
    with_player_game_data(|data| {
        let magnitude = ((level_up_cost(data.level) * percent.abs() / 100.0).floor() as i64).max(1);
        magnitude * percent.signum() as i64
    })
    .unwrap_or(0)
}

/// Registers the passive-rune tick as a recurring task on the game's own
/// `FrameBegin` task group, then parks the calling thread forever. Meant to
/// run on its own worker thread spawned from `DllMain`; never returns.
/// `General.ReloadKey` is watched by `common::reload` on its own thread (see
/// `lib.rs`) - every key below is read per tick, so a reload is picked up
/// for free.
pub fn run() {
    let cs_task = common::task::wait_for_cs_task();

    // Milestones are parsed once at startup rather than re-read every tick
    // like Enabled/Interval/Amount/Percent below - re-parsing would also require
    // deciding what happens to `next_milestone`'s progress on a config
    // change, which isn't worth it (a milestone already granted stays
    // granted across a hot reload; only future thresholds see updated
    // amounts).
    let milestones = parse_milestones(&config::get_string("Rune.Milestone", DEFAULT_MILESTONES));
    let mut next_milestone: usize = 0;

    // `session_elapsed_ms` never resets (drives milestones); `interval_elapsed_ms`
    // resets every time it crosses `Rune.Passive.Interval` (drives the
    // per-tick rune grant) - two independent clocks. Both only advance
    // while the player is in the world (`common::player::
    // main_player_chr_ins_ptr`, 2026-09-23): before, they started counting as soon as
    // the task was registered, so time spent on the title screen/menus and
    // loading screens counted toward milestones too. Paused, not reset, when
    // the player leaves the world (loading screen, quit to title) - a
    // loading screen mid-session must not wipe milestone progress.
    let mut session_elapsed_ms: f64 = 0.0;
    let mut interval_elapsed_ms: f64 = 0.0;
    let mut interest_state = InterestState::default();

    let installed = common::task::run_recurring_safe(
        cs_task,
        "Rune",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            // Rune.Passive.Enabled=false turns off both the per-interval
            // grant and the milestone bonuses below - same "whole section"
            // scope as Regen.PerTick.Enabled/Regen.PerHit.Enabled.
            if !config::get_bool("Rune.Passive.Enabled", true) {
                return;
            }

            // Same "player is actually in the world" gate DropMultiplier/
            // SomeTweaks use - checked here (not just inside apply_runes) so
            // both clocks pause, not just the grant.
            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }

            let dt_ms = (data.delta_time.time as f64) * 1000.0;
            session_elapsed_ms += dt_ms;

            while next_milestone < milestones.len() && session_elapsed_ms >= milestones[next_milestone].at_ms {
                let milestone = &milestones[next_milestone];
                if milestone.bonus != 0 {
                    // Not in the game world yet (still at the title/loading
                    // screen past this milestone's threshold) - leave
                    // `next_milestone` as-is and retry next tick instead of
                    // silently skipping the bonus forever.
                    let Some(granted) = apply_runes(milestone.bonus) else {
                        break;
                    };
                    if config::get_bool("LogFile", false) {
                        logger::log(&format!(
                            "Milestone bonus {:+} at {}s -> held {}, level {}",
                            milestone.bonus,
                            milestone.at_ms / 1000.0,
                            granted.held,
                            granted.level
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

            // Rune.Passive.Amount (fixed), Rune.Passive.Percent (% of the
            // next level-up cost) and Rune.Passive.Interest (% of held
            // runes) all stack - each is off at 0, independently, and each
            // takes runes away when negative (2026-09-23).
            // Clamped to the ranges documented in PassiveRunes.ini.
            let fixed = (config::get_int("Rune.Passive.Amount", 50) as i64)
                .clamp(-(MAX_HELD_RUNES as i64), MAX_HELD_RUNES as i64);
            let percent_setting = config::get_double("Rune.Passive.Percent", 0.25).clamp(-100.0, 100.0);
            let percent = percent_runes(percent_setting);
            let interest_setting = config::get_double("Rune.Passive.Interest", 0.0).clamp(-100.0, 100.0);
            let (interest, principal) = interest_runes(interest_setting, &mut interest_state);
            let amount = fixed + percent + interest;
            if amount == 0 {
                return;
            }
            let Some(granted) = apply_runes(amount) else {
                return;
            };
            interest_state.last_held = Some(granted.held);
            if config::get_bool("LogFile", false) {
                logger::log(&format!(
                    "{amount:+} runes ({fixed:+} fixed, {percent:+} from {percent_setting}% of {} next-level cost, {interest:+} from {interest_setting}% interest on {principal}) -> held {}, level {}, session {:.0}s",
                    level_up_cost(granted.level),
                    granted.held,
                    granted.level,
                    session_elapsed_ms / 1000.0
                ));
            }
        },
    );

    // Success is already logged by run_recurring_safe ("Rune: task registered.").
    if !installed {
        logger::error("Rune tick failed to install - game may need a mod update, check Nexus.");
    }

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
