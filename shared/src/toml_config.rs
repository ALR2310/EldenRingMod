//! TOML config for mods whose settings outgrow a flat ini - lists of rules,
//! per-entry tables (SpeedMultiplier's SpEffect rules, 2026-10-02). The ini
//! reader ([`crate::config`]) folds every `[Section]` into one namespace and
//! has no arrays, so that shape would need numbered keys (`Speed01`...).
//!
//! Each mod declares its settings as a serde struct `T`:
//! `#[serde(default, deny_unknown_fields)]` on every struct, with a
//! `Default` impl holding the same values as the mod's embedded template
//! (a unit test in the mod should assert `template == T::default()`).
//! - Missing keys fall back to `T::default()`, so an old file still loads.
//! - Unknown/misspelled keys and wrong types are an error *with the line*
//!   (toml's own message), not silently ignored.
//! - A file that fails to parse never replaces the config in use: at
//!   start-up the mod runs on `T::default()`, on reload it keeps the last
//!   good config.
//!
//! Like [`crate::config::load_or_create_default`], start-up writes the
//! template if the file doesn't exist, and otherwise adds any key the
//! template has but the user's file lacks (new keys after an update), with
//! the template's comments - via `toml_edit`, so the user's own comments,
//! values and layout are kept.
//!
//! The parsed config is an `Arc<T>` snapshot: [`TomlConfig::get`] once per
//! tick, then plain field reads (no string-keyed lookups per value).
//!
//! Logging and the reload hotkey still come from the ini-based `logger` /
//! `reload` by default; a TOML mod drives them with
//! [`crate::logger::set_enabled`] and [`crate::reload::run_with`].

use std::fs;
use std::sync::{Arc, RwLock};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use toml_edit::{DocumentMut, Item, Table};

pub struct TomlConfig<T> {
    path: String,
    current: RwLock<Arc<T>>,
}

/// What [`TomlConfig::load_or_create`] did, for the mod to log once its
/// logger is up (the logger's own on/off switch lives in this config).
pub struct LoadReport {
    /// The file didn't exist and the template was written.
    pub created: bool,
    /// Keys added from the template to an existing file.
    pub added_keys: usize,
    /// Parse error - the mod is running on `T::default()`.
    pub error: Option<String>,
}

impl<T> TomlConfig<T>
where
    T: DeserializeOwned + Default + Send + Sync,
{
    pub fn load_or_create(path: &str, template: &str) -> (Self, LoadReport) {
        let mut report = LoadReport {
            created: false,
            added_keys: 0,
            error: None,
        };
        match fs::read_to_string(path) {
            Ok(text) => {
                if let Some((merged, added)) = add_missing_keys(&text, template) {
                    if fs::write(path, merged).is_ok() {
                        report.added_keys = added;
                    }
                }
            }
            Err(_) => {
                report.created = fs::write(path, template).is_ok();
            }
        }

        let config = match read(path) {
            Ok(config) => config,
            Err(err) => {
                report.error = Some(err);
                T::default()
            }
        };
        (
            Self {
                path: path.to_string(),
                current: RwLock::new(Arc::new(config)),
            },
            report,
        )
    }

    /// The config in use - take it once per tick.
    pub fn get(&self) -> Arc<T> {
        self.current.read().unwrap().clone()
    }

    /// Re-reads the file. On error the previous config stays in use.
    pub fn reload(&self) -> Result<(), String> {
        let config = read(&self.path)?;
        *self.current.write().unwrap() = Arc::new(config);
        Ok(())
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

/// `#[serde(deserialize_with = "common::toml_config::key_name")]` for a
/// hotkey field read by [`crate::input::parse_virtual_key`]: accepts a
/// string (`"F5"`, `"0x74"`, `"116"`) or a bare number (`116`, the VK code
/// in decimal) - an ini took both, TOML would reject the number as the
/// wrong type.
pub fn key_name<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum KeyName {
        Name(String),
        Code(i64),
    }
    Ok(match KeyName::deserialize(deserializer)? {
        KeyName::Name(name) => name,
        KeyName::Code(code) => code.to_string(),
    })
}

fn read<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("can't read the file: {e}"))?;
    toml::from_str(&text).map_err(|e| e.to_string().trim_end().to_string())
}

/// `user` with every key/table from `template` that it lacks, and how many
/// values were added; `None` if nothing is missing or `user` isn't valid
/// TOML (left for the parse step to report).
fn add_missing_keys(user: &str, template: &str) -> Option<(String, usize)> {
    let mut doc: DocumentMut = user.parse().ok()?;
    let template: DocumentMut = template.parse().ok()?;
    let added = merge_table(doc.as_table_mut(), template.as_table());
    (added > 0).then(|| (doc.to_string(), added))
}

fn merge_table(user: &mut Table, template: &Table) -> usize {
    let mut added = 0;
    for (name, item) in template.iter() {
        match user.get_mut(name) {
            None => {
                let (key, _) = template.get_key_value(name).unwrap();
                added += count_values(item);
                user.insert_formatted(key, item.clone());
            }
            Some(existing) => {
                if let (Some(existing), Some(item)) = (existing.as_table_mut(), item.as_table()) {
                    added += merge_table(existing, item);
                }
            }
        }
    }
    added
}

fn count_values(item: &Item) -> usize {
    match item.as_table() {
        Some(table) => table.iter().map(|(_, item)| count_values(item)).sum(),
        None => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = "\
[General]
# Reload key
ReloadKey = \"F5\"

[Speed]
# all actions
PlayerAll = 1.0
# moving
PlayerMovement = 1.2

[Logging]
LogFile = true
";

    #[test]
    fn complete_file_is_left_alone() {
        assert!(add_missing_keys(TEMPLATE, TEMPLATE).is_none());
    }

    #[test]
    fn missing_key_is_added_with_its_comment_and_user_values_kept() {
        let user = "\
[General]
ReloadKey = \"F6\" # mine

[Speed]
PlayerAll = 2.0

[Logging]
LogFile = false
";
        let (merged, added) = add_missing_keys(user, TEMPLATE).unwrap();
        assert_eq!(added, 1);
        assert!(merged.contains("ReloadKey = \"F6\" # mine"));
        assert!(merged.contains("PlayerAll = 2.0"));
        assert!(merged.contains("# moving\nPlayerMovement = 1.2"));
        assert!(merged.contains("LogFile = false"));
    }

    #[test]
    fn missing_table_is_added() {
        let user = "[General]\nReloadKey = \"F5\"\n";
        let (merged, added) = add_missing_keys(user, TEMPLATE).unwrap();
        assert_eq!(added, 3);
        let doc: DocumentMut = merged.parse().unwrap();
        assert_eq!(doc["Speed"]["PlayerMovement"].as_float(), Some(1.2));
        assert_eq!(doc["Logging"]["LogFile"].as_bool(), Some(true));
    }

    #[test]
    fn invalid_user_file_is_not_touched() {
        assert!(add_missing_keys("[Speed\nPlayerAll = ", TEMPLATE).is_none());
    }
}
