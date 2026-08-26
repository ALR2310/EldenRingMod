//! Shared `CSTaskImp` acquisition and a panic-safe wrapper for recurring
//! per-frame tasks, used by every feature module that registers a task on
//! the game's own `FrameBegin` task group.

use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

use eldenring::cs::CSTaskImp;
use eldenring::fd4::FD4TaskData;
use eldenring::util::system::wait_for_system_init;
use fromsoftware_shared::{Program, RecurringTaskHandle, SharedTaskImpExt};

use common::logger;

/// Waits for the earliest reliable "the game process is actually alive"
/// signal (`CSWindow`'s global hInstance, populated right after CRT init -
/// see the crate's own doc comment on this function), retrying past
/// `SystemInitError::InvalidRva`/`Timeout` instead of giving up - same
/// reasoning as the retry loop below, just one step earlier. Ported from
/// `.docs/UltimatePassiveRegeneration`'s own `wait_for_system_init_until_ready`
/// (2026-08-26) - this crate already ships the exact helper meant for this,
/// `sometweaks` just hadn't called it before, going straight for
/// `CSTaskImp` instead.
fn wait_for_system_init_until_ready(tag: &str) {
    let program = Program::current();
    loop {
        if wait_for_system_init(&program, Duration::from_secs(5)).is_ok() {
            return;
        }
        logger::log(&format!("{tag}: system not initialized yet, retrying..."));
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// `CSTaskImp::wait_for_instance` treats `SystemInitError::InvalidRva` as
/// immediately fatal and never retries it, even with `Duration::MAX` - it
/// only retries the `Null` case internally. `InvalidRva` fires whenever the
/// version-specific RVA lookup runs before the game executable has finished
/// unpacking/relocating (e.g. Arxan), a timing race against how early this
/// DLL's worker thread happens to start. Retrying here with a short delay
/// rides out that race instead of permanently disabling a feature for the
/// session on a one-off early poll (2026-08-24). `tag` only prefixes the
/// retry log line so it's clear which feature is waiting.
pub fn wait_for_cs_task(tag: &str) -> &'static CSTaskImp {
    wait_for_system_init_until_ready(tag);

    loop {
        match CSTaskImp::wait_for_instance(Duration::MAX) {
            Ok(instance) => return instance,
            Err(err) => {
                logger::log(&format!("{tag}: CSTaskImp not ready yet ({err:?}), retrying in 1s..."));
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

/// Registers `f` as a recurring task the same way `cs_task.run_recurring`
/// does, but catches any panic `f` raises for a given frame instead of
/// letting it unwind into the game's own call stack - `f` just gets
/// skipped for that one frame (`tag` identifies which feature in the log),
/// which is harmless for every tick-based feature in this crate (no
/// feature keeps state that's unsafe to leave stale for a single frame).
/// Ported from `.docs/UltimatePassiveRegeneration`'s own `catch_unwind`
/// wrapping around every per-frame read of live game memory (2026-08-26) -
/// unlike that mod, this is centralized here instead of duplicated in
/// every feature module. Requires `[profile.release]`'s `panic = "abort"`
/// to be turned off (see workspace `Cargo.toml`) - `catch_unwind` cannot
/// catch anything once a panic aborts the process outright.
pub fn run_recurring_safe<TIndex, F>(
    cs_task: &'static CSTaskImp,
    tag: &'static str,
    group: TIndex,
    mut f: F,
) -> RecurringTaskHandle<FD4TaskData>
where
    F: FnMut(&FD4TaskData) + 'static + Send,
    CSTaskImp: SharedTaskImpExt<TIndex, FD4TaskData>,
{
    cs_task.run_recurring(
        move |data: &FD4TaskData| {
            if panic::catch_unwind(AssertUnwindSafe(|| f(data))).is_err() {
                logger::log(&format!("{tag}: tick panicked, skipped this frame."));
            }
        },
        group,
    )
}
