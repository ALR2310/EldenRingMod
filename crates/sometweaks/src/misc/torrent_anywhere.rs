//! Allows riding Torrent (the spectral horse) anywhere, including areas that
//! normally force a dismount - port of `.docs/torent_anywhere/
//! CavaloLivre_V7_Cardoso.dll` (a reference-only, non-Rust tool in this
//! repo), decompiled with Ghidra to recover its exact patch bytes rather
//! than guessing. Three independent techniques, same as that DLL:
//!
//! 1. **`area_list_check`/`direct_ride_check`**: two near-identical
//!    functions that both do `ptr = [this+0x68]; restricted = ptr[0x36] !=
//!    0; return restricted` (`cmp byte [ptr+0x36],0; setne al`) - these gate
//!    whether the current area/situation permits riding. Neither `ptr+0x68`
//!    nor the byte at `+0x36` is reflected by `fromsoftware-rs` (confirmed
//!    by search - not on `ChrIns`/`PlayerIns`/`WorldChrMan`/
//!    `CSChrDataModule`, nor any ride/mount/area-restriction struct it
//!    defines), so this stays a raw in-place code patch: `cmp+setne`
//!    (6 bytes) becomes `mov byte [ptr+0x36],0` (permanently clears the
//!    restriction flag itself, not just this one check) followed by
//!    `xor al,al` (forces the return value to "not restricted" too) - same
//!    total instruction length, so this overwrites in place with no
//!    jump/stub needed at all (unlike `weight_multiplier`'s
//!    `common::codepatch`, which redirects because its replacement doesn't
//!    fit in the original bytes).
//! 2. **`abyssal_forced_dismount_skip`**: skips the forced-dismount call
//!    that runs when SpEffect `19995` ("Forced Torrent Dismount Abyssal
//!    Woods") is active, by flipping a single byte: the `jz +0x10` (`74
//!    10`) guarding `call [rax+0xB0]` (the actual dismount) becomes `jmp
//!    +0x10` (`EB 10`) - skips that call unconditionally, regardless of
//!    whether the player actually has the effect.
//! 3. **SpEffect `19996`** ("Remove Forced Torrent Dismount Abyssal Woods")
//!    is re-applied to the player every tick, continuously counteracting
//!    `19995` in case the game re-applies it. Unlike the reference DLL
//!    (which resolves the game's own `ApplyEffect` function via its own
//!    AOB scan, then calls it through a raw function pointer from a
//!    `Sleep`-looping thread), this uses `fromsoftware-rs`'s own
//!    `ChrInsExt::apply_speffect` (resolved via the crate's built-in RVA
//!    table, not a manual AOB) inside a `CSTaskImp` tick registered on
//!    `CSTaskGroupIndex::FrameBegin` - the same pattern `regen.rs` and the
//!    library's own `examples/apply-speffect` use for periodic SpEffect
//!    application, safer than a sleeping thread poking `WorldChrMan` off
//!    the game's own frame schedule.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, ChrInsExt, WorldChrMan};
use eldenring::fd4::FD4TaskData;
use fromsoftware_shared::FromStatic;

use common::codepatch;
use common::config;
use common::logger;
use common::memscan;

// SpEffect that removes the forced-dismount debuff the game applies in
// Abyssal Woods - re-applying it every tick keeps that debuff from ever
// taking hold, on top of the dismount-skip code patch below (belt and
// braces, matching the reference DLL's own design: it does both rather
// than relying on either alone).
const REMOVE_FORCED_DISMOUNT_SPEFFECT: i32 = 19996;
const APPLY_INTERVAL_MS: f64 = 1000.0;

const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

/// `mov rax,[rax+0x68]; cmp byte[rax+0x36],0; setne al; mov dil,1; mov
/// [rsi],al` -> `mov rax,[rax+0x68]; mov byte[rax+0x36],0; xor al,al; mov
/// dil,1; mov [rsi],al` (same 16 bytes, in place).
const AREA_LIST_CHECK_PATTERN: &str = "48 8B 40 68 80 78 36 00 0F 95 C0 40 B7 01 88 06";
const AREA_LIST_CHECK_PATCHED: [u8; 16] = [
    0x48, 0x8B, 0x40, 0x68, 0xC6, 0x40, 0x36, 0x00, 0x32, 0xC0, 0x40, 0xB7, 0x01, 0x88, 0x06, 0x90,
];

