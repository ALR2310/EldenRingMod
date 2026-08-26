//! Allows summoning spirit ashes without a rebirth monument nearby -
//! ported from `.docs/x10_summon/er10x.dll`'s `summon_anywhere` option,
//! reverse engineered from its own embedded `Sig` byte patterns
//! (`kSigGate`/`kSigGateEntry`) and independently confirmed against the
//! real `eldenring.exe` via Ghidra:
//!
//! - `kSigGate` (22 bytes) is a small, UNIQUE inner check inside the
//!   summon-gate function: `test al,al; jz+0x10; mov eax,[rbx+0x38]; test
//!   eax,eax; jz+9; cmp eax,[rbx+0x3c]; jz+4; mov al,1; jmp+2; xor al,al`
//!   (a "current slot count < max" style check). Confirmed to match
//!   exactly once, at RVA 0x4b6e0d in the game build this was tested
//!   against.
//! - `kSigGateEntry` (21 bytes, the function's own prologue) matches 3
//!   places in the executable (not unique on its own), but ends up at
//!   exactly `kSigGate`'s match address minus `0x7D` - the same fixed
//!   offset `er10x.dll` itself uses to walk back from the inner check to
//!   the function entry it actually patches. Confirmed: RVA `0x4b6e0d -
//!   0x7D = 0x4b6d90`, and disassembly there is exactly the expected
//!   entry prologue, using the same `WorldChrMan`-relative global
//!   (`0x143d65f88`) and buddy-manager-ish offset (`+0x1e508`) already
//!   independently confirmed while investigating `rune::keep_on_death`.
//!
//! Unlike `er10x.dll`'s own hook (which replays the ENTIRE function's
//! logic itself, calling the original when it needs the real answer),
//! this patches only the function's first 15 bytes (`mov [rsp+8],rbx; mov
//! [rsp+0x10],rsi; push rdi; sub rsp,0x20` - the same instruction-length
//! cut `er10x.dll` itself uses) with a stub that checks the single field
//! `er10x.dll`'s own hook checks (`*(i32*)(this+0x20) < 0`, "no rebirth
//! monument nearby") and returns success immediately when true; otherwise
//! it replays those 4 original instructions and jumps back into the real
//! function, letting vanilla behavior run unchanged. `this` (RCX) and the
//! summon id (RDX) are untouched at this point in the function, since the
//! patched bytes are the very first thing that runs.
//!
//! Simplification, not yet in this port: `er10x.dll` also caches a
//! "summon band busy" check (whether all 10 summon slots are already
//! used) before bypassing, to avoid queuing more requests than the engine
//! can serve. This is omitted here - the engine's own hard 10-slot
//! reservation still caps how many characters can actually spawn
//! regardless, so the risk is at most a harmless extra request being
//! silently dropped, not a crash.

use std::time::Duration;

use common::codepatch;
use common::config;
use common::logger;
use common::memscan;

/// See the module doc comment - matches `kSigGate` from `er10x.dll`.
const GATE_INNER_PATTERN: &str =
    "84 C0 74 10 8B 43 38 85 C0 74 09 3B 43 3C 74 04 B0 01 EB 02 32 C0";
/// Fixed distance from [GATE_INNER_PATTERN]'s match back to the function
/// entry `er10x.dll` itself patches (same value it uses).
const ENTRY_OFFSET_FROM_INNER: usize = 0x7D;
/// The function entry's first 15 bytes - verified against the live game
/// before patching (fail-safe if a game update shifts this layout):
/// `mov [rsp+8],rbx; mov [rsp+0x10],rsi; push rdi; sub rsp,0x20`.
const ENTRY_PREFIX: [u8; 15] = [
    0x48, 0x89, 0x5C, 0x24, 0x08, 0x48, 0x89, 0x74, 0x24, 0x10, 0x57, 0x48, 0x83, 0xEC, 0x20,
];

const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

/// Builds the stub: `cmp dword [rcx+0x20],0; jge replay; xor eax,eax; ret;
/// replay: mov [rsp+8],rbx; mov [rsp+0x10],rsi; push rdi; sub rsp,0x20` -
/// [common::codepatch::install_jmp_hook] appends the jump back into the
/// real function (to entry+15) after this stub's last byte, so the
/// "replay" path falls straight through into vanilla code with the same
/// register/stack state the original prologue would have produced.
fn build_stub() -> Vec<u8> {
    let mut body = Vec::with_capacity(24);

    // cmp dword ptr [rcx+0x20], 0
    body.extend_from_slice(&[0x83, 0x79, 0x20, 0x00]);
    // jge +3 (to "replay", skipping the early-return below)
    body.extend_from_slice(&[0x7D, 0x03]);
    // xor eax,eax
    body.extend_from_slice(&[0x31, 0xC0]);
    // ret
    body.push(0xC3);

    // replay: re-run the original 15-byte prologue we overwrote.
    body.extend_from_slice(&[0x48, 0x89, 0x5C, 0x24, 0x08]); // mov [rsp+8],rbx
    body.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x10]); // mov [rsp+0x10],rsi
    body.push(0x57); // push rdi
    body.extend_from_slice(&[0x48, 0x83, 0xEC, 0x20]); // sub rsp,0x20

    body
}

fn install() -> bool {
    let Some(inner) = memscan::wait_for_pattern_in_module(GATE_INNER_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::log("Spirit.Summon.Anywhere: ERROR - gate pattern not found within the timeout. Game may have been updated - re-check GATE_INNER_PATTERN.");
        return false;
    };
    let entry = unsafe { inner.sub(ENTRY_OFFSET_FROM_INNER) };

    let actual_prefix = unsafe { std::slice::from_raw_parts(entry, ENTRY_PREFIX.len()) };
    if actual_prefix != ENTRY_PREFIX {
        logger::log("Spirit.Summon.Anywhere: ERROR - function entry doesn't match the expected prologue (layout differs from expected), disabled.");
        return false;
    }

    let stub = build_stub();
    let Some(stub_addr) = codepatch::install_jmp_hook(entry, ENTRY_PREFIX.len(), &stub) else {
        logger::log("Spirit.Summon.Anywhere: ERROR - failed to install hook (couldn't allocate stub or patch target).");
        return false;
    };

    logger::log(&format!("Spirit.Summon.Anywhere: gate hook installed at {entry:p}. stub={stub_addr:p}"));
    true
}

/// Installs the summon-gate hook once (retrying the AOB scan for up to
/// [SCAN_TIMEOUT]). Meant to run on its own worker thread spawned from
/// `DllMain`; returns once done (no tick/hotkey loop for this module).
pub fn run() {
    if !config::get_bool("Spirit.Enabled", true) || !config::get_bool("Spirit.Summon.Anywhere", false) {
        logger::log("Spirit.Summon.Anywhere=false (or Spirit.Enabled=false) - skipping entirely at startup.");
        return;
    }

    if !install() {
        logger::log("Spirit.Summon.Anywhere disabled for this session (hook install failed).");
    }
}
