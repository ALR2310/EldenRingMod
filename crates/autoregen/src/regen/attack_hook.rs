//! Inline hook into the game's own "hit lands, apply damage" call site (found
//! via a public Elden Ring Cheat Engine table, Hexinton's all-in-one v6.1 -
//! the same AOB pair backs its "NoHitbox+ReflectAttack" cheat). Unlike a
//! polling-based "did an enemy's HP just drop" heuristic, this fires exactly
//! once per real hit-resolution call, so it does NOT fire for poison/bleed
//! ticks, fall damage, or other non-hit damage sources.
//!
//! Ported from LifeBetween's `src/regen/attack_hook.rs`, itself ported 1:1
//! from the original C++ AttackHook.cpp + AttackTrampoline.asm: fromsoftware-rs
//! doesn't reflect this call site (it's a bespoke hook derived from a CE
//! table, not part of the game's own reflected engine types), so this part
//! stays hand-written AOB + trampoline - only the actual heal application
//! (`regen::heal_main_player`) goes through real `CSChrDataModule` fields
//! instead of raw offsets.
//!
//! Extended beyond the original AutoRegen/LifeBetween feature set with
//! HpOnDamage/FpOnDamage/StaminaOnDamage: the same hit-resolution call also
//! tells us who was on the *receiving* end of a hit (`ctx+8`), so a second,
//! independent heal can fire when the player takes a hit rather than lands
//! one - useful as a "damage taken" cushion distinct from the "reward for
//! attacking" HpOnHit/FpOnHit/StaminaOnHit.
//!
//! Register contract on entry (matches the 4 mov/movzx instructions this hook
//! overwrites in the game's code):
//!   rdi = arg1 = ctx (becomes rcx for the real call) - `[ctx+8]` is the hit's
//!         target, confirmed by symmetry testing (see AutoRegen's README):
//!         when an enemy hits the player, `[ctx+8]==player` and `rsi`=enemy;
//!         when the player hits that same enemy back, `[ctx+8]`=enemy and
//!         `rsi`==player - roles swap symmetrically, so `[ctx+8]`=target,
//!         `rsi`=attacker, unambiguously.
//!   rsi = arg2 = attacker (becomes rdx)
//!   rbx = arg3 = hit_info (becomes r8)
//!   r14b = arg4, a byte flag (becomes r9b, zero-extended into r9d)
//! rdi/rsi/rbx/r14 are Win64 non-volatile registers the surrounding game code
//! relies on being unchanged once execution jumps back - this trampoline never
//! writes to them.

use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};

use super::{self as regen, HealField};
use common::config;
use common::logger;
use common::memscan;

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

// Offset within `ctx` (arg1) of the hit's target pointer - see the module
// doc comment for how this was confirmed against `rsi` (the attacker).
const CTX_TARGET_OFFSET: isize = 0x8;

// Weapon-vs-spell filter for the OnHit heals: asks the hit's own "source
// object" what kind it is via the vtable call FromSoft's engine uses for the
// same purpose elsewhere - sourceType==1 for a direct/melee hit, ==3 for a
// bullet/projectile (most spells). Only applied to HpOnHit/FpOnHit/
// StaminaOnHit (the "reward for attacking" heals) - HpOnDamage/FpOnDamage/
// StaminaOnDamage fire on any confirmed hit landing on the player regardless
// of the attacker's damage type, since there's no equivalent "don't reward
// spell damage" reasoning on the receiving end.
const HITINFO_SOURCE_OBJECT_OFFSET: isize = 0x1D8;

// Final TOTAL damage amount for this hit - already after defense/absorption
// AND already the sum across damage types (physical + any elemental/magic
// component on the weapon). Only used for logging - read raw rather than
// through a fromsoftware-rs struct since this hit-info layout isn't part of
// its reflected API either.
//
// This field is an OUTPUT of the hooked call, not an input: it isn't written
// into hit_info until the real hit-resolution function itself runs. The
// trampoline below therefore calls that function directly (via
// G_TARGET_ADDR) BEFORE calling on_attack_observed, instead of letting the
// game's own (now-skipped) CALL instruction run after us.
const HITINFO_DAMAGE_OFFSET: isize = 0x228;

// Only used for the optional DebugLog dump below - AtkParam category/id of
// the hit, same fields the C++ AttackHook.cpp read for the same purpose.
const HITINFO_ATK_PARAM_ID_OFFSET: isize = 0x40;
const HITINFO_ATK_PARAM_CATEGORY_OFFSET: isize = 0x44;

