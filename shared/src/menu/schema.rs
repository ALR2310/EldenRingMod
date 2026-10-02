//! The in-game config menu's description of a mod: a hand-written menu file
//! `<Mod>.menu.toml` next to the mod's config template, embedded in the DLL
//! (`include_str!`) and not shipped to users (2026-10-02, the user's choice
//! - the menu is readable/editable as a file instead of living in code; a
//! `#[derive(ConfigUi)]` attempt was dropped the same day).
//!
//! The menu host may live in another mod's DLL, so the menu text crosses
//! the DLL boundary through [export_menu_api]'s exports and is parsed there
//! by [parse_menu]. Design: TODO.md "Menu chỉnh cấu hình trong game".
//!
//! # Menu file format
//!
//! ```toml
//! Version = 1                    # menu format version (MENU_VERSION)
//! Tab = "Speed Multiplier"       # tab title
//!
//! [Player]                       # = the config's [Player] table, a group
//! Label = "Player"
//! Description = "Groups of player actions"
//! Slider = { Min = 0.1, Max = 10.0, Step = 0.05 }   # default for sliders below
//! All  = { Kind = "Slider", Label = "All actions", Description = "..." }
//! Walk = { Kind = "Slider", Label = "Walk", Max = 3.0 }  # own max
//!
//! [Override]                     # = the config's [[Override]] list
//! Kind = "List"
//! Label = "Overrides"
//! Title = "SpEffect {SpEffect}"  # each block's header
//! SpEffect = { Kind = "IdList", Label = "SpEffect ids" }
//! Optional = ["Player", "Torrent"]   # any key of these groups, optional
//! ```
//!
//! - File order = menu order. A config key not listed is not in the menu.
//! - Field kinds: `Slider` (float), `Int`, `Bool`, `Key` (hotkey picker),
//!   `Text`, `IdList` (one id or a list of ids).
//! - `Label`, `Description`, `Slider`, `Kind`, `Title`, `Optional` are
//!   group settings, so a config key can't have one of those names.

use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

/// Menu file format version this code reads.
pub const MENU_VERSION: u32 = 1;

const GROUP_SETTINGS: [&str; 6] = ["Label", "Description", "Slider", "Kind", "Title", "Optional"];

#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub version: u32,
    pub tab: String,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// The config table's name (`Player`).
    pub key: String,
    pub label: String,
    pub description: String,
    /// `Some` for a `Kind = "List"` group (`[[Override]]` in the config).
    pub list: Option<ListInfo>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListInfo {
    /// Block header; `{Key}` is replaced by that key's value.
    pub title: String,
    /// Groups whose keys a block may set, each optional.
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// The config key (`Walk`).
    pub key: String,
    pub label: String,
    pub description: String,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Slider { min: f64, max: f64, step: f64 },
    Int { min: i64, max: i64 },
    Bool,
    Key,
    Text,
    IdList,
}

