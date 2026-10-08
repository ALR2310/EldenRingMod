//! `MultiSpirit` (hot reload): summon a DIFFERENT Spirit Ash while spirits
//! are already out and the old ones stay, instead of being sent back.
//! Re-using an Ash that is already out sends back only that Ash's spirits.
//!
//! Reverse engineered from Solid Uncapper 2.3.3's "Multi Spirit Coexist"
//! (see README 2026-09-29) and re-implemented here, on `eldenring.exe`
//! 2.7.1.0. **Every patch only redirects an existing `call` (or `jmp`)**
//! ([codepatch::redirect_rel32]: the 4 displacement bytes change, nothing
//! else) - since 2026-10-01. Until 1.1.3 most were inline `jmp` hooks that
//! rewrote the instructions around the call; Seamless Co-op 2.0.1 looks up
//! its own patch sites by byte signature (`E8 ? ? ? ? 83 F8 02 41 0F 44 FC`
//! = patch 7's HP call, among others) and aborted the game with "No such
//! pattern" whenever MultiSpirit was installed. Its signatures wildcard a
//! call's displacement, so a redirected call still matches.
//!
//! 1. `DoSummon` (`sub_1404B85B0`) asks `GetBuddyState(mgr, -1)` and, on
//!    state 2 (a spirit is out), raises `disappear_requested` (+0x28) and
//!    returns - vanilla's "one group at a time". That `call`
//!    ([DOSUMMON_AOB]) goes through a stub: on state 2, [decide_state]
//!    checks whether the requested Ash (SpEffect `edi`) still has live
//!    spirits in `groups` - yes → 2 (dismiss request), no → 0 (summon it
//!    too). (First version compared `edi` with the active/pending SpEffect
//!    `+0x24`/`+0x20` - that only knows the LAST Ash summoned.)
//! 2. `SummonBuddyManager::Update` (`sub_1404B86D0`) calls `sub_1404BB9A0`
//!    to recall the player's whole existing group right before spawning the
//!    new one ([RECALL_AOB]). Skipped while the feature is on.
//! 3. The item UI: the 3 UI callers of `GetBuddyState` ([UI_CALL_SITES]:
//!    `sub_1407C3930` - the "use item" prompt, state 2 = "send the spirit
//!    back?" dialog 20000600 - and the two FP-cost/label helpers
//!    `sub_140847830`/`sub_140847B20`) see any state >= 1 as 0 while the
//!    feature is on (other Ashes no longer greyed out / no dialog).
//! 4. `CanUseItem` (`sub_14068EE60`) marks a buddy item usable only when
//!    `GetBuddyState(mgr, goodsId) != -1` ([CANUSE_AOB]); with spirits out
//!    every other Ash answers -1. On -1 it asks again with goodsId -1; 2
//!    (spirits out, same area) → 0 (usable). Away from any summoning area it
//!    stays -1, like vanilla.
//! 5. `DoSummon` copies the current stone `+0x38` into the active one
//!    `+0x3C` on every summon; `+0x38` can already be 0 by a second summon,
//!    which zeroed `+0x3C` and made `sub_1404B92B0`'s "left the summoning
//!    area" branch dismiss everything ~10 s later (test 3/4, 2026-09-29).
//!    [decide_state] sets `+0x38 = +0x3C` first when it is 0 (until 1.1.3:
//!    an inline patch on the copy itself).
//! 6. Per-Ash send-back: vanilla only has `DisappearAll` (`sub_1404B8160`).
//!    The `call` serving the dismiss request in `sub_1404B92B0` (the
//!    `+0x28` branch, [DISAPPEAR_CALL_AOB]) goes through
//!    [disappear_all_hook]: every live entry of the other Ashes is masked
//!    (its `disappear_requested` raised) around the real call, so only the
//!    pressed Ash's spirits leave - Solid Uncapper's approach. The active
//!    stone `+0x3C` is restored afterwards while others remain. (Until
//!    1.1.3: a detour on `DisappearAll`'s own prologue.)
//! 7. FP/HP cost: `sub_1403C0930` builds an item's use cost; for Ashes it
//!    zeroes the FP (`+0xB8`) and HP (`+0xB4`) cost when `GetBuddyState(mgr,
//!    -1) == 2` (vanilla: using an Ash with a spirit out only sends it back).
//!    Both calls ([COST_CALL_SITES]) ask [cost_state] instead: free only if
//!    the pressed Ash itself has live spirits. The goods row is the
//!    function's `[rbp-0x38]`; an Ash's `refId_default` (+0) is its trigger
//!    SpEffect.
//!
//! All stubs read one byte ([ENABLED]) that the tick below keeps in sync
//! with the ini, so F5 switches the behavior live without re-patching.


