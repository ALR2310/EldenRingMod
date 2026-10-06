//! TOML config for mods whose settings outgrow a flat ini - lists of rules,
//! per-entry tables (SpeedMultiplier's SpEffect rules, 2026-10-02). The ini
//! reader ([`crate::config`]) folds every `[Section]` into one namespace and
//! has no arrays, so that shape would need numbered keys (`Speed01`...).
//!
//! Each mod declares its settings as a serde struct `T`:
//! `#[derive(Default)]` + `#[serde(default, deny_unknown_fields)]` on every
//! struct. **The defaults live only in the mod's embedded template**
//! (2026-10-02 - before, every value was written twice, in the template
//! and in a hand-written `Default`, kept equal by a test). A file is read
//! in two passes ([parse]): the user's text alone, for errors with their
//! line (missing keys get the derived zero values there); then the
//! user's values laid over the template's, for the real config - so a
//! missing key gets its template value. `T::default()` (zeros) is never
//! a config in use; [defaults] is the template's. A mod test should
//! assert [template_missing_keys] is empty, or a field the template lacks
//! would silently be zero.
//! - Missing keys fall back to the template, so an old file still loads.
//! - Unknown/misspelled keys and wrong types are an error *with the line*
//!   (toml's own message), not silently ignored - on reload, while the user
//!   is editing. At start-up they are moved out first (below).
//! - A file that fails to parse never replaces the config in use: at
//!   start-up the mod runs on the template's values, on reload it keeps
//!   the last good config.
//!
//! Versioned migrations (2026-10-02, replacing a flat rename list - it
//! depended on its own order, couldn't split or convert a key, and a
//! dropped entry silently lost a value). The file carries a format version
//! at [`Migration::version_key`] (e.g. `General.ConfigVersion`); the
//! template's value there is the current version. A file without one is
//! version 1, the first released format. [`Migration::steps`]`[i]` takes a
//! file from version `i + 1` to `i + 2`, so each step runs exactly once per
//! file, in order. **A released step must never change** - files already
//! past it would not rerun it; fix mistakes with a new step.
//!
//! Start-up brings an existing file in line with the template, via
//! `toml_edit` so the user's own comments, values and layout are kept:
//! 1. The migration steps the file hasn't run yet, then its version is set
//!    to the current one.
//! 2. Keys and tables the template has but the file lacks are added, with
//!    the template's comments (new keys after an update).
//! 3. Keys the template doesn't have are taken out and written back as
//!    comments in a block at the end of the file - the ini's `[Legacy]`,
//!    but a comment so the file still parses. Top-level names in
//!    [`Migration::keep`] are left alone (lists the template has no entry
//!    for, e.g. `[[Override]]`).
//! A file from a *newer* version (the mod was downgraded) is not touched
//! at all - step 3 would otherwise strip the newer keys. A file that isn't
//! valid TOML is left untouched (the parse step reports it). No file -> the
//! template is written.
//!
//! The parsed config is an `Arc<T>` snapshot: [`TomlConfig::get`] once per
//! tick, then plain field reads (no string-keyed lookups per value).
//!
//! Logging and the reload hotkey still come from the ini-based `logger` /
//! `reload` by default; a TOML mod drives them with
//! [`crate::logger::set_enabled`] and [`crate::reload::run_with`].

use std::fs;
use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, Table};

pub struct TomlConfig<T> {
    path: String,
    template: String,
    current: RwLock<Arc<T>>,
}

/// One migration step: changes a file of the previous format version into
/// the next (the template is there for its comments, see [rename_key]).
pub type Step = fn(&mut DocumentMut, &DocumentMut);

/// How an existing file is brought in line with the template, see the
/// module doc.
#[derive(Clone, Copy)]
pub struct Migration<'a> {
    /// Dotted path of the format version (`"General.ConfigVersion"`). The
    /// template must hold the current version there.
    pub version_key: &'a str,
    /// `steps[i]`: version `i + 1` -> `i + 2`. Append only.
    pub steps: &'a [Step],
    /// Top-level names never treated as unknown (`"Override"`).
    pub keep: &'a [&'a str],
}

