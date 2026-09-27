//! Widens the local player's spirit-summon "band" in
//! `WorldChrMan.summon_buddy_chr_set` from the engine's hard-coded 10
//! slots to `MaxSpirits` ([band_len]) - the real reason no amount of chain lengthening
//! (`chain.rs`, `sometweaks`'s `summon_count.rs`, `er10x.dll`) could ever
//! spawn more than 10 spirits.
//!
//! Found in IDA on `eldenring.exe` 2.7.1.0 (2026-09-25), confirmed by
//! `probe.rs`'s in-game slot log (see README):
//!
//! - `sub_140493380(chr_set, player_index, spawn_info, cursor)` - the
//!   summon ChrSet's "spawn into my band" function, called once per spirit
//!   from the SummonBuddyManager spawn routine (`sub_1404BAEA0`, which
//!   passes the local player's session index, `< 6`, and
//!   `SummonBuddyManager::last_buddy_slot` as `cursor`):
//!   ```text
//!   band_start = 10 * (player_index + 2)   // player 0 -> slot 20
//!   cursor %= 10
//!   for i in 0..10 { slot = band_start + (cursor + i) wrapped to 10;
//!                    if entries[slot] is empty -> spawn there }
//!   return null                             // band full - spirit dropped
//!   ```
//!   The ChrSet's capacity is 80 = 20 + 6 bands * 10, so offline (player 0
//!   alone) slots 30..79 are never used by anything.
//! - `sub_140493D60(spawned_chr)` - the next cursor:
//!   `(slot_of(spawned_chr) - 20 + 1) % 10`.
//!
//! Patches:
//! 1. [BAND_ALLOC_AOB]: the slot-search loop (everything from computing
//!    `band_start` up to the "band full" exit) is replaced by a stub that
//!    does the same search over the band [band_layout] gives the calling
//!    player index, bounds-checked against the ChrSet's own `capacity`. Exits exactly where vanilla does: "found" jumps to
//!    the original spawn code with `rbx` = entry, `edi` = slot index; "not
//!    found" falls through to the original `mov rax, r15` (null) return.
//! 2. [NEXT_CURSOR_AOB]: the next-cursor computation stops wrapping
//!    (`(slot - 20 + 1) % 10` -> `slot - 20 + 1`): it has no idea which
//!    player's band `slot` is in, so patch 1 turns that absolute value
//!    back into a position inside the caller's own band instead. (First
//!    version did `% band_len` here, which is only right for a band that
//!    starts at slot 20 - partners' spirits ended up scattered across
//!    their band, Seamless test 2026-09-26.)
//!
//! 3. [CAPACITY_INIT_AOB] (only when `MaxSpirits > 60`): the summon
//!    ChrSet's own init (`sub_1404954B0`) allocates a fixed 80-entry array
//!    (`alloc(0x500)`, `capacity = 0x50`, `memset(.., 0x500)`) - those
//!    three imm32s are raised to `20 + MaxSpirits` so the game builds a
//!    bigger array itself, instead of this mod reallocating a live one.
//!    Must run before the world is created (the DLL loads at process
//!    start); if it ran too late the array just stays at 80, and patch 1's
//!    `slot < capacity` check keeps everything in bounds regardless. Slot
//!    indices are encoded in FieldInsHandles with a 20-bit mask
//!    (`0xFFFFF`), so large capacities are representable.
//!
//! Player index 0 is the host (and the only player offline). Co-op
//! partners (index 1..5) get bands of their own, laid out by [band_layout]
//! from the ChrSet's real capacity so they never overlap player 0's - a
//! remote spirit is created on every machine at its OWNER's slot number
//! (`sub_140493210`, which also does no bounds check), so bands must not
//! overlap across players. With vanilla's 80-slot capacity that layout
//! collapses to exactly vanilla's 10-slot bands; with Seamless Co-op's
//! 1000 (observed 2026-09-25) every player gets up to 163. Limits: a
//! partner WITHOUT this mod still uses vanilla's 30..79 bands, which do
//! overlap player 0's widened band; and all machines are assumed to share
//! the same capacity (true under Seamless, which sets it on everyone).

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use common::codepatch;
use common::logger;
use common::memscan;