use std::sync::atomic::{AtomicI32, AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, SummonBuddyManager};

use common::{codepatch, config, logger, memscan};

/// `call GetBuddyState; cmp eax,1; jbe ..; cmp eax,2; jnz ..; mov byte
/// [rbx+0x28],1` in `DoSummon` (0x1404B85C9 in 2.7.1.0, unique; `edi` =
/// requested SpEffect, `rbx` = manager at the call). The `call` is at +0.
const DOSUMMON_AOB: &str = "E8 ?? ?? ?? ?? 83 F8 01 76 1B 83 F8 02 75 04 C6 43 28 01";

/// `mov edx,[rax]; mov rcx,r15; call recall; cmp dword [r15+0x88],0` in
/// `SummonBuddyManager::Update` (0x1404B89AB in 2.7.1.0). Unique in 2.7.1.0.
const RECALL_AOB: &str = "8B 10 49 8B CF E8 ?? ?? ?? ?? 41 83 BF 88 00 00 00 00";
const RECALL_CALL_OFFSET: usize = 5;

/// The 3 UI callers of `GetBuddyState`: (pattern, offset of the 5-byte
/// `call` within it). Each unique in 2.7.1.0 (0x1407C3C9E, 0x140847A1B,
/// 0x140847CDF - the same patterns Solid Uncapper uses).
const UI_CALL_SITES: [(&str, usize); 3] = [
    ("48 8B C8 83 CA FF E8 ?? ?? ?? ?? 85 C0 74 ?? 83 E8 01", 6),
    ("48 8B C8 8B D3 E8 ?? ?? ?? ?? 83 F8 02 75", 5),
    ("48 8B C8 8B D6 E8 ?? ?? ?? ?? 83 F8 02 75", 5),
];

/// `mov rcx,rax; mov edx,r13d; call GetBuddyState; cmp eax,-1; setnz
/// r13b` in `CanUseItem` (0x14069037D in 2.7.1.0). Unique in 2.7.1.0.
const CANUSE_AOB: &str = "48 8B C8 41 8B D5 E8 ?? ?? ?? ?? 83 F8 FF 41 0F 95 C5";
const CANUSE_CALL_OFFSET: usize = 6;

/// The 2 `call GetBuddyState(mgr, -1)` in the item-cost builder
/// `sub_1403C0930` (FP at 0x1403C0B79, HP at 0x1403C0C2F in 2.7.1.0):
/// (pattern, offset of the 5-byte `call`). Both unique in 2.7.1.0.
const COST_CALL_SITES: [(&str, usize); 2] = [
    ("48 8B C8 41 8B D5 E8 ?? ?? ?? ?? 48 8B 55 C8 83 F8 02 41 0F 44 FC", 6),
    ("48 8B C8 41 8B D5 E8 ?? ?? ?? ?? 83 F8 02 41 0F 44 FC F7 DF 89 BB B4 00 00 00", 6),
];

