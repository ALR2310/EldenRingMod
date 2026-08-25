//! Shared `CSTaskImp` acquisition, used by every feature module that
//! registers a recurring task on the game's own `FrameBegin` task group.

use std::time::Duration;

use eldenring::cs::CSTaskImp;

use common::logger;

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
