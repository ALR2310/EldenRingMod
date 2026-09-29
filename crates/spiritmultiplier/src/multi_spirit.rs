//! `MultiSpirit` (hot reload): summon a DIFFERENT Spirit Ash while spirits
//! are already out and the old ones stay, instead of being sent back.
//! Re-using an Ash that is already out sends back only that Ash's spirits.
//!
//! Reverse engineered from Solid Uncapper 2.3.3's "Multi Spirit Coexist"
//! (see README 2026-09-29) and re-implemented here - the same two patch
//! points in `SummonBuddyManager`, on `eldenring.exe` 2.7.1.0:
//!
//! 1. `DoSummon` (`sub_1404B85B0`) asks `GetBuddyState(mgr, -1)` and, on
//!    state 2 (a spirit is out), raises `disappear_requested` (+0x28) and
//!    returns - vanilla's "one group at a time". Patched right after that
//!    call ([DOSUMMON_AOB] + 5, 14 bytes): [decide_dismiss] checks whether
//!    the requested Ash (SpEffect `edi`) still has live spirits in
//!    `groups`. No → the state becomes 0 and the normal summon path runs.
//!    Yes, or the feature off → the original bytes, replayed (dismiss
//!    request). (First version compared `edi` with the active/pending
//!    SpEffect `+0x24`/`+0x20` - that only knows the LAST Ash summoned.)
//! 2. `SummonBuddyManager::Update` (`sub_1404B86D0`) calls `sub_1404BB9A0`
//!    to recall the player's whole existing group right before spawning the
//!    new one ([RECALL_AOB] + 5, the 5-byte `call`). Patched to skip that
//!    call while the feature is on.
//!
//! 3. The item UI: with patches 1+2 alone, other Ashes stayed greyed out
//!    while a spirit was out (in-game test 2026-09-29). Like Solid
//!    Uncapper, the 3 UI callers of `GetBuddyState` ([UI_CALL_SITES]:
//!    `sub_1407C3930` - the "use item" prompt, state 2 = "send the spirit
//!    back?" dialog 20000600 - and the two FP-cost/label helpers
//!    `sub_140847830`/`sub_140847B20`) get their `call` replaced by a stub
//!    that calls the real function and turns any state >= 1 into 0 while
//!    the feature is on. Other callers (DoSummon, Update) still see the
//!    truth, so re-using the same Ash still dismisses it (patch 1) - just
//!    without the confirmation dialog.
//!
//! 4. The actual greying out (in-game test 2 - patch 3 alone did not fix
//!    it): `CanUseItem` (`sub_14068EE60`) marks a buddy item usable only
//!    when `GetBuddyState(mgr, goodsId) != -1` ([CANUSE_AOB]). With spirits
//!    out, every Ash except the active one answers -1 there (not the
//!    active goods id, same summoning area) - so they grey out. Patched:
//!    on -1, ask again with goodsId -1; if that says 2 (spirits out, same
//!    area), answer 0 instead. Away from any summoning area it stays -1, so
//!    Ashes still grey out there like vanilla.
//!
//! 5. Test 3/4: with 1-4 in, the second group spawned next to the first,
//!    then both vanished ~10 s later (`buddyDisappearDelaySec`). A probe on
//!    `DisappearAll` (`sub_1404B8160`) showed it firing every frame from
//!    `sub_1404B92B0`'s "left the summoning area" branch (`+0x20 < 0 &&
//!    !is_within_warn_range(+0xB7) && !is_within_activation_range(+0xB5)`)
//!    right after the second summon. Those flags come from
//!    `sub_1404BD870`, which measures the player against the BuddyStoneParam
//!    region of the ACTIVE stone `+0x3C`; `DoSummon` copies the current
//!    stone `+0x38` into it on every successful summon
//!    ([DOSUMMON_STONE_OFFSET]). Suspected: `+0x38` is already 0 by the
//!    second summon, so `+0x3C` becomes 0, both flags go false, and
//!    `DisappearAll` (which also zeroes `+0x3C`) keeps firing. Patched: if
//!    `+0x38` is 0, keep the old `+0x3C`. Confirmed by SlotProbe's stones
//!    log in test 5 - 4 different Ashes stayed out together.
//!
//! 6. Per-Ash send-back: vanilla only has `DisappearAll`, so re-using one
//!    Ash sent all of them back (test 5). `DisappearAll` is detoured
//!    ([disappear_all_hook]): for the call that serves a dismiss request
//!    set by [decide_dismiss], every live entry of the other Ashes is
//!    masked (its `disappear_requested` raised) around the original call,
//!    so only the pressed Ash's spirits leave - Solid Uncapper's approach.
//!    The active stone `+0x3C` is restored afterwards while others remain.
//!
//! All stubs read one byte ([ENABLED]) that the tick below keeps in sync
//! with the ini, so F5 switches the behavior live without re-patching.


