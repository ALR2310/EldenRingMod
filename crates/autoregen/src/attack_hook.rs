//! Inline hook into the game's own "hit lands, apply damage" call site (found
//! via a public Elden Ring Cheat Engine table, Hexinton's all-in-one v6.1 -
//! the same AOB pair backs its "NoHitbox+ReflectAttack" cheat). Unlike a
//! polling-based "did an enemy's HP just drop" heuristic, this fires exactly
//! once per real hit-resolution call, so it does NOT fire for poison/bleed
//! ticks, fall damage, or other non-hit damage sources.
//!
//! Ported 1:1 from [`SomeTweaks`](../sometweaks)'s `regen::attack_hook`
//! module (see `regen.rs`'s doc comment for why) - itself originally derived
//! from AutoRegen's own C++ `AttackHook.cpp`/`AttackTrampoline.asm`, brought
//! back here with SomeTweaks' `Regen.PerHit.Enabled`/`Trigger` config shape:
//! `Trigger` now picks between fixed points, percent of max stat, and
//! percent of damage dealt (lifesteal) - one value field per stat instead of
//! separate flat/percent ini keys.
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

use crate::regen::{self, HealField};
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

// Offset within `ctx` (arg1) of the hit's target pointer - lets
// on_attack_observed tell "player just landed a hit" (attacker==player) apart
// from "player just took a hit" (target==player), the latter only feeding
// regen::mark_combat_activity(), never a heal. Roles swap symmetrically
// between "enemy hits player" and "player hits enemy", same confirmation
// AutoRegen's original C++ port of this hook used.
const CTX_TARGET_OFFSET: isize = 0x8;

// Weapon-vs-spell filter for the OnHit heals: asks the hit's own "source
// object" what kind it is via the vtable call FromSoft's engine uses for the
// same purpose elsewhere - sourceType==1 for a direct/melee hit, ==3 for a
// bullet/projectile (most spells).
const HITINFO_SOURCE_OBJECT_OFFSET: isize = 0x1D8;

// Final TOTAL damage amount for this hit - already after defense/absorption
// AND already the sum across damage types (physical + any elemental/magic
// component on the weapon), confirmed in-game against an elemental-infused
// weapon: matches the single number the game itself shows popping up on hit,
// not a per-damage-type partial. Read raw rather than through a
// fromsoftware-rs struct since this hit-info layout isn't part of its
// reflected API either.
//
// This field is an OUTPUT of the hooked call, not an input: it isn't written
// into hit_info until the real hit-resolution function itself runs. The
// trampoline below therefore calls that function directly (via
// G_TARGET_ADDR) BEFORE calling on_attack_observed, instead of letting the
// game's own (now-skipped) CALL instruction run after us - reading this
// offset from a "peek before the real call" hook always returns 0.
const HITINFO_DAMAGE_OFFSET: isize = 0x228;

static ENABLED: AtomicI32 = AtomicI32::new(0);
static TRIGGER: AtomicI32 = AtomicI32::new(0);

// Raw Regen.PerHit.HP/FP/Stamina ini values, unscaled - what each means
// depends on TRIGGER (see OnHitParams below). Stored as raw f64 bits since
// there's no AtomicF64 in std.
static HP_BITS: AtomicU64 = AtomicU64::new(0);
static FP_BITS: AtomicU64 = AtomicU64::new(0);
static STAMINA_BITS: AtomicU64 = AtomicU64::new(0);

fn store_f64(cell: &AtomicU64, value: f64) {
    cell.store(value.to_bits(), Ordering::Relaxed);
}

fn load_f64(cell: &AtomicU64) -> f64 {
    f64::from_bits(cell.load(Ordering::Relaxed))
}

/// The `[Regen Per Hit]` ini values. `trigger` (the `Regen.PerHit.Trigger`
/// key) picks what `hp`/`fp`/`stamina` (the raw `Regen.PerHit.HP/FP/Stamina`
/// values) mean - only one of the three modes ever applies per hit:
/// - `0` (Fixed points): flat amount restored per hit, independent of max.
/// - `1` (Percent of max stat): `1` = 1% of max, same "1 = 1%" convention as
///   `Regen.PerTick.HP/FP/Stamina` under `Unit=1`.
/// - `2` (Percent of damage dealt): `1` = 1% of the hit's own damage total
///   (true lifesteal, scales with how hard the hit landed).
#[derive(Clone, Copy)]
pub struct OnHitParams {
    pub enabled: bool,
    pub trigger: i32,
    pub hp: f64,
    pub fp: f64,
    pub stamina: f64,
}

