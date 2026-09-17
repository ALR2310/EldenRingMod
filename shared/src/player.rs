//! Shared "is the player actually in the game world" resolution. Every
//! feature that touches a singleton whose data isn't populated until a real
//! player character exists (`GameDataMan`'s save data, `SoloParamRepository`'s
//! param tables) must gate on this, not just on the singleton's own
//! `instance()`/`instance_mut()` returning `Ok` - that alone only means the
//! manager object exists, which happens as soon as a save slot loads, still
//! at the title/loading screen well before its data is actually populated
//! (confirmed the hard way, in `sometweaks`: premature rune grants in
//! `rune::reward`, and a crash in `drop_rate` before `SoloParamRepository`
//! was gated the same way, 2026-08-24/25).
//!
//! Ported from `sometweaks::player`/`risearcher::player` (both carried an
//! identical copy) once a third mod needed the same gate.

use std::time::Duration;

use eldenring::cs::{SoloParamRepository, WorldChrMan};
use fromsoftware_shared::FromStatic;

/// The pad-input bits read each frame off the main player's own
/// `CSChrActionRequestModule` - shared by every feature that needs to
/// classify player input (e.g. "was that hit a plain attack or an Ash of
/// War/Skill?", "is the player idle?", "did they just start a gesture?").
///
/// `r1`/`r2`/`l1`/`l2`/`new_gesture` come from `new_action_presses` (fires
/// exactly 1 frame, on the press) - confirmed in-game (2026-09-03) to
/// correlate with Ash of War/Skill hits far more reliably than any
/// `AtkParam` field does (see AutoRegen's README). `requested_gesture` is a
/// plain value (not a bit), only meaningful the same frame `new_gesture` is
/// set. `busy` covers every other way the player can be "not idle" - held
/// (not just newly-pressed) actions from `action_requests`, plus actual
/// movement input - read from the same module so a caller checking idle
/// state doesn't need yet another `WorldChrMan::instance()` call of its own.
///
/// Ported from `autoregen::regen`'s `ActionSnapshot` (2026-09-17) once
/// `sometweaks::player`'s own narrower `NewActionPresses` (just `r1`/`r2`/
/// `l1`/`l2`, for telling a Skill hit from a plain one) turned out to be a
/// strict subset of the exact same fields, read off the exact same struct -
/// callers that only need the 4 press bits can just ignore the rest.
pub struct ActionSnapshot {
    pub r1: bool,
    pub r2: bool,
    pub l1: bool,
    pub l2: bool,
    pub new_gesture: bool,
    pub requested_gesture: i32,
    pub busy: bool,
}

/// Reads this frame's action-input snapshot (see [ActionSnapshot]) off the
/// main player's `CSChrActionRequestModule`. `None` if not resolved yet.
pub fn main_player_action_snapshot() -> Option<ActionSnapshot> {
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

/// Returns the main player's `ChrIns` address if currently resolved, `None`
/// otherwise (title screen, loading, no save loaded, ...).
pub fn main_player_chr_ins_ptr() -> Option<*const u8> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    world_chr_man
        .main_player
        .as_ref()
        .map(|p| &p.chr_ins as *const _ as *const u8)
}

// How often to log a reminder while still waiting for the player - not a
// warning (taking a few minutes to pick a save and load in is completely
// normal, unlike a slow CSTaskImp init), just a "still here, still trying"
// breadcrumb so a long wait doesn't look like the thread died.
const REMINDER_EVERY: Duration = Duration::from_secs(30);

/// Waits - forever, never gives up, like [`crate::task::wait_for_cs_task`] -
/// for `SoloParamRepository` (the live in-memory regulation.bin) AND for the
/// player to actually be in the game world before returning.
/// `SoloParamRepository::instance_mut()` alone can return `Ok` as soon as the
/// manager object exists (title/loading screen, well before its param
/// tables are actually populated) - same premature-ready-singleton class
/// [main_player_chr_ins_ptr] itself exists to guard against.
///
/// Used to take a `timeout` and return `Option` (2026-08-24 through
/// 2026-09-17) - every caller passed the same `Duration::from_secs(300)` and
/// treated a timeout as "disabled for this session" (some, like
/// `risearcher`, didn't even keep the reload-watch task running past that
/// point). In practice this just meant a player who alt-tabbed or was slow
/// to pick a save lost the feature for the rest of the session, silently
/// (only a log line, easy to miss) unless they knew to press `ReloadKey`
/// afterward. There's no real reason to ever give up here - unlike a
/// version-locked lookup, this is purely "hasn't happened yet", so it's
/// changed to retry forever, the same way `wait_for_cs_task` already did.
pub fn wait_for_solo_param_repository() -> &'static mut SoloParamRepository {
    let step = Duration::from_millis(200);
    let mut waited = Duration::ZERO;
    let mut last_reminder = Duration::ZERO;
    loop {
        if main_player_chr_ins_ptr().is_some() {
            if let Ok(repo) = unsafe { SoloParamRepository::instance_mut() } {
                return repo;
            }
        }
        if waited - last_reminder >= REMINDER_EVERY {
            last_reminder = waited;
            crate::logger::log("Still waiting for the player to be in the game world (SoloParamRepository)...");
        }
        std::thread::sleep(step);
        waited += step;
    }
}
