//! Applies RiseArcher's bow/crossbow/ballista/arrow/bolt buffs directly to
//! the live `EquipParamWeapon` regulation rows, ported from
//! `csv/RiseArcher.MASSEDIT` - same filters (weaponCategory/wepType/sortId),
//! same fields, but every multiplier now comes from the ini instead of being
//! hardcoded into the Mass Edit command text.
//!
//! `sortId == 9999999` is FromSoftware's own convention marking an NPC-only
//! duplicate row (hidden from the player's menu) - always excluded, exactly
//! like the original `!prop sortId 9999999` filter on every Mass Edit line.

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

struct Config {
    bow_damage_multiplier: f64,
    bow_scaling_multiplier: f64,
    bow_allow_ash_of_war: bool,
    crossbow_damage_multiplier: f64,
    ballista_damage_multiplier: f64,
    bow_family_weight_multiplier: f64,
    bow_family_sell_value_multiplier: f64,
    arrow_max_quantity: u8,
    bolt_max_quantity: u8,
}

impl Config {
    fn load() -> Self {
        Self {
            bow_damage_multiplier: config::get_double("Bow.DamageMultiplier", 1.5),
            bow_scaling_multiplier: config::get_double("Bow.ScalingMultiplier", 2.0),
            bow_allow_ash_of_war: config::get_bool("Bow.AllowAshOfWar", true),
            crossbow_damage_multiplier: config::get_double("Crossbow.DamageMultiplier", 3.0),
            ballista_damage_multiplier: config::get_double("Ballista.DamageMultiplier", 4.0),
            bow_family_weight_multiplier: config::get_double("BowCrossbowBallista.WeightMultiplier", 0.5),
            bow_family_sell_value_multiplier: config::get_double("BowCrossbowBallista.SellValueMultiplier", 2.0),
            arrow_max_quantity: config::get_int("Arrow.MaxQuantity", 255).clamp(0, 255) as u8,
            bolt_max_quantity: config::get_int("Bolt.MaxQuantity", 255).clamp(0, 255) as u8,
        }
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

fn apply_damage_multiplier(row: &mut EQUIP_PARAM_WEAPON_ST, factor: f64) {
    row.set_attack_base_physics(scale_u16(row.attack_base_physics(), factor));
    row.set_attack_base_magic(scale_u16(row.attack_base_magic(), factor));
    row.set_attack_base_fire(scale_u16(row.attack_base_fire(), factor));
    row.set_attack_base_thunder(scale_u16(row.attack_base_thunder(), factor));
    row.set_attack_base_dark(scale_u16(row.attack_base_dark(), factor));
}

fn apply_bow(row: &mut EQUIP_PARAM_WEAPON_ST, cfg: &Config) {
    apply_damage_multiplier(row, cfg.bow_damage_multiplier);
    row.set_correct_strength(scale_f32(row.correct_strength(), cfg.bow_scaling_multiplier));
    row.set_correct_agility(scale_f32(row.correct_agility(), cfg.bow_scaling_multiplier));
    row.set_correct_magic(scale_f32(row.correct_magic(), cfg.bow_scaling_multiplier));
    row.set_correct_faith(scale_f32(row.correct_faith(), cfg.bow_scaling_multiplier));
    row.set_correct_luck(scale_f32(row.correct_luck(), cfg.bow_scaling_multiplier));
    if cfg.bow_allow_ash_of_war {
        row.set_gem_mount_type(2); // normalizes every bow to accept an Ash of War - some vanilla bows ship with this at 0
    }
}

/// Applies every RiseArcher weapon buff to the live `EquipParamWeapon` rows.
/// Returns how many rows were touched, for logging.
pub fn apply(repo: &mut SoloParamRepository) -> usize {
    let cfg = Config::load();
    let mut changed = 0;

    for (_id, row) in repo.rows_mut::<EquipParamWeapon>() {
        if row.sort_id() == NPC_ONLY_SORT_ID {
            continue;
        }

        let category = row.weapon_category();
        let wep_type = row.wep_type();
        let is_bow_family = category == WEAPON_CATEGORY_BOW || category == WEAPON_CATEGORY_CROSSBOW_FAMILY;
        if !is_bow_family && category != WEAPON_CATEGORY_ARROW && category != WEAPON_CATEGORY_BOLT {
            continue;
        }

        match category {
            WEAPON_CATEGORY_BOW => apply_bow(row, &cfg),
            WEAPON_CATEGORY_CROSSBOW_FAMILY if wep_type == WEP_TYPE_CROSSBOW => {
                apply_damage_multiplier(row, cfg.crossbow_damage_multiplier);
            }
            WEAPON_CATEGORY_CROSSBOW_FAMILY if wep_type == WEP_TYPE_BALLISTA => {
                apply_damage_multiplier(row, cfg.ballista_damage_multiplier);
            }
            WEAPON_CATEGORY_ARROW => row.set_max_arrow_quantity(cfg.arrow_max_quantity),
            WEAPON_CATEGORY_BOLT => row.set_max_arrow_quantity(cfg.bolt_max_quantity),
            _ => {}
        }

        // Weight/sell-value only apply to Bow + Crossbow + Ballista (10|11),
        // not Arrow/Bolt (13/14) - matches the original's separate Mass Edit
        // filter line for this pair of fields.
        if is_bow_family {
            row.set_weight(scale_f32(row.weight(), cfg.bow_family_weight_multiplier));
            row.set_sell_value(scale_i32(row.sell_value(), cfg.bow_family_sell_value_multiplier));
        }

        changed += 1;
    }

    changed
}