/// What [`TomlConfig::load_or_create`] did, for the mod to log once its
/// logger is up (the logger's own on/off switch lives in this config).
#[derive(Default, Debug)]
pub struct LoadReport {
    /// The file didn't exist and the template was written.
    pub created: bool,
    /// `(from, to)` format versions, when migration steps ran.
    pub migrated: Option<(u32, u32)>,
    /// The file is from a newer format version than this mod knows - it was
    /// left untouched. `(file, current)`.
    pub newer: Option<(u32, u32)>,
    /// Keys added from the template to an existing file.
    pub added_keys: usize,
    /// Keys the template doesn't have, moved to the comment block at the end
    /// of the file (`"Speed.PlayerMovement = 1.2"`).
    pub unknown: Vec<String>,
    /// Parse error - the mod is running on the template's values.
    pub error: Option<String>,
}

impl<T> TomlConfig<T>
where
    T: DeserializeOwned + Send + Sync,
{
    pub fn load_or_create(path: &str, template: &str, migration: Migration) -> (Self, LoadReport) {
        let mut report = LoadReport::default();
        match fs::read_to_string(path) {
            Ok(text) => {
                let sync = sync_with_template(&text, template, migration);
                report.newer = sync.newer;
                if let Some(new_text) = &sync.text {
                    if fs::write(path, new_text).is_ok() {
                        report.migrated = sync.migrated;
                        report.added_keys = sync.added;
                        report.unknown = sync.unknown;
                    }
                }
            }
            Err(_) => {
                report.created = fs::write(path, template).is_ok();
            }
        }

        let config = match read(path, template) {
            Ok(config) => config,
            Err(err) => {
                report.error = Some(err);
                defaults(template)
            }
        };
        (
            Self {
                path: path.to_string(),
                template: template.to_string(),
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
        let config = read(&self.path, &self.template)?;
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

fn read<T: DeserializeOwned>(path: &str, template: &str) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("can't read the file: {e}"))?;
    parse(&text, template)
}

fn toml_error(e: impl std::fmt::Display) -> String {
    e.to_string().trim_end().to_string()
}

/// `text` read as a `T` the way a config file is: errors (unknown keys,
/// wrong types, syntax) reported with their line from `text` alone, then
/// the values taken with every key `text` lacks filled from `template`.
pub fn parse<T: DeserializeOwned>(text: &str, template: &str) -> Result<T, String> {
    toml::from_str::<T>(text).map_err(toml_error)?;
    let mut merged: toml::Table = toml::from_str(template).map_err(|e| format!("template: {}", toml_error(e)))?;
    let user: toml::Table = toml::from_str(text).map_err(toml_error)?;
    lay_over(&mut merged, user);
    T::deserialize(toml::Value::Table(merged)).map_err(toml_error)
}

/// `user`'s values over `base`: tables merge key by key, anything else
/// (values, arrays, `[[lists]]`) is replaced whole.
fn lay_over(base: &mut toml::Table, user: toml::Table) {
    for (key, value) in user {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(base_table)), toml::Value::Table(user_table)) => lay_over(base_table, user_table),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

/// The template's values - the defaults. Panics if the template doesn't
/// parse (a mod test parses it, so that's a build-time mistake).
pub fn defaults<T: DeserializeOwned>(template: &str) -> T {
    toml::from_str(template).expect("the embedded template parses")
}

/// Dotted paths of every field `T` has but `template` doesn't set -
/// which [parse] would leave at the derived zero value. Names in `keep`
/// (lists like `[[Override]]`) are skipped. For a mod's unit test.
pub fn template_missing_keys<T: DeserializeOwned + Serialize>(template: &str, keep: &[&str]) -> Vec<String> {
    let fields = toml::Table::try_from(defaults::<T>(template)).expect("config serializes");
    let present: toml::Table = toml::from_str(template).expect("the embedded template parses");
    let mut missing = Vec::new();
    missing_keys(&fields, &present, "", keep, &mut missing);
    missing
}

fn missing_keys(fields: &toml::Table, present: &toml::Table, prefix: &str, keep: &[&str], out: &mut Vec<String>) {
    for (key, value) in fields {
        if prefix.is_empty() && keep.contains(&key.as_str()) {
            continue;
        }
        let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
        match (value, present.get(key)) {
            (_, None) => out.push(path),
            (toml::Value::Table(f), Some(toml::Value::Table(p))) => missing_keys(f, p, &path, keep, out),
            _ => {}
        }
    }
}

const UNKNOWN_HEADER: &str = "\n# ---- Keys this version doesn't use (renamed or removed) - kept here as\n# comments so nothing you set is lost. Safe to delete. ----\n";

#[derive(Default)]
struct Synced {
    /// The new file text, `None` if nothing changed.
    text: Option<String>,
    migrated: Option<(u32, u32)>,
    newer: Option<(u32, u32)>,
    added: usize,
    unknown: Vec<String>,
}

fn version_at(doc: &DocumentMut, path: &str) -> Option<u32> {
    let (parent, key) = split_path(path);
    get_table(doc.as_table(), &parent)?
        .get(key)?
        .as_integer()
        .and_then(|v| u32::try_from(v).ok())
}

/// `user` brought in line with `template` (see the module doc). A `user`
/// that isn't valid TOML is left as it is.
fn sync_with_template(user: &str, template: &str, migration: Migration) -> Synced {
    let mut out = Synced::default();
    let (Ok(mut doc), Ok(template)) = (user.parse::<DocumentMut>(), template.parse::<DocumentMut>()) else {
        return out;
    };
    let current = version_at(&template, migration.version_key).unwrap_or(1);
    let file_version = version_at(&doc, migration.version_key).unwrap_or(1);
    if file_version > current {
        out.newer = Some((file_version, current));
        return out;
    }

    let mut changed = false;
    if file_version < current {
        for step in migration.steps.iter().skip(file_version as usize - 1).take((current - file_version) as usize) {
            step(&mut doc, &template);
        }
        out.migrated = Some((file_version, current));
        changed = true;
    }
    // Write the version whenever the file lacks it or is behind - merge
    // below would otherwise add the template's (current) value without the
    // steps having run... which they now have.
    if version_at(&doc, migration.version_key) != Some(current) {
        set_value(&mut doc, &template, migration.version_key, current as i64);
        changed = true;
    }

    out.added = merge_table(doc.as_table_mut(), template.as_table());
    take_unknown(doc.as_table_mut(), template.as_table(), "", migration.keep, &mut out.unknown);
    if !changed && out.added == 0 && out.unknown.is_empty() {
        return out;
    }

    let mut text = doc.to_string();
    if !out.unknown.is_empty() {
        if !text.contains(UNKNOWN_HEADER.trim_start()) {
            text = text.trim_end().to_string() + "\n" + UNKNOWN_HEADER;
        }
        for line in &out.unknown {
            text.push_str("# ");
            text.push_str(line);
            text.push('\n');
        }
    }
    out.text = Some(text);
    out
}

/// For migration steps: moves the value at `old` to `new` (dotted paths),
/// creating `new`'s tables (with the template's comments) as needed, and
/// keeping the template's comment above the key. Does nothing if `old`
/// isn't a plain value or `new` is already set. Returns whether it moved.
pub fn rename_key(doc: &mut DocumentMut, template: &DocumentMut, old: &str, new: &str) -> bool {
    let (old_parent, old_key) = split_path(old);
    let (new_parent, new_key) = split_path(new);
    if get_table(doc.as_table(), &new_parent).is_some_and(|t| t.contains_key(new_key)) {
        return false;
    }
    let Some(old_table) = get_table_mut(doc.as_table_mut(), &old_parent) else {
        return false;
    };
    if !old_table.get(old_key).is_some_and(Item::is_value) {
        return false;
    }
    let item = old_table.remove(old_key).unwrap();
    let toml_edit::Item::Value(v) = item else { return false };
    set_value(doc, template, new, v);
    true
}

/// For migration steps: the plain value at `path`, if any.
pub fn get_value<'a>(doc: &'a DocumentMut, path: &str) -> Option<&'a toml_edit::Value> {
    let (parent, key) = split_path(path);
    get_table(doc.as_table(), &parent)?.get(key)?.as_value()
}

