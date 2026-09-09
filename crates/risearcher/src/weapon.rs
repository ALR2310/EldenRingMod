//! Applies RiseArcher's bow/crossbow/ballista/arrow/bolt buffs directly to
//! the live `EquipParamWeapon` regulation rows, ported from
//! `csv/RiseArcher.MASSEDIT` - same filters (weaponCategory/wepType/sortId),
//! same fields, but every multiplier now comes from the ini instead of being
//! hardcoded into the Mass Edit command text.
//!
//! `sortId == 9999999` is FromSoftware's own convention marking an NPC-only
//! duplicate row (hidden from the player's menu) - always excluded, exactly
//! like the original `!prop sortId 9999999` filter on every Mass Edit line.

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{EquipParamWeapon, SoloParamRepository};
use eldenring::param::EQUIP_PARAM_WEAPON_ST;

use common::config;

const NPC_ONLY_SORT_ID: i32 = 9999999;
const WEAPON_CATEGORY_BOW: u8 = 10;
const WEAPON_CATEGORY_CROSSBOW_FAMILY: u8 = 11; // Crossbow AND Ballista share this category, split by wepType below
const WEP_TYPE_CROSSBOW: u16 = 55;
const WEP_TYPE_BALLISTA: u16 = 56;
const WEAPON_CATEGORY_ARROW: u8 = 13;
const WEAPON_CATEGORY_BOLT: u8 = 14;

/// Which of RiseArcher's 5 weapon buckets a row falls into, resolved once
/// per row from `weaponCategory`/`wepType` - the single source of truth for
/// both what counts as "relevant" ([is_relevant]) and which ini multipliers
/// apply ([apply]), instead of repeating the same category/wepType match in
/// both places.
#[derive(Clone, Copy)]
enum WeaponKind {
    Bow,
    Crossbow,
    Ballista,
    Arrow,
    Bolt,
}

fn weapon_kind(category: u8, wep_type: u16) -> Option<WeaponKind> {
    match category {
        WEAPON_CATEGORY_BOW => Some(WeaponKind::Bow),
        WEAPON_CATEGORY_CROSSBOW_FAMILY if wep_type == WEP_TYPE_CROSSBOW => Some(WeaponKind::Crossbow),
        WEAPON_CATEGORY_CROSSBOW_FAMILY if wep_type == WEP_TYPE_BALLISTA => Some(WeaponKind::Ballista),
        WEAPON_CATEGORY_ARROW => Some(WeaponKind::Arrow),
        WEAPON_CATEGORY_BOLT => Some(WeaponKind::Bolt),
        _ => None,
    }
}

struct Config {
    bow_damage_multiplier: f64,
    bow_scaling_multiplier: f64,
    bow_unlock_aow: bool,
    bow_weight_multiplier: f64,
    bow_sell_value_multiplier: f64,
    crossbow_damage_multiplier: f64,
    crossbow_weight_multiplier: f64,
    crossbow_sell_value_multiplier: f64,
    ballista_damage_multiplier: f64,
    ballista_weight_multiplier: f64,
    ballista_sell_value_multiplier: f64,
    arrow_max_quantity: u8,
    bolt_max_quantity: u8,
}

impl Config {
    fn load() -> Self {
        Self {
            bow_damage_multiplier: config::get_double("Bow.DamageMultiplier", 1.5),
            bow_scaling_multiplier: config::get_double("Bow.ScalingMultiplier", 2.0),
            bow_unlock_aow: config::get_bool("Bow.UnlockAOW", true),
            bow_weight_multiplier: config::get_double("Bow.WeightMultiplier", 0.5),
            bow_sell_value_multiplier: config::get_double("Bow.SellValueMultiplier", 2.0),
            crossbow_damage_multiplier: config::get_double("Crossbow.DamageMultiplier", 3.0),
            crossbow_weight_multiplier: config::get_double("Crossbow.WeightMultiplier", 0.5),
            crossbow_sell_value_multiplier: config::get_double("Crossbow.SellValueMultiplier", 2.0),
            ballista_damage_multiplier: config::get_double("Ballista.DamageMultiplier", 4.0),
            ballista_weight_multiplier: config::get_double("Ballista.WeightMultiplier", 0.5),
            ballista_sell_value_multiplier: config::get_double("Ballista.SellValueMultiplier", 2.0),
            arrow_max_quantity: config::get_int("Bullet.Arrow.MaxQuantity", 255).clamp(0, 255) as u8,
            bolt_max_quantity: config::get_int("Bullet.Bolt.MaxQuantity", 255).clamp(0, 255) as u8,
        }
    }
}

/// The game's own original values RiseArcher scales multiplicatively, keyed
/// by row ID - captured once (lazily, on the first call to [apply]) before
/// any edit, so every later `ReloadKey` press (see `crate::reload`) rescales
/// from the true baseline instead of compounding (2x then reload at 2x again
/// would become 4x, not stay at 2x) - same pattern as
/// `sometweaks::drop_rate`'s own original-weights snapshot.
#[derive(Clone, Copy)]
struct Baseline {
    attack: [u16; 5],
    correct: [f32; 5],
    weight: f32,
    sell_value: i32,
}

// Recovered with `unwrap_or_else(|poisoned| poisoned.into_inner())` at every
// lock site rather than a plain `.unwrap()` - `lib.rs::apply_with_retry`
// retries `apply` across a `catch_unwind` boundary (regulation.bin's param
// resource files can still be loading when this first runs - see its doc
// comment), and a panic while this mutex was held would otherwise poison it
// permanently, turning every retry's own `.lock().unwrap()` into a second,
// unrelated panic before it ever got a chance to try again.
static ORIGINALS: Mutex<Option<HashMap<u32, Baseline>>> = Mutex::new(None);

