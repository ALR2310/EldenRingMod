//! Resolves the game's global runtime heap allocator without touching
//! `eldenring::rva::get()`'s version-gated `runtime_heap_allocator` RVA -
//! same story as `task_hook.rs` (see its doc comment), for a different call
//! site: anything that needs `DLAllocator::runtime_heap_allocator()` to
//! build a `DLString` (e.g. an in-game announcement banner) hits the same
//! panic on any game version `fromsoftware-rs` wasn't last published for,
//! exactly like `register_task` did before `task_hook.rs` existed.
//!
//! Unlike `WorldChrMan`/`CSTaskImp`, this global isn't a Dantelion2-reflected
//! singleton (no `#[shared::singleton(...)]` on `DLAllocator`), so the
//! name-based reflection trick `task::wait_for_cs_task` uses doesn't apply -
//! resolving it needs a byte anchor into actual game code instead. That
//! global turns out to be read from hundreds of inlined call sites across
//! the executable (it's DLKR's one heap allocator, used practically
//! everywhere), but exactly one of them is a small, self-contained "read or
//! lazily construct" wrapper function with its own prologue/epilogue -
//! verified (2026-09-11) unique in `.text` and byte-identical (aside from
//! its own unrelated rip-relative operands) on both a 1.16.2 exe
//! (`eldenring` 0.14.0's RVA 0x4842d40) and a 1.17.0 exe (`fromsoftware-rs`
//! commit `acb2a19`'s RVA 0x4846dc0).

use std::sync::atomic::{AtomicBool, Ordering};

use eldenring::dlkr::DLAllocator;

use common::logger;
use common::memscan;

// Logged once (not every call - a caller may hit this on every reload-key
// press) so a healthy AOB match doesn't spam the log the way a failure
// legitimately should.
static LOGGED_SUCCESS: AtomicBool = AtomicBool::new(false);

// `sub rsp,0x28; call <ctor?>; mov rax,[rip+X] (the global we want);
// test rax,rax; jnz +0xc; call <lazy-init>; mov [rip+Y],rax;
// mov [rip+Z],rax; add rsp,0x28; ret` - the `mov rax,[rip+X]` load at
// pattern offset `LOAD_INSTR_OFFSET` is what actually reads
// `runtime_heap_allocator`; everything else just anchors the match to this
// one specific, otherwise-unremarkable getter.
const GETTER_PATTERN: &str = "48 83 ec 28 e8 ?? ?? ?? ?? 48 8b 05 ?? ?? ?? ?? 48 85 c0 75 0c e8 ?? ?? ?? ?? 48 89 05 ?? ?? ?? ?? 48 89 05 ?? ?? ?? ?? 48 83 c4 28 c3";
const LOAD_INSTR_OFFSET: usize = 9; // "48 8b 05 <disp32>" (7 bytes) starts here

/// Resolves `DLAllocator::runtime_heap_allocator()`'s global via AOB
/// instead of `rva::get()`. Returns `None` (logged) if the pattern isn't
/// found or the global is still null - callers should treat that like any
/// other AOB-based feature failing, not fatal.
pub fn runtime_heap_allocator() -> Option<&'static DLAllocator> {
    let Some(matched) = memscan::find_pattern_in_module(GETTER_PATTERN) else {
        logger::log("AllocHook: runtime_heap_allocator getter pattern not found.");
        return None;
    };

    unsafe {
        let load_instr = matched.add(LOAD_INSTR_OFFSET);
        // "48 8b 05" is 3 bytes, disp32 follows - the whole instruction is 7
        // bytes, rip-relative from the byte right after it.
        let disp32 = i32::from_le_bytes(*(load_instr.add(3) as *const [u8; 4]));
        let global_ptr_addr = load_instr.add(7).offset(disp32 as isize) as *const *const DLAllocator;
        let instance_ptr = *global_ptr_addr;
        if instance_ptr.is_null() {
            logger::log("AllocHook: runtime_heap_allocator resolved but its instance is still null.");
            return None;
        }
        if LOGGED_SUCCESS.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
            logger::log("Allocator AOB found.");
        }
        Some(&*instance_ptr)
    }
}
