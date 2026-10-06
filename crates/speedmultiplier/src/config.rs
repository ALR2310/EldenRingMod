//! `SpeedMultiplier.toml` (2026-10-02, replaces `SpeedMultiplier.ini`):
//! loaded through `common::toml_config`, so the per-SpEffect overrides can
//! be a list of tables instead of numbered ini keys. Key names stay
//! PascalCase, as in the ini.
//!
//! Tables `[Player]` / `[Torrent]` (2026-10-02, the user's layout - before
//! release it was one `[Speed]` with `Player*` / `Torrent` keys).
//! `General.ConfigVersion` is the file format version for future
//! migrations ([STEPS]); 1 is the first released format.
//!
//! `[[Override]]` (2026-10-02, Nexus request from Lwingr): `SpEffect` is
//! one id or a list (the player has ANY of them -> active), plus any
//! `Player.*` / `Torrent.*` key as a dotted key (`Player.Roll = 1.5`).
//! Every active override applies, top to bottom, so a later one wins on a
//! key both set - see [Config::effective_speed].
//!
//! Defaults live only in the embedded template (2026-10-02): `Default` on
//! these structs is the derived zero value `common::toml_config` needs
//! while parsing, never a config in use - see [template] and the test
//! `template_sets_every_field`.

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Deserializer, Serialize};

use common::toml_config::{LoadReport, Migration, Step, TomlConfig};

/// Embedded verbatim at compile time - single source of truth for the
/// default file.
pub const TEMPLATE: &str = include_str!("../SpeedMultiplier.toml");

