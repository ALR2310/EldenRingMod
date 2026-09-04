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
//!   the function entry it actually patches.
//!
//! `er10x.dll` (and this module, originally) patches the function's own
//! ENTRY (`kSigGateEntry`, walking back `0x7D` bytes from the inner
//! check) - **abandoned 2026-09-04**: `Seamless Co-op`'s `ersc.dll` hooks
//! that exact same entry for its own purposes (it needs to gate summons
//! for co-op sessions too), so whichever of the two mods patches it last
//! wins and the other's hook never installs - confirmed via this crate's
//! own fail-safe prologue check logging "function entry doesn't match
//! the expected prologue" whenever `ersc.dll` loads first. Rather than
//! fight over the same address (or try to chain into `ersc.dll`'s own
//! hook, fragile and liable to break on its updates), this patches
//! `kSigGate` ITSELF instead - deeper into the function, past whatever
//! `ersc.dll` touches at entry, and confirmed still byte-identical
//! (unpatched) even with `ersc.dll` active (same AOB scan still finds it,
//! only the entry-prologue check downstream of it used to fail).
//!
//! The patch: prepend a check of the same field `er10x.dll`'s own entry
//! hook reads (`*(i32*)(rbx+0x20) < 0`, "no rebirth monument nearby",
//! `rbx` holding `this` at this point in the function same as at entry)
//! before the original 22 bytes - if true, force `al = 1` (this block's
//! own "eligible" result) and skip them entirely; otherwise replay them
//! verbatim, unchanged behavior. `rbx`/`al`'s incoming value are both
//! exactly what vanilla already has at this address (nothing upstream of
//! it in the function was touched, by us or by `ersc.dll`), so the
//! "replay" path is provably identical to un-patched vanilla. Also grants
//! free resummon (press the ash again, get a fresh set instantly, no
//! waiting) as a side effect - vanilla's own cooldown check runs later in
//! this same function and never gets a chance to block anything once
//! this stub has already forced eligibility to succeed.
//!
//! ## Hook B: area eligibility
//!
//! `.docs/SummonAnywhere/src/summon_anywhere.cpp` (traces back to
//! `soarqin/ER-EzMod`, MIT) has a 2nd, unrelated hook worth keeping: some
//! areas (dungeons, certain boss arenas) flat-out disallow summoning via a
//! small struct at `[rbp-68]` whose `+0x20`/`+0x1c` fields represent "this
//! area forbids it". Clearing both fields whenever the struct exists makes
//! summoning allowed everywhere. Re-verified against this project's own
//! copy of `eldenring.exe` (2.7.0.0) before porting - matched exactly once
//! (unique), byte for byte, at RVA `0xea5860`, via a standalone Python
//! PE-section scanner (`.text`-only, no full Ghidra re-analysis needed).
//!
//! ## Dead end: "Hook A" / summon-range leash (reverted 2026-09-04)
//!
//! The same `.docs/SummonAnywhere/src/summon_anywhere.cpp` also has a
//! "Hook A" that was ported alongside Hook B above, patching
//! `mov rax,[rdi+28]; movss xmmN,[rax+84]` to force a hardcoded 1000.0
//! whenever the referenced object's id is the sentinel `0x7D0` (2000) -
//! this DID stop spirits from despawning after a short while, but a
//! deeper disassembly of the surrounding function (capstone, no Ghidra)
//! revealed `[rax+0x84]` isn't a despawn leash at all - it's
//! `SummonBuddyManager`/`SummonBuddyWarpManager`'s own
//! `trigger_dist_to_player` (the distance at which a lagging buddy gets
//! WARPED closer to the player, a convenience/anti-stuck mechanic, not
//! the despawn gate). Forcing it to 1000.0 doesn't disable a check - it
//! actively ENABLES a warp/position-sync code branch that was previously
//! always skipped (the sentinel's own uninitialized `+0x84` defaulted to
//! 0, which always satisfied the "already close enough, skip" condition).
//! That branch calls several more functions using position data from the
//! same sentinel object, which was never configured with a real position
//! - user-confirmed 2026-09-04: at one specific Grace ("cổng bão"/Stone
//! gate), a freshly summoned spirit walked in a fixed compass direction
//! (not toward the player) until reaching the forest edge, then finally
//! started fighting - consistent with pathing toward a stale/zeroed
//! coordinate rather than the player's actual position. Not reproduced at
//! several other Graces tested, consistent with the target coordinate
//! being map-relative (so its apparent direction/distance from the
//! summon point differs per location). Removed - see
//! [install_despawn_skip] below for the actual, safer fix for the
//! original despawn problem.
//!
//! ## The real despawn fix: `kSigDespawn` (2026-09-04)
//!
//! Went back to `er10x.dll`'s own `kSigDespawn` signature (found in the
//! decompiled dump - `D:/tmp/er10x_v2_dump.txt` - months ago, but never
//! actually ported; a software-fallback field-zero was used instead, then
//! removed as insufficient - see the "Also grants unlimited resummon"/
//! "gate alone isn't enough" history further down in this crate's own git
//! history / README). Its 27-byte anchor still matches exactly once
//! against this project's own current `eldenring.exe` (2.7.0.0) -
//! re-verified independently via a Python `capstone` disassembly (not
//! copied from the old dump's addresses, which are for a different game
//! build). Context around the match makes the mechanism clear:
//!
//! ```text
//! mov rcx, [rcx+0x1e508]     ; WorldChrMan.summon_buddy_manager
//! ...
//! cmp dword [r15+0x20], 0
//! mov byte [r15+0x28], 0     ; <- pattern starts here
//! jge skip                  ; if >= 0, skip the despawn check entirely
//! cmp byte [r15+0xb7], 0
//! jne skip                  ; flag set -> skip
//! cmp byte [r15+0xb5], 0
//! jne skip                  ; flag set -> skip
//! call cleanup(r15)          ; only reached when all 3 conditions hold
//! skip:
//! ```
//!
//! `[r15+0x20] < 0` reads as a countdown/timer going negative, and
//! `+0xb7`/`+0xb5` land in the same region of `SummonBuddyManager` as
//! `is_within_activation_range`/`is_within_warn_range` (per
//! `fromsoftware-rs`'s own field layout) - i.e. this is the actual
//! "has the summon been out of range for too long" despawn decision, not
//! anything to do with the warp-distance field Hook A mistakenly patched.
//! The same cleanup function is also called from 2 OTHER, unrelated
//! trigger sites just above this one (not touched here), so this patch
//! only removes the distance-based auto-despawn, not every legitimate way
//! a summon can end (e.g. the player manually dismissing it still works).
//!
//! Patch: flip the `jge`'s opcode byte (`7D` -> `EB`, same rel8 operand
//! reused) so it's an unconditional `jmp` - the despawn-decision block
//! never runs. A single-byte in-place edit, same
//! `common::codepatch::overwrite_bytes` technique `torrent_anywhere.rs`
//! uses for its own conditional-jump flips - no codecave, no risk of the
//! stale-position bug the abandoned Hook A introduced.
//!
//! ## Dead end: patching the resummon-cooldown helper directly (reverted 2026-09-04)
//!
//! Went looking for a separate fix for er10x.dll's `unlimited_resummon`
//! (NOPing a `jne` that skips a helper checking
//! `SummonBuddyManager.item_use_cooldown_timer`/`.groups`, plus a tick
//! forcing `is_within_activation_range`/`active_summmon_buddy_stone_entity_id`
//! etc.) before realizing the gate hook above already gives free resummon
//! for free (see its own doc comment) - this was unnecessary. Worse. it
//! was actively harmful: with both installed, summons lost their
//! name/HP-bar overlay and despawned after a few seconds, in EVERY area
//! (not just monument-free ones) - confirmed by testing with the whole
//! feature off (fine), on but this extra patch skipped (fine), and on
//! with it (broken). Almost certainly because the skipped helper does
//! more than gate-check - probably also registers/tracks the summon as a
//! side effect, and unconditionally zeroing
//! `active_summmon_buddy_stone_entity_id` every frame stomped legitimate
//! tracking for ordinary near-monument summons too. Removed entirely.

