//! Minimal flat INI reader: "key=value" per line, "#" or ";" starts a comment.
//! Section headers ("[Section]") are accepted but ignored - all keys share one
//! namespace. Generic over the caller's own default template: unlike a
//! per-mod copy of this file, this crate has no ini of its own to embed, so
//! every entry point below takes the mod's `include_str!`-embedded default as
//! a parameter instead of baking one in.

use std::collections::HashMap;
use std::fs;
use std::sync::{LazyLock, RwLock};

static VALUES: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn parse(content: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with(';')
            || trimmed.starts_with('[')
        {
            continue;
        }
        let Some(eq) = trimmed.find('=') else { continue };
        let key = trimmed[..eq].trim().to_string();
        let value = trimmed[eq + 1..].trim().to_string();
        values.insert(key, value);
    }
    values
}

/// Loads `ini_path` into the shared config map, replacing whatever was loaded
/// before. Silently leaves the previous values in place if the file can't be
/// read (e.g. transient lock during a hot reload).
pub fn load(ini_path: &str) {
    if let Ok(content) = fs::read_to_string(ini_path) {
        *VALUES.write().unwrap() = parse(&content);
    }
}

/// If `ini_path` doesn't exist yet, writes `default_ini` (the caller's
/// embedded default template) to it first (so a fresh install gets a real,
/// editable file instead of relying silently on in-code defaults). If it does
/// exist, merges in any keys `default_ini` has that the user's file is
/// missing (e.g. after updating the mod to a version with new keys) - see
/// [migrate]. Either way, loads the (possibly just-updated) file afterward.
///
/// Returns how many new keys were merged in, so the caller can log it once
/// its logger is up.
pub fn load_or_create_default(ini_path: &str, default_ini: &str) -> usize {
    let migrated = if fs::exists(ini_path).unwrap_or(false) {
        migrate(ini_path, default_ini)
    } else {
        let _ = fs::write(ini_path, default_ini);
        0
    };
    load(ini_path);
    migrated
}

/// Merges any key present in `default_ini` but missing from the user's
/// existing `ini_path` into that file - comments and layout come from the
/// template, but every key the user already has keeps their value untouched.
/// A no-op (no disk write) if the file already has every key the template
/// defines.
///
/// Keys the user's file has that the template no longer defines (renamed or
/// removed in a newer version) aren't silently discarded: they're moved to a
/// trailing `[Legacy]` block instead, so nothing the user configured
/// disappears without a trace.
fn migrate(ini_path: &str, default_ini: &str) -> usize {
    let Ok(existing_content) = fs::read_to_string(ini_path) else {
        return 0;
    };
    let existing_values = parse(&existing_content);
    let template_values = parse(default_ini);

    let missing_count = template_values
        .keys()
        .filter(|k| !existing_values.contains_key(*k))
        .count();
    if missing_count == 0 {
        return 0; // already has every key the current template defines
    }

    let mut merged = String::new();
    for line in default_ini.lines() {
        let trimmed = line.trim();
        let is_kv = !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with(';')
            && !trimmed.starts_with('[')
            && trimmed.contains('=');
        if is_kv {
            let key = trimmed[..trimmed.find('=').unwrap()].trim();
            if let Some(value) = existing_values.get(key) {
                merged.push_str(key);
                merged.push('=');
                merged.push_str(value);
                merged.push('\n');
                continue;
            }
        }
        merged.push_str(line);
        merged.push('\n');
    }

    let legacy: Vec<(&String, &String)> = existing_values
        .iter()
        .filter(|(k, _)| !template_values.contains_key(*k))
        .collect();
    if !legacy.is_empty() {
        merged.push_str(
            "\n[Legacy]\n\
             ; Keys below no longer exist in the shipped default ini template\n\
             ; (renamed or removed in a newer version) - kept here instead of being\n\
             ; silently discarded. Safe to delete once you've moved the value elsewhere.\n",
        );
        for (key, value) in legacy {
            merged.push_str(key);
            merged.push('=');
            merged.push_str(value);
            merged.push('\n');
        }
    }

    let _ = fs::write(ini_path, merged);
    missing_count
}

