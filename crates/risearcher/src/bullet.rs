//! Applies RiseArcher's Arrow/Great Arrow/Radahn's Spear/Bolt projectile
//! tuning directly to the live `Bullet` regulation rows, ported from
//! `csv/RiseArcher.MASSEDIT`'s ~1000 hardcoded per-ID lines.
//!
//! `Bullet` has no field to filter by like `EquipParamWeapon.weaponCategory`
//! (confirmed during the original CSV analysis - see the project's
//! `README.md`), so which rows to touch has to be data, not a predicate.
//! What *is* regular is the ID layout within each arrow/bolt "family": a
//! base ID (the plain arrow/bolt itself) plus a fixed set of offsets for its
//! Ash of War variants - e.g. `base+11` = Mighty Shot, `base+50` = Rain of
//! Arrows' own speed, `base+51` = Rain of Arrows' range, etc. This module
//! encodes that structure once (`FAMILIES` below) instead of repeating it
//! per weapon like the original Mass Edit file did.

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{Bullet, SoloParamRepository};
use eldenring::param::BULLET_PARAM_ST;

use common::config;
use common::logger;

/// What an offset within a family means, i.e. which field(s) it tunes.
/// Every family's `base + 0` offset (and, for [ArrowFamily::Arrow], every
/// AOW variant offset except Rain of Arrows' split-out sub-bullets) is a
/// [Speed] bullet - Rain of Arrows alone splits into 3 separate bullet rows
/// (range/count/spread) because the skill needs to tune each independently.
#[derive(Clone, Copy)]
enum Role {
    /// `initVellocity`/`maxVellocity` - plain projectile speed.
    Speed,
    /// `dist` (Attenuation Range - where damage falloff *starts*, not travel
    /// distance) - Rain of Arrows' spread-out sub-bullet for range.
    RainOfArrowsRange,
    /// `numShoot` - Rain of Arrows' sub-bullet controlling arrow count. Set
    /// to an absolute count rather than multiplied - the vanilla value is
    /// always 1, so "count" is a more honest ini key than a multiplier of an
    /// always-1 base.
    RainOfArrowsCount,
    /// `shootAngleYMaxRandom`/`shootAngleXMaxRandom` - Rain of Arrows'
    /// sub-bullet controlling the spread cone's angle.
    RainOfArrowsSpread,
}

/// The offsets that exist for a full-featured arrow (every AOW it can carry).
/// `+0` = the arrow itself, `+11/+20/+30` = Mighty Shot/Barrage/Enchanted
/// Shot (all just faster, like the base arrow), `+50..+54` = Rain of Arrows
/// split across 3 sub-bullets, `+70` = Through and Through.
const ARROW_OFFSETS: &[(u32, Role)] = &[
    (0, Role::Speed),
    (11, Role::Speed),
    (20, Role::Speed),
    (30, Role::Speed),
    (50, Role::Speed),
    (51, Role::RainOfArrowsRange),
    (52, Role::RainOfArrowsCount),
    (54, Role::RainOfArrowsSpread),
    (70, Role::Speed),
];

/// Great Arrows can't be fired with Mighty Shot/Barrage/Enchanted Shot, so
/// they skip straight from the base arrow to Rain of Arrows.
const GREAT_ARROW_OFFSETS: &[(u32, Role)] = &[
    (0, Role::Speed),
    (50, Role::Speed),
    (51, Role::RainOfArrowsRange),
    (52, Role::RainOfArrowsCount),
    (54, Role::RainOfArrowsSpread),
    (70, Role::Speed),
];

/// Radahn's Spear (Lion Greatbow's exclusive ammo) is the one irregular
/// case: no `+0`/`+70` rows exist for it at all - only Rain of Arrows.
const RADAHNS_SPEAR_OFFSETS: &[(u32, Role)] = &[
    (50, Role::Speed),
    (51, Role::RainOfArrowsRange),
    (52, Role::RainOfArrowsCount),
    (54, Role::RainOfArrowsSpread),
];

/// Bolts (crossbow/ballista ammo) never carry an Ash of War - just the one
/// plain bullet.
const BOLT_OFFSETS: &[(u32, Role)] = &[(0, Role::Speed)];

/// Base IDs for every regular Arrow type (Arrow, Fire Arrow, Serpent Arrow,
/// the various bone arrows, ...). Each one combines with [ARROW_OFFSETS].
const ARROW_BASES: &[u32] = &[
    20000000, 20000100, 20000200, 20000300, 20000400, 20000500, 20000600, 20000700, 20000800, 20000900,
    20001000, 20001100, 20001200, 20001300, 20001400, 20001500, 20001600, 20001700, 20001800, 20001900,
    20002000, 20002100, 20002300, 20002400, 20002500, 20002600, 20002700, 20002800, 20002900, 20003000,
    20003100, 20003200,
];

/// Base IDs for every Great Arrow type. Combines with [GREAT_ARROW_OFFSETS].
/// Note the gap at 20005400 - that base belongs to Radahn's Spear instead
/// (see [RADAHNS_SPEAR_BASE]), which doesn't have a `+0` row of its own.
const GREAT_ARROW_BASES: &[u32] = &[20005000, 20005100, 20005200, 20005300, 20005500, 20005600];

const RADAHNS_SPEAR_BASE: u32 = 20005400;

/// Base IDs for every Bolt type (crossbow bolts and ballista bolts/greatbolts
/// alike). Combines with [BOLT_OFFSETS].
const BOLT_BASES: &[u32] = &[
    20007000, 20007100, 20007200, 20007300, 20007400, 20007500, 20007600, 20007700, 20007800, 20007900,
    20008000, 20008100, 20008200, 20008300, 20008400, 20008500, 20008600, 20008700, 20008800, 20008900,
    20009000, 20009100, 20009200, 20009300,
];

