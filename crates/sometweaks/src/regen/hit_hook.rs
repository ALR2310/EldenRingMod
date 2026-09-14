//! Heal-on-hit, ported from AutoRegen's `hit_hook.rs` (2026-09-14) - replaces
//! `attack_hook.rs`'s Hexinton-CE-table hijack of a mid-function call site
//! with a hook on the real, independent hit-resolution function's own entry
//! point instead, found via static analysis of `eldenring.exe` (Ghidra),
//! anchored only on `CSChrDataModule`'s real field offsets from
//! `fromsoftware-rs` - not on any 3rd-party AOB.
//!
//! Why: `attack_hook.rs`'s trampoline hijacked a call site *inside* another
//! function's body and hand-reconstructed the call to the real hit-resolution
//! function, leaving that enclosing function's compiler-emitted unwind
//! metadata (`.pdata`/`.xdata`) mismatched with what actually ran at runtime -
//! confirmed (in AutoRegen, same shared hook design) as the cause of a
//! reproducible crash with `Regen.PerHit` enabled in some areas (crash dumps +
//! a before/after test build both pointed at this trampoline, see AutoRegen's
//! README). This hook instead patches a real, standalone, compiler-emitted
//! function's own prologue and relocates it into a trampoline that resumes
//! the function normally afterward, instead of hijacking another function's
//! body.
//!
//! Call chain (see AutoRegen's `.docs/reverse_engineering/` scripts/output
//! for how this was found - same game version, same call chain, since both
//! mods target the same `eldenring.exe`):
//!   hit-resolution (`attack_hook.rs`'s old OnAttack target) -> `FUN_140448910(
//!   ctx, attacker, hit_info, _, flag)` -> `ApplyHpDelta(target_module,
//!   -hit_info[0x228], ...)` -> `SetHp(module, clamp(new_hp, 0, max_hp))`.
//! `hit_info+0x228` (the damage amount) matches `attack_hook.rs`'s existing
//! `HITINFO_DAMAGE_OFFSET`.
//!
//! Register contract on entry, unchanged from `attack_hook.rs`'s doc comment
//! (this is the same `ctx`/`attacker`/`hit_info`/flag quadruple, just
//! delivered via a real function's own args instead of 4 register-only
//! mov/movzx instructions before a call site):
//!   rcx = ctx, rdx = attacker, r8 = hit_info, r9 = flag (byte, zero-extended)

use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use common::config;
use common::logger;
use common::memscan;
use super::{self as regen, HealField};

static PENDING_LOGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn queue_log(message: String) {
    if let Ok(mut queue) = PENDING_LOGS.lock() {
        queue.push(message);
    }
}

/// Writes out any DebugLog lines queued by the hook instead of writing
/// directly - synchronous file I/O (`Mutex<File>` + `writeln!`, see
/// `common::logger`) on the hot hit-resolution path risks disrupting a
/// backstab's tightly-timed critical-hit sequence (suspected + fixed in
/// AutoRegen 2026-09-03) - moved off the hot path entirely here from the
/// start rather than repeating that bug. Called once per frame from
/// `regen::run`'s tick, well outside the game's own hit-resolution call
/// stack.
pub fn flush_pending_logs() {
    let messages: Vec<String> = {
        let Ok(mut queue) = PENDING_LOGS.lock() else {
            return;
        };
        std::mem::take(&mut *queue)
    };
    for message in messages {
        logger::debug(&message);
    }
}

unsafe extern "system" {
    fn VirtualProtect(
        lp_address: *mut c_void,
        dw_size: usize,
        fl_new_protect: u32,
        lpfl_old_protect: *mut u32,
    ) -> i32;
}

const PAGE_EXECUTE_READWRITE: u32 = 0x40;

// `FUN_140448910`'s own prologue, found via Ghidra static analysis of the
// live game exe (see AutoRegen's `.docs/reverse_engineering/DumpFnPrologue.java`
// output) - NOT from any 3rd-party CE table. Anchored on the full 40-byte
// window (through the stack-cookie load and the R12 shadow-space spill)
// rather than just the first 15 bytes, to cut collision risk with other
// functions sharing the same generic "push non-volatile regs, sub rsp, N"
// MSVC prologue shape - only verified against the one game version AutoRegen
// tested against so far, not yet cross-checked against a 2nd version.
const HIT_APPLY_PATTERN: &str = "4c 8b dc 55 53 56 57 41 56 41 57 49 8d 6b 88 48 81 ec 48 01 00 00 48 8b 05 ?? ?? ?? ?? 48 33 c4 48 89 45 20 4d 89 63 20";

