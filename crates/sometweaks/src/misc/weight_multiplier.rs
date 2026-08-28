//! Scales the player's equip load (both the displayed number AND the actual
//! gameplay roll/run behavior) by a configurable factor, ported from
//! [`WeightMultiplier`](../../../weightmultiplier)'s `hook.rs` - same single
//! hook, patched right after the game finishes summing every equipped item's
//! weight (`movaps xmm0,xmm6`), so the multiply runs exactly once per
//! recalculation instead of compounding across the summing loop's 5
//! iterations. See that crate's README for the full "3 tries to find the
//! right offset" reverse-engineering writeup.
//!
//! Unlike [`super::super::rune::multiplier`] (and `regen/attack_hook.rs`),
//! the patched instruction here is only 7 bytes long - not enough room for
//! the `mov reg, imm64; jmp reg` absolute redirect those use (needs ~12
//! bytes). Uses [`common::codepatch`] instead: a 5-byte relative `E9 rel32`
//! JMP whose stub has to be allocated within reach of a 32-bit signed
//! displacement.
//!
//! Kept `WeightMultiplier`'s ini key as a direct multiplier (`SomeTweaks.ini`
//! has `WeightMultiplier=0.5` in `[Misc]`, same "1 = unchanged" convention
//! as `RuneMultiplier`) rather than the standalone crate's
//! `WeightReductionPercent` (0-100, `factor = 1 - percent/100`) - no
//! conversion needed, just clamp to non-negative.
//!
//! Hot-reload works the same way [`super::super::rune::multiplier`]'s does:
//! the stub reads [WEIGHT_FACTOR] through a baked-in pointer on every
//! weight recalculation, so updating that atomic after install takes
//! effect immediately with no re-patching - a tick on `FrameBegin` just
//! keeps it in sync with `WeightMultiplier` (2026-08-25, was previously
//! "no hot-reload" here for lack of a tick loop to piggyback on; now uses
//! [crate::task::wait_for_cs_task] the same way every other tick-based
//! feature in this crate does).

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use eldenring::cs::CSTaskGroupIndex;

use common::codepatch;
use common::config;
use common::logger;
use common::memscan;

// Cross-checked against a third-party "NoWeight" DLL confirmed to work
// correctly in-game (WeightValue=0 actually changes the roll weight class,
// not just the displayed number) - its own log printed the exact address it
// patches, which matched this same pattern's match position exactly. See
// `WeightMultiplier/README.md`'s "Lich su tim offset" for the full story of
// why this exact instruction (not 5 bytes earlier, not the `movss` that only
// affects the UI number) is the correct hook point for a clean percentage.
const ANCHOR_PATTERN: &str = "FF C3 83 FB ?? 7C ?? 4C 8D 5C 24 ??";
const ANCHOR_TO_TARGET: usize = 12; // skip inc+cmp+jl (7 bytes) + lea r11,[rsp+0x70] (5 bytes)
const TARGET_INSTRUCTION_LEN: usize = 7; // "movaps xmm0,xmm6" (3) + "mov rbx,[r11+0x10]" (4)

// The multiplier applied to the finished equip-load total, stored as raw f32
// bits (no AtomicF32 in std) - read by the injected stub via an absolute
// address baked into it at hook-install time. Re-synced every tick (see
// `run`) so `General.ReloadKey` picks up a new value without re-patching.
static WEIGHT_FACTOR: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0f32 bit pattern

/// Re-reads `WeightMultiplier` from the shared config and stores it into
/// [WEIGHT_FACTOR], logging only when it actually changed - called once at
/// startup and then every tick, same pattern as
/// `rune::multiplier::apply_multiplier`.
fn apply_weight_factor() {
    let multiplier = config::get_double("WeightMultiplier", 1.0).max(0.0);
    let factor = (multiplier as f32).to_bits();
    let previous = WEIGHT_FACTOR.swap(factor, Ordering::Relaxed);
    if previous != factor {
        logger::log(&format!("WeightMultiplier={multiplier:.3}."));
    }
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

// How long [install] retries the AOB scan before giving up. `WeightMultiplier`'s
// standalone crate instead used a single fixed `InitialDelaySeconds=5` wait
// with no retry - replaced here with polling (via `common::memscan::
// wait_for_pattern_in_module`) so a slow/loaded machine (game still
// unpacking/relocating past 5s) doesn't lose this feature for the whole
// session, and a fast one doesn't wait longer than it needs to. Not exposed
// as ini keys - SomeTweaks has no other module that needs this kind of
// startup tuning, so more knobs aren't worth it.
const SCAN_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const ANCHOR_SCAN_TIMEOUT: Duration = Duration::from_secs(60);

fn install() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(ANCHOR_PATTERN, SCAN_RETRY_INTERVAL, ANCHOR_SCAN_TIMEOUT) else {
        logger::error("WeightMultiplier: weight-summing-loop anchor pattern not found within the timeout. Game may have been updated - re-check ANCHOR_PATTERN.");
        return false;
    };
    let target = unsafe { anchor.add(ANCHOR_TO_TARGET) };
    logger::log(&format!("WeightMultiplier: anchor found at {anchor:p}, patch target at {target:p}."));

    let factor_addr = &WEIGHT_FACTOR as *const AtomicU32 as u64;
    let stub_body = build_stub(factor_addr);

    let Some(stub) = codepatch::install_jmp_hook(target, TARGET_INSTRUCTION_LEN, &stub_body) else {
        logger::error("WeightMultiplier: failed to install hook (couldn't allocate stub or patch target).");
        return false;
    };

    logger::log(&format!("WeightMultiplier: hook installed. stub={stub:p}"));
    true
}

/// Installs the weight-scaling hook once (retrying the AOB scan for up to
/// [ANCHOR_SCAN_TIMEOUT]), then re-applies `WeightMultiplier` every tick on
/// the game's own `FrameBegin` task group for the rest of the DLL's
/// lifetime - see [WEIGHT_FACTOR]'s doc comment for why hot-reload needs no
/// re-patching. Meant to run on its own worker thread spawned from
/// `DllMain`; never returns (except early, if the hook fails to install).
pub fn run() {
    apply_weight_factor();

    if !install() {
        logger::warn("WeightMultiplier: disabled for this session (hook install failed).");
        return;
    }

    logger::log("WeightMultiplier: hook active.");

    let cs_task = crate::task::wait_for_cs_task();
    let _handle = crate::task::run_recurring_safe(
        cs_task,
        "WeightMultiplier",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| {
            apply_weight_factor();
        },
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