use std::sync::atomic::{AtomicI32, AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, SummonBuddyManager};

use common::{codepatch, config, logger, memscan};

/// `call GetBuddyState; cmp eax,1; jbe ..; cmp eax,2; jnz ..; mov byte
/// [rbx+0x28],1` in `DoSummon` (0x1404B85C9 in 2.7.1.0). Unique in 2.7.1.0.
const DOSUMMON_AOB: &str = "E8 ?? ?? ?? ?? 83 F8 01 76 1B 83 F8 02 75 04 C6 43 28 01";
/// Hook starts at `cmp eax,1`, replaces the 14 bytes up to `xor al,al`
/// (the "return 0" exit the appended jump-back lands on).
const DOSUMMON_HOOK_OFFSET: usize = 5;
const DOSUMMON_HOOK_LEN: usize = 14;
/// Offset from the hook site of the summon path (`test edi,edi` at
/// 0x1404B85EE) - the `jbe` target: 0x1404B85D1 + 2 + 0x1B.
const DOSUMMON_SUMMON_OFFSET: usize = 0x20;

/// `mov edx,[rax]; mov rcx,r15; call recall; cmp dword [r15+0x88],0` in
/// `SummonBuddyManager::Update` (0x1404B89AB in 2.7.1.0). Unique in 2.7.1.0.
const RECALL_AOB: &str = "8B 10 49 8B CF E8 ?? ?? ?? ?? 41 83 BF 88 00 00 00 00";
const RECALL_CALL_OFFSET: usize = 5;
const RECALL_CALL_LEN: usize = 5;

/// The 3 UI callers of `GetBuddyState`: (pattern, offset of the 5-byte
/// `call` within it). Each unique in 2.7.1.0 (0x1407C3C9E, 0x140847A1B,
/// 0x140847CDF - the same patterns Solid Uncapper uses).
const UI_CALL_SITES: [(&str, usize); 3] = [
    ("48 8B C8 83 CA FF E8 ?? ?? ?? ?? 85 C0 74 ?? 83 E8 01", 6),
    ("48 8B C8 8B D3 E8 ?? ?? ?? ?? 83 F8 02 75", 5),
    ("48 8B C8 8B D6 E8 ?? ?? ?? ?? 83 F8 02 75", 5),
];

/// `mov eax,[rbx+0x38]; mov rsi,[rsp+0x38]` in `DoSummon`'s success path
/// (0x1404B862A), right before `mov [rbx+0x3C],eax`. Offset from the
/// [DOSUMMON_AOB] match; the 8 bytes are checked before patching.
const DOSUMMON_STONE_OFFSET: usize = 0x61;
const DOSUMMON_STONE_BYTES: [u8; 8] = [0x8B, 0x43, 0x38, 0x48, 0x8B, 0x74, 0x24, 0x38];

/// `mov rcx,rax; mov edx,r13d; call GetBuddyState; cmp eax,-1; setnz
/// r13b` in `CanUseItem` (0x14069037D in 2.7.1.0). Unique in 2.7.1.0.
const CANUSE_AOB: &str = "48 8B C8 41 8B D5 E8 ?? ?? ?? ?? 83 F8 FF 41 0F 95 C5";
const CANUSE_CALL_OFFSET: usize = 6;

/// Entry of `DisappearAll(mgr)` (`sub_1404B8160`, 2.7.1.0): `push rbp;
/// push rsi; push rdi; sub rsp,0x30; mov qword [rsp+0x20],-2; ...`
/// (Solid Uncapper's send-back signature). Detoured for patch 6.
const DISAPPEAR_ALL_AOB: &str =
    "40 55 56 57 48 83 EC 30 48 C7 44 24 20 FE FF FF FF 48 89 5C 24 68 48 8B E9 C7 41 3C";