/// `cmp byte [r15+0x28],0; jz ..; mov rcx,r15; call DisappearAll; cmp
/// dword [r15+0x20],0; mov byte [r15+0x28],0` in `sub_1404B92B0`
/// (0x1404B957D in 2.7.1.0): the dismiss-request branch. The `call` is at
/// +10.
const DISAPPEAR_CALL_AOB: &str = "41 80 7F 28 00 74 08 49 8B CF E8 ?? ?? ?? ?? 41 83 7F 20 00 41 C6 47 28 00";
const DISAPPEAR_CALL_OFFSET: usize = 10;

/// Trigger SpEffect of the Ash whose re-use [decide_state] just turned
/// into a dismiss request; -1 = none pending.
static DISMISS_TARGET: AtomicI32 = AtomicI32::new(-1);
/// `DisappearAll` (the dismiss-request call's original destination). 0
/// until installed.
static DISAPPEAR_ALL: AtomicUsize = AtomicUsize::new(0);

/// 1 = multi-spirit on. Read by every stub.
static ENABLED: AtomicU8 = AtomicU8::new(0);

const TICK_INTERVAL_MS: f64 = 1000.0;
const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

fn rel8(from_next: usize, to: usize) -> u8 {
    (to as i64 - from_next as i64) as i8 as u8
}

/// `call qword [rip+2]; jmp +8; dq target` - an absolute call.
fn push_call(c: &mut Vec<u8>, target: u64) {
    c.extend_from_slice(&[0xFF, 0x15, 0x02, 0x00, 0x00, 0x00]);
    c.extend_from_slice(&[0xEB, 0x08]);
    c.extend_from_slice(&target.to_le_bytes());
}

/// `jmp qword [rip]; dq target` - an absolute tail jump.
fn push_jmp(c: &mut Vec<u8>, target: u64) {
    c.extend_from_slice(&[0xFF, 0x25, 0x00, 0x00, 0x00, 0x00]);
    c.extend_from_slice(&target.to_le_bytes());
}

/// `mov r11, &ENABLED; cmp byte [r11], 0; je <patched later>` - returns
/// the offset of the `je`.
fn push_enabled_check(c: &mut Vec<u8>, enabled: u64) -> usize {
    c.extend_from_slice(&[0x49, 0xBB]);
    c.extend_from_slice(&enabled.to_le_bytes());
    c.extend_from_slice(&[0x41, 0x80, 0x3B, 0x00]);
    let je = c.len();
    c.extend_from_slice(&[0x74, 0]);
    je
}

/// BuddyParam ids the Ash with this trigger SpEffect spawns (its chain in
/// `trigger_speffect_to_buddy_map` - also covers our own chain extension,
/// which only repeats the original ids).
///
/// An upgraded Ash triggers its own SpEffect (goods `refId_default`, e.g.
/// Mimic Tear +4 = 207004) but the map is keyed on the +0 one only;
/// `DoSummon` (`sub_1404B85B0`) rounds it down itself: `100 * (id / 100)`
/// (vanilla: 924 Ash goods -> 86 keys, no two Ashes share one). Same here -
/// without it, re-using a +1..+10 Ash summoned more instead of sending it
/// back, and its send-back was charged FP (Nexus bug report, 2026-10-01).
fn buddy_ids(manager: &SummonBuddyManager, speffect: i32) -> Vec<i32> {
    let key = if speffect >= 0 { 100 * (speffect / 100) } else { speffect };
    manager
        .trigger_speffect_to_buddy_map
        .iter_chains()
        .find(|(k, _)| **k == key)
        .map(|(_, head)| head.iter().copied().collect())
        .unwrap_or_default()
}

