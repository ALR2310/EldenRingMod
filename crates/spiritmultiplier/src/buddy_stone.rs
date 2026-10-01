//! Spirit summoning pools (`BuddyStoneParam` - one row per summoning pool
//! area), two hot-reload toggles:
//!
//! - `NoRestResummon`: re-summon after the spirits die (or are sent back)
//!   without resting at a Site of Grace. The once-per-rest lock is the
//!   row's `summonedEventFlagId` (+0xC): the game raises that event flag on
//!   a summon and refuses the pool while it is on; resting clears it.
//!   Written as 0 = no flag, so nothing ever locks the pool.
//! - `SummonAnywhere`: summon outside the vanilla areas. Every row gets
//!   `activateRange` (+0x1C) = 65535, `overwriteReturnRange` (+0x1E) = -1
//!   (no override), `overwriteActivateRegionEntityId` (+0x20) = 0 (use the
//!   range, not a map region) and `eliminateTargetEntityId` (+0x8) = 0 (the
//!   pool no longer closes once its boss is dead).
//!
//! Both are exactly what Solid Uncapper 2.3.3 writes for its `No-Rest
//! Re-Summon` / `Summon Anywhere` settings (`sub_180031EC0` /
//! `sub_1800321B0`, snapshot in `sub_180031D30` - see README 2026-09-29);
//! re-implemented here. Rows are walked by index (`common::params`), and
//! the original values are captured once before the first write, so every
//! later change is applied from that snapshot (F5 works both ways).
//!
//! The param edits alone only work while some pool's stone object is
//! loaded near the player (Nexus reports 2026-09-30: nothing outside pools
//! in the Shadowlands, or anywhere for one player). Three more things in
//! `SummonBuddyManager` depend on a loaded stone (2.7.1.0):
//!
//! 1. `GetBuddyState` (`sub_1404B72F0`) - and the HUD's
//!    `sub_1404B7610` - require the player to carry a SpEffect with
//!    stateInfo 373 (`sub_1404FA370(.., 0x175)`), which the pool area puts
//!    on the player. [STATE_CALL_AOB] / [STATE_TAIL_AOB]: that check
//!    answers true while Anywhere is on.
//! 2. The current stone `+0x38` is cleared every frame by
//!    `sub_1404B6E80` (called from the talk-script update `sub_140EAFDE0`)
//!    and set again only by a loaded stone's own talk script (ESD command
//!    122 -> `sub_1404B8050`) - with none loaded it stays 0 and
//!    `GetBuddyState` says -1 (greyed out). The same stateInfo stub inside
//!    `GetBuddyState` writes [FALLBACK_STONE] (the last real stone seen, or
//!    a default row) into `+0x38` when it is 0, right before
//!    `GetBuddyState` reads it - and every summon path asks `GetBuddyState`
//!    first. A BuddyStoneParam row id is the stone's entity id (param 134
//!    is looked up with it directly, `sub_140D28320`). (First try wrote
//!    `+0x38` from the FrameBegin tick - the clear ran after it; 1.1.1-1.1.3
//!    then patched the clear itself, an inline hook.)
//! 3. `sub_1404BD870` recomputes "in the active stone's range" (`+0xB5`)
//!    every frame by finding the stone object; unloaded -> false -> the
//!    "left the summoning area" dismiss-all. [INRANGE_AOB]: `+0xB5` forced
//!    to 1 (and the warn flag `+0xB7` to 0, as the function itself does)
//!    right after it while Anywhere is on.
//!
//! All three only redirect an existing `call` / `jmp`
//! (`codepatch::redirect_rel32`, see `multi_spirit.rs` - Seamless Co-op
//! aborts the game when its own byte signatures stop matching).

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::time::Duration;

