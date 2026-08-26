//! Multiplies rune gains from every source by a configurable factor, ported
//! from [`RuneMultiplier`](../../../runemultiplier)'s `hook.rs` - same single
//! hook on `AddSoul_Call` (the game's lowest-level "add this many runes"
//! function, used by both enemy kills and rune items), same absolute
//! `mov reg, imm64; jmp reg` redirect/stub-return jumps (the same technique
//! `regen/attack_hook.rs` already uses in this crate). Kept under the
//! `Rune.Multiplier` key (`[Rune Reward]` section - grouped alongside the
//! rest of SomeTweaks' own rune-related keys, unlike the standalone crate's
//! own `Multiplier`/`[Settings]` naming) instead of renaming again.
//!
//! Disassembly at `AddSoul_Call` (see `RuneMultiplier/README.md` for the full
//! reverse-engineering writeup):
//!
//! ```text
//! MOV R9D,[RCX+0x6C]      ; R9D = current rune count
//! LEA R8D,[R9+RDX*1]      ; sum = current + amount (RDX = amount to add)
//! CMP R8D,0x3B9AC9FF      ; clamp to the 999,999,999 cap
//! ...
//! MOV [RCX+0x6C],EAX      ; write new rune count - used by every source
//! RET
//! ```
//!
//! The multiply has to happen to EDX *before* any of that runs, so the first
//! 3 instructions (12 bytes: `mov r9d,[rcx+0x6c]` + `xor r11d,r11d` +
//! `mov [rsp+0x10],r11d` - none of which read RDX/EDX) are overwritten with a
//! redirect to a generated stub that multiplies EDX, re-runs those 3
//! instructions (copied live from the game, never hardcoded), then jumps back
//! into `AddSoul_Call` right after them.

use std::ffi::c_void;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use eldenring::cs::CSTaskGroupIndex;

use common::config;
use common::logger;
use common::memscan;