/// `push rbp; push rsi; push rdi; sub rsp,0x30; mov qword [rsp+0x20],-2`
/// - position independent, replayed verbatim in the stub.
const DISAPPEAR_ALL_STOLEN: usize = 17;

/// Trigger SpEffect of the Ash whose re-use [decide_dismiss] just turned
/// into a dismiss request; -1 = none pending.
static DISMISS_TARGET: AtomicI32 = AtomicI32::new(-1);
/// Entry of the original `DisappearAll` (stolen prologue + jump back),
/// inside the detour stub. 0 until installed.
static DISAPPEAR_ALL_ORIGINAL: AtomicUsize = AtomicUsize::new(0);

/// 1 = multi-spirit on. Read by every stub.
static ENABLED: AtomicU8 = AtomicU8::new(0);

const TICK_INTERVAL_MS: f64 = 1000.0;
const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

fn rel8(from_next: usize, to: usize) -> u8 {
    (to as i64 - from_next as i64) as i8 as u8
}

/// BuddyParam ids the Ash with this trigger SpEffect spawns (its chain in
/// `trigger_speffect_to_buddy_map` - also covers our own chain extension,
/// which only repeats the original ids).
fn buddy_ids(manager: &SummonBuddyManager, speffect: i32) -> Vec<i32> {
    manager
        .trigger_speffect_to_buddy_map
        .iter_chains()
        .find(|(key, _)| **key == speffect)
        .map(|(_, head)| head.iter().copied().collect())
        .unwrap_or_default()
}

/// Called from the DoSummon stub (patch 1) when a spirit is out: 1 = this
/// Ash already has live spirits (dismiss it - [DISMISS_TARGET] remembers
/// which one for patch 6), 0 = a different Ash (summon it too). Runs on
/// the game thread, inside `SummonBuddyManager::Update`.
unsafe extern "system" fn decide_dismiss(manager: *mut SummonBuddyManager, speffect: i32) -> u8 {
    let result = std::panic::catch_unwind(|| {
        let manager = unsafe { &*manager };
        let ids = buddy_ids(manager, speffect);
        let alive = !ids.is_empty()
            && manager.groups.iter().any(|pair| {
                pair.second
                    .iter()
                    .any(|g| !g.disappear_requested && !g.is_remote && ids.contains(&g.buddy_param_id))
            });
        if alive {
            DISMISS_TARGET.store(speffect, Ordering::Relaxed);
        }
        alive as u8
    });
    // On a panic, fall back to vanilla (dismiss everything).
    result.unwrap_or(1)
}