// Only the first 15 bytes (9 instructions: the MOV R11,RSP / 6 PUSHes / LEA)
// are patched+relocated - the function's actual prologue, ending exactly on
// an instruction boundary just before `SUB RSP,0x148`. None of these 9
// instructions touch RCX/RDX/R8/R9 (the incoming ctx/attacker/hit_info/flag
// args) or RIP-relative data, so relocating them verbatim into the
// trampoline is safe with no operand fixup needed.
const RELOCATE_LEN: usize = 0xF;

static G_RETURN_ADDR: AtomicUsize = AtomicUsize::new(0);

// The 15 relocated prologue bytes, followed by a "mov rax, imm64; jmp rax"
// (12 bytes) back into the real function right after its own prologue -
// without this trailing jump, execution would fall straight off the end of
// the relocated bytes into whatever static data follows in memory (a real
// bug hit once while developing this in AutoRegen - ACCESS_VIOLATION on the
// very first hit). Lives in .data, so `install` marks the whole buffer's
// page RWX rather than relying on the module's own R-X section for it.
const RELOCATED_BUF_LEN: usize = RELOCATE_LEN + 12;

#[unsafe(no_mangle)]
static mut G_RELOCATED_BYTES: [u8; RELOCATED_BUF_LEN] = [0u8; RELOCATED_BUF_LEN];

unsafe extern "C" {
    fn hit_trampoline();
}

std::arch::global_asm!(
    r#"
.global hit_trampoline
hit_trampoline:
    # Entered via a JMP placed at the function's own entry (not a mid-call
    # hijack) - rsp here is exactly what a normal `call` to this function
    # left it as (return address already pushed by the real caller).
    push    r12
    push    r13
    mov     r12, rsp

    # Save the 4 incoming args (rcx=ctx, rdx=attacker, r8=hit_info,
    # r9=flag) - our own call below clobbers these per the Win64 ABI, but
    # the relocated prologue/rest of the real function still needs them
    # untouched.
    push    rcx
    push    rdx
    push    r8
    push    r9
    mov     r13, rsp

    and     rsp, -16
    sub     rsp, 0x20
    call    on_hit_pre

    mov     rsp, r13
    pop     r9
    pop     r8
    pop     rdx
    pop     rcx
    pop     r13
    pop     r12

    # rsp is now back to exactly what it was at trampoline entry - replay
    # the function's own relocated prologue, then resume it normally.
    lea     r11, [rip + G_RELOCATED_BYTES]
    jmp     r11
"#
);

fn looks_like_pointer(p: *const c_void) -> bool {
    !p.is_null() && (p as usize) >= 0x10000
}

// Same vtable-based "sourceType" read `attack_hook.rs` used - kept identical
// since it's confirmed correct independently (matches the real hit-resolution
// function's own `(**(code**)(*attacker+0x10))(attacker) != 1` check).
fn get_source_object_type(source_object: *const c_void) -> i32 {
    if !looks_like_pointer(source_object) {
        return -1;
    }
    unsafe {
        let vtable = *(source_object as *const *const c_void);
        if !looks_like_pointer(vtable) {
            return -1;
        }
        let type_fn_ptr = *(vtable.byte_add(0x10) as *const *const c_void);
        if !looks_like_pointer(type_fn_ptr) {
            return -1;
        }
        let type_fn: extern "C" fn(*const c_void) -> i32 = std::mem::transmute(type_fn_ptr);
        type_fn(source_object)
    }
}

const HITINFO_DAMAGE_OFFSET: isize = 0x228;
const HITINFO_SOURCE_OBJECT_OFFSET: isize = 0x1D8;
const CTX_TARGET_OFFSET: isize = 0x8;

fn read_damage(hit_info: *const c_void) -> Option<i32> {
    if !looks_like_pointer(hit_info) {
        return None;
    }
    unsafe { Some(*(hit_info.byte_offset(HITINFO_DAMAGE_OFFSET) as *const i32)) }
}

fn read_source_type(hit_info: *const c_void) -> i32 {
    if !looks_like_pointer(hit_info) {
        return -1;
    }
    unsafe {
        let source_object = *(hit_info.byte_offset(HITINFO_SOURCE_OBJECT_OFFSET) as *const *const c_void);
        get_source_object_type(source_object)
    }
}

fn read_target(ctx: *const c_void) -> Option<*const c_void> {
    if !looks_like_pointer(ctx) {
        return None;
    }
    unsafe { Some(*(ctx.byte_offset(CTX_TARGET_OFFSET) as *const *const c_void)) }
}