use eldenring::cs::{BuddyStoneParam, CSTaskGroupIndex, SoloParamRepository, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::{codepatch, config, logger, memscan};

/// `mov edx,0x175; mov rcx,[rax+0x178]; call sub_1404FA370; test al,al; jz
/// ..; mov eax,[rbx+0x38]` in `GetBuddyState` (0x1404B735C in 2.7.1.0,
/// unique; rbx = manager). The `call` is at +12.
const STATE_CALL_AOB: &str = "BA 75 01 00 00 48 8B 88 78 01 00 00 E8 ?? ?? ?? ?? 84 C0 74 10 8B 43 38";
const STATE_CALL_OFFSET: usize = 12;
/// Same check as a tail call in `sub_1404B7610` (0x1404B7655, unique):
/// `... add rsp,0x28; jmp sub_1404FA370`. The `jmp` is at +16.
const STATE_TAIL_AOB: &str = "BA 75 01 00 00 48 8B 88 78 01 00 00 48 83 C4 28 E9";
const STATE_TAIL_OFFSET: usize = 16;
/// `movaps xmm1,xmm10; mov rcx,r15; call sub_1404B8EB0; movaps xmm1,xmm10;
/// mov rcx,r15; call sub_1404BD870; cmp qword [r15+0xE8],0` in
/// `SummonBuddyManager::Update` (0x1404B8B1B, unique; r15 = manager). The
/// second `call` is at +19.
const INRANGE_AOB: &str =
    "41 0F 28 CA 49 8B CF E8 ?? ?? ?? ?? 41 0F 28 CA 49 8B CF E8 ?? ?? ?? ?? 49 83 BF E8 00 00 00 00";
const INRANGE_OFFSET: usize = 19;

const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

/// 1 = SummonAnywhere applied. Read by every stub.
static ANYWHERE: AtomicU8 = AtomicU8::new(0);
/// Stone written into an empty `+0x38` (2.) while Anywhere is on; 0 = none.
static FALLBACK_STONE: AtomicU32 = AtomicU32::new(0);

const TICK_INTERVAL_MS: f64 = 1000.0;

const ANYWHERE_ACTIVATE_RANGE: u16 = u16::MAX;
const ANYWHERE_RETURN_RANGE: i16 = -1;

/// Original values of the fields this module writes, per row index.
struct Original {
    index: usize,
    eliminate_target_entity_id: u32,
    summoned_event_flag_id: u32,
    activate_range: u16,
    overwrite_return_range: i16,
    overwrite_activate_region_entity_id: u32,
}

fn rel8(from_next: usize, to: usize) -> u8 {
    (to as i64 - from_next as i64) as i8 as u8
}

/// `call qword [rip+2]; jmp +8; dq target` - an absolute call.
fn push_call(c: &mut Vec<u8>, target: u64) {
    c.extend_from_slice(&[0xFF, 0x15, 0x02, 0x00, 0x00, 0x00]);
    c.extend_from_slice(&[0xEB, 0x08]);
    c.extend_from_slice(&target.to_le_bytes());
}

/// `mov r11, &ANYWHERE; cmp byte [r11], 0; je <patched later>` - returns
/// the offset of the `je` to patch.
fn push_anywhere_check(c: &mut Vec<u8>, anywhere: u64) -> usize {
    c.extend_from_slice(&[0x49, 0xBB]);
    c.extend_from_slice(&anywhere.to_le_bytes());
    c.extend_from_slice(&[0x41, 0x80, 0x3B, 0x00]);
    let je = c.len();
    c.extend_from_slice(&[0x74, 0]);
    je
}

/// Destination of `GetBuddyState`'s `call` to the stateInfo check (args in
/// rcx/edx, rbx = manager; entered by that `call`, rsp % 16 == 8): call
/// it; while Anywhere is on answer `al = 1`, and fill an empty current
/// stone `[rbx+0x38]` with [FALLBACK_STONE] (2.). r11 is volatile.
fn build_state_call_stub(anywhere: u64, fallback: u64, check: u64) -> Vec<u8> {
    let mut c = Vec::with_capacity(80);
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub rsp, 0x28
    push_call(&mut c, check);
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    let je_ret = push_anywhere_check(&mut c, anywhere);
    c.extend_from_slice(&[0xB0, 0x01]); // mov al, 1
    c.extend_from_slice(&[0x83, 0x7B, 0x38, 0x00]); // cmp dword [rbx+0x38], 0
    let jne_ret = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne ret (a real stone is set)
    c.extend_from_slice(&[0x49, 0xBB]); // mov r11, &FALLBACK_STONE
    c.extend_from_slice(&fallback.to_le_bytes());
    c.extend_from_slice(&[0x45, 0x8B, 0x1B]); // mov r11d, [r11]
    c.extend_from_slice(&[0x44, 0x89, 0x5B, 0x38]); // mov [rbx+0x38], r11d
    let ret = c.len();
    c.push(0xC3); // ret
    c[je_ret + 1] = rel8(je_ret + 2, ret);
    c[jne_ret + 1] = rel8(jne_ret + 2, ret);
    c
}

/// Destination of the tail `jmp` to the stateInfo check in
/// `sub_1404B7610` (frame already torn down, rsp % 16 == 8): re-align +
/// shadow space, call, same `al = 1` override, `ret` to the real caller.
fn build_state_tail_stub(anywhere: u64, check: u64) -> Vec<u8> {
    let mut c = Vec::with_capacity(56);
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub rsp, 0x28
    push_call(&mut c, check);
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    let je_ret = push_anywhere_check(&mut c, anywhere);
    c.extend_from_slice(&[0xB0, 0x01]); // mov al, 1
    let ret = c.len();
    c.push(0xC3); // ret
    c[je_ret + 1] = rel8(je_ret + 2, ret);
    c
}

/// Destination of `Update`'s `call sub_1404BD870` (rcx = r15 = manager,
/// r15 callee-saved): call it, then while Anywhere is on force "in
/// activation range" (`+0xB5` = 1) and clear "in warn range" (`+0xB7` = 0).
fn build_inrange_stub(anywhere: u64, compute: u64) -> Vec<u8> {
    let mut c = Vec::with_capacity(64);
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub rsp, 0x28
    push_call(&mut c, compute);
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    let je_ret = push_anywhere_check(&mut c, anywhere);
    c.extend_from_slice(&[0x41, 0xC6, 0x87, 0xB5, 0x00, 0x00, 0x00, 0x01]); // mov byte [r15+0xB5], 1
    c.extend_from_slice(&[0x41, 0xC6, 0x87, 0xB7, 0x00, 0x00, 0x00, 0x00]); // mov byte [r15+0xB7], 0
    let ret = c.len();
    c.push(0xC3); // ret
    c[je_ret + 1] = rel8(je_ret + 2, ret);
    c
}

/// Finds `pattern` and redirects the 5-byte call/jmp at `offset` to
/// `build(original destination)`.
fn install(name: &str, pattern: &str, offset: usize, build: impl Fn(u64) -> Vec<u8>) -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(pattern, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error(&format!("SummonAnywhere: {name} pattern not found. Game may have been updated."));
        return false;
    };
    let site = unsafe { anchor.add(offset) };
    let Some(target) = codepatch::rel32_target(site) else {
        return false;
    };
    let Some(at) = codepatch::redirect_rel32(site, &build(target)) else {
        logger::error(&format!("SummonAnywhere: {name} redirect failed."));
        return false;
    };
    logger::log(&format!("SummonAnywhere: {name} at {site:p} (was {target:#x}) -> stub {at:p}."));
    true
}

