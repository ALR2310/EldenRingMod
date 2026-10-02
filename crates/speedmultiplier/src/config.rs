//! `SpeedMultiplier.toml` (2026-10-02, replaces `SpeedMultiplier.ini`):
//! loaded through `common::toml_config`, so the upcoming per-SpEffect
//! rules can be a list of tables instead of numbered ini keys. Key names
//! stay PascalCase, as in the ini.
//!
//! `Default` must match the embedded template - the test below checks it.

use std::sync::{Arc, OnceLock};

use serde::Deserialize;

use common::toml_config::{LoadReport, TomlConfig};

/// Embedded verbatim at compile time - single source of truth for the
/// default file.
pub const TEMPLATE: &str = include_str!("../SpeedMultiplier.toml");

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Config {
    pub general: General,
    pub speed: Speed,
    pub logging: Logging,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct General {
    #[serde(deserialize_with = "common::toml_config::key_name")]
    pub reload_key: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Speed {
    pub player_all: f32,
    pub player_movement: f32,
    pub player_roll: f32,
    pub player_attack: f32,
    pub player_skill: f32,
    pub player_cast: f32,
    pub player_item: f32,
    pub player_other: f32,
    pub torrent: f32,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Logging {
    pub log_file: bool,
    pub speed_probe: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General::default(),
            speed: Speed::default(),
            logging: Logging::default(),
        }
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            reload_key: "F5".to_string(),
        }
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self {
            player_all: 1.0,
            player_movement: 1.2,
            player_roll: 1.1,
            player_attack: 1.2,
            player_skill: 1.2,
            player_cast: 1.2,
            player_item: 1.0,
            player_other: 1.0,
            torrent: 1.3,
        }
    }
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            log_file: true,
            speed_probe: false,
        }
    }
}

static CONFIG: OnceLock<TomlConfig<Config>> = OnceLock::new();

/// Loads (creating / adding missing keys to) `path`. Call once, first thing
/// in `DllMain`'s thread; the report is for logging once the logger is up.
pub fn init(path: &str) -> LoadReport {
    let (config, report) = TomlConfig::load_or_create(path, TEMPLATE);
    let _ = CONFIG.set(config);
    report
}

/// The config in use - take it once per tick.
pub fn get() -> Arc<Config> {
    match CONFIG.get() {
        Some(config) => config.get(),
        None => Arc::new(Config::default()),
    }
}

pub fn reload() -> Result<(), String> {
    match CONFIG.get() {
        Some(config) => config.reload(),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_matches_default() {
        let parsed: Config = toml::from_str(TEMPLATE).expect("template parses");
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn missing_keys_fall_back_to_default() {
        let parsed: Config = toml::from_str("[Speed]\nPlayerRoll = 2.0\n").unwrap();
        assert_eq!(parsed.speed.player_roll, 2.0);
        assert_eq!(parsed.speed.torrent, 1.3);
        assert!(parsed.logging.log_file);
    }

    #[test]
    fn misspelled_key_is_an_error_with_its_line() {
        let err = toml::from_str::<Config>("[Speed]\nPlayerRol = 2.0\n").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("PlayerRol"), "{msg}");
        assert!(msg.contains("line 2"), "{msg}");
    }

    #[test]
    fn reload_key_takes_a_name_or_a_number() {
        let parsed: Config = toml::from_str("[General]
ReloadKey = \"0x74\"
").unwrap();
        assert_eq!(parsed.general.reload_key, "0x74");
        let parsed: Config = toml::from_str("[General]
ReloadKey = 116
").unwrap();
        assert_eq!(parsed.general.reload_key, "116");
        // A TOML hex literal is an integer too (user test, 2026-10-02).
        let parsed: Config = toml::from_str("[General]
ReloadKey = 0x75
").unwrap();
        assert_eq!(common::input::parse_virtual_key(&parsed.general.reload_key, 0x74), 0x75);
    }

    #[test]
    fn integer_value_is_accepted_as_a_speed() {
        // Users will write `Torrent = 2`, not `2.0`.
        let parsed: Config = toml::from_str("[Speed]\nTorrent = 2\n").unwrap();
        assert_eq!(parsed.speed.torrent, 2.0);
    }
}