/// Stub for patch 1. Entry: `eax` = GetBuddyState result, `edi` = requested
/// SpEffect, `rbx` = SummonBuddyManager, rsp 16-aligned with the function's
/// own outgoing-args area at [rsp..rsp+0x20] usable as shadow space. The
/// call to [decide_dismiss] clobbers only volatile registers (rax, rcx,
/// rdx, r8-r11), none of which the original code reads after this point.
fn build_dosummon_stub(enabled: u64, decide: u64, summon_path: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(96);

    c.extend_from_slice(&[0x83, 0xF8, 0x02]); // cmp eax, 2
    let jne_orig = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne orig
    c.extend_from_slice(&[0x49, 0xBB]); // mov r11, &ENABLED
    c.extend_from_slice(&enabled.to_le_bytes());
    c.extend_from_slice(&[0x41, 0x80, 0x3B, 0x00]); // cmp byte [r11], 0
    let je_off = c.len();
    c.extend_from_slice(&[0x74, 0]); // je orig (feature off, eax still 2)
    c.extend_from_slice(&[0x48, 0x8B, 0xCB]); // mov rcx, rbx
    c.extend_from_slice(&[0x8B, 0xD7]); // mov edx, edi
    c.extend_from_slice(&[0x48, 0xB8]); // mov rax, decide_dismiss
    c.extend_from_slice(&decide.to_le_bytes());
    c.extend_from_slice(&[0xFF, 0xD0]); // call rax
    c.extend_from_slice(&[0x84, 0xC0]); // test al, al
    c.extend_from_slice(&[0xB8, 0x02, 0x00, 0x00, 0x00]); // mov eax, 2 (flags kept)
    let jnz_same = c.len();
    c.extend_from_slice(&[0x75, 0]); // jnz orig (same Ash: vanilla dismiss)
    c.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax -> "nothing out"

    // orig: the 14 replaced bytes, replayed.
    let orig = c.len();
    c.extend_from_slice(&[0x83, 0xF8, 0x01]); // cmp eax, 1
    let jbe_summon = c.len();
    c.extend_from_slice(&[0x76, 0]); // jbe summon
    c.extend_from_slice(&[0x83, 0xF8, 0x02]); // cmp eax, 2
    let jne_ret0 = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne ret0
    c.extend_from_slice(&[0xC6, 0x43, 0x28, 0x01]); // mov byte [rbx+0x28], 1
    let jmp_ret0 = c.len();
    c.extend_from_slice(&[0xEB, 0]); // jmp ret0

    let summon = c.len();
    c.extend_from_slice(&[0xFF, 0x25, 0, 0, 0, 0]); // jmp qword [rip+0]
    c.extend_from_slice(&summon_path.to_le_bytes());

    // ret0 = end of body: install_jmp_hook appends the jump back to
    // `xor al,al` (vanilla's "return 0").
    let ret0 = c.len();

    c[jne_orig + 1] = rel8(jne_orig + 2, orig);
    c[je_off + 1] = rel8(je_off + 2, orig);
    c[jnz_same + 1] = rel8(jnz_same + 2, orig);
    c[jbe_summon + 1] = rel8(jbe_summon + 2, summon);
    c[jne_ret0 + 1] = rel8(jne_ret0 + 2, ret0);
    c[jmp_ret0 + 1] = rel8(jmp_ret0 + 2, ret0);
    c
}

/// Stub for patch 2, replacing `call recall` (rcx/edx already set up).
/// rax is free here (its value was consumed by `mov edx,[rax]`).
fn build_recall_stub(enabled: u64, recall: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(48);
    c.extend_from_slice(&[0x48, 0xB8]); // mov rax, &ENABLED
    c.extend_from_slice(&enabled.to_le_bytes());
    c.extend_from_slice(&[0x80, 0x38, 0x00]); // cmp byte [rax], 0
    let jne_skip = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne skip (feature on)
    c.extend_from_slice(&[0xFF, 0x15, 0x02, 0x00, 0x00, 0x00]); // call qword [rip+2]
    c.extend_from_slice(&[0xEB, 0x08]); // jmp over the address
    c.extend_from_slice(&recall.to_le_bytes());
    let skip = c.len(); // end of body -> appended jump back
    c[jne_skip + 1] = rel8(jne_skip + 2, skip);
    c
}

/// Stub replacing one UI `call GetBuddyState` (args already in rcx/edx):
/// call the real function, then `eax = 0` if the feature is on and the
/// state is >= 1. Reached by `jmp`, so the stack is aligned exactly as it
/// was for the original `call`; r11 is volatile and free after the call.
fn build_ui_call_stub(enabled: u64, get_buddy_state: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(48);
    c.extend_from_slice(&[0xFF, 0x15, 0x02, 0x00, 0x00, 0x00]); // call qword [rip+2]
    c.extend_from_slice(&[0xEB, 0x08]); // jmp over the address
    c.extend_from_slice(&get_buddy_state.to_le_bytes());
    c.extend_from_slice(&[0x49, 0xBB]); // mov r11, &ENABLED
    c.extend_from_slice(&enabled.to_le_bytes());
    c.extend_from_slice(&[0x41, 0x80, 0x3B, 0x00]); // cmp byte [r11], 0
    let je_end = c.len();
    c.extend_from_slice(&[0x74, 0]); // je end (feature off)
    c.extend_from_slice(&[0x83, 0xF8, 0x01]); // cmp eax, 1
    let jl_end = c.len();
    c.extend_from_slice(&[0x7C, 0]); // jl end (state 0 / -1: leave it)
    c.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax
    let end = c.len(); // -> appended jump back
    c[je_end + 1] = rel8(je_end + 2, end);
    c[jl_end + 1] = rel8(jl_end + 2, end);
    c
}