fn install_patches() -> bool {
    let flag = ANYWHERE.as_ptr() as u64;
    let fallback = FALLBACK_STONE.as_ptr() as u64;
    install("stateInfo check", STATE_CALL_AOB, STATE_CALL_OFFSET, |t| build_state_call_stub(flag, fallback, t))
        && install("stateInfo tail check", STATE_TAIL_AOB, STATE_TAIL_OFFSET, |t| build_state_tail_stub(flag, t))
        && install("in-range check", INRANGE_AOB, INRANGE_OFFSET, |t| build_inrange_stub(flag, t))
}

/// Stone used while none has been seen yet this session: the first row
/// without a doping SpEffect (plain spirits), else the first row.
fn default_stone(repo: &mut SoloParamRepository) -> Option<u32> {
    let ids = common::params::row_ids::<BuddyStoneParam>(repo)?;
    let mut plain = None;
    common::params::for_each_row_mut::<BuddyStoneParam>(repo, |index, row| {
        if plain.is_none() && ids[index] > 0 && row.doping_sp_effect_id() <= 0 {
            plain = Some(ids[index]);
        }
    });
    plain.or_else(|| ids.iter().copied().find(|&id| id > 0))
}

fn capture(repo: &mut SoloParamRepository) -> Option<Vec<Original>> {
    if let Err(err) = common::params::check::<BuddyStoneParam>(repo) {
        logger::error(&format!("BuddyStone: {err} - not touching params."));
        return None;
    }
    let mut rows = Vec::new();
    common::params::for_each_row_mut::<BuddyStoneParam>(repo, |index, row| {
        rows.push(Original {
            index,
            eliminate_target_entity_id: row.eliminate_target_entity_id(),
            summoned_event_flag_id: row.summoned_event_flag_id(),
            activate_range: row.activate_range(),
            overwrite_return_range: row.overwrite_return_range(),
            overwrite_activate_region_entity_id: row.overwrite_activate_region_entity_id(),
        });
    });
    let locked = rows.iter().filter(|r| r.summoned_event_flag_id != 0).count();
    logger::log(&format!(
        "BuddyStone: {} summoning pool row(s), {locked} with a once-per-rest flag.",
        rows.len()
    ));
    Some(rows)
}

