//! Prevents the player's currently-held rune count from being lost on
//! death - by NOPing the exact `CALL` that moves current runes into a
//! death bloodstain, port of the technique reverse engineered from
//! `.docs/DisableRuneLoss.dll` (see this crate's README's "Giai ma
//! DisableRuneLoss.dll" entry for the full writeup), confirmed against the
//! real `eldenring.exe` via Ghidra (RVA `0x594f6c`; the `CALL` sits at
//! `+5` within the matched pattern).
//!
//! An earlier version of this feature instead set `ChrIns::chr_flags1c6
//! .has_dropped_runes()` on the player's own death flag, reusing a typed
//! `fromsoftware-rs` field whose doc comment ("prevents dead character
//! from rewarding runes twice") looked like the right lock - **confirmed
//! in-game NOT to work** (2026-08-25, rune still dropped as normal). That
//! flag is almost certainly for the "this NPC's death grants a rune
//! reward to its killer" bookkeeping (shared by every `ChrIns`, enemies
//! included), unrelated to the player's OWN held-rune-loss-on-death
//! system - which, per this AOB, isn't gated by anything on `ChrIns` at
//! all. The AOB-scan-and-NOP technique below is what actually works -
//! **confirmed in-game** (2026-08-25).
//!
//! Applied once at startup only, no hot-reload (same convention as
//! `misc::torrent_anywhere`'s code patches) - restart the game with
//! `Rune.KeepOnDeath=false` to go back to vanilla.

use std::time::Duration;

use common::codepatch;
use common::config;
use common::logger;
use common::memscan;

/// `mov r8b,1; mov rdx,rbx; call <drop-runes-into-bloodstain>; mov
/// rbx,[rsp+0x20]; xor al,al; add rsp,0x28; ret` - the `CALL` (byte `0xE8`
/// at offset 5 within this match) is what actually moves the player's
/// current rune count into a bloodstain record and zeroes it.
const CALL_SITE_PATTERN: &str = "b0 01 ? 8b ? e8 ? ? ? ? ? 8b ? ? ? 32 c0 ? 83 ? 28 c3";
const CALL_OFFSET: usize = 5;
const CALL_OPCODE: u8 = 0xE8;
const NOP5: [u8; 5] = [0x90, 0x90, 0x90, 0x90, 0x90];

const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

/// Scans for [CALL_SITE_PATTERN], verifies the byte at `+`[CALL_OFFSET] is
/// really [CALL_OPCODE] (fail-safe if the game updated and shifted this
/// layout), then NOPs the 5-byte `CALL` there. Returns `false` (and logs
/// why) on any failure - the original bytes are left untouched in that
/// case.
fn apply_patch() -> bool {
    let Some(addr) = memscan::wait_for_pattern_in_module(CALL_SITE_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::log("Rune.KeepOnDeath: ERROR - pattern not found within the timeout. Game may have been updated - re-check CALL_SITE_PATTERN.");
        return false;
    };
    let call_site = unsafe { addr.add(CALL_OFFSET) };
    if unsafe { *call_site } != CALL_OPCODE {
        logger::log("Rune.KeepOnDeath: ERROR - byte at the expected CALL site isn't 0xE8 (layout differs from expected), disabled.");
        return false;
    }
    if !unsafe { codepatch::overwrite_bytes(call_site, &NOP5) } {
        logger::log("Rune.KeepOnDeath: ERROR - VirtualProtect failed, disabled.");
        return false;
    }
    logger::log(&format!("Rune.KeepOnDeath: rune-loss-on-death CALL NOPed at {call_site:p}."));
    true
}

/// Applies the patch once. Meant to run on its own worker thread spawned
/// from `DllMain`; returns once done (no tick/hotkey loop for this
/// module).
pub fn run() {
    if !config::get_bool("Rune.KeepOnDeath", false) {
        logger::log("Rune.KeepOnDeath=false - skipping entirely at startup.");
        return;
    }

    if !apply_patch() {
        logger::log("Rune.KeepOnDeath disabled for this session (patch failed).");
    }
}