/// First slot of player 0's band (`10 * (0 + 2)`); slots below it hold
/// each player's Torrent (slot 0 = own, 1.. = co-op partners', seen in the
/// Seamless Co-op test 2026-09-25).
pub const BAND_START: u32 = 20;
/// Session player indices the spawn routine accepts (`< 6`).
const MAX_PLAYERS: u32 = 6;
/// The summon ChrSet's vanilla capacity.
const VANILLA_CAPACITY: u32 = 80;
/// `MaxSpirits` default: everything vanilla's 80-slot array can hold above
/// [BAND_START] - needs no capacity patch.
pub const DEFAULT_MAX_SPIRITS: u32 = VANILLA_CAPACITY - BAND_START;
/// Upper bound on `MaxSpirits` - a sanity guard, not a known engine limit.
pub const MAX_MAX_SPIRITS: u32 = 1000;

/// Player 0's band length, fixed once by [install] from `MaxSpirits`.
static BAND_LEN: AtomicU32 = AtomicU32::new(DEFAULT_MAX_SPIRITS);

/// Player 0's band length = the most spirits one cast can actually spawn.
pub fn band_len() -> u32 {
    BAND_LEN.load(Ordering::Relaxed)
}

/// `mov edx,0x500; mov r8d,8; call [rax+0x50]; mov [rsi+0x18],rax;
/// test rax,rax; jz ..; mov edi,0x50; mov [rsi+0x10],edi; xor edx,edx;
/// mov r8d,0x500` in the summon ChrSet init `sub_1404954B0` (0x140495534 in
/// 2.7.1.0). Unique in 2.7.1.0.
const CAPACITY_INIT_AOB: &str =
    "BA 00 05 00 00 41 B8 08 00 00 00 FF 50 50 48 89 46 18 48 85 C0 74 ?? BF 50 00 00 00 89 7E 10 33 D2 41 B8 00 05 00 00";
/// Offsets of the three imm32s: alloc size, capacity (also the init loop
/// count), memset size.
const CAPACITY_ALLOC_SIZE_OFFSET: usize = 0x01;
const CAPACITY_COUNT_OFFSET: usize = 0x18;
const CAPACITY_MEMSET_SIZE_OFFSET: usize = 0x23;
/// `sizeof(ChrSetEntry)`.
const CHR_SET_ENTRY_SIZE: u32 = 16;

/// `movzx eax,dl; add eax,2; lea eax,[rax+rax*4]; lea r10d,[rax+rax];
/// mov eax,0x66666667; imul r9d` - start of the band search in
/// `sub_140493380` (0x1404933AF in 2.7.1.0). Unique in 2.7.1.0.
const BAND_ALLOC_AOB: &str = "0F B6 C2 83 C0 02 8D 04 80 44 8D 14 00 B8 67 66 66 66 41 F7 E9";
/// Bytes replaced, up to the "band full" exit (`mov rax, r15` at
/// 0x140493419) which the auto-appended jump-back lands on.
const BAND_ALLOC_REPLACED_LEN: usize = 0x6A;
/// Offset of the "slot found" continuation (`call
/// __current_exception_context` at 0x140493421).
const BAND_ALLOC_FOUND_OFFSET: usize = 0x72;
/// Sanity checks on the two exits before patching: `mov rax, r15` and a
/// `call rel32`.
const BAND_ALLOC_NOT_FOUND_BYTES: [u8; 3] = [0x49, 0x8B, 0xC7];
const CALL_REL32_OPCODE: u8 = 0xE8;

/// `sub eax,0x14; js ...; lea ecx,[rax+1]; mov eax,0x66666667; imul ecx;
/// sar edx,2` in `sub_140493D60` (0x140493D7C in 2.7.1.0). Unique in
/// 2.7.1.0.
const NEXT_CURSOR_AOB: &str = "83 E8 14 78 22 8D 48 01 B8 67 66 66 66 F7 E9 C1 FA 02";
/// Patch starts at `lea ecx,[rax+1]` and runs up to the `js` target
/// (`xor eax,eax` at 0x140493DA3).
const NEXT_CURSOR_PATCH_OFFSET: usize = 5;
const NEXT_CURSOR_PATCH_LEN: usize = 0x22;

const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const SCAN_TIMEOUT: Duration = Duration::from_secs(60);

/// Band `(start, length)` for `player_index` - documentation/test mirror of
/// what [build_band_stub]'s machine code computes.
#[cfg(test)]
fn band_layout(player_index: u32, band_len: u32, capacity: u32) -> (u32, u32) {
    if player_index == 0 {
        return (BAND_START, band_len);
    }
    let part = (capacity - BAND_START) / MAX_PLAYERS;
    (BAND_START + player_index * part, band_len.min(part))
}

