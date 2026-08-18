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

pub fn log(message: &str) {
    let mut guard = LOG_FILE.lock().unwrap();
    if let Some(file) = guard.as_mut() {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(file, "[{now}] {message}");
    }
}
