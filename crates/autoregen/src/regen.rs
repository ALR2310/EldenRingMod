//! Tick-based HP/FP/Stamina regen. Reads HP/FP/Stamina through
//! [`fromsoftware-rs`](https://github.com/vswarte/fromsoftware-rs)'s real
//! `CSChrDataModule` fields instead of hand-maintained byte offsets, and runs
//! as a task registered on the game's own per-frame scheduler (`CSTaskImp`)
//! instead of a separate sleeping OS thread - `main_player` is only safe to
//! mutate from the game's main thread, which is exactly where
//! `CSTaskGroupIndex::FrameBegin` tasks run.
//!
//! Config shape (`Regen.PerTick.*`/`Regen.PerHit.*`, `Enabled`/`Trigger`/
//! `Unit`) matches [`SomeTweaks`](../sometweaks)'s `regen` module - that
//! crate had in turn started as a straight port of AutoRegen's own older
//! `Condition`/`HpPctOnHit`-style ini (separate flat/percent keys per stat,
//! no `Enabled` flag), then evolved its own cleaner design (one value field
//! per stat + a `Unit`/`Trigger` selector for what it means, plus an
//! explicit `Enabled` instead of "0 disables everything"). Backported here so
//! both mods share the same config shape and code, rather than AutoRegen
//! being stuck with the older design it started from.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use eldenring::cs::{AnnounceNotification, CSMenuManImp, CSTaskGroupIndex, CSTaskImp, MenuString, WorldChrMan};
use eldenring::dlkr::DLAllocator;
use eldenring::dltx::DLString;
use eldenring::util::input;
use eldenring::util::system::wait_for_system_init;
use fromsoftware_shared::{FromStatic, Program, RecurringTaskHandle, SharedTaskImpExt};

use crate::attack_hook;
use common::config;
use common::input::parse_virtual_key;
use common::logger;

const VK_F5: i32 = 0x74;

// How long a hit landed or taken counts as "in combat" for
// Regen.PerTick.Trigger=1/2, before it's considered over. There's no
// reliable "is the player currently fighting" flag exposed by the game
// engine itself, so this approximates it the way many action games do:
// combat is "active" for a grace period after the last confirmed hit, rather
// than instantaneous.
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

