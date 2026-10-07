//! Hooks the game's own talk-message lookup function so `grace_menu`'s 3
//! inserted Site of Grace items can show a real vanilla word for their
//! own displayed text - fetched live, in whatever language the game is
//! currently displaying, not hardcoded to one language - see
//! [vanilla_text].
//!
//! Ported from the open-source `erdGameTools` project
//! (`.docs/Elden_Ring_game_tools/src/grace_test_messages.cpp`, an actively
//! maintained mod confirmed already working against the exact game
//! version `mod_test` runs) - same idea `EldenConvenienceMod` and every
//! other Grace-menu mod investigated here settled on: nobody patches the
//! game's own FMG text data. Instead, the lookup FUNCTION itself is
//! hooked; for the "event text for talk" message category (bnd id 33)
//! and one of a small set of IDs THIS module reserves (in the 90000000+
//! range - comfortably clear of both real vanilla IDs, which run in the
//! low tens of millions, and `erdGameTools`' own reserved
//! `69010000..69015000` range, in case both mods are ever loaded
//! together), the hook returns a string this module owns; every other
//! `(bnd_id, msg_id)` falls through to the real lookup, untouched.
//!
//! ## Hooking `get_message`
//!
//! `get_message`'s real address isn't fixed - resolved the same way
//! `erdGameTools` does: an AOB anchor pattern matches a CALL SITE
//! elsewhere in the game that invokes it, and the CALL's own rel32
//! operand is followed to get `get_message`'s actual entry (confirmed via
//! Ghidra against the current game build, 2026-08-28 -
//! `.docs/reverse_engineering/DumpGetMessageFn.java`/`D:/tmp/get_message_dump.txt`).
//! Its signature (Win64 fastcall): `(msg_repository: rcx, unknown: edx,
//! bnd_id: r8d, msg_id: r9d) -> rax (const wchar_t*)`.
//!
//! Its own first 15 bytes (verified live, not hardcoded, before ever
//! patching - fails closed if they don't match what was confirmed via
//! Ghidra) are:
//! ```text
//! 3B 51 10          cmp edx, dword ptr [rcx+0x10]
//! 73 xx             jae <fail1>                      (rel8, read live)
//! 44 3B 41 14       cmp r8d, dword ptr [rcx+0x14]
//! 73 xx             jae <fail2>                      (rel8, read live)
//! 48 8B 41 08       mov rax, qword ptr [rcx+8]
//! ```
//! Both `jae`s branch to the same "out of range, return 0" tail further
//! into the function (confirmed by the Ghidra decompile: both range
//! checks guard the same lookup). Unlike every other hook in this crate
//! (whose replayed prologue bytes never contain a jump - relocating them
//! to a stub at a different address is always safe verbatim), these 2
//! conditional jumps DO need fixing up: their `rel8` displacements are
//! only valid at their ORIGINAL address, so the stub replays each as an
//! inverted short jump (`jb` instead of `jae`) skipping over an absolute
//! `mov r11, imm64; jmp r11` to the jump's real, computed-live target
//! (standard technique for relocating a short/near conditional jump to a
//! destination outside a plain rel8/rel32's reach) - not `jae` itself,
//! since a relocated stub can land anywhere in the 64-bit address space,
//! arbitrarily far from where the jump's real target sits.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use common::logger;
use common::memscan;

unsafe extern "system" {
    fn VirtualAlloc(lp_address: *mut std::ffi::c_void, dw_size: usize, fl_allocation_type: u32, fl_protect: u32) -> *mut std::ffi::c_void;
    fn VirtualProtect(lp_address: *mut std::ffi::c_void, dw_size: usize, fl_new_protect: u32, lpfl_old_protect: *mut u32) -> i32;
    fn FlushInstructionCache(h_process: *mut std::ffi::c_void, lp_base_address: *const std::ffi::c_void, dw_size: usize) -> i32;
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
}

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

// The msgbnd category for Site of Grace / NPC talk menu text - erdGameTools'
// own `msgbnd_event_text_for_talk`.
const EVENT_TEXT_FOR_TALK_BND_ID: u32 = 33;

/// This module's own reserved message IDs - never real FMG entries, only
/// ever resolved by [get_message_detour] below. `grace_menu` uses these
/// directly as the `message_id` passed to `add_talk_list_data`.
pub const UPGRADE_MSG_ID: i32 = 90000001;
pub const SHOP_MSG_ID: i32 = 90000002;
pub const SELL_MSG_ID: i32 = 90000003;

