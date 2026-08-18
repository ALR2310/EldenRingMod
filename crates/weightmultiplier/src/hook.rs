//! Scales the player's equip load (both the displayed number AND the actual
//! gameplay roll/run behavior) by a configurable factor, ported from
//! WeightEngine.cpp/CodePatch.cpp - same single hook, patched into the exact
//! instruction (`movaps xmm0,xmm6`, confirmed against a working third-party
//! "NoWeight" DLL - see the original README's "Lịch sử tìm offset" for the
//! full story of how two earlier hook points were tried and ruled out) right
//! after the game finishes summing every equipped item's weight, so the
//! multiply runs exactly once per recalculation instead of compounding
//! across the summing loop's 5 iterations.
//!
//! Unlike `autoregen`/`sometweaks`/`runemultiplier`, the patched instruction
//! here is only 7 bytes long - not enough room for the `mov reg, imm64; jmp
//! reg` absolute redirect those use (needs ~12 bytes). This keeps the
//! original technique instead: a 5-byte relative `E9 rel32` JMP, via
//! [`common::codepatch`] (ported from the C++ `CodePatch.cpp`/`.h`), whose
//! stub has to be allocated within reach of a 32-bit signed displacement.
//!
//! Deliberately has no hotkey-driven config reload, unlike the other mods in
//! this workspace - kept from the original design on purpose (one less
//! hotkey to collide with another mod's own binding). Change
//! `WeightReductionPercent` and restart the game to apply a new value.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use common::codepatch;
use common::config;
use common::logger;
use common::memscan;

// Cross-checked against a third-party "NoWeight" DLL confirmed to work
// correctly in-game (WeightValue=0 actually changes the roll weight class,
// not just the displayed number) - its own log printed the exact address it
// patches, which matched this same pattern's match position exactly:
//
//   addss xmm6, dword ptr [rax + 0xc]   ; add this slot's item weight
//   inc ebx                             ; <-- the third-party DLL hooks here (sets a fixed value)
//   cmp ebx, 5
//   jl <loop start>
//   lea r11, [rsp+0x70]
//   movaps xmm0, xmm6                   ; <-- this hook's target: copy the finished total out
//   mov rbx, [r11+0x10]
//
// Hooking the third-party DLL's spot instead would compound a percentage
// multiply across all 5 loop iterations (items summed earlier end up scaled
// down far more than later ones) - fine for "set to a fixed value", wrong
// for a clean percentage. `movaps xmm0,xmm6` runs exactly once, after the
// loop has already finished, so scaling xmm6 right there multiplies the true
// final total by a single clean factor.
const ANCHOR_PATTERN: &str = "FF C3 83 FB ?? 7C ?? 4C 8D 5C 24 ??";
const ANCHOR_TO_TARGET: usize = 12; // skip inc+cmp+jl (7 bytes) + lea r11,[rsp+0x70] (5 bytes)
const TARGET_INSTRUCTION_LEN: usize = 7; // "movaps xmm0,xmm6" (3) + "mov rbx,[r11+0x10]" (4)

// The multiplier applied to the finished equip-load total, stored as raw f32
// bits (no AtomicF32 in std) - read by the injected stub via an absolute
// address baked into it at hook-install time. No hot-reload in this mod, so
// this is written once by [init_weight_factor] before [install] runs and
// never again.
static WEIGHT_FACTOR: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0f32 bit pattern

fn init_weight_factor() {
    let mut percent = config::get_double("WeightReductionPercent", 0.0);
    if percent > 100.0 {
        percent = 100.0;
    }
    let factor = (1.0 - percent / 100.0) as f32;
    WEIGHT_FACTOR.store(factor.to_bits(), Ordering::Relaxed);
    logger::log(&format!("WeightReductionPercent={percent:.2} -> weightFactor={factor:.3}"));
}

fn build_stub(weight_factor_addr: u64) -> Vec<u8> {
    let mut body = Vec::with_capacity(21);

    // mov rcx, &WEIGHT_FACTOR
    body.extend_from_slice(&[0x48, 0xB9]);
    body.extend_from_slice(&weight_factor_addr.to_le_bytes());

    // mulss xmm6, dword ptr [rcx] (scale the finished total in place, once, before it's copied out)
    body.extend_from_slice(&[0xF3, 0x0F, 0x59, 0x31]);

    // movaps xmm0, xmm6 (original instruction, re-executed with the now-scaled xmm6)
    body.extend_from_slice(&[0x0F, 0x28, 0xC6]);

    // mov rbx, qword ptr [r11+0x10] (original next instruction, re-executed)
    body.extend_from_slice(&[0x49, 0x8B, 0x5B, 0x10]);

    body
}

fn install() -> bool {
    let Some(anchor) = memscan::find_pattern_in_module(ANCHOR_PATTERN) else {
        logger::log("ERROR: weight-summing-loop anchor pattern not found. Game may have been updated - re-check ANCHOR_PATTERN.");
        return false;
    };
    let target = unsafe { anchor.add(ANCHOR_TO_TARGET) };
    logger::log(&format!("Anchor found at {anchor:p}, patch target at {target:p}."));

    let factor_addr = &WEIGHT_FACTOR as *const AtomicU32 as u64;
    let stub_body = build_stub(factor_addr);

    let Some(stub) = codepatch::install_jmp_hook(target, TARGET_INSTRUCTION_LEN, &stub_body) else {
        logger::log("ERROR: failed to install hook (couldn't allocate stub or patch target).");
        return false;
    };

    logger::log(&format!("Hook installed. stub={stub:p}"));
    true
}

/// Waits `InitialDelaySeconds`, then installs the hook. Meant to run on its
/// own worker thread spawned from `DllMain`; returns once done (there's no
/// per-tick or hotkey-watching loop for this mod).
pub fn run() {
    let initial_delay_seconds = config::get_int("InitialDelaySeconds", 5).max(0) as u64;
    std::thread::sleep(Duration::from_secs(initial_delay_seconds));

    init_weight_factor();

    if !install() {
        logger::log("WeightMultiplier disabled for this session (hook install failed).");
        return;
    }

    logger::log("Hook active.");
}