static HP_ON_HIT: AtomicI32 = AtomicI32::new(0);
static FP_ON_HIT: AtomicI32 = AtomicI32::new(0);
static STAMINA_ON_HIT: AtomicI32 = AtomicI32::new(0);

// Percent fractions (already divided by 100, same convention as
// HpPct/FpPct/StaminaPct) - stored as raw f64 bits since there's no AtomicF64
// in std.
static HP_PCT_ON_HIT_BITS: AtomicU64 = AtomicU64::new(0);
static FP_PCT_ON_HIT_BITS: AtomicU64 = AtomicU64::new(0);
static STAMINA_PCT_ON_HIT_BITS: AtomicU64 = AtomicU64::new(0);

// HpOnDamage/FpOnDamage/StaminaOnDamage are flat-only (no percent keys in the
// ini) - a flat cushion applied whenever the player is the one taking the hit.
static HP_ON_DAMAGE: AtomicI32 = AtomicI32::new(0);
static FP_ON_DAMAGE: AtomicI32 = AtomicI32::new(0);
static STAMINA_ON_DAMAGE: AtomicI32 = AtomicI32::new(0);

fn store_pct(cell: &AtomicU64, value: f64) {
    cell.store(value.to_bits(), Ordering::Relaxed);
}

fn load_pct(cell: &AtomicU64) -> f64 {
    f64::from_bits(cell.load(Ordering::Relaxed))
}

/// The HpOnHit/FpOnHit/StaminaOnHit ini values: flat amount plus
/// percent-of-max fraction (already divided by 100) for each of HP/FP/Stamina.
/// Applied when the player's own attack lands a confirmed weapon hit.
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
    fn wants_hook(&self) -> bool {
        self.hp_flat > 0
            || self.fp_flat > 0
            || self.stamina_flat > 0
            || self.hp_pct > 0.0
            || self.fp_pct > 0.0
            || self.stamina_pct > 0.0
    }
}

/// The HpOnDamage/FpOnDamage/StaminaOnDamage ini values: flat amount only.
/// Applied when the player is the one on the receiving end of a confirmed
/// hit from something else.
#[derive(Clone, Copy)]
pub struct OnDamageParams {
    pub hp_flat: i32,
    pub fp_flat: i32,
    pub stamina_flat: i32,
}

impl OnDamageParams {
    fn wants_hook(&self) -> bool {
        self.hp_flat > 0 || self.fp_flat > 0 || self.stamina_flat > 0
    }
}

#[derive(Clone, Copy)]
pub struct HookParams {
    pub on_hit: OnHitParams,
    pub on_damage: OnDamageParams,
}