pub fn run() {
    let patched = install_patches();
    if !patched {
        logger::error("SummonAnywhere: code patches not installed - it only works near loaded summoning pools.");
    }
    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    // None = not captured yet; Some(None) = capture failed, stay off.
    let mut originals: Option<Option<Vec<Original>>> = None;
    let mut applied: Option<(bool, bool)> = None;
    // Last real current stone (+0x38) seen, else the default row.
    let mut fallback_stone: Option<u32> = None;

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "BuddyStone",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            // Same in-world gate as every other param-touching feature.
            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }

            // Every frame: remember the last real stone as the fallback (2.).
            if ANYWHERE.load(Ordering::Relaxed) != 0 {
                if let Ok(world_chr_man) = unsafe { WorldChrMan::instance() } {
                    let current = world_chr_man.summon_buddy_manager.buddy_stone_entity_id;
                    if current != 0 && Some(current) != fallback_stone {
                        fallback_stone = Some(current);
                        FALLBACK_STONE.store(current, Ordering::Relaxed);
                    }
                }
            }

            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;
            let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
                return;
            };

            let no_rest = config::get_bool("NoRestResummon", true);
            let anywhere = config::get_bool("SummonAnywhere", true);
            let Some(rows) = originals.get_or_insert_with(|| capture(repo)) else {
                return;
            };
            if fallback_stone.is_none() {
                fallback_stone = default_stone(repo);
            }
            if applied == Some((no_rest, anywhere)) {
                return;
            }

            for original in rows.iter() {
                let Some(row) = repo.get_row_by_index_mut::<BuddyStoneParam>(original.index) else {
                    continue;
                };
                row.set_summoned_event_flag_id(if no_rest { 0 } else { original.summoned_event_flag_id });
                if anywhere {
                    row.set_activate_range(ANYWHERE_ACTIVATE_RANGE);
                    row.set_overwrite_return_range(ANYWHERE_RETURN_RANGE);
                    row.set_overwrite_activate_region_entity_id(0);
                    row.set_eliminate_target_entity_id(0);
                } else {
                    row.set_activate_range(original.activate_range);
                    row.set_overwrite_return_range(original.overwrite_return_range);
                    row.set_overwrite_activate_region_entity_id(original.overwrite_activate_region_entity_id);
                    row.set_eliminate_target_entity_id(original.eliminate_target_entity_id);
                }
            }
            let active = anywhere && patched;
            ANYWHERE.store(active as u8, Ordering::Relaxed);
            FALLBACK_STONE.store(if active { fallback_stone.unwrap_or(0) } else { 0 }, Ordering::Relaxed);
            if active {
                logger::log(&format!("SummonAnywhere: fallback stone {}.", fallback_stone.unwrap_or(0)));
            }
            logger::log(&format!(
                "NoRestResummon={no_rest}, SummonAnywhere={anywhere}: applied to {} pool row(s).",
                rows.len()
            ));
            applied = Some((no_rest, anywhere));
        },
    );

    logger::log("BuddyStone: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints the stubs for manual disassembly (capstone).
    #[test]
    fn dump_stubs() {
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        println!("STATE_CALL_STUB={}", hex(&build_state_call_stub(0x1122334455667788, 0x5566778899AABBCC, 0x1404FA370)));
        println!("STATE_TAIL_STUB={}", hex(&build_state_tail_stub(0x1122334455667788, 0x1404FA370)));
        println!("INRANGE_STUB={}", hex(&build_inrange_stub(0x1122334455667788, 0x1404BD870)));
    }
}
