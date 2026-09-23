//! Per-mod log file, gated entirely by the ini's `[Logging] LogFile` key
//! (2026-09-23): `LogFile=false` means no file is created or written at
//! all, `LogFile=true` creates it (truncating any previous run's log) on
//! the first line written. Checked on every write, so flipping the key with
//! `ReloadKey` takes effect immediately - turning it on mid-session starts
//! the file then, turning it off stops writing.
//!
//! Before this, every mod except RiseArcher/RuneMultiplier called `init`
//! unconditionally, so the file was always created and `LogFile` only gated
//! a mod's own verbose lines - not what the key's name says. A mod that
//! wants a log out of the box ships with `LogFile=true` as its default.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::sync::{LazyLock, Mutex, Once};

use crate::config;

struct LogState {
    /// Set by [init]; `None` means `init` hasn't run yet.
    path: Option<String>,
    /// Opened lazily by the first write with `LogFile` on.
    file: Option<File>,
}

static LOG: LazyLock<Mutex<LogState>> = LazyLock::new(|| Mutex::new(LogState { path: None, file: None }));
static PANIC_HOOK_INSTALLED: Once = Once::new();

/// Records where the log goes (`file_name`, e.g. "AutoRegen.log", in
/// `log_dir`) without touching the disk - the file is only created by the
/// first line written while `LogFile` is on (see module doc). Call after
/// the ini is loaded. No-op if already initialized.
pub fn init(log_dir: &str, file_name: &str) {
    let mut guard = LOG.lock().unwrap();
    if guard.path.is_none() {
        guard.path = Some(format!("{log_dir}\\{file_name}"));
    }
}

/// Creates (truncating) and opens the log file at `path`.
fn open(path: &str) -> Option<File> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = fs::create_dir_all(dir);
    }

    // Truncate any previous run's log first, as a separate open/close from
    // the handle kept below - `File::create` always starts the file at 0
    // bytes.
    let _ = File::create(path);

    // Kept open in *append* mode rather than a plain write handle: append
    // mode re-resolves the true end-of-file at every write, so if something
    // else (e.g. the user clearing the file's content in an editor) changes
    // the file size while this handle is open, the next log line still lands
    // at the real end of the file instead of at this handle's old cached
    // write position (which, on a plain write handle, gets zero-padded up to
    // the stale offset).
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Writes one line as `[timestamp] [LEVEL] message`. `level` is padded to a
/// fixed 5 characters so every line's message column starts at the same
/// offset, which is what makes a log skimmable at a glance (2026-08-28).
fn write_line(level: &str, message: &str) {
    // Read before taking the log lock - never hold both at once.
    if !config::get_bool("LogFile", false) {
        return;
    }
    let mut guard = LOG.lock().unwrap();
    if guard.file.is_none() {
        let Some(path) = guard.path.clone() else {
            return;
        };
        guard.file = open(&path);
    }
    if let Some(file) = guard.file.as_mut() {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(file, "[{now}] [{level:<5}] {message}");
    }
}

/// Plain informational line. Kept as `log` (rather than renamed to `info`)
/// so every existing call site across all mod crates keeps working - INFO is
/// the right level for the overwhelming majority of them.
pub fn log(message: &str) {
    write_line("INFO", message);
}

/// Something unexpected that the mod recovered from or is still retrying
/// past - the feature is not (yet) lost.
pub fn warn(message: &str) {
    write_line("WARN", message);
}

/// A feature failed and stays disabled for the rest of the session. Do not
/// repeat the word "ERROR" inside `message` - the level column already says
/// it.
pub fn error(message: &str) {
    write_line("ERROR", message);
}

/// Verbose detail only useful when diagnosing a specific feature (byte
/// dumps, per-tick values). Call sites are expected to already be behind
/// their own ini flag (DebugLog/RegenLog/...).
pub fn debug(message: &str) {
    write_line("DEBUG", message);
}

/// Installs a process-wide panic hook that logs an ERROR line (thread name,
/// source location, panic message) before falling through to the previous
/// hook. Without this, a panic on any of the background threads these mods
/// spawn (the `std::thread::spawn` in `DllMain`, hudhook's render/input
/// hook threads) unwinds silently under this workspace's `panic = "unwind"`
/// profile - the thread just dies and the mod's log stops dead with no
/// error line, which is exactly how a fromsoftware-rs struct offset going
/// stale after a game update has bitten this workspace before (see
/// SoloParamRepository's `.expect()` panics). Call once, as early as
/// possible in `DllMain`, right after `init`. Safe to call more than once
/// (e.g. from a hot-reload path) - only the first call installs the hook.
pub fn install_panic_hook() {
    PANIC_HOOK_INSTALLED.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let thread = std::thread::current();
            let thread_name = thread.name().unwrap_or("<unnamed>");
            let location = info
                .location()
                .map(|l| l.to_string())
                .unwrap_or_else(|| "<unknown location>".to_string());
            let payload = info.payload();
            let message = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic payload>".to_string()
            };
            error(&format!("PANIC on thread '{thread_name}' at {location}: {message}"));
            default_hook(info);
        }));
    });
}
