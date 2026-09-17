//! Tick-based HP/FP/Stamina regen. Reads HP/FP/Stamina through
//! [`fromsoftware-rs`](https://github.com/vswarte/fromsoftware-rs)'s real
//! `CSChrDataModule` fields instead of hand-maintained byte offsets, and runs
//! as a task registered on the game's own per-frame scheduler (`CSTaskImp`)
//! instead of a separate sleeping OS thread - `main_player` is only safe to
//! mutate from the game's main thread, which is exactly where
//! `CSTaskGroupIndex::FrameBegin` tasks run.
//!
//! Config shape (`Regen.PerTick.*`/`Regen.PerHit.*`, `Enabled`/`Trigger`/
//! `ValueType`/`Mode`) matches [`SomeTweaks`](../sometweaks)'s `regen`
//! module - that crate had in turn started as a straight port of AutoRegen's
//! own older `Condition`/`HpPctOnHit`-style ini (separate flat/percent keys
//! per stat, no `Enabled` flag), then evolved its own cleaner design (one
//! value field per stat + a `ValueType`/`Mode` selector for what it means, plus an
//! explicit `Enabled` instead of "0 disables everything"). Backported here so
//! both mods share the same config shape and code, rather than AutoRegen
//! being stuck with the older design it started from.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use eldenring::cs::{AnnounceNotification, CSMenuManImp, CSTaskGroupIndex, MenuString, WorldChrMan};
use eldenring::dltx::DLString;
use eldenring::util::input;
use fromsoftware_shared::FromStatic;

use crate::hit_hook;
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

/// Computes one `Regen.PerTick` heal amount, honoring `value_type`: `0` =
/// `value` as flat points, `1` = `value`% of max stat, `2` = `value`% of the
/// stat's *missing* amount (`max - current`) - heals fast while low, tapering
/// off as the stat approaches max instead of restoring the same amount
/// regardless of how full the stat already is. Modes `1`/`2` restore at
/// least 1 point if the percent alone would round to 0 (as long as `value` is
/// positive and there's still room to heal).
fn compute_tick_heal(value_type: i32, value: f64, current: i32, max: i32) -> i32 {
    match value_type {
        1 => {
            if value <= 0.0 {
                0
            } else {
                ((value / 100.0 * max as f64) as i32).max(1)
            }
        }
        2 => {
            if value <= 0.0 {
                0
            } else {
                let missing = (max - current) as f64;
                ((value / 100.0 * missing) as i32).max(1)
            }
        }
        _ => value.round() as i32,
    }
}

#[derive(Clone, Copy)]
pub enum HealField {
    Hp,
    Fp,
    Stamina,
}

/// Resolves the main player and gives `f` mutable access to `field`'s
/// `(current, max)` pair. `None` if the player isn't currently resolved or is
/// dead - shared by [heal_main_player] (attack-hook heal-on-hit) and
/// [heal_main_player_tick] (the tick loop below).
fn with_stat_mut<R>(field: HealField, f: impl FnOnce(&mut i32, i32) -> R) -> Option<R> {
    let world_chr_man = unsafe { WorldChrMan::instance_mut() }.ok()?;
    let main_player = world_chr_man.main_player.as_mut()?;
    let data = &mut main_player.chr_ins.modules.data;
    if data.hp <= 0 {
        return None; // dead - don't touch anything
    }
    Some(match field {
        HealField::Hp => {
            let max = data.max_hp;
            f(&mut data.hp, max)
        }
        HealField::Fp => {
            let max = data.max_fp;
            f(&mut data.fp, max)
        }
        HealField::Stamina => {
            let max = data.max_stamina;
            f(&mut data.stamina, max)
        }
    })
}

/// Applies a heal to the resolved main player, given the same
/// `(flat_amount, percent_fraction)` convention as [apply_heal]. No-op
/// (returns 0) if the player isn't currently resolved or is dead - used by
/// `hit_hook`'s heal-on-hit (`Regen.PerHit`, no cap/`ValueType=2` support).
pub fn heal_main_player(field: HealField, flat_amount: i32, percent_fraction: f64) -> i32 {
    with_stat_mut(field, |current, max| apply_heal(current, max, flat_amount, percent_fraction))
        .unwrap_or(0)
}

