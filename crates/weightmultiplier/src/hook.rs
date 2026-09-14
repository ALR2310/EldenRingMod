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
//! `Mode=1` (`FixedValue`) reuses this exact same hook point instead of
//! patching mid-loop the way a third-party "NoWeight" DLL does (see the
//! original README's "Lịch sử tìm offset") - since this hook already runs
//! exactly once, after the loop has finished summing, overwriting `xmm6`
//! here works just as well as multiplying it, without needing a second
//! patch site.
//!
//! `ReloadKey` polling (added later - the mod originally had none, on
//! purpose, to avoid a hotkey collision) is a plain `Sleep`-based loop via
//! [`common::input::is_key_pressed`], not a task registered on the game's
//! own scheduler like `autoregen`/`sometweaks`/`runemultiplier` use - this
//! crate deliberately has no `fromsoftware-rs` dependency (see `Cargo.toml`),
//! so it has no access to that scheduler. Changing `Multiplier`/`FixedValue`
//! on reload is a plain value write (the stub already reads it from a fixed
//! address every time it runs); changing `Mode` additionally re-patches the
//! stub's `mulss`/`movss` instruction in place via
//! [`common::codepatch::overwrite_bytes`], since which of the two runs is
//! baked into the executing code itself, not read from memory - same
//! inherent risk as the original `install()` (rewriting code the CPU could
//! be mid-execution of), just repeated on demand instead of only once.

use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use common::codepatch;
use common::config;
use common::input::{is_key_pressed, parse_virtual_key};
use common::logger;
use common::memscan;

const VK_F5: i32 = 0x74;

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

const MODE_MULTIPLIER: i32 = 0;
const MODE_FIXED_VALUE: i32 = 1;

// The mode currently baked into the installed stub's mulss/movss instruction
// - compared against on reload to decide whether that instruction needs
// re-patching (see [reload_weight_value]).
static WEIGHT_MODE: AtomicI32 = AtomicI32::new(MODE_MULTIPLIER);

// The value applied to the finished equip-load total - a multiplier factor
// when WEIGHT_MODE is MODE_MULTIPLIER, or the fixed total itself when
// MODE_FIXED_VALUE. Stored as raw f32 bits (no AtomicF32 in std) - read by
// the injected stub via an absolute address baked into it at hook-install
// time, every time it runs, so updating this alone (no re-patch needed) is
// enough to hot-reload a new Multiplier/FixedValue number.
static WEIGHT_VALUE: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0f32 bit pattern

// Address of the mulss/movss instruction inside the installed stub (see
// [mode_instruction] + STUB_MODE_INSTR_OFFSET) - 0 until [install] succeeds.
// Only [reload_weight_value] writes to this address again, if Mode changes.
static STUB_MODE_INSTR_ADDR: AtomicU64 = AtomicU64::new(0);

// Offset of the mulss/movss instruction within the stub built by
// [build_stub]: right after the 10-byte "mov rcx, imm64" that loads
// &WEIGHT_VALUE.
const STUB_MODE_INSTR_OFFSET: usize = 10;

/// The 4-byte instruction [build_stub] injects for `mode`: `mulss xmm6,[rcx]`
/// (MODE_MULTIPLIER) or `movss xmm6,[rcx]` (MODE_FIXED_VALUE, overwrites the
/// finished total outright instead of scaling it).
fn mode_instruction(mode: i32) -> [u8; 4] {
    if mode == MODE_FIXED_VALUE {
        [0xF3, 0x0F, 0x10, 0x31]
    } else {
        [0xF3, 0x0F, 0x59, 0x31]
    }
}

fn read_weight_value(mode: i32) -> f32 {
    (if mode == MODE_FIXED_VALUE {
        config::get_double("FixedValue", 0.0)
    } else {
        config::get_double("Multiplier", 1.0)
    }) as f32
}

fn init_weight_value() {
    let mode = config::get_int("Mode", MODE_MULTIPLIER);
    let value = read_weight_value(mode);
    WEIGHT_MODE.store(mode, Ordering::Relaxed);
    WEIGHT_VALUE.store(value.to_bits(), Ordering::Relaxed);
    if config::get_bool("LogFile", false) {
        logger::log(&format!("Mode={mode}, value={value:.3}"));
    }
}