/// For migration steps: removes the value at `path`, returning it.
pub fn remove_value(doc: &mut DocumentMut, path: &str) -> Option<toml_edit::Value> {
    let (parent, key) = split_path(path);
    let table = get_table_mut(doc.as_table_mut(), &parent)?;
    if !table.get(key).is_some_and(Item::is_value) {
        return None;
    }
    table.remove(key)?.into_value().ok()
}

/// For migration steps: sets `path` to `v`, creating its tables (with the
/// template's comments) as needed and keeping the template's comment above
/// the key when the key is new.
pub fn set_value(doc: &mut DocumentMut, template: &DocumentMut, path: &str, v: impl Into<toml_edit::Value>) {
    let (parent, key) = split_path(path);
    let mut table = doc.as_table_mut();
    let mut template_table = Some(template.as_table());
    for part in &parent {
        let from_template = template_table.and_then(|t| t.get(part)).and_then(Item::as_table);
        if !table.contains_key(part) {
            let mut fresh = Table::new();
            if let Some(t) = from_template {
                fresh.decor_mut().clone_from(t.decor());
            }
            table.insert(part, Item::Table(fresh));
        }
        template_table = from_template;
        table = table.get_mut(part).and_then(Item::as_table_mut).unwrap();
    }
    let v: toml_edit::Value = v.into();
    if let Some(existing) = table.get_mut(key).and_then(Item::as_value_mut) {
        // Keep the user's inline comment / spacing around the value.
        let decor = existing.decor().clone();
        *existing = v;
        *existing.decor_mut() = decor;
        return;
    }
    match template_table.and_then(|t| t.get_key_value(key)) {
        Some((template_key, template_item)) => {
            let mut v = v;
            if let Some(template_value) = template_item.as_value() {
                *v.decor_mut() = template_value.decor().clone();
            }
            table.insert_formatted(template_key, Item::Value(v));
        }
        None => {
            table.insert(key, Item::Value(v));
        }
    }
}

