//! Inline hook into the game's own "hit lands, apply damage" call site (found
//! via a public Elden Ring Cheat Engine table, Hexinton's all-in-one v6.1 -
//! the same AOB pair backs its "NoHitbox+ReflectAttack" cheat). Unlike a
//! polling-based "did an enemy's HP just drop" heuristic, this fires exactly
//! once per real hit-resolution call, so it does NOT fire for poison/bleed
//! ticks, fall damage, or other non-hit damage sources.
//!
//! Ported 1:1 from the C++ AttackHook.cpp + AttackTrampoline.asm in AutoRegen:
//! fromsoftware-rs doesn't reflect this call site (it's a bespoke hook derived
//! from a CE table, not part of the game's own reflected engine types), so
//! this part stays hand-written AOB + trampoline, same as before - only the
//! actual heal application (`regen::heal_main_player`) now goes through real
//! `CSChrDataModule` fields instead of raw offsets.
//!
//! Register contract on entry (matches the 4 mov/movzx instructions this hook
//! overwrites in the game's code):
//!   rdi = arg1 (becomes rcx for the real call)
//!   rsi = arg2 (becomes rdx)
//!   rbx = arg3 (becomes r8)
//!   r14b = arg4, a byte flag (becomes r9b, zero-extended into r9d)
//! rdi/rsi/rbx/r14 are Win64 non-volatile registers the surrounding game code
//! relies on being unchanged once execution jumps back - this trampoline never
//! writes to them.

use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};

use common::logger;
use common::memscan;
use super::{self as regen, HealField};

unsafe extern "system" {
    fn VirtualProtect(
        lp_address: *mut c_void,
        dw_size: usize,
        fl_new_protect: u32,
        lpfl_old_protect: *mut u32,
    ) -> i32;
}

const PAGE_EXECUTE_READWRITE: u32 = 0x40;

// From Hexinton's public CE table ("NoHitbox+ReflectAttack"). Targets the
// hit-resolution call site: fires once per landed hit, never for DOT/fall
// damage. rsi=attacker, [ctx+8]=target (roles swap symmetrically between
// "enemy hits player" and "player hits enemy").
const ON_ATTACK_PATTERN: &str = "45 0F B6 CE 4C 8B C3 48 8B D6 48 8B CF E8";
// Offset to the untouched "E8 <rel32>" CALL - the first 13 bytes are the 4
// mov/movzx instructions Install() patches over.
const CALL_SITE_OFFSET: usize = 0xD;

// Weapon-vs-spell filter for the OnHit heals: asks the hit's own "source
// object" what kind it is via the vtable call FromSoft's engine uses for the
// same purpose elsewhere - sourceType==1 for a direct/melee hit, ==3 for a
// bullet/projectile (most spells).
const HITINFO_SOURCE_OBJECT_OFFSET: isize = 0x1D8;

// Final TOTAL damage amount for this hit - already after defense/absorption
// AND already the sum across damage types (physical + any elemental/magic
// component on the weapon), confirmed in-game against an elemental-infused
// weapon (2026-08-18): matches the single number the game itself shows
// popping up on hit, not a per-damage-type partial. Only used for logging -
// read raw rather than through a fromsoftware-rs struct since this hit-info
// layout isn't part of its reflected API either.
//
// This field is an OUTPUT of the hooked call, not an input: it isn't written
// into hit_info until the real hit-resolution function itself runs. The
// trampoline below therefore calls that function directly (via
// G_TARGET_ADDR) BEFORE calling on_attack_observed, instead of letting the
// game's own (now-skipped) CALL instruction run after us like the original
// AutoRegen hook did - reading this offset from the old "peek before the
// real call" hook always returned 0, confirmed in-game (2026-08-17).
const HITINFO_DAMAGE_OFFSET: isize = 0x228;

static HP_ON_HIT: AtomicI32 = AtomicI32::new(0);
static FP_ON_HIT: AtomicI32 = AtomicI32::new(0);
static STAMINA_ON_HIT: AtomicI32 = AtomicI32::new(0);

// Percent fractions (already divided by 100, same convention as
// Regen.HpPct/FpPct/StaminaPct) - stored as raw f64 bits since there's no
// AtomicF64 in std.
static HP_PCT_ON_HIT_BITS: AtomicU64 = AtomicU64::new(0);
static FP_PCT_ON_HIT_BITS: AtomicU64 = AtomicU64::new(0);
static STAMINA_PCT_ON_HIT_BITS: AtomicU64 = AtomicU64::new(0);

fn store_pct(cell: &AtomicU64, value: f64) {
    cell.store(value.to_bits(), Ordering::Relaxed);
}

fn load_pct(cell: &AtomicU64) -> f64 {
    f64::from_bits(cell.load(Ordering::Relaxed))
}

