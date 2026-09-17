//! Registers a per-frame task without touching `fromsoftware-rs`'s
//! version-gated `eldenring::rva::get()`.
//!
//! `fromsoftware_shared::task::SharedTaskImpExt::run_recurring` resolves the
//! game's real task-registration function through a per-game-version RVA
//! table (`rva::get().register_task`) - that table only ever contains the ONE
//! exact version string the crate was last published for, and panics hard on
//! any other (see AutoRegen's README, "AOB thay `rva::get()` cho tick đăng
//! ký" for the full story of why this exists: testing a mod against a newer
//! game patch than the pinned `fromsoftware-rs` release panicked immediately
//! at startup instead of just this one feature failing).
//!
//! Verified (2026-09-09) that the actual game function `register_task`
//! resolves to is byte-identical, aside from its `call`/`lea` rip-relative
//! operands (which necessarily shift with relinking), across a 1.16.2 exe
//! (`eldenring` 0.14.0's RVA 0xeb1fe0) and a 1.17.0 exe (`fromsoftware-rs`
//! commit `acb2a19`'s RVA 0xeb3de0) - a byte pattern anchored on everything
//! BUT those operands is unique in `.text` on both. Scanning for that
//! pattern with `common::memscan` (same technique as AutoRegen's `hit_hook`)
//! finds the function regardless of which exact version shifted it,
//! sidestepping `rva::get()`'s hard version gate entirely for this one call
//! site.
//!
//! The actual registration call still needs to hand the game a pointer that
//! *looks* like one of its own native task objects (a C++ vtable at offset
//! 0, dispatched to when the task group runs) - `fromsoftware_shared::task`
//! already implements exactly this trick for the officially-resolved
//! address, but its `RecurringTask`/`self_ref` bookkeeping is private to
//! that crate, so it can't be reused for a different address. `Task` below
//! is a second, independent copy of the same trick (a fake vtable built with
//! the same `vtable-rs` crate `fromsoftware-shared` itself uses) built
//! specifically to be paired with the AOB-resolved address instead.

use std::ffi::c_void;
use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp};
use eldenring::fd4::FD4TaskData;
use vtable_rs::VPtr;

use common::logger;
use common::memscan;

const REGISTER_TASK_PATTERN: &str = "48 89 5c 24 08 57 48 83 ec 40 48 8d 4c 24 20 49 8b d8 8b fa e8 ?? ?? ?? ?? 48 8b 0d ?? ?? ?? ?? 48 85 c9 75 2e 48 8d 0d ?? ?? ?? ?? e8 ?? ?? ?? ?? 4c 8b c8 4c 8d 05 ?? ?? ?? ?? ba b4 00 00 00 48 8d 0d ?? ?? ?? ?? e8 ?? ?? ?? ?? 48 8b 0d";

#[vtable_rs::vtable]
trait TaskVmt {
    fn get_runtime_class(&self) -> usize;
    fn destructor(&mut self);
    fn execute(&mut self, data: *const c_void);
}

/// Layout only needs to satisfy what the game itself reads: a vtable pointer
/// at offset 0. Everything past that (here, just the closure) is opaque to
/// the game - it's only ever touched through `execute` below - so unlike
/// `fromsoftware_shared::task::RecurringTask` this doesn't need to mirror
/// that struct's exact field layout, just the same vtable trick.
#[repr(C)]
struct Task {
    vftable: VPtr<dyn TaskVmt, Self>,
    closure: Box<dyn FnMut(&FD4TaskData) + Send>,
}

impl TaskVmt for Task {
    // Both left `unimplemented!()` (panicking) until 2026-09-12 - suspected
    // (not yet confirmed reproduced) cause of a stutter reported after
    // AutoRegen 2.5.0's release: if `CSTaskImp` ever calls either of these
    // directly (e.g. a periodic task-list maintenance/enumeration pass) - as
    // opposed to `execute`, which only ever runs through a caller's own
    // `catch_unwind` (see `task::run_recurring_safe`) - the panic would
    // unwind straight into the game's own (non-Rust) call frames with no
    // landing pad, which is undefined behavior. Neither is safety-critical
    // for a task that's never actually inspected or torn down (`Box::leak`'d,
    // lives for the DLL's lifetime), so both are now harmless no-ops instead.
    extern "C" fn get_runtime_class(&self) -> usize {
        0
    }

    extern "C" fn destructor(&mut self) {}

    extern "C" fn execute(&mut self, data: *const c_void) {
        (self.closure)(unsafe { &*(data as *const FD4TaskData) });
    }
}

/// Registers `closure` on `group`, the same effect as
/// `CSTaskImp::run_recurring` but resolving the game's task-registration
/// function via AOB (see module doc). Returns `false` (closure never
/// installed, logged) if the pattern can't be found within 60s - callers
/// should treat that the same as any other AOB-based feature failing to find
/// its anchor.
///
/// The registered task is never unregistered - it's meant for features that
/// park their thread forever and never drop a handle - it lives for the
/// DLL's lifetime, so leaking its allocation here changes nothing
/// observable.
pub fn run_recurring<F>(cs_task: &'static CSTaskImp, group: CSTaskGroupIndex, closure: F) -> bool
where
    F: FnMut(&FD4TaskData) + 'static + Send,
{
    let Some(register_task_addr) = memscan::wait_for_pattern_in_module(
        REGISTER_TASK_PATTERN,
        Duration::from_millis(500),
        Duration::from_secs(60),
    ) else {
        logger::error("Could not locate the game's task-registration function (AOB pattern not found) - task NOT installed.");
        return false;
    };
    logger::log("register_task AOB found.");

    let task: &'static Task = Box::leak(Box::new(Task {
        vftable: Default::default(),
        closure: Box::new(closure),
    }));

    let register_task: extern "C" fn(&'static CSTaskImp, CSTaskGroupIndex, &'static Task) =
        unsafe { std::mem::transmute(register_task_addr) };
    register_task(cs_task, group, task);
    true
}
