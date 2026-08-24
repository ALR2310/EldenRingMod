//! Two independent single-hook "multiply a configurable value" features,
//! kept together in one file since neither shares any code with the other
//! (different anchor, different patch technique, different ini key) and each
//! is small enough on its own that a dedicated file/module per feature was
//! more ceremony than benefit. [rune_multiplier]::run and
//! [weight_multiplier]::run are each spawned on their own worker thread from
//! `lib.rs`, independent of each other and of `regen`/`rune_reward`.

/// Multiplies rune gains from every source by a configurable factor, ported
/// from [`RuneMultiplier`](../../runemultiplier)'s `hook.rs` - same single
/// hook on `AddSoul_Call` (the game's lowest-level "add this many runes"
/// function, used by both enemy kills and rune items), same absolute
/// `mov reg, imm64; jmp reg` redirect/stub-return jumps (the same technique
/// `regen/attack_hook.rs` already uses in this crate). Kept under the
/// `Rune.Multiplier` key (`[Rune Reward]` section - grouped alongside the
/// rest of SomeTweaks' own rune-related keys, unlike the standalone crate's
/// own `Multiplier`/`[Settings]` naming) instead of renaming again.
///
/// Disassembly at `AddSoul_Call` (see `RuneMultiplier/README.md` for the full
/// reverse-engineering writeup):
///
/// ```text
/// MOV R9D,[RCX+0x6C]      ; R9D = current rune count
/// LEA R8D,[R9+RDX*1]      ; sum = current + amount (RDX = amount to add)
/// CMP R8D,0x3B9AC9FF      ; clamp to the 999,999,999 cap
/// ...
/// MOV [RCX+0x6C],EAX      ; write new rune count - used by every source
/// RET
/// ```
///
/// The multiply has to happen to EDX *before* any of that runs, so the first
/// 3 instructions (12 bytes: `mov r9d,[rcx+0x6c]` + `xor r11d,r11d` +
/// `mov [rsp+0x10],r11d` - none of which read RDX/EDX) are overwritten with a
/// redirect to a generated stub that multiplies EDX, re-runs those 3
/// instructions (copied live from the game, never hardcoded), then jumps back
/// into `AddSoul_Call` right after them.
pub mod rune_multiplier {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::time::Duration;

    use eldenring::cs::{CSTaskGroupIndex, CSTaskImp};
    use fromsoftware_shared::SharedTaskImpExt;

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
    /// `regen::run`, which already reloads the shared config map) picks up a new
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