unsafe extern "system" {
    fn VirtualAlloc(lp_address: *mut c_void, dw_size: usize, fl_allocation_type: u32, fl_protect: u32) -> *mut c_void;
    fn VirtualProtect(lp_address: *mut c_void, dw_size: usize, fl_new_protect: u32, lpfl_old_protect: *mut u32) -> i32;
    fn FlushInstructionCache(h_process: *mut c_void, lp_base_address: *const c_void, dw_size: usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
}

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

// Only used to LOCATE AddSoul_Call - not patched itself. Matches the
// "Rune Multiplier" CT table entry for the enemy-kill reward scaling step
// (`mulss xmm0,xmm1`); kept purely as a stable anchor since AddSoul_Call
// itself has no equally distinctive byte pattern of its own to scan for.
const ANCHOR_PATTERN: &str = "F3 0F 59 C1 F3 0F 2C F8 48 8B 8B";

// "call AddSoul_Call" sits at anchor+0x11 (mulss[4] + cvttss2si[4] +
// "mov rcx,[rbx+0x570]"[7] + "mov edx,edi"[2] = 17 bytes).
const ADDSOUL_CALL_SITE_OFFSET: usize = 0x11;

// First 3 instructions of AddSoul_Call that get overwritten - none of them
// read RDX/EDX (the amount about to be multiplied), so the multiply can run
// first and the copied originals re-run after with the new value in place.
const ADDSOUL_PREFIX_LEN: usize = 12;

// The multiplier as a Q20 fixed-point integer (value = round(multiplier *
// 2^20)), read by the injected stub through a pointer baked in at install
// time. Hot-reload updates this value directly - the stub always re-reads it
// through the pointer, so no re-patching is ever needed.
static FIXED_Q20: AtomicI64 = AtomicI64::new(1 << 20);

/// Re-reads `Rune.Multiplier` from the shared config and stores it into
/// [FIXED_Q20], logging only when it actually changed - called once at
/// startup and then every tick, so `General.ReloadKey` (polled by
/// `reload::run`, which already reloads the shared config map) picks up a new
/// value without this module needing to watch the key itself.
fn apply_multiplier() {
    let multiplier = config::get_double("Rune.Multiplier", 1.0);
    let fixed = (multiplier * (1i64 << 20) as f64 + 0.5) as i64;
    let previous = FIXED_Q20.swap(fixed, Ordering::Relaxed);
    if previous != fixed {
        logger::log(&format!("Rune.Multiplier={multiplier:.3}"));
    }
}

/// Reads a 5-byte "E8 rel32" CALL instruction at `call_site` and returns its
/// absolute target, or `None` if the byte there isn't 0xE8 (layout differs
/// from expected - fail safe rather than jumping into the wrong place).
fn resolve_call_target(call_site: *const u8) -> Option<*mut u8> {
    unsafe {
        if *call_site != 0xE8 {
            return None;
        }
        let rel = i32::from_le_bytes(*(call_site.add(1) as *const [u8; 4]));
        Some(call_site.add(5).offset(rel as isize) as *mut u8)
    }
}

/// Builds the stub's machine code: multiply EDX by [FIXED_Q20] using
/// integer-only Q20 fixed-point math (deliberately avoiding XMM registers -
/// `AddSoul_Call` is called far more often, from far more call sites, than
/// any single hook site, so nothing here should assume any XMM register is
/// safe to clobber), then re-run `original_prefix` (the 12 bytes overwritten
/// at the patch site) followed by an absolute jump back to
/// `return_addr` (`addsoul_entry + ADDSOUL_PREFIX_LEN`).
///
/// `AddSoul_Call` is the single generic "current += amount" function, reused
/// for every rune-count change - gains (kill rewards, item pickups) AND
/// spends (level-up cost, shop purchases) alike, both passed as a signed
/// EDX. Multiplying unconditionally would scale spends too (over-deducting
/// on level-up, inflating shop prices), so the multiply block is skipped
/// whenever the amount is <= 0.
fn build_stub(original_prefix: &[u8; ADDSOUL_PREFIX_LEN], return_addr: *const u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(50);

    // movsxd rax, edx (sign-extend the amount to 64-bit)
    body.extend_from_slice(&[0x48, 0x63, 0xC2]);

    // test eax, eax; jle skip_multiply (rel8) - only scale gains (amount >
    // 0); leave spends (amount <= 0) untouched so level-up costs and shop
    // prices aren't affected.
    body.extend_from_slice(&[0x85, 0xC0]);
    let multiply_block_len: u8 = 10 + 3 + 4 + 4 + 2; // mov r10,imm64 + mov r8,[r10] + imul + sar + mov edx,eax
    body.extend_from_slice(&[0x7E, multiply_block_len]);

    // mov r10, &FIXED_Q20
    body.extend_from_slice(&[0x49, 0xBA]);
    body.extend_from_slice(&(&FIXED_Q20 as *const AtomicI64 as u64).to_le_bytes());

    // mov r8, qword ptr [r10]
    body.extend_from_slice(&[0x4D, 0x8B, 0x02]);

    // imul rax, r8
    body.extend_from_slice(&[0x49, 0x0F, 0xAF, 0xC0]);

    // sar rax, 20 (undo the Q20 scale, arithmetic shift keeps sign)
    body.extend_from_slice(&[0x48, 0xC1, 0xF8, 0x14]);

    // mov edx, eax (write the scaled amount back where AddSoul_Call expects it)
    body.extend_from_slice(&[0x89, 0xC2]);

    // skip_multiply: re-run the original first 3 instructions we had to overwrite.
    body.extend_from_slice(original_prefix);

    // mov r11, return_addr; jmp r11 - absolute jump back, so the stub can
    // live anywhere in the 64-bit address space regardless of how far it
    // ends up from AddSoul_Call.
    body.extend_from_slice(&[0x49, 0xBB]);
    body.extend_from_slice(&(return_addr as u64).to_le_bytes());
    body.extend_from_slice(&[0x41, 0xFF, 0xE3]);

    body
}

fn hex_dump(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X} ")).collect()
}

