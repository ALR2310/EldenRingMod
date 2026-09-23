//! Shared `CSTaskImp` acquisition and a panic-safe wrapper for recurring
//! per-frame tasks, resolving both without touching `fromsoftware-rs`'s
//! version-gated `eldenring::rva::get()` (see `task_hook`'s doc comment for
//! why that matters).
//!
//! Ported from `autoregen::regen`'s `wait_for_cs_task`/`run_recurring_safe`
//! (2026-09-11/12), which itself superseded an older pattern - waiting on
//! `eldenring::util::system::wait_for_system_init` then retrying
//! `CSTaskImp::wait_for_instance` past `SystemInitError::InvalidRva` - that
//! `sometweaks`/`risearcher` both still carry their own copy of as of this
//! writing. `CSTaskImp::instance()` (a name-based Dantelion2 singleton
//! lookup, not an RVA table) sidesteps that whole class of failure instead
//! of just retrying past it, so this only needs the simpler retry loop
//! below. `run_recurring_safe` here also takes a `tag` (as `sometweaks`/
//! `risearcher`'s own `task.rs` already did) for per-feature panic logging,
//! wider than `autoregen`'s single-feature version.

use std::time::{Duration, Instant};

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp};
use eldenring::fd4::FD4TaskData;
use fromsoftware_shared::FromStatic;

use crate::logger;
use crate::memscan::CachedAddr;

use crate::task_hook;

// `CSTaskImp` never moves once constructed - cached so a mod registering
// several tasks (e.g. `common::reload` + its own feature) waits for it
// once and logs "found" once, instead of every thread polling on its own.
static CS_TASK: CachedAddr = CachedAddr::new();

const WARN_AFTER: Duration = Duration::from_secs(15);

/// Polls for `CSTaskImp` (the game's per-frame task scheduler) via its
/// name-based singleton lookup, retrying every 500ms - warns once (not
/// spamming) if it takes longer than `WARN_AFTER` to show up, since that
/// usually means the game needs a mod update rather than just being slow to
/// start. Never gives up.
pub fn wait_for_cs_task() -> &'static CSTaskImp {
    let addr = CS_TASK.get_or_resolve(|| Some(poll_cs_task() as *const CSTaskImp as usize));
    // `poll_cs_task` never gives up, so the cache is always filled here.
    unsafe { &*(addr.expect("poll_cs_task never returns None") as *const CSTaskImp) }
}

fn poll_cs_task() -> &'static CSTaskImp {
    let start = Instant::now();
    let mut warned = false;
    loop {
        match unsafe { CSTaskImp::instance() } {
            Ok(instance) => {
                logger::log("CSTaskImp found.");
                return instance;
            }
            Err(_) => {
                if !warned && start.elapsed() >= WARN_AFTER {
                    warned = true;
                    logger::error(&format!(
                        "CSTaskImp not found after {}s - game may need a mod update, check Nexus.",
                        WARN_AFTER.as_secs()
                    ));
                } else {
                    logger::log("CSTaskImp not ready yet, retrying in 500ms...");
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

/// Registers `f` as a recurring task via [task_hook::run_recurring] (AOB,
/// not `rva::get()`), but catches any panic `f` raises for a given frame
/// instead of letting it unwind into the game's own call stack - `f` just
/// gets skipped for that one frame (`tag` identifies which feature in the
/// log). Returns `false` if [task_hook::run_recurring] itself failed to
/// install (its own AOB pattern not found). Requires `[profile.release]`'s
/// `panic = "abort"` to be off (see workspace `Cargo.toml`) - `catch_unwind`
/// cannot catch anything once a panic aborts the process outright.
pub fn run_recurring_safe<F>(cs_task: &'static CSTaskImp, tag: &'static str, group: CSTaskGroupIndex, mut f: F) -> bool
where
    F: FnMut(&FD4TaskData) + 'static + Send,
{
    let installed = task_hook::run_recurring(cs_task, group, move |data: &FD4TaskData| {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(data))).is_err() {
            logger::error(&format!("{tag}: tick panicked, skipped this frame."));
        }
    });
    if installed {
        logger::log(&format!("{tag}: task registered."));
    }
    installed
}