/// `mov rcx,[rax+0x68]; cmp byte[rcx+0x36],0; setne al; add rsp,0x28; ret`
/// -> `mov rcx,[rax+0x68]; mov byte[rcx+0x36],0; xor al,al; add rsp,0x28;
/// ret` (same 16 bytes, in place).
const DIRECT_RIDE_CHECK_PATTERN: &str = "48 8B 48 68 80 79 36 00 0F 95 C0 48 83 C4 28 C3";
const DIRECT_RIDE_CHECK_PATCHED: [u8; 16] = [
    0x48, 0x8B, 0x48, 0x68, 0xC6, 0x41, 0x36, 0x00, 0x32, 0xC0, 0x48, 0x83, 0xC4, 0x28, 0xC3, 0x90,
];

/// `mov edx,0x4E1B(19995); mov rcx,rbx; call HasSpEffect; test al,al; jz
/// +0x10; mov rcx,[rbx+0x6A0]; mov rax,[rcx]; call [rax+0xB0]` - the `jz`
/// (byte `0x74` at offset 15) becomes `jmp` (`0xEB`), skipping the
/// forced-dismount vtable call unconditionally.
const ABYSSAL_DISMOUNT_SKIP_PATTERN: &str =
    "BA 1B 4E 00 00 48 8B CB E8 ?? ?? ?? ?? 84 C0 74 10 48 8B 8B A0 06 00 00 48 8B 01 FF 90 B0 00 00 00";
const ABYSSAL_JZ_OFFSET: usize = 15;
const ABYSSAL_JMP_OPCODE: u8 = 0xEB;

fn apply_code_patches() -> usize {
    let mut applied = 0;

    match memscan::wait_for_pattern_in_module(AREA_LIST_CHECK_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) {
        Some(addr) if unsafe { codepatch::overwrite_bytes(addr, &AREA_LIST_CHECK_PATCHED) } => {
            logger::log(&format!("TorrentAnywhere: area_list_check patched at {addr:p}."));
            applied += 1;
        }
        _ => logger::log("TorrentAnywhere: ERROR - area_list_check pattern not found/patch failed."),
    }

    match memscan::wait_for_pattern_in_module(DIRECT_RIDE_CHECK_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) {
        Some(addr) if unsafe { codepatch::overwrite_bytes(addr, &DIRECT_RIDE_CHECK_PATCHED) } => {
            logger::log(&format!("TorrentAnywhere: direct_ride_check patched at {addr:p}."));
            applied += 1;
        }
        _ => logger::log("TorrentAnywhere: ERROR - direct_ride_check pattern not found/patch failed."),
    }

    match memscan::wait_for_pattern_in_module(ABYSSAL_DISMOUNT_SKIP_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) {
        Some(addr) if unsafe { codepatch::overwrite_bytes(addr.add(ABYSSAL_JZ_OFFSET), &[ABYSSAL_JMP_OPCODE]) } => {
            logger::log(&format!("TorrentAnywhere: abyssal_forced_dismount_skip patched at {addr:p}."));
            applied += 1;
        }
        _ => logger::log("TorrentAnywhere: ERROR - abyssal_forced_dismount_skip pattern not found/patch failed."),
    }

    applied
}

/// Patches the 2 area-restriction checks and the Abyssal Woods dismount
/// skip once (retrying each AOB scan for up to [SCAN_TIMEOUT]), then
/// registers a tick on the game's own `FrameBegin` task group that
/// re-applies [REMOVE_FORCED_DISMOUNT_SPEFFECT] to the player every
/// [APPLY_INTERVAL_MS]. Meant to run on its own worker thread spawned from
/// `DllMain`; never returns.
pub fn run() {
    if !config::get_bool("TorrentAnywhere", false) {
        logger::log("TorrentAnywhere=false - skipping entirely at startup.");
        return;
    }

    let applied = apply_code_patches();
    logger::log(&format!("TorrentAnywhere: {applied}/3 code patch(es) applied."));

    let cs_task = crate::task::wait_for_cs_task("TorrentAnywhere");
    let mut elapsed_ms: f64 = 0.0;

    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "TorrentAnywhere",
        CSTaskGroupIndex::FrameBegin,
        move |data: &FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < APPLY_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
                return;
            };
            let Some(main_player) = world_chr_man.main_player.as_mut() else {
                return;
            };
            main_player.chr_ins.apply_speffect(REMOVE_FORCED_DISMOUNT_SPEFFECT, true);
        },
    );

    logger::log("TorrentAnywhere: SpEffect reapply tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