/// Whether the Ash with this trigger SpEffect has live spirits of the
/// local player (`groups` entries not already leaving, not remote), or
/// `None` if the SpEffect summons nothing - not a Spirit Ash at all, e.g.
/// ELDEN RING Reforged's Spirit-Severing Blade, which sends spirits back
/// through the same path. Callers keep vanilla behavior for those (until
/// 2026-10-01 they were treated as "a different Ash, summon it", so the
/// Blade summoned nothing and sent nothing back).
fn ash_alive(manager: &SummonBuddyManager, speffect: i32) -> Option<bool> {
    let ids = buddy_ids(manager, speffect);
    if ids.is_empty() {
        return None;
    }
    Some(manager.groups.iter().any(|pair| {
        pair.second
            .iter()
            .any(|g| group_alive(g) && !g.is_remote && ids.contains(&g.buddy_param_id))
    }))
}

/// A `groups` entry that is neither leaving nor dead. A killed spirit keeps
/// its entry for a while (until the game cleans it up), so without the HP
/// check re-using the Ash of a dead spirit - while another Ash still has
/// live ones - counted as "already out" and only sent it back; the next press
/// summoned it (user report, 2026-10-08, Noble Sorcerer + Jellyfish).
fn group_alive(group: &eldenring::cs::SummonBuddyGroup) -> bool {
    !group.disappear_requested && unsafe { group.chr_ins.as_ref() }.modules.data.hp > 0
}

/// Called from the item-cost stubs (patch 7) when `GetBuddyState` said 2
/// (spirits out): 2 = the pressed Ash is out, so this use sends it back -
/// free, like vanilla; 0 = a different Ash, charge its normal cost.
/// `goods_row` = the item's EquipParamGoods row (`refId_default` at +0).
unsafe extern "system" fn cost_state(manager: *mut SummonBuddyManager, goods_row: *const i32) -> u32 {
    if goods_row.is_null() {
        return 2;
    }
    let result = std::panic::catch_unwind(|| ash_alive(unsafe { &*manager }, unsafe { goods_row.read_unaligned() }));
    match result {
        Ok(Some(false)) => 0,
        // The pressed Ash is out, not an Ash at all, or a panic: vanilla.
        Ok(Some(true)) | Ok(None) | Err(_) => 2,
    }
}

/// Called from the DoSummon stub (patch 1) when `GetBuddyState` said 2
/// (spirits out), returns the state `DoSummon` continues with: 2 = this Ash
/// already has live spirits, dismiss it ([DISMISS_TARGET] remembers which
/// one for patch 6); 0 = a different Ash, summon it too - after making sure
/// the current stone is set (patch 5). Runs on the game thread, inside
/// `SummonBuddyManager::Update`.
unsafe extern "system" fn decide_state(manager: *mut SummonBuddyManager, speffect: i32) -> u32 {
    let result = std::panic::catch_unwind(|| {
        let manager = unsafe { &mut *manager };
        match ash_alive(manager, speffect) {
            // Not a Spirit Ash: vanilla dismiss-all, no target.
            None => return 2,
            Some(true) => {
                DISMISS_TARGET.store(speffect, Ordering::Relaxed);
                return 2;
            }
            Some(false) => {}
        }
        if manager.buddy_stone_entity_id == 0 {
            manager.buddy_stone_entity_id = manager.active_summmon_buddy_stone_entity_id;
        }
        0
    });
    // On a panic, fall back to vanilla (dismiss everything).
    result.unwrap_or(2)
}

/// Stub for patch 1, the destination of `DoSummon`'s `call GetBuddyState`
/// (args in rcx/edx; `edi` = requested SpEffect and `rbx` = manager, both
/// set before the call). Entered by that `call` (rsp % 16 == 8): 0x28 =
/// shadow space + realignment around each inner call.
fn build_dosummon_stub(enabled: u64, get_buddy_state: u64, decide: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(96);
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub rsp, 0x28
    push_call(&mut c, get_buddy_state);
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    c.extend_from_slice(&[0x83, 0xF8, 0x02]); // cmp eax, 2
    let jne_ret = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne ret
    let je_ret = push_enabled_check(&mut c, enabled); // feature off: keep 2
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub rsp, 0x28
    c.extend_from_slice(&[0x48, 0x8B, 0xCB]); // mov rcx, rbx
    c.extend_from_slice(&[0x8B, 0xD7]); // mov edx, edi
    c.extend_from_slice(&[0x48, 0xB8]); // mov rax, decide_state
    c.extend_from_slice(&decide.to_le_bytes());
    c.extend_from_slice(&[0xFF, 0xD0]); // call rax -> eax = 2 or 0
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    let ret = c.len();
    c.push(0xC3); // ret
    c[jne_ret + 1] = rel8(jne_ret + 2, ret);
    c[je_ret + 1] = rel8(je_ret + 2, ret);
    c
}