struct Config {
    speed_multiplier: f64,
    range_multiplier: f64,
    spread_multiplier: f64,
    rain_of_arrows_count: u16,
    debug_log: bool,
}

impl Config {
    fn load() -> Self {
        Self {
            speed_multiplier: config::get_double("Bullet.SpeedMultiplier", 3.5),
            range_multiplier: config::get_double("Bullet.RangeMultiplier", 2.0),
            spread_multiplier: config::get_double("Bullet.SpreadMultiplier", 2.0),
            rain_of_arrows_count: config::get_int("Bullet.RainOfArrowsCount", 6).clamp(0, u16::MAX as i32) as u16,
            debug_log: config::get_bool("LogFile", false),
        }
    }
}

fn scale_f32(value: f32, factor: f64) -> f32 {
    (value as f64 * factor) as f32
}

/// The game's own original values RiseArcher scales multiplicatively, keyed
/// by row ID - captured once (lazily, on the first call to [apply]) before
/// any edit, so every later `ReloadKey` press (see `crate::reload`) rescales
/// from the true baseline instead of compounding - same reasoning as
/// `weapon::Baseline`. `num_shoot` (used by [Role::RainOfArrowsCount]) is set
/// to an absolute count rather than scaled, so it needs no baseline.
#[derive(Clone, Copy)]
struct Baseline {
    init_vellocity: f32,
    max_vellocity: f32,
    dist: f32,
    angle_y: f32,
    angle_x: f32,
}

static ORIGINALS: Mutex<Option<HashMap<u32, Baseline>>> = Mutex::new(None);

fn snapshot(row: &BULLET_PARAM_ST) -> Baseline {
    Baseline {
        init_vellocity: row.init_vellocity(),
        max_vellocity: row.max_vellocity(),
        dist: row.dist(),
        angle_y: row.shoot_angle_y_max_random(),
        angle_x: row.shoot_angle_x_max_random(),
    }
}

fn apply_role(row: &mut BULLET_PARAM_ST, role: Role, baseline: &Baseline, cfg: &Config) {
    match role {
        Role::Speed => {
            row.set_init_vellocity(scale_f32(baseline.init_vellocity, cfg.speed_multiplier));
            row.set_max_vellocity(scale_f32(baseline.max_vellocity, cfg.speed_multiplier));
        }
        Role::RainOfArrowsRange => {
            row.set_dist(scale_f32(baseline.dist, cfg.range_multiplier));
        }
        Role::RainOfArrowsCount => {
            row.set_num_shoot(cfg.rain_of_arrows_count);
        }
        Role::RainOfArrowsSpread => {
            row.set_shoot_angle_y_max_random(scale_f32(baseline.angle_y, cfg.spread_multiplier));
            row.set_shoot_angle_x_max_random(scale_f32(baseline.angle_x, cfg.spread_multiplier));
        }
    }
}

/// Every Bullet ID RiseArcher tunes, paired with which fields to touch
/// there - the flattened combination of every family's bases and offsets.
fn all_ids() -> Vec<(u32, Role)> {
    let mut ids = Vec::new();
    for &base in ARROW_BASES {
        for &(offset, role) in ARROW_OFFSETS {
            ids.push((base + offset, role));
        }
    }
    for &base in GREAT_ARROW_BASES {
        for &(offset, role) in GREAT_ARROW_OFFSETS {
            ids.push((base + offset, role));
        }
    }
    for &(offset, role) in RADAHNS_SPEAR_OFFSETS {
        ids.push((RADAHNS_SPEAR_BASE + offset, role));
    }
    for &base in BOLT_BASES {
        for &(offset, role) in BOLT_OFFSETS {
            ids.push((base + offset, role));
        }
    }
    ids
}

/// Applies every RiseArcher bullet tuning to the live `Bullet` rows, always
/// derived from each row's cached original values (see [Baseline]) so this
/// is safe to call repeatedly (hot reload) without compounding. Returns how
/// many rows were actually found and touched, for logging - a lower count
/// than expected is the signal that the ID layout has drifted (game update
/// added/removed a variant) and [ARROW_BASES]/[GREAT_ARROW_BASES]/
/// [BOLT_BASES] need re-checking against a fresh CSV export.
pub fn apply(repo: &mut SoloParamRepository) -> usize {
    let cfg = Config::load();
    let ids = all_ids();

    let mut snapshot_guard = ORIGINALS.lock().unwrap();
    let originals = snapshot_guard.get_or_insert_with(|| {
        let mut map = HashMap::new();
        for &(id, _role) in &ids {
            if let Some(row) = repo.get_mut::<Bullet>(id) {
                map.insert(id, snapshot(row));
            }
        }
        map
    });

    let mut changed = 0;
    let mut missing = 0;
    for &(id, role) in &ids {
        match (repo.get_mut::<Bullet>(id), originals.get(&id)) {
            (Some(row), Some(baseline)) => {
                apply_role(row, role, baseline, &cfg);
                changed += 1;
            }
            _ => missing += 1,
        }
    }

    if missing > 0 {
        logger::log(&format!(
            "WARNING: {missing} Bullet ID(s) from RiseArcher's known ID list weren't found in this game version - ID layout may have drifted, see bullet.rs."
        ));
    }
    if cfg.debug_log {
        logger::log(&format!("Bullet: applied to {changed} row(s), {missing} expected ID(s) missing."));
    }

    changed
}