use std::time::Duration;

use common::codepatch;
use common::config;
use common::logger;
use common::memscan;

/// See the module doc comment - matches `kSigGate` from `er10x.dll`. Also
/// the direct patch target now (see "abandoned" note above) - this is no
/// longer just an anchor to walk back from.
const GATE_INNER_PATTERN: &str =
    "84 C0 74 10 8B 43 38 85 C0 74 09 3B 43 3C 74 04 B0 01 EB 02 32 C0";
const GATE_INNER_LEN: usize = 22;

const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

/// Builds the stub: `cmp dword [rbx+0x20],0; jl bypass; <replay the
/// original 22 bytes>; jmp resume; bypass: mov al,1` -
/// [common::codepatch::install_jmp_hook] appends the jump back into the
/// real function (to `entry+GATE_INNER_LEN`) right after this stub's last
/// byte (`resume`), so both the "replay" and "bypass" paths converge
/// there with `al` holding this block's own "eligible" result either way.
fn build_stub() -> Vec<u8> {
    let mut body = Vec::with_capacity(32);

    // cmp dword ptr [rbx+0x20], 0
    body.extend_from_slice(&[0x83, 0x7B, 0x20, 0x00]);
    let jl_pos = body.len();
    body.extend_from_slice(&[0x7C, 0]); // jl bypass (patched below)

    // replay: re-run the original 22 bytes verbatim (see module doc
    // comment - nothing upstream of this address was touched by us or by
    // ersc.dll, so this is provably identical to un-patched vanilla).
    body.extend_from_slice(&[0x84, 0xC0]); // test al,al
    body.extend_from_slice(&[0x74, 0x10]); // jz +0x10
    body.extend_from_slice(&[0x8B, 0x43, 0x38]); // mov eax,[rbx+0x38]
    body.extend_from_slice(&[0x85, 0xC0]); // test eax,eax
    body.extend_from_slice(&[0x74, 0x09]); // jz +9
    body.extend_from_slice(&[0x3B, 0x43, 0x3C]); // cmp eax,[rbx+0x3c]
    body.extend_from_slice(&[0x74, 0x04]); // jz +4
    body.extend_from_slice(&[0xB0, 0x01]); // mov al,1
    body.extend_from_slice(&[0xEB, 0x02]); // jmp +2
    body.extend_from_slice(&[0x32, 0xC0]); // xor al,al

    let jmp_pos = body.len();
    body.extend_from_slice(&[0xEB, 0]); // jmp resume (patched below, skips bypass)

    let bypass_pos = body.len();
    body.extend_from_slice(&[0xB0, 0x01]); // bypass: mov al,1

    let resume_pos = body.len();
    body[jl_pos + 1] = (bypass_pos as i64 - (jl_pos as i64 + 2)) as u8;
    body[jmp_pos + 1] = (resume_pos as i64 - (jmp_pos as i64 + 2)) as u8;

    body
}