/// Stub replacing `CanUseItem`'s `call GetBuddyState(mgr, goodsId)`. Needs
/// `rcx` (mgr) again for a second call, so it reserves 0x30 bytes: 0x20
/// shadow space for the callee (which spills into it) + a slot for rcx at
/// +0x20 - a multiple of 16, keeping the call-site alignment. rcx/rdx/r11
/// are volatile and the caller reloads rdx right after.
fn build_canuse_stub(enabled: u64, get_buddy_state: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(96);
    let call_gbs = |c: &mut Vec<u8>| {
        c.extend_from_slice(&[0xFF, 0x15, 0x02, 0x00, 0x00, 0x00]); // call qword [rip+2]
        c.extend_from_slice(&[0xEB, 0x08]); // jmp over the address
        c.extend_from_slice(&get_buddy_state.to_le_bytes());
    };
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x30]); // sub rsp, 0x30
    c.extend_from_slice(&[0x48, 0x89, 0x4C, 0x24, 0x20]); // mov [rsp+0x20], rcx
    call_gbs(&mut c); // eax = GetBuddyState(mgr, goodsId)
    c.extend_from_slice(&[0x49, 0xBB]); // mov r11, &ENABLED
    c.extend_from_slice(&enabled.to_le_bytes());
    c.extend_from_slice(&[0x41, 0x80, 0x3B, 0x00]); // cmp byte [r11], 0
    let je_end = c.len();
    c.extend_from_slice(&[0x74, 0]); // je end (feature off)
    c.extend_from_slice(&[0x83, 0xF8, 0xFF]); // cmp eax, -1
    let jne_end = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne end (usable already / active Ash)
    c.extend_from_slice(&[0x48, 0x8B, 0x4C, 0x24, 0x20]); // mov rcx, [rsp+0x20]
    c.extend_from_slice(&[0xBA, 0xFF, 0xFF, 0xFF, 0xFF]); // mov edx, -1
    call_gbs(&mut c); // eax = GetBuddyState(mgr, -1)
    c.extend_from_slice(&[0x83, 0xF8, 0x02]); // cmp eax, 2
    let jne_neg = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne neg
    c.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax -> usable
    let jmp_end = c.len();
    c.extend_from_slice(&[0xEB, 0]); // jmp end
    let neg = c.len();
    c.extend_from_slice(&[0xB8, 0xFF, 0xFF, 0xFF, 0xFF]); // mov eax, -1 (vanilla answer)
    let end = c.len();
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x30]); // add rsp, 0x30 -> appended jump back
    c[je_end + 1] = rel8(je_end + 2, end);
    c[jne_end + 1] = rel8(jne_end + 2, end);
    c[jne_neg + 1] = rel8(jne_neg + 2, neg);
    c[jmp_end + 1] = rel8(jmp_end + 2, end);
    c
}

fn install_canuse() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(CANUSE_AOB, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("MultiSpirit: CanUseItem pattern not found. Game may have been updated - re-check CANUSE_AOB.");
        return false;
    };
    let site = unsafe { anchor.add(CANUSE_CALL_OFFSET) };
    let rel = unsafe { (site.add(1) as *const i32).read_unaligned() };
    let target = (site as u64 + 5).wrapping_add_signed(rel as i64);
    let stub = build_canuse_stub(ENABLED.as_ptr() as u64, target);
    let Some(at) = codepatch::install_jmp_hook(site, 5, &stub) else {
        logger::error("MultiSpirit: CanUseItem hook install failed.");
        return false;
    };
    logger::log(&format!("MultiSpirit: CanUseItem patched at {site:p} (GetBuddyState {target:#x}), stub at {at:p}."));
    true
}

