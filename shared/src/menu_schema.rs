//! What the in-game config menu needs to draw a mod's settings: one
//! [UiField] per key, nested like the TOML tables. Built from the config
//! struct itself by `#[derive(ConfigUi)]` (crate `config_ui_derive`,
//! re-exported here) - labels, ranges and control kinds sit on the struct,
//! the defaults stay in the template (see [`crate::toml_config`]).
//!
//! The menu host may live in another mod's DLL, so the schema crosses the
//! DLL boundary as text: [schema_toml] (2026-10-02, design in TODO.md
//! "Menu chỉnh cấu hình trong game").

use serde::{Deserialize, Serialize};

pub use config_ui_derive::ConfigUi;

/// Implemented by `#[derive(ConfigUi)]`.
pub trait ConfigUi {
    fn ui_fields() -> Vec<UiField>;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct UiField {
    /// The TOML key (`Walk`), as serde names it.
    pub key: String,
    pub label: String,
    /// From the field's `///` doc comment; may be empty.
    pub description: String,
    pub kind: UiKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "Type", rename_all = "PascalCase")]
pub enum UiKind {
    /// Slider.
    Float { min: f64, max: f64, step: f64 },
    Int { min: i64, max: i64 },
    /// Checkbox.
    Bool,
    /// Hotkey picker (the value is a key name, see `crate::input`).
    Key,
    Text,
    /// A TOML table: its own fields.
    Group { fields: Vec<UiField> },
    /// A `[[list]]`: the fields of one entry.
    List { item: Vec<UiField> },
}

/// Schema format version - bump on an incompatible change to the types
/// above, so a menu host can skip a schema it doesn't understand.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Schema {
    pub version: u32,
    /// Tab title in the menu.
    pub tab: String,
    pub fields: Vec<UiField>,
}

/// `T`'s schema as TOML text, for the DLL export.
pub fn schema_toml<T: ConfigUi>(tab: &str) -> String {
    let schema = Schema {
        version: SCHEMA_VERSION,
        tab: tab.to_string(),
        fields: T::ui_fields(),
    };
    toml::to_string(&schema).expect("schema serializes")
}

/// Dotted key paths of every value field in `fields` (lists contribute
/// their entry fields under `List[]`).
pub fn field_paths(fields: &[UiField]) -> Vec<String> {
    fn walk(fields: &[UiField], prefix: &str, out: &mut Vec<String>) {
        for f in fields {
            let path = if prefix.is_empty() { f.key.clone() } else { format!("{prefix}.{}", f.key) };
            match &f.kind {
                UiKind::Group { fields } => walk(fields, &path, out),
                UiKind::List { item } => walk(item, &format!("{path}[]"), out),
                _ => out.push(path),
            }
        }
    }
    let mut out = Vec::new();
    walk(fields, "", &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(ConfigUi, serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    #[allow(dead_code)]
    #[ui(min = 0.1, max = 10.0, step = 0.05)]
    struct Player {
        /// walking
        walk: f32,
        /// all actions
        #[ui(label = "All actions", max = 5.0)]
        all: f32,
        #[ui(hidden)]
        secret: f32,
    }

    #[derive(ConfigUi, serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    #[allow(dead_code)]
    struct General {
        /// Reload key
        #[ui(key)]
        reload_key: String,
        reload_banner: bool,
        #[serde(rename = "Ver")]
        config_version: u32,
    }

    #[derive(ConfigUi, serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    #[allow(dead_code)]
    struct Entry {
        sp_effect: i32,
        maybe: Option<f32>,
    }

    #[derive(ConfigUi, serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    #[allow(dead_code)]
    struct Config {
        general: General,
        player: Player,
        #[serde(rename = "Override")]
        overrides: Vec<Entry>,
    }

    #[test]
    fn derive_reads_names_docs_types_and_attrs() {
        let fields = Player::ui_fields();
        assert_eq!(fields.len(), 2, "hidden field left out");
        assert_eq!(fields[0].key, "Walk");
        assert_eq!(fields[0].label, "Walk");
        assert_eq!(fields[0].description, "walking");
        assert_eq!(fields[0].kind, UiKind::Float { min: 0.1, max: 10.0, step: 0.05 });
        assert_eq!(fields[1].label, "All actions");
        assert_eq!(fields[1].kind, UiKind::Float { min: 0.1, max: 5.0, step: 0.05 });

        let general = General::ui_fields();
        assert_eq!(general[0].kind, UiKind::Key);
        assert_eq!(general[1].kind, UiKind::Bool);
        assert_eq!(general[2].key, "Ver");
        assert_eq!(general[2].kind, UiKind::Int { min: 0, max: 100 });
    }

    #[test]
    fn nested_structs_and_lists() {
        assert_eq!(
            field_paths(&Config::ui_fields()),
            vec![
                "General.ReloadKey",
                "General.ReloadBanner",
                "General.Ver",
                "Player.Walk",
                "Player.All",
                "Override[].SpEffect", // Option field skipped
            ]
        );
    }

    #[test]
    fn schema_round_trips_as_toml() {
        let text = schema_toml::<Config>("Test Mod");
        let schema: Schema = toml::from_str(&text).unwrap();
        assert_eq!(schema.version, SCHEMA_VERSION);
        assert_eq!(schema.tab, "Test Mod");
        assert_eq!(schema.fields, Config::ui_fields());
    }
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
///     tab: "Speed Multiplier",
///     config: crate::config::Config,
///     path: crate::config::path,      // fn() -> Option<String>
///     reload: crate::config::reload,  // fn() -> Result<(), String>
/// }
/// ```
///
/// Generates `alr_menu_schema_v1() -> *const c_char` (the [schema_toml]
/// text), `alr_menu_config_path_v1() -> *const c_char` (null until the
/// config is loaded) and `alr_menu_reload_v1() -> i32` (0 = reloaded,
/// 1 = the file has an error and the previous config stays).
#[macro_export]
macro_rules! export_menu_api {
    (tab: $tab:expr, config: $config:ty, path: $path:path, reload: $reload:path $(,)?) => {
        static __ALR_MENU_SCHEMA: $crate::menu_schema::__export::Text = $crate::menu_schema::__export::Text::new();
        static __ALR_MENU_PATH: $crate::menu_schema::__export::Text = $crate::menu_schema::__export::Text::new();

        #[unsafe(no_mangle)]
        pub extern "C" fn alr_menu_schema_v1() -> *const ::std::ffi::c_char {
            __ALR_MENU_SCHEMA.get_or(|| $crate::menu_schema::schema_toml::<$config>($tab))
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