impl OnHitParams {
    /// Whether any heal would actually happen with these values - separate
    /// from whether the hook is needed just to track combat activity for
    /// `Regen.PerTick.Trigger` (see `regen.rs`).
    pub fn wants_heal(&self) -> bool {
        self.enabled && (self.hp > 0.0 || self.fp > 0.0 || self.stamina > 0.0)
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

/// Reads the hit's target pointer from `ctx+8`. `None` if `ctx` itself
/// doesn't look like a valid pointer.
fn read_target(ctx: *const c_void) -> Option<*const c_void> {
    if !looks_like_pointer(ctx) {
        return None;
    }
    unsafe { Some(*(ctx.byte_offset(CTX_TARGET_OFFSET) as *const *const c_void)) }
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
extern "C" fn on_attack_observed(ctx: *mut c_void, attacker_ptr: *mut c_void, hit_info: *mut c_void, _r14_flag: u64) {
    let _ = std::panic::catch_unwind(|| apply_hit_heal(ctx, attacker_ptr, hit_info));
}

fn apply_hit_heal(ctx: *mut c_void, attacker_ptr: *mut c_void, hit_info: *mut c_void) {
    let Some(player_ptr) = regen::main_player_chr_ins_ptr() else {
        return;
    };

    let is_player_attacker = attacker_ptr as *const u8 == player_ptr;
    let is_player_target = read_target(ctx)
        .map(|target| looks_like_pointer(target) && target as *const u8 == player_ptr)
        .unwrap_or(false);

    // Any confirmed hit the player is involved in (landed or taken) counts
    // as combat activity, regardless of whether any heal is configured -
    // this is what Regen.PerTick.Trigger=1/2 checks.
    if is_player_attacker || is_player_target {
        regen::mark_combat_activity();
    }

    if !is_player_attacker {
        return; // not the player's own hit - no heal-on-hit to apply
    }

    if config::get_bool("RegenLog", false) {
        if let Some(damage) = read_damage(hit_info) {
            logger::log(&format!("AttackHook: player dealt {damage} damage."));
        }
    }

    if ENABLED.load(Ordering::Relaxed) == 0 || !is_weapon_damage_hit(hit_info) {
        return;
    }

    apply_on_hit_heal(hit_info);
}

/// Applies the `[Regen Per Hit]` heal per the current `Trigger`. See
/// `OnHitParams` for what each of `Trigger`'s 3 modes means.
fn apply_on_hit_heal(hit_info: *const c_void) {
    let (hp_healed, fp_healed, stamina_healed, source);
    match TRIGGER.load(Ordering::Relaxed) {
        2 => {
            // Percent of damage dealt.
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
            // Percent of max stat.
            hp_healed = regen::heal_main_player(HealField::Hp, 0, load_f64(&HP_BITS) / 100.0);
            fp_healed = regen::heal_main_player(HealField::Fp, 0, load_f64(&FP_BITS) / 100.0);
            stamina_healed =
                regen::heal_main_player(HealField::Stamina, 0, load_f64(&STAMINA_BITS) / 100.0);
            source = "hit landed".to_string();
        }
        _ => {
            // Fixed points.
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
    if (hp_healed <= 0 && fp_healed <= 0 && stamina_healed <= 0) || !config::get_bool("RegenLog", false) {
        return;
    }
    logger::log(&format!(
        "AttackHook: player {source} -> +{hp_healed} HP, +{fp_healed} FP, +{stamina_healed} Stamina"
    ));
}

/// Updates the on-hit heal amounts without reinstalling the hook - lets
/// General.ReloadKey pick up new Regen.PerHit values from the ini.
pub fn update_params(params: OnHitParams) {
    ENABLED.store(params.enabled as i32, Ordering::Relaxed);
    TRIGGER.store(params.trigger, Ordering::Relaxed);
    store_f64(&HP_BITS, params.hp);
    store_f64(&FP_BITS, params.fp);
    store_f64(&STAMINA_BITS, params.stamina);
}

/// Scans for the hook site and installs it, healing per `params` (per its
/// `Trigger` mode) per confirmed player hit thereafter, and tracking combat
/// activity for `Regen.PerTick`'s `Trigger` regardless. Safe to call once at
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
    // <rel32>" CALL at call_site - read live from the game rather than
    // hardcoded, so it can't drift from the actual instruction.
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

    logger::log("AttackHook: installed - Regen Per Hit now applies only on the player's own confirmed weapon-source hits, and combat activity is tracked for Regen Per Tick's Trigger.");
    true
}