/// Whether `row` is one RiseArcher touches at all - excludes the NPC-only
/// duplicate rows and every weapon category outside the bow/crossbow/
/// ballista/arrow/bolt family.
fn is_relevant(row: &EQUIP_PARAM_WEAPON_ST) -> bool {
    row.sort_id() != NPC_ONLY_SORT_ID && weapon_kind(row.weapon_category(), row.wep_type()).is_some()
}

fn snapshot(row: &EQUIP_PARAM_WEAPON_ST) -> Baseline {
    Baseline {
        attack: [
            row.attack_base_physics(),
            row.attack_base_magic(),
            row.attack_base_fire(),
            row.attack_base_thunder(),
            row.attack_base_dark(),
        ],
        correct: [
            row.correct_strength(),
            row.correct_agility(),
            row.correct_magic(),
            row.correct_faith(),
            row.correct_luck(),
        ],
        weight: row.weight(),
        sell_value: row.sell_value(),
    }
}

fn scale_u16(value: u16, factor: f64) -> u16 {
    (value as f64 * factor).round().clamp(0.0, u16::MAX as f64) as u16
}

fn scale_f32(value: f32, factor: f64) -> f32 {
    (value as f64 * factor) as f32
}

fn scale_i32(value: i32, factor: f64) -> i32 {
    (value as f64 * factor).round() as i32
}

fn apply_damage_multiplier(row: &mut EQUIP_PARAM_WEAPON_ST, attack: [u16; 5], factor: f64) {
    row.set_attack_base_physics(scale_u16(attack[0], factor));
    row.set_attack_base_magic(scale_u16(attack[1], factor));
    row.set_attack_base_fire(scale_u16(attack[2], factor));
    row.set_attack_base_thunder(scale_u16(attack[3], factor));
    row.set_attack_base_dark(scale_u16(attack[4], factor));
}

fn apply_weight_and_sell_value(row: &mut EQUIP_PARAM_WEAPON_ST, baseline: &Baseline, weight_mult: f64, sell_mult: f64) {
    row.set_weight(scale_f32(baseline.weight, weight_mult));
    row.set_sell_value(scale_i32(baseline.sell_value, sell_mult));
}

fn apply_bow(row: &mut EQUIP_PARAM_WEAPON_ST, baseline: &Baseline, cfg: &Config) {
    apply_damage_multiplier(row, baseline.attack, cfg.bow_damage_multiplier);
    row.set_correct_strength(scale_f32(baseline.correct[0], cfg.bow_scaling_multiplier));
    row.set_correct_agility(scale_f32(baseline.correct[1], cfg.bow_scaling_multiplier));
    row.set_correct_magic(scale_f32(baseline.correct[2], cfg.bow_scaling_multiplier));
    row.set_correct_faith(scale_f32(baseline.correct[3], cfg.bow_scaling_multiplier));
    row.set_correct_luck(scale_f32(baseline.correct[4], cfg.bow_scaling_multiplier));
    if cfg.bow_unlock_aow {
        row.set_gem_mount_type(2); // normalizes every bow to accept an Ash of War - some vanilla bows ship with this at 0
    }
    apply_weight_and_sell_value(row, baseline, cfg.bow_weight_multiplier, cfg.bow_sell_value_multiplier);
}

/// Applies every RiseArcher weapon buff to the live `EquipParamWeapon` rows,
/// always derived from each row's cached original values (see [Baseline]) so
/// this is safe to call repeatedly (hot reload) without compounding. Returns
/// how many rows were touched, for logging.
pub fn apply(repo: &mut SoloParamRepository) -> usize {
    let cfg = Config::load();

    let mut snapshot_guard = ORIGINALS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let originals = snapshot_guard.get_or_insert_with(|| {
        repo.rows_mut::<EquipParamWeapon>()
            .filter(|(_, row)| is_relevant(row))
            .map(|(id, row)| (id, snapshot(row)))
            .collect()
    });

    let mut changed = 0;
    for (id, row) in repo.rows_mut::<EquipParamWeapon>() {
        let Some(baseline) = originals.get(&id) else {
            continue; // not one of RiseArcher's rows (see is_relevant)
        };
        let Some(kind) = weapon_kind(row.weapon_category(), row.wep_type()) else {
            continue; // shouldn't happen - baseline is only ever inserted for a recognized kind
        };

        match kind {
            WeaponKind::Bow => apply_bow(row, baseline, &cfg),
            WeaponKind::Crossbow => {
                apply_damage_multiplier(row, baseline.attack, cfg.crossbow_damage_multiplier);
                apply_weight_and_sell_value(row, baseline, cfg.crossbow_weight_multiplier, cfg.crossbow_sell_value_multiplier);
            }
            WeaponKind::Ballista => {
                apply_damage_multiplier(row, baseline.attack, cfg.ballista_damage_multiplier);
                apply_weight_and_sell_value(row, baseline, cfg.ballista_weight_multiplier, cfg.ballista_sell_value_multiplier);
            }
            WeaponKind::Arrow => row.set_max_arrow_quantity(cfg.arrow_max_quantity),
            WeaponKind::Bolt => row.set_max_arrow_quantity(cfg.bolt_max_quantity),
        }

        changed += 1;
    }

    changed
}