/// Re-reads `Mode`/`Multiplier`/`FixedValue` from the just-`config::load`ed
/// ini and applies them to the already-installed hook: always updates
/// `WEIGHT_VALUE` (the stub re-reads it every time it runs, no patch
/// needed); if `Mode` itself changed, also re-patches the stub's
/// mulss/movss instruction in place via `codepatch::overwrite_bytes` - see
/// the module doc comment for why that's riskier than the value-only path.
fn reload_weight_value() {
    let new_mode = config::get_int("Mode", MODE_MULTIPLIER);
    let value = read_weight_value(new_mode);
    WEIGHT_VALUE.store(value.to_bits(), Ordering::Relaxed);

    let old_mode = WEIGHT_MODE.swap(new_mode, Ordering::Relaxed);
    if new_mode != old_mode {
        let addr = STUB_MODE_INSTR_ADDR.load(Ordering::Relaxed);
        if addr != 0 {
            let patched = unsafe { codepatch::overwrite_bytes(addr as *mut u8, &mode_instruction(new_mode)) };
            if !patched {
                logger::log("ERROR: failed to re-patch stub for new Mode - keeping the previous Mode's behavior.");
                WEIGHT_MODE.store(old_mode, Ordering::Relaxed);
            }
        }
    }

    if config::get_bool("LogFile", false) {
        logger::log(&format!("Mode={new_mode}, value={value:.3}"));
    }
}

fn build_stub(mode: i32, weight_value_addr: u64) -> Vec<u8> {
    let mut body = Vec::with_capacity(21);

    // mov rcx, &WEIGHT_VALUE
    body.extend_from_slice(&[0x48, 0xB9]);
    body.extend_from_slice(&weight_value_addr.to_le_bytes());
    debug_assert_eq!(body.len(), STUB_MODE_INSTR_OFFSET);

    // mulss/movss xmm6, dword ptr [rcx] (scale or overwrite the finished
    // total in place, once, before it's copied out)
    body.extend_from_slice(&mode_instruction(mode));

    // movaps xmm0, xmm6 (original instruction, re-executed with the now-scaled/overwritten xmm6)
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
    if config::get_bool("LogFile", false) {
        logger::log(&format!("Anchor found at {anchor:p}, patch target at {target:p}."));
    }

    let value_addr = &WEIGHT_VALUE as *const AtomicU32 as u64;
    let stub_body = build_stub(WEIGHT_MODE.load(Ordering::Relaxed), value_addr);

    let Some(stub) = codepatch::install_jmp_hook(target, TARGET_INSTRUCTION_LEN, &stub_body) else {
        logger::log("ERROR: failed to install hook (couldn't allocate stub or patch target).");
        return false;
    };
    STUB_MODE_INSTR_ADDR.store(unsafe { stub.add(STUB_MODE_INSTR_OFFSET) } as u64, Ordering::Relaxed);

    if config::get_bool("LogFile", false) {
        logger::log(&format!("Hook installed. stub={stub:p}"));
    }
    true
}

/// Waits `LoadDelay`, installs the hook, then polls `ReloadKey` forever.
/// Meant to run on its own worker thread spawned from `DllMain`; never
/// returns once the hook is active (matches every other mod in this
/// workspace having some kind of never-ending loop on its own thread/task).
pub fn run(ini_path: String) {
    let load_delay_ms = config::get_int("LoadDelay", 5000).max(0) as u64;
    std::thread::sleep(Duration::from_millis(load_delay_ms));

    init_weight_value();

    if !install() {
        logger::log("WeightMultiplier disabled for this session (hook install failed).");
        return;
    }

    logger::log("Hook active.");

    loop {
        std::thread::sleep(Duration::from_millis(100));

        // This loop runs on its own OS thread, not one the game itself calls
        // into (unlike autoregen/sometweaks's per-frame task callbacks), so a
        // panic here can't corrupt the game's own call stack either way -
        // catch_unwind is just to keep this one iteration's panic from
        // killing the whole reload thread (and with it, ReloadKey) for the
        // rest of the session.
        if let Err(panic) = std::panic::catch_unwind(|| {
            let reload_key = parse_virtual_key(&config::get_string("ReloadKey", "F5"), VK_F5);
            if is_key_pressed(reload_key) {
                config::load(&ini_path);
                reload_weight_value();
                logger::log("Config reloaded (hotkey pressed).");
            }
        }) {
            logger::error(&format!("ReloadKey poll panicked, skipped: {panic:?}"));
        }
    }
}
