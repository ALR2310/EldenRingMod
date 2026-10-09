//! `RegenMode` / `RegenValue` (hot reload): every second, heals every
//! character in `WorldChrMan::summon_buddy_chr_set` - spirit ashes and
//! Torrent (user's choice, 2026-09-29). `RegenValue` 0 = off. Modes (2026-10-09):
//!
//! - 1: `RegenValue` HP per second (flat);
//! - 2 (default): `RegenValue` percent of the character's max HP;
//! - 3: `RegenValue` percent of the HP it is missing (heals fast when badly
//!   hurt, slowly when nearly full).
//!
//! The first version had a single `Regen` key (percent of max HP, mode 2);
//! it is carried over to `RegenValue` when the ini is updated.
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

/// How `RegenValue` is read, see the module doc.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    FlatHp,
    PercentOfMax,
    PercentOfMissing,
}

impl Mode {
    /// The `RegenMode` ini value; anything else is the default (percent of max).
    fn from_ini(value: i32) -> Mode {
        match value {
            1 => Mode::FlatHp,
            3 => Mode::PercentOfMissing,
            _ => Mode::PercentOfMax,
        }
    }
}

/// HP to heal a character with `hp` of `max_hp` this second: at least 1 (a
/// small percent of a small pool must not round to nothing), at most what is
/// missing; 0 if it is already full. `value` as in the module doc.
fn heal_amount(mode: Mode, value: f64, hp: i32, max_hp: i32) -> i32 {
    let missing = (max_hp - hp).max(0);
    if missing == 0 {
        return 0;
    }
    let raw = match mode {
        Mode::FlatHp => value,
        Mode::PercentOfMax => value / 100.0 * max_hp as f64,
        Mode::PercentOfMissing => value / 100.0 * missing as f64,
    };
    (raw as i32).clamp(1, missing)
}

/// Heals each live summon by [heal_amount]. Dead characters (`hp <= 0`) are
/// left alone.
fn heal_summons(mode: Mode, value: f64) {
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
        data.hp += heal_amount(mode, value, data.hp, data.max_hp);
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

            let value = config::get_double("RegenValue", 0.5);
            if value > 0.0 {
                heal_summons(Mode::from_ini(config::get_int("RegenMode", 2)), value);
            }
        },
    );

    logger::log("Regen: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat() {
        assert_eq!(heal_amount(Mode::FlatHp, 10.0, 100, 1000), 10);
        // Never more than is missing, never less than 1, nothing when full.
        assert_eq!(heal_amount(Mode::FlatHp, 10.0, 995, 1000), 5);
        assert_eq!(heal_amount(Mode::FlatHp, 0.2, 100, 1000), 1);
        assert_eq!(heal_amount(Mode::FlatHp, 10.0, 1000, 1000), 0);
    }

    #[test]
    fn percent_of_max() {
        assert_eq!(heal_amount(Mode::PercentOfMax, 0.75, 100, 2000), 15);
        assert_eq!(heal_amount(Mode::PercentOfMax, 0.01, 100, 2000), 1);
    }

    #[test]
    fn percent_of_missing() {
        // 10% of the 800 HP missing, and 10% of the 50 missing (5).
        assert_eq!(heal_amount(Mode::PercentOfMissing, 10.0, 200, 1000), 80);
        assert_eq!(heal_amount(Mode::PercentOfMissing, 10.0, 950, 1000), 5);
        assert_eq!(heal_amount(Mode::PercentOfMissing, 0.1, 990, 1000), 1);
    }

    #[test]
    fn mode_from_ini() {
        assert_eq!(Mode::from_ini(1), Mode::FlatHp);
        assert_eq!(Mode::from_ini(2), Mode::PercentOfMax);
        assert_eq!(Mode::from_ini(3), Mode::PercentOfMissing);
        assert_eq!(Mode::from_ini(0), Mode::PercentOfMax);
        assert_eq!(Mode::from_ini(9), Mode::PercentOfMax);
    }
}