// The REAL vanilla message IDs whose own text these entries show (see
// [vanilla_text]) - the same IDs `EldenConvenienceMod` borrowed wholesale
// for its own equivalent Upgrade/Purchase/Sell entries. Fetched live
// through the hooked function itself (see [call_get_message]) so the
// text always matches whatever language the game is currently
// displaying, instead of hardcoding one language's wording.
//
// `pub` so `mod.rs` can use these directly as the `add_talk_list_data`
// message id (bypassing [UPGRADE_MSG_ID]/[SHOP_MSG_ID]/[SELL_MSG_ID]
// entirely) when [is_installed] is false - see its doc comment.
pub const VANILLA_UPGRADE_MSG_ID: i32 = 22130001;
pub const VANILLA_SHOP_MSG_ID: i32 = 26000010;
pub const VANILLA_SELL_MSG_ID: i32 = 20000011;

fn to_utf16_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Reads a null-terminated UTF-16 buffer the game owns into an owned,
/// null-terminated `Vec<u16>` this module owns instead - the game's own
/// buffer isn't guaranteed to stay valid/unchanged indefinitely, unlike
/// this module's own permanently-leaked strings.
unsafe fn copy_wide_cstr(ptr: *const u16) -> Vec<u16> {
    if ptr.is_null() {
        return Vec::new();
    }
    let mut len = 0;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let mut out = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
    out.push(0);
    out
}

/// Calls back into `get_message` itself (through the same hooked entry
/// point - safe to call recursively: `vanilla_msg_id` is never one of
/// this module's own reserved IDs, so [get_message_detour] just falls
/// through to the real lookup for it, same as any other caller in the
/// game would see) to fetch a REAL vanilla message's own text, in
/// whatever language the game is currently displaying.
unsafe fn call_get_message(msg_repository: usize, unknown: u32, bnd_id: u32, vanilla_msg_id: i32) -> Vec<u16> {
    let addr = GET_MESSAGE_ADDR.load(Ordering::Relaxed);
    if addr == 0 {
        return Vec::new();
    }
    type GetMessageFn = extern "system" fn(usize, u32, u32, i32) -> *const u16;
    let f: GetMessageFn = unsafe { std::mem::transmute::<usize, GetMessageFn>(addr) };
    unsafe { copy_wide_cstr(f(msg_repository, unknown, bnd_id, vanilla_msg_id)) }
}

/// Fetches `vanilla_msg_id`'s own text once (cached forever after the
/// first successful fetch - a talk-list entry's own text is asked for
/// repeatedly, every time the menu list is drawn) and leaks it
/// permanently, same as a fully custom string would be.
fn vanilla_text(msg_repository: usize, unknown: u32, bnd_id: u32, vanilla_msg_id: i32) -> Vec<u16> {
    let vanilla = unsafe { call_get_message(msg_repository, unknown, bnd_id, vanilla_msg_id) };
    let vanilla_len = vanilla.len().saturating_sub(1); // drop the trailing NUL before decoding
    to_utf16_z(&String::from_utf16_lossy(&vanilla[..vanilla_len]))
}

fn custom_text_ptr(msg_repository: usize, unknown: u32, bnd_id: u32, msg_id: i32) -> Option<*const u16> {
    static UPGRADE_UTF16: OnceLock<Vec<u16>> = OnceLock::new();
    static SHOP_UTF16: OnceLock<Vec<u16>> = OnceLock::new();
    static SELL_UTF16: OnceLock<Vec<u16>> = OnceLock::new();

    match msg_id {
        UPGRADE_MSG_ID => Some(UPGRADE_UTF16.get_or_init(|| vanilla_text(msg_repository, unknown, bnd_id, VANILLA_UPGRADE_MSG_ID)).as_ptr()),
        SHOP_MSG_ID => Some(SHOP_UTF16.get_or_init(|| vanilla_text(msg_repository, unknown, bnd_id, VANILLA_SHOP_MSG_ID)).as_ptr()),
        SELL_MSG_ID => Some(SELL_UTF16.get_or_init(|| vanilla_text(msg_repository, unknown, bnd_id, VANILLA_SELL_MSG_ID)).as_ptr()),
        _ => None,
    }
}

static GET_MESSAGE_ADDR: AtomicUsize = AtomicUsize::new(0);
static HOOK_INSTALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether [install] actually hooked `get_message` this session. `false`
/// (anchor pattern not found, or the game's own prologue no longer
/// matches what Ghidra confirmed - e.g. after a game update changes the
/// function's exact bytes, as happened going from 1.16.2 to 1.17.0) means
/// [UPGRADE_MSG_ID]/[SHOP_MSG_ID]/[SELL_MSG_ID] resolve to nothing (never
/// real FMG entries, and nothing left to intercept the lookup for them) -
/// `mod.rs` checks this to fall back to the REAL vanilla ids
/// ([VANILLA_UPGRADE_MSG_ID] etc.) directly instead, so the menu items
/// still show real, correctly-localized text rather than going blank.
pub fn is_installed() -> bool {
    HOOK_INSTALLED.load(Ordering::Relaxed)
}