/// Format migration steps for `General.ConfigVersion` (see
/// `common::toml_config`): `STEPS[i]` takes a file from version `i + 1` to
/// `i + 2`. Version 1 = the first released TOML format (1.1.0). Append
/// only - a released step must never change.
const STEPS: &[Step] = &[];

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Config {
    pub general: General,
    pub player: Player,
    pub torrent: Torrent,
    pub logging: Logging,
    #[serde(rename = "Override")]
    pub overrides: Vec<Override>,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct General {
    /// File format version, managed by the mod.
    pub config_version: u32,
    #[serde(deserialize_with = "common::toml_config::key_name")]
    pub reload_key: String,
    pub reload_banner: bool,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Player {
    pub all: f32,
    pub walk: f32,
    pub run: f32,
    pub sneak: f32,
    pub jump: f32,
    pub roll: f32,
    pub ladder: f32,
    pub attack: f32,
    pub critical: f32,
    pub skill: f32,
    pub cast: f32,
    pub item: f32,
    pub other: f32,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Torrent {
    pub all: f32,
    pub walk: f32,
    pub run: f32,
    pub jump: f32,
    pub other: f32,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Logging {
    pub log_file: bool,
    pub speed_probe: bool,
    pub effect_probe: bool,
}

/// `[Player]` + `[Torrent]` after overrides - what `speed.rs` applies.
#[derive(Debug, Clone, PartialEq)]
pub struct Speeds {
    pub player: Player,
    pub torrent: Torrent,
}

/// The player's equip load class (2026-10-06, user test: `ChrCtrl.weight_type`
/// 1..4 went with the roll anims `027100`-`027130` as the gear got heavier;
/// 0 = not loaded yet, treated as no class).
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadClass {
    Light,
    Medium,
    Heavy,
    Overweight,
}

impl LoadClass {
    pub fn from_weight_type(weight_type: u32) -> Option<Self> {
        match weight_type {
            1 => Some(Self::Light),
            2 => Some(Self::Medium),
            3 => Some(Self::Heavy),
            4 => Some(Self::Overweight),
            _ => None,
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "light" => Some(Self::Light),
            "medium" => Some(Self::Medium),
            "heavy" => Some(Self::Heavy),
            "overweight" => Some(Self::Overweight),
            _ => None,
        }
    }
}

/// One `[[Override]]`: the keys it sets while its conditions hold - the
/// player has any of `sp_effect` (when given) AND the equip load class is
/// one of `equip_load` (when given). At least one condition is required: an
/// override without any is an error, not one that is always on.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(try_from = "RawOverride", rename_all = "PascalCase")]
pub struct Override {
    #[serde(rename = "SpEffect", skip_serializing_if = "Vec::is_empty")]
    pub sp_effect: Vec<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub equip_load: Vec<LoadClass>,
    pub player: PlayerOverride,
    pub torrent: TorrentOverride,
}

/// [Override] as written in the file, before the "needs a condition" check.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
struct RawOverride {
    #[serde(rename = "SpEffect", default, deserialize_with = "sp_effect_ids")]
    sp_effect: Vec<i32>,
    #[serde(default, deserialize_with = "equip_load_classes")]
    equip_load: Vec<LoadClass>,
    #[serde(default)]
    player: PlayerOverride,
    #[serde(default)]
    torrent: TorrentOverride,
}

impl TryFrom<RawOverride> for Override {
    type Error = String;

    fn try_from(raw: RawOverride) -> Result<Self, String> {
        if raw.sp_effect.is_empty() && raw.equip_load.is_empty() {
            return Err(
                "an [[Override]] needs a condition: SpEffect = ... and/or EquipLoad = ...".to_string(),
            );
        }
        Ok(Override {
            sp_effect: raw.sp_effect,
            equip_load: raw.equip_load,
            player: raw.player,
            torrent: raw.torrent,
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct PlayerOverride {
    pub all: Option<f32>,
    pub walk: Option<f32>,
    pub run: Option<f32>,
    pub sneak: Option<f32>,
    pub jump: Option<f32>,
    pub roll: Option<f32>,
    pub ladder: Option<f32>,
    pub attack: Option<f32>,
    pub critical: Option<f32>,
    pub skill: Option<f32>,
    pub cast: Option<f32>,
    pub item: Option<f32>,
    pub other: Option<f32>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct TorrentOverride {
    pub all: Option<f32>,
    pub walk: Option<f32>,
    pub run: Option<f32>,
    pub jump: Option<f32>,
    pub other: Option<f32>,
}

impl Override {
    /// Writes every key this override sets into `speeds`.
    fn apply_to(&self, speeds: &mut Speeds) {
        let (o, p) = (&self.player, &mut speeds.player);
        let pairs = [
            (o.all, &mut p.all),
            (o.walk, &mut p.walk),
            (o.run, &mut p.run),
            (o.sneak, &mut p.sneak),
            (o.jump, &mut p.jump),
            (o.roll, &mut p.roll),
            (o.ladder, &mut p.ladder),
            (o.attack, &mut p.attack),
            (o.critical, &mut p.critical),
            (o.skill, &mut p.skill),
            (o.cast, &mut p.cast),
            (o.item, &mut p.item),
            (o.other, &mut p.other),
            (self.torrent.all, &mut speeds.torrent.all),
            (self.torrent.walk, &mut speeds.torrent.walk),
            (self.torrent.run, &mut speeds.torrent.run),
            (self.torrent.jump, &mut speeds.torrent.jump),
            (self.torrent.other, &mut speeds.torrent.other),
        ];
        for (value, slot) in pairs {
            if let Some(value) = value {
                *slot = value;
            }
        }
    }
}

impl Config {
    /// `[Player]` / `[Torrent]` with every override whose conditions hold
    /// (`has_sp_effect` for `SpEffect`, `load` for `EquipLoad`; `None` = class
    /// unknown, never matches an `EquipLoad`) applied on top, in file order, plus
    /// the indices of those overrides.
    pub fn effective_speed(
        &self,
        has_sp_effect: impl Fn(i32) -> bool,
        load: Option<LoadClass>,
    ) -> (Speeds, Vec<usize>) {
        let mut speeds = Speeds {
            player: self.player.clone(),
            torrent: self.torrent.clone(),
        };
        let mut active = Vec::new();
        for (index, o) in self.overrides.iter().enumerate() {
            let sp_effect_ok = o.sp_effect.is_empty() || o.sp_effect.iter().any(|&id| has_sp_effect(id));
            let load_ok = o.equip_load.is_empty() || load.is_some_and(|class| o.equip_load.contains(&class));
            if sp_effect_ok && load_ok {
                o.apply_to(&mut speeds);
                active.push(index);
            }
        }
        (speeds, active)
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
                return Err(A::Error::custom("SpEffect = [] is empty - list at least one id"));
            }
            Ok(ids)
        }
    }
    deserializer.deserialize_any(Ids)
}

/// `EquipLoad = "Heavy"` or `EquipLoad = ["Light", "Medium"]` (any case). An empty
/// list or an unknown name is an error.
fn equip_load_classes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<LoadClass>, D::Error> {
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    let names = match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(name) => vec![name],
        OneOrMany::Many(names) => names,
    };
    if names.is_empty() {
        return Err(D::Error::custom("EquipLoad = [] is empty - list at least one class"));
    }
    names
        .iter()
        .map(|name| {
            LoadClass::parse(name).ok_or_else(|| {
                D::Error::custom(format!(
                    "EquipLoad \"{name}\" is not a class - use Light, Medium, Heavy or Overweight"
                ))
            })
        })
        .collect()
}

const MIGRATION: Migration = Migration {
    version_key: "General.ConfigVersion",
    steps: STEPS,
    keep: &["Override"],
};

static CONFIG: OnceLock<TomlConfig<Config>> = OnceLock::new();

/// Loads (creating / bringing in line with the template) `path`. Call
/// once, first thing in `DllMain`'s thread; the report is for logging once
/// the logger is up.
pub fn init(path: &str) -> LoadReport {
    let (config, report) = TomlConfig::load_or_create(
        path,
        TEMPLATE,
        MIGRATION,
    );
    let _ = CONFIG.set(config);
    report
}

/// The config in use - take it once per tick.
pub fn get() -> Arc<Config> {
    match CONFIG.get() {
        Some(config) => config.get(),
        None => Arc::new(template()),
    }
}

/// The template's values - the defaults.
pub fn template() -> Config {
    common::toml_config::defaults(TEMPLATE)
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
    fn template_sets_every_field() {
        let missing = common::toml_config::template_missing_keys::<Config>(TEMPLATE, &["Override"]);
        assert!(missing.is_empty(), "template lacks {missing:?}");
    }

    /// Parsed the way the loader does (two passes over the template).
    fn parse_cfg(text: &str) -> Result<Config, String> {
        common::toml_config::parse(text, TEMPLATE)
    }

    #[test]
    fn missing_keys_fall_back_to_default() {
        let parsed: Config = parse_cfg("[Player]\nRoll = 2.0\n").unwrap();
        assert_eq!(parsed.player.roll, 2.0);
        assert_eq!(parsed.torrent.run, template().torrent.run);
        assert!(parsed.logging.log_file);
    }

    #[test]
    fn misspelled_key_is_an_error_with_its_line() {
        let err = parse_cfg("[Player]\nRol = 2.0\n").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Rol"), "{msg}");
        assert!(msg.contains("line 2"), "{msg}");
    }

    #[test]
    fn reload_key_takes_a_name_or_a_number() {
        let parsed: Config = parse_cfg("[General]\nReloadKey = \"0x74\"\n").unwrap();
        assert_eq!(parsed.general.reload_key, "0x74");
        let parsed: Config = parse_cfg("[General]\nReloadKey = 116\n").unwrap();
        assert_eq!(parsed.general.reload_key, "116");
        // A TOML hex literal is an integer too (user test, 2026-10-02).
        let parsed: Config = parse_cfg("[General]\nReloadKey = 0x75\n").unwrap();
        assert_eq!(common::input::parse_virtual_key(&parsed.general.reload_key, 0x74), 0x75);
    }

    fn has(ids: &'static [i32]) -> impl Fn(i32) -> bool {
        move |id| ids.contains(&id)
    }

    const OVERRIDES: &str = "
[[Override]]
SpEffect = [1111, 1112]
Player.Roll = 1.5

[[Override]]
SpEffect = 2222
Player.Attack = 1.6
Player.Roll = 2.0
Torrent.All = 2.5
";

    #[test]
    fn override_needs_any_of_its_sp_effects() {
        let config: Config = parse_cfg(OVERRIDES).unwrap();
        let (speeds, active) = config.effective_speed(has(&[1112]), None);
        assert_eq!(active, vec![0]);
        assert_eq!(speeds.player.roll, 1.5);
        assert_eq!(speeds.player.attack, template().player.attack);
        assert_eq!(speeds.torrent.all, template().torrent.all);
    }

    #[test]
    fn active_overrides_stack_and_the_later_one_wins() {
        let config: Config = parse_cfg(OVERRIDES).unwrap();
        let (speeds, active) = config.effective_speed(has(&[1111, 2222]), None);
        assert_eq!(active, vec![0, 1]);
        assert_eq!(speeds.player.roll, 2.0);
        assert_eq!(speeds.player.attack, 1.6);
        assert_eq!(speeds.player.walk, template().player.walk);
        assert_eq!(speeds.torrent.all, 2.5);
    }

    #[test]
    fn no_active_override_keeps_speed() {
        let config: Config = parse_cfg(OVERRIDES).unwrap();
        let (speeds, active) = config.effective_speed(has(&[]), None);
        assert!(active.is_empty());
        assert_eq!(speeds.player, template().player);
        assert_eq!(speeds.torrent, template().torrent);
    }

    #[test]
    fn override_as_sub_tables_works_too() {
        let config: Config = parse_cfg("[[Override]]\nSpEffect = 1\n[Override.Player]\nRun = 3\n").unwrap();
        assert_eq!(config.overrides[0].player.run, Some(3.0));
    }

    #[test]
    fn sp_effect_ids_as_strings_are_accepted() {
        let config: Config = parse_cfg("[[Override]]\nSpEffect = [\"1111\", 2222]\nTorrent.All = 2\n").unwrap();
        assert_eq!(config.overrides[0].sp_effect, vec![1111, 2222]);
        assert_eq!(config.overrides[0].torrent.all, Some(2.0));
    }

    const LOAD_OVERRIDES: &str = "
[[Override]]
EquipLoad = \"Heavy\"
Player.Walk = 0.9

[[Override]]
EquipLoad = [\"light\", \"Medium\"]
Player.Run = 1.1

[[Override]]
SpEffect = 5
EquipLoad = \"Heavy\"
Player.Roll = 0.8
";

    #[test]
    fn override_load_matches_its_class() {
        let config: Config = parse_cfg(LOAD_OVERRIDES).unwrap();
        let (speeds, active) = config.effective_speed(has(&[]), Some(LoadClass::Heavy));
        assert_eq!(active, vec![0]);
        assert_eq!(speeds.player.walk, 0.9);
        let (speeds, active) = config.effective_speed(has(&[]), Some(LoadClass::Light));
        assert_eq!(active, vec![1]);
        assert_eq!(speeds.player.run, 1.1);
        let (_, active) = config.effective_speed(has(&[]), Some(LoadClass::Overweight));
        assert!(active.is_empty());
        // Class unknown (0 = not loaded yet): an EquipLoad override never matches.
        let (_, active) = config.effective_speed(has(&[]), None);
        assert!(active.is_empty());
    }

    #[test]
    fn override_with_sp_effect_and_load_needs_both() {
        let config: Config = parse_cfg(LOAD_OVERRIDES).unwrap();
        let (speeds, active) = config.effective_speed(has(&[5]), Some(LoadClass::Heavy));
        assert_eq!(active, vec![0, 2]);
        assert_eq!(speeds.player.roll, 0.8);
        let (_, active) = config.effective_speed(has(&[5]), Some(LoadClass::Light));
        assert_eq!(active, vec![1]);
    }

    #[test]
    fn weight_type_maps_to_a_class() {
        assert_eq!(LoadClass::from_weight_type(0), None);
        assert_eq!(LoadClass::from_weight_type(1), Some(LoadClass::Light));
        assert_eq!(LoadClass::from_weight_type(4), Some(LoadClass::Overweight));
        assert_eq!(LoadClass::from_weight_type(5), None);
    }

    #[test]
    fn override_errors_are_reported() {
        for (text, needle) in [
            ("[[Override]]\nPlayer.Roll = 1.5\n", "needs a condition"),
            ("[[Override]]\nEquipLoad = []\n", "empty"),
            ("[[Override]]\nEquipLoad = \"Fat\"\n", "not a class"),
            ("[[Override]]\nSpEffect = []\n", "empty"),
            ("[[Override]]\nSpEffect = \"abc\"\n", "not a number"),
            ("[[Override]]\nSpEffect = 1\nPlayer.Rol = 1\n", "Rol"),
            ("[[Override]]\nSpEffect = 1\nRoll = 1\n", "Roll"),
        ] {
            let err = parse_cfg(text).unwrap_err();
            assert!(err.contains(needle), "{text:?} -> {err}");
        }
    }

    #[test]
    fn integer_value_is_accepted_as_a_speed() {
        // Users will write `All = 2`, not `2.0`.
        let parsed: Config = parse_cfg("[Torrent]\nAll = 2\n").unwrap();
        assert_eq!(parsed.torrent.all, 2.0);
    }

    #[test]
    fn steps_cover_every_version() {
        // The template's ConfigVersion is the current version: one step per
        // version after 1.
        let doc: toml::Value = toml::from_str(TEMPLATE).unwrap();
        let current = doc["General"]["ConfigVersion"].as_integer().unwrap();
        assert_eq!(STEPS.len() as i64, current - 1);
    }
}
