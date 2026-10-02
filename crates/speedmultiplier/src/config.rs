//! `SpeedMultiplier.toml` (2026-10-02, replaces `SpeedMultiplier.ini`):
//! loaded through `common::toml_config`, so the per-SpEffect overrides can
//! be a list of tables instead of numbered ini keys. Key names stay
//! PascalCase, as in the ini.
//!
//! `[[Override]]` (2026-10-02, Nexus request from Lwingr): `SpEffect` is
//! one id or a list (the player has ANY of them -> active), plus any of the
//! `[Speed]` keys. Every active override applies, top to bottom, so a later
//! one wins on a key both set - see [Config::effective_speed].
//!
//! `Default` must match the embedded template - the test below checks it.

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Deserializer};

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
    #[serde(rename = "Override")]
    pub overrides: Vec<Override>,
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

/// One `[[Override]]`: the `[Speed]` keys it sets while the player has any
/// of `sp_effect`. No `#[serde(default)]` on the struct - an override
/// without `SpEffect` is an error, not one that is always on.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
pub struct Override {
    #[serde(rename = "SpEffect", deserialize_with = "sp_effect_ids")]
    pub sp_effect: Vec<i32>,
    #[serde(default)]
    pub player_all: Option<f32>,
    #[serde(default)]
    pub player_movement: Option<f32>,
    #[serde(default)]
    pub player_roll: Option<f32>,
    #[serde(default)]
    pub player_attack: Option<f32>,
    #[serde(default)]
    pub player_skill: Option<f32>,
    #[serde(default)]
    pub player_cast: Option<f32>,
    #[serde(default)]
    pub player_item: Option<f32>,
    #[serde(default)]
    pub player_other: Option<f32>,
    #[serde(default)]
    pub torrent: Option<f32>,
}

impl Override {
    /// Writes every key this override sets into `speed`.
    fn apply_to(&self, speed: &mut Speed) {
        let pairs = [
            (self.player_all, &mut speed.player_all),
            (self.player_movement, &mut speed.player_movement),
            (self.player_roll, &mut speed.player_roll),
            (self.player_attack, &mut speed.player_attack),
            (self.player_skill, &mut speed.player_skill),
            (self.player_cast, &mut speed.player_cast),
            (self.player_item, &mut speed.player_item),
            (self.player_other, &mut speed.player_other),
            (self.torrent, &mut speed.torrent),
        ];
        for (value, slot) in pairs {
            if let Some(value) = value {
                *slot = value;
            }
        }
    }
}

impl Config {
    /// `[Speed]` with every override whose SpEffects `has_sp_effect` finds
    /// applied on top, in file order, plus the indices of those overrides.
    pub fn effective_speed(&self, has_sp_effect: impl Fn(i32) -> bool) -> (Speed, Vec<usize>) {
        let mut speed = self.speed.clone();
        let mut active = Vec::new();
        for (index, o) in self.overrides.iter().enumerate() {
            if o.sp_effect.iter().any(|&id| has_sp_effect(id)) {
                o.apply_to(&mut speed);
                active.push(index);
            }
        }
        (speed, active)
    }
}