pub fn get_string(key: &str, default: &str) -> String {
    VALUES
        .read()
        .unwrap()
        .get(key)
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

pub fn get_int(key: &str, default: i32) -> i32 {
    VALUES
        .read()
        .unwrap()
        .get(key)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

pub fn get_double(key: &str, default: f64) -> f64 {
    VALUES
        .read()
        .unwrap()
        .get(key)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

pub fn get_bool(key: &str, default: bool) -> bool {
    VALUES
        .read()
        .unwrap()
        .get(key)
        .map(|v| v.trim().to_ascii_lowercase())
        .map(|v| match v.as_str() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            _ => default,
        })
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // A small stand-in template, not any real mod's ini - this crate has no
    // ini of its own, so the migrate() tests exercise it against a fixture
    // instead of a real `include_str!`-embedded default.
    const TEST_TEMPLATE: &str = "\
[General]
ReloadKey=F5

[Section]
Hp=0
Fp=0
";

    /// Unique-per-test scratch file under the OS temp dir, so tests running
    /// in parallel don't clobber each other. Best-effort cleanup on drop.
    struct TempIni(PathBuf);

    impl TempIni {
        fn new(name: &str, content: &str) -> Self {
            let path = std::env::temp_dir().join(format!("common_config_test_{name}.ini"));
            fs::write(&path, content).unwrap();
            Self(path)
        }

        fn path(&self) -> &str {
            self.0.to_str().unwrap()
        }

        fn read(&self) -> String {
            fs::read_to_string(&self.0).unwrap()
        }
    }

    impl Drop for TempIni {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn migrate_adds_missing_key_with_template_default() {
        let without_reload_key: String = TEST_TEMPLATE
            .lines()
            .filter(|l| !l.trim_start().starts_with("ReloadKey="))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(parse(&without_reload_key).get("ReloadKey").is_none());

        let ini = TempIni::new("adds_missing", &without_reload_key);
        let migrated = migrate(ini.path(), TEST_TEMPLATE);
        assert_eq!(migrated, 1);

        let merged = parse(&ini.read());
        assert_eq!(merged.get("ReloadKey").map(String::as_str), Some("F5"));
    }

    #[test]
    fn migrate_preserves_existing_customized_values() {
        let customized = TEST_TEMPLATE.replace("Hp=0", "Hp=25");
        let ini = TempIni::new("preserves_custom", &customized);

        // Nothing missing yet - migrate must be a no-op.
        assert_eq!(migrate(ini.path(), TEST_TEMPLATE), 0);
        assert_eq!(ini.read(), customized);

        // Now also drop a different key so migrate actually rewrites the
        // file, and confirm the customized value survives the rewrite.
        let with_gap: String = customized
            .lines()
            .filter(|l| !l.trim_start().starts_with("Fp="))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&ini.0, &with_gap).unwrap();

        let migrated = migrate(ini.path(), TEST_TEMPLATE);
        assert_eq!(migrated, 1);
        let merged = parse(&ini.read());
        assert_eq!(merged.get("Hp").map(String::as_str), Some("25"));
    }

    #[test]
    fn migrate_moves_unknown_keys_to_legacy_section_instead_of_dropping_them() {
        let with_gap: String = TEST_TEMPLATE
            .lines()
            .filter(|l| !l.trim_start().starts_with("ReloadKey="))
            .collect::<Vec<_>>()
            .join("\n");
        let with_stale_key = format!("{with_gap}\nSomeRemovedFeature.Flag=true\n");
        let ini = TempIni::new("legacy_section", &with_stale_key);

        migrate(ini.path(), TEST_TEMPLATE);

        let content = ini.read();
        assert!(content.contains("[Legacy]"));
        let merged = parse(&content);
        assert_eq!(
            merged.get("SomeRemovedFeature.Flag").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn migrate_is_a_noop_when_nothing_is_missing() {
        let ini = TempIni::new("noop", TEST_TEMPLATE);
        assert_eq!(migrate(ini.path(), TEST_TEMPLATE), 0);
        assert_eq!(ini.read(), TEST_TEMPLATE);
    }
}