/// The 6 Regen.*OnHit ini values: flat amount plus percent-of-max fraction
/// (already divided by 100) for each of HP/FP/Stamina.
#[derive(Clone, Copy)]
pub struct OnHitParams {
    pub hp_flat: i32,
    pub fp_flat: i32,
    pub stamina_flat: i32,
    pub hp_pct: f64,
    pub fp_pct: f64,
    pub stamina_pct: f64,
}

impl OnHitParams {
    pub fn wants_hook(&self) -> bool {
        self.hp_flat > 0
            || self.fp_flat > 0
            || self.stamina_flat > 0
            || self.hp_pct > 0.0
            || self.fp_pct > 0.0
            || self.stamina_pct > 0.0
    }
}

// Read by AttackTrampoline (see the global_asm! block below) as the address to
// jump to once it's done - the instruction right after the original CALL
// (call_site + 5), since that CALL is no longer executed by the game itself
// (see G_TARGET_ADDR below) - we call it ourselves instead, so control must
// resume past it, not at it.
#[unsafe(no_mangle)]
static G_RETURN_ADDR: AtomicUsize = AtomicUsize::new(0);

// The real hit-resolution function's address, resolved once at install time
// from the untouched "E8 <rel32>" CALL this hook sits next to. The
// trampoline calls this directly so it can read hit_info's OUTPUT fields
// (like damage) afterward, then jumps past the original CALL entirely
// instead of letting the game execute it a second time.
#[unsafe(no_mangle)]
static G_TARGET_ADDR: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" {
    fn attack_trampoline();
}

std::arch::global_asm!(
    r#"
.global attack_trampoline
attack_trampoline:
    push    r12
    push    r13
    mov     r12, rsp

    and     rsp, -16
    sub     rsp, 0x20

    # Call the real hit-resolution function ourselves first, with the exact
    # same args the game's own (now-skipped) CALL would have used - this is
    # what actually applies the hit and fills in hit_info's output fields.
    mov     rcx, rdi
    mov     rdx, rsi
    mov     r8,  rbx
    movzx   r9d, r14b
    mov     rax, qword ptr [rip + G_TARGET_ADDR]
    call    rax
    mov     r13, rax        # stash its return value - callers past call_site+5 may depend on it

    # Now observe: hit_info is fully populated at this point.
    mov     rcx, rdi
    mov     rdx, rsi
    mov     r8,  rbx
    movzx   r9d, r14b
    call    on_attack_observed

    mov     rax, r13        # restore the real function's return value
    mov     rsp, r12
    pop     r13
    pop     r12

    # r11 is volatile (caller-saved) and free to clobber here - rax must stay
    # untouched since it now holds the real function's return value, and
    # r12/r13/rdi/rsi/rbx/r14 were just restored to their original values.
    mov     r11, qword ptr [rip + G_RETURN_ADDR]
    jmp     r11
"#
);

fn looks_like_pointer(p: *const c_void) -> bool {
    !p.is_null() && (p as usize) >= 0x10000
}

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

fn read_damage(hit_info: *const c_void) -> Option<i32> {
    if !looks_like_pointer(hit_info) {
        return None;
    }
    unsafe { Some(*(hit_info.byte_offset(HITINFO_DAMAGE_OFFSET) as *const i32)) }
}

fn is_weapon_damage_hit(hit_info: *const c_void) -> bool {
    if !looks_like_pointer(hit_info) {
        return false;
    }
    unsafe {
        let source_object = *(hit_info.byte_offset(HITINFO_SOURCE_OBJECT_OFFSET) as *const *const c_void);
        let source_type = get_source_object_type(source_object);
        if source_type == 3 {
            return false; // bullet/projectile-style source
        }
        source_type == 1
    }
}

/// Called from the trampoline on every hit-resolution call, with the same 4
/// arguments the real game function receives. Must not throw/panic - wrapped
/// in `catch_unwind` so an unexpected condition skips one heal instead of
/// aborting the process outright.
///
/// Note: unlike the C++ version, this does not install a Windows SEH (__try/
/// __except) guard around the actual pointer dereferences below - Rust has no
/// direct equivalent short of a global vectored exception handler, which is a
/// separate, more invasive undertaking. `looks_like_pointer` sanity checks are
/// kept before every dereference as the primary safety net instead.
#[unsafe(no_mangle)]
extern "C" fn on_attack_observed(_ctx: *mut c_void, attacker_ptr: *mut c_void, hit_info: *mut c_void, _r14_flag: u64) {
    let _ = std::panic::catch_unwind(|| apply_hit_heal(attacker_ptr, hit_info));
}

