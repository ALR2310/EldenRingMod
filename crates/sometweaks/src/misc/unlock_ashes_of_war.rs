//! Unlocks every Ash of War for every weapon category, and lets every
//! weapon have an Ash of War mounted at all - ported from the Nexus mod
//! "Unlocked Ashes of War and Enchantments"
//! (nexusmods.com/eldenring/mods/271), whose CSV export I diffed directly
//! (`EquipParamWeapon.csv`/`EquipParamGem.csv`) rather than reverse
//! engineering, since it ships as a plain `regulation.bin` param edit with
//! no code patch involved. Split from that mod's single combined toggle
//! into 2 independent features here - see [super::unlock_enchantments] for
//! the other half (`isEnhance`, which governs affinity/infusion, not Ash
//! of War mounting):
//!
//! - `EquipParamWeapon`: EVERY row (3554/3554 in the reference dump, no
//!   exceptions - not even ammo) gets `gemMountType=2`.
//! - `EquipParamGem` (Ash of War definitions - keeps Dark Souls 3's "Gem"
//!   naming): EVERY row gets all 44 `canMountWep_*` weapon-category
//!   compatibility flags (Dagger/SwordNormal/.../Bow.../Staff/Shield/
//!   Torch/...) forced to `1` - every Ash of War becomes valid on every
//!   weapon category, including ones with no vanilla Ash of War support
//!   at all (bows, shields, torches, ...). Whether the game actually has a
//!   matching moveset/animation for e.g. an Ash of War on a bow is
//!   untested - this only removes the game's own compatibility check, it
//!   doesn't add missing animations.
//!
//! Applied once at startup only, no hot-reload (same convention as
//! `misc::weight_multiplier`/`misc::torrent_anywhere`) - reverting ~3800
//! rows' worth of edits on toggle isn't worth the snapshot bookkeeping
//! `drop_rate` needs for its much smaller single-field edit; restart the
//! game with `UnlockAshesOfWar=false` to go back to vanilla.

use std::time::Duration;

use eldenring::cs::{EquipParamGem, EquipParamWeapon, SoloParamRepository};

use common::config;
use common::logger;

/// Sets every `EquipParamWeapon` row's `gemMountType`, and every
/// `EquipParamGem` row's `canMountWep_*` flags. Returns
/// `(weapon_rows_touched, gem_rows_touched)` for logging.
fn apply(repo: &mut SoloParamRepository) -> (usize, usize) {
    let mut weapons = 0;
    for (_, row) in repo.rows_mut::<EquipParamWeapon>() {
        row.set_gem_mount_type(2);
        weapons += 1;
    }

    let mut gems = 0;
    for (_, row) in repo.rows_mut::<EquipParamGem>() {
        row.set_can_mount_wep_dagger(true);
        row.set_can_mount_wep_sword_normal(true);
        row.set_can_mount_wep_sword_large(true);
        row.set_can_mount_wep_sword_gigantic(true);
        row.set_can_mount_wep_saber_normal(true);
        row.set_can_mount_wep_saber_large(true);
        row.set_can_mount_wep_katana(true);
        row.set_can_mount_wep_sword_double_edge(true);
        row.set_can_mount_wep_sword_pierce(true);
        row.set_can_mount_wep_rapier_heavy(true);
        row.set_can_mount_wep_axe_normal(true);
        row.set_can_mount_wep_axe_large(true);
        row.set_can_mount_wep_hammer_normal(true);
        row.set_can_mount_wep_hammer_large(true);
        row.set_can_mount_wep_flail(true);
        row.set_can_mount_wep_spear_normal(true);
        row.set_can_mount_wep_spear_large(true);
        row.set_can_mount_wep_spear_heavy(true);
        row.set_can_mount_wep_spear_axe(true);
        row.set_can_mount_wep_sickle(true);
        row.set_can_mount_wep_knuckle(true);
        row.set_can_mount_wep_claw(true);
        row.set_can_mount_wep_whip(true);
        row.set_can_mount_wep_axhammer_large(true);
        row.set_can_mount_wep_bow_small(true);
        row.set_can_mount_wep_bow_normal(true);
        row.set_can_mount_wep_bow_large(true);
        row.set_can_mount_wep_closs_bow(true);
        row.set_can_mount_wep_ballista(true);
        row.set_can_mount_wep_staff(true);
        row.set_can_mount_wep_sorcery(true);
        row.set_can_mount_wep_talisman(true);
        row.set_can_mount_wep_shield_small(true);
        row.set_can_mount_wep_shield_normal(true);
        row.set_can_mount_wep_shield_large(true);
        row.set_can_mount_wep_torch(true);
        row.set_can_mount_wep_hand_to_hand(true);
        row.set_can_mount_wep_perfume_bottle(true);
        row.set_can_mount_wep_thrusting_shield(true);
        row.set_can_mount_wep_throwing_weapon(true);
        row.set_can_mount_wep_reverse_hand_sword(true);
        row.set_can_mount_wep_light_greatsword(true);
        row.set_can_mount_wep_great_katana(true);
        row.set_can_mount_wep_beast_claw(true);
        gems += 1;
    }

    (weapons, gems)
}

/// Applies the param edits once, waiting (up to 300s) for
/// `SoloParamRepository` to actually be populated
/// (`player::wait_for_solo_param_repository`). Meant to run on its own
/// worker thread spawned from `DllMain`; returns once done (no tick/hotkey
/// loop for this module).
pub fn run() {
    if !config::get_bool("UnlockAshesOfWar", false) {
        logger::log("UnlockAshesOfWar=false - skipping entirely at startup.");
        return;
    }

    let Some(repo) = crate::player::wait_for_solo_param_repository(Duration::from_secs(300)) else {
        logger::log("ERROR: SoloParamRepository never became available - UnlockAshesOfWar disabled for this session.");
        return;
    };

    let (weapons, gems) = apply(repo);
    logger::log(&format!(
        "UnlockAshesOfWar: gemMountType=2 applied to {weapons} EquipParamWeapon row(s), all canMountWep_* flags applied to {gems} EquipParamGem row(s)."
    ));
}
