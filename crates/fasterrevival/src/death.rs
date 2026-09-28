//! Player death handling: makes the game "kill" the main player (the step
//! that leads to the "YOU DIED" screen and respawn) as soon as the death
//! animation starts, instead of near its end. The pause after "YOU DIED"
//! itself is cut by [crate::fade]. Always on - no ini switch.
//!
//! ## How vanilla does it (IDA, exe 2.7.1.0, 2026-09-28)
//!
//! - HP hits 0 -> `ChrIns+0x1C5` bit 7 (`chr_flags1c5.death_flag`) is set.
//!   A per-frame check (`sub_1403F84C0`) either requests the death
//!   animation or, for deaths without one, calls the kill function
//!   straight away.
//! - The death animation's TAE carries a `ChrActionFlag` (event type 0)
//!   action 12 "Kill Character" event near its end. The type-0 handler
//!   (`sub_140428DE0`, reached from `CSChrTaeAnimEvent`'s vtable) calls the
//!   kill function [KILL_CHR_PATTERN] (`sub_1403EDA70(ChrIns*)`) while that
//!   event is active.
//! - The kill function is idempotent: it bails out if
//!   `ChrCtrl.modifier+0x24` bit 0 (unnamed `unk1cflags` in fromsoftware-rs)
//!   is already set, and sets that bit itself first thing - so calling it
//!   early makes the TAE event's own later call a no-op.
//!
//! The file-patching FasterDeathAnimation tool (0-F) gets the same result
//! by moving the action 12 event to `t=0` in `a00.tae`; calling the same
//! function directly avoids replacing `c0000.anibnd.dcx` (conflicts with
//! animation mods). Not replicated yet: action 20 "Send Ghost Info"
//! (`ChrCtrlModifier` action flag bit 1), which the tool also moves - its
//! handler has an extra check (`sub_1404C8730`) not understood yet.
//!
//! Death is detected as `hp <= 0`, not `death_flag`: in-game (2026-09-28)
//! a FrameBegin task never saw `death_flag` set at all - it's consumed
//! inside the same frame's character update.
//!
//! The kill is only sent while one of the animations that carry the event
//! is playing ([has_kill_event]).
//!
//! Every death logs how long it took from HP 0 until the game registered
//! it and until respawn. `Debug.DeathProbe` adds the animation trace (anim
//! ID, play time, length) in between.

use std::time::Instant;