/// Whether this hit's source type matches `damage_type` (the
/// `Regen.PerHit.DamageType` ini value): `0` = melee only (`sourceType==1`),
/// `1` = ranged/spells only (`sourceType==3`), `2` = either (no filtering).
fn matches_damage_type(hit_info: *const c_void, damage_type: i32) -> bool {
    if damage_type == 2 {
        return true;
    }
    let source_type = read_source_type(hit_info);
    if damage_type == 1 {
        source_type == 3
    } else {
        source_type == 1
    }
}

static ENABLED: AtomicI32 = AtomicI32::new(0);
static TRIGGER: AtomicI32 = AtomicI32::new(0);
static DAMAGE_TYPE: AtomicI32 = AtomicI32::new(0);
static EXCLUDE_AOW: AtomicI32 = AtomicI32::new(0);
static HP_BITS: AtomicU64 = AtomicU64::new(0);
static FP_BITS: AtomicU64 = AtomicU64::new(0);
static STAMINA_BITS: AtomicU64 = AtomicU64::new(0);

fn store_f64(cell: &AtomicU64, value: f64) {
    cell.store(value.to_bits(), Ordering::Relaxed);
}

fn load_f64(cell: &AtomicU64) -> f64 {
    f64::from_bits(cell.load(Ordering::Relaxed))
}

/// Same shape as `attack_hook::OnHitParams` - see its doc comment for what
/// each field means.
#[derive(Clone, Copy)]
pub struct OnHitParams {
    pub enabled: bool,
    pub trigger: i32,
    pub damage_type: i32,
    pub exclude_aow: bool,
    pub hp: f64,
    pub fp: f64,
    pub stamina: f64,
}

impl OnHitParams {
    pub fn wants_heal(&self) -> bool {
        self.enabled && (self.hp > 0.0 || self.fp > 0.0 || self.stamina > 0.0)
    }
}

pub fn update_params(params: OnHitParams) {
    ENABLED.store(params.enabled as i32, Ordering::Relaxed);
    TRIGGER.store(params.trigger, Ordering::Relaxed);
    DAMAGE_TYPE.store(params.damage_type, Ordering::Relaxed);
    EXCLUDE_AOW.store(params.exclude_aow as i32, Ordering::Relaxed);
    store_f64(&HP_BITS, params.hp);
    store_f64(&FP_BITS, params.fp);
    store_f64(&STAMINA_BITS, params.stamina);
}

/// Called from `hit_trampoline` with `FUN_140448910`'s own incoming args
/// (ctx, attacker, hit_info, flag) - BEFORE the real function runs, so this
/// only ever observes, never delays/replaces the game's own damage
/// application.
#[unsafe(no_mangle)]
extern "C" fn on_hit_pre(ctx: *mut c_void, attacker_ptr: *mut c_void, hit_info: *mut c_void, _flag: u64) {
    let _ = std::panic::catch_unwind(|| apply_hit_heal(ctx, attacker_ptr, hit_info));
}

fn apply_hit_heal(ctx: *mut c_void, attacker_ptr: *mut c_void, hit_info: *mut c_void) {
    let Some(player_ptr) = crate::player::main_player_chr_ins_ptr() else {
        return;
    };

    let is_player_attacker = attacker_ptr as *const u8 == player_ptr;
    let is_player_target = read_target(ctx)
        .map(|target| looks_like_pointer(target) && target as *const u8 == player_ptr)
        .unwrap_or(false);

    if is_player_attacker || is_player_target {
        regen::mark_combat_activity();
    }

    if !is_player_attacker {
        return;
    }

    if config::get_bool("DebugLog", false) {
        if let Some(damage) = read_damage(hit_info) {
            let source_type = read_source_type(hit_info);
            queue_log(format!("HitHook: player dealt {damage} damage (sourceType={source_type})."));
        }
    }

    if ENABLED.load(Ordering::Relaxed) == 0
        || !matches_damage_type(hit_info, DAMAGE_TYPE.load(Ordering::Relaxed))
        || (EXCLUDE_AOW.load(Ordering::Relaxed) != 0 && regen::is_last_attack_skill())
    {
        return;
    }

    apply_on_hit_heal(hit_info);
}