fn split_path(path: &str) -> (Vec<&str>, &str) {
    let mut parts: Vec<&str> = path.split('.').collect();
    let key = parts.pop().unwrap_or_default();
    (parts, key)
}

fn get_table<'a>(mut table: &'a Table, path: &[&str]) -> Option<&'a Table> {
    for part in path {
        table = table.get(part)?.as_table()?;
    }
    Some(table)
}

fn get_table_mut<'a>(mut table: &'a mut Table, path: &[&str]) -> Option<&'a mut Table> {
    for part in path {
        table = table.get_mut(part)?.as_table_mut()?;
    }
    Some(table)
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

/// Removes every key of `user` that `template` doesn't have, adding a
/// `path = value` line per removed value to `out`.
fn take_unknown(user: &mut Table, template: &Table, prefix: &str, keep: &[&str], out: &mut Vec<String>) {
    let names: Vec<String> = user.iter().map(|(name, _)| name.to_string()).collect();
    for name in names {
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}.{name}") };
        if prefix.is_empty() && keep.contains(&name.as_str()) {
            continue;
        }
        match template.get(&name) {
            Some(template_item) => {
                if let (Some(user_table), Some(template_table)) =
                    (user.get_mut(&name).and_then(Item::as_table_mut), template_item.as_table())
                {
                    take_unknown(user_table, template_table, &path, keep, out);
                }
            }
            None => {
                if let Some(item) = user.remove(&name) {
                    describe(&item, &path, out);
                }
            }
        }
    }
}