/// Stub for patch 2, the destination of `call recall` (rcx/edx set up):
/// return at once while the feature is on, else tail-jump to the real
/// function. rax is free (the result is not used).
fn build_recall_stub(enabled: u64, recall: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(48);
    c.extend_from_slice(&[0x48, 0xB8]); // mov rax, &ENABLED
    c.extend_from_slice(&enabled.to_le_bytes());
    c.extend_from_slice(&[0x80, 0x38, 0x00]); // cmp byte [rax], 0
    let jne_skip = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne skip (feature on)
    push_jmp(&mut c, recall);
    let skip = c.len();
    c.push(0xC3); // ret
    c[jne_skip + 1] = rel8(jne_skip + 2, skip);
    c
}

/// Stub for patch 3, the destination of one UI `call GetBuddyState`: call
/// the real function, then `eax = 0` if the feature is on and the state is
/// >= 1.
fn build_ui_call_stub(enabled: u64, get_buddy_state: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(48);
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub rsp, 0x28
    push_call(&mut c, get_buddy_state);
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    let je_ret = push_enabled_check(&mut c, enabled);
    c.extend_from_slice(&[0x83, 0xF8, 0x01]); // cmp eax, 1
    let jl_ret = c.len();
    c.extend_from_slice(&[0x7C, 0]); // jl ret (state 0 / -1: leave it)
    c.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax
    let ret = c.len();
    c.push(0xC3); // ret
    c[je_ret + 1] = rel8(je_ret + 2, ret);
    c[jl_ret + 1] = rel8(jl_ret + 2, ret);
    c
}

/// Stub for patch 4, the destination of `CanUseItem`'s `call
/// GetBuddyState(mgr, goodsId)`. Needs rcx (mgr) again for a second call:
/// 0x38 = 0x20 shadow + a slot for rcx at +0x20 + realignment.
fn build_canuse_stub(enabled: u64, get_buddy_state: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(96);
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x38]); // sub rsp, 0x38
    c.extend_from_slice(&[0x48, 0x89, 0x4C, 0x24, 0x20]); // mov [rsp+0x20], rcx
    push_call(&mut c, get_buddy_state); // eax = GetBuddyState(mgr, goodsId)
    let je_end = push_enabled_check(&mut c, enabled);
    c.extend_from_slice(&[0x83, 0xF8, 0xFF]); // cmp eax, -1
    let jne_end = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne end (usable already / active Ash)
    c.extend_from_slice(&[0x48, 0x8B, 0x4C, 0x24, 0x20]); // mov rcx, [rsp+0x20]
    c.extend_from_slice(&[0xBA, 0xFF, 0xFF, 0xFF, 0xFF]); // mov edx, -1
    push_call(&mut c, get_buddy_state); // eax = GetBuddyState(mgr, -1)
    c.extend_from_slice(&[0x83, 0xF8, 0x02]); // cmp eax, 2
    let jne_neg = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne neg
    c.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax -> usable
    let jmp_end = c.len();
    c.extend_from_slice(&[0xEB, 0]); // jmp end
    let neg = c.len();
    c.extend_from_slice(&[0xB8, 0xFF, 0xFF, 0xFF, 0xFF]); // mov eax, -1 (vanilla answer)
    let end = c.len();
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x38]); // add rsp, 0x38
    c.push(0xC3); // ret
    c[je_end + 1] = rel8(je_end + 2, end);
    c[jne_end + 1] = rel8(jne_end + 2, end);
    c[jne_neg + 1] = rel8(jne_neg + 2, neg);
    c[jmp_end + 1] = rel8(jmp_end + 2, end);
    c
}