/// Detour body for `DisappearAll(mgr)`: 1 = handled here, skip the
/// original; 0 = run the original (vanilla: dismiss everything).
///
/// Only filters the call that serves a same-Ash re-use - `sub_1404B92B0`
/// calls `DisappearAll` because `disappear_requested` (+0x28) is set, and
/// [DISMISS_TARGET] says which Ash asked for it. Every other reason
/// (leaving the summoning area, death, SpEffect 202, ...) stays vanilla.
/// Like Solid Uncapper: every live entry of another Ash gets its own
/// `disappear_requested` raised for the duration of the original call, so
/// the original skips it, then it is lowered again.
unsafe extern "system" fn disappear_all_hook(manager: *mut SummonBuddyManager) -> u8 {
    if ENABLED.load(Ordering::Relaxed) == 0 || !unsafe { (*manager).disappear_requested } {
        return 0;
    }
    let target = DISMISS_TARGET.swap(-1, Ordering::Relaxed);
    let original = DISAPPEAR_ALL_ORIGINAL.load(Ordering::Relaxed);
    if target == -1 || original == 0 {
        return 0;
    }
    let result = std::panic::catch_unwind(|| {
        let ids = buddy_ids(unsafe { &*manager }, target);
        let mut masked: Vec<*mut bool> = Vec::new();
        let mut target_alive = 0usize;
        let mut others_alive = 0usize;
        for pair in unsafe { &mut *manager }.groups.iter_mut() {
            for group in pair.second.iter_mut() {
                if group.disappear_requested {
                    continue;
                }
                if !group.is_remote && ids.contains(&group.buddy_param_id) {
                    target_alive += 1;
                } else {
                    if !group.is_remote {
                        others_alive += 1;
                    }
                    group.disappear_requested = true;
                    masked.push(&mut group.disappear_requested);
                }
            }
        }
        if target_alive > 0 {
            let active_stone = unsafe { (*manager).active_summmon_buddy_stone_entity_id };
            let original: unsafe extern "system" fn(*mut SummonBuddyManager) -> u64 =
                unsafe { std::mem::transmute(original) };
            unsafe { original(manager) };
            // The original zeroes the active stone; with other groups
            // still out that would trip the "left the summoning area"
            // dismiss-all on the next frame (see patch 5).
            if others_alive > 0 {
                unsafe { (*manager).active_summmon_buddy_stone_entity_id = active_stone };
            }
        }
        for flag in masked {
            unsafe { *flag = false };
        }
        (target_alive, others_alive)
    });
    match result {
        Ok((target_alive, others_alive)) => {
            logger::log(&format!(
                "MultiSpirit: sent back Ash (SpEffect {target}): {target_alive} spirit(s), {others_alive} other(s) kept."
            ));
            // Target already gone: nothing to send back, and the others
            // must not go either - skip the original in both cases.
            1
        }
        Err(_) => 0,
    }
}

/// Entry stub for `DisappearAll`: call [disappear_all_hook] (rcx = mgr
/// already), `ret` if it handled the call, else fall through to the
/// stolen prologue - which, with the appended jump back, is also the
/// "call the original" entry ([DISAPPEAR_ALL_ORIGINAL] = stub + the
/// returned offset). At entry rsp % 16 == 8: `push rcx` + 0x20 shadow space
/// realigns it for the call.
fn build_disappear_all_stub(hook: u64, stolen: &[u8]) -> (Vec<u8>, usize) {
    let mut c: Vec<u8> = Vec::with_capacity(64);
    c.push(0x51); // push rcx
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x20]); // sub rsp, 0x20
    c.extend_from_slice(&[0x48, 0xB8]); // mov rax, disappear_all_hook
    c.extend_from_slice(&hook.to_le_bytes());
    c.extend_from_slice(&[0xFF, 0xD0]); // call rax
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x20]); // add rsp, 0x20
    c.push(0x59); // pop rcx
    c.extend_from_slice(&[0x84, 0xC0]); // test al, al
    c.extend_from_slice(&[0x74, 0x01]); // jz original
    c.push(0xC3); // ret (handled)
    let original = c.len();
    c.extend_from_slice(stolen);
    (c, original)
}