fn number(v: &Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

fn text(t: &dyn TableLike, key: &str) -> String {
    t.get(key).and_then(Item::as_str).unwrap_or_default().to_string()
}

fn num_in(t: &dyn TableLike, key: &str) -> Option<f64> {
    t.get(key).and_then(Item::as_value).and_then(number)
}

/// Parses a menu file. Errors name the group/key at fault.
pub fn parse_menu(menu: &str) -> Result<Menu, String> {
    let doc: DocumentMut = menu.parse().map_err(|e: toml_edit::TomlError| e.to_string().trim_end().to_string())?;
    let version = doc
        .get("Version")
        .and_then(Item::as_integer)
        .ok_or("missing `Version`")? as u32;
    if version != MENU_VERSION {
        return Err(format!("menu format version {version}, this code reads {MENU_VERSION}"));
    }
    let tab = doc.get("Tab").and_then(Item::as_str).ok_or("missing `Tab`")?.to_string();

    let mut groups = Vec::new();
    for (key, item) in doc.iter() {
        if key == "Version" || key == "Tab" {
            continue;
        }
        let table = item.as_table().ok_or_else(|| format!("`{key}`: expected a [table]"))?;
        groups.push(parse_group(key, table)?);
    }
    Ok(Menu { version, tab, groups })
}

fn parse_group(key: &str, t: &Table) -> Result<Group, String> {
    let slider = t.get("Slider").and_then(Item::as_table_like);
    let default_range = (
        slider.and_then(|s| num_in(s, "Min")).unwrap_or(0.0),
        slider.and_then(|s| num_in(s, "Max")).unwrap_or(10.0),
        slider.and_then(|s| num_in(s, "Step")).unwrap_or(0.01),
    );
    let list = match t.get("Kind").and_then(Item::as_str) {
        None => None,
        Some("List") => Some(ListInfo {
            title: text(t, "Title"),
            optional: t
                .get("Optional")
                .and_then(Item::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        }),
        Some(other) => return Err(format!("[{key}] Kind = \"{other}\": a group's Kind can only be \"List\"")),
    };

    let mut fields = Vec::new();
    for (name, item) in t.iter() {
        if GROUP_SETTINGS.contains(&name) {
            continue;
        }
        let f = item
            .as_table_like()
            .ok_or_else(|| format!("{key}.{name}: expected {{ Kind = ..., Label = ... }}"))?;
        let kind = match f.get("Kind").and_then(Item::as_str) {
            Some("Slider") => Kind::Slider {
                min: num_in(f, "Min").unwrap_or(default_range.0),
                max: num_in(f, "Max").unwrap_or(default_range.1),
                step: num_in(f, "Step").unwrap_or(default_range.2),
            },
            Some("Int") => Kind::Int {
                min: num_in(f, "Min").unwrap_or(0.0) as i64,
                max: num_in(f, "Max").unwrap_or(100.0) as i64,
            },
            Some("Bool") => Kind::Bool,
            Some("Key") => Kind::Key,
            Some("Text") => Kind::Text,
            Some("IdList") => Kind::IdList,
            Some(other) => return Err(format!("{key}.{name}: unknown Kind \"{other}\"")),
            None => return Err(format!("{key}.{name}: missing Kind")),
        };
        let label = match text(f, "Label") {
            l if l.is_empty() => name.to_string(),
            l => l,
        };
        fields.push(Field {
            key: name.to_string(),
            label,
            description: text(f, "Description"),
            kind,
        });
    }
    Ok(Group {
        key: key.to_string(),
        label: match text(t, "Label") {
            l if l.is_empty() => key.to_string(),
            l => l,
        },
        description: text(t, "Description"),
        list,
        fields,
    })
}

/// Problems between a menu file and the config template it describes, for
/// a mod's unit test: every menu key must exist in the template with a
/// value its Kind can edit; a list's `Optional` groups must be menu groups.
/// (List groups aren't checked against the template - `[[Override]]` is
/// only a comment there.)
pub fn check_menu(menu: &str, template: &str) -> Vec<String> {
    let menu = match parse_menu(menu) {
        Ok(menu) => menu,
        Err(e) => return vec![format!("menu file: {e}")],
    };
    let template: DocumentMut = match template.parse() {
        Ok(t) => t,
        Err(e) => return vec![format!("template: {e}")],
    };
    let mut problems = Vec::new();
    let plain: Vec<&str> = menu.groups.iter().filter(|g| g.list.is_none()).map(|g| g.key.as_str()).collect();
    for group in &menu.groups {
        if let Some(list) = &group.list {
            for name in &list.optional {
                if !plain.contains(&name.as_str()) {
                    problems.push(format!("[{}] Optional names `{name}`, which isn't a group in the menu", group.key));
                }
            }
            continue;
        }
        let Some(table) = template.get(&group.key).and_then(Item::as_table) else {
            problems.push(format!("[{}]: no such table in the template", group.key));
            continue;
        };
        for field in &group.fields {
            let path = format!("{}.{}", group.key, field.key);
            let Some(value) = table.get(&field.key).and_then(Item::as_value) else {
                problems.push(format!("{path}: no such key in the template"));
                continue;
            };
            let fits = match field.kind {
                Kind::Slider { .. } => number(value).is_some(),
                Kind::Int { .. } => value.as_integer().is_some(),
                Kind::Bool => value.as_bool().is_some(),
                Kind::Key => value.as_str().is_some() || value.as_integer().is_some(),
                Kind::Text => value.as_str().is_some(),
                Kind::IdList => value.as_integer().is_some() || value.as_array().is_some(),
            };
            if !fits {
                problems.push(format!("{path}: {:?} can't edit the template value `{}`", field.kind, value.to_string().trim()));
            }
        }
    }
    problems
}

// --- DLL exports ---------------------------------------------------------
//
// The menu host - possibly another mod's DLL - finds a mod by looking up
// these symbols in every loaded module (`GetProcAddress`). Plain C types
// only: the two DLLs may be built by different compilers at different
// times. The `_v1` suffix is the export interface version; an incompatible
// change gets new `_v2` names next to these.

/// Implements the exports for [export_menu_api]. Not for direct use.
#[doc(hidden)]
pub mod __export {
    use std::ffi::{CString, c_char};
    use std::sync::OnceLock;

    pub struct Text(OnceLock<CString>);

    impl Text {
        pub const fn new() -> Self {
            Self(OnceLock::new())
        }

        /// Keeps `value` for the rest of the process; later calls are
        /// ignored.
        pub fn set(&self, value: String) {
            let _ = self.0.set(CString::new(value).unwrap_or_default());
        }

        pub fn get_or(&self, make: impl FnOnce() -> String) -> *const c_char {
            self.0.get_or_init(|| CString::new(make()).unwrap_or_default()).as_ptr()
        }

        pub fn ptr(&self) -> *const c_char {
            self.0.get().map_or(std::ptr::null(), |s| s.as_ptr())
        }
    }
}

/// Exports this mod to the in-game config menu. Call once at the crate
/// root:
///
/// ```ignore
/// common::export_menu_api! {
///     menu: include_str!("../SpeedMultiplier.menu.toml"),
///     path: crate::config::path,      // fn() -> Option<String>
///     reload: crate::reload_config,   // fn() -> Result<(), String>
/// }
/// ```
///
/// Generates `alr_menu_schema_v1() -> *const c_char` (the menu file's
/// text), `alr_menu_config_path_v1() -> *const c_char` (null until the
/// config is loaded) and `alr_menu_reload_v1() -> i32` (0 = reloaded,
/// 1 = the file has an error and the previous config stays).
#[macro_export]
macro_rules! export_menu_api {
    (menu: $menu:expr, path: $path:path, reload: $reload:path $(,)?) => {
        static __ALR_MENU_SCHEMA: $crate::menu::schema::__export::Text = $crate::menu::schema::__export::Text::new();
        static __ALR_MENU_PATH: $crate::menu::schema::__export::Text = $crate::menu::schema::__export::Text::new();

        #[unsafe(no_mangle)]
        pub extern "C" fn alr_menu_schema_v1() -> *const ::std::ffi::c_char {
            __ALR_MENU_SCHEMA.get_or(|| ($menu).to_string())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn alr_menu_config_path_v1() -> *const ::std::ffi::c_char {
            if let Some(path) = $path() {
                __ALR_MENU_PATH.set(path);
            }
            __ALR_MENU_PATH.ptr()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn alr_menu_reload_v1() -> i32 {
            match ::std::panic::catch_unwind(|| $reload()) {
                Ok(Ok(())) => 0,
                _ => 1,
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = "\
[General]
ConfigVersion = 1
ReloadKey = \"F5\"
ReloadBanner = true

[Player]
All = 1.0
Walk = 1
";

    const MENU: &str = "\
Version = 1
Tab = \"Test Mod\"

[General]
ReloadKey    = { Kind = \"Key\", Label = \"Reload key\" }
ReloadBanner = { Kind = \"Bool\", Description = \"Show a banner\" }

[Player]
Label = \"Player\"
Description = \"Groups of player actions\"
Slider = { Min = 0.1, Max = 10.0, Step = 0.05 }
Walk = { Kind = \"Slider\", Label = \"Walk\", Max = 3.0 }
All  = { Kind = \"Slider\", Label = \"All actions\" }

[Override]
Kind = \"List\"
Label = \"Overrides\"
Title = \"SpEffect {SpEffect}\"
SpEffect = { Kind = \"IdList\", Label = \"SpEffect ids\" }
Optional = [\"Player\"]
";

    #[test]
    fn parses_in_file_order_with_group_defaults() {
        let menu = parse_menu(MENU).unwrap();
        assert_eq!(menu.tab, "Test Mod");
        let keys: Vec<&str> = menu.groups.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(keys, ["General", "Player", "Override"]);

        let general = &menu.groups[0];
        assert_eq!(general.label, "General"); // defaults to the key
        assert_eq!(general.fields[0].kind, Kind::Key);
        assert_eq!(general.fields[1].label, "ReloadBanner");
        assert_eq!(general.fields[1].description, "Show a banner");

        let player = &menu.groups[1];
        assert_eq!(player.description, "Groups of player actions");
        assert_eq!(player.fields[0].key, "Walk"); // file order, not alphabetical
        assert_eq!(player.fields[0].kind, Kind::Slider { min: 0.1, max: 3.0, step: 0.05 });
        assert_eq!(player.fields[1].kind, Kind::Slider { min: 0.1, max: 10.0, step: 0.05 });

        let list = menu.groups[2].list.as_ref().unwrap();
        assert_eq!(list.title, "SpEffect {SpEffect}");
        assert_eq!(list.optional, ["Player"]);
        assert_eq!(menu.groups[2].fields[0].kind, Kind::IdList);
    }

    #[test]
    fn menu_matches_template() {
        assert_eq!(check_menu(MENU, TEMPLATE), Vec::<String>::new());
    }

    #[test]
    fn check_menu_reports_mistakes() {
        let bad = MENU
            .replace("Walk = { Kind = \"Slider\"", "Walk = { Kind = \"Bool\"")
            .replace("All  = {", "Al  = {")
            .replace("Optional = [\"Player\"]", "Optional = [\"Torrent\"]");
        let problems = check_menu(&bad, TEMPLATE);
        assert!(problems.iter().any(|p| p.starts_with("Player.Walk: Bool")), "{problems:?}");
        assert!(problems.iter().any(|p| p == "Player.Al: no such key in the template"), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("`Torrent`")), "{problems:?}");
    }

    #[test]
    fn parse_errors_name_the_place() {
        assert!(parse_menu("Tab = \"x\"").unwrap_err().contains("Version"));
        let err = parse_menu("Version = 1\nTab = \"x\"\n[P]\nA = { Kind = \"Dial\" }\n").unwrap_err();
        assert!(err.contains("P.A") && err.contains("Dial"), "{err}");
    }
}