use eldenring::cs::{CSTaskGroupIndex, ChrIns, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::{config, logger, memscan};

use crate::fade::{self, FadeState};

// `sub_1403EDA70` (2.7.1.0) / `sub_1403ED840` (2.6.2.0), unique in both.
// `sub_1403EDB60` (action 47 "Kill Character (No Item Drop)") shares the
// whole prologue and differs only by one missing call, hence the long tail.
const KILL_CHR_PATTERN: &str = "40 53 48 83 EC 40 48 C7 44 24 20 FE FF FF FF 0F 29 74 24 30 48 8B D9 48 8B 41 58 \
     48 8B 90 C8 00 00 00 F6 42 24 01 0F 85 ?? ?? ?? ?? E8 ?? ?? ?? ?? 8B 43 68 83 F8 03 0F 84 ?? ?? ?? ?? \
     83 F8 0A 0F 84 ?? ?? ?? ?? 48 8B CB E8 ?? ?? ?? ?? 48 8B CB E8 ?? ?? ?? ?? 0F 28 F0";

type KillChrFn = unsafe extern "C" fn(*mut ChrIns);

// Offset of the "already killed" flag byte inside `ChrCtrlModifier`
// (bit 0) - see module doc.
const MODIFIER_KILLED_OFFSET: usize = 0x24;

/// Whether `anim_id` is one of the player animations whose TAE carries the
/// action 12 "Kill Character" event - the ones the file-patching tool
/// rewrites, listed by running it on its repo's `c0000.anibnd.dcx`
/// (2026-09-28): `a000_017xxx` (regular deaths), `a000_070xxx` (deaths
/// inside an enemy grab/throw), `a000_117xxx`, `a000_075003`,
/// `a000_000150`. Anim IDs read from `CSChrTimeActModule` are
/// `category * 1_000_000 + id`, e.g. `17002` = `a000_017002`.
///
/// Deaths without such an animation (e.g. falling, anim `4100`) are
/// already killed by the game on the frame HP hits 0, nothing to speed up.
/// Only the currently playing anim counts: in-game the anim queue can still
/// switch between other (attack) anims for a frame or two after HP 0
/// before the death animation starts, and killing then is not what the
/// TAE event does.
fn has_kill_event(anim_id: i32) -> bool {
    matches!(anim_id, 150 | 17_000..=17_999 | 70_000..=70_999 | 75_003 | 117_000..=117_999)
}

// Log at most this often while dead - every frame would bury the log.
const PROBE_INTERVAL_MS: u128 = 100;

#[derive(Default)]
struct DeathState {
    // `Some` from the first frame HP is seen at 0 until respawn.
    since: Option<Instant>,
    kill_sent: bool,
    killed_logged: bool,
    last_log: Option<Instant>,
    last_anim_id: i32,
    last_killed: bool,
    fade: FadeState,
}

pub fn run() {
    let kill_chr: Option<KillChrFn> = match memscan::find_pattern_in_module(KILL_CHR_PATTERN) {
        Some(addr) => {
            logger::log("Kill function found.");
            Some(unsafe { std::mem::transmute::<*mut u8, KillChrFn>(addr) })
        }
        None => {
            logger::error("Kill function not found - game version not supported, death speed-up disabled.");
            None
        }
    };

    let cs_task = common::task::wait_for_cs_task();
    let mut state = DeathState::default();
    common::task::run_recurring_safe(
        cs_task,
        "Death",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| tick(&mut state, kill_chr),
    );
}

fn tick(state: &mut DeathState, kill_chr: Option<KillChrFn>) {
    let probe = config::get_bool("DeathProbe", false);

    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else { return };
    let Some(main_player) = world_chr_man.main_player.as_ref() else { return };
    let chr_ins = &main_player.chr_ins;
    let hp = chr_ins.modules.data.hp;

    fade::apply(&mut state.fade);

    if hp > 0 {
        if let Some(since) = state.since.take() {
            logger::log(&format!("Respawned: {} ms after HP hit 0.", since.elapsed().as_millis()));
        }
        return;
    }

    let time_act = &chr_ins.modules.time_act;
    let cur = &time_act.anim_queue[time_act.read_idx as usize % time_act.anim_queue.len()];
    let modifier = &*chr_ins.chr_ctrl.modifier as *const _ as *const u8;
    let killed = unsafe { *modifier.add(MODIFIER_KILLED_OFFSET) } & 1 != 0;

    let now = Instant::now();
    let since = match state.since {
        Some(t) => t,
        None => {
            state.since = Some(now);
            state.kill_sent = false;
            state.killed_logged = false;
            state.last_log = None;
            state.last_anim_id = -1;
            logger::log(&format!(
                "Died: anim={} death_flag={} killed={}.",
                cur.anim_id,
                chr_ins.chr_flags1c5.death_flag(),
                killed,
            ));
            now
        }
    };
    let elapsed = now.duration_since(since).as_millis();

    if !killed && !state.kill_sent {
        if let Some(kill_chr) = kill_chr {
            if has_kill_event(cur.anim_id) {
                state.kill_sent = true;
                unsafe { kill_chr(chr_ins as *const ChrIns as *mut ChrIns) };
                logger::log(&format!("Killed early: +{elapsed} ms, anim={} t={:.3}.", cur.anim_id, cur.play_time));
            }
        }
    }

    if killed && !state.killed_logged {
        state.killed_logged = true;
        logger::log(&format!("Death registered by the game: +{elapsed} ms (anim={} t={:.3}).", cur.anim_id, cur.play_time));
    }

    if probe {
        let changed = cur.anim_id != state.last_anim_id || killed != state.last_killed;
        let due = state.last_log.is_none_or(|t| now.duration_since(t).as_millis() >= PROBE_INTERVAL_MS);
        if changed || due {
            logger::log(&format!(
                "DeathProbe: +{elapsed} ms anim={} t={:.3}/{:.3} death_flag={} killed={}{}",
                cur.anim_id,
                cur.play_time,
                cur.anim_length,
                chr_ins.chr_flags1c5.death_flag(),
                killed,
                if changed { " *" } else { "" },
            ));
            state.last_log = Some(now);
            state.last_anim_id = cur.anim_id;
            state.last_killed = killed;
        }
    }
}