/// Applies one `Regen.PerTick` heal to the resolved main player - see
/// [compute_tick_heal] for what `value_type`/`value` mean. `cap_pct`
/// (`Regen.PerTick.Cap`, percent of the stat's true max) is a ceiling ticks
/// never restore past, independent of `value_type`: already at or above the
/// cap is a no-op even if the stat isn't at its true max yet. No-op (returns
/// 0) if the player isn't currently resolved or is dead.
pub fn heal_main_player_tick(field: HealField, value_type: i32, value: f64, cap_pct: f64) -> i32 {
    with_stat_mut(field, |current, max| {
        let cap = (((cap_pct / 100.0) * max as f64) as i32).clamp(0, max);
        if *current >= cap {
            return 0;
        }
        let heal = compute_tick_heal(value_type, value, *current, max);
        if heal <= 0 {
            return 0;
        }
        let before = *current;
        *current = (*current + heal).min(cap);
        *current - before
    })
    .unwrap_or(0)
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
    let Some(allocator) = engine::alloc_hook::runtime_heap_allocator() else {
        return;
    };
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

/// The pad-input bits read each frame to decide `LAST_ATTACK_WAS_SKILL`,
/// `IS_GESTURE_ACTIVE` and `is_idle()` below.
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
///
/// `new_gesture` alone isn't enough to know a sit actually started (a press
/// blocked mid-cast still fires it) - see `confirm_pending_gesture` for the
/// fields that verify what happens next.
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

// Default `Regen.PerTick.GestureId` for every "sitting" gesture: Prayer, Desperate
// Prayer, Dejection, Patches' Crouch, Crossed Legs, Rest, Sitting Sideways,
// Dozing Cross-Legged, Spread Out, Balled Up.
//
// These are HALF the GESTURE_ID values shown in the public GESTURE_ID
// dropdown in The Grand Archives' Elden Ring Cheat Engine table
// (github.com/The-Grand-Archives/Elden-Ring-CT-TGA) - e.g. that table lists
// Dejection as 160, but `requested_gesture` reads back 80 in-game (confirmed
// 2026-09-10 via the `LogFile` debug line below: Dejection=80, Rest=92,
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
const DEFAULT_GESTURE_IDS: &str = "80,90,91,92,93,94,95,97,100,101";

/// Parses `Regen.PerTick.GestureId` (comma-separated GESTURE_ID values, see
/// `DEFAULT_GESTURE_IDS`) fresh from config - only called the 1 frame a
/// gesture is newly requested (see `update_last_attack_input`), not every
/// frame, so re-parsing instead of caching is cheap. Unparseable entries
/// (typos, stray commas) are silently skipped rather than failing the whole
/// list. Renamed from `Gesture.SittingId` (2026-09-17) - this feature isn't
/// really about "sitting" specifically, it's "heal while a configured
/// gesture is active", defaulting to the sitting-family gestures below; the
/// old key still works via `config::migrate`'s generic `[Legacy]` handling.
fn gesture_trigger_ids() -> Vec<i32> {
    config::get_string("Regen.PerTick.GestureId", DEFAULT_GESTURE_IDS)
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
static IS_GESTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Reads the main player's currently-playing TAE animation ID
/// (`CSChrTimeActModule::anim_queue[read_idx].anim_id`). `None` if not
/// resolved yet. `0` = plain on-foot idle (confirmed in-game, 2026-09-17 -
/// see README) - used by `confirm_pending_gesture` below as the "didn't actually
/// transition" signal, without needing a full TAE-id -> gesture mapping
/// table (the approach abandoned for `Regen.PerTick.Trigger=4` itself, see
/// the "Thêm `Regen.PerTick.Trigger=3`" README entry).
fn current_anim_id() -> Option<i32> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    let main_player = world_chr_man.main_player.as_ref()?;
    let time_act = &main_player.chr_ins.modules.time_act;
    let read_idx = time_act.read_idx;
    Some(time_act.anim_queue[read_idx as usize % time_act.anim_queue.len()].anim_id)
}

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

// Timestamp (`now_ms()`) a gesture press matching `gesture_trigger_ids()` was
// seen while not already sitting, or 0 while no confirmation is pending -
// backs `confirm_pending_gesture`'s delayed check below. 0 doubles as "nothing
// pending" the same way `IDLE_SINCE_MS` doubles as "currently busy".
static PENDING_GESTURE_SINCE_MS: AtomicU64 = AtomicU64::new(0);

// `current_anim_id()` read at the exact moment `PENDING_GESTURE_SINCE_MS` was
// set - see `confirm_pending_gesture`. Only meaningful while a confirmation is
// pending (`PENDING_GESTURE_SINCE_MS != 0`), so its own idle value doesn't need
// a sentinel.
static PENDING_GESTURE_ANIM_ID: AtomicI32 = AtomicI32::new(0);

// `current_anim_id()` as of the last frame `confirm_pending_gesture` ran, and
// the timestamp (`now_ms()`) it last CHANGED - tracks how long the
// currently-playing animation has held steady, independent of
// `PENDING_SIT_*` (updated every frame, pending or not, so it's already
// warmed up whenever a new pending confirmation starts). 0/0 = never
// observed yet.
static LAST_SEEN_ANIM_ID: AtomicI32 = AtomicI32::new(0);
static LAST_SEEN_ANIM_SINCE_MS: AtomicU64 = AtomicU64::new(0);

// Minimum time to wait after a sit-gesture press before even looking -
// avoids a same-frame read that's technically already "different" purely
// from timing luck. Real sit/idle animations hold their id far longer than
// this either way, so this is just a floor, not the actual confirmation
// signal (see `ANIM_STABLE_MS`).
const GESTURE_CONFIRM_MIN_WAIT_MS: u64 = 200;

// How long `current_anim_id()` must have held the SAME value before trusting
// it - see `confirm_pending_gesture`. Added 2026-09-17 after in-game testing
// caught a real false-positive a single-sample check (the original version
// of this fix) missed: rapid-pressing gesture 3x (sit, stand-up cancel,
// sit again) could catch the read exactly mid-transition of the STANDING-UP
// animation (itself non-zero and different from whatever was playing at
// the 3rd press) and wrongly confirm sitting while the character was
// actually finishing standing up. A genuine sit/idle holds its id for
// seconds; transitional animations observed in-game changed roughly every
// 0.5-1s - 300ms initially, widened to 1000ms for extra margin against that
// (the resulting extra healing delay on a real sit is still imperceptible).
const ANIM_STABLE_MS: u64 = 1000;

// Give up waiting and reject a pending sit if `current_anim_id()` never
// settles (stops changing) within this long after the press - prevents
// `PENDING_GESTURE_SINCE_MS` from lingering forever if the animation stays in
// flux (e.g. repeated interruptions) without ever tripping the `busy`
// cancel. Kept a comfortable margin above `ANIM_STABLE_MS` (3x) so a
// slower-settling transition still has room to be confirmed rather than
// timing out right as it stabilizes.
const GESTURE_CONFIRM_TIMEOUT_MS: u64 = 3000;

/// Updates `LAST_SEEN_ANIM_ID`/`LAST_SEEN_ANIM_SINCE_MS` from
/// `current_anim_id()` - called every frame (pending confirmation or not) so
/// `confirm_pending_gesture` always has an up-to-date "how long has this held
/// steady" answer instead of only starting to track once a press happens.
fn track_current_anim() {
    let Some(anim_id) = current_anim_id() else {
        return;
    };
    if LAST_SEEN_ANIM_ID.swap(anim_id, Ordering::Relaxed) != anim_id {
        LAST_SEEN_ANIM_SINCE_MS.store(now_ms(), Ordering::Relaxed);
    }
}

/// 4th attempt at fixing the Kolagon mid-cast false-positive (see the
/// investigation note on `ActionSnapshot` and README, 2026-09-14/15/17) -
/// this one doesn't try to predict from a field read at the exact press
/// frame (all 3 prior attempts confirmed-failed that way). Instead: a sit
/// gesture press only starts a PENDING confirmation (`PENDING_GESTURE_SINCE_MS`/
/// `PENDING_GESTURE_ANIM_ID`) instead of latching `IS_GESTURE_ACTIVE` immediately; once
/// `current_anim_id()` (via `track_current_anim`) has held STEADY
/// (`ANIM_STABLE_MS`) at a value that's both non-zero and different from
/// what was playing at press time, that's trusted as a real transition and
/// `IS_GESTURE_ACTIVE` is set. Requiring STABILITY (not just a single differing
/// sample, which an earlier version of this fix used) matters for 2
/// separate false-positives, both confirmed in-game (2026-09-17, see
/// README):
/// - A press blocked mid-cast: the spellcast anim doesn't move for well over
///   a single sample's delay, so "differs from press-time id" alone
///   eventually fails once the cast finishes and falls back to idle (`0`) -
///   but that transient `0` is also technically "different", so a
///   single-sample check checking only "!= 0" (this fix's 1st version)
///   already covered this specific case; stability isn't strictly needed
///   for it.
/// - Rapid re-presses (sit, stand-up cancel, sit again): a single-sample
///   check could catch the read exactly mid-transition of the STANDING-UP
///   animation (non-zero, differs from the 3rd press's id) and wrongly
///   confirm sitting while the character was still in the middle of
///   standing up - this is what stability actually guards against; a
///   genuine settled sit/idle holds its id for seconds, a transition passes
///   through several ids well under `ANIM_STABLE_MS`.
///
/// Called every frame regardless of `new_gesture` since confirmation
/// resolves on LATER frames than the press itself.
fn confirm_pending_gesture(busy: bool) {
    let pending_since = PENDING_GESTURE_SINCE_MS.load(Ordering::Relaxed);
    if pending_since == 0 {
        return;
    }
    if busy {
        // Any held movement/action before confirmation already cancels the
        // attempt in-game - same rule `IS_GESTURE_ACTIVE` itself follows elsewhere.
        PENDING_GESTURE_SINCE_MS.store(0, Ordering::Relaxed);
        return;
    }
    let elapsed = now_ms().saturating_sub(pending_since);
    if elapsed < GESTURE_CONFIRM_MIN_WAIT_MS {
        return;
    }
    let press_anim_id = PENDING_GESTURE_ANIM_ID.load(Ordering::Relaxed);
    let anim_id = LAST_SEEN_ANIM_ID.load(Ordering::Relaxed);
    let anim_since = LAST_SEEN_ANIM_SINCE_MS.load(Ordering::Relaxed);
    let stable_for = now_ms().saturating_sub(anim_since);
    if anim_id != 0 && anim_id != press_anim_id && stable_for >= ANIM_STABLE_MS {
        PENDING_GESTURE_SINCE_MS.store(0, Ordering::Relaxed);
        IS_GESTURE_ACTIVE.store(true, Ordering::Relaxed);
        if config::get_bool("LogFile", false) {
            logger::log(&format!(
                "Gesture: confirm anim_id={anim_id} stable_for={stable_for}ms (was {press_anim_id} at press) -> is_gesture_active=true"
            ));
        }
    } else if elapsed >= GESTURE_CONFIRM_TIMEOUT_MS {
        PENDING_GESTURE_SINCE_MS.store(0, Ordering::Relaxed);
        IS_GESTURE_ACTIVE.store(false, Ordering::Relaxed);
        if config::get_bool("LogFile", false) {
            logger::log(&format!(
                "Gesture: confirm timed out, anim_id={anim_id} stable_for={stable_for}ms (was {press_anim_id} at press) -> is_gesture_active=false"
            ));
        }
    }
    // Otherwise: still waiting for `anim_id` to settle - try again next frame.
}

/// Updates `LAST_ATTACK_WAS_SKILL`, `IS_GESTURE_ACTIVE` and `IDLE_SINCE_MS` from
/// this frame's action input. No-op if the player isn't resolved yet. Called
/// every frame, independent of any Regen.PerHit/PerTick config.
pub fn update_last_attack_input() {
    let Some(snapshot) = main_player_action_snapshot() else {
        return;
    };
    track_current_anim();
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
        // IS_GESTURE_ACTIVE stayed true, so PerTick kept healing after they'd
        // already gotten up. This cancel path is immediate, unlike starting
        // a new sit (see `confirm_pending_gesture`) - standing up is confirmed
        // by definition, nothing to wait on.
        if IS_GESTURE_ACTIVE.load(Ordering::Relaxed) {
            IS_GESTURE_ACTIVE.store(false, Ordering::Relaxed);
            PENDING_GESTURE_SINCE_MS.store(0, Ordering::Relaxed);
            if config::get_bool("LogFile", false) {
                logger::log(&format!(
                    "Gesture: requested_gesture={} -> is_gesture_active=false (cancel)",
                    snapshot.requested_gesture
                ));
            }
        } else if PENDING_GESTURE_SINCE_MS.load(Ordering::Relaxed) != 0 {
            // A repeat press while the FIRST attempt hasn't even been
            // confirmed yet - ignored, deliberately NOT restarted from this
            // press. An earlier version restarted `PENDING_GESTURE_ANIM_ID` here,
            // which broke on a real rapid double-press in-game (2026-09-17):
            // by the 2nd press the animation had already transitioned partway
            // into sitting down, so the new baseline captured THAT
            // in-progress id - once the sit fully settled, it no longer
            // looked "different" from its own already-transitioning
            // baseline, so it silently never confirmed at all despite the
            // character visibly ending up seated. Leaving the ORIGINAL
            // (pre-transition) baseline in place lets the same pending
            // attempt confirm correctly once things settle; if the repeat
            // press was actually meant to stand back up, that's caught for
            // free once `IS_GESTURE_ACTIVE` does become `true` - the branch above
            // then cancels it immediately on the next press, same as always.
            if config::get_bool("LogFile", false) {
                logger::log(&format!(
                    "Gesture: requested_gesture={} -> ignored (still pending confirm)",
                    snapshot.requested_gesture
                ));
            }
        } else if gesture_trigger_ids().contains(&snapshot.requested_gesture) {
            let press_anim_id = current_anim_id().unwrap_or(0);
            PENDING_GESTURE_ANIM_ID.store(press_anim_id, Ordering::Relaxed);
            PENDING_GESTURE_SINCE_MS.store(now_ms().max(1), Ordering::Relaxed);
            if config::get_bool("LogFile", false) {
                logger::log(&format!(
                    "Gesture: requested_gesture={} anim_id={press_anim_id} -> pending confirm",
                    snapshot.requested_gesture
                ));
            }
        }
    } else if snapshot.busy {
        IS_GESTURE_ACTIVE.store(false, Ordering::Relaxed);
    }
    confirm_pending_gesture(snapshot.busy);

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

/// Whether the player's last-requested gesture matches
/// `Regen.PerTick.GestureId` (defaults to the sitting-family gestures) and
/// hasn't been interrupted since (see `IS_GESTURE_ACTIVE`).
pub fn is_gesture_active() -> bool {
    IS_GESTURE_ACTIVE.load(Ordering::Relaxed)
}

/// Whether the player's currently-playing attack was started by the L2
/// (Skill/Weapon Art) button - see `LAST_ATTACK_WAS_SKILL`.
pub fn is_last_attack_skill() -> bool {
    LAST_ATTACK_WAS_SKILL.load(Ordering::Relaxed)
}

/// Registers the Regen.* tick as a recurring task on the game's own
/// `FrameBegin` task group and blocks the calling thread forever watching for
/// `General.ReloadKey`. Meant to run on its own worker thread spawned from
/// `DllMain`; never returns.
///
/// `CSTaskImp` acquisition and the panic-safe wrapper both come from the
/// shared `engine` crate now (2026-09-14) - see its `task` module doc
/// comment for why this no longer needs its own copy of either (this crate
/// is where both were originally written, before being extracted once a
/// second/third mod needed the exact same thing).
pub fn run(ini_path: String) {
    let cs_task = engine::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    let mut attack_hook_installed = false;
    let mut last_logged_tick_state: Option<(i32, bool)> = None;

    engine::task::run_recurring_safe(
        cs_task,
        "Regen",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            // Writes out any LogFile lines the hit hook queued instead of
            // writing directly - see hit_hook::flush_pending_logs. Always
            // runs, every frame, regardless of what else below is enabled.
            hit_hook::flush_pending_logs();

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
            // combat only, 3 = idle only, 4 = while a configured gesture is
            // active only (see `Regen.PerTick.GestureId`, defaults to the
            // sitting-family gestures).
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
            let on_hit_params = hit_hook::OnHitParams {
                enabled: config::get_bool("Regen.PerHit.Enabled", false),
                trigger: config::get_int("Regen.PerHit.Mode", 0),
                damage_type: config::get_int("Regen.PerHit.DamageType", 0),
                exclude_aow: config::get_bool("Regen.PerHit.ExcludeAow", false),
                hp: config::get_double("Regen.PerHit.HP", 0.0),
                fp: config::get_double("Regen.PerHit.FP", 0.0),
                stamina: config::get_double("Regen.PerHit.SP", 0.0),
            };
            let chr_resolved = engine::player::main_player_chr_ins_ptr().is_some();
            let hook_wanted = on_hit_params.wants_heal() || needs_combat_tracking;
            if hook_wanted && chr_resolved && !attack_hook_installed {
                attack_hook_installed = hit_hook::try_install(on_hit_params);
            } else if attack_hook_installed {
                hit_hook::update_params(on_hit_params);
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
                4 => is_gesture_active(),
                _ => true,
            };
            if config::get_bool("LogFile", false) && condition != 0 {
                // Only log when `condition_met` itself (the actual outcome -
                // does this tick heal or not) CHANGES, not any time
                // `in_combat`/`idle`/`gesture_active` shift on their own - an
                // earlier version deduped on the whole tuple, which still
                // logged a fresh line every time the player simply moved
                // then stopped (each toggling `idle` even when it doesn't
                // change `condition_met` at all, e.g. `Trigger=1`/`2`) -
                // flooded LogFile over a long session just as badly as the
                // original per-tick spam this dedup was meant to fix
                // (reported 2026-09-14, this narrower version 2026-09-17).
                // The detail fields are still read fresh and included in
                // whichever line DOES get logged, for context.
                let state = (condition, condition_met);
                if last_logged_tick_state != Some(state) {
                    last_logged_tick_state = Some(state);
                    let in_combat = is_in_combat();
                    let idle = is_idle();
                    let gesture_active = is_gesture_active();
                    logger::log(&format!(
                        "Regen.PerTick: trigger={condition} -> condition_met={condition_met} (in_combat={in_combat}, idle={idle}, gesture_active={gesture_active})"
                    ));
                }
            }
            if !condition_met {
                return;
            }

            // Regen.PerTick.ValueType picks what the HP/FP/SP values below
            // mean: 0 = flat points, 1 = percent of max stat, 2 = percent of
            // the stat's missing amount (max - current) - see
            // compute_tick_heal. Regen.PerTick.Cap (percent of true max) caps
            // how far any of the 3 stats gets restored by this tick,
            // regardless of ValueType or Trigger.
            let unit = config::get_int("Regen.PerTick.ValueType", 0);
            let cap_pct = config::get_double("Regen.PerTick.Cap", 100.0);
            let hp_value = config::get_double("Regen.PerTick.HP", 0.0);
            let fp_value = config::get_double("Regen.PerTick.FP", 0.0);
            let stamina_value = config::get_double("Regen.PerTick.SP", 0.0);

            heal_main_player_tick(HealField::Hp, unit, hp_value, cap_pct);
            heal_main_player_tick(HealField::Fp, unit, fp_value, cap_pct);
            heal_main_player_tick(HealField::Stamina, unit, stamina_value, cap_pct);
        },
    );

    logger::log("Regen tick registered on CSTaskGroupIndex::FrameBegin.");

    // `_handle` cancels the recurring task if dropped - park this thread
    // forever so it stays alive for the lifetime of the DLL.
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