fn apply_on_hit_heal(hit_info: *const c_void) {
    let (hp_healed, fp_healed, stamina_healed, source);
    match TRIGGER.load(Ordering::Relaxed) {
        2 => {
            let Some(damage) = read_damage(hit_info) else {
                return;
            };
            if damage <= 0 {
                return;
            }
            let hp_flat = (damage as f64 * load_f64(&HP_BITS) / 100.0) as i32;
            let fp_flat = (damage as f64 * load_f64(&FP_BITS) / 100.0) as i32;
            let stamina_flat = (damage as f64 * load_f64(&STAMINA_BITS) / 100.0) as i32;
            hp_healed = regen::heal_main_player(HealField::Hp, hp_flat, 0.0);
            fp_healed = regen::heal_main_player(HealField::Fp, fp_flat, 0.0);
            stamina_healed = regen::heal_main_player(HealField::Stamina, stamina_flat, 0.0);
            source = format!("dealt {damage} damage");
        }
        1 => {
            hp_healed = regen::heal_main_player(HealField::Hp, 0, load_f64(&HP_BITS) / 100.0);
            fp_healed = regen::heal_main_player(HealField::Fp, 0, load_f64(&FP_BITS) / 100.0);
            stamina_healed =
                regen::heal_main_player(HealField::Stamina, 0, load_f64(&STAMINA_BITS) / 100.0);
            source = "hit landed".to_string();
        }
        _ => {
            hp_healed = regen::heal_main_player(HealField::Hp, load_f64(&HP_BITS).round() as i32, 0.0);
            fp_healed = regen::heal_main_player(HealField::Fp, load_f64(&FP_BITS).round() as i32, 0.0);
            stamina_healed = regen::heal_main_player(
                HealField::Stamina,
                load_f64(&STAMINA_BITS).round() as i32,
                0.0,
            );
            source = "hit landed".to_string();
        }
    }
    if (hp_healed <= 0 && fp_healed <= 0 && stamina_healed <= 0) || !config::get_bool("DebugLog", false) {
        return;
    }
    queue_log(format!(
        "HitHook: player {source} -> +{hp_healed} HP, +{fp_healed} FP, +{stamina_healed} SP"
    ));
}

/// Scans for `FUN_140448910`'s prologue and installs the hook. Returns
/// `false` (logged) if the pattern isn't found - same "fail closed, keep
/// running without this feature" contract as `attack_hook::install`.
pub fn install(params: OnHitParams) -> bool {
    update_params(params);

    let Some(fn_entry) = memscan::find_pattern_in_module(HIT_APPLY_PATTERN) else {
        logger::error("HitHook: ApplyHit prologue pattern not found, heal-on-hit disabled.");
        return false;
    };
    logger::log("HitHook: ApplyHit prologue AOB found.");

    let return_addr = fn_entry as usize + RELOCATE_LEN;
    unsafe {
        let relocated_ptr = (&raw mut G_RELOCATED_BYTES) as *mut u8;
        std::ptr::copy_nonoverlapping(fn_entry, relocated_ptr, RELOCATE_LEN);

        let tail = relocated_ptr.add(RELOCATE_LEN);
        *tail = 0x48;
        *tail.add(1) = 0xB8; // mov rax, imm64
        std::ptr::copy_nonoverlapping((return_addr as u64).to_le_bytes().as_ptr(), tail.add(2), 8);
        *tail.add(10) = 0xFF;
        *tail.add(11) = 0xE0; // jmp rax

        G_RETURN_ADDR.store(return_addr, Ordering::Relaxed);
    }

    // Patch: mov rax, <hit_trampoline>; jmp rax (10 + 2 = 12 bytes), leaving
    // 3 bytes of the 15-byte prologue window as NOP filler (control flow
    // never reaches them - they exist so a disassembler/dump reads cleanly).
    let mut patch = [0x90u8; RELOCATE_LEN];
    patch[0] = 0x48;
    patch[1] = 0xB8; // mov rax, imm64
    let trampoline_addr = hit_trampoline as *const () as usize as u64;
    patch[2..10].copy_from_slice(&trampoline_addr.to_le_bytes());
    patch[10] = 0xFF;
    patch[11] = 0xE0; // jmp rax

    let mut old_protect: u32 = 0;
    let ok = unsafe {
        VirtualProtect(
            fn_entry as *mut c_void,
            RELOCATE_LEN,
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
    };
    if ok == 0 {
        logger::error("HitHook: VirtualProtect failed, heal-on-hit disabled.");
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(patch.as_ptr(), fn_entry as *mut u8, RELOCATE_LEN);
        VirtualProtect(fn_entry as *mut c_void, RELOCATE_LEN, old_protect, &mut old_protect);

        let mut old_protect2: u32 = 0;
        VirtualProtect(
            (&raw mut G_RELOCATED_BYTES) as *mut c_void,
            RELOCATED_BUF_LEN,
            PAGE_EXECUTE_READWRITE,
            &mut old_protect2,
        );
    }

    logger::log("HitHook: installed.");
    true
}