/// Stub for patch 7, the destination of one `call GetBuddyState(mgr, -1)`
/// in `sub_1403C0930`. The goods row is the caller's `[rbp-0x38]`
/// (rbp-framed; rbp is untouched here). 0x38 as in [build_canuse_stub].
fn build_cost_stub(enabled: u64, get_buddy_state: u64, cost: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(96);
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x38]); // sub rsp, 0x38
    c.extend_from_slice(&[0x48, 0x89, 0x4C, 0x24, 0x20]); // mov [rsp+0x20], rcx
    push_call(&mut c, get_buddy_state);
    c.extend_from_slice(&[0x83, 0xF8, 0x02]); // cmp eax, 2
    let jne_end = c.len();
    c.extend_from_slice(&[0x75, 0]); // jne end (no spirit out: cost stays)
    let je_end = push_enabled_check(&mut c, enabled); // feature off: vanilla free
    c.extend_from_slice(&[0x48, 0x8B, 0x4C, 0x24, 0x20]); // mov rcx, [rsp+0x20]
    c.extend_from_slice(&[0x48, 0x8B, 0x55, 0xC8]); // mov rdx, [rbp-0x38] (goods row)
    c.extend_from_slice(&[0x48, 0xB8]); // mov rax, cost_state
    c.extend_from_slice(&cost.to_le_bytes());
    c.extend_from_slice(&[0xFF, 0xD0]); // call rax -> eax = 2 or 0
    let end = c.len();
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x38]); // add rsp, 0x38
    c.push(0xC3); // ret
    c[jne_end + 1] = rel8(jne_end + 2, end);
    c[je_end + 1] = rel8(je_end + 2, end);
    c
}

/// Patch 6, called by its stub in place of `DisappearAll(mgr)` for the
/// dismiss-request call: 1 = handled here, 0 = let the stub run the real
/// `DisappearAll` (vanilla: dismiss everything).
///
/// Only filters when [DISMISS_TARGET] says which Ash asked for it. Like
/// Solid Uncapper: every live entry of another Ash gets its own
/// `disappear_requested` raised for the duration of the real call, so it
/// skips them, then it is lowered again.
unsafe extern "system" fn disappear_all_hook(manager: *mut SummonBuddyManager) -> u8 {
    if ENABLED.load(Ordering::Relaxed) == 0 || !unsafe { (*manager).disappear_requested } {
        return 0;
    }
    let target = DISMISS_TARGET.swap(-1, Ordering::Relaxed);
    let original = DISAPPEAR_ALL.load(Ordering::Relaxed);
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
            // must not go either - skip the real call in both cases.
            1
        }
        Err(_) => 0,
    }
}

/// Stub for patch 6, the destination of the dismiss-request `call
/// DisappearAll` (rcx = manager): ask [disappear_all_hook]; if it did not
/// handle the call, tail-jump to the real `DisappearAll` with rcx restored.
fn build_disappear_stub(hook: u64, disappear_all: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(64);
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x38]); // sub rsp, 0x38
    c.extend_from_slice(&[0x48, 0x89, 0x4C, 0x24, 0x20]); // mov [rsp+0x20], rcx
    c.extend_from_slice(&[0x48, 0xB8]); // mov rax, disappear_all_hook
    c.extend_from_slice(&hook.to_le_bytes());
    c.extend_from_slice(&[0xFF, 0xD0]); // call rax
    c.extend_from_slice(&[0x48, 0x8B, 0x4C, 0x24, 0x20]); // mov rcx, [rsp+0x20]
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x38]); // add rsp, 0x38
    c.extend_from_slice(&[0x84, 0xC0]); // test al, al
    let jnz_ret = c.len();
    c.extend_from_slice(&[0x75, 0]); // jnz ret (handled)
    push_jmp(&mut c, disappear_all);
    let ret = c.len();
    c.push(0xC3); // ret
    c[jnz_ret + 1] = rel8(jnz_ret + 2, ret);
    c
}