impl HookParams {
    pub fn wants_hook(&self) -> bool {
        self.on_hit.wants_hook() || self.on_damage.wants_hook()
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

/// Logged only when `DebugLog=1` - lets a specific weapon/spell that's being
/// classified wrong (see `is_weapon_damage_hit`) be diagnosed from the log
/// alone, same fields the C++ AttackHook.cpp dumped for the same purpose.
fn log_hit_debug(hit_info: *const c_void) {
    if !looks_like_pointer(hit_info) {
        return;
    }
    unsafe {
        let atk_category = *(hit_info.byte_offset(HITINFO_ATK_PARAM_CATEGORY_OFFSET) as *const i32);
        let atk_id = *(hit_info.byte_offset(HITINFO_ATK_PARAM_ID_OFFSET) as *const i32);
        let damage = *(hit_info.byte_offset(HITINFO_DAMAGE_OFFSET) as *const i32);
        let source_object = *(hit_info.byte_offset(HITINFO_SOURCE_OBJECT_OFFSET) as *const *const c_void);
        let source_type = get_source_object_type(source_object);
        logger::log(&format!(
            "AttackHook debug: hitInfo={hit_info:p} atkCategory={atk_category} atkId={atk_id} damage={damage} sourceObj={source_object:p} sourceType={source_type}"
        ));
    }
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

/// Reads the hit's target pointer from `ctx+8`. `None` if `ctx` itself
/// doesn't look like a valid pointer.
fn read_target(ctx: *const c_void) -> Option<*const c_void> {
    if !looks_like_pointer(ctx) {
        return None;
    }
    unsafe { Some(*(ctx.byte_offset(CTX_TARGET_OFFSET) as *const *const c_void)) }
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
extern "C" fn on_attack_observed(ctx: *mut c_void, attacker_ptr: *mut c_void, hit_info: *mut c_void, _r14_flag: u64) {
    let _ = std::panic::catch_unwind(|| apply_hit_heal(ctx, attacker_ptr, hit_info));
}

fn apply_hit_heal(ctx: *mut c_void, attacker_ptr: *mut c_void, hit_info: *mut c_void) {
    let Some(player_ptr) = regen::main_player_chr_ins_ptr() else {
        return;
    };

    if config::get_bool("DebugLog", false) {
        log_hit_debug(hit_info);
    }

    let is_player_attacker = attacker_ptr as *const u8 == player_ptr;
    if is_player_attacker && is_weapon_damage_hit(hit_info) {
        apply_on_hit_heal();
    }

    let is_player_target = read_target(ctx)
        .map(|target| looks_like_pointer(target) && target as *const u8 == player_ptr)
        .unwrap_or(false);
    // Excludes the (impossible in practice, but cheap to guard) case of the
    // player hitting themselves - only a hit from something else counts as
    // "damage taken".
    if is_player_target && !is_player_attacker {
        apply_on_damage_heal();
    }
}

fn apply_on_hit_heal() {
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

fn apply_on_damage_heal() {
    let hp_healed = regen::heal_main_player(HealField::Hp, HP_ON_DAMAGE.load(Ordering::Relaxed), 0.0);
    let fp_healed = regen::heal_main_player(HealField::Fp, FP_ON_DAMAGE.load(Ordering::Relaxed), 0.0);
    let stamina_healed = regen::heal_main_player(HealField::Stamina, STAMINA_ON_DAMAGE.load(Ordering::Relaxed), 0.0);
    if hp_healed <= 0 && fp_healed <= 0 && stamina_healed <= 0 {
        return;
    }
    logger::log(&format!(
        "AttackHook: player took a hit -> +{hp_healed} HP, +{fp_healed} FP, +{stamina_healed} Stamina"
    ));
}

/// Updates the amounts healed per hit/per damage taken without reinstalling
/// the hook - lets ReloadKey pick up new values from the ini.
pub fn update_params(params: HookParams) {
    HP_ON_HIT.store(params.on_hit.hp_flat, Ordering::Relaxed);
    FP_ON_HIT.store(params.on_hit.fp_flat, Ordering::Relaxed);
    STAMINA_ON_HIT.store(params.on_hit.stamina_flat, Ordering::Relaxed);
    store_pct(&HP_PCT_ON_HIT_BITS, params.on_hit.hp_pct);
    store_pct(&FP_PCT_ON_HIT_BITS, params.on_hit.fp_pct);
    store_pct(&STAMINA_PCT_ON_HIT_BITS, params.on_hit.stamina_pct);

    HP_ON_DAMAGE.store(params.on_damage.hp_flat, Ordering::Relaxed);
    FP_ON_DAMAGE.store(params.on_damage.fp_flat, Ordering::Relaxed);
    STAMINA_ON_DAMAGE.store(params.on_damage.stamina_flat, Ordering::Relaxed);
}

/// Scans for the hook site and installs it, healing per `params` (on-hit and
/// on-damage) thereafter. Safe to call once at startup; returns `false` (and
/// logs why) if the AOB isn't found - callers should keep running without
/// this feature rather than treat it as fatal.
pub fn install(params: HookParams) -> bool {
    update_params(params);

    let Some(on_attack) = memscan::find_pattern_in_module(ON_ATTACK_PATTERN) else {
        // Also expected if another mod (e.g. Seamless Co-op) already hooked
        // this same call site first - fails closed rather than overwriting
        // whatever it installed.
        logger::log("AttackHook: ERROR - OnAttack pattern not found (possibly patched by another mod), heal-on-hit/heal-on-damage disabled.");
        return false;
    };

    let call_site = unsafe { on_attack.add(CALL_SITE_OFFSET) };

    // Resolve the real hit-resolution function from the untouched "E8
    // <rel32>" CALL at call_site - read live from the game rather than
    // hardcoded, so it can't drift from the actual instruction.
    let call_opcode = unsafe { *call_site };
    if call_opcode != 0xE8 {
        logger::log("AttackHook: ERROR - byte at the expected CALL site isn't 0xE8 (layout differs from expected), heal-on-hit/heal-on-damage disabled.");
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
        logger::log("AttackHook: ERROR - VirtualProtect failed, heal-on-hit/heal-on-damage disabled.");
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(patch.as_ptr(), on_attack, patch.len());
        VirtualProtect(on_attack as *mut c_void, patch.len(), old_protect, &mut old_protect);
    }

    logger::log("AttackHook: installed - HpOnHit/FpOnHit/StaminaOnHit and HpOnDamage/FpOnDamage/StaminaOnDamage are now active.");
    true
}