// --- Despawn trigger skip (replaces the old "Hook A", see module doc comment) ---
// Same 27-byte anchor er10x.dll's own `kSigDespawn` uses - re-derived
// independently against this project's own eldenring.exe copy (2.7.0.0),
// not copied from the old dump, and confirmed unique before patching.
const DESPAWN_TRIGGER_PATTERN: &str =
    "41 C6 47 28 00 7D 1C 41 80 BF B7 00 00 00 00 75 12 41 80 BF B5 00 00 00 00 75 08";
/// Offset within [DESPAWN_TRIGGER_PATTERN] of the `jge` opcode byte (`7D`) -
/// changed to `EB` (unconditional `jmp`, same rel8 operand reused) so the
/// despawn-decision block is always skipped.
const DESPAWN_JGE_OFFSET: usize = 5;
const JMP_OPCODE: u8 = 0xEB;

fn install_despawn_skip() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(DESPAWN_TRIGGER_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("Spirit.Summon.Anywhere: despawn-trigger pattern not found within the timeout. Game may have been updated - re-check DESPAWN_TRIGGER_PATTERN.");
        return false;
    };
    let jge = unsafe { anchor.add(DESPAWN_JGE_OFFSET) };
    if !unsafe { codepatch::overwrite_bytes(jge, &[JMP_OPCODE]) } {
        logger::error("Spirit.Summon.Anywhere: despawn-trigger patch failed.");
        return false;
    }
    logger::log(&format!("Spirit.Summon.Anywhere: despawn-trigger jge->jmp patched at {jge:p}."));
    true
}

// --- Hook B: area eligibility (see module doc comment) -----------------
const AREA_ELIGIBILITY_PATTERN: &str = "48 8B 45 98 48 85 C0 0F 84 ?? ?? ?? ?? 8B 40 20";
const AREA_ELIGIBILITY_HOOK_LEN: usize = 7;