#[derive(Clone, Copy)]
pub enum HealField {
    Hp,
    Fp,
    Stamina,
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

/// Shows `text` in the game's own top-of-screen system announcement banner
/// (the same widget used for things like "Autosaving...") - queued onto
/// `CSMenuMan`'s `FeSystemAnnounceViewModel`, so the game's existing
/// fade-in/scroll/fade-out playback handles displaying and dismissing it, no
/// timer of our own needed. No-op (silently) if `CSMenuMan` isn't resolved
/// yet or the string fails to encode - a missed reload confirmation isn't
/// worth a log line.
fn show_announcement(text: &str) {
    let Ok(menu_man) = (unsafe { CSMenuManImp::instance_mut() }) else {
        return;
    };
    let allocator = DLAllocator::runtime_heap_allocator();
    let Ok(allocated_string) = DLString::from_str(text, allocator) else {
        return;
    };
    menu_man.system_announce_view_model.notifications.push_back(AnnounceNotification {
        is_active: true,
        message: MenuString {
            static_string: std::ptr::null(),
            allocated_string,
        },
    });
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

/// The pad-input bits read each frame to decide `LAST_ATTACK_WAS_SKILL`,
/// `IS_SITTING` and `is_idle()` below.
///
/// `r1`/`r2`/`l1`/`l2`/`new_gesture` come from `new_action_presses` (fires
/// exactly 1 frame, on the press) - confirmed in-game (2026-09-03) to
/// correlate with Ash of War/skill hits far more reliably than any
/// `AtkParam` field does (see AutoRegen's README). `requested_gesture` is a
/// plain value (not a bit), only meaningful the same frame `new_gesture` is
/// set. `busy` covers every other way the player can be "not idle" - held
/// (not just newly-pressed) actions from `action_requests`, plus actual
/// movement input - read from the same module so idle detection doesn't
/// need yet another `WorldChrMan::instance()` call of its own.
struct ActionSnapshot {
    r1: bool,
    r2: bool,
    l1: bool,
    l2: bool,
    new_gesture: bool,
    requested_gesture: i32,
    busy: bool,
}

/// Reads this frame's action-input snapshot (see [`ActionSnapshot`]) off the
/// main player's `CSChrActionRequestModule`. `None` if not resolved yet.
fn main_player_action_snapshot() -> Option<ActionSnapshot> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    let main_player = world_chr_man.main_player.as_ref()?;
    let action_request = &main_player.chr_ins.modules.action_request;
    let new_presses = &action_request.new_action_presses;
    let held = &action_request.action_requests;
    let busy = action_request.movement_request_flags.raw_input()
        || held.r1()
        || held.r2()
        || held.l1()
        || held.l2()
        || held.sp_move()
        || held.jump()
        || held.use_item()
        || held.action()
        || held.guard()
        || held.rideon()
        || held.rideoff()
        || held.ladderup()
        || held.ladderdown();
    Some(ActionSnapshot {
        r1: new_presses.r1(),
        r2: new_presses.r2(),
        l1: new_presses.l1(),
        l2: new_presses.l2(),
        new_gesture: new_presses.gesture(),
        requested_gesture: action_request.requested_gesture,
        busy,
    })
}

// Whether the last attack-starting button the player pressed was L2 (Skill/
// Weapon Art), as opposed to R1/R2/L1 (a plain attack) - in Elden Ring, the
// Skill button is L2 regardless of which hand is currently active/2-handed,
// L1 is only ever a left-hand light attack or guard/parry, never a skill
// trigger, so grouping it with R1/R2 is correct, not a gap. Confirmed
// in-game (2026-09-03): a multi-hit Ash of War's later hits land well after
// L2 is released, so checking the CURRENT l2 bit only catches the earliest
// hit of a skill - this instead latches onto the last actual button PRESS
// (new_*, not the held state) and keeps that answer until a different
// attack button is pressed, so every hit in between (however delayed)
// still reads correctly.
static LAST_ATTACK_WAS_SKILL: AtomicBool = AtomicBool::new(false);

// Default `Gesture.SittingId` for every "sitting" gesture: Prayer, Desperate
// Prayer, Dejection, Patches' Crouch, Crossed Legs, Rest, Sitting Sideways,
// Dozing Cross-Legged, Spread Out, Balled Up.
//
// These are HALF the GESTURE_ID values shown in the public GESTURE_ID
// dropdown in The Grand Archives' Elden Ring Cheat Engine table
// (github.com/The-Grand-Archives/Elden-Ring-CT-TGA) - e.g. that table lists
// Dejection as 160, but `requested_gesture` reads back 80 in-game (confirmed
// 2026-09-10 via the `RegenLog` debug line below: Dejection=80, Rest=92,
// Sitting Sideways=93, each exactly that table's ID / 2). fromsoftware-rs
// doesn't ship a named enum for these, and no public source documents this
// /2 factor - it was reverse-engineered from live log output, not read off
// any reference, so treat any ID from that table as needing /2 first, not
// as a literal `requested_gesture` value. Deliberately excludes "Fetal
// Position" (192 in that table, 96 here) - a separate, visually similar
// gesture Kolagon's request (see README) didn't list. Kept editable via ini
// rather than hard-coded so users can add/remove entries themselves - e.g.
// after a future game update adds a new gesture this list hasn't been
// updated for yet - without needing a new DLL build.
const DEFAULT_SIT_GESTURE_IDS: &str = "80,90,91,92,93,94,95,97,100,101";

/// Parses `Gesture.SittingId` (comma-separated GESTURE_ID values, see
/// `DEFAULT_SIT_GESTURE_IDS`) fresh from config - only called the 1 frame a
/// gesture is newly requested (see `update_last_attack_input`), not every
/// frame, so re-parsing instead of caching is cheap. Unparseable entries
/// (typos, stray commas) are silently skipped rather than failing the whole
/// list.
fn sit_gesture_ids() -> Vec<i32> {
    config::get_string("Gesture.SittingId", DEFAULT_SIT_GESTURE_IDS)
        .split(',')
        .filter_map(|id| id.trim().parse::<i32>().ok())
        .collect()
}

// Whether the player's currently-latched gesture is one of `SIT_GESTURE_IDS`
// - same latch approach as `LAST_ATTACK_WAS_SKILL`, since `requested_gesture`
// is only valid the 1 frame `new_action_presses.gesture()` fires, not for as
// long as the sit animation keeps playing. Cleared the moment `busy` is true
// (movement or any other held action) rather than waiting for a specific
// "gesture ended" signal - fromsoftware-rs exposes no such signal, and any
// of those inputs already cancels the sit animation in-game anyway.
static IS_SITTING: AtomicBool = AtomicBool::new(false);

// Timestamp (`now_ms()`) the player became idle (`busy` went false), or 0
// while currently busy - backs `is_idle()`'s `IDLE_GRACE_MS` delay below.
// 0 doubles as "currently busy" since `now_ms()` is relative to process
// start and never 0 again after the first frame.
static IDLE_SINCE_MS: AtomicU64 = AtomicU64::new(0);

// How long the player must stand completely still before Regen.PerTick.
// Trigger=3 (idle) starts applying - avoids topping off on every brief pause
// between actions (e.g. mid-fight positioning) being treated as "idle".
// Requested (2026-09-10) after testing Trigger=3 with no delay at all.
const IDLE_GRACE_MS: u64 = 5000;

/// Updates `LAST_ATTACK_WAS_SKILL`, `IS_SITTING` and `IDLE_SINCE_MS` from
/// this frame's action input. No-op if the player isn't resolved yet. Called
/// every frame, independent of any Regen.PerHit/PerTick config.
pub fn update_last_attack_input() {
    let Some(snapshot) = main_player_action_snapshot() else {
        return;
    };
    if snapshot.r1 || snapshot.r2 || snapshot.l1 {
        LAST_ATTACK_WAS_SKILL.store(false, Ordering::Relaxed);
    } else if snapshot.l2 {
        LAST_ATTACK_WAS_SKILL.store(true, Ordering::Relaxed);
    }

    if snapshot.new_gesture {
        // A 2nd gesture request while already sitting always cancels/stands
        // up in-game first, whether it's the same gesture or a different one
        // - it never switches straight into the newly-selected gesture.
        // Since this cancel fires the exact same signal (new_action_presses.
        // gesture() + requested_gesture=<id>) as starting one, the only way
        // to tell them apart is context: already sitting means this press
        // must be the cancel. Confirmed as a real bug in-game (2026-09-10):
        // without this check, re-pressing a gesture stands the player up but
        // IS_SITTING stayed true, so PerTick kept healing after they'd
        // already gotten up.
        let is_sit_gesture = !IS_SITTING.load(Ordering::Relaxed) && sit_gesture_ids().contains(&snapshot.requested_gesture);
        IS_SITTING.store(is_sit_gesture, Ordering::Relaxed);
        if config::get_bool("RegenLog", false) {
            logger::log(&format!(
                "Gesture: requested_gesture={} -> is_sitting={is_sit_gesture}",
                snapshot.requested_gesture
            ));
        }
    } else if snapshot.busy {
        IS_SITTING.store(false, Ordering::Relaxed);
    }

    if snapshot.busy {
        IDLE_SINCE_MS.store(0, Ordering::Relaxed);
    } else if IDLE_SINCE_MS.load(Ordering::Relaxed) == 0 {
        IDLE_SINCE_MS.store(now_ms(), Ordering::Relaxed);
    }
}

/// Whether the player has been doing nothing at all - no movement input, no
/// held attack/item/guard/mount action - for at least `IDLE_GRACE_MS`.
/// `false` before the player has ever been resolved (matches
/// `is_in_combat()`'s "assume active" default).
pub fn is_idle() -> bool {
    let since = IDLE_SINCE_MS.load(Ordering::Relaxed);
    since != 0 && now_ms().saturating_sub(since) >= IDLE_GRACE_MS
}

/// Whether the player's last-requested gesture is a sitting one and hasn't
/// been interrupted since (see `IS_SITTING`).
pub fn is_sitting() -> bool {
    IS_SITTING.load(Ordering::Relaxed)
}

/// Whether the player's currently-playing attack was started by the L2
/// (Skill/Weapon Art) button - see `LAST_ATTACK_WAS_SKILL`.
pub fn is_last_attack_skill() -> bool {
    LAST_ATTACK_WAS_SKILL.load(Ordering::Relaxed)
}

/// Waits for the earliest reliable "the game process is actually alive"
/// signal (`CSWindow`'s global hInstance, populated right after CRT init -
/// see the crate's own doc comment on this function), retrying past
/// `SystemInitError::InvalidRva`/`Timeout` instead of giving up. Ported
/// from `.docs/UltimatePassiveRegeneration` via `sometweaks::task`
/// (2026-08-26) - `fromsoftware-rs` already ships this helper for exactly
/// this purpose, AutoRegen just hadn't called it before, going straight
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
/// unpacking/relocating (e.g. Arxan), a timing race against how early this
/// DLL's worker thread happens to start, unrelated to where the DLL is
/// loaded from. Reported in the wild (Nexus comment, 2026-08-26): AutoRegen
/// disabled itself for the whole session on a one-off early poll. Retrying
/// here with a short delay rides out that race instead.
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
/// letting it unwind into the game's own call stack - `f` just gets
/// skipped for that one frame (logged), which is harmless here (no state
/// this tick keeps is unsafe to leave stale for a single frame). Ported
/// from `.docs/UltimatePassiveRegeneration` via `sometweaks::task`
/// (2026-08-26). Requires `[profile.release]`'s `panic = "abort"` to be
/// off (see workspace `Cargo.toml`) - `catch_unwind` cannot catch
/// anything once a panic aborts the process outright.
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
                logger::log("Regen tick panicked, skipped this frame.");
            }
        },
        group,
    )
}