/// Replaces `mov eax,[rbx+0x38]; mov rsi,[rsp+0x38]` (8 bytes): same, but
/// if the feature is on and the current stone is 0, take the active one
/// (`[rbx+0x3C]`) so the following `mov [rbx+0x3C],eax` keeps it. Reached
/// by `jmp`, so `[rsp+0x38]` is the same slot as in the original. r11 is
/// free (volatile, not used by the code that follows before a call).
fn build_stone_stub(enabled: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(40);
    c.extend_from_slice(&[0x8B, 0x43, 0x38]); // mov eax, [rbx+0x38]
    c.extend_from_slice(&[0x85, 0xC0]); // test eax, eax
    let jnz_done = c.len();
    c.extend_from_slice(&[0x75, 0]); // jnz done (current stone valid)
    c.extend_from_slice(&[0x49, 0xBB]); // mov r11, &ENABLED
    c.extend_from_slice(&enabled.to_le_bytes());
    c.extend_from_slice(&[0x41, 0x80, 0x3B, 0x00]); // cmp byte [r11], 0
    let je_done = c.len();
    c.extend_from_slice(&[0x74, 0]); // je done (feature off: vanilla 0)
    c.extend_from_slice(&[0x8B, 0x43, 0x3C]); // mov eax, [rbx+0x3C] (keep active stone)
    let done = c.len();
    c.extend_from_slice(&[0x48, 0x8B, 0x74, 0x24, 0x38]); // mov rsi, [rsp+0x38]
    c[jnz_done + 1] = rel8(jnz_done + 2, done);
    c[je_done + 1] = rel8(je_done + 2, done);
    c
}

fn install_stone_keep() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(DOSUMMON_AOB, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("MultiSpirit: DoSummon pattern not found (stone keep).");
        return false;
    };
    let site = unsafe { anchor.add(DOSUMMON_STONE_OFFSET) };
    if unsafe { std::slice::from_raw_parts(site, 8) } != DOSUMMON_STONE_BYTES {
        logger::error("MultiSpirit: DoSummon stone copy differs from 2.7.1.0 - not patching.");
        return false;
    }
    let Some(at) = codepatch::install_jmp_hook(site, DOSUMMON_STONE_BYTES.len(), &build_stone_stub(ENABLED.as_ptr() as u64)) else {
        logger::error("MultiSpirit: stone keep hook install failed.");
        return false;
    };
    logger::log(&format!("MultiSpirit: DoSummon stone copy patched at {site:p}, stub at {at:p}."));
    true
}

fn install_disappear_all() -> bool {
    let Some(site) = memscan::wait_for_pattern_in_module(DISAPPEAR_ALL_AOB, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("MultiSpirit: DisappearAll pattern not found. Game may have been updated - re-check DISAPPEAR_ALL_AOB.");
        return false;
    };
    let stolen = unsafe { std::slice::from_raw_parts(site, DISAPPEAR_ALL_STOLEN) }.to_vec();
    let (stub, original_offset) = build_disappear_all_stub(disappear_all_hook as *const () as usize as u64, &stolen);
    let Some(at) = codepatch::install_jmp_hook(site, DISAPPEAR_ALL_STOLEN, &stub) else {
        logger::error("MultiSpirit: DisappearAll hook install failed.");
        return false;
    };
    DISAPPEAR_ALL_ORIGINAL.store(at as usize + original_offset, Ordering::Relaxed);
    logger::log(&format!("MultiSpirit: DisappearAll detoured at {site:p}, stub at {at:p}."));
    true
}

fn install_ui_calls() -> bool {
    for (index, (pattern, call_offset)) in UI_CALL_SITES.iter().enumerate() {
        let Some(anchor) = memscan::wait_for_pattern_in_module(pattern, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
            logger::error(&format!("MultiSpirit: UI call site {index} not found. Game may have been updated."));
            return false;
        };
        let site = unsafe { anchor.add(*call_offset) };
        let rel = unsafe { (site.add(1) as *const i32).read_unaligned() };
        let target = (site as u64 + 5).wrapping_add_signed(rel as i64);
        let stub = build_ui_call_stub(ENABLED.as_ptr() as u64, target);
        let Some(at) = codepatch::install_jmp_hook(site, 5, &stub) else {
            logger::error(&format!("MultiSpirit: UI call site {index} hook install failed."));
            return false;
        };
        logger::log(&format!("MultiSpirit: UI call site {index} patched at {site:p} (GetBuddyState {target:#x}), stub at {at:p}."));
    }
    true
}