/// Replacement for the band search. Entry state (from vanilla): `dl` =
/// player index, `r9d` = cursor, `r14` = the ChrSet, `r15` = 0; `rax`,
/// `rcx`, `rdx`, `r8`-`r11` are free (vanilla overwrites them all here).
fn build_band_stub(band_len: u32, found_addr: u64) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(128);

    // r10d = band start, r11d = band length (see [band_layout]).
    c.extend_from_slice(&[0x44, 0x0F, 0xB6, 0xDA]); // movzx r11d, dl (player index)
    c.extend_from_slice(&[0x45, 0x85, 0xDB]); // test r11d, r11d
    let jnz_others_pos = c.len();
    c.extend_from_slice(&[0x75, 0]); // jnz others (patched below)

    // Player 0: [BAND_START, BAND_START + band_len).
    c.extend_from_slice(&[0x41, 0xBA]); // mov r10d, BAND_START
    c.extend_from_slice(&BAND_START.to_le_bytes());
    c.extend_from_slice(&[0x41, 0xBB]); // mov r11d, band_len
    c.extend_from_slice(&band_len.to_le_bytes());
    let jmp_cursor_pos = c.len();
    c.extend_from_slice(&[0xEB, 0]); // jmp cursor (patched below)

    // Players 1..5: part = (capacity - BAND_START) / 6,
    // start = BAND_START + index * part, length = min(band_len, part).
    let others_pos = c.len();
    c.extend_from_slice(&[0x41, 0x8B, 0x46, 0x10]); // mov eax, [r14+0x10] (capacity)
    c.extend_from_slice(&[0x83, 0xE8, BAND_START as u8]); // sub eax, BAND_START
    c.extend_from_slice(&[0x31, 0xD2]); // xor edx, edx
    c.push(0xB9); // mov ecx, MAX_PLAYERS
    c.extend_from_slice(&MAX_PLAYERS.to_le_bytes());
    c.extend_from_slice(&[0xF7, 0xF1]); // div ecx (eax = part)
    c.extend_from_slice(&[0x41, 0x89, 0xC2]); // mov r10d, eax
    c.extend_from_slice(&[0x45, 0x0F, 0xAF, 0xD3]); // imul r10d, r11d
    c.extend_from_slice(&[0x41, 0x83, 0xC2, BAND_START as u8]); // add r10d, BAND_START
    c.extend_from_slice(&[0x41, 0xBB]); // mov r11d, band_len
    c.extend_from_slice(&band_len.to_le_bytes());
    c.extend_from_slice(&[0x41, 0x39, 0xC3]); // cmp r11d, eax
    c.extend_from_slice(&[0x76, 0x03]); // jbe +3 (band_len <= part)
    c.extend_from_slice(&[0x41, 0x89, 0xC3]); // mov r11d, eax

    // cursor: r9d = (cursor - (start - BAND_START)) mod r11d, kept
    // non-negative. `cursor` is `last_slot - 19` from the patched next-
    // cursor function (or 0 before the first summon), i.e. relative to
    // slot 20 - rebase it onto this caller's own band start.
    let cursor_pos = c.len();
    c.extend_from_slice(&[0x44, 0x89, 0xC8]); // mov eax, r9d
    c.extend_from_slice(&[0x44, 0x29, 0xD0]); // sub eax, r10d
    c.extend_from_slice(&[0x83, 0xC0, BAND_START as u8]); // add eax, BAND_START
    c.push(0x99); // cdq
    c.extend_from_slice(&[0x41, 0xF7, 0xFB]); // idiv r11d
    c.extend_from_slice(&[0x85, 0xD2]); // test edx, edx
    c.extend_from_slice(&[0x79, 0x03]); // jns +3
    c.extend_from_slice(&[0x44, 0x01, 0xDA]); // add edx, r11d
    c.extend_from_slice(&[0x41, 0x89, 0xD1]); // mov r9d, edx

    c.extend_from_slice(&[0x4D, 0x8B, 0x46, 0x18]); // mov r8, [r14+0x18] (entries)
    c.extend_from_slice(&[0x31, 0xC9]); // xor ecx, ecx (i = 0)

    let loop_top = c.len();
    c.extend_from_slice(&[0x44, 0x89, 0xC8]); // mov eax, r9d
    c.extend_from_slice(&[0x01, 0xC8]); // add eax, ecx
    c.extend_from_slice(&[0x44, 0x39, 0xD8]); // cmp eax, r11d
    c.extend_from_slice(&[0x7C, 0x03]); // jl +3
    c.extend_from_slice(&[0x44, 0x29, 0xD8]); // sub eax, r11d
    c.extend_from_slice(&[0x41, 0x8D, 0x3C, 0x02]); // lea edi, [r10+rax] (slot)
    c.extend_from_slice(&[0x41, 0x3B, 0x7E, 0x10]); // cmp edi, [r14+0x10] (capacity)
    let jae_pos = c.len();
    c.extend_from_slice(&[0x73, 0]); // jae next (patched below)
    c.extend_from_slice(&[0x89, 0xFB]); // mov ebx, edi
    c.extend_from_slice(&[0x48, 0xC1, 0xE3, 0x04]); // shl rbx, 4
    c.extend_from_slice(&[0x4C, 0x01, 0xC3]); // add rbx, r8 (&entries[slot])
    c.extend_from_slice(&[0x4C, 0x39, 0x3B]); // cmp [rbx], r15 (chr_ins == null?)
    let je_pos = c.len();
    c.extend_from_slice(&[0x74, 0]); // je found (patched below)

    let next_pos = c.len();
    c.extend_from_slice(&[0xFF, 0xC1]); // inc ecx
    c.extend_from_slice(&[0x44, 0x39, 0xD9]); // cmp ecx, r11d
    let jl_pos = c.len();
    c.extend_from_slice(&[0x7C, 0]); // jl loop_top (patched below)
    let jmp_nf_pos = c.len();
    c.extend_from_slice(&[0xEB, 0]); // jmp not_found (patched below)

    let found_pos = c.len();
    c.extend_from_slice(&[0xFF, 0x25, 0, 0, 0, 0]); // jmp qword [rip+0]
    c.extend_from_slice(&found_addr.to_le_bytes());

    // not_found = end of body: install_jmp_hook appends the jump back to
    // the vanilla "band full" exit right here.
    let not_found_pos = c.len();

    let rel8 = |from_next: usize, to: usize| (to as i64 - from_next as i64) as i8 as u8;
    c[jnz_others_pos + 1] = rel8(jnz_others_pos + 2, others_pos);
    c[jmp_cursor_pos + 1] = rel8(jmp_cursor_pos + 2, cursor_pos);
    c[jae_pos + 1] = rel8(jae_pos + 2, next_pos);
    c[je_pos + 1] = rel8(je_pos + 2, found_pos);
    c[jl_pos + 1] = rel8(jl_pos + 2, loop_top);
    c[jmp_nf_pos + 1] = rel8(jmp_nf_pos + 2, not_found_pos);

    c
}