/// Scans for the anchor pattern, resolves `AddSoul_Call` from it, and patches
/// its first 12 bytes to redirect into a freshly allocated stub. Returns
/// `false` (and logs why) on any failure - the original bytes are left
/// untouched in that case.
fn install(debug_log: bool) -> bool {
    let Some(anchor) = memscan::find_pattern_in_module(ANCHOR_PATTERN) else {
        logger::log("RuneMultiplier: ERROR - anchor pattern not found (used to locate AddSoul_Call). Game may have been updated - re-check ANCHOR_PATTERN.");
        return false;
    };
    logger::log(&format!("RuneMultiplier: anchor found at {anchor:p} (known-good offset is +0x630CB3)."));

    let call_site = unsafe { anchor.add(ADDSOUL_CALL_SITE_OFFSET) };
    let Some(addsoul_entry) = resolve_call_target(call_site) else {
        logger::log("RuneMultiplier: ERROR - couldn't resolve AddSoul_Call from the anchor - byte layout differs from expected.");
        return false;
    };
    logger::log(&format!("RuneMultiplier: AddSoul_Call resolved at {addsoul_entry:p}."));

    let original_prefix: [u8; ADDSOUL_PREFIX_LEN] =
        unsafe { std::slice::from_raw_parts(addsoul_entry, ADDSOUL_PREFIX_LEN) }
            .try_into()
            .unwrap();
    if debug_log {
        logger::log(&format!("RuneMultiplier: AddSoul_Call original prefix bytes: {}", hex_dump(&original_prefix)));
    }

    let return_addr = unsafe { addsoul_entry.add(ADDSOUL_PREFIX_LEN) };
    let stub_body = build_stub(&original_prefix, return_addr);
    if debug_log {
        logger::log(&format!("RuneMultiplier: AddSoul_Call stub body bytes: {}", hex_dump(&stub_body)));
    }

    let stub = unsafe {
        VirtualAlloc(std::ptr::null_mut(), stub_body.len(), MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE)
    };
    if stub.is_null() {
        logger::log("RuneMultiplier: ERROR - VirtualAlloc failed for the stub.");
        return false;
    }
    unsafe { std::ptr::copy_nonoverlapping(stub_body.as_ptr(), stub as *mut u8, stub_body.len()) };

    // Patch: mov rax, <stub>; jmp rax (10 + 2 = 12 bytes) - fills the entire
    // overwritten region exactly, no NOP padding needed.
    let mut patch = [0u8; ADDSOUL_PREFIX_LEN];
    patch[0] = 0x48;
    patch[1] = 0xB8; // mov rax, imm64
    patch[2..10].copy_from_slice(&(stub as u64).to_le_bytes());
    patch[10] = 0xFF;
    patch[11] = 0xE0; // jmp rax

    let mut old_protect: u32 = 0;
    let ok = unsafe { VirtualProtect(addsoul_entry as *mut c_void, ADDSOUL_PREFIX_LEN, PAGE_EXECUTE_READWRITE, &mut old_protect) };
    if ok == 0 {
        logger::log("RuneMultiplier: ERROR - VirtualProtect failed, RuneMultiplier disabled.");
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(patch.as_ptr(), addsoul_entry, ADDSOUL_PREFIX_LEN);
        VirtualProtect(addsoul_entry as *mut c_void, ADDSOUL_PREFIX_LEN, old_protect, &mut old_protect);
        FlushInstructionCache(GetCurrentProcess(), addsoul_entry as *const c_void, ADDSOUL_PREFIX_LEN);
        FlushInstructionCache(GetCurrentProcess(), stub as *const c_void, stub_body.len());
    }

    logger::log(&format!("RuneMultiplier: AddSoul_Call hook installed. stub={stub:p}"));
    true
}

/// Installs the hook, then re-applies `RuneMultiplier` every tick on the
/// game's own `FrameBegin` task group for the rest of the DLL's lifetime -
/// hot reload doesn't need to re-patch anything, the stub always reads
/// [FIXED_Q20] through a pointer, so just keeping that value current is
/// enough. Meant to run on its own worker thread spawned from `DllMain`;
/// never returns (except early, if the hook fails to install - the anchor
/// pattern not being found doesn't affect any other module, so this only
/// disables RuneMultiplier for the session rather than the whole DLL).
pub fn run() {
    apply_multiplier();

    let debug_log = config::get_bool("DebugLog", false);
    if !install(debug_log) {
        logger::log("RuneMultiplier disabled for this session (hook install failed).");
        return;
    }

    let cs_task = crate::task::wait_for_cs_task("RuneMultiplier");
    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "RuneMultiplier",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| {
            apply_multiplier();
        },
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
