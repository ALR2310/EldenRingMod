//! Lets every weapon be enchanted - spells that add a damage-boosting aura
//! to the weapon (e.g. Bloodflame Blade, Order's Blade, Scholar's
//! Armament), matching the game's own wording ("This weapon cannot be
//! enchanted") - NOT weapon affinity/infusion, despite how similar those
//! two terms sound in English. The other half of the Nexus mod "Unlocked
//! Ashes of War and Enchantments" (nexusmods.com/eldenring/mods/271) - see
//! [super::unlock_ashes_of_war] for the Ash-of-War-mounting half
//! (`gemMountType`/`canMountWep_*`).
//!
//! `EquipParamWeapon::isEnhance` is what actually gates whether a weapon
//! can be enchanted at all - forced to `1` on EVERY row (3554/3554 in the
//! reference dump).
//!
//! Applied once at startup only, no hot-reload (same convention as
//! `misc::unlock_ashes_of_war`/`misc::weight_multiplier`/
//! `misc::torrent_anywhere`) - restart the game with
//! `UnlockEnchantments=false` to go back to vanilla.

use std::time::Duration;

use eldenring::cs::{EquipParamWeapon, SoloParamRepository};

use common::config;
use common::logger;

/// Sets every `EquipParamWeapon` row's `isEnhance`. Returns the number of
/// rows touched, for logging.
fn apply(repo: &mut SoloParamRepository) -> usize {
    let mut weapons = 0;
    for (_, row) in repo.rows_mut::<EquipParamWeapon>() {
        row.set_is_enhance(true);
        weapons += 1;
    }
    weapons
}

/// Applies the param edit once, waiting (up to 300s) for
/// `SoloParamRepository` to actually be populated
/// (`player::wait_for_solo_param_repository`). Meant to run on its own
/// worker thread spawned from `DllMain`; returns once done (no tick/hotkey
/// loop for this module).
pub fn run() {
    if !config::get_bool("UnlockEnchantments", false) {
        logger::log("UnlockEnchantments=false - skipping entirely at startup.");
        return;
    }

    let Some(repo) = crate::player::wait_for_solo_param_repository(Duration::from_secs(300)) else {
        logger::log("ERROR: SoloParamRepository never became available - UnlockEnchantments disabled for this session.");
        return;
    };

    let weapons = apply(repo);
    logger::log(&format!("UnlockEnchantments: isEnhance=1 applied to {weapons} EquipParamWeapon row(s)."));
}
