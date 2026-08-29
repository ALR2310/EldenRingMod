//! Lets you open the map and fast-travel from anywhere - including
//! mid-combat and inside dungeons/caves, where the game normally blocks
//! both - ported from `.docs/FastTravel/Zibinha_FastTravel.dll` (a
//! reference-only, non-Rust tool in this repo), decompiled with Ghidra to
//! recover its exact patch bytes rather than guessing
//! (`Zibinha_MapInCombat.dll` in the same folder is a strict subset: same
//! first 2 patches, without the third). Three independent techniques, same
//! as that DLL:
//!
//! 1. **`map_check`**: `call <is-map-blocked-in-combat check>; test al,al;
//!    jnz +0x11` - the `call` (5 bytes) becomes `xor rax,rax; nop; nop`, so
//!    the check function never runs and its result (`al`) is always 0,
//!    meaning the following `jnz` (the "map is blocked" branch) is never
//!    taken.
//! 2. **`warp_block`**: `je +0x2E` guarding a menu path becomes `jmp +0x2E`
//!    (byte `0x74` -> `0xEB`, same displacement) - unconditionally takes
//!    whatever branch the game takes when warping IS allowed.
//! 3. **`field_area_unlock`**: resolves `FieldArea`'s global pointer slot
//!    once via the `mov rcx,[rip+disp32]` RIP-relative load at the head of
//!    its own AOB (standard disp32 resolution: `slot_addr = match_addr + 7
//!    + disp32`), then re-reads that slot every tick and zeroes offset
//!    `+0xA0` of whatever instance it currently points to - this looked in
//!    Ghidra like the flag/counter that blocks fast travel while inside a
//!    dungeon/cave. Not in `fromsoftware-rs`'s reflected `FieldArea`
//!    binding (confirmed: no field at that offset), so this stays a raw
//!    pointer poke, same as the other two patches. The reference DLL
//!    re-polls this every 100ms from a `Sleep`-looping thread because
//!    `FieldArea` isn't allocated yet when the DLL first loads; this uses a
//!    `CSTaskImp` tick on `CSTaskGroupIndex::FrameBegin` instead, same
//!    reasoning as every other per-frame check in this crate.
//!
//! All three patches are found and applied once at startup - unlike
//! `unlock_shop`'s snapshot+restore pattern, there's no "original value" to
//! restore for a code patch, so like `torrent_anywhere`/
//! `unlock_ashes_of_war` this key has no hot-reload; restart the game with
//! `WarpAnywhere=false` to go back to vanilla.

use std::time::Duration;

use eldenring::cs::CSTaskGroupIndex;
use eldenring::fd4::FD4TaskData;

use common::codepatch;
use common::config;
use common::logger;
use common::memscan;

const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

/// `call <map-blocked-in-combat check>; test al,al; jnz +0x11; cmp byte
/// [rbx+0x3EC2],al; jnz +9; and esi,0xFFFFFFFE; mov [rsp+0x40],esi` -> the
/// `call` (5 bytes) becomes `xor rax,rax; nop; nop`.
const MAP_CHECK_PATTERN: &str = "E8 ?? ?? ?? ?? 84 C0 75 11 38 83 C2 3E 00 00 75 09 83 E6 FE 89 74 24 40";
const MAP_CHECK_PATCHED: [u8; 5] = [0x48, 0x31, 0xC0, 0x90, 0x90];

/// `je +0x2E; mov dword[rbp+0x50],0x258; mov dword[rbp+0x54],2; mov
/// dword[rbp+0x58],1` -> `je` (`74 2E`) becomes `jmp` (`EB 2E`).
const WARP_BLOCK_PATTERN: &str = "74 2E C7 45 50 58 02 00 00 C7 45 54 02 00 00 00 C7 45 58 01 00 00 00";
const WARP_BLOCK_PATCHED: [u8; 2] = [0xEB, 0x2E];

/// `mov rcx,[rip+disp32]; ...; movzx r12d,byte[rcx+??]; call ...; movsxd
/// rax,dword[rdi+??]; ...; test rax,rax` - only the `mov rcx,[rip+disp32]`
/// head (offset 0..7) is used, to resolve `FieldArea`'s global pointer slot.
const FIELD_AREA_PATTERN: &str =
    "48 8B 0D ?? ?? ?? ?? 48 ?? ?? ?? 44 0F B6 61 ?? E8 ?? ?? ?? ?? 48 63 87 ?? ?? ?? ?? 48 ?? ?? ?? 48 85 C0";
const FIELD_AREA_INSTANCE_OFFSET: usize = 0xA0;

fn apply_map_check() -> bool {
    match memscan::wait_for_pattern_in_module(MAP_CHECK_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) {
        Some(addr) if unsafe { codepatch::overwrite_bytes(addr, &MAP_CHECK_PATCHED) } => {
            logger::log(&format!("WarpAnywhere: map_check patched at {addr:p}."));
            true
        }
        _ => {
            logger::error("WarpAnywhere: map_check pattern not found/patch failed.");
            false
        }
    }
}

fn apply_warp_block() -> bool {
    match memscan::wait_for_pattern_in_module(WARP_BLOCK_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) {
        Some(addr) if unsafe { codepatch::overwrite_bytes(addr, &WARP_BLOCK_PATCHED) } => {
            logger::log(&format!("WarpAnywhere: warp_block patched at {addr:p}."));
            true
        }
        _ => {
            logger::error("WarpAnywhere: warp_block pattern not found/patch failed.");
            false
        }
    }
}

/// Resolves `FieldArea`'s global pointer-slot address from the AOB match's
/// `mov rcx,[rip+disp32]` head - standard RIP-relative resolution:
/// `match_addr + 7 (instruction length) + disp32`. Returns the address of
/// the slot itself (not its current contents) - the slot's address is fixed
/// for the life of the process, but the pointer stored inside it changes as
/// `FieldArea` gets allocated/reallocated, so it must be re-read every tick.
fn resolve_field_area_slot() -> Option<usize> {
    let addr = memscan::wait_for_pattern_in_module(FIELD_AREA_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT)?;
    let disp = unsafe { std::ptr::read_unaligned(addr.add(3) as *const i32) };
    let slot = unsafe { addr.add(7).offset(disp as isize) };
    logger::log(&format!("WarpAnywhere: field_area AOB at {addr:p}, global slot at {slot:p}."));
    Some(slot as usize)
}

/// Patches `map_check`/`warp_block` once, then registers a tick on the
/// game's own `FrameBegin` task group that keeps `FieldArea`'s `+0xA0`
/// field zeroed for as long as an instance exists. Meant to run on its own
/// worker thread spawned from `DllMain`; never returns.
pub fn run() {
    if !config::get_bool("WarpAnywhere", false) {
        logger::log("WarpAnywhere=false - skipping entirely at startup.");
        return;
    }

    apply_map_check();
    apply_warp_block();

    let Some(field_area_slot) = resolve_field_area_slot() else {
        logger::error(
            "WarpAnywhere: field_area pattern not found, dungeon/cave warp unlock disabled for this session.",
        );
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    };

    let cs_task = crate::task::wait_for_cs_task();
    let mut logged_once = false;

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "WarpAnywhere",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &FD4TaskData| {
            let instance = unsafe { std::ptr::read(field_area_slot as *const usize) };
            if instance == 0 {
                return;
            }
            unsafe { *((instance + FIELD_AREA_INSTANCE_OFFSET) as *mut i32) = 0 };
            if !logged_once {
                logger::log("WarpAnywhere: FieldArea instance found, dungeon/cave warp unlock applied.");
                logged_once = true;
            }
        },
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