/// `mov rax,[rbp-0x68]; test rax,rax; je resume;
/// mov dword[rax+20],0; mov word[rax+1c],0xFFFF; test rax,rax;`
/// (resume = end of body - the redundant final `test rax,rax` leaves
/// EFLAGS set correctly for the `je` immediately after the bytes this
/// hook overwrites, which vanilla still executes once control resumes).
fn build_area_eligibility_stub() -> Vec<u8> {
    let mut body = Vec::with_capacity(29);

    body.extend_from_slice(&[0x48, 0x8B, 0x45, 0x98]); // mov rax,[rbp-0x68]
    body.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax,rax

    let je_pos = body.len();
    body.extend_from_slice(&[0x0F, 0x84, 0, 0, 0, 0]); // je resume (patched below)

    body.extend_from_slice(&[0xC7, 0x40, 0x20, 0x00, 0x00, 0x00, 0x00]); // mov dword[rax+0x20],0
    body.extend_from_slice(&[0x66, 0xC7, 0x40, 0x1C, 0xFF, 0xFF]); // mov word[rax+0x1c],0xFFFF

    // `je` lands HERE - the null path re-tests rax (already known null,
    // but done anyway to match the reference cave exactly) so both paths
    // converge on the same flags before falling into the auto-appended
    // jump back.
    let final_test_pos = body.len();
    body.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax,rax

    let je_rel = (final_test_pos as i64 - (je_pos as i64 + 6)) as i32;
    body[je_pos + 2..je_pos + 6].copy_from_slice(&je_rel.to_le_bytes());

    body
}

fn install_area_eligibility() -> bool {
    let Some(entry) = memscan::wait_for_pattern_in_module(AREA_ELIGIBILITY_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("Spirit.Summon.Anywhere: AreaEligibility pattern not found within the timeout. Game may have been updated - see .docs/SummonAnywhere's own version caveat.");
        return false;
    };
    let stub = build_area_eligibility_stub();
    let Some(stub_addr) = codepatch::install_jmp_hook(entry, AREA_ELIGIBILITY_HOOK_LEN, &stub) else {
        logger::error("Spirit.Summon.Anywhere: AreaEligibility - failed to install hook.");
        return false;
    };
    logger::log(&format!("Spirit.Summon.Anywhere: AreaEligibility hook installed at {entry:p}. stub={stub_addr:p}"));
    true
}

fn install_gate() -> bool {
    let Some(entry) = memscan::wait_for_pattern_in_module(GATE_INNER_PATTERN, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("Spirit.Summon.Anywhere: gate pattern not found within the timeout. Game may have been updated - re-check GATE_INNER_PATTERN.");
        return false;
    };

    let stub = build_stub();
    let Some(stub_addr) = codepatch::install_jmp_hook(entry, GATE_INNER_LEN, &stub) else {
        logger::error("Spirit.Summon.Anywhere: failed to install hook (couldn't allocate stub or patch target).");
        return false;
    };

    logger::log(&format!("Spirit.Summon.Anywhere: gate hook installed at {entry:p}. stub={stub_addr:p}"));
    true
}

/// Installs all 3 hooks once (retrying each AOB scan for up to
/// [SCAN_TIMEOUT]): the cast-time gate (`er10x.dll`, also grants free
/// resummon as a side effect - see module doc comment), and the
/// summon-range + area-eligibility pair (`.docs/SummonAnywhere`) that
/// fixes summons despawning on their own. Each logs its own
/// success/failure independently. Meant to run on its own worker thread
/// spawned from `DllMain`; never returns.
pub fn run() {
    if !config::get_bool("Spirit.Enabled", true) || !config::get_bool("Spirit.Summon.Anywhere", false) {
        logger::log("Spirit.Summon.Anywhere=false (or Spirit.Enabled=false) - skipping entirely at startup.");
        return;
    }

    install_gate();
    install_despawn_skip();
    install_area_eligibility();

    logger::log("Spirit.Summon.Anywhere: spirits can now be summoned without a Summoning Pool nearby.");
    logger::log("Spirit.Summon.Anywhere: an Ash can also be resummoned instantly, without resting at a Grace first.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