/// Called from the stub (see [install]) with the exact same 4 arguments
/// the real `get_message` receives. Returns 0 ("not handled - continue
/// into the real function") or a `*const u16` to a permanently-owned,
/// null-terminated string this module leaks once and reuses forever.
#[unsafe(no_mangle)]
extern "system" fn get_message_detour(msg_repository: usize, unknown: u32, bnd_id: u32, msg_id: i32) -> usize {
    if bnd_id != EVENT_TEXT_FOR_TALK_BND_ID {
        return 0;
    }
    custom_text_ptr(msg_repository, unknown, bnd_id, msg_id).map_or(0, |p| p as usize)
}

const GET_MESSAGE_CALL_SITE_PATTERN: &str = "8B DA 44 8B CA 33 D2 48 8B F9 44 8D 42 6F";
const GET_MESSAGE_CALL_OFFSET: usize = 14;

// The 5 non-jump bytes verified live before patching (see module doc
// comment) - if the game updates and these no longer match, [install]
// fails closed instead of guessing.
const EXPECTED_CMP1: [u8; 3] = [0x3B, 0x51, 0x10];
const EXPECTED_JAE_OPCODE: u8 = 0x73;
const EXPECTED_CMP2: [u8; 4] = [0x44, 0x3B, 0x41, 0x14];
const EXPECTED_MOV: [u8; 4] = [0x48, 0x8B, 0x41, 0x08];
const PROLOGUE_LEN: usize = 15;

/// Builds the stub's bytes. Every jump it emits targets an ABSOLUTE
/// address (either something else entirely, or a fixed-length skip
/// within its own body) - none of them depend on where this stub itself
/// ends up allocated, so this can run before or after `VirtualAlloc`.
/// See the module doc comment for the full layout/reasoning.
fn build_stub(get_message: *mut u8, jump1_target: *mut u8, jump2_target: *mut u8) -> Vec<u8> {
    let detour_addr = get_message_detour as *const () as u64;
    let mut body = Vec::with_capacity(100);

    // ---- section A: call the detour, return early if it handled this id ----
    body.push(0x51); // push rcx
    body.push(0x52); // push rdx
    body.extend_from_slice(&[0x41, 0x50]); // push r8
    body.extend_from_slice(&[0x41, 0x51]); // push r9
    body.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub rsp, 0x28
    body.extend_from_slice(&[0x49, 0xBB]); // mov r11, imm64
    body.extend_from_slice(&detour_addr.to_le_bytes());
    body.extend_from_slice(&[0x41, 0xFF, 0xD3]); // call r11
    body.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    body.extend_from_slice(&[0x41, 0x59]); // pop r9
    body.extend_from_slice(&[0x41, 0x58]); // pop r8
    body.push(0x5A); // pop rdx
    body.push(0x59); // pop rcx
    body.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
    // jz rel32 -> "not_handled" (section B, right after this jz) - rel32 is
    // always exactly 1 here since section B starts immediately after this
    // 6-byte instruction, but computed rather than hardcoded for clarity.
    let jz_end = body.len() + 6;
    let not_handled_label = jz_end + 1; // section B starts right here
    body.extend_from_slice(&[0x0F, 0x84]);
    body.extend_from_slice(&((not_handled_label as i64 - jz_end as i64) as i32).to_le_bytes());
    body.push(0xC3); // ret (handled: rax already holds the custom string)
    debug_assert_eq!(body.len(), not_handled_label);

    // ---- section B: replay the original prologue, jumps fixed up ----
    body.extend_from_slice(&EXPECTED_CMP1);
    push_relocated_jae(&mut body, jump1_target);
    body.extend_from_slice(&EXPECTED_CMP2);
    push_relocated_jae(&mut body, jump2_target);
    body.extend_from_slice(&EXPECTED_MOV);
    // mov r11, imm64(get_message + PROLOGUE_LEN); jmp r11 - resume the
    // real function right after the bytes we overwrote.
    let resume_addr = unsafe { get_message.add(PROLOGUE_LEN) } as u64;
    body.extend_from_slice(&[0x49, 0xBB]);
    body.extend_from_slice(&resume_addr.to_le_bytes());
    body.extend_from_slice(&[0x41, 0xFF, 0xE3]); // jmp r11

    body
}

