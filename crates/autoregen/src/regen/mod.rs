//! Tick-based HP/FP/Stamina regen, ported from the original AutoRegen's
//! RegenEngine.cpp/AttackHook.cpp - same feature, same ini keys, but reading
//! HP/FP/Stamina through fromsoftware-rs's real `CSChrDataModule` fields
//! instead of hand-maintained byte offsets (`CHR_PATTERN` + `WalkPointerChain`),
//! and running as a task registered on the game's own per-frame scheduler
//! (`CSTaskImp`) instead of a separate sleeping OS thread - `main_player` is
//! only safe to mutate from the game's main thread, which is exactly where
//! `CSTaskGroupIndex::FrameBegin` tasks run. Ported from LifeBetween's
//! `src/regen/mod.rs`, the reference implementation this rewrite is based on.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp, WorldChrMan};
use eldenring::util::input;
use fromsoftware_shared::{FromStatic, SharedTaskImpExt};

mod attack_hook;

use common::config;
use common::input::parse_virtual_key;
use common::logger;

const VK_F5: i32 = 0x74;

/// Heals `flat_amount` plus `percent_fraction` of max (e.g. 0.01 = 1%, already
/// divided by 100), clamped to max, with at least 1 point restored if the
/// percent alone would round to 0. Returns the amount actually restored (0 if
/// the player isn't resolved, is dead, or the heal is a no-op).
fn apply_heal(current: &mut i32, max: i32, flat_amount: i32, percent_fraction: f64) -> i32 {
    let percent_heal = if percent_fraction > 0.0 {
        ((percent_fraction * max as f64) as i32).max(1)
    } else {
        0
    };
    let heal = flat_amount + percent_heal;
    if heal <= 0 || *current >= max {
        return 0;
    }
    let before = *current;
    *current = (*current + heal).min(max);
    *current - before
}

#[derive(Clone, Copy)]
pub enum HealField {
    Hp,
    Fp,
    Stamina,
}

/// Applies a heal to the resolved main player, given the same
/// `(flat_amount, percent_fraction)` convention as [apply_heal]. No-op
/// (returns 0) if the player isn't currently resolved or is dead - shared by
/// the tick loop below and by `attack_hook`'s heal-on-hit/heal-on-damage.
pub fn heal_main_player(field: HealField, flat_amount: i32, percent_fraction: f64) -> i32 {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
        return 0;
    };
    let Some(main_player) = world_chr_man.main_player.as_mut() else {
        return 0;
    };
    let data = &mut main_player.chr_ins.modules.data;
    if data.hp <= 0 {
        return 0; // dead - don't touch anything
    }

    match field {
        HealField::Hp => {
            let max = data.max_hp;
            apply_heal(&mut data.hp, max, flat_amount, percent_fraction)
        }
        HealField::Fp => {
            let max = data.max_fp;
            apply_heal(&mut data.fp, max, flat_amount, percent_fraction)
        }
        HealField::Stamina => {
            let max = data.max_stamina;
            apply_heal(&mut data.stamina, max, flat_amount, percent_fraction)
        }
    }
}

/// Returns the main player's `ChrIns` address, used by `attack_hook` to
/// confirm a hit's attacker/target is the player themselves. `None` if not
/// resolved yet (title screen, loading, ...).
pub fn main_player_chr_ins_ptr() -> Option<*const u8> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    world_chr_man
        .main_player
        .as_ref()
        .map(|p| &p.chr_ins as *const _ as *const u8)
}

/// Registers the tick regen as a recurring task on the game's own
/// `FrameBegin` task group and blocks the calling thread forever watching for
/// `ReloadKey`. Meant to run on its own worker thread spawned from `DllMain`;
/// never returns.
pub fn run(ini_path: String) {
    let cs_task = match CSTaskImp::wait_for_instance(Duration::MAX) {
        Ok(instance) => instance,
        Err(err) => {
            logger::log(&format!("ERROR: CSTaskImp never became available ({err:?}) - AutoRegen disabled for this session."));
            return;
        }
    };

    let mut elapsed_ms: f64 = 0.0;
    let mut attack_hook_installed = false;

    let _handle = cs_task.run_recurring(
        move |data: &eldenring::fd4::FD4TaskData| {
            // ReloadKey - already debounced by eldenring::util::input, so this
            // fires once per physical press regardless of how many frames the
            // key stays down.
            let reload_key = parse_virtual_key(&config::get_string("ReloadKey", "F5"), VK_F5);
            if input::is_key_pressed(reload_key) {
                config::load(&ini_path);
                logger::log("Config reloaded (hotkey pressed).");
            }

            // Heal-on-hit/heal-on-damage are independent of this tick's own
            // interval (see attack_hook.rs) - installed once we're in-game so
            // other mods that scan/patch the same game code get to finish
            // their own startup scans first, and re-synced every tick so a
            // hot reload updates them without reinstalling the hook.
            let hook_params = attack_hook::HookParams {
                on_hit: attack_hook::OnHitParams {
                    hp_flat: config::get_int("HpOnHit", 0),
                    fp_flat: config::get_int("FpOnHit", 0),
                    stamina_flat: config::get_int("StaminaOnHit", 0),
                    hp_pct: config::get_double("HpPctOnHit", 0.0) / 100.0,
                    fp_pct: config::get_double("FpPctOnHit", 0.0) / 100.0,
                    stamina_pct: config::get_double("StaminaPctOnHit", 0.0) / 100.0,
                },
                on_damage: attack_hook::OnDamageParams {
                    hp_flat: config::get_int("HpOnDamage", 0),
                    fp_flat: config::get_int("FpOnDamage", 0),
                    stamina_flat: config::get_int("StaminaOnDamage", 0),
                },
            };
            let chr_resolved = main_player_chr_ins_ptr().is_some();
            if hook_params.wants_hook() && chr_resolved && !attack_hook_installed {
                attack_hook_installed = attack_hook::install(hook_params);
            } else if attack_hook_installed {
                attack_hook::update_params(hook_params);
            }

            // Interval=0 disables the whole tick-based regen feature, same
            // "0 disables" convention as every other key in this section.
            let interval_ms = config::get_int("Interval", 1000).max(0) as f64;
            if interval_ms <= 0.0 {
                return;
            }

            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < interval_ms {
                return;
            }
            elapsed_ms = 0.0;

            // Percent values are given directly as a percent (1 = 1%), so
            // divide by 100 to get the fraction of max restored per tick.
            // Flat values are an absolute amount, independent of max. Both
            // apply together, so either can be left at 0 to use only the
            // other.
            let hp_flat = config::get_int("Hp", 0);
            let fp_flat = config::get_int("Fp", 0);
            let stamina_flat = config::get_int("Stamina", 0);
            let hp_fraction = config::get_double("HpPct", 0.0) / 100.0;
            let fp_fraction = config::get_double("FpPct", 0.0) / 100.0;
            let stamina_fraction = config::get_double("StaminaPct", 0.0) / 100.0;

            heal_main_player(HealField::Hp, hp_flat, hp_fraction);
            heal_main_player(HealField::Fp, fp_flat, fp_fraction);
            heal_main_player(HealField::Stamina, stamina_flat, stamina_fraction);
        },
        CSTaskGroupIndex::FrameBegin,
    );

    logger::log("Regen tick registered on CSTaskGroupIndex::FrameBegin.");

    // `_handle` cancels the recurring task if dropped - park this thread
    // forever so it stays alive for the lifetime of the DLL.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