    /// `CSTaskImp::wait_for_instance` treats `SystemInitError::InvalidRva` as
    /// immediately fatal and never retries it, even with `Duration::MAX` - it
    /// only retries the `Null` case internally. `InvalidRva` fires whenever the
    /// version-specific RVA lookup runs before the game executable has finished
    /// unpacking/relocating (e.g. Arxan), which is a timing race against how
    /// early this DLL's worker thread happens to start. Retrying here with a
    /// short delay rides out that race instead of permanently disabling the
    /// hot-reload loop for the session on a one-off early poll. Same fix as
    /// `regen::wait_for_cs_task`/PassiveRunes' `wait_for_cs_task` (2026-08-24).
    fn wait_for_cs_task() -> &'static CSTaskImp {
        loop {
            match CSTaskImp::wait_for_instance(Duration::MAX) {
                Ok(instance) => return instance,
                Err(err) => {
                    logger::log(&format!("RuneMultiplier: CSTaskImp not ready yet ({err:?}), retrying in 1s..."));
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    /// Installs the hook, then re-applies `RuneMultiplier` every tick on the
    /// game's own `FrameBegin` task group for the rest of the DLL's lifetime -
    /// hot reload doesn't need to re-patch anything, the stub always reads
    /// [FIXED_Q20] through a pointer, so just keeping that value current is
    /// enough. Meant to run on its own worker thread spawned from `DllMain`
    /// (alongside `regen::run`/`rune_reward::run`/`weight_multiplier::run`,
    /// not instead of them); never returns (except early, if the hook fails to install -
    /// the anchor pattern not being found doesn't affect any other module, so
    /// this only disables RuneMultiplier for the session rather than the
    /// whole DLL).
    pub fn run() {
        apply_multiplier();

        let debug_log = config::get_bool("DebugLog", false);
        if !install(debug_log) {
            logger::log("RuneMultiplier disabled for this session (hook install failed).");
            return;
        }

        let cs_task = wait_for_cs_task();
        let _handle = cs_task.run_recurring(
            move |_data: &eldenring::fd4::FD4TaskData| {
                apply_multiplier();
            },
            CSTaskGroupIndex::FrameBegin,
        );

        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
}

/// Scales the player's equip load (both the displayed number AND the actual
/// gameplay roll/run behavior) by a configurable factor, ported from
/// [`WeightMultiplier`](../weightmultiplier)'s `hook.rs` - same single hook,
/// patched right after the game finishes summing every equipped item's
/// weight (`movaps xmm0,xmm6`), so the multiply runs exactly once per
/// recalculation instead of compounding across the summing loop's 5
/// iterations. See that crate's README for the full "3 tries to find the
/// right offset" reverse-engineering writeup.
///
/// Unlike [rune_multiplier] (and `regen/attack_hook.rs`), the patched instruction here
/// is only 7 bytes long - not enough room for the `mov reg, imm64; jmp reg`
/// absolute redirect those use (needs ~12 bytes). Uses [`common::codepatch`]
/// instead: a 5-byte relative `E9 rel32` JMP whose stub has to be allocated
/// within reach of a 32-bit signed displacement.
///
/// Kept `WeightMultiplier`'s ini key as a direct multiplier (`SomeTweaks.ini`
/// has `WeightMultiplier=0.5` in `[Misc]`, same "1 = unchanged" convention
/// as `RuneMultiplier`) rather than the standalone crate's
/// `WeightReductionPercent` (0-100, `factor = 1 - percent/100`) - no
/// conversion needed, just clamp to non-negative.
///
/// Deliberately has no hotkey-driven config reload, same as the standalone
/// crate: change `WeightMultiplier` and restart the game to apply a new
/// value - this hook has no per-tick loop of its own to piggyback a re-read
/// on (unlike [rune_multiplier], which already needs `CSTaskImp` for its own tick), and
/// a live weight-scaling patch on a code path other mods (`RiseArcher`, etc.)
/// might also hook isn't worth complicating for a value that's rarely
/// tweaked mid-session.
pub mod weight_multiplier {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use common::codepatch;
    use common::config;
    use common::logger;
    use common::memscan;

    // Cross-checked against a third-party "NoWeight" DLL confirmed to work
    // correctly in-game (WeightValue=0 actually changes the roll weight class,
    // not just the displayed number) - its own log printed the exact address it
    // patches, which matched this same pattern's match position exactly. See
    // `WeightMultiplier/README.md`'s "Lịch sử tìm offset" for the full story of
    // why this exact instruction (not 5 bytes earlier, not the `movss` that only
    // affects the UI number) is the correct hook point for a clean percentage.
    const ANCHOR_PATTERN: &str = "FF C3 83 FB ?? 7C ?? 4C 8D 5C 24 ??";
    const ANCHOR_TO_TARGET: usize = 12; // skip inc+cmp+jl (7 bytes) + lea r11,[rsp+0x70] (5 bytes)
    const TARGET_INSTRUCTION_LEN: usize = 7; // "movaps xmm0,xmm6" (3) + "mov rbx,[r11+0x10]" (4)

    // The multiplier applied to the finished equip-load total, stored as raw f32
    // bits (no AtomicF32 in std) - read by the injected stub via an absolute
    // address baked into it at hook-install time. No hot-reload for this module,
    // so this is written once by [init_weight_factor] before [install] runs and
    // never again.
    static WEIGHT_FACTOR: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0f32 bit pattern

    fn init_weight_factor() {
        let multiplier = config::get_double("WeightMultiplier", 1.0).max(0.0);
        let factor = multiplier as f32;
        WEIGHT_FACTOR.store(factor.to_bits(), Ordering::Relaxed);
        logger::log(&format!("WeightMultiplier={multiplier:.3}"));
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
            logger::log("WeightMultiplier: ERROR - weight-summing-loop anchor pattern not found. Game may have been updated - re-check ANCHOR_PATTERN.");
            return false;
        };
        let target = unsafe { anchor.add(ANCHOR_TO_TARGET) };
        logger::log(&format!("WeightMultiplier: anchor found at {anchor:p}, patch target at {target:p}."));

        let factor_addr = &WEIGHT_FACTOR as *const AtomicU32 as u64;
        let stub_body = build_stub(factor_addr);

        let Some(stub) = codepatch::install_jmp_hook(target, TARGET_INSTRUCTION_LEN, &stub_body) else {
            logger::log("WeightMultiplier: ERROR - failed to install hook (couldn't allocate stub or patch target).");
            return false;
        };

        logger::log(&format!("WeightMultiplier: hook installed. stub={stub:p}"));
        true
    }

    // Same fixed 5s wait `WeightMultiplier`'s standalone crate exposes as
    // `InitialDelaySeconds` - the AOB scan below has no retry loop (unlike
    // `regen`/`rune_reward`/`super::rune_multiplier`'s `wait_for_cs_task`), so
    // if it runs before the game's own anti-tamper unpacking/relocation finishes,
    // it fails to find the pattern and stays disabled for the rest of the
    // session. Not exposed as its own ini key here - SomeTweaks has no other
    // module that needs a startup delay, so one more knob isn't worth it.
    const INITIAL_DELAY: Duration = Duration::from_secs(5);

    /// Waits [INITIAL_DELAY], then installs the weight-scaling hook once. Meant
    /// to run on its own worker thread spawned from `DllMain`; returns once done
    /// (no per-tick or hotkey-watching loop for this module).
    pub fn run() {
        std::thread::sleep(INITIAL_DELAY);

        init_weight_factor();

        if !install() {
            logger::log("WeightMultiplier disabled for this session (hook install failed).");
            return;
        }

        logger::log("WeightMultiplier: hook active.");
    }
}