fn install_dosummon() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(DOSUMMON_AOB, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("MultiSpirit: DoSummon pattern not found. Game may have been updated - re-check DOSUMMON_AOB.");
        return false;
    };
    let site = unsafe { anchor.add(DOSUMMON_HOOK_OFFSET) };
    let summon_path = site as u64 + DOSUMMON_SUMMON_OFFSET as u64;
    // Sanity: the summon path starts with `test edi,edi` (85 FF).
    if unsafe { std::slice::from_raw_parts(summon_path as *const u8, 2) } != [0x85, 0xFF] {
        logger::error("MultiSpirit: DoSummon layout differs from 2.7.1.0 - not patching.");
        return false;
    }
    let stub = build_dosummon_stub(ENABLED.as_ptr() as u64, decide_dismiss as *const () as usize as u64, summon_path);
    match codepatch::install_jmp_hook(site, DOSUMMON_HOOK_LEN, &stub) {
        Some(at) => {
            logger::log(&format!("MultiSpirit: DoSummon patched at {site:p}, stub at {at:p}."));
            true
        }
        None => {
            logger::error("MultiSpirit: DoSummon hook install failed.");
            false
        }
    }
}

fn install_recall() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(RECALL_AOB, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("MultiSpirit: recall pattern not found. Game may have been updated - re-check RECALL_AOB.");
        return false;
    };
    let site = unsafe { anchor.add(RECALL_CALL_OFFSET) };
    let rel = unsafe { (site.add(1) as *const i32).read_unaligned() };
    let recall = (site as u64 + RECALL_CALL_LEN as u64).wrapping_add_signed(rel as i64);
    let stub = build_recall_stub(ENABLED.as_ptr() as u64, recall);
    match codepatch::install_jmp_hook(site, RECALL_CALL_LEN, &stub) {
        Some(at) => {
            logger::log(&format!("MultiSpirit: recall call patched at {site:p} (recall {recall:#x}), stub at {at:p}."));
            true
        }
        None => {
            logger::error("MultiSpirit: recall hook install failed.");
            false
        }
    }
}

/// Installs both patches once, then keeps [ENABLED] in sync with the ini.
/// Never returns.
pub fn run() {
    ENABLED.store(config::get_bool("MultiSpirit", true) as u8, Ordering::Relaxed);
    // Stone keep first: it patches inside the bytes the DoSummon hook
    // leaves alone (0x61 past its anchor vs the hook's 5..19). DisappearAll
    // before DoSummon, so a dismiss target is never set without the detour
    // that consumes it.
    if !(install_stone_keep()
        && install_recall()
        && install_disappear_all()
        && install_dosummon()
        && install_ui_calls()
        && install_canuse())
    {
        logger::error("MultiSpirit: not installed - summoning another Ash still sends the current spirits back.");
        return;
    }

    let cs_task = common::task::wait_for_cs_task();
    let mut elapsed_ms: f64 = 0.0;
    let mut last: Option<bool> = None;
    let _handle = common::task::run_recurring_safe(
        cs_task,
        "MultiSpirit",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;
            let on = config::get_bool("MultiSpirit", true);
            ENABLED.store(on as u8, Ordering::Relaxed);
            if last != Some(on) {
                logger::log(&format!("MultiSpirit={on}."));
                last = Some(on);
            }
        },
    );
    logger::log("MultiSpirit: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints both stubs for manual disassembly (capstone).
    #[test]
    fn dump_stubs() {
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        println!("DOSUMMON_STUB={}", hex(&build_dosummon_stub(0x1122334455667788, 0x99AABBCCDDEEFF00, 0x1404B85EE)));
        println!("RECALL_STUB={}", hex(&build_recall_stub(0x1122334455667788, 0x1404BB9A0)));
        println!("UI_STUB={}", hex(&build_ui_call_stub(0x1122334455667788, 0x1404B72F0)));
        println!("CANUSE_STUB={}", hex(&build_canuse_stub(0x1122334455667788, 0x1404B72F0)));
        println!("STONE_STUB={}", hex(&build_stone_stub(0x1122334455667788)));
        let stolen = [0x40, 0x55, 0x56, 0x57, 0x48, 0x83, 0xEC, 0x30, 0x48, 0xC7, 0x44, 0x24, 0x20, 0xFE, 0xFF, 0xFF, 0xFF];
        let (stub, original) = build_disappear_all_stub(0x99AABBCCDDEEFF00, &stolen);
        println!("DISAPPEAR_STUB={} original@{original:#x}", hex(&stub));
    }
}