/// `lea eax,[rax+1]; add rsp,0x28; ret` - NOP-padded to
/// [NEXT_CURSOR_PATCH_LEN]. Replaces vanilla's inlined `(x + 1) % 10` (x =
/// `slot - 20`, already known `>= 0` here) and its identical epilogue:
/// returns `slot - 19` unwrapped, which [build_band_stub] maps back into
/// the caller's band.
fn build_next_cursor_patch() -> Vec<u8> {
    let mut c: Vec<u8> = Vec::with_capacity(NEXT_CURSOR_PATCH_LEN);
    c.extend_from_slice(&[0x8D, 0x40, 0x01]); // lea eax, [rax+1]
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    c.push(0xC3); // ret
    c.resize(NEXT_CURSOR_PATCH_LEN, 0x90);
    c
}

fn install_next_cursor() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(NEXT_CURSOR_AOB, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("Band: next-cursor pattern not found. Game may have been updated - re-check NEXT_CURSOR_AOB.");
        return false;
    };
    let site = unsafe { anchor.add(NEXT_CURSOR_PATCH_OFFSET) };
    if !unsafe { codepatch::overwrite_bytes(site, &build_next_cursor_patch()) } {
        logger::error("Band: next-cursor patch failed.");
        return false;
    }
    logger::log(&format!("Band: next-cursor wrap removed at {site:p}."));
    true
}

