use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::sync::{LazyLock, Mutex};

static LOG_FILE: LazyLock<Mutex<Option<File>>> = LazyLock::new(|| Mutex::new(None));

/// Opens `file_name` (e.g. "AutoRegen.log") in `log_dir`, truncating any
/// previous run's log. No-op if already initialized (safe to call again
/// after a hot reload flips a mod's own DebugLog on/off).
pub fn init(log_dir: &str, file_name: &str) {
    let mut guard = LOG_FILE.lock().unwrap();
    if guard.is_some() {
        return;
    }
    let _ = fs::create_dir_all(log_dir);
    let path = format!("{log_dir}\\{file_name}");

    // Truncate any previous run's log first, as a separate open/close from
    // the handle kept below - `File::create` always starts the file at 0
    // bytes.
    let _ = File::create(&path);

    // Kept open in *append* mode rather than a plain write handle: append
    // mode re-resolves the true end-of-file at every write, so if something
    // else (e.g. the user clearing the file's content in an editor) changes
    // the file size while this handle is open, the next log line still lands
    // at the real end of the file instead of at this handle's old cached
    // write position (which, on a plain write handle, gets zero-padded up to
    // the stale offset).
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
        *guard = Some(file);
    }
}

/// Writes one line as `[timestamp] [LEVEL] message`. `level` is padded to a
/// fixed 5 characters so every line's message column starts at the same
/// offset, which is what makes a log skimmable at a glance (2026-08-28).
fn write_line(level: &str, message: &str) {
    let mut guard = LOG_FILE.lock().unwrap();
    if let Some(file) = guard.as_mut() {
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