/// `path = value` lines for a removed item (one per value in a table).
fn describe(item: &Item, path: &str, out: &mut Vec<String>) {
    match item {
        Item::Value(value) => out.push(format!("{path} = {}", value.to_string().trim())),
        Item::Table(table) => {
            for (name, item) in table.iter() {
                describe(item, &format!("{path}.{name}"), out);
            }
        }
        Item::ArrayOfTables(array) => {
            for table in array.iter() {
                out.push(format!("[[{path}]]"));
                for (name, item) in table.iter() {
                    describe(item, name, out);
                }
            }
        }
        Item::None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Format version 3 of a made-up mod: v1 had `[Speed] PlayerRun`,
    /// v2 renamed it to `[Player] Run`, v3 split `Player.Move` into
    /// `Player.Walk` + `Player.Run`... see the steps below.
    const TEMPLATE: &str = "\
[General]
ConfigVersion = 3   # managed by the mod
# Reload key
ReloadKey = \"F5\"

[Player]
# all actions
All = 1.0
# walking
Walk = 1.2
# running
Run = 1.2

[Logging]
LogFile = true
";

    /// 1 -> 2: `Speed.PlayerAll` / `Speed.PlayerMove` moved under `[Player]`.
    fn step_1_to_2(doc: &mut DocumentMut, template: &DocumentMut) {
        rename_key(doc, template, "Speed.PlayerAll", "Player.All");
        rename_key(doc, template, "Speed.PlayerMove", "Player.Move");
    }

    /// 2 -> 3: `Player.Move` split into `Player.Walk` + `Player.Run`.
    fn step_2_to_3(doc: &mut DocumentMut, template: &DocumentMut) {
        if let Some(v) = remove_value(doc, "Player.Move") {
            set_value(doc, template, "Player.Walk", v.clone());
            set_value(doc, template, "Player.Run", v);
        }
    }

    const STEPS: &[Step] = &[step_1_to_2, step_2_to_3];

    fn migration() -> Migration<'static> {
        Migration {
            version_key: "General.ConfigVersion",
            steps: STEPS,
            keep: &["Override"],
        }
    }

    fn sync(user: &str) -> Synced {
        sync_with_template(user, TEMPLATE, migration())
    }

    fn doc(text: &str) -> DocumentMut {
        text.parse().unwrap()
    }

    #[test]
    fn complete_file_is_left_alone() {
        assert!(sync(TEMPLATE).text.is_none());
    }

    #[test]
    fn version_1_file_runs_every_step() {
        let s = sync("[Speed]\nPlayerAll = 2.0\nPlayerMove = 1.5\n");
        assert_eq!(s.migrated, Some((1, 3)));
        let d = doc(s.text.as_deref().unwrap());
        assert_eq!(d["General"]["ConfigVersion"].as_integer(), Some(3));
        assert_eq!(d["Player"]["All"].as_float(), Some(2.0));
        assert_eq!(d["Player"]["Walk"].as_float(), Some(1.5));
        assert_eq!(d["Player"]["Run"].as_float(), Some(1.5));
        assert!(s.unknown.is_empty(), "{:?}", s.unknown);
        assert!(sync(s.text.as_deref().unwrap()).text.is_none());
    }

    #[test]
    fn version_2_file_runs_only_the_last_step() {
        let s = sync("[General]\nConfigVersion = 2\n\n[Player]\nAll = 3.0\nMove = 0.5\n");
        assert_eq!(s.migrated, Some((2, 3)));
        let d = doc(s.text.as_deref().unwrap());
        assert_eq!(d["Player"]["All"].as_float(), Some(3.0));
        assert_eq!(d["Player"]["Walk"].as_float(), Some(0.5));
        assert_eq!(d["Player"]["Run"].as_float(), Some(0.5));
    }

    #[test]
    fn steps_never_rerun_on_a_current_file() {
        // v3 file with a stray old key: step 1->2 must not move it.
        let user = format!("{TEMPLATE}\n[Speed]\nPlayerAll = 9.0\n");
        let s = sync(&user);
        assert_eq!(s.migrated, None);
        let d = doc(s.text.as_deref().unwrap());
        assert_eq!(d["Player"]["All"].as_float(), Some(1.0));
        assert_eq!(s.unknown, vec!["Speed.PlayerAll = 9.0"]);
    }

    #[test]
    fn newer_file_is_not_touched() {
        let user = TEMPLATE.replace("ConfigVersion = 3", "ConfigVersion = 4") + "\n[Future]\nX = 1\n";
        let s = sync(&user);
        assert_eq!(s.newer, Some((4, 3)));
        assert!(s.text.is_none());
    }

    #[test]
    fn missing_key_is_added_with_its_comment_and_user_values_kept() {
        let user = "\
[General]
ConfigVersion = 3
ReloadKey = \"F6\" # mine

[Player]
All = 2.0
Walk = 1.0

[Logging]
LogFile = false
";
        let s = sync(user);
        assert_eq!(s.added, 1);
        let text = s.text.unwrap();
        assert!(text.contains("ReloadKey = \"F6\" # mine"));
        assert!(text.contains("All = 2.0"));
        assert!(text.contains("# running\nRun = 1.2"));
        assert!(text.contains("LogFile = false"));
    }

    #[test]
    fn missing_table_is_added() {
        let s = sync("[General]\nConfigVersion = 3\nReloadKey = \"F5\"\n");
        assert_eq!(s.added, 4);
        let d = doc(s.text.as_deref().unwrap());
        assert_eq!(d["Player"]["Run"].as_float(), Some(1.2));
        assert_eq!(d["Logging"]["LogFile"].as_bool(), Some(true));
    }

    #[test]
    fn invalid_user_file_is_not_touched() {
        assert!(sync("[Player\nAll = ").text.is_none());
    }

    #[test]
    fn unknown_keys_become_comments_at_the_end() {
        let user = format!("{TEMPLATE}\n[Old]\nA = 1\nB = \"x\"\n").replace("Run = 1.2", "Run = 1.2\nTypo = 3");
        let s = sync(&user);
        assert_eq!(s.unknown, vec!["Player.Typo = 3", "Old.A = 1", "Old.B = \"x\""]);
        let text = s.text.unwrap();
        let d = doc(&text);
        assert!(d.get("Old").is_none());
        assert!(text.contains("# Player.Typo = 3\n"));
        assert!(sync(&text).text.is_none());
    }

    #[test]
    fn kept_names_are_not_unknown() {
        let user = format!("{TEMPLATE}\n[[Override]]\nSpEffect = 1\n");
        assert!(sync(&user).text.is_none());
    }

    #[test]
    fn rename_never_overwrites_a_set_key() {
        let template = doc(TEMPLATE);
        let mut d = doc("[Player]\nAll = 5.0\n[Speed]\nPlayerAll = 2.0\n");
        assert!(!rename_key(&mut d, &template, "Speed.PlayerAll", "Player.All"));
        assert_eq!(d["Player"]["All"].as_float(), Some(5.0));
    }

    #[derive(serde::Deserialize, serde::Serialize, Default, Debug, PartialEq)]
    #[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
    struct Cfg {
        player: PlayerCfg,
    }

    #[derive(serde::Deserialize, serde::Serialize, Default, Debug, PartialEq)]
    #[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
    struct PlayerCfg {
        walk: f32,
        run: f32,
    }

    const CFG_TEMPLATE: &str = "[Player]
Walk = 1.1
Run = 1.2
";

    #[test]
    fn missing_keys_take_the_template_value() {
        let cfg: Cfg = parse("[Player]
Run = 3.0
", CFG_TEMPLATE).unwrap();
        assert_eq!(cfg.player.walk, 1.1);
        assert_eq!(cfg.player.run, 3.0);
        let cfg: Cfg = parse("", CFG_TEMPLATE).unwrap();
        assert_eq!(cfg, defaults::<Cfg>(CFG_TEMPLATE));
    }

    #[test]
    fn errors_keep_the_users_line() {
        let err = parse::<Cfg>("[Player]
Run = 1.0
Rn = 2.0
", CFG_TEMPLATE).unwrap_err();
        assert!(err.contains("Rn") && err.contains("line 3"), "{err}");
        let err = parse::<Cfg>("[Player]
Run = \"fast\"
", CFG_TEMPLATE).unwrap_err();
        assert!(err.contains("line 2"), "{err}");
    }

    #[test]
    fn template_missing_keys_finds_unset_fields() {
        assert!(template_missing_keys::<Cfg>(CFG_TEMPLATE, &[]).is_empty());
        assert_eq!(template_missing_keys::<Cfg>("[Player]
Walk = 1.1
", &[]), vec!["Player.Run"]);
    }

    #[test]
    fn set_value_keeps_the_users_inline_comment() {
        let template = doc(TEMPLATE);
        let mut d = doc("[Player]\nRun = 1.0 # mine\n");
        set_value(&mut d, &template, "Player.Run", 2.0);
        assert!(d.to_string().contains("Run = 2.0 # mine"), "{}", d);
    }
}