fn install_band_alloc() -> bool {
    let Some(site) = memscan::wait_for_pattern_in_module(BAND_ALLOC_AOB, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("Band: slot-search pattern not found. Game may have been updated - re-check BAND_ALLOC_AOB.");
        return false;
    };

    let not_found = unsafe { std::slice::from_raw_parts(site.add(BAND_ALLOC_REPLACED_LEN), 3) };
    let found_opcode = unsafe { *site.add(BAND_ALLOC_FOUND_OFFSET) };
    if not_found != BAND_ALLOC_NOT_FOUND_BYTES || found_opcode != CALL_REL32_OPCODE {
        logger::error("Band: slot-search function layout differs from 2.7.1.0 - not patching.");
        return false;
    }

    let found_addr = site as u64 + BAND_ALLOC_FOUND_OFFSET as u64;
    let stub = build_band_stub(band_len(), found_addr);
    let Some(stub_addr) = codepatch::install_jmp_hook(site, BAND_ALLOC_REPLACED_LEN, &stub) else {
        logger::error("Band: slot-search hook install failed.");
        return false;
    };
    logger::log(&format!(
        "Band: slot search widened to {} slots (player 0) at {site:p}, stub at {stub_addr:p}.",
        band_len()
    ));
    true
}

fn install_capacity(capacity: u32) -> bool {
    let Some(site) = memscan::wait_for_pattern_in_module(CAPACITY_INIT_AOB, SCAN_RETRY_INTERVAL, SCAN_TIMEOUT) else {
        logger::error("Band: ChrSet init pattern not found. Game may have been updated - re-check CAPACITY_INIT_AOB.");
        return false;
    };
    let bytes = capacity * CHR_SET_ENTRY_SIZE;
    for (offset, value) in [
        (CAPACITY_ALLOC_SIZE_OFFSET, bytes),
        (CAPACITY_COUNT_OFFSET, capacity),
        (CAPACITY_MEMSET_SIZE_OFFSET, bytes),
    ] {
        if !unsafe { codepatch::overwrite_bytes(site.add(offset), &value.to_le_bytes()) } {
            logger::error("Band: ChrSet capacity patch failed.");
            return false;
        }
    }
    logger::log(&format!(
        "Band: summon ChrSet capacity {VANILLA_CAPACITY} -> {capacity} patched at {site:p} (takes effect when the world is created)."
    ));
    true
}

/// Installs every patch once at startup, for `max_spirits` (clamped to
/// [DEFAULT_MAX_SPIRITS]..=[MAX_MAX_SPIRITS]). At the default 60 the
/// game's own 80-slot ChrSet is left exactly as vanilla builds it - the
/// capacity patch only runs when the user raises `MaxSpirits` past 60, at
/// their own risk (user decision, 2026-09-25). The capacity patch goes
/// first (it must beat the world's creation), then the cursor patch
/// (harmless on its own: vanilla's search still does `cursor % 10`), then
/// the widened search. If the capacity patch fails, the band falls back
/// to what 80 slots hold.
pub fn install(max_spirits: u32) -> bool {
    let mut band_len = max_spirits.clamp(DEFAULT_MAX_SPIRITS, MAX_MAX_SPIRITS);
    if band_len > DEFAULT_MAX_SPIRITS && !install_capacity(BAND_START + band_len) {
        band_len = DEFAULT_MAX_SPIRITS;
    }
    BAND_LEN.store(band_len, Ordering::Relaxed);
    install_next_cursor() && install_band_alloc()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints the stub bytes for manual disassembly (capstone) - run with
    /// `cargo test -p spiritmultiplier -- --nocapture`.
    #[test]
    fn dump_stubs() {
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        println!("BAND_STUB={}", hex(&build_band_stub(DEFAULT_MAX_SPIRITS, 0x1404933AF + BAND_ALLOC_FOUND_OFFSET as u64)));
        println!("CURSOR_PATCH={}", hex(&build_next_cursor_patch()));
        assert_eq!(build_next_cursor_patch().len(), NEXT_CURSOR_PATCH_LEN);
    }

    #[test]
    fn layout() {
        // Vanilla capacity: partners get exactly vanilla's bands.
        for i in 1..MAX_PLAYERS {
            assert_eq!(band_layout(i, 60, 80), (10 * (i + 2), 10));
        }
        // Seamless capacity: no band overlaps another or runs past capacity.
        let bands: Vec<_> = (0..MAX_PLAYERS).map(|i| band_layout(i, 60, 1000)).collect();
        for w in bands.windows(2) {
            assert!(w[0].0 + w[0].1 <= w[1].0);
        }
        let last = bands.last().unwrap();
        assert!(last.0 + last.1 <= 1000);
        println!("seamless bands: {bands:?}");
    }
}
