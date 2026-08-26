//! Tick-based HP/FP/Stamina regen, ported from AutoRegen's RegenEngine.cpp -
//! same feature, same Regen.* ini keys, but reading HP/FP/Stamina through
//! fromsoftware-rs's real `CSChrDataModule` fields instead of hand-maintained
//! byte offsets, and running as a task registered on the game's own per-frame
//! scheduler (`CSTaskImp`) instead of a separate sleeping OS thread - `main_player`
//! is only safe to mutate from the game's main thread, which is exactly where
//! `CSTaskGroupIndex::FrameBegin` tasks run.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use fromsoftware_shared::FromStatic;

mod attack_hook;

use common::config;
use common::logger;

// How long a hit landed or taken counts as "in combat" for Regen.PerTick's
// Trigger=1/2, before it's considered over. There's no reliable "is the
// player currently fighting" flag exposed by the game engine itself, so this
// approximates it the way many action games do: combat is "active" for a
// grace period after the last confirmed hit, rather than instantaneous.
const COMBAT_TIMEOUT_MS: u64 = 15000;

static LAST_COMBAT_ACTIVITY_MS: AtomicU64 = AtomicU64::new(0);

fn clock_start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

fn now_ms() -> u64 {
    clock_start().elapsed().as_millis() as u64
}

/// Called by `attack_hook` whenever the player lands or takes a confirmed
/// hit - resets the combat timer regardless of whether any heal is
/// configured for that hit.
pub fn mark_combat_activity() {
    LAST_COMBAT_ACTIVITY_MS.store(now_ms(), Ordering::Relaxed);
}

/// Whether the player is considered "in combat" right now: a hit was landed
/// or taken within the last `COMBAT_TIMEOUT_MS`. `false` before any combat
/// activity has ever been recorded this session.
pub fn is_in_combat() -> bool {
    let last = LAST_COMBAT_ACTIVITY_MS.load(Ordering::Relaxed);
    last != 0 && now_ms().saturating_sub(last) < COMBAT_TIMEOUT_MS
}

/// Splits a `Regen.PerTick.Unit`-tagged ini value into the `(flat_amount,
/// percent_fraction)` pair [apply_heal] expects: `Unit=0` treats `value` as
/// flat points, `Unit=1` as a percent of max (divided by 100 into a
/// fraction). Only one of the pair is ever non-zero, since the ini has a
/// single field per stat rather than separate flat/percent keys.
fn split_by_unit(unit: i32, value: f64) -> (i32, f64) {
    if unit == 1 {
        (0, value / 100.0)
    } else {
        (value.round() as i32, 0.0)
    }
}

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

/// Applies a heal to the resolved main player, given the same
/// `(flat_amount, percent_fraction)` convention as [apply_heal]. No-op
/// (returns 0) if the player isn't currently resolved or is dead - shared by
/// the tick loop below and by `attack_hook`'s heal-on-hit.
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

#[derive(Clone, Copy)]
pub enum HealField {
    Hp,
    Fp,
    Stamina,
}

/// Registers the Regen.* tick as a recurring task on the game's own
/// `FrameBegin` task group. Meant to run on its own worker thread spawned
/// from `DllMain`; never returns.
pub fn run() {
    let cs_task = crate::task::wait_for_cs_task("Regen");

    let mut elapsed_ms: f64 = 0.0;
    let mut attack_hook_installed = false;

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "Regen",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            // Regen.PerTick.Trigger picks which side of combat the tick heal
            // below applies on: 0 = Always, 1 = out of combat only, 2 = in
            // combat only. Read up front since it also decides whether the
            // attack hook needs to be installed purely to track combat
            // activity, even if Regen Per Hit itself is disabled.
            let per_tick_enabled = config::get_bool("Regen.PerTick.Enabled", true);
            let condition = config::get_int("Regen.PerTick.Trigger", 0);
            let needs_combat_tracking = per_tick_enabled && (condition == 1 || condition == 2);

            // Heal-on-hit is independent of this tick's own interval (see
            // attack_hook.rs) - installed once we're in-game so other mods that
            // scan/patch the same game code get to finish their own startup
            // scans first, and re-synced every tick so a hot reload updates it
            // without reinstalling the hook.
            let on_hit_params = attack_hook::OnHitParams {
                enabled: config::get_bool("Regen.PerHit.Enabled", false),
                trigger: config::get_int("Regen.PerHit.Trigger", 0),
                damage_type: config::get_int("Regen.PerHit.DamageType", 0),
                hp: config::get_double("Regen.PerHit.HP", 0.0),
                fp: config::get_double("Regen.PerHit.FP", 0.0),
                stamina: config::get_double("Regen.PerHit.Stamina", 0.0),
            };
            let chr_resolved = crate::player::main_player_chr_ins_ptr().is_some();
            let hook_wanted = on_hit_params.wants_heal() || needs_combat_tracking;
            if hook_wanted && chr_resolved && !attack_hook_installed {
                attack_hook_installed = attack_hook::install(on_hit_params);
            } else if attack_hook_installed {
                attack_hook::update_params(on_hit_params);
            }

            // Regen.PerTick.Enabled=false or Interval=0 disables the whole
            // tick-based regen feature, same "0 disables" convention as every
            // other key in this section.
            if !per_tick_enabled {
                return;
            }
            let interval_ms = config::get_int("Regen.PerTick.Interval", 1000).max(0) as f64;
            if interval_ms <= 0.0 {
                return;
            }

            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < interval_ms {
                return;
            }
            elapsed_ms = 0.0;

            let condition_met = match condition {
                1 => !is_in_combat(),
                2 => is_in_combat(),
                _ => true,
            };
            if !condition_met {
                return;
            }

            // Regen.PerTick.Unit picks what the HP/FP/Stamina values below
            // mean: 0 = flat points, 1 = percent of max stat (divided by 100
            // to get the fraction restored per tick).
            let unit = config::get_int("Regen.PerTick.Unit", 0);
            let hp_value = config::get_double("Regen.PerTick.HP", 0.0);
            let fp_value = config::get_double("Regen.PerTick.FP", 0.0);
            let stamina_value = config::get_double("Regen.PerTick.Stamina", 0.0);
            let (hp_flat, hp_fraction) = split_by_unit(unit, hp_value);
            let (fp_flat, fp_fraction) = split_by_unit(unit, fp_value);
            let (stamina_flat, stamina_fraction) = split_by_unit(unit, stamina_value);

            heal_main_player(HealField::Hp, hp_flat, hp_fraction);
            heal_main_player(HealField::Fp, fp_flat, fp_fraction);
            heal_main_player(HealField::Stamina, stamina_flat, stamina_fraction);
        },
    );

    logger::log("Regen tick registered on CSTaskGroupIndex::FrameBegin.");

    // `_handle` cancels the recurring task if dropped - park this thread
    // forever so it stays alive for the lifetime of the DLL.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