/// `SpEffect = 1234`, `SpEffect = [1234, 1235]`; ids also accepted as
/// strings (`"1234"`). An empty list is an error - it could never match.
fn sp_effect_ids<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<i32>, D::Error> {
    use serde::de::{Error, SeqAccess, Visitor};

    fn id<E: Error>(value: i64) -> Result<i32, E> {
        i32::try_from(value)
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| E::custom(format!("SpEffect id {value} is not a valid id")))
    }
    fn id_str<E: Error>(value: &str) -> Result<i32, E> {
        let n: i64 = value
            .trim()
            .parse()
            .map_err(|_| E::custom(format!("SpEffect id \"{value}\" is not a number")))?;
        id(n)
    }

    struct Ids;
    impl<'de> Visitor<'de> for Ids {
        type Value = Vec<i32>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a SpEffect id or a list of ids, e.g. 1234 or [1234, 1235]")
        }
        fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(vec![id(v)?])
        }
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(vec![id_str(v)?])
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum One {
                Num(i64),
                Text(String),
            }
            let mut ids = Vec::new();
            while let Some(one) = seq.next_element::<One>()? {
                ids.push(match one {
                    One::Num(n) => id(n)?,
                    One::Text(t) => id_str(&t)?,
                });
            }
            if ids.is_empty() {
                return Err(A::Error::custom(
                    "SpEffect = [] is empty - list at least one id",
                ));
            }
            Ok(ids)
        }
    }
    deserializer.deserialize_any(Ids)
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Logging {
    pub log_file: bool,
    pub speed_probe: bool,
    pub effect_probe: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General::default(),
            speed: Speed::default(),
            logging: Logging::default(),
            overrides: Vec::new(),
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
            effect_probe: false,
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
        let parsed: Config = toml::from_str(
            "[General]
ReloadKey = \"0x74\"
",
        )
        .unwrap();
        assert_eq!(parsed.general.reload_key, "0x74");
        let parsed: Config = toml::from_str(
            "[General]
ReloadKey = 116
",
        )
        .unwrap();
        assert_eq!(parsed.general.reload_key, "116");
        // A TOML hex literal is an integer too (user test, 2026-10-02).
        let parsed: Config = toml::from_str(
            "[General]
ReloadKey = 0x75
",
        )
        .unwrap();
        assert_eq!(
            common::input::parse_virtual_key(&parsed.general.reload_key, 0x74),
            0x75
        );
    }

    fn has(ids: &'static [i32]) -> impl Fn(i32) -> bool {
        move |id| ids.contains(&id)
    }

    const OVERRIDES: &str = "
[[Override]]
SpEffect = [1111, 1112]
PlayerRoll = 1.5

[[Override]]
SpEffect = 2222
PlayerAttack = 1.6
PlayerRoll = 2.0
";

    #[test]
    fn override_needs_any_of_its_sp_effects() {
        let config: Config = toml::from_str(OVERRIDES).unwrap();
        let (speed, active) = config.effective_speed(has(&[1112]));
        assert_eq!(active, vec![0]);
        assert_eq!(speed.player_roll, 1.5);
        assert_eq!(speed.player_attack, 1.2); // [Speed] default
    }

    #[test]
    fn active_overrides_stack_and_the_later_one_wins() {
        let config: Config = toml::from_str(OVERRIDES).unwrap();
        let (speed, active) = config.effective_speed(has(&[1111, 2222]));
        assert_eq!(active, vec![0, 1]);
        assert_eq!(speed.player_roll, 2.0);
        assert_eq!(speed.player_attack, 1.6);
        assert_eq!(speed.player_movement, 1.2);
    }

    #[test]
    fn no_active_override_keeps_speed() {
        let config: Config = toml::from_str(OVERRIDES).unwrap();
        let (speed, active) = config.effective_speed(has(&[]));
        assert!(active.is_empty());
        assert_eq!(speed, Speed::default());
    }

    #[test]
    fn sp_effect_ids_as_strings_are_accepted() {
        let config: Config =
            toml::from_str("[[Override]]\nSpEffect = [\"1111\", 2222]\nTorrent = 2\n").unwrap();
        assert_eq!(config.overrides[0].sp_effect, vec![1111, 2222]);
        assert_eq!(config.overrides[0].torrent, Some(2.0));
    }

    #[test]
    fn override_errors_are_reported() {
        for (text, needle) in [
            ("[[Override]]\nPlayerRoll = 1.5\n", "SpEffect"),
            ("[[Override]]\nSpEffect = []\n", "empty"),
            ("[[Override]]\nSpEffect = \"abc\"\n", "not a number"),
            ("[[Override]]\nSpEffect = 1\nPlayerRol = 1\n", "PlayerRol"),
        ] {
            let err = toml::from_str::<Config>(text).unwrap_err().to_string();
            assert!(err.contains(needle), "{text:?} -> {err}");
        }
    }

    #[test]
    fn integer_value_is_accepted_as_a_speed() {
        // Users will write `Torrent = 2`, not `2.0`.
        let parsed: Config = toml::from_str("[Speed]\nTorrent = 2\n").unwrap();
        assert_eq!(parsed.speed.torrent, 2.0);
    }
}