/// Finds `pattern`, takes the `call` at `offset`, and redirects it to
/// `build(original destination)`. Returns the original destination.
fn install(name: &str, pattern: &str, offset: usize, build: impl Fn(u64) -> Vec<u8>) -> Option<u64> {
    let Some(anchor) = memscan::wait_for_pattern_in_module(pattern, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error(&format!("MultiSpirit: {name} pattern not found. Game may have been updated."));
        return None;
    };
    let site = unsafe { anchor.add(offset) };
    let target = codepatch::rel32_target(site)?;
    let Some(at) = codepatch::redirect_rel32(site, &build(target)) else {
        logger::error(&format!("MultiSpirit: {name} redirect failed."));
        return None;
    };
    logger::log(&format!("MultiSpirit: {name} call at {site:p} (was {target:#x}) -> stub {at:p}."));
    Some(target)
}

fn install_all() -> bool {
    let enabled = ENABLED.as_ptr() as u64;
    // DisappearAll first, so a dismiss target is never set without the
    // hook that consumes it.
    let Some(disappear_all) = install("DisappearAll", DISAPPEAR_CALL_AOB, DISAPPEAR_CALL_OFFSET, |t| {
        build_disappear_stub(disappear_all_hook as *const () as usize as u64, t)
    }) else {
        return false;
    };
    DISAPPEAR_ALL.store(disappear_all as usize, Ordering::Relaxed);

    let decide = decide_state as *const () as usize as u64;
    let cost = cost_state as *const () as usize as u64;
    install("recall", RECALL_AOB, RECALL_CALL_OFFSET, |t| build_recall_stub(enabled, t)).is_some()
        && install("DoSummon", DOSUMMON_AOB, 0, |t| build_dosummon_stub(enabled, t, decide)).is_some()
        && UI_CALL_SITES.iter().enumerate().all(|(i, (pattern, offset))| {
            install(&format!("UI site {i}"), pattern, *offset, |t| build_ui_call_stub(enabled, t)).is_some()
        })
        && install("CanUseItem", CANUSE_AOB, CANUSE_CALL_OFFSET, |t| build_canuse_stub(enabled, t)).is_some()
        && COST_CALL_SITES.iter().enumerate().all(|(i, (pattern, offset))| {
            install(&format!("item cost site {i}"), pattern, *offset, |t| build_cost_stub(enabled, t, cost)).is_some()
        })
}

/// Installs every patch once, then keeps [ENABLED] in sync with the ini.
/// Never returns.
pub fn run() {
    ENABLED.store(config::get_bool("MultiSpirit", true) as u8, Ordering::Relaxed);
    if !install_all() {
        logger::error("MultiSpirit: not (fully) installed - summoning another Ash may still send the current spirits back.");
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

    /// Prints every stub for manual disassembly (capstone).
    #[test]
    fn dump_stubs() {
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        let (en, gbs, f) = (0x1122334455667788, 0x1404B72F0, 0x99AABBCCDDEEFF00);
        println!("DOSUMMON_STUB={}", hex(&build_dosummon_stub(en, gbs, f)));
        println!("RECALL_STUB={}", hex(&build_recall_stub(en, 0x1404BB9A0)));
        println!("UI_STUB={}", hex(&build_ui_call_stub(en, gbs)));
        println!("CANUSE_STUB={}", hex(&build_canuse_stub(en, gbs)));
        println!("COST_STUB={}", hex(&build_cost_stub(en, gbs, f)));
        println!("DISAPPEAR_STUB={}", hex(&build_disappear_stub(f, 0x1404B8160)));
    }
}