/// Registers the Regen.* tick as a recurring task on the game's own
/// `FrameBegin` task group and blocks the calling thread forever watching for
/// `General.ReloadKey`. Meant to run on its own worker thread spawned from
/// `DllMain`; never returns.
pub fn run(ini_path: String) {
    let cs_task = wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    let mut attack_hook_installed = false;

    let _handle = run_recurring_safe(
        cs_task,
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            // Writes out any RegenLog lines the attack hook queued instead of
            // writing directly - see attack_hook::flush_pending_logs. Always
            // runs, every frame, regardless of what else below is enabled.
            attack_hook::flush_pending_logs();

            // Latches "was the last attack button pressed L2 (Skill)?" so
            // attack_hook can tell a skill hit from a plain one even several
            // frames after the button was released - see
            // LAST_ATTACK_WAS_SKILL. Also unconditional every frame.
            update_last_attack_input();

            // General.ReloadKey - already debounced by eldenring::util::input,
            // so this fires once per physical press regardless of how many
            // frames the key stays down.
            let reload_key = parse_virtual_key(&config::get_string("ReloadKey", "F5"), VK_F5);
            if input::is_key_pressed(reload_key) {
                config::load(&ini_path);
                logger::log("Config reloaded (hotkey pressed).");
                show_announcement("AutoRegen: config reloaded");
            }

            // Regen.PerTick.Trigger picks which player state the tick heal
            // below applies on: 0 = Always, 1 = out of combat only, 2 = in
            // combat only, 3 = idle only, 4 = sitting (via gesture) only.
            // Read up front since it also decides whether the attack hook
            // needs to be installed purely to track combat activity, even if
            // Regen Per Hit itself is disabled.
            let per_tick_enabled = config::get_bool("Regen.PerTick.Enabled", true);
            let condition = config::get_int("Regen.PerTick.Trigger", 0);
            let needs_combat_tracking = per_tick_enabled && (condition == 1 || condition == 2);

            // Regen Per Hit is independent of this tick's own interval (see
            // attack_hook.rs) - installed once we're in-game so other mods
            // that scan/patch the same game code get to finish their own
            // startup scans first, and re-synced every tick so a hot reload
            // updates it without reinstalling the hook.
            let on_hit_params = attack_hook::OnHitParams {
                enabled: config::get_bool("Regen.PerHit.Enabled", false),
                trigger: config::get_int("Regen.PerHit.Trigger", 0),
                damage_type: config::get_int("Regen.PerHit.DamageType", 0),
                exclude_aow: config::get_bool("Regen.PerHit.ExcludeAow", false),
                hp: config::get_double("Regen.PerHit.HP", 0.0),
                fp: config::get_double("Regen.PerHit.FP", 0.0),
                stamina: config::get_double("Regen.PerHit.SP", 0.0),
            };
            let chr_resolved = main_player_chr_ins_ptr().is_some();
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
                3 => is_idle(),
                4 => is_sitting(),
                _ => true,
            };
            if config::get_bool("RegenLog", false) && condition != 0 {
                logger::log(&format!(
                    "Regen.PerTick: trigger={condition} -> condition_met={condition_met} (in_combat={}, idle={}, sitting={})",
                    is_in_combat(),
                    is_idle(),
                    is_sitting()
                ));
            }
            if !condition_met {
                return;
            }

            // Regen.PerTick.Unit picks what the HP/FP/SP values below
            // mean: 0 = flat points, 1 = percent of max stat (divided by 100
            // to get the fraction restored per tick).
            let unit = config::get_int("Regen.PerTick.Unit", 0);
            let hp_value = config::get_double("Regen.PerTick.HP", 0.0);
            let fp_value = config::get_double("Regen.PerTick.FP", 0.0);
            let stamina_value = config::get_double("Regen.PerTick.SP", 0.0);
            let (hp_flat, hp_fraction) = split_by_unit(unit, hp_value);
            let (fp_flat, fp_fraction) = split_by_unit(unit, fp_value);
            let (stamina_flat, stamina_fraction) = split_by_unit(unit, stamina_value);

            let hp_healed = heal_main_player(HealField::Hp, hp_flat, hp_fraction);
            let fp_healed = heal_main_player(HealField::Fp, fp_flat, fp_fraction);
            let stamina_healed = heal_main_player(HealField::Stamina, stamina_flat, stamina_fraction);
            if config::get_bool("RegenLog", false) {
                logger::log(&format!(
                    "Regen.PerTick: +{hp_healed} HP, +{fp_healed} FP, +{stamina_healed} SP"
                ));
            }
        },
    );

    logger::log("Regen tick registered on CSTaskGroupIndex::FrameBegin.");

    // `_handle` cancels the recurring task if dropped - park this thread
    // forever so it stays alive for the lifetime of the DLL.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