fn apply_hit_heal(attacker_ptr: *mut c_void, hit_info: *mut c_void) {
    let Some(player_ptr) = regen::main_player_chr_ins_ptr() else {
        return;
    };
    if attacker_ptr as *const u8 != player_ptr {
        return; // not the player's own hit
    }

    if let Some(damage) = read_damage(hit_info) {
        logger::log(&format!("AttackHook: player dealt {damage} damage."));
    }

    if !is_weapon_damage_hit(hit_info) {
        return;
    }

    let hp_healed = regen::heal_main_player(
        HealField::Hp,
        HP_ON_HIT.load(Ordering::Relaxed),
        load_pct(&HP_PCT_ON_HIT_BITS),
    );
    let fp_healed = regen::heal_main_player(
        HealField::Fp,
        FP_ON_HIT.load(Ordering::Relaxed),
        load_pct(&FP_PCT_ON_HIT_BITS),
    );
    let stamina_healed = regen::heal_main_player(
        HealField::Stamina,
        STAMINA_ON_HIT.load(Ordering::Relaxed),
        load_pct(&STAMINA_PCT_ON_HIT_BITS),
    );
    if hp_healed <= 0 && fp_healed <= 0 && stamina_healed <= 0 {
        return;
    }

    logger::log(&format!(
        "AttackHook: player hit landed -> +{hp_healed} HP, +{fp_healed} FP, +{stamina_healed} Stamina"
    ));
}

/// Updates the amounts healed per hit without reinstalling the hook - lets
/// General.ReloadKey pick up new Regen.*OnHit values from the ini.
pub fn update_params(params: OnHitParams) {
    HP_ON_HIT.store(params.hp_flat, Ordering::Relaxed);
    FP_ON_HIT.store(params.fp_flat, Ordering::Relaxed);
    STAMINA_ON_HIT.store(params.stamina_flat, Ordering::Relaxed);
    store_pct(&HP_PCT_ON_HIT_BITS, params.hp_pct);
    store_pct(&FP_PCT_ON_HIT_BITS, params.fp_pct);
    store_pct(&STAMINA_PCT_ON_HIT_BITS, params.stamina_pct);
}

/// Scans for the hook site and installs it, healing per `params` (flat plus
/// percent-of-max) per confirmed player hit thereafter. Safe to call once at
/// startup; returns `false` (and logs why) if the AOB isn't found - callers
/// should keep running without this feature rather than treat it as fatal.
pub fn install(params: OnHitParams) -> bool {
    update_params(params);

    let Some(on_attack) = memscan::find_pattern_in_module(ON_ATTACK_PATTERN) else {
        // Also expected if another mod (e.g. Seamless Co-op) already hooked
        // this same call site first - fails closed rather than overwriting
        // whatever it installed.
        logger::log("AttackHook: ERROR - OnAttack pattern not found (possibly patched by another mod), heal-on-hit disabled.");
        return false;
    };

    let call_site = unsafe { on_attack.add(CALL_SITE_OFFSET) };

    // Resolve the real hit-resolution function from the untouched "E8
    // <rel32>" CALL at call_site, the same way RuneMultiplier resolves
    // AddSoul_Call - read live from the game rather than hardcoded, so it
    // can't drift from the actual instruction.
    let call_opcode = unsafe { *call_site };
    if call_opcode != 0xE8 {
        logger::log("AttackHook: ERROR - byte at the expected CALL site isn't 0xE8 (layout differs from expected), heal-on-hit disabled.");
        return false;
    }
    let rel32 = unsafe { i32::from_le_bytes(*(call_site.add(1) as *const [u8; 4])) };
    let target_addr = unsafe { call_site.add(5).offset(rel32 as isize) } as usize;
    G_TARGET_ADDR.store(target_addr, Ordering::Relaxed);
    // Resume past the CALL (call_site + 5) once the trampoline is done -
    // it calls target_addr itself, so the game's own CALL instruction must
    // never execute (that would apply the hit twice).
    G_RETURN_ADDR.store(call_site as usize + 5, Ordering::Relaxed);

    // Patch: mov rax, <attack_trampoline>; jmp rax (10 + 2 = 12 bytes),
    // overwriting only the 4 mov/movzx instructions before the real CALL -
    // the CALL itself (at on_attack+0xD) is left completely untouched.
    let mut patch = [0u8; 12];
    patch[0] = 0x48;
    patch[1] = 0xB8; // mov rax, imm64
    let trampoline_addr = attack_trampoline as *const () as usize as u64;
    patch[2..10].copy_from_slice(&trampoline_addr.to_le_bytes());
    patch[10] = 0xFF;
    patch[11] = 0xE0; // jmp rax

    let mut old_protect: u32 = 0;
    let ok = unsafe {
        VirtualProtect(
            on_attack as *mut c_void,
            patch.len(),
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
    };
    if ok == 0 {
        logger::log("AttackHook: ERROR - VirtualProtect failed, heal-on-hit disabled.");
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(patch.as_ptr(), on_attack, patch.len());
        VirtualProtect(on_attack as *mut c_void, patch.len(), old_protect, &mut old_protect);
    }

    logger::log("AttackHook: installed - Regen.*OnHit now applies only on the player's own confirmed weapon-source hits.");
    true
}