/// Appends `jb <skip>; mov r11, imm64(target); jmp r11; <skip>:` - an
/// inverted-and-skipped near jump, the standard way to relocate a short
/// conditional jump (`jae`, here) to a target outside a rel8/rel32's
/// reach from the new location. `body`'s length must already reflect
/// everything written before this call - the `jb`'s rel8 is computed
/// against the fixed 13-byte block it skips (`mov r11,imm64` + `jmp r11`).
fn push_relocated_jae(body: &mut Vec<u8>, target: *mut u8) {
    const SKIP_BLOCK_LEN: u8 = 13; // mov r11,imm64 (10) + jmp r11 (3)
    body.extend_from_slice(&[0x72, SKIP_BLOCK_LEN]); // jb rel8
    body.extend_from_slice(&[0x49, 0xBB]); // mov r11, imm64
    body.extend_from_slice(&(target as u64).to_le_bytes());
    body.extend_from_slice(&[0x41, 0xFF, 0xE3]); // jmp r11
}

pub fn install() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(GET_MESSAGE_CALL_SITE_PATTERN, Duration::from_millis(500), Duration::from_secs(60))
    else {
        logger::error("GraceMenu: get_message call-site anchor pattern not found within the timeout. Game may have been updated - custom menu text disabled.");
        return false;
    };

    let call_site = unsafe { anchor.add(GET_MESSAGE_CALL_OFFSET) };
    if unsafe { *call_site } != 0xE8 {
        logger::error("GraceMenu: byte at the expected get_message CALL site isn't 0xE8 (layout differs from expected), custom menu text disabled.");
        return false;
    }
    let rel32 = unsafe { i32::from_le_bytes(*(call_site.add(1) as *const [u8; 4])) };
    let get_message = unsafe { call_site.add(5).offset(rel32 as isize) };

    let prologue = unsafe { std::slice::from_raw_parts(get_message, PROLOGUE_LEN) };
    if prologue[0..3] != EXPECTED_CMP1
        || prologue[3] != EXPECTED_JAE_OPCODE
        || prologue[5..9] != EXPECTED_CMP2
        || prologue[9] != EXPECTED_JAE_OPCODE
        || prologue[11..15] != EXPECTED_MOV
    {
        logger::error("GraceMenu: get_message prologue changed, custom menu text disabled.");
        return false;
    }
    let rel8_1 = prologue[4] as i8;
    let rel8_2 = prologue[10] as i8;
    let jump1_target = unsafe { get_message.add(5).offset(rel8_1 as isize) };
    let jump2_target = unsafe { get_message.add(11).offset(rel8_2 as isize) };

    // Stored BEFORE patching - [call_get_message] calls through this same
    // address, which is valid as an entry point whether or not it's been
    // patched yet (the patch redirects to a stub that still ends up
    // running the real logic for any non-reserved id).
    GET_MESSAGE_ADDR.store(get_message as usize, Ordering::Relaxed);

    let stub_body = build_stub(get_message, jump1_target, jump2_target);

    let stub = unsafe { VirtualAlloc(std::ptr::null_mut(), stub_body.len(), MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE) };
    if stub.is_null() {
        logger::error("GraceMenu: VirtualAlloc failed for the get_message hook stub, custom menu text disabled.");
        return false;
    }
    unsafe { std::ptr::copy_nonoverlapping(stub_body.as_ptr(), stub as *mut u8, stub_body.len()) };

    let mut patch = [0x90u8; PROLOGUE_LEN]; // NOP-fill the remainder
    patch[0] = 0x49;
    patch[1] = 0xBB; // mov r11, imm64
    patch[2..10].copy_from_slice(&(stub as u64).to_le_bytes());
    patch[10] = 0x41;
    patch[11] = 0xFF;
    patch[12] = 0xE3; // jmp r11

    let mut old_protect: u32 = 0;
    let ok = unsafe { VirtualProtect(get_message as *mut std::ffi::c_void, PROLOGUE_LEN, PAGE_EXECUTE_READWRITE, &mut old_protect) };
    if ok == 0 {
        logger::error("GraceMenu: VirtualProtect failed on get_message, custom menu text disabled.");
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(patch.as_ptr(), get_message, PROLOGUE_LEN);
        VirtualProtect(get_message as *mut std::ffi::c_void, PROLOGUE_LEN, old_protect, &mut old_protect);
        FlushInstructionCache(GetCurrentProcess(), get_message as *const std::ffi::c_void, PROLOGUE_LEN);
        FlushInstructionCache(GetCurrentProcess(), stub as *const std::ffi::c_void, stub_body.len());
    }

    HOOK_INSTALLED.store(true, Ordering::Relaxed);
    logger::log(&format!("GraceMenu: hooked get_message at {get_message:p} (stub={stub:p}) - custom menu text active."));
    true
}
