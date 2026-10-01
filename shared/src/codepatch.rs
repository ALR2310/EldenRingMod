//! Generic inline-hook helper: redirects execution at `target_addr` to a
//! small stub of caller-supplied "body" bytes, then jumps back to right
//! after the overwritten original bytes. Ported from the C++ `CodePatch.cpp`/
//! `.h` used by RuneMultiplier and ReductionWeight's original versions.
//!
//! This is the "not enough room" fallback: it patches the target with a
//! 5-byte relative `E9 rel32` JMP (NOP-filling anything past that), which
//! means the generated stub must land within a signed 32-bit displacement of
//! the target - [alloc_near] searches nearby pages for that. When the patch
//! site has ~12 bytes of room instead, prefer a `mov reg, imm64; jmp reg`
//! absolute redirect (see `attack_hook.rs` in `autoregen`/`sometweaks`, or
//! `runemultiplier`'s `hook.rs`) - it needs no such search, since an absolute
//! jump reaches any 64-bit address regardless of where the stub ends up.

use std::ffi::c_void;

unsafe extern "system" {
    fn VirtualAlloc(lp_address: *mut c_void, dw_size: usize, fl_allocation_type: u32, fl_protect: u32) -> *mut c_void;
    fn VirtualFree(lp_address: *mut c_void, dw_size: usize, dw_free_type: u32) -> i32;
    fn VirtualProtect(lp_address: *mut c_void, dw_size: usize, fl_new_protect: u32, lpfl_old_protect: *mut u32) -> i32;
    fn FlushInstructionCache(h_process: *mut c_void, lp_base_address: *const c_void, dw_size: usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
}

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const MEM_RELEASE: u32 = 0x8000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

// rel32 JMP displacement must fit in a signed 32-bit int. Stay well clear of
// the edges (0x70000000 instead of 0x7FFFFFFF) so the search below never
// produces a stub that's technically in range but only barely.
const MAX_REL32_RANGE: isize = 0x7000_0000;
const SEARCH_STEP: usize = 0x1_0000;

/// Searches backward first, then forward, in page-aligned steps from `anchor`
/// for `size` bytes of `VirtualAlloc`-able executable memory - same strategy
/// the original C++ used (matching the reference cheat DLLs this technique
/// was reverse engineered from).
fn alloc_near(anchor: *mut u8, size: usize) -> Option<*mut u8> {
    let anchor_addr = anchor as isize;

    let mut addr = anchor_addr;
    while addr > anchor_addr - MAX_REL32_RANGE && addr > 0x10000 {
        let mem = unsafe { VirtualAlloc(addr as *mut c_void, size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE) };
        if !mem.is_null() {
            return Some(mem as *mut u8);
        }
        addr -= SEARCH_STEP as isize;
    }

    let mut addr = anchor_addr;
    while addr < anchor_addr + MAX_REL32_RANGE {
        let mem = unsafe { VirtualAlloc(addr as *mut c_void, size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE) };
        if !mem.is_null() {
            return Some(mem as *mut u8);
        }
        addr += SEARCH_STEP as isize;
    }

    None
}

/// Overwrites `patched.len()` bytes at `addr` with `patched` in place - no
/// jump/stub, for when the replacement instructions fit exactly in the
/// original byte window (unlike [install_jmp_hook], which redirects
/// because its replacement doesn't fit). Returns `false` (leaving the
/// original bytes untouched) if `VirtualProtect` fails.
pub unsafe fn overwrite_bytes(addr: *mut u8, patched: &[u8]) -> bool {
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;

    let mut old_protect: u32 = 0;
    let ok = unsafe { VirtualProtect(addr as *mut c_void, patched.len(), PAGE_EXECUTE_READWRITE, &mut old_protect) };
    if ok == 0 {
        return false;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(patched.as_ptr(), addr, patched.len());
        VirtualProtect(addr as *mut c_void, patched.len(), old_protect, &mut old_protect);
    }
    true
}

/// Allocates a stub near `target_addr`, copies `stub_body` into it, appends a
/// JMP back to `target_addr + original_len`, then overwrites `original_len`
/// bytes at `target_addr` with a JMP into the stub (NOP-padded if
/// `original_len > 5`).
///
/// `stub_body` must not itself jump/return - this appends the return jump.
/// `original_len` must be >= 5 (a relative JMP needs 5 bytes). Returns the
/// stub's address on success; the original bytes are left untouched on any
/// failure.
pub fn install_jmp_hook(target_addr: *mut u8, original_len: usize, stub_body: &[u8]) -> Option<*mut u8> {
    if original_len < 5 {
        return None;
    }

    let stub_size = stub_body.len() + 5; // +5 for the JMP back
    let stub = alloc_near(target_addr, stub_size)?;

    unsafe {
        std::ptr::copy_nonoverlapping(stub_body.as_ptr(), stub, stub_body.len());

        let jmp_back = stub.add(stub_body.len());
        let return_addr = target_addr.add(original_len);
        *jmp_back = 0xE9;
        let rel = (return_addr as isize - jmp_back as isize - 5) as i32;
        jmp_back.add(1).copy_from_nonoverlapping(rel.to_le_bytes().as_ptr(), 4);

        let mut old_protect: u32 = 0;
        if VirtualProtect(target_addr as *mut c_void, original_len, PAGE_EXECUTE_READWRITE, &mut old_protect) == 0 {
            VirtualFree(stub as *mut c_void, 0, MEM_RELEASE);
            return None;
        }

        std::ptr::write_bytes(target_addr, 0x90, original_len); // NOP-fill first
        *target_addr = 0xE9;
        let rel_in = (stub as isize - target_addr as isize - 5) as i32;
        target_addr.add(1).copy_from_nonoverlapping(rel_in.to_le_bytes().as_ptr(), 4);

        let mut unused: u32 = 0;
        VirtualProtect(target_addr as *mut c_void, original_len, old_protect, &mut unused);
        FlushInstructionCache(GetCurrentProcess(), target_addr as *const c_void, original_len);
        FlushInstructionCache(GetCurrentProcess(), stub as *const c_void, stub_size);
    }

    Some(stub)
}

/// Destination of the 5-byte `call rel32` (`E8`) or `jmp rel32` (`E9`) at
/// `site`, or `None` if `site` holds neither.
pub fn rel32_target(site: *const u8) -> Option<u64> {
    let opcode = unsafe { site.read() };
    if opcode != 0xE8 && opcode != 0xE9 {
        return None;
    }
    let rel = unsafe { (site.add(1) as *const i32).read_unaligned() };
    Some((site as u64 + 5).wrapping_add_signed(rel as i64))
}

/// Points the existing `call rel32` / `jmp rel32` at `site` to a new stub
/// holding `stub_body`, allocated near `site` - only the 4 displacement
/// bytes change; the opcode byte and every other byte of the game's code
/// stay as they were.
///
/// Prefer this over [install_jmp_hook] whenever the patch site is a call:
/// other DLL mods (Seamless Co-op among them) find their own patch sites by
/// byte signature and abort the game when one no longer matches, and those
/// signatures wildcard a call's displacement (`E8 ? ? ? ?`) but not its
/// opcode or the instructions around it. If another mod already redirected
/// the same call, [rel32_target] returns its stub, so the two chain.
///
/// `stub_body` runs exactly as the original destination would: entered by
/// the same `call` (or `jmp`), so it must end with its own `ret` / tail
/// jump - nothing is appended. Returns the stub's address on success; the
/// original bytes are left untouched on any failure.
pub fn redirect_rel32(site: *mut u8, stub_body: &[u8]) -> Option<*mut u8> {
    rel32_target(site)?;
    let stub = alloc_near(site, stub_body.len())?;
    unsafe {
        std::ptr::copy_nonoverlapping(stub_body.as_ptr(), stub, stub_body.len());
        let rel = stub as isize - site as isize - 5;
        if rel < i32::MIN as isize || rel > i32::MAX as isize {
            VirtualFree(stub as *mut c_void, 0, MEM_RELEASE);
            return None;
        }
        let mut old_protect: u32 = 0;
        if VirtualProtect(site.add(1) as *mut c_void, 4, PAGE_EXECUTE_READWRITE, &mut old_protect) == 0 {
            VirtualFree(stub as *mut c_void, 0, MEM_RELEASE);
            return None;
        }
        (site.add(1) as *mut i32).write_unaligned(rel as i32);
        let mut unused: u32 = 0;
        VirtualProtect(site.add(1) as *mut c_void, 4, old_protect, &mut unused);
        FlushInstructionCache(GetCurrentProcess(), site as *const c_void, 5);
        FlushInstructionCache(GetCurrentProcess(), stub as *const c_void, stub_body.len());
    }
    Some(stub)
}
